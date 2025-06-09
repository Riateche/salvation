#![allow(clippy::new_without_default)]

pub use crate::draw::DrawEvent;
use crate::widgets::WidgetGeometry;
use {
    crate::{
        types::{Point, Rect},
        widgets::{WidgetAddress, WidgetCommon},
    },
    accesskit::{Action, ActionData},
    derive_more::From,
    winit::{
        dpi::PhysicalPosition,
        event::{DeviceId, ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, TouchPhase},
        keyboard::ModifiersState,
    },
};

#[derive(Debug, Clone, From)]
pub enum Event {
    MouseInput(MouseInputEvent),
    MouseScroll(MouseScrollEvent),
    MouseEnter(MouseEnterEvent),
    MouseMove(MouseMoveEvent),
    MouseLeave(MouseLeaveEvent),
    KeyboardInput(KeyboardInputEvent),
    InputMethod(InputMethodEvent),
    Draw(DrawEvent),
    Layout(LayoutEvent),
    FocusIn(FocusInEvent),
    FocusOut(FocusOutEvent),
    // TODO: use a callback instead
    WindowFocusChange(WindowFocusChangeEvent),
    AccessibilityAction(AccessibilityActionEvent),
    StyleChange(StyleChangeEvent),
}

#[derive(Debug, Clone)]
pub struct MouseInputEvent {
    pub device_id: DeviceId,
    pub state: ElementState,
    pub button: MouseButton,
    pub num_clicks: u32,
    /// Position in widget coordinates
    pub pos: Point,
    pub pos_in_window: Point,
}

impl MouseInputEvent {
    pub fn map_to_child(&self, rect_in_parent: Rect, force: bool) -> Option<Self> {
        if force || rect_in_parent.contains(self.pos) {
            let mut event = self.clone();
            event.pos -= rect_in_parent.top_left();
            Some(event)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct MouseScrollEvent {
    pub device_id: DeviceId,
    pub delta: MouseScrollDelta,
    pub touch_phase: TouchPhase,
    /// Position in widget coordinates
    pub pos: Point,
    pub pos_in_window: Point,
}

impl MouseScrollEvent {
    pub fn map_to_child(&self, rect_in_parent: Rect, force: bool) -> Option<Self> {
        if force || rect_in_parent.contains(self.pos) {
            let mut event = self.clone();
            event.pos -= rect_in_parent.top_left();
            Some(event)
        } else {
            None
        }
    }

    pub fn unified_delta(&self, widget_common: &WidgetCommon) -> PhysicalPosition<f64> {
        match self.delta {
            MouseScrollDelta::LineDelta(dx, dy) => {
                let line_height = widget_common.common_style.font_metrics.line_height;
                PhysicalPosition::new((line_height * dx).into(), (line_height * dy).into())
            }
            MouseScrollDelta::PixelDelta(delta) => delta,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MouseMoveEvent {
    pub device_id: DeviceId,
    /// Position in widget coordinates
    pub pos: Point,
    pub pos_in_window: Point,
}

impl MouseMoveEvent {
    pub fn map_to_child(&self, rect_in_parent: Rect, force: bool) -> Option<Self> {
        if force || rect_in_parent.contains(self.pos) {
            let mut event = self.clone();
            event.pos -= rect_in_parent.top_left();
            Some(event)
        } else {
            None
        }
    }

    pub fn create_enter_event(&self) -> MouseEnterEvent {
        MouseEnterEvent {
            device_id: self.device_id,
            pos: self.pos,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MouseEnterEvent {
    pub device_id: DeviceId,
    pub pos: Point,
}

#[derive(Debug, Clone)]
pub struct MouseLeaveEvent {}

#[derive(Debug, Clone)]
pub struct KeyboardInputEvent {
    pub device_id: DeviceId,
    pub info: KeyEvent,
    pub is_synthetic: bool,
    pub modifiers: ModifiersState,
}

#[derive(Debug, Clone)]
pub struct InputMethodEvent {
    pub info: Ime,
}

#[derive(Debug, Clone)]
pub struct LayoutEvent {
    // None means widget is hidden
    pub new_geometry: Option<WidgetGeometry>,
    // TODO: Rc?
    pub changed_size_hints: Vec<WidgetAddress>,
}

impl LayoutEvent {
    pub fn size_hints_changed_within(&self, addr: &WidgetAddress) -> bool {
        self.changed_size_hints
            .iter()
            .any(|changed| changed.starts_with(addr))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusReason {
    Mouse,
    Tab,
    /// A widget was automatically focused because there was no focused widget previously.
    Auto,
}

#[derive(Debug, Clone)]
pub struct FocusInEvent {
    pub reason: FocusReason,
}

#[derive(Debug, Clone)]
pub struct FocusOutEvent {}

#[derive(Debug, Clone)]
pub struct WindowFocusChangeEvent {
    pub is_window_focused: bool,
}

#[derive(Debug, Clone)]
pub struct AccessibilityActionEvent {
    pub action: Action,
    pub data: Option<ActionData>,
}

#[derive(Debug, Clone)]
pub struct ScrollToRectRequest {
    pub address: WidgetAddress,
    pub rect: Rect,
}

#[derive(Debug, Clone)]
pub struct StyleChangeEvent {}
