use {
    super::{
        css::{
            convert_background, convert_border, convert_font, convert_main_color, convert_padding,
            convert_zoom, is_root, Element, PseudoClass,
        },
        RelativeOffset, Style,
    },
    crate::{
        layout::{
            grid::{GridAxisOptions, GridOptions},
            Alignment,
        },
        style::{
            css::{convert_spacing, get_border_collapse, is_root_min},
            defaults,
        },
        types::{PhysicalPixels, Point, PpxSuffix},
    },
    log::warn,
    std::any::Any,
    tiny_skia::{Color, GradientStop, SpreadMode},
};

#[derive(Debug, Clone)]
pub struct ComputedBorderStyle {
    pub width: PhysicalPixels,
    pub color: Color,
    pub radius: PhysicalPixels,
}

impl Default for ComputedBorderStyle {
    fn default() -> Self {
        Self {
            width: Default::default(),
            color: Color::TRANSPARENT,
            radius: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct CommonComputedStyle {
    pub min_padding: Point,
    pub min_spacing: Point,
    pub preferred_padding: Point,
    pub preferred_spacing: Point,
    pub min_padding_with_border: Point,
    pub preferred_padding_with_border: Point,
    pub border: ComputedBorderStyle,
    pub background: Option<ComputedBackground>,
    pub text_color: tiny_skia::Color,
    pub font_metrics: cosmic_text::Metrics,
    pub border_collapse: PhysicalPixels,
    pub grid: GridOptions,
}

impl ComputedElementStyle for CommonComputedStyle {
    fn new(style: &Style, element: &Element, scale: f32) -> Self {
        let rules = style.find_rules(|s| element.matches(s));
        let mut rules_with_root = style.find_rules(is_root);
        rules_with_root.extend(rules.clone());
        let element_min = element
            .clone()
            .with_pseudo_class(PseudoClass::Custom("min".into()));
        let mut min_rules_with_root = style.find_rules(is_root_min);
        min_rules_with_root.extend(style.find_rules(|s| element_min.matches(s)));
        let properties_with_root =
            style.find_rules(|selector| is_root(selector) || element.matches(selector));

        let scale = scale * convert_zoom(&rules);
        let font = convert_font(&rules, Some(&style.root_font_style()));
        let min_padding = convert_padding(&min_rules_with_root, scale, font.font_size);
        let preferred_padding = convert_padding(&rules_with_root, scale, font.font_size);

        let min_spacing = convert_spacing(&min_rules_with_root, scale, font.font_size);
        let preferred_spacing = convert_spacing(&rules_with_root, scale, font.font_size);

        let text_color = convert_main_color(&properties_with_root).unwrap_or_else(|| {
            warn!("text color is not specified");
            defaults::text_color()
        });
        let border = convert_border(&rules_with_root, scale, text_color);
        let background = convert_background(&rules);
        let border_collapse = if get_border_collapse(&rules_with_root) {
            border.width
        } else {
            0.ppx()
        };

        let grid = GridOptions {
            x: GridAxisOptions {
                min_padding: min_padding.x(),
                min_spacing: min_spacing.x(),
                preferred_padding: preferred_padding.x(),
                preferred_spacing: preferred_spacing.x(),
                border_collapse,
                // TODO: alignment from css
                alignment: Alignment::Start,
            },
            y: GridAxisOptions {
                min_padding: min_padding.y(),
                min_spacing: min_spacing.y(),
                preferred_padding: preferred_padding.y(),
                preferred_spacing: preferred_spacing.y(),
                border_collapse,
                alignment: Alignment::Start,
            },
        };

        Self {
            preferred_padding,
            min_padding,
            preferred_spacing,
            min_spacing,
            min_padding_with_border: min_padding + Point::new(border.width, border.width),
            preferred_padding_with_border: preferred_padding
                + Point::new(border.width, border.width),
            font_metrics: font.to_metrics(scale),
            border,
            background,
            text_color,
            border_collapse,
            grid,
        }
    }
}

pub trait ComputedElementStyle: Any + Sized {
    fn new(style: &Style, element: &Element, scale: f32) -> Self;
}

#[derive(Debug, Clone)]
pub enum ComputedBackground {
    Solid { color: Color },
    LinearGradient(ComputedLinearGradient),
}

#[derive(Debug, Clone)]
pub struct ComputedLinearGradient {
    pub start: RelativeOffset,
    pub end: RelativeOffset,
    pub stops: Vec<GradientStop>,
    pub mode: SpreadMode,
}
