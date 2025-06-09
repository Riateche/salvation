use {
    super::{Widget, WidgetCommonTyped, WidgetExt, WidgetGeometry},
    crate::{
        event::LayoutEvent,
        impl_widget_common,
        key::Key,
        layout::SizeHints,
        types::{PhysicalPixels, PpxSuffix, Rect},
    },
    anyhow::Result,
};

pub struct Stack {
    common: WidgetCommonTyped<Self>,
}

impl Stack {
    // TODO: impl explicit rect setting for universal grid layout?
    pub fn add<T: Widget>(&mut self, key: Key, rect: Rect) -> &mut T {
        let geometry = self.common.geometry.clone();
        let widget = self.common.add_child_with_key::<T>(key.clone());
        if let Some(geometry) = geometry {
            widget.set_geometry(Some(WidgetGeometry::new(&geometry, rect)), &[]);
        }
        self.common.update();
        self.common
            .children
            .get_mut(&key)
            .unwrap()
            .downcast_mut::<T>()
            .unwrap()
    }
}

impl Widget for Stack {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        Ok(())
    }

    fn handle_size_hint_x_request(&mut self) -> Result<crate::layout::SizeHints> {
        let max = self
            .common
            .children
            .values()
            .filter_map(|c| c.common().rect_in_parent())
            .map(|rect| rect.bottom_right().x())
            .max()
            .unwrap_or(0.ppx());
        Ok(SizeHints {
            min: max,
            preferred: max,
            is_fixed: true,
        })
    }

    fn handle_size_hint_y_request(&mut self, _size_x: PhysicalPixels) -> Result<SizeHints> {
        let max = self
            .common
            .children
            .values()
            .filter_map(|c| c.common().rect_in_parent())
            .map(|rect| rect.bottom_right().y())
            .max()
            .unwrap_or(0.ppx());
        Ok(SizeHints {
            min: max,
            preferred: max,
            is_fixed: true,
        })
    }
}
