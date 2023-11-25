#![allow(clippy::single_match)]

use std::collections::{HashMap, HashSet};

use anyhow::{bail, Context, Result};
use itertools::Itertools;
use lightningcss::{
    properties::custom::{CustomPropertyName, Token, TokenOrValue},
    rules::CssRule,
    selector::{Component, PseudoClass, PseudoElement, Selector},
    stylesheet::StyleSheet,
};
use lightningcss::{
    properties::{
        border::{BorderSideWidth, LineStyle},
        font::{FontSize, LineHeight},
        size::Size,
        Property,
    },
    values::{
        color::CssColor,
        gradient::{Gradient, GradientItem, LineDirection, LinearGradient},
        image::Image,
        length::{Length, LengthPercentage, LengthPercentageOrAuto, LengthValue},
        percentage::DimensionPercentage,
        position::{HorizontalPositionKeyword, VerticalPositionKeyword},
    },
};
use log::warn;
use tiny_skia::{Color, GradientStop, SpreadMode};

use crate::types::{LogicalPixels, LpxSuffix, PhysicalPixels, Point};

use super::{
    computed::{ComputedBackground, ComputedBorderStyle, ComputedLinearGradient},
    defaults::DEFAULT_LINE_HEIGHT,
    FontStyle, RelativeOffset,
};

fn convert_color(color: &CssColor) -> Result<Color> {
    if let CssColor::RGBA(color) = color {
        Ok(Color::from_rgba8(
            color.red,
            color.green,
            color.blue,
            color.alpha,
        ))
    } else {
        bail!("unsupported color, use rgb: {color:?}")
    }
}

fn convert_length(value: &LengthValue, font_size: Option<LogicalPixels>) -> Result<LogicalPixels> {
    match value {
        LengthValue::Px(size) => Ok(size.lpx()),
        LengthValue::Em(size) => {
            if let Some(font_size) = font_size {
                Ok(font_size * *size)
            } else {
                bail!("unsupported value (em), font size is unknown");
            }
        }
        _ => {
            bail!("unsupported value, use px: {value:?}");
        }
    }
}

#[allow(clippy::collapsible_match)]
fn convert_font_size(size: &FontSize) -> Result<LogicalPixels> {
    if let FontSize::Length(size) = size {
        if let LengthPercentage::Dimension(size) = size {
            return convert_length(size, None);
        }
    }
    bail!("unsupported font size, use px: {size:?}");
}

fn convert_dimension_percentage(
    value: &DimensionPercentage<LengthValue>,
    total: Option<LogicalPixels>,
    font_size: Option<LogicalPixels>,
) -> Result<LogicalPixels> {
    match value {
        DimensionPercentage::Dimension(value) => convert_length(value, font_size),
        DimensionPercentage::Percentage(value) => {
            if let Some(total) = total {
                Ok(total * value.0)
            } else {
                bail!("percentage is unsupported in this context");
            }
        }
        DimensionPercentage::Calc(_) => bail!("calc is unsupported"),
    }
}

fn convert_line_height(value: &LineHeight, font_size: LogicalPixels) -> Result<LogicalPixels> {
    match value {
        LineHeight::Normal => Ok(font_size * DEFAULT_LINE_HEIGHT),
        LineHeight::Number(value) => Ok(font_size * *value),
        LineHeight::Length(value) => {
            convert_dimension_percentage(value, Some(font_size), Some(font_size))
        }
    }
}

// TODO: pass root properties instead?
pub fn convert_font(
    properties: &[&Property<'static>],
    root: Option<&FontStyle>,
) -> Result<FontStyle> {
    let mut font_size = None;
    let mut line_height = None;
    for property in properties {
        match property {
            Property::FontSize(size) => {
                font_size = Some(convert_font_size(size)?);
            }
            Property::Font(font) => {
                font_size = Some(convert_font_size(&font.size)?);
            }
            _ => {}
        }
    }

    let font_size = font_size
        .or_else(|| root.map(|root| root.font_size))
        .context("missing root font size")?;

    for property in properties {
        match property {
            Property::LineHeight(value) => {
                line_height = Some(convert_line_height(value, font_size)?);
            }
            _ => {}
        }
    }

    let line_height = line_height.unwrap_or_else(|| font_size * DEFAULT_LINE_HEIGHT);

    Ok(FontStyle {
        font_size,
        line_height,
    })
}

pub fn convert_main_color(properties: &[&Property<'static>]) -> Result<Option<Color>> {
    let mut color = None;
    for property in properties {
        match property {
            Property::Color(value) => {
                color = Some(convert_color(value)?);
            }
            _ => {}
        }
    }
    Ok(color)
}

fn convert_single_padding(
    value: &LengthPercentageOrAuto,
    font_size: LogicalPixels,
) -> Result<LogicalPixels> {
    match value {
        LengthPercentageOrAuto::Auto => Ok(0.0.into()),
        LengthPercentageOrAuto::LengthPercentage(value) => {
            if let LengthPercentage::Dimension(value) = value {
                convert_length(value, Some(font_size))
            } else {
                bail!("unsupported value ({value:?})")
            }
        }
    }
}

pub fn convert_padding(
    properties: &[&Property<'static>],
    scale: f32,
    font_size: LogicalPixels,
) -> Result<Point> {
    let mut left = None;
    let mut top = None;
    for property in properties {
        match property {
            Property::Padding(value) => {
                left = Some(convert_single_padding(&value.left, font_size)?);
                top = Some(convert_single_padding(&value.top, font_size)?);
            }
            Property::PaddingLeft(value) => {
                left = Some(convert_single_padding(value, font_size)?);
            }
            Property::PaddingTop(value) => {
                top = Some(convert_single_padding(value, font_size)?);
            }
            _ => {}
        }
    }
    Ok(Point::new(
        left.unwrap_or_default().to_physical(scale).get(),
        top.unwrap_or_default().to_physical(scale).get(),
    ))
}

#[allow(dead_code)]
pub fn convert_width(
    properties: &[&Property<'static>],
    scale: f32,
    font_size: LogicalPixels,
) -> Result<Option<PhysicalPixels>> {
    let mut width = None;
    for property in properties {
        match property {
            Property::Width(value) => match value {
                Size::Auto => {}
                Size::LengthPercentage(value) => {
                    width = Some(convert_dimension_percentage(value, None, Some(font_size))?);
                }
                _ => warn!("unsupported width value: {value:?}"),
            },
            _ => {}
        }
    }
    Ok(width.map(|width| width.to_physical(scale)))
}

pub fn convert_min_width(
    properties: &[&Property<'static>],
    scale: f32,
    font_size: LogicalPixels,
) -> Result<Option<PhysicalPixels>> {
    let mut width = None;
    for property in properties {
        match property {
            Property::MinWidth(value) => match value {
                Size::Auto => {}
                Size::LengthPercentage(value) => {
                    width = Some(convert_dimension_percentage(value, None, Some(font_size))?);
                }
                _ => warn!("unsupported width value: {value:?}"),
            },
            _ => {}
        }
    }
    Ok(width.map(|width| width.to_physical(scale)))
}

fn convert_border_width(width: &BorderSideWidth) -> Result<LogicalPixels> {
    if let BorderSideWidth::Length(width) = width {
        match width {
            Length::Value(width) => convert_length(width, None),
            Length::Calc(_) => bail!("calc is unsupported"),
        }
    } else {
        bail!("unsupported border width (use explicit width): {width:?}");
    }
}

pub fn convert_border(
    properties: &[&Property<'static>],
    scale: f32,
    text_color: Color,
) -> Result<ComputedBorderStyle> {
    let mut width = None;
    let mut color = None;
    let mut radius = None;
    let mut style = LineStyle::None;
    for property in properties {
        match property {
            Property::Border(value) => {
                width = Some(convert_border_width(&value.width)?);
                color = Some(convert_color(&value.color)?);
                style = value.style;
            }
            Property::BorderWidth(value) => {
                // TODO: support different sides
                width = Some(convert_border_width(&value.top)?);
            }
            Property::BorderColor(value) => {
                color = Some(convert_color(&value.top)?);
            }
            Property::BorderStyle(value) => {
                style = value.top;
            }
            Property::BorderRadius(value, _prefix) => {
                radius = Some(convert_dimension_percentage(&value.top_left.0, None, None)?);
            }
            _ => {}
        }
    }

    match style {
        LineStyle::None => Ok(ComputedBorderStyle::default()),
        LineStyle::Solid => Ok(ComputedBorderStyle {
            width: width.unwrap_or_default().to_physical(scale),
            color: color.unwrap_or(text_color),
            radius: radius.unwrap_or_default().to_physical(scale),
        }),
        _ => bail!("unsupported border line style: {style:?}"),
    }
}

fn convert_linear_gradient(value: &LinearGradient) -> Result<ComputedLinearGradient> {
    let (start, end) = match value.direction {
        LineDirection::Angle(_) => bail!("angle in unsupported in gradient"),
        LineDirection::Horizontal(value) => match value {
            HorizontalPositionKeyword::Left => {
                (RelativeOffset::new(0.0, 0.0), RelativeOffset::new(1.0, 0.0))
            }
            HorizontalPositionKeyword::Right => {
                (RelativeOffset::new(1.0, 0.0), RelativeOffset::new(0.0, 0.0))
            }
        },
        LineDirection::Vertical(value) => match value {
            VerticalPositionKeyword::Top => {
                (RelativeOffset::new(0.0, 1.0), RelativeOffset::new(0.0, 0.0))
            }
            VerticalPositionKeyword::Bottom => {
                (RelativeOffset::new(0.0, 0.0), RelativeOffset::new(0.0, 1.0))
            }
        },
        LineDirection::Corner {
            horizontal,
            vertical,
        } => match (horizontal, vertical) {
            (HorizontalPositionKeyword::Left, VerticalPositionKeyword::Top) => {
                (RelativeOffset::new(1.0, 1.0), RelativeOffset::new(0.0, 0.0))
            }
            (HorizontalPositionKeyword::Right, VerticalPositionKeyword::Top) => {
                (RelativeOffset::new(0.0, 1.0), RelativeOffset::new(1.0, 0.0))
            }
            (HorizontalPositionKeyword::Left, VerticalPositionKeyword::Bottom) => {
                (RelativeOffset::new(1.0, 0.0), RelativeOffset::new(0.0, 1.0))
            }
            (HorizontalPositionKeyword::Right, VerticalPositionKeyword::Bottom) => {
                (RelativeOffset::new(0.0, 0.0), RelativeOffset::new(1.0, 1.0))
            }
        },
    };
    let mut stops = Vec::new();
    for item in &value.items {
        match item {
            GradientItem::ColorStop(value) => {
                let position = value
                    .position
                    .as_ref()
                    .context("gradient stop without position is unsupported")?;
                let position = match position {
                    DimensionPercentage::Dimension(_) => {
                        bail!("absolute position in gradient is unsupported")
                    }
                    DimensionPercentage::Percentage(value) => value.0,
                    DimensionPercentage::Calc(_) => bail!("calc is unsupported"),
                };
                stops.push(GradientStop::new(position, convert_color(&value.color)?));
            }
            GradientItem::Hint(_) => bail!("gradient hints are not supported"),
        }
    }
    Ok(ComputedLinearGradient {
        start,
        end,
        stops,
        mode: SpreadMode::Pad,
    })
}

pub fn convert_background_color(properties: &[&Property<'static>]) -> Result<Option<Color>> {
    let bg = convert_background(properties)?;
    if let Some(bg) = bg {
        match bg {
            ComputedBackground::Solid { color } => Ok(Some(color)),
            ComputedBackground::LinearGradient(_) => {
                bail!("only background color is supported in this context")
            }
        }
    } else {
        Ok(None)
    }
}

pub fn convert_background(properties: &[&Property<'static>]) -> Result<Option<ComputedBackground>> {
    let mut final_background = None;
    for property in properties {
        match property {
            Property::Background(backgrounds) => {
                if backgrounds.is_empty() {
                    warn!("empty vec in Property::Background");
                    continue;
                }
                if backgrounds.len() > 1 {
                    warn!("multiple backgrounds are not supported");
                }
                let background = &backgrounds[0];
                final_background = Some(ComputedBackground::Solid {
                    color: convert_color(&background.color)?,
                });
                match &background.image {
                    Image::None => {}
                    Image::Url(_) => bail!("url() is not supported in background"),
                    Image::Gradient(value) => match &**value {
                        Gradient::Linear(value) => {
                            final_background = Some(ComputedBackground::LinearGradient(
                                convert_linear_gradient(value)?,
                            ));
                        }
                        _ => bail!("unsupported gradient"),
                    },
                    Image::ImageSet(_) => bail!("ImageSet is not supported in background"),
                }
            }
            Property::BackgroundColor(value) => {
                final_background = Some(ComputedBackground::Solid {
                    color: convert_color(value)?,
                });
            }
            _ => {}
        }
    }
    Ok(final_background)
}

pub fn get_border_collapse(properties: &[&Property<'static>]) -> bool {
    let mut value = false;
    for property in properties {
        match property {
            Property::Custom(property) => {
                if let CustomPropertyName::Unknown(name) = &property.name {
                    if name.as_ref() == "border-collapse" {
                        if property.value.0.len() != 1 {
                            warn!("expected 1 token in border-collapse proprety");
                            continue;
                        }
                        if let TokenOrValue::Token(Token::Ident(ident)) = &property.value.0[0] {
                            match ident.as_ref() {
                                "collapse" => {
                                    value = true;
                                }
                                "separate" => {
                                    value = false;
                                }
                                _ => {
                                    warn!("invalid value of border-collapse proprety: {ident:?}");
                                }
                            }
                        } else {
                            warn!("expected ident in border-collapse proprety");
                            continue;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    value
}

pub fn convert_content_url(properties: &[&Property<'static>]) -> Result<Option<String>> {
    let mut final_url = None;
    for property in properties {
        match property {
            Property::Custom(property) => {
                if let CustomPropertyName::Unknown(name) = &property.name {
                    if name.as_ref() == "content" {
                        if property.value.0.len() != 1 {
                            warn!("expected 1 token in content proprety");
                            continue;
                        }
                        if let TokenOrValue::Url(url) = &property.value.0[0] {
                            final_url = Some(url.url.to_string());
                        } else {
                            warn!("expected url() in content proprety");
                            continue;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(final_url)
}

pub fn replace_vars(style_sheet: &mut StyleSheet) {
    //let mut style_sheet: StyleSheet<'static, 'static> = style_sheet.into_owned();
    let mut vars = HashMap::new();
    for rule in &style_sheet.rules.0 {
        if let CssRule::Style(rule) = rule {
            // println!("selectors: {:?}", rule.selectors);
            for selector in &rule.selectors.0 {
                // print_selector(selector);
                if is_root(selector) {
                    // println!("found root!");
                    for (property, _) in rule.declarations.iter() {
                        //println!("root declaration: {declaration:?}");
                        if let Property::Custom(property) = property {
                            if let CustomPropertyName::Custom(name) = &property.name {
                                vars.insert(name.as_ref().to_string(), property.value.clone());
                            }
                        }
                    }
                }
                // if let Some(data) = as_tag_with_class(selector) {
                // println!(
                //     "found tag with class {:?}, {}",
                //     data.tag,
                //     serde_json::to_string(&data.class).unwrap()
                // );
                // }
                // print_selector(selector);
            }
        }
    }
    for rule in &mut style_sheet.rules.0 {
        if let CssRule::Style(rule) = rule {
            for property in rule.declarations.iter_mut() {
                if let Property::Unparsed(property) = property {
                    let mut new_tokens = Vec::new();
                    for token in &property.value.0 {
                        if let TokenOrValue::Var(variable) = token {
                            if let Some(value) = vars.get(variable.name.ident.as_ref()) {
                                // println!("substitute!");
                                // TODO: use substitute_variables
                                new_tokens.extend(value.0.clone());
                                continue;
                            }
                        }
                        new_tokens.push(token.clone());
                    }
                    property.value.0 = new_tokens;
                }
            }
        }
    }

    // println!("vars: {vars:#?}");
}

#[allow(dead_code)]
fn print_selector(selector: &Selector) {
    println!("selector: {:?}", selector);
    let mut iter = selector.iter();
    loop {
        for x in &mut iter {
            println!("selector item: {:?}", x);
            print_component(x);
            if matches!(x, Component::Root) {
                println!("found root!");
            }
            if let Component::Negation(inner) = x {
                println!("found not! inner:");
                print_selector(&inner[0]);
                println!("inner end");
            }
        }
        if let Some(seq) = iter.next_sequence() {
            println!("seq: {seq:?}");
        } else {
            println!("no seq");
            break;
        }
    }
}

fn print_component(component: &Component) {
    match component {
        Component::Combinator(_) => println!("Combinator"),
        Component::ExplicitAnyNamespace => println!("ExplicitAnyNamespace"),
        Component::ExplicitNoNamespace => println!("ExplicitNoNamespace"),
        Component::DefaultNamespace(_) => println!("DefaultNamespace"),
        Component::Namespace(..) => println!("Namespace"),
        Component::ExplicitUniversalType => println!("ExplicitUniversalType"),
        Component::LocalName(_) => println!("LocalName"),
        Component::ID(_) => println!("ID"),
        Component::Class(_) => println!("Class"),
        Component::AttributeInNoNamespaceExists { .. } => println!("AttributeInNoNamespaceExists"),
        Component::AttributeInNoNamespace { .. } => println!("AttributeInNoNamespace"),
        Component::AttributeOther(_) => println!("AttributeOther"),
        Component::Negation(_) => println!("Negation"),
        Component::Root => println!("Root"),
        Component::Empty => println!("Empty"),
        Component::Scope => println!("Scope"),
        Component::Nth(_) => println!("Nth"),
        Component::NthOf(_) => println!("NthOf"),
        Component::NonTSPseudoClass(x) => {
            println!("NonTSPseudoClass");
            if let PseudoClass::Custom { name } = x {
                println!("name = {name:?}");
            }
        }
        Component::Slotted(_) => println!("Slotted"),
        Component::Part(_) => println!("Part"),
        Component::Host(_) => println!("Host"),
        Component::Where(_) => println!("Where"),
        Component::Is(_) => println!("Is"),
        Component::Any(..) => println!("Any"),
        Component::Has(_) => println!("Has"),
        Component::PseudoElement(_) => println!("PseudoElement"),
        Component::Nesting => println!("Nesting"),
    }
}

pub fn selector_items<'i, 'a>(selector: &'a Selector<'i>) -> Option<Vec<&'a Component<'i>>> {
    let mut iter = selector.iter();
    let components = (&mut iter).collect_vec();
    if iter.next_sequence().is_some() {
        if iter.next().is_none() && iter.next_sequence().is_none() {
            // workaround for "::selection"
            return Some(components);
        }
        warn!("nesting in CSS selectors is not supported (selector: {selector:?})");
        return None;
    }
    Some(components)
}

pub fn is_root(selector: &Selector) -> bool {
    selector_items(selector).map_or(false, |items| {
        items.len() == 1 && matches!(items[0], Component::Root)
    })
}

pub fn is_selection(selector: &Selector) -> bool {
    selector_items(selector).map_or(false, |items| {
        items.len() == 1
            && matches!(
                items[0],
                Component::PseudoElement(PseudoElement::Selection(_))
            )
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MyPseudoClass {
    Hover,
    Focus,
    Active,
    Enabled,
    Disabled,
    Min,
    Other,
}

impl<'a, 'b> From<&'a PseudoClass<'b>> for MyPseudoClass {
    fn from(value: &'a PseudoClass<'b>) -> Self {
        match value {
            PseudoClass::Hover => Self::Hover,
            PseudoClass::Focus => Self::Focus,
            PseudoClass::Active => Self::Active,
            PseudoClass::Enabled => Self::Enabled,
            PseudoClass::Disabled => Self::Disabled,
            PseudoClass::Custom { name } => {
                if name.as_ref() == "min" {
                    Self::Min
                } else {
                    Self::Other
                }
            }
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: &'static str,
    pub classes: HashSet<&'static str>,
    pub pseudo_classes: HashSet<MyPseudoClass>,
}

impl Element {
    pub fn new(tag: &'static str) -> Self {
        Self {
            tag,
            classes: HashSet::new(),
            pseudo_classes: HashSet::new(),
        }
    }

    pub fn with_class(mut self, class: &'static str) -> Self {
        self.classes.insert(class);
        self
    }

    pub fn with_pseudo_class(mut self, class: MyPseudoClass) -> Self {
        self.pseudo_classes.insert(class);
        self
    }

    pub fn matches(&self, selector: &Selector) -> bool {
        let Some(items) = selector_items(selector) else {
            return false;
        };
        for item in items {
            match item {
                Component::NonTSPseudoClass(c) => {
                    if !self.pseudo_classes.contains(&c.into()) {
                        return false;
                    }
                }
                Component::Class(c) => {
                    if !self.classes.contains(c.as_ref()) {
                        return false;
                    }
                }
                Component::LocalName(name) => {
                    if self.tag != name.lower_name.as_ref() {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

// pub struct TagSelector<'a, 'b> {
//     pub tag: &'a str,
//     pub class: Option<&'a PseudoClass<'b>>,
// }

// pub fn as_tag_with_class<'a, 'b>(selector: &'a Selector<'b>) -> Option<TagSelector<'a, 'b>> {
//     let items = selector_items(selector)?;
//     if items.len() > 2 {
//         return None;
//     }
//     Some(TagSelector {
//         tag: as_tag(items.get(0)?)?,
//         class: items.get(1).and_then(|i| as_class(i)),
//     })
// }

// fn as_tag<'a>(component: &'a Component<'_>) -> Option<&'a str> {
//     if let Component::LocalName(component) = component {
//         Some(&component.lower_name)
//     } else {
//         None
//     }
// }

// fn as_class<'a, 'b>(component: &'a Component<'b>) -> Option<&'a PseudoClass<'b>> {
//     if let Component::NonTSPseudoClass(component) = component {
//         Some(component)
//     } else {
//         None
//     }
// }

// pub fn is_tag_with_no_class(selector: &Selector, tag: &str) -> bool {
//     as_tag_with_class(selector).map_or(false, |data| data.tag == tag && data.class.is_none())
// }

// pub fn is_tag_with_custom_class(selector: &Selector, tag: &str, class: &str) -> bool {
//     as_tag_with_class(selector).map_or(false, |data| {
//         data.tag == tag
//             && data
//                 .class
//                 .map_or(false, |c| as_custom_class(c) == Some(class))
//     })
// }

// fn as_custom_class<'a>(class: &'a PseudoClass<'_>) -> Option<&'a str> {
//     if let PseudoClass::Custom { name } = class {
//         Some(name)
//     } else {
//         None
//     }
// }
