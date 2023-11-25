use std::{cmp::max, fmt::Display, rc::Rc};

use accesskit::{Action, DefaultActionVerb, NodeBuilder, Role};
use anyhow::Result;
use cosmic_text::Attrs;
use log::warn;
use salvation_macros::impl_with;
use tiny_skia::Pixmap;
use winit::event::MouseButton;

use crate::{
    callback::Callback,
    draw::DrawEvent,
    event::{
        AccessibleActionEvent, FocusReason, MouseEnterEvent, MouseInputEvent, MouseLeaveEvent,
        WidgetScopeChangeEvent,
    },
    style::button::{ButtonState, ComputedStyle, ComputedVariantStyle},
    system::send_window_request,
    text_editor::TextEditor,
    types::{Point, Rect},
    window::SetFocusRequest,
};

use super::{Widget, WidgetCommon};

// TODO: pub(crate)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role1 {
    Default,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollDown,
    ScrollGripX,
    ScrollGripY,
}

pub struct Button {
    editor: TextEditor,
    icon: Option<Rc<Pixmap>>,
    text_visible: bool,
    // TODO: Option inside callback
    on_clicked: Option<Callback<String>>,
    is_pressed: bool,
    common: WidgetCommon,
    role: Role1,
}

#[impl_with]
impl Button {
    pub fn new(text: impl Display) -> Self {
        let mut common = WidgetCommon::new();
        common.set_focusable(true);
        let mut editor = TextEditor::new(&text.to_string());
        editor.set_cursor_hidden(true);
        Self {
            editor,
            icon: None,
            text_visible: true,
            on_clicked: None,
            is_pressed: false,
            common,
            role: Role1::Default,
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.editor.set_text(&text.to_string(), Attrs::new());
        self.common.size_hint_changed();
        self.common.update();
    }

    pub fn set_text_visible(&mut self, value: bool) {
        self.text_visible = value;
        self.common.size_hint_changed();
        self.common.update();
    }

    // TODO: set_icon should preferably work with SVG icons
    // pub fn set_icon(&mut self, icon: Option<Rc<Pixmap>>) {
    //     self.icon = icon;
    //     self.common.size_hint_changed();
    //     self.common.update();
    // }

    pub fn on_clicked(&mut self, callback: Callback<String>) {
        self.on_clicked = Some(callback);
    }

    fn click(&mut self) {
        if let Some(on_clicked) = &self.on_clicked {
            on_clicked.invoke(self.editor.text());
        }
    }

    fn current_style(&self) -> &ComputedStyle {
        match self.role {
            Role1::Default => &self.common.style().button,
            Role1::ScrollLeft => &self.common.style().scroll_bar.scroll_left,
            Role1::ScrollRight => &self.common.style().scroll_bar.scroll_right,
            Role1::ScrollUp => &self.common.style().scroll_bar.scroll_up,
            Role1::ScrollDown => &self.common.style().scroll_bar.scroll_down,
            Role1::ScrollGripX => &self.common.style().scroll_bar.scroll_grip_x,
            Role1::ScrollGripY => &self.common.style().scroll_bar.scroll_grip_y,
        }
    }

    fn current_variant_style(&self) -> &ComputedVariantStyle {
        let state = if self.common.is_enabled() {
            ButtonState::Enabled {
                focused: self.common.is_focused(),
                mouse_over: self.common.is_mouse_over,
                pressed: self.is_pressed,
            }
        } else {
            ButtonState::Disabled
        };
        self.current_style().variants.get(&state).unwrap()
    }

    // TODO: pub(crate)
    pub fn set_role(&mut self, role: Role1) {
        self.role = role;
        self.icon = self.current_variant_style().icon.clone();
        self.common.set_focusable(role == Role1::Default);
        self.common.size_hint_changed();
        self.common.update();
    }
}

impl Widget for Button {
    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        let size = self.common.size_or_err()?;
        let style = self.current_variant_style().clone();

        event.stroke_and_fill_rounded_rect(
            Rect {
                top_left: Point::default(),
                size,
            },
            &style.border,
            style.background.as_ref(),
        );

        if self.text_visible {
            self.editor.set_text_color(style.text_color);
            let editor_pixmap = self.editor.pixmap();
            let padding = Point {
                x: max(0, size.x - editor_pixmap.width() as i32) / 2,
                y: max(0, size.y - editor_pixmap.height() as i32) / 2,
            };
            event.draw_pixmap(padding, editor_pixmap.as_ref());
        }

        // TODO: display icon and text side by side if both are present
        if let Some(icon) = &self.icon {
            let pos = Point {
                x: max(0, size.x - icon.width() as i32) / 2,
                y: max(0, size.y - icon.height() as i32) / 2,
            };
            event.draw_pixmap(pos, (**icon).as_ref());
        }
        Ok(())
    }

    fn handle_mouse_enter(&mut self, _event: MouseEnterEvent) -> Result<bool> {
        Ok(true)
    }

    fn handle_mouse_leave(&mut self, _event: MouseLeaveEvent) -> Result<()> {
        self.is_pressed = false;
        self.common.update();
        Ok(())
    }

    fn handle_mouse_input(&mut self, event: MouseInputEvent) -> Result<bool> {
        // TODO: only on release, check buttons
        if event.button() == MouseButton::Left {
            self.is_pressed = self.common.is_enabled() && event.state().is_pressed();
            if event.state().is_pressed() {
                self.click();
            }
            self.common.update();
        }

        let mount_point = &self
            .common
            .mount_point
            .as_ref()
            .expect("cannot handle event when unmounted");
        if self.role == Role1::Default {
            send_window_request(
                mount_point.address.window_id,
                SetFocusRequest {
                    widget_id: self.common.id,
                    reason: FocusReason::Mouse,
                },
            );
        }
        Ok(true)
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }
    fn handle_accessible_action(&mut self, event: AccessibleActionEvent) -> Result<()> {
        if self.role != Role1::Default {
            warn!("unexpected accessible action for role: {:?}", self.role);
            return Ok(());
        }
        let mount_point = &self
            .common
            .mount_point
            .as_ref()
            .expect("cannot handle event when unmounted");

        match event.action {
            Action::Default => self.click(),
            Action::Focus => {
                send_window_request(
                    mount_point.address.window_id,
                    SetFocusRequest {
                        widget_id: self.common.id,
                        // TODO: separate reason?
                        reason: FocusReason::Mouse,
                    },
                );
            }
            _ => {}
        }
        Ok(())
    }

    fn accessible_node(&mut self) -> Option<accesskit::NodeBuilder> {
        if self.role != Role1::Default {
            return None;
        }

        let mut node = NodeBuilder::new(Role::Button);
        node.set_name(self.editor.text().as_str());
        node.add_action(Action::Focus);
        //node.add_action(Action::Default);
        node.set_default_action_verb(DefaultActionVerb::Click);
        Some(node)
    }

    fn size_hint_x(&mut self) -> Result<i32> {
        let style = &self.common.style().button;

        // TODO: support text with icon
        let content_size = if self.text_visible {
            self.editor.size().x
        } else if let Some(icon) = &self.icon {
            icon.width() as i32
        } else {
            0
        };

        Ok(content_size + 2 * style.padding_with_border.x)
    }

    fn size_hint_y(&mut self, _size_x: i32) -> Result<i32> {
        // TODO: use size_x, handle multiple lines
        let style = &self.common.style().button;

        // TODO: support text with icon
        let content_size = if self.text_visible {
            self.editor.size().y
        } else if let Some(icon) = &self.icon {
            icon.height() as i32
        } else {
            0
        };

        Ok(content_size + 2 * style.padding_with_border.y)
    }

    fn handle_widget_scope_change(&mut self, _event: WidgetScopeChangeEvent) -> Result<()> {
        self.icon = self.current_variant_style().icon.clone();
        self.common.size_hint_changed();
        self.common.update();
        Ok(())
    }
}
