use {
    crate::{
        accessible::AccessibleNodes,
        draw::DrawEvent,
        event::FocusReason,
        event_loop::with_active_event_loop,
        key::Key,
        system::with_system,
        types::{PhysicalPixels, Point, Rect, Size},
        widgets::{RawWidgetId, Widget, WidgetAddress, WidgetExt},
        window_handler::WindowInfo,
    },
    accesskit::NodeId,
    anyhow::{bail, Context},
    derivative::Derivative,
    derive_more::From,
    log::warn,
    std::{
        cell::RefCell,
        collections::HashSet,
        fmt::Display,
        mem,
        num::NonZeroU32,
        panic::catch_unwind,
        rc::Rc,
        sync::Mutex,
        time::{Duration, Instant},
    },
    tiny_skia::Pixmap,
    winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{ElementState, MouseButton, WindowEvent},
        keyboard::ModifiersState,
        window::{
            CursorIcon, Fullscreen, Icon, Theme, WindowAttributes, WindowButtons, WindowLevel,
        },
    },
};

// Extra size to avoid visual artifacts when resizing the window.
// Must be > 0 to avoid panic on pixmap creation.
const EXTRA_SURFACE_SIZE: u32 = 50;
// TODO: get system setting
const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy)]
pub enum MouseEventState {
    NotAccepted,
    AcceptedBy(RawWidgetId),
}

impl MouseEventState {
    pub fn is_accepted(self) -> bool {
        matches!(self, Self::AcceptedBy(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(RawWidgetId);

#[derive(Derivative)]
#[derivative(Debug)]
pub struct WindowInner {
    pub id: WindowId,
    pub root_widget_id: RawWidgetId,
    pub cursor_position: Option<Point>,
    pub cursor_entered: bool,
    pub modifiers_state: ModifiersState,
    pub pressed_mouse_buttons: HashSet<MouseButton>,
    pub is_window_focused: bool,
    pub accessible_nodes: AccessibleNodes,
    pub mouse_entered_widgets: Vec<(Rect, RawWidgetId)>,
    pub current_mouse_event_state: Option<MouseEventState>,
    pub pixmap: Rc<RefCell<Pixmap>>,
    // Drop order must be maintained as
    // `surface` -> `softbuffer_context` -> `winit_window`.
    #[derivative(Debug = "ignore")]
    pub surface: Option<softbuffer::Surface<Rc<winit::window::Window>, Rc<winit::window::Window>>>,
    #[derivative(Debug = "ignore")]
    pub softbuffer_context: Option<softbuffer::Context<Rc<winit::window::Window>>>,
    // Mutex provides unwind safety.
    #[derivative(Debug = "ignore")]
    pub accesskit_adapter: Option<Mutex<accesskit_winit::Adapter>>,
    pub winit_window: Option<Rc<winit::window::Window>>,
    pub min_inner_size: Size,
    pub preferred_inner_size: Size,
    pub input_method_enabled: bool,
    pub ime_cursor_area: Rect,

    pub pending_size_hint_invalidations: Vec<WidgetAddress>,
    pub pending_redraw: bool,
    pub pending_accessible_updates: Vec<WidgetAddress>,

    // TODO: refactor as struct
    pub focusable_widgets: Vec<(Vec<(Key, RawWidgetId)>, RawWidgetId)>,
    pub focusable_widgets_changed: bool,

    pub focused_widget: Option<(Vec<(Key, RawWidgetId)>, RawWidgetId)>,
    pub mouse_grabber_widget: Option<RawWidgetId>,
    pub num_clicks: u32,
    pub last_click_button: Option<MouseButton>,
    pub last_click_instant: Option<Instant>,
    pub is_delete_widget_on_close_enabled: bool,

    pub attributes: Attributes,
}

#[derive(Debug)]
pub struct Attributes {
    pub position: Option<Point>,
    pub resizable: bool,
    pub enabled_buttons: WindowButtons,
    pub title: Option<String>,
    pub maximized: bool,
    pub visible: bool,
    pub transparent: bool,
    pub blur: bool,
    pub decorations: bool,
    pub window_icon: Option<Icon>,
    pub preferred_theme: Option<Theme>,
    pub resize_increments: Option<Size>,
    pub content_protected: bool,
    pub window_level: WindowLevel,
    pub active: Option<bool>,
    pub fullscreen: Option<Fullscreen>,
    // TODO: more platform specific
    pub x11_window_type: Option<Vec<X11WindowType>>,
    pub skip_windows_taskbar: Option<bool>,
}

impl Default for Attributes {
    fn default() -> Self {
        Attributes {
            position: None,
            resizable: true,
            enabled_buttons: WindowButtons::all(),
            title: None,
            maximized: false,
            fullscreen: None,
            visible: true,
            transparent: false,
            blur: false,
            decorations: true,
            window_level: Default::default(),
            window_icon: None,
            preferred_theme: None,
            resize_increments: None,
            content_protected: false,
            active: None,
            x11_window_type: None,
            skip_windows_taskbar: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Window(Rc<RefCell<WindowInner>>);

impl Window {
    pub(crate) fn new(root_widget_id: RawWidgetId) -> Self {
        let window = Window(Rc::new(RefCell::new(WindowInner {
            id: WindowId(RawWidgetId::new_unique()),
            root_widget_id,
            cursor_position: None,
            cursor_entered: false,
            modifiers_state: ModifiersState::default(),
            pressed_mouse_buttons: HashSet::new(),
            is_window_focused: false,
            accessible_nodes: AccessibleNodes::new(),
            mouse_entered_widgets: Vec::new(),
            current_mouse_event_state: None,
            pixmap: Rc::new(RefCell::new(Pixmap::new(1, 1).unwrap())),
            surface: None,
            softbuffer_context: None,
            accesskit_adapter: None,
            winit_window: None,
            input_method_enabled: false,
            ime_cursor_area: Rect::default(),
            pending_size_hint_invalidations: Vec::new(),
            pending_redraw: false,
            pending_accessible_updates: Vec::new(),
            focusable_widgets: Vec::new(),
            focusable_widgets_changed: false,
            focused_widget: None,
            mouse_grabber_widget: None,
            num_clicks: 0,
            last_click_button: None,
            last_click_instant: None,
            is_delete_widget_on_close_enabled: true,
            // This is updated in `init_window`
            min_inner_size: Size::default(),
            // This is updated in `init_window`
            preferred_inner_size: Size::default(),
            attributes: Attributes::default(),
        })));

        let info = WindowInfo {
            id: window.id(),
            root_widget_id,
            shared_window_data: window.clone(),
        };
        // TODO: when to remove?
        with_system(|system| {
            system.windows.insert(info.id, info);
            system.had_any_windows = true;
        });

        window
    }

    pub fn has_winit_window(&self) -> bool {
        self.0.borrow().winit_window.is_some()
    }

    pub fn init_winit_window(&self, root_widget: &mut dyn Widget) {
        let mut inner_guard = self.0.borrow_mut();
        let inner = &mut *inner_guard;
        if inner.winit_window.is_some() {
            return;
        }

        let size_hints_x = root_widget.size_hint_x();
        // TODO: adjust size_x for screen size
        let preferred_size = Size::new(
            size_hints_x.preferred,
            root_widget.size_hint_y(size_hints_x.preferred).preferred,
        );
        let min_size = Size::new(
            size_hints_x.min,
            root_widget.size_hint_y(size_hints_x.min).min,
        );

        // TODO: all attrs
        let mut attrs = WindowAttributes::default()
            .with_inner_size(PhysicalSize::from(preferred_size))
            .with_min_inner_size(PhysicalSize::from(min_size))
            .with_resizable(inner.attributes.resizable)
            .with_enabled_buttons(inner.attributes.enabled_buttons)
            .with_maximized(inner.attributes.maximized)
            // Window must be hidden until we initialize accesskit
            .with_visible(false)
            .with_transparent(inner.attributes.transparent)
            .with_blur(inner.attributes.blur)
            .with_decorations(inner.attributes.decorations)
            .with_resizable(inner.attributes.resizable)
            .with_window_icon(inner.attributes.window_icon.clone())
            .with_theme(inner.attributes.preferred_theme)
            .with_content_protected(inner.attributes.content_protected)
            .with_window_level(inner.attributes.window_level)
            .with_fullscreen(inner.attributes.fullscreen.clone());

        if let Some(title) = &inner.attributes.title {
            attrs = attrs.with_title(title);
        }
        if let Some(active) = inner.attributes.active {
            attrs = attrs.with_active(active);
        }
        if let Some(position) = &inner.attributes.position {
            attrs = attrs.with_position(PhysicalPosition::from(*position));
        }
        if let Some(resize_increments) = &inner.attributes.resize_increments {
            attrs = attrs.with_resize_increments(PhysicalSize::from(*resize_increments));
        }
        #[cfg(all(unix, not(target_vendor = "apple")))]
        {
            use winit::platform::x11::WindowAttributesExtX11;

            if let Some(v) = &inner.attributes.x11_window_type {
                attrs = attrs.with_x11_window_type(v.iter().copied().map(Into::into).collect());
            }
        }
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;

            if let Some(v) = &inner.attributes.skip_windows_taskbar {
                attrs = attrs.with_skip_taskbar(v);
            }
        }
        let winit_window = Rc::new(with_active_event_loop(|event_loop| {
            event_loop.create_window(attrs).unwrap()
        }));
        let softbuffer_context = softbuffer::Context::new(winit_window.clone()).unwrap();
        let surface = softbuffer::Surface::new(&softbuffer_context, winit_window.clone()).unwrap();
        let accesskit_adapter = with_active_event_loop(|event_loop| {
            accesskit_winit::Adapter::with_event_loop_proxy(
                event_loop,
                &winit_window,
                with_system(|system| system.event_loop_proxy.clone()),
            )
        });
        winit_window.set_visible(inner.attributes.visible);
        let winit_id = winit_window.id();
        inner.winit_window = Some(winit_window);
        inner.softbuffer_context = Some(softbuffer_context);
        inner.surface = Some(surface);
        inner.accesskit_adapter = Some(Mutex::new(accesskit_adapter));
        drop(inner_guard);

        let info = WindowInfo {
            id: self.id(),
            root_widget_id: self.root_widget_id(),
            shared_window_data: self.clone(),
        };
        // TODO: when to remove?
        with_system(|system| {
            system.windows_by_winit_id.insert(winit_id, info);
        });
        self.set_visible(true);
        // For some reason it's necessary to request redraw again after initializing accesskit on Windows.
        self.clear_pending_redraw();
        self.request_redraw();
    }

    pub fn id(&self) -> WindowId {
        self.0.borrow().id
    }

    pub fn root_widget_id(&self) -> RawWidgetId {
        self.0.borrow().root_widget_id
    }

    pub(crate) fn accessible_root(&self) -> NodeId {
        self.0.borrow().accessible_nodes.root()
    }

    pub(crate) fn accessible_unmount(&self, parent: Option<NodeId>, child: NodeId) {
        self.0.borrow_mut().accessible_nodes.unmount(parent, child);
    }

    pub(crate) fn accessible_mount(
        &self,
        parent: Option<NodeId>,
        child: NodeId,
        key_in_parent: Key,
    ) {
        self.0
            .borrow_mut()
            .accessible_nodes
            .mount(parent, child, key_in_parent);
    }

    pub(crate) fn accessible_update(&self, id: NodeId, node: Option<accesskit::Node>) {
        self.0.borrow_mut().accessible_nodes.update(id, node);
    }

    pub fn mouse_grabber_widget(&self) -> Option<RawWidgetId> {
        self.0.borrow().mouse_grabber_widget
    }

    pub(crate) fn set_mouse_grabber_widget(&self, id: Option<RawWidgetId>) {
        self.0.borrow_mut().mouse_grabber_widget = id;
    }

    pub fn focused_widget(&self) -> Option<RawWidgetId> {
        self.0.borrow().focused_widget.as_ref().map(|x| x.1)
    }

    pub fn cursor_position(&self) -> Option<Point> {
        self.0.borrow().cursor_position
    }

    pub(crate) fn num_clicks(&self) -> u32 {
        self.0.borrow().num_clicks
    }

    pub(crate) fn any_mouse_buttons_pressed(&self) -> bool {
        !self.0.borrow().pressed_mouse_buttons.is_empty()
    }

    pub(crate) fn is_mouse_button_pressed(&self, button: MouseButton) -> bool {
        self.0.borrow().pressed_mouse_buttons.contains(&button)
    }

    pub fn is_delete_widget_on_close_enabled(&self) -> bool {
        self.0.borrow().is_delete_widget_on_close_enabled
    }

    pub fn set_visible(&self, visible: bool) {
        if let Some(w) = self.0.borrow().winit_window.as_ref() {
            w.set_visible(visible);
        }
    }

    pub(crate) fn set_cursor(&self, icon: CursorIcon) {
        if let Some(w) = self.0.borrow().winit_window.as_ref() {
            w.set_cursor(icon);
        }
    }

    pub fn modifiers(&self) -> ModifiersState {
        self.0.borrow().modifiers_state
    }

    pub(crate) fn set_modifiers(&self, modifiers: ModifiersState) {
        self.0.borrow_mut().modifiers_state = modifiers;
    }

    pub fn inner_size(&self) -> Size {
        self.0
            .borrow()
            .winit_window
            .as_ref()
            .map(|w| w.inner_size())
            .unwrap_or_default()
            .into()
    }

    pub fn min_inner_size(&self) -> Size {
        self.0.borrow().min_inner_size
    }

    pub fn preferred_inner_size(&self) -> Size {
        self.0.borrow().preferred_inner_size
    }

    pub(crate) fn set_min_inner_size(&self, size: Size) {
        let this = &mut *self.0.borrow_mut();
        if size != this.min_inner_size {
            if let Some(w) = this.winit_window.as_ref() {
                w.set_min_inner_size(Some(PhysicalSize::from(size)));
            }
            this.min_inner_size = size;
        }
    }

    pub fn set_preferred_inner_size(&self, size: Size) {
        self.0.borrow_mut().preferred_inner_size = size;
    }

    pub fn request_inner_size(&self, size: Size) -> Option<Size> {
        self.0
            .borrow()
            .winit_window
            .as_ref()
            .and_then(|w| w.request_inner_size(PhysicalSize::from(size)))
            .map(Into::into)
    }

    pub(crate) fn clear_pending_redraw(&self) {
        self.0.borrow_mut().pending_redraw = false;
    }

    pub fn is_focused(&self) -> bool {
        self.0.borrow().is_window_focused
    }

    pub(crate) fn pass_event_to_accesskit(&self, event: &WindowEvent) {
        let this = &mut *self.0.borrow_mut();
        if let Some(adapter) = this.accesskit_adapter.as_ref() {
            let Ok(mut adapter) = adapter.lock() else {
                return;
            };
            adapter.process_event(this.winit_window.as_ref().unwrap(), event);
        }
    }

    pub(crate) fn prepare_draw(&self) -> Option<DrawEvent> {
        let this = &mut *self.0.borrow_mut();
        if this.winit_window.is_none() {
            warn!("cannot draw without a window");
            return None;
        }
        let (width, height) = {
            let size = this.winit_window.as_ref().unwrap().inner_size();
            (
                size.width + EXTRA_SURFACE_SIZE,
                size.height + EXTRA_SURFACE_SIZE,
            )
        };

        this.surface
            .as_mut()
            .unwrap()
            .resize(
                NonZeroU32::new(width).unwrap(),
                NonZeroU32::new(height).unwrap(),
            )
            .unwrap();

        {
            let mut pixmap = this.pixmap.borrow_mut();
            if pixmap.width() != width || pixmap.height() != height {
                *pixmap = Pixmap::new(width, height).unwrap();
            }
        }

        if !this.pending_redraw {
            return None;
        }
        let draw_event = DrawEvent::new(
            Rc::clone(&this.pixmap),
            Point::default(),
            Rect::from_pos_size(
                Point::default(),
                Size::new(
                    PhysicalPixels::from_i32(width as i32),
                    PhysicalPixels::from_i32(height as i32),
                ),
            ),
        );
        // TODO: option to turn off background, allow customizing with classes or inline style
        let color = with_system(|system| system.style.root_background_color());
        this.pixmap.borrow_mut().fill(color);
        this.pending_redraw = false;
        Some(draw_event)
    }

    pub(crate) fn finalize_draw(&self) {
        let this = &mut *self.0.borrow_mut();
        if this.winit_window.is_none() {
            warn!("cannot draw without a window");
            return;
        }
        let mut buffer = this.surface.as_mut().unwrap().buffer_mut().unwrap();
        buffer.copy_from_slice(bytemuck::cast_slice(this.pixmap.borrow().data()));

        // tiny-skia uses an RGBA format, while softbuffer uses XRGB. To convert, we need to
        // iterate over the pixels and shift the pixels over.
        buffer.iter_mut().for_each(|pixel| {
            let [r, g, b, _] = pixel.to_ne_bytes();
            *pixel = (b as u32) | ((g as u32) << 8) | ((r as u32) << 16);
        });

        buffer.present().unwrap();
    }

    pub(crate) fn cursor_entered(&self) {
        let this = &mut *self.0.borrow_mut();
        this.cursor_entered = true;
    }

    pub(crate) fn cursor_left(&self) {
        let this = &mut *self.0.borrow_mut();
        this.cursor_entered = false;
        this.cursor_position = None;
    }

    pub(crate) fn cursor_moved(&self, pos_in_window: Point) -> bool {
        let this = &mut *self.0.borrow_mut();
        if this.cursor_position != Some(pos_in_window) {
            this.cursor_position = Some(pos_in_window);
            true
        } else {
            false
        }
    }

    pub(crate) fn mouse_input(&self, state: ElementState, button: MouseButton) {
        let this = &mut *self.0.borrow_mut();
        match state {
            ElementState::Pressed => {
                this.pressed_mouse_buttons.insert(button);
                let had_recent_click = this
                    .last_click_instant
                    .is_some_and(|last| last.elapsed() < DOUBLE_CLICK_TIMEOUT);
                let same_button = this.last_click_button == Some(button);
                if had_recent_click && same_button {
                    this.num_clicks += 1;
                } else {
                    this.num_clicks = 1;
                    this.last_click_button = Some(button);
                }
                this.last_click_instant = Some(Instant::now());
            }
            ElementState::Released => {
                this.pressed_mouse_buttons.remove(&button);
            }
        }
    }

    pub(crate) fn is_mouse_entered(&self, id: RawWidgetId) -> bool {
        self.0
            .borrow()
            .mouse_entered_widgets
            .iter()
            .any(|(_, x)| *x == id)
    }

    pub(crate) fn add_mouse_entered(&self, rect: Rect, id: RawWidgetId) {
        self.0.borrow_mut().mouse_entered_widgets.push((rect, id));
    }

    pub(crate) fn ime_enabled(&self) {
        let this = &*self.0.borrow();
        if let Some(w) = this.winit_window.as_ref() {
            w.set_ime_cursor_area(
                PhysicalPosition::from(this.ime_cursor_area.top_left()),
                PhysicalSize::from(this.ime_cursor_area.size()),
            );
        }
    }

    pub(crate) fn focus_changed(&self, focused: bool) -> bool {
        let this = &mut *self.0.borrow_mut();
        this.is_window_focused = focused;
        if !focused && this.mouse_grabber_widget.is_some() {
            this.mouse_grabber_widget = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn pop_mouse_entered_widget(&self) -> Option<RawWidgetId> {
        let this = &mut *self.0.borrow_mut();
        let pos = this.cursor_position;
        let list = &mut this.mouse_entered_widgets;
        let index = list.iter().position(|(rect, id)| {
            pos.is_none_or(|pos| !rect.contains(pos)) && Some(*id) != this.mouse_grabber_widget
        })?;
        Some(list.remove(index).1)
    }

    pub fn set_ime_cursor_area(&self, rect: Rect) {
        let mut this = self.0.borrow_mut();
        if this.ime_cursor_area != rect {
            if let Some(w) = this.winit_window.as_ref() {
                w.set_ime_cursor_area(
                    PhysicalPosition::from(rect.top_left()),
                    PhysicalSize::from(rect.size()),
                );
            } //TODO: actual size
            this.ime_cursor_area = rect;
        }
    }

    // TODO: should there be a proper way to do this in winit?
    pub fn cancel_ime_preedit(&self) {
        let this = self.0.borrow();
        if this.input_method_enabled {
            if let Some(w) = this.winit_window.as_ref() {
                w.set_ime_allowed(false);
                w.set_ime_allowed(true);
            }
        }
    }

    // Only for the case when ime_allowed is changed for the focused widget.
    pub fn set_ime_allowed(&self, allowed: bool) {
        let this = self.0.borrow();
        if let Some(w) = this.winit_window.as_ref() {
            w.set_ime_allowed(allowed);
        }
    }

    pub fn request_redraw(&self) {
        let mut this = self.0.borrow_mut();
        if !this.pending_redraw {
            this.pending_redraw = true;
            if let Some(w) = this.winit_window.as_ref() {
                w.request_redraw();
            }
        }
    }

    pub fn add_focusable_widget(&self, addr: WidgetAddress, id: RawWidgetId) {
        let mut this = self.0.borrow_mut();
        let Some(relative_addr) = addr.strip_prefix(this.root_widget_id) else {
            warn!("add_focusable_widget: address outside root");
            return;
        };
        let pair = (relative_addr.to_vec(), id);
        if let Err(index) = this.focusable_widgets.binary_search(&pair) {
            this.focusable_widgets.insert(index, pair);
            this.focusable_widgets_changed = true;
        }
    }

    pub fn remove_focusable_widget(&self, addr: WidgetAddress, id: RawWidgetId) {
        let mut this = self.0.borrow_mut();
        let Some(relative_addr) = addr.strip_prefix(this.root_widget_id) else {
            warn!("remove_focusable_widget: address outside root");
            return;
        };
        let pair = (relative_addr.to_vec(), id);
        if let Ok(index) = this.focusable_widgets.binary_search(&pair) {
            this.focusable_widgets.remove(index);
            this.focusable_widgets_changed = true;
        }
    }

    pub(crate) fn move_keyboard_focus(
        &self,
        direction: i32,
    ) -> Option<(Vec<(Key, RawWidgetId)>, RawWidgetId)> {
        let this = self.0.borrow();
        let focused_widget = this.focused_widget.as_ref()?;
        if this.focusable_widgets.is_empty() {
            None
        } else if let Ok(index) = this.focusable_widgets.binary_search(focused_widget) {
            let new_index =
                (index as i32 + direction).rem_euclid(this.focusable_widgets.len() as i32);
            Some(this.focusable_widgets[new_index as usize].clone())
        } else {
            warn!("focused widget is unknown");
            this.focusable_widgets.first().cloned()
        }
    }

    pub(crate) fn pending_auto_focus(&self) -> Option<(Vec<(Key, RawWidgetId)>, RawWidgetId)> {
        let this = &*self.0.borrow();
        if this.focused_widget.is_none() {
            this.focusable_widgets.first().cloned()
        } else {
            None
        }
    }

    pub(crate) fn push_accessible_updates(&mut self) {
        let this = &mut *self.0.borrow_mut();
        if let Some(adapter) = this.accesskit_adapter.as_ref() {
            let update = this.accessible_nodes.take_update();
            let r = catch_unwind(|| {
                adapter
                    .lock()
                    .expect("accesskit adapter mutex is poisoned")
                    .update_if_active(|| update);
            });
            if let Err(err) = r {
                let err_msg = err
                    .downcast_ref::<&'static str>()
                    .map(|s| s.to_string())
                    .or_else(|| err.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| format!("{err:?}"));
                warn!("accesskit panicked: {err_msg}");
                this.accesskit_adapter = None;
            }
        }
    }

    pub(crate) fn invalidate_size_hint(&self, addr: WidgetAddress) {
        let this = &mut *self.0.borrow_mut();
        this.pending_size_hint_invalidations.push(addr);
    }

    pub(crate) fn take_pending_size_hint_invalidations(&self) -> Vec<WidgetAddress> {
        mem::take(&mut self.0.borrow_mut().pending_size_hint_invalidations)
    }

    pub(crate) fn request_accessible_update(&self, addr: WidgetAddress) {
        let this = &mut *self.0.borrow_mut();
        this.pending_accessible_updates.push(addr);
    }

    pub(crate) fn take_pending_accessible_updates(&self) -> Vec<WidgetAddress> {
        mem::take(&mut self.0.borrow_mut().pending_accessible_updates)
    }

    pub(crate) fn unset_focus(&self) -> Option<(Vec<(Key, RawWidgetId)>, RawWidgetId)> {
        let mut this = self.0.borrow_mut();
        let old = this.focused_widget.take();
        if let Some(w) = this.winit_window.as_ref() {
            w.set_ime_allowed(false);
        }
        this.input_method_enabled = false;
        this.accessible_nodes.set_focus(None);
        old
    }

    pub(crate) fn set_focus(
        &self,
        addr_id: (Vec<(Key, RawWidgetId)>, RawWidgetId),
        input_method_enabled: bool,
    ) {
        let mut this = self.0.borrow_mut();
        this.accessible_nodes.set_focus(Some(addr_id.1.into()));
        this.focused_widget = Some(addr_id);
        if let Some(w) = this.winit_window.as_ref() {
            w.set_ime_allowed(input_method_enabled);
        }
        this.input_method_enabled = input_method_enabled;
    }

    pub(crate) fn is_registered_as_focusable(
        &self,
        addr_id: &(Vec<(Key, RawWidgetId)>, RawWidgetId),
    ) -> bool {
        let this = &*self.0.borrow();
        this.focusable_widgets.binary_search(addr_id).is_ok()
    }

    pub(crate) fn focused_widget_is_focusable(&self) -> bool {
        let this = &*self.0.borrow();
        if let Some(focused_widget) = &this.focused_widget {
            this.focusable_widgets.binary_search(focused_widget).is_ok()
        } else {
            true
        }
    }

    pub(crate) fn focusable_widgets_changed(&self) -> bool {
        let this = &*self.0.borrow();
        this.focusable_widgets_changed
    }

    pub(crate) fn clear_focusable_widgets_changed(&self) {
        let this = &mut *self.0.borrow_mut();
        this.focusable_widgets_changed = false;
    }

    pub(crate) fn current_mouse_event_state(&self) -> anyhow::Result<MouseEventState> {
        let this = &*self.0.borrow();
        this.current_mouse_event_state
            .context("no current mouse event")
    }

    pub(crate) fn accept_current_mouse_event(&self, widget_id: RawWidgetId) -> anyhow::Result<()> {
        let this = &mut *self.0.borrow_mut();
        if let Some(state) = &mut this.current_mouse_event_state {
            if let MouseEventState::NotAccepted = state {
                *state = MouseEventState::AcceptedBy(widget_id);
                Ok(())
            } else {
                bail!("event already accepted");
            }
        } else {
            bail!("no current mouse event");
        }
    }

    pub(crate) fn init_mouse_event_state(&self) -> anyhow::Result<()> {
        let this = &mut *self.0.borrow_mut();
        if this.current_mouse_event_state.is_none() {
            this.current_mouse_event_state = Some(MouseEventState::NotAccepted);
            Ok(())
        } else {
            bail!("window already has another current mouse event");
        }
    }

    pub(crate) fn take_mouse_event_state(&self) -> anyhow::Result<MouseEventState> {
        let this = &mut *self.0.borrow_mut();
        this.current_mouse_event_state
            .take()
            .context("no current mouse event")
    }

    pub fn set_title(&self, title: impl Display) {
        let this = &mut *self.0.borrow_mut();
        let title = title.to_string();
        if Some(&title) == this.attributes.title.as_ref() {
            return;
        }
        if let Some(window) = &this.winit_window {
            window.set_title(&title);
        }
        this.attributes.title = Some(title);
    }

    pub fn set_decorations(&self, value: bool) {
        let this = &mut *self.0.borrow_mut();
        if value == this.attributes.decorations {
            return;
        }
        if let Some(window) = &this.winit_window {
            window.set_decorations(value);
        }
        this.attributes.decorations = value;
    }

    pub fn set_window_level(&self, value: WindowLevel) {
        let this = &mut *self.0.borrow_mut();
        if value == this.attributes.window_level {
            return;
        }
        if let Some(window) = &this.winit_window {
            window.set_window_level(value);
        }
        this.attributes.window_level = value;
    }

    #[allow(unused_variables)]
    pub fn set_x11_window_type(&self, value: Vec<X11WindowType>) {
        #[cfg(all(unix, not(target_vendor = "apple")))]
        {
            let this = &mut *self.0.borrow_mut();
            if Some(&value) == this.attributes.x11_window_type.as_ref() {
                return;
            }
            if this.winit_window.is_some() {
                warn!("changing x11 window type after window creation is unsupported");
            }
            this.attributes.x11_window_type = Some(value);
        }
    }

    #[allow(unused_variables)]
    pub fn set_skip_windows_taskbar(&self, value: bool) {
        #[cfg(windows)]
        {
            let this = &mut *self.0.borrow_mut();
            if Some(value) == this.attributes.skip_windows_taskbar {
                return;
            }
            if let Some(window) = &this.winit_window {
                window.set_skip_windows_taskbar(value);
            }
            this.attributes.skip_windows_taskbar = Some(value);
        }
    }

    pub fn deregister(&self) {
        let this = self.0.borrow();
        let id = this.id;
        let winit_id = this.winit_window.as_ref().map(|w| w.id());
        drop(this);
        with_system(|system| {
            system.windows.remove(&id);
            if let Some(winit_id) = winit_id {
                system.windows_by_winit_id.remove(&winit_id);
            }
        });
    }
}

#[derive(Debug, From)]
pub enum WindowRequest {
    SetFocus(SetFocusRequest),
    ScrollToRect(ScrollToRectRequest),
}

#[derive(Debug)]
pub struct SetFocusRequest {
    pub widget_id: RawWidgetId,
    pub reason: FocusReason,
}

#[derive(Debug)]
pub struct ScrollToRectRequest {
    pub widget_id: RawWidgetId,
    // In widget coordinates.
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum X11WindowType {
    Desktop,
    Dock,
    Toolbar,
    Menu,
    Utility,
    Splash,
    Dialog,
    DropdownMenu,
    PopupMenu,
    Tooltip,
    Notification,
    Combo,
    Dnd,
    Normal,
}

#[cfg(all(unix, not(target_vendor = "apple")))]
impl From<X11WindowType> for winit::platform::x11::WindowType {
    fn from(value: X11WindowType) -> Self {
        match value {
            X11WindowType::Desktop => winit::platform::x11::WindowType::Desktop,
            X11WindowType::Dock => winit::platform::x11::WindowType::Dock,
            X11WindowType::Toolbar => winit::platform::x11::WindowType::Toolbar,
            X11WindowType::Menu => winit::platform::x11::WindowType::Menu,
            X11WindowType::Utility => winit::platform::x11::WindowType::Utility,
            X11WindowType::Splash => winit::platform::x11::WindowType::Splash,
            X11WindowType::Dialog => winit::platform::x11::WindowType::Dialog,
            X11WindowType::DropdownMenu => winit::platform::x11::WindowType::DropdownMenu,
            X11WindowType::PopupMenu => winit::platform::x11::WindowType::PopupMenu,
            X11WindowType::Tooltip => winit::platform::x11::WindowType::Tooltip,
            X11WindowType::Notification => winit::platform::x11::WindowType::Notification,
            X11WindowType::Combo => winit::platform::x11::WindowType::Combo,
            X11WindowType::Dnd => winit::platform::x11::WindowType::Dnd,
            X11WindowType::Normal => winit::platform::x11::WindowType::Normal,
        }
    }
}
