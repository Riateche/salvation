use std::cmp::max;

use anyhow::Result;
use salvation_macros::impl_with;

use crate::{
    callback::widget_callback,
    event::LayoutEvent,
    layout::{
        grid::{self, GridAxisOptions, GridOptions},
        LayoutItemOptions, SizeHintMode,
    },
    types::{Axis, Rect},
};

use super::{scroll_bar::ScrollBar, Widget, WidgetCommon, WidgetExt, WidgetId};

pub struct ScrollArea {
    common: WidgetCommon,
}

const INDEX_SCROLL_BAR_X: usize = 0;
const INDEX_SCROLL_BAR_Y: usize = 1;
const INDEX_VIEWPORT: usize = 2;
const INDEX_CONTENT: usize = 3;

#[impl_with]
impl ScrollArea {
    pub fn new(content: Box<dyn Widget>) -> Self {
        let mut this = Self::default();
        this.set_content(content);
        this
    }

    fn has_content(&self) -> bool {
        self.common.children.len() > INDEX_CONTENT
    }

    pub fn set_content(&mut self, content: Box<dyn Widget>) {
        if self.has_content() {
            self.common.remove_child(INDEX_CONTENT).unwrap();
        }
        self.common.add_child(content, LayoutItemOptions::default());
    }
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
    fn grid_options(&self) -> GridOptions {
        GridOptions {
            x: GridAxisOptions {
                min_padding: 0,
                min_spacing: 0,
                preferred_padding: 0,
                preferred_spacing: 0,
            },
            y: GridAxisOptions {
                min_padding: 0,
                min_spacing: 0,
                preferred_padding: 0,
                preferred_spacing: 0,
            },
        }
    }

    fn relayout(&mut self) -> Result<()> {
        let options = self.grid_options();
        let size = self.common.size_or_err()?;
        let mut rects = grid::layout(&mut self.common.children, &options, size)?;
        if self.has_content() {
            let value_x = self.common.children[INDEX_SCROLL_BAR_X]
                .widget
                .downcast_ref::<ScrollBar>()
                .unwrap()
                .value();
            let value_y = self.common.children[INDEX_SCROLL_BAR_Y]
                .widget
                .downcast_ref::<ScrollBar>()
                .unwrap()
                .value();
            println!("value_x={value_x}, value_y={value_y}");

            let viewport_rect = *rects.get(&INDEX_VIEWPORT).unwrap();
            let content_size_x = self.common.children[INDEX_CONTENT]
                .widget
                .cached_size_hint_x(SizeHintMode::Preferred);
            let content_size_y = self.common.children[INDEX_CONTENT]
                .widget
                .cached_size_hint_y(content_size_x, SizeHintMode::Preferred);
            let content_rect = Rect::from_xywh(-value_x, -value_y, content_size_x, content_size_y)
                .translate(viewport_rect.top_left);
            rects.insert(INDEX_CONTENT, content_rect);

            let max_value_x = max(0, content_size_x - viewport_rect.size.x);
            let max_value_y = max(0, content_size_y - viewport_rect.size.y);
            println!("max_value_x={max_value_x}, max_value_y={max_value_y}");
            self.common.children[INDEX_SCROLL_BAR_X]
                .widget
                .downcast_mut::<ScrollBar>()
                .unwrap()
                .set_value_range(0..=max_value_x);
            self.common.children[INDEX_SCROLL_BAR_Y]
                .widget
                .downcast_mut::<ScrollBar>()
                .unwrap()
                .set_value_range(0..=max_value_y);
        }
        self.common.set_child_rects(&rects)?;
        Ok(())
    }
}

impl Default for ScrollArea {
    fn default() -> Self {
        let mut common = WidgetCommon::new();

        let relayout = widget_callback(WidgetId::<Self>::new(common.id), |this, _: i32| {
            this.relayout()
        });
        // TODO: icons, localized name
        common.add_child(
            ScrollBar::new()
                .with_on_value_changed(relayout.clone())
                .boxed(),
            LayoutItemOptions::from_pos_in_grid(0, 1),
        );
        common.add_child(
            ScrollBar::new()
                .with_axis(Axis::Y)
                .with_on_value_changed(relayout)
                .boxed(),
            LayoutItemOptions::from_pos_in_grid(1, 0),
        );
        common.add_child(
            Viewport::new().boxed(),
            LayoutItemOptions::from_pos_in_grid(0, 0),
        );
        Self { common }
    }
}

impl Widget for ScrollArea {
    fn common(&self) -> &super::WidgetCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::WidgetCommon {
        &mut self.common
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        self.relayout()
    }

    fn size_hint_x(&mut self, mode: SizeHintMode) -> Result<i32> {
        let options = self.grid_options();
        grid::size_hint_x(&mut self.common.children, &options, mode)
    }
    fn is_size_hint_x_fixed(&mut self) -> bool {
        let options = self.grid_options();
        grid::is_size_hint_x_fixed(&mut self.common.children, &options)
    }
    fn is_size_hint_y_fixed(&mut self) -> bool {
        let options = self.grid_options();
        grid::is_size_hint_y_fixed(&mut self.common.children, &options)
    }
    fn size_hint_y(&mut self, size_x: i32, mode: SizeHintMode) -> Result<i32> {
        let options = self.grid_options();
        grid::size_hint_y(&mut self.common.children, &options, size_x, mode)
    }
}

// TODO: public type for empty widget?
struct Viewport {
    common: WidgetCommon,
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            common: WidgetCommon::new(),
        }
    }
}

impl Widget for Viewport {
    fn common(&self) -> &WidgetCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn size_hint_x(&mut self, _mode: SizeHintMode) -> Result<i32> {
        Ok(0)
    }
    fn size_hint_y(&mut self, _size_x: i32, _mode: SizeHintMode) -> Result<i32> {
        Ok(0)
    }
    fn is_size_hint_x_fixed(&mut self) -> bool {
        false
    }
    fn is_size_hint_y_fixed(&mut self) -> bool {
        false
    }
}
