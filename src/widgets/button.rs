use std::{cmp::max, fmt::Display};

use cosmic_text::{Attrs, Buffer, Shaping};
use tiny_skia::{Color, GradientStop, LinearGradient, Pixmap, SpreadMode, Transform};
use winit::event::MouseButton;

use crate::{
    callback::Callback,
    draw::{draw_text, unrestricted_text_size, DrawEvent},
    event::{CursorMovedEvent, MouseInputEvent},
    system::with_system,
    types::{Point, Rect, Size},
};

use super::{Widget, WidgetCommon};

pub struct Button {
    text: String,
    buffer: Option<Buffer>,
    text_pixmap: Option<Pixmap>,
    unrestricted_text_size: Size,
    redraw_text: bool,
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

impl Button {
    pub fn new(text: impl Display) -> Self {
        let mut common = WidgetCommon::new();
        common.is_focusable = true;
        Self {
            text: text.to_string(),
            buffer: None,
            text_pixmap: None,
            unrestricted_text_size: Size::default(),
            redraw_text: true,
            on_clicked: None,
            enabled: false,
            state: ButtonState::Default,
            common,
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.text = text.to_string();
        self.redraw_text = true;
    }

    //TODO: needs some automatic redraw?
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
        }
    }

    pub fn on_clicked(&mut self, callback: Callback<String>) {
        self.on_clicked = Some(callback);
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
            Color::from_rgba8(171, 171, 171, 255)
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

        with_system(|system| {
            let mut buffer = self
                .buffer
                .get_or_insert_with(|| Buffer::new(&mut system.font_system, system.font_metrics))
                .borrow_with(&mut system.font_system);

            let text_color = if self.enabled {
                system.palette.foreground
            } else {
                Color::from_rgba8(191, 191, 191, 255)
            };

            if self.redraw_text {
                buffer.set_text(&self.text, Attrs::new(), Shaping::Advanced);
                self.unrestricted_text_size = unrestricted_text_size(&mut buffer);
                let pixmap = draw_text(
                    &mut buffer,
                    self.unrestricted_text_size,
                    text_color,
                    &mut system.swash_cache,
                );
                self.text_pixmap = Some(pixmap);
                self.redraw_text = false;
            }

            if let Some(pixmap) = &self.text_pixmap {
                let padding = Point {
                    x: max(0, event.rect.size.x - pixmap.width() as i32) / 2,
                    y: max(0, event.rect.size.y - pixmap.height() as i32) / 2,
                };
                event.draw_pixmap(padding, pixmap.as_ref());
            }
        });
    }

    fn on_mouse_input(&mut self, event: MouseInputEvent) -> bool {
        if event.button == MouseButton::Left {
            if event.state.is_pressed() {
                if self.enabled {
                    self.state = ButtonState::Pressed;
                }
            } else if self.enabled {
                self.state = ButtonState::Hover;    
            }
        }
        if let Some(on_clicked) = &self.on_clicked {
            on_clicked.invoke(self.text.clone());
        }
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
}
