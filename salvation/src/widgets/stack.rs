use {
    super::{RawWidgetId, Widget, WidgetCommon, WidgetCommonTyped},
    crate::{
        event::LayoutEvent, impl_widget_common, layout::SizeHintMode, system::ReportError,
        types::Rect,
    },
    anyhow::Result,
    std::collections::HashMap,
};

pub struct Stack {
    common: WidgetCommon,
    rects: HashMap<RawWidgetId, Option<Rect>>,
}

impl Stack {
    // TODO: impl explicit rect setting for universal grid layout?
    pub fn add<T: Widget>(&mut self, rect: Rect) -> &mut T {
        let index = self.common.children.len();
        let widget = self.common.add_child::<T>(Default::default());
        let id = widget.common().id;
        self.common
            .set_child_rect(index, Some(rect))
            .or_report_err();
        self.rects.insert(id, Some(rect));
        self.common.update();
        self.common.children[index]
            .widget
            .downcast_mut::<T>()
            .unwrap()
    }
}

impl Widget for Stack {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self {
            common: common.into(),
            rects: HashMap::new(),
        }
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        Ok(())
    }

    fn recalculate_size_hint_x(&mut self, _mode: SizeHintMode) -> Result<i32> {
        let max = self
            .common
            .children
            .iter()
            .filter_map(|c| c.rect_in_parent)
            .map(|rect| rect.bottom_right().x)
            .max()
            .unwrap_or(0);
        Ok(max)
    }

    fn recalculate_size_hint_y(&mut self, _size_x: i32, _mode: SizeHintMode) -> Result<i32> {
        let max = self
            .common
            .children
            .iter()
            .filter_map(|c| c.rect_in_parent)
            .map(|rect| rect.bottom_right().y)
            .max()
            .unwrap_or(0);
        Ok(max)
    }
}
