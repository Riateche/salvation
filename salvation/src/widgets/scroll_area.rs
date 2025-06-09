use {
    super::{
        scroll_bar::ScrollBar, Widget, WidgetAddress, WidgetCommonTyped, WidgetExt, WidgetGeometry,
    },
    crate::{
        event::{LayoutEvent, MouseScrollEvent},
        impl_widget_common,
        layout::{grid::grid_layout, SizeHints},
        types::{Axis, PhysicalPixels, PpxSuffix, Rect},
    },
    anyhow::Result,
    salvation_macros::impl_with,
    std::cmp::max,
};

pub struct ScrollArea {
    common: WidgetCommonTyped<Self>,
}

const INDEX_SCROLL_BAR_X: u64 = 0;
const INDEX_SCROLL_BAR_Y: u64 = 1;
const INDEX_VIEWPORT: u64 = 2;

const KEY_CONTENT_IN_VIEWPORT: u64 = 0;

#[impl_with]
impl ScrollArea {
    fn has_content(&self) -> bool {
        self.common
            .get_dyn_child(INDEX_VIEWPORT)
            .unwrap()
            .common()
            .has_child(KEY_CONTENT_IN_VIEWPORT)
    }

    // TODO: naming?
    pub fn set_content<T: Widget>(&mut self) -> &mut T {
        assert!(!self.has_content());
        self.common
            .get_dyn_child_mut(INDEX_VIEWPORT)
            .unwrap()
            .common_mut()
            .add_child_with_key::<T>(KEY_CONTENT_IN_VIEWPORT)
    }

    // pub fn set_content(&mut self, content: Box<dyn Widget>) {
    //     if self.has_content() {
    //         self.common.children[INDEX_VIEWPORT]
    //             .widget
    //             .common_mut()
    //             .remove_child(0)
    //             .unwrap();
    //     }
    //     self.common.children[INDEX_VIEWPORT]
    //         .widget
    //         .common_mut()
    //         .add_child(content, LayoutItemOptions::default());
    // }
    // TODO: take_content; default impl for empty scroll area

    // pub fn on_value_changed(&mut self, callback: Callback<i32>) {
    //     self.value_changed = Some(callback);
    // }

    // fn size_hints(&mut self) -> SizeHints {
    //     let xscroll_x = self.common.children[0].widget.cached_size_hint_x();
    //     let yscroll_x = self.common.children[1].widget.cached_size_hint_x();
    //     let content_x = if let Some(child) = self.common.children.get(2) {
    //         widget.cached_size_hint_x()
    //     } else {
    //         SizeHint::new_fallback()
    //     };

    //     let xscroll_y = self.common.children[0]
    //         .widget
    //         .cached_size_hint_y(xscroll_x.preferred);
    //     let yscroll_y = self.common.children[1]
    //         .widget
    //         .cached_size_hint_y(yscroll_x.preferred);
    //     let content_y = self.common.children[2]
    //         .widget
    //         .cached_size_hint_y(content_x.preferred);
    //     SizeHints {
    //         xscroll_x,
    //         yscroll_x,
    //         content_x,
    //         xscroll_y: xscroll_y,
    //         yscroll_y,
    //         content_y,
    //     }
    // }

    fn relayout(&mut self, changed_size_hints: &[WidgetAddress]) -> Result<()> {
        let geometry = self.common.geometry_or_err()?.clone();
        grid_layout(self, changed_size_hints);

        if self.has_content() {
            let value_x = self
                .common
                .get_child::<ScrollBar>(INDEX_SCROLL_BAR_X)
                .unwrap()
                .value();
            let value_y = self
                .common
                .get_child::<ScrollBar>(INDEX_SCROLL_BAR_Y)
                .unwrap()
                .value();

            let Some(viewport_rect) = self
                .common
                .get_dyn_child_mut(INDEX_VIEWPORT)
                .unwrap()
                .common_mut()
                .rect_in_parent()
            else {
                return Ok(());
            };
            let content_size_x = self
                .common
                .get_dyn_child_mut(INDEX_VIEWPORT)
                .unwrap()
                .common_mut()
                .get_dyn_child_mut(KEY_CONTENT_IN_VIEWPORT)
                .unwrap()
                .size_hint_x()
                .preferred;
            let content_size_y = self
                .common
                .get_dyn_child_mut(INDEX_VIEWPORT)
                .unwrap()
                .common_mut()
                .get_dyn_child_mut(KEY_CONTENT_IN_VIEWPORT)
                .unwrap()
                .size_hint_y(content_size_x)
                .preferred;
            let content_rect = Rect::from_xywh(
                PhysicalPixels::from_i32(-value_x),
                PhysicalPixels::from_i32(-value_y),
                content_size_x,
                content_size_y,
            );
            self.common
                .get_dyn_child_mut(INDEX_VIEWPORT)
                .unwrap()
                .common_mut()
                .get_dyn_child_mut(KEY_CONTENT_IN_VIEWPORT)
                .unwrap()
                .set_geometry(
                    Some(WidgetGeometry::new(&geometry, content_rect)),
                    changed_size_hints,
                );

            let max_value_x = max(0.ppx(), content_size_x - viewport_rect.size_x());
            let max_value_y = max(0.ppx(), content_size_y - viewport_rect.size_y());
            self.common
                .get_child_mut::<ScrollBar>(INDEX_SCROLL_BAR_X)
                .unwrap()
                .set_value_range(0..=max_value_x.to_i32());
            self.common
                .get_child_mut::<ScrollBar>(INDEX_SCROLL_BAR_Y)
                .unwrap()
                .set_value_range(0..=max_value_y.to_i32());
        }
        Ok(())
    }
}

impl Widget for ScrollArea {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        let relayout = common.callback(|this, _| this.relayout(&[]));

        // TODO: icons, localized name
        common
            .add_child_with_key::<ScrollBar>(INDEX_SCROLL_BAR_X)
            .set_column(0)
            .set_row(1)
            .on_value_changed(relayout.clone());
        common
            .add_child_with_key::<ScrollBar>(INDEX_SCROLL_BAR_Y)
            .set_column(1)
            .set_row(0)
            .set_axis(Axis::Y)
            .on_value_changed(relayout);
        common
            .add_child_with_key::<Viewport>(INDEX_VIEWPORT)
            .set_column(0)
            .set_row(0);
        Self { common }
    }

    fn handle_layout(&mut self, event: LayoutEvent) -> Result<()> {
        self.relayout(&event.changed_size_hints)
    }

    fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> Result<bool> {
        let delta = event.unified_delta(&self.common);

        let scroll_x = self
            .common
            .get_child_mut::<ScrollBar>(INDEX_SCROLL_BAR_X)
            .unwrap();
        let new_value_x = scroll_x.value() - delta.x.round() as i32;
        scroll_x.set_value(new_value_x.clamp(
            *scroll_x.value_range().start(),
            *scroll_x.value_range().end(),
        ));

        let scroll_y = self
            .common
            .get_child_mut::<ScrollBar>(INDEX_SCROLL_BAR_Y)
            .unwrap();
        let new_value_y = scroll_y.value() - delta.y.round() as i32;
        scroll_y.set_value(new_value_y.clamp(
            *scroll_y.value_range().start(),
            *scroll_y.value_range().end(),
        ));
        Ok(true)
    }
}

pub struct Viewport {
    common: WidgetCommonTyped<Self>,
}

impl Viewport {}

impl Widget for Viewport {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }

    fn handle_size_hint_x_request(&mut self) -> Result<SizeHints> {
        Ok(SizeHints {
            min: 0.ppx(),
            preferred: 0.ppx(),
            is_fixed: false,
        })
    }
    fn handle_size_hint_y_request(&mut self, _size_x: PhysicalPixels) -> Result<SizeHints> {
        Ok(SizeHints {
            min: 0.ppx(),
            preferred: 0.ppx(),
            is_fixed: false,
        })
    }
}
