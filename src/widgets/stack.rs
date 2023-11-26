use anyhow::Result;
use std::collections::HashMap;

use crate::{event::LayoutEvent, layout::SizeHint, system::ReportError, types::Rect};

use super::{RawWidgetId, Widget, WidgetCommon};

pub struct Stack {
    common: WidgetCommon,
    rects: HashMap<RawWidgetId, Option<Rect>>,
}

impl Stack {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            common: WidgetCommon::new(),
            rects: HashMap::new(),
        }
    }

    // TODO: impl explicit rect setting for universal grid layout?
    pub fn add(&mut self, rect: Rect, widget: Box<dyn Widget>) {
        let id = widget.common().id;
        let index = self.common.add_child(widget, Default::default());
        self.common
            .set_child_rect(index, Some(rect))
            .or_report_err();
        self.rects.insert(id, Some(rect));
        self.common.update();
    }
}

impl Widget for Stack {
    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }
    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        Ok(())
    }

    fn size_hint_x(&mut self) -> Result<SizeHint> {
        let max = self
            .common
            .children
            .iter()
            .filter_map(|c| c.rect_in_parent)
            .map(|rect| rect.bottom_right().x)
            .max()
            .unwrap_or(0);
        Ok(SizeHint::new_fixed(max))
    }

    fn size_hint_y(&mut self, _size_x: i32) -> Result<SizeHint> {
        let max = self
            .common
            .children
            .iter()
            .filter_map(|c| c.rect_in_parent)
            .map(|rect| rect.bottom_right().y)
            .max()
            .unwrap_or(0);
        Ok(SizeHint::new_fixed(max))
    }
}
