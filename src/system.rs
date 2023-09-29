use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    time::{Duration, Instant},
};

use anyhow::Result;
use arboard::Clipboard;
use cosmic_text::{FontSystem, SwashCache};
use log::warn;
use winit::{event_loop::EventLoopProxy, window::WindowId};

use crate::{
    callback::WidgetCallback,
    event_loop::UserEvent,
    style::computed::ComputedStyle,
    timer::{TimerId, Timers, WidgetTimer},
    widgets::{RawWidgetId, Widget, WidgetAddress, WidgetId},
    window::{Window, WindowRequest},
};

thread_local! {
    pub static SYSTEM: SharedSystemData = SharedSystemData(RefCell::new(None));
}

pub struct SharedSystemDataInner {
    pub address_book: HashMap<RawWidgetId, WidgetAddress>,
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,

    pub default_style: Rc<ComputedStyle>,
    pub event_loop_proxy: EventLoopProxy<UserEvent>,
    pub timers: Timers,
    pub clipboard: Clipboard,
    pub new_windows: Vec<Window>,
    pub exit_after_last_window_closes: bool,
}

pub struct SharedSystemData(pub RefCell<Option<SharedSystemDataInner>>);

const EMPTY_ERR: &str = "system not initialized yet";

pub fn address(id: RawWidgetId) -> Option<WidgetAddress> {
    with_system(|system| system.address_book.get(&id).cloned())
}

pub fn register_address(id: RawWidgetId, address: WidgetAddress) -> Option<WidgetAddress> {
    with_system(|system| system.address_book.insert(id, address))
}

pub fn unregister_address(id: RawWidgetId) -> Option<WidgetAddress> {
    with_system(|system| system.address_book.remove(&id))
}

pub fn with_system<R>(f: impl FnOnce(&mut SharedSystemDataInner) -> R) -> R {
    SYSTEM.with(|system| f(system.0.borrow_mut().as_mut().expect(EMPTY_ERR)))
}

pub fn send_window_request(window_id: WindowId, request: impl Into<WindowRequest>) {
    with_system(|system| {
        let _ = system
            .event_loop_proxy
            .send_event(UserEvent::WindowRequest(window_id, request.into()));
    });
}

pub fn add_timer<W: Widget, F>(duration: Duration, widget_id: WidgetId<W>, func: F) -> TimerId
where
    F: Fn(&mut W, Instant) -> Result<()> + 'static,
{
    add_timer_or_interval(duration, None, widget_id, func)
}

pub fn add_interval<W: Widget, F>(interval: Duration, widget_id: WidgetId<W>, func: F) -> TimerId
where
    F: Fn(&mut W, Instant) -> Result<()> + 'static,
{
    add_timer_or_interval(interval, Some(interval), widget_id, func)
}

pub fn add_timer_or_interval<W: Widget, F>(
    duration: Duration,
    interval: Option<Duration>,
    widget_id: WidgetId<W>,
    func: F,
) -> TimerId
where
    F: Fn(&mut W, Instant) -> Result<()> + 'static,
{
    with_system(|system| {
        system.timers.add(
            Instant::now() + duration,
            WidgetTimer {
                interval,
                callback: WidgetCallback::new(
                    widget_id.0,
                    Rc::new(move |widget, event| {
                        func(
                            widget.downcast_mut::<W>().expect("widget type mismatch"),
                            event,
                        )
                    }),
                ),
            },
        )
    })
}

pub fn report_error(error: impl Into<anyhow::Error>) {
    // TODO: display popup error message or custom hook
    warn!("{:?}", error.into());
}

pub trait ReportError {
    type Output;
    fn or_report_err(self) -> Option<Self::Output>;
}

impl<T, E> ReportError for Result<T, E>
where
    E: Into<anyhow::Error>,
{
    type Output = T;

    fn or_report_err(self) -> Option<Self::Output> {
        self.map_err(|err| report_error(err)).ok()
    }
}
