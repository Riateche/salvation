use std::collections::HashMap;

use anyhow::Result;
use itertools::Itertools;
use log::warn;

use crate::{
    style::{
        css::{convert_background, convert_background_color, convert_border, convert_main_color},
        css::{is_root, is_selection},
        defaults,
    },
    types::{PhysicalPixels, Point},
};

use super::{
    computed::{ComputedBackground, ComputedBorderStyle},
    css::{convert_font, convert_min_width, convert_padding, Element, MyPseudoClass},
    defaults::DEFAULT_INPUT_MIN_WIDTH_EM,
    ElementState, FontStyle, Style,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextInputState {
    Enabled { focused: bool, mouse_over: bool },
    Disabled,
}

impl Default for TextInputState {
    fn default() -> Self {
        Self::Enabled {
            focused: false,
            mouse_over: false,
        }
    }
}

impl TextInputState {
    pub fn element(&self) -> Element {
        let mut element = Element::new("text-input");
        match self {
            Self::Enabled {
                focused,
                mouse_over,
            } => {
                element.pseudo_classes.insert(MyPseudoClass::Enabled);
                if *focused {
                    element.pseudo_classes.insert(MyPseudoClass::Focus);
                }
                if *mouse_over {
                    element.pseudo_classes.insert(MyPseudoClass::Hover);
                }
            }
            Self::Disabled => {
                element.pseudo_classes.insert(MyPseudoClass::Disabled);
            }
        }
        element
    }
}

impl ElementState for TextInputState {
    fn all() -> Vec<Self> {
        let all_bools = [false, true];
        let mut r = all_bools
            .into_iter()
            .cartesian_product(all_bools)
            .map(|(focused, mouse_over)| Self::Enabled {
                focused,
                mouse_over,
            })
            .collect_vec();
        r.push(Self::Disabled);
        r
    }
}

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub padding_with_border: Point,
    pub min_width: PhysicalPixels,
    pub font_metrics: cosmic_text::Metrics,
    pub variants: HashMap<TextInputState, ComputedVariantStyle>,
}

impl ComputedStyle {
    pub fn new(style: &Style, scale: f32, root_font: &FontStyle) -> Result<ComputedStyle> {
        let element = Element::new("text-input");

        let properties = style.find_rules(|s| element.matches(s));
        let font = convert_font(&properties, Some(root_font))?;
        let padding = convert_padding(&properties, scale, font.font_size)?;
        let min_width = convert_min_width(&properties, scale, font.font_size)?
            .unwrap_or_else(|| (font.font_size * DEFAULT_INPUT_MIN_WIDTH_EM).to_physical(scale));

        // TODO: variant-specific selection css rules?
        let selection_properties = style.find_rules(is_selection);
        let selected_text_color = convert_main_color(&selection_properties)?.unwrap_or_else(|| {
            warn!("selected text color is unspecified");
            defaults::selected_text_color()
        });
        let selected_text_background = convert_background_color(&selection_properties)?
            .unwrap_or_else(|| {
                warn!("selected text background is unspecified");
                defaults::selected_text_background()
            });

        let variants = TextInputState::all()
            .into_iter()
            .map(|state| {
                let element = state.element();
                let rules = style.find_rules(|selector| element.matches(selector));
                let rules_with_root =
                    style.find_rules(|selector| is_root(selector) || element.matches(selector));
                let text_color = convert_main_color(&rules_with_root)?.unwrap_or_else(|| {
                    warn!("main text color is unspecified");
                    defaults::text_color()
                });
                let border = convert_border(&rules, scale, text_color)?;
                let background = convert_background(&rules)?;

                let style = ComputedVariantStyle {
                    border,
                    background,
                    text_color,
                    selected_text_color,
                    selected_text_background,
                };
                anyhow::Ok((state, style))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        let border_width = variants
            .get(&TextInputState::default())
            .expect("expected item for each state")
            .border
            .width;

        Ok(Self {
            padding_with_border: padding + Point::new(border_width.get(), border_width.get()),
            min_width,
            font_metrics: font.to_metrics(scale),
            variants,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ComputedVariantStyle {
    pub border: ComputedBorderStyle,
    #[allow(dead_code)] // TODO: implement
    pub background: Option<ComputedBackground>,
    pub text_color: tiny_skia::Color,
    pub selected_text_color: tiny_skia::Color,
    pub selected_text_background: tiny_skia::Color,
}
