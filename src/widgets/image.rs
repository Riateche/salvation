use std::path::Path;

use anyhow::Result;
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
    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        if let Some(pixmap) = &self.pixmap {
            event.draw_pixmap(Point::default(), pixmap.as_ref());
        }
        Ok(())
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn size_hint_x(&mut self) -> Result<SizeHint> {
        let value = self.pixmap.as_ref().map_or(0, |p| p.width() as i32);
        Ok(SizeHint::new_fixed(value))
    }

    fn size_hint_y(&mut self, _size_x: i32) -> Result<SizeHint> {
        let value = self.pixmap.as_ref().map_or(0, |p| p.height() as i32);
        Ok(SizeHint::new_fixed(value))
    }
}
