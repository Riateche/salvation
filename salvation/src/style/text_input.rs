use {
    super::{
        css::{convert_font, convert_width, Element, PseudoClass},
        defaults::{DEFAULT_MIN_WIDTH_EM, DEFAULT_PREFERRED_WIDTH_EM},
        Style,
    },
    crate::{style::computed::ComputedElementStyle, system::ReportError, types::PhysicalPixels},
    log::warn,
};

#[derive(Debug, Clone)]
pub struct TextInputStyle {
    pub min_width: PhysicalPixels,
    pub preferred_width: PhysicalPixels,
}

impl ComputedElementStyle for TextInputStyle {
    fn new(style: &Style, element: &Element, scale: f32) -> TextInputStyle {
        let element_min = element
            .clone()
            .with_pseudo_class(PseudoClass::Custom("min".into()));

        let properties = style.find_rules(|s| element.matches(s));
        let font = convert_font(&properties, Some(&style.root_font_style()));
        let preferred_width = convert_width(&properties, scale, font.font_size)
            .or_report_err()
            .flatten()
            .unwrap_or_else(|| {
                warn!("missing width in text input css");
                (font.font_size * DEFAULT_PREFERRED_WIDTH_EM).to_physical(scale)
            });

        let min_properties = style.find_rules(|s| element_min.matches(s));
        let min_width = convert_width(&min_properties, scale, font.font_size)
            .or_report_err()
            .flatten()
            .unwrap_or_else(|| {
                warn!("missing width in text input min css");
                (font.font_size * DEFAULT_MIN_WIDTH_EM).to_physical(scale)
            });

        Self {
            min_width,
            preferred_width,
        }
    }
}
