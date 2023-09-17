use std::{cmp::max, fmt::Display};

use accesskit::{Action, DefaultActionVerb, NodeBuilder, Role};
use cosmic_text::Attrs;
use tiny_skia::{Color, GradientStop, LinearGradient, SpreadMode, Transform};
use winit::event::MouseButton;

use crate::{
    callback::Callback,
    draw::DrawEvent,
    event::CursorMovedEvent,
    event::{AccessibleEvent, FocusReason, MouseInputEvent},
    layout::SizeHint,
    system::{send_window_request, with_system},
    text_editor::TextEditor,
    types::{Point, Rect},
    window::SetFocusRequest,
};

use super::{Widget, WidgetCommon};

pub struct Button {
    editor: TextEditor,
    // TODO: Option inside callback
    on_clicked: Option<Callback<String>>,
    state: ButtonState,
    enabled: bool,
    common: WidgetCommon,
}

#[derive(PartialEq)]
enum ButtonState {
    Default,
    Hover,
    Pressed,
}

const PADDING: Point = Point { x: 10, y: 5 };

impl Button {
    pub fn new(text: impl Display) -> Self {
        let mut common = WidgetCommon::new();
        common.is_focusable = true;
        let mut editor = TextEditor::new(&text.to_string());
        editor.set_cursor_hidden(true);
        let mut this = Self {
            editor,
            on_clicked: None,
            enabled: true,
            state: ButtonState::Default,
            common,
        };
        this.update_color();
        this
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.editor.set_text(&text.to_string(), Attrs::new());
    }

    //TODO: needs some automatic redraw?
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.update_color();
        }
    }

    pub fn on_clicked(&mut self, callback: Callback<String>) {
        self.on_clicked = Some(callback);
    }

    fn click(&mut self) {
        if let Some(on_clicked) = &self.on_clicked {
            on_clicked.invoke(self.editor.text());
        }
    }

    fn update_color(&mut self) {
        self.editor.set_text_color(if self.enabled {
            with_system(|system| system.palette.foreground)
        } else {
            Color::from_rgba8(191, 191, 191, 255)
        });
    }
}

impl Widget for Button {
    fn on_draw(&mut self, event: DrawEvent) {
        let start = tiny_skia::Point {
            x: event.rect.top_left.x as f32,
            y: event.rect.top_left.y as f32,
        };
        let end = tiny_skia::Point {
            x: event.rect.top_left.x as f32,
            y: event.rect.top_left.y as f32 + event.rect.size.y as f32,
        };
        let gradient = if !self.enabled {
            LinearGradient::new(
                start,
                end,
                vec![
                    GradientStop::new(0.0, Color::from_rgba8(254, 254, 254, 255)),
                    GradientStop::new(1.0, Color::from_rgba8(238, 238, 238, 255)),
                ],
                SpreadMode::Pad,
                Transform::default(),
            )
        } else {
            match self.state {
                ButtonState::Default => LinearGradient::new(
                    start,
                    end,
                    vec![
                        GradientStop::new(0.0, Color::from_rgba8(254, 254, 254, 255)),
                        GradientStop::new(1.0, Color::from_rgba8(238, 238, 238, 255)),
                    ],
                    SpreadMode::Pad,
                    Transform::default(),
                ),
                ButtonState::Hover => LinearGradient::new(
                    start,
                    end,
                    vec![
                        GradientStop::new(1.0, Color::from_rgba8(254, 254, 254, 255)),
                        GradientStop::new(1.0, Color::from_rgba8(247, 247, 247, 255)),
                    ],
                    SpreadMode::Pad,
                    Transform::default(),
                ),
                ButtonState::Pressed => LinearGradient::new(
                    start,
                    end,
                    vec![GradientStop::new(
                        1.0,
                        Color::from_rgba8(219, 219, 219, 255),
                    )],
                    SpreadMode::Pad,
                    Transform::default(),
                ),
            }
        }
        .expect("failed to create gradient");
        let border_color = if self.enabled {
            if self.common.is_focused {
                Color::from_rgba8(38, 112, 158, 255)
            } else {
                Color::from_rgba8(171, 171, 171, 255)
            }
        } else {
            Color::from_rgba8(196, 196, 196, 255)
        };
        event.stroke_and_fill_rounded_rect(
            Rect {
                top_left: Point::default(),
                size: event.rect.size,
            },
            2.0,
            1.0,
            gradient,
            border_color,
        );

        let editor_pixmap = self.editor.pixmap();
        let padding = Point {
            x: max(0, event.rect.size.x - editor_pixmap.width() as i32) / 2,
            y: max(0, event.rect.size.y - editor_pixmap.height() as i32) / 2,
        };
        event.draw_pixmap(padding, editor_pixmap.as_ref());
    }

    fn on_mouse_input(&mut self, event: MouseInputEvent) -> bool {
        // TODO: only on release, check buttons
        if event.button == MouseButton::Left {
            if event.state.is_pressed() {
                if self.enabled {
                    self.state = ButtonState::Pressed;
                    self.click();
                }
            } else if self.enabled {
                self.state = ButtonState::Hover;
            }
        }

        let mount_point = &self
            .common
            .mount_point
            .as_ref()
            .expect("cannot handle event when unmounted");
        send_window_request(
            mount_point.address.window_id,
            SetFocusRequest {
                widget_id: self.common.id,
                reason: FocusReason::Mouse,
            },
        );
        true
    }

    // TODO: mouse out event
    fn on_cursor_moved(&mut self, _event: CursorMovedEvent) -> bool {
        self.state = ButtonState::Hover;
        false
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }
    fn on_accessible(&mut self, event: AccessibleEvent) {
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
    }

    fn accessible_node(&mut self) -> Option<accesskit::NodeBuilder> {
        let mut node = NodeBuilder::new(Role::Button);
        node.set_name(self.editor.text().as_str());
        node.add_action(Action::Focus);
        //node.add_action(Action::Default);
        node.set_default_action_verb(DefaultActionVerb::Click);
        Some(node)
    }

    fn size_hint_x(&mut self) -> SizeHint {
        SizeHint {
            min: self.editor.size().x,
            preferred: self.editor.size().x + 2 * PADDING.x,
            is_fixed: true,
        }
    }

    fn size_hint_y(&mut self, _size_x: i32) -> SizeHint {
        // TODO: use size_x, handle multiple lines
        SizeHint {
            min: self.editor.size().y,
            preferred: self.editor.size().y + 2 * PADDING.y,
            is_fixed: true,
        }
    }
}
