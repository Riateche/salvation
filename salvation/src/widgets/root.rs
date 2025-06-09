use {
    super::{Widget, WidgetCommonTyped},
    crate::impl_widget_common,
};

pub struct RootWidget {
    common: WidgetCommonTyped<Self>,
}

impl Widget for RootWidget {
    impl_widget_common!();

    fn is_window_root_type() -> bool {
        true
    }

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }
}
