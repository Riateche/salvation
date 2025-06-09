use {
    super::{computed::ComputedElementStyle, css::Element},
    crate::style::{
        css::{convert_content_url, convert_zoom},
        Style,
    },
    log::warn,
    std::rc::Rc,
    tiny_skia::Pixmap,
};

#[derive(Debug, Clone, Default)]
pub struct ComputedButtonStyle {
    pub icon: Option<Rc<Pixmap>>,
}

impl ComputedElementStyle for ComputedButtonStyle {
    fn new(style: &Style, element: &Element, scale: f32) -> ComputedButtonStyle {
        let properties = style.find_rules(|s| element.matches(s));

        let scale = scale * convert_zoom(&properties);
        let mut icon = None;
        if let Some(url) = convert_content_url(&properties) {
            //println!("icon url: {url:?}");
            match style.load_pixmap(&url, scale) {
                Ok(pixmap) => icon = Some(pixmap),
                Err(err) => warn!("failed to load icon: {err:?}"),
            }
        }
        Self { icon }
    }
}
