use {
    super::Style,
    crate::{
        style::FontStyle,
        types::{LogicalPixels, LpxSuffix},
    },
    tiny_skia::Color,
};

macro_rules! file {
    ($path: literal) => {
        (
            $path,
            &include_bytes!(concat!("../../themes/default/", $path))[..],
        )
    };
}

pub fn default_style() -> Style {
    Style::load_bundled(
        include_str!("../../themes/default/theme.css"),
        [
            file!("scroll_left.svg"),
            file!("scroll_right.svg"),
            file!("scroll_up.svg"),
            file!("scroll_down.svg"),
            file!("scroll_grip_x.svg"),
            file!("scroll_grip_y.svg"),
            file!("scroll_left_disabled.svg"),
            file!("scroll_right_disabled.svg"),
            file!("scroll_up_disabled.svg"),
            file!("scroll_down_disabled.svg"),
            file!("scroll_grip_x_disabled.svg"),
            file!("scroll_grip_y_disabled.svg"),
        ],
    )
    .unwrap()
}

pub fn font_size() -> LogicalPixels {
    13.0.lpx()
}

pub fn background_color() -> Color {
    Color::from_rgba8(255, 255, 255, 255)
}

pub fn text_color() -> Color {
    Color::from_rgba8(0, 0, 0, 255)
}

pub fn selected_text_color() -> Color {
    Color::from_rgba8(255, 255, 255, 255)
}

pub fn selected_text_background() -> Color {
    Color::from_rgba8(100, 100, 150, 255)
}

pub const DEFAULT_PREFERRED_WIDTH_EM: f32 = 10.0;
pub const DEFAULT_MIN_WIDTH_EM: f32 = 2.0;

pub const DEFAULT_LINE_HEIGHT: f32 = 1.2;

pub fn font_style() -> FontStyle {
    FontStyle {
        font_size: font_size(),
        line_height: font_size() * DEFAULT_LINE_HEIGHT,
    }
}
