use tiny_skia::Color;

use super::Style;

pub fn default_style() -> Style {
    Style::load(include_str!("../../themes/default/theme.css")).unwrap()
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
