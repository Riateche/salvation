use {
    crate::context::Context,
    salvation::{widgets::WidgetExt, App},
    strum::{EnumIter, EnumString, IntoStaticStr},
};

pub mod scroll_bar;
pub mod scroll_bar_mouse_scroll;
pub mod scroll_bar_pager;
pub mod scroll_bar_resize;
pub mod scroll_bar_right_arrow;
pub mod scroll_bar_slider;
pub mod scroll_bar_slider_extremes;
pub mod text_input;

macro_rules! tests {
    ($($name:ident,)*) => {
        #[derive(Debug, Clone, Copy, EnumString, EnumIter, IntoStaticStr)]
        #[allow(non_camel_case_types)]
        pub enum TestCase {
            $($name,)*
        }

        pub fn run_test_case(app: App, test_case: TestCase) -> anyhow::Result<()> {
            match test_case {
                $(
                    TestCase::$name => app.run(|| $name::RootWidget::new().boxed()),
                )*
            }
        }

        pub fn run_test_check(ctx: &mut Context, test_case: TestCase) -> anyhow::Result<()> {
            match test_case {
                $(
                    TestCase::$name => $name::check(ctx),
                )*
            }
        }
    }
}

tests! {
    scroll_bar,
    scroll_bar_right_arrow,
    scroll_bar_slider,
    scroll_bar_slider_extremes,
    scroll_bar_pager,
    scroll_bar_mouse_scroll,
    scroll_bar_resize,
    text_input,
}
