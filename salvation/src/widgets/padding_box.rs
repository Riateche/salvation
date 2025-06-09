use {
    super::{Widget, WidgetCommonTyped},
    crate::impl_widget_common,
};

pub struct PaddingBox {
    common: WidgetCommonTyped<Self>,
}

impl PaddingBox {
    // TODO: method to set content and options
}

impl Widget for PaddingBox {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }
}
