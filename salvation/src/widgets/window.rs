use {
    super::{Widget, WidgetCommonTyped},
    crate::{impl_widget_common, window::X11WindowType},
    std::fmt::Display,
    winit::window::WindowLevel,
};

pub struct WindowWidget {
    common: WidgetCommonTyped<Self>,
}

impl WindowWidget {
    pub fn set_title(&mut self, title: impl Display) -> &mut Self {
        self.common.window.as_ref().unwrap().set_title(title);
        self
    }

    pub fn set_decorations(&mut self, value: bool) -> &mut Self {
        self.common.window.as_ref().unwrap().set_decorations(value);
        self
    }

    pub fn set_window_level(&mut self, value: WindowLevel) -> &mut Self {
        self.common.window.as_ref().unwrap().set_window_level(value);
        self
    }

    pub fn set_x11_window_type(&mut self, value: Vec<X11WindowType>) -> &mut Self {
        self.common
            .window
            .as_ref()
            .unwrap()
            .set_x11_window_type(value);
        self
    }

    pub fn set_skip_windows_taskbar(&mut self, value: bool) -> &mut Self {
        self.common
            .window
            .as_ref()
            .unwrap()
            .set_skip_windows_taskbar(value);
        self
    }
}

impl Widget for WindowWidget {
    impl_widget_common!();

    fn is_window_root_type() -> bool {
        true
    }

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }
}

impl Drop for WindowWidget {
    fn drop(&mut self) {
        self.common.window.as_ref().unwrap().deregister();
    }
}
