use std::fmt::Display;

use cosmic_text::{Attrs, Buffer, Shaping};
use tiny_skia::Pixmap;

use crate::{
    draw::{draw_text, unrestricted_text_size, DrawEvent},
    types::{Point, Size},
};

use super::{Widget, WidgetCommon};

pub struct Label {
    text: String,
    buffer: Option<Buffer>,
    pixmap: Option<Pixmap>,
    unrestricted_text_size: Size,
    redraw_text: bool,
    common: WidgetCommon,
}

impl Label {
    pub fn new(text: impl Display) -> Self {
        Self {
            text: text.to_string(),
            buffer: None,
            pixmap: None,
            unrestricted_text_size: Size::default(),
            redraw_text: true,
            common: WidgetCommon::new(),
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.text = text.to_string();
        self.redraw_text = true;
    }
}

impl Widget for Label {
    fn on_draw(&mut self, event: DrawEvent) -> bool {
        let system = &mut *self
            .common
            .mount_point
            .as_ref()
            .expect("cannot draw when unmounted")
            .system
            .0
            .borrow_mut();

        let mut buffer = self
            .buffer
            .get_or_insert_with(|| Buffer::new(&mut system.font_system, system.font_metrics))
            .borrow_with(&mut system.font_system);

        if self.redraw_text {
            buffer.set_text(&self.text, Attrs::new(), Shaping::Advanced);
            self.unrestricted_text_size = unrestricted_text_size(&mut buffer);
            let pixmap = draw_text(
                &mut buffer,
                self.unrestricted_text_size,
                system.palette.foreground,
                &mut system.swash_cache,
            );
            self.pixmap = Some(pixmap);
            self.redraw_text = false;
        }

        if let Some(pixmap) = &self.pixmap {
            event.draw_pixmap(Point::default(), pixmap.as_ref());
        }
        true
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }
}
