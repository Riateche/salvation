use {
    crate::{
        callback::{CallbackId, InvokeCallbackEvent},
        style::defaults::default_style,
        system::{
            address, take_pending_children_updates, with_system, ReportError,
            SharedSystemDataInner, SystemConfig, SYSTEM,
        },
        timer::Timers,
        widgets::{
            get_widget_by_address_mut, get_widget_by_id_mut, root::RootWidget, RawWidgetId, Widget,
            WidgetAddress, WidgetCommon, WidgetCreationContext, WidgetExt,
        },
        window::{WindowId, WindowRequest},
    },
    arboard::Clipboard,
    cosmic_text::{fontdb, FontSystem, SwashCache},
    derive_more::From,
    log::warn,
    scoped_tls::scoped_thread_local,
    std::{
        any::Any,
        collections::HashMap,
        fmt::Debug,
        path::PathBuf,
        time::{Duration, Instant},
    },
    winit::{
        application::ApplicationHandler,
        event::{StartCause, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    },
};

#[derive(Debug, From)]
pub(crate) enum UserEvent {
    InvokeCallback(InvokeCallbackEvent),
    WindowRequest(WindowId, WindowRequest),
    Accesskit(accesskit_winit::Event),
    DeleteWidget(RawWidgetId),
}

scoped_thread_local!(static ACTIVE_EVENT_LOOP: ActiveEventLoop);

pub(crate) fn with_active_event_loop<F, R>(f: F) -> R
where
    F: FnOnce(&ActiveEventLoop) -> R,
{
    ACTIVE_EVENT_LOOP.with(f)
}

fn dispatch_widget_callback(
    root_widget: &mut dyn Widget,
    callback_id: CallbackId,
    event: Box<dyn Any + Send>,
) {
    let Some(callback) = with_system(|s| s.widget_callbacks.get(&callback_id).cloned()) else {
        warn!("unknown widget callback id");
        return;
    };
    let Some(address) = address(callback.widget_id) else {
        return;
    };
    let Ok(widget) = get_widget_by_address_mut(root_widget, &address) else {
        return;
    };
    (callback.func)(widget, event).or_report_err();
    widget.update_accessible();
}

fn default_scale(event_loop: &ActiveEventLoop) -> f32 {
    let monitor = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next());
    if let Some(monitor) = monitor {
        monitor.scale_factor() as f32
    } else {
        warn!("unable to find any monitors");
        1.0
    }
}

pub struct App {
    system_fonts: bool,
    custom_font_paths: Vec<PathBuf>,
    fixed_scale: Option<f32>,
    auto_repeat_delay: Option<Duration>,
    auto_repeat_interval: Option<Duration>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> App {
        App {
            system_fonts: true,
            custom_font_paths: vec![],
            fixed_scale: None,
            auto_repeat_delay: None,
            auto_repeat_interval: None,
        }
    }

    pub fn with_system_fonts(mut self, enable: bool) -> App {
        self.system_fonts = enable;
        self
    }

    pub fn with_font(mut self, path: PathBuf) -> App {
        self.custom_font_paths.push(path);
        self
    }

    pub fn with_scale(mut self, scale: f32) -> App {
        self.fixed_scale = Some(scale);
        self
    }

    pub fn with_auto_repeat_delay(mut self, delay: Duration) -> App {
        self.auto_repeat_delay = Some(delay);
        self
    }

    pub fn with_auto_repeat_interval(mut self, interval: Duration) -> App {
        self.auto_repeat_interval = Some(interval);
        self
    }

    pub fn run(
        self,
        init: impl FnOnce(&mut RootWidget) -> anyhow::Result<()> + 'static,
    ) -> anyhow::Result<()> {
        let event_loop = EventLoop::<UserEvent>::with_user_event().build()?;
        let mut handler = Handler::new(self, &event_loop, init);
        event_loop.run_app(&mut handler)?;
        // Delete widgets before de-initializing the system.
        handler.root_widget = None;
        // This is needed to make sure we drop winit window objects before
        // event loop is destroyed.
        SYSTEM.with(|system| *system.0.borrow_mut() = None);
        Ok(())
    }
}

pub fn run(
    init: impl FnOnce(&mut RootWidget) -> anyhow::Result<()> + 'static,
) -> anyhow::Result<()> {
    App::new().run(init)
}

type BoxInitFn = Box<dyn FnOnce(&mut RootWidget) -> anyhow::Result<()>>;

struct Handler {
    app: App,
    is_initialized: bool,
    init: Option<BoxInitFn>,
    root_widget: Option<Box<dyn Widget>>,
    event_loop_proxy: Option<EventLoopProxy<UserEvent>>,
}

impl Handler {
    fn new(
        app: App,
        event_loop: &EventLoop<UserEvent>,
        init: impl FnOnce(&mut RootWidget) -> anyhow::Result<()> + 'static,
    ) -> Self {
        Self {
            app,
            init: Some(Box::new(init)),
            is_initialized: false,
            root_widget: None,
            event_loop_proxy: Some(event_loop.create_proxy()),
        }
    }

    fn before_handler(&mut self) {
        if self.is_initialized {
            while let Some(timer) = with_system(|system| system.timers.pop()) {
                timer.callback.invoke(Instant::now());
            }
        }
    }

    fn after_widget_activity(&mut self) {
        loop {
            let mut addrs = take_pending_children_updates();
            if addrs.is_empty() {
                break;
            }
            // Update upper layers first.
            addrs.sort_unstable_by_key(|addr| addr.len());
            for addr in addrs {
                if let Ok(widget) =
                    get_widget_by_address_mut(self.root_widget.as_mut().unwrap().as_mut(), &addr)
                {
                    widget.update_children();
                }
            }
        }
        let windows = with_system(|s| s.windows.clone());
        //println!("after widget activity1");
        for (_, window) in windows {
            //println!("root_widget_id {:?}", window.root_widget_id);
            if let Some(window_root_widget) = get_widget_by_id_mut(
                self.root_widget.as_mut().unwrap().as_mut(),
                window.root_widget_id,
            )
            .or_report_err()
            {
                window.with_root(window_root_widget).after_widget_activity();
            }
        }
        //println!("after widget activity1 ok");

        let exit = with_system(|s| {
            s.had_any_windows && s.windows.is_empty() && s.config.exit_after_last_window_closes
        });
        if exit {
            with_active_event_loop(|event_loop| event_loop.exit());
        }
    }
}

const DEFAULT_AUTO_REPEAT_DELAY: Duration = Duration::from_millis(500);
const DEFAULT_AUTO_REPEAT_INTERVAL: Duration = Duration::from_millis(50);

impl ApplicationHandler<UserEvent> for Handler {
    // TODO: It's recommended that applications should only initialize their graphics context
    // and create a window after they have received their first `Resumed` event.
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        ACTIVE_EVENT_LOOP.set(event_loop, || {
            self.before_handler();

            let Some(window) = with_system(|s| s.windows_by_winit_id.get(&window_id).cloned())
            else {
                if !matches!(event, WindowEvent::Destroyed | WindowEvent::RedrawRequested) {
                    warn!("missing window object when dispatching event: {:?}", event);
                }
                return;
            };
            let Some(root_widget) = &mut self.root_widget else {
                warn!(
                    "cannot dispatch event when root widget doesn't exist: {:?}",
                    event
                );
                return;
            };

            if let Some(window_root_widget) =
                get_widget_by_id_mut(root_widget.as_mut(), window.root_widget_id).or_report_err()
            {
                window.with_root(window_root_widget).handle_event(event);
            }

            self.after_widget_activity();
        })
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        ACTIVE_EVENT_LOOP.set(event_loop, || {
            self.before_handler();
            if cause == StartCause::Init {
                let mut db = fontdb::Database::new();
                for custom_font_path in &self.app.custom_font_paths {
                    if let Err(err) = db.load_font_file(custom_font_path) {
                        warn!(
                            "failed to initialize custom font from {:?}: {:?}",
                            custom_font_path, err
                        );
                    }
                }
                if self.app.system_fonts {
                    db.load_system_fonts();
                }
                let font_system =
                    FontSystem::new_with_locale_and_db(FontSystem::new().locale().to_string(), db);
                let scale = match self.app.fixed_scale {
                    None => default_scale(event_loop),
                    Some(fixed_scale) => fixed_scale,
                };

                let shared_system_data = SharedSystemDataInner {
                    config: SystemConfig {
                        exit_after_last_window_closes: true,
                        // TODO: should we fetch system settings instead?
                        auto_repeat_delay: self
                            .app
                            .auto_repeat_delay
                            .unwrap_or(DEFAULT_AUTO_REPEAT_DELAY),
                        auto_repeat_interval: self
                            .app
                            .auto_repeat_interval
                            .unwrap_or(DEFAULT_AUTO_REPEAT_INTERVAL),
                    },
                    address_book: HashMap::new(),
                    font_system,
                    swash_cache: SwashCache::new(),
                    event_loop_proxy: self.event_loop_proxy.take().expect("only happens once"),
                    // TODO: how to detect monitor scale change?
                    style: default_style(),
                    timers: Timers::new(),
                    clipboard: Clipboard::new().expect("failed to initialize clipboard"),
                    had_any_windows: false,
                    windows: HashMap::new(),
                    windows_by_winit_id: HashMap::new(),
                    widget_callbacks: HashMap::new(),
                    application_shortcuts: Vec::new(),
                    pending_children_updates: Vec::new(),
                    current_children_update: None,
                };
                SYSTEM.with(|system| {
                    *system.0.borrow_mut() = Some(shared_system_data);
                });

                let id = RawWidgetId::new_unique();
                let ctx = WidgetCreationContext {
                    parent_id: None,
                    address: WidgetAddress::root(id),
                    window: None,
                    // Scale doesn't matter for root widget. Window will set scale for its content.
                    parent_scale: scale,
                    is_parent_enabled: true,
                    is_window_root: false,
                };
                let mut root_widget = RootWidget::new(WidgetCommon::new(ctx));
                self.init.take().expect("double init")(&mut root_widget).or_report_err();
                self.root_widget = Some(Box::new(root_widget));

                self.is_initialized = true;
            }
            self.after_widget_activity();
        })
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        ACTIVE_EVENT_LOOP.set(event_loop, || {
            self.before_handler();
            let Some(root_widget) = &mut self.root_widget else {
                warn!(
                    "cannot dispatch event when root widget doesn't exist: {:?}",
                    event
                );
                return;
            };
            match event {
                UserEvent::WindowRequest(window_id, request) => {
                    let Some(window) = with_system(|s| s.windows.get(&window_id).cloned()) else {
                        warn!("missing window object when dispatching WindowRequest");
                        return;
                    };

                    let Ok(window_root_widget) =
                        get_widget_by_id_mut(root_widget.as_mut(), window.root_widget_id)
                    else {
                        warn!("missing root widget when dispatching WindowRequest");
                        return;
                    };
                    window.with_root(window_root_widget).handle_request(request);
                }
                // TODO: remove event, remove window directly
                UserEvent::InvokeCallback(event) => {
                    dispatch_widget_callback(root_widget.as_mut(), event.callback_id, event.event);
                }
                UserEvent::Accesskit(event) => {
                    let Some(window) =
                        with_system(|s| s.windows_by_winit_id.get(&event.window_id).cloned())
                    else {
                        warn!("missing window object when dispatching Accesskit event");
                        return;
                    };
                    let Ok(root_widget) =
                        get_widget_by_id_mut(root_widget.as_mut(), window.root_widget_id)
                    else {
                        warn!("missing root widget when dispatching Accesskit event");
                        return;
                    };
                    window.with_root(root_widget).handle_accesskit_event(event);
                }
                UserEvent::DeleteWidget(id) => {
                    if id == root_widget.common().id() {
                        self.root_widget = None;
                        with_active_event_loop(|event_loop| event_loop.exit());
                    } else if let Some(address) = address(id) {
                        if let Some(parent_id) = address.parent_widget_id() {
                            if let Ok(parent) =
                                get_widget_by_id_mut(root_widget.as_mut(), parent_id)
                            {
                                match parent
                                    .common_mut()
                                    .remove_child(&address.path.last().unwrap().0)
                                {
                                    Ok(_) => {}
                                    Err(err) => {
                                        warn!("failed to remove widget: {:?}", err);
                                    }
                                }
                            } else {
                                warn!("DeleteWidget: failed to get parent widget");
                            }
                        } else {
                            warn!("DeleteWidget: no parent");
                        }
                    } else {
                        warn!("DeleteWidget: no address");
                    }
                }
            }
            self.after_widget_activity();
        })
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        ACTIVE_EVENT_LOOP.set(event_loop, || {
            self.before_handler();
            let next_timer = with_system(|system| system.timers.next_instant());
            if let Some(next_timer) = next_timer {
                event_loop.set_control_flow(ControlFlow::WaitUntil(next_timer));
            } else {
                event_loop.set_control_flow(ControlFlow::Wait);
            }
            self.after_widget_activity();
        })
    }
}
