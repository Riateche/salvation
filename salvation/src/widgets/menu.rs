use {
    super::{label::Label, window::WindowWidget, Widget, WidgetCommonTyped, WidgetExt},
    crate::{impl_widget_common, window::X11WindowType},
    winit::window::WindowLevel,
};

pub struct Menu {
    common: WidgetCommonTyped<Self>,
}

impl Menu {}

impl Widget for Menu {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        let window = common
            .add_child::<WindowWidget>()
            .set_decorations(false)
            .set_window_level(WindowLevel::AlwaysOnTop)
            .set_x11_window_type(vec![X11WindowType::Menu])
            .set_skip_windows_taskbar(true);
        window
            .common_mut()
            .add_child::<Label>()
            .set_column(0)
            .set_row(0)
            .set_text("menu content 1\nmenu content 2\nmenu content 3");
        Self { common }
    }
}
