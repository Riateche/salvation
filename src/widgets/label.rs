use std::fmt::Display;

use cosmic_text::{Attrs, Buffer, Shaping};
use tiny_skia::Pixmap;

use crate::{
    draw::{draw_text, unrestricted_text_size, DrawContext},
    types::{Point, Size},
    Widget,
};

pub struct Label {
    text: String,
    buffer: Option<Buffer>,
    pixmap: Option<Pixmap>,
    unrestricted_text_size: Size,
    redraw_text: bool,
}

impl Label {
    pub fn new(text: impl Display) -> Self {
        Self {
            text: text.to_string(),
            buffer: None,
            pixmap: None,
            unrestricted_text_size: Size::default(),
            redraw_text: true,
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.text = text.to_string();
        self.redraw_text = true;
    }
}

impl Widget for Label {
    fn draw(&mut self, ctx: &mut DrawContext<'_>) {
        let mut buffer = self
            .buffer
            .get_or_insert_with(|| Buffer::new(ctx.font_system, ctx.font_metrics))
            .borrow_with(ctx.font_system);

        if self.redraw_text {
            buffer.set_text(&self.text, Attrs::new(), Shaping::Advanced);
            self.unrestricted_text_size = unrestricted_text_size(&mut buffer);
            let pixmap = draw_text(
                &mut buffer,
                self.unrestricted_text_size,
                ctx.palette.foreground,
                ctx.swash_cache,
            );
            self.pixmap = Some(pixmap);
            self.redraw_text = false;
        }

        if let Some(pixmap) = &self.pixmap {
            ctx.draw_pixmap(Point::default(), pixmap.as_ref());
        }
    }
}
