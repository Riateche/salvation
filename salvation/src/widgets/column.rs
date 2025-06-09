use {
    super::{Widget, WidgetCommonTyped},
    crate::impl_widget_common,
};

pub struct Column {
    // TODO: add layout options
    common: WidgetCommonTyped<Self>,
}

impl Widget for Column {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }
}
