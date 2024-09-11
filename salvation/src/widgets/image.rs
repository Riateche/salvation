use std::path::Path;

use anyhow::Result;
use png::DecodingError;
use tiny_skia::Pixmap;

use crate::{draw::DrawEvent, impl_widget_common, layout::SizeHintMode, types::Point};

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
    impl_widget_common!();

    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        if let Some(pixmap) = &self.pixmap {
            event.draw_pixmap(Point::default(), pixmap.as_ref());
        }
        Ok(())
    }

    fn recalculate_size_hint_x(&mut self, _mode: SizeHintMode) -> Result<i32> {
        Ok(self.pixmap.as_ref().map_or(0, |p| p.width() as i32))
    }

    fn recalculate_size_hint_y(&mut self, _size_x: i32, _mode: SizeHintMode) -> Result<i32> {
        Ok(self.pixmap.as_ref().map_or(0, |p| p.height() as i32))
    }
}
