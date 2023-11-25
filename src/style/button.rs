use std::{collections::HashMap, rc::Rc};

use anyhow::Result;
use itertools::Itertools;
use log::warn;
use tiny_skia::Pixmap;

use crate::{
    style::{css::convert_content_url, defaults},
    types::Point,
};

use super::{
    computed::{ComputedBackground, ComputedBorderStyle},
    css::is_root,
    css::{
        convert_background, convert_border, convert_font, convert_main_color, convert_padding,
        Element, MyPseudoClass,
    },
    ElementState, FontStyle, Style,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ButtonState {
    Enabled {
        focused: bool,
        mouse_over: bool,
        pressed: bool,
    },
    Disabled,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self::Enabled {
            focused: false,
            mouse_over: false,
            pressed: false,
        }
    }
}

impl ButtonState {
    pub fn element(&self) -> Element {
        let mut element = Element::new("button");
        match self {
            Self::Enabled {
                focused,
                mouse_over,
                pressed,
            } => {
                element.pseudo_classes.insert(MyPseudoClass::Enabled);
                if *focused {
                    element.pseudo_classes.insert(MyPseudoClass::Focus);
                }
                if *mouse_over {
                    element.pseudo_classes.insert(MyPseudoClass::Hover);
                }
                if *pressed {
                    element.pseudo_classes.insert(MyPseudoClass::Active);
                }
            }
            Self::Disabled => {
                element.pseudo_classes.insert(MyPseudoClass::Disabled);
            }
        }
        element
    }
}

impl ElementState for ButtonState {
    fn all() -> Vec<Self> {
        let all_bools = [false, true];
        let mut r = all_bools
            .into_iter()
            .cartesian_product(all_bools)
            .cartesian_product(all_bools)
            .map(|((focused, mouse_over), pressed)| Self::Enabled {
                focused,
                mouse_over,
                pressed,
            })
            .collect_vec();
        r.push(Self::Disabled);
        r
    }
}

#[derive(Debug, Clone)]
pub struct ComputedVariantStyle {
    pub border: ComputedBorderStyle,
    #[allow(dead_code)] // TODO: implement
    pub background: Option<ComputedBackground>,
    pub text_color: tiny_skia::Color,
    pub icon: Option<Rc<Pixmap>>,
}

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub preferred_padding_with_border: Point,
    pub font_metrics: cosmic_text::Metrics,
    pub variants: HashMap<ButtonState, ComputedVariantStyle>,
}

impl ComputedStyle {
    pub fn new(
        style: &Style,
        scale: f32,
        root_font: &FontStyle,
        class: Option<&'static str>,
    ) -> Result<ComputedStyle> {
        let mut element = Element::new("button");
        if let Some(class) = class {
            element.classes.insert(class);
        }

        let properties = style.find_rules(|s| element.matches(s));
        let font = convert_font(&properties, Some(root_font))?;
        let preferred_padding = convert_padding(&properties, scale, font.font_size)?;

        let variants = ButtonState::all()
            .into_iter()
            .map(|state| {
                let mut element_variant = state.element();
                if let Some(class) = class {
                    element_variant.classes.insert(class);
                }
                println!("element_variant: {element_variant:?}");
                let rules = style.find_rules(|selector| element_variant.matches(selector));
                let rules_with_root = style
                    .find_rules(|selector| is_root(selector) || element_variant.matches(selector));
                let text_color = convert_main_color(&rules_with_root)?.unwrap_or_else(|| {
                    warn!("main text color is unspecified");
                    defaults::text_color()
                });
                let border = convert_border(&rules, scale, text_color)?;
                let background = convert_background(&rules)?;

                let icon = if let Some(url) = convert_content_url(&rules)? {
                    println!("icon url: {url:?}");
                    Some(style.load_pixmap(&url, scale)?)
                } else {
                    None
                };

                let style = ComputedVariantStyle {
                    border,
                    background,
                    icon,
                    text_color,
                };
                anyhow::Ok((state, style))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        let border_width = variants
            .get(&ButtonState::default())
            .expect("expected item for each state")
            .border
            .width;

        // println!(
        //     "button style: {:?}",
        //     variants
        //         .get(&ButtonState::default())
        //         .expect("expected item for each state")
        //         .background
        // );

        Ok(Self {
            preferred_padding_with_border: preferred_padding
                + Point::new(border_width.get(), border_width.get()),
            font_metrics: font.to_metrics(scale),
            variants,
        })
    }
}
