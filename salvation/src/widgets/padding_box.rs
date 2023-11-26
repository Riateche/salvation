use anyhow::Result;
use std::cmp::max;

use crate::{
    event::LayoutEvent,
    layout::{LayoutItemOptions, SizeHintMode},
    types::{Point, Rect, Size},
};

use super::{Widget, WidgetCommon, WidgetExt};

const PADDING: Point = Point { x: 10, y: 10 };

#[derive(Default)]
pub struct PaddingBox {
    common: WidgetCommon,
}

impl PaddingBox {
    pub fn new(content: Box<dyn Widget>) -> Self {
        let mut common = WidgetCommon::new();
        common.add_child(content, LayoutItemOptions::from_pos_in_grid(0, 0));
        Self { common }
    }
    // TODO: method to set content and options
}

impl Widget for PaddingBox {
    fn common(&self) -> &super::WidgetCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::WidgetCommon {
        &mut self.common
    }

    fn recalculate_size_hint_x(&mut self, mode: SizeHintMode) -> Result<i32> {
        if let Some(content) = self.common.children.get_mut(0) {
            Ok(content.widget.size_hint_x(mode) + PADDING.x * 2)
        } else {
            Ok(0)
        }
    }

    fn recalculate_size_hint_y(&mut self, size_x: i32, mode: SizeHintMode) -> Result<i32> {
        let child_size_x = max(0, size_x - 2 * PADDING.x);
        if let Some(content) = self.common.children.get_mut(0) {
            Ok(content.widget.size_hint_y(child_size_x, mode) + PADDING.y * 2)
        } else {
            Ok(0)
        }
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        if self.common.children.is_empty() {
            return Ok(());
        }

        let Some(self_rect) = self.common.rect_in_window else {
            return Ok(());
        };
        let rect = Rect {
            top_left: PADDING,
            size: Size {
                x: max(0, self_rect.size.x - 2 * PADDING.x),
                y: max(0, self_rect.size.y - 2 * PADDING.y),
            },
        };
        self.common.set_child_rect(0, Some(rect))?;
        Ok(())
    }
}
