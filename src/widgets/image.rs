use std::path::Path;

use png::DecodingError;
use tiny_skia::Pixmap;

use crate::{draw::DrawEvent, layout::SizeHint, types::Point};

use super::{Widget, WidgetCommon};

pub struct Image {
    pixmap: Option<Pixmap>,
    common: WidgetCommon,
}

impl Image {
    pub fn load_png<P: AsRef<Path>>(path: P) -> Result<Self, DecodingError> {
        Ok(Self {
            pixmap: Some(Pixmap::load_png(path)?),
            common: WidgetCommon::new(),
        })
    }

    pub fn new(pixmap: Pixmap) -> Self {
        Self {
            pixmap: Some(pixmap),
            common: WidgetCommon::new(),
        }
    }
}

impl Widget for Image {
    fn on_draw(&mut self, event: DrawEvent) {
        if let Some(pixmap) = &self.pixmap {
            event.draw_pixmap(Point::default(), pixmap.as_ref());
        }
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn size_hint_x(&mut self) -> SizeHint {
        let size = self.pixmap.as_ref().map_or(0, |p| p.width() as i32);

        SizeHint {
            min: size,
            preferred: size,
            is_fixed: true,
        }
    }

    fn size_hint_y(&mut self, _size_x: i32) -> SizeHint {
        let size = self.pixmap.as_ref().map_or(0, |p| p.height() as i32);

        SizeHint {
            min: size,
            preferred: size,
            is_fixed: true,
        }
    }
}
