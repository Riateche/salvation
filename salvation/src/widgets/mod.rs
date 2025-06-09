use {crate::system::address, anyhow::Result, log::warn, std::fmt::Debug, thiserror::Error};

mod address;
pub mod button;
pub mod column;
mod common;
mod ext;
mod id;
pub mod image;
pub mod label;
pub mod menu;
pub mod padding_box;
pub mod root;
pub mod row;
pub mod scroll_area;
pub mod scroll_bar;
pub mod stack;
pub mod text_input;
mod widget_trait;
pub mod window;

pub use self::{
    address::WidgetAddress,
    common::{
        EventFilterFn, WidgetCommon, WidgetCommonTyped, WidgetCreationContext, WidgetGeometry,
    },
    ext::WidgetExt,
    id::{RawWidgetId, WidgetId},
    widget_trait::Widget,
};

#[derive(Debug, Error)]
#[error("widget not found")]
pub struct WidgetNotFound;

pub fn get_widget_by_address_mut<'a>(
    root_widget: &'a mut dyn Widget,
    address: &WidgetAddress,
) -> Result<&'a mut dyn Widget, WidgetNotFound> {
    let root_address = &root_widget.common().address;

    if !address.starts_with(root_address) {
        warn!("get_widget_by_address_mut: address is not within root widget");
        return Err(WidgetNotFound);
    }
    let root_address_len = root_address.path.len();
    let mut current_widget = root_widget;
    for (key, _id) in &address.path[root_address_len..] {
        current_widget = current_widget
            .common_mut()
            .children
            .get_mut(key)
            .ok_or(WidgetNotFound)?
            .as_mut();
    }
    Ok(current_widget)
}

pub fn get_widget_by_id_mut(
    root_widget: &mut dyn Widget,
    id: RawWidgetId,
) -> Result<&mut dyn Widget, WidgetNotFound> {
    let address = address(id).ok_or(WidgetNotFound)?;
    get_widget_by_address_mut(root_widget, &address)
}

pub fn invalidate_size_hint_cache(widget: &mut dyn Widget, pending: &[WidgetAddress]) {
    let common = widget.common_mut();
    for pending_addr in pending {
        if pending_addr.starts_with(&common.address) {
            common.clear_size_hint_cache();
            for child in common.children.values_mut() {
                invalidate_size_hint_cache(child.as_mut(), pending);
            }
            return;
        }
    }
}

#[macro_export]
macro_rules! impl_widget_common {
    () => {
        fn type_name() -> &'static str {
            std::any::type_name::<Self>()
        }

        fn common(&self) -> &$crate::WidgetCommon {
            &self.common.common
        }

        fn common_mut(&mut self) -> &mut $crate::WidgetCommon {
            &mut self.common.common
        }
    };
}
