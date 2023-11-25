use anyhow::{Context, Result};
use salvation_macros::impl_with;

use crate::{
    event::LayoutEvent,
    layout::{
        grid::{self, GridAxisOptions, GridOptions},
        Alignment, LayoutItemOptions,
    },
};

use super::{Widget, WidgetCommon};

// TODO: get from style, apply scale
const SPACING: i32 = 10;

pub struct Column {
    // TODO: add layout options
    common: WidgetCommon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Options {
    pub alignment: Option<Alignment>,
    // TODO: alignment, priority, stretch, etc.
}

#[impl_with]
impl Column {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            common: WidgetCommon::new(),
        }
    }

    pub fn add_child(&mut self, widget: Box<dyn Widget>) {
        let row = self.common.children.len();
        self.common
            .add_child(widget, LayoutItemOptions::from_pos_in_grid(0, row as i32));
        self.common.update();
    }

    pub fn set_options(&mut self, index: usize, options: Options) -> Result<()> {
        let mut all_options = self
            .common
            .children
            .get(index)
            .context("invalid child index")?
            .options
            .clone();
        all_options.x.alignment = options.alignment;
        self.common.set_child_options(index, all_options)
    }

    pub fn and_options(mut self, options: Options) -> Self {
        let index = self.common.children.len();
        self.set_options(index, options)
            .expect("should not fail with correct index");
        self
    }

    fn grid_options(&self) -> GridOptions {
        GridOptions {
            x: GridAxisOptions {
                padding: 0,
                spacing: 0,
                border_collapse: 0,
            },
            y: GridAxisOptions {
                padding: 0,
                spacing: SPACING,
                border_collapse: 0,
            },
        }
    }
}

impl Widget for Column {
    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        let options = self.grid_options();
        let size = self.common.size_or_err()?;
        let rects = grid::layout(&mut self.common.children, &options, size)?;
        self.common.set_child_rects(&rects)?;
        Ok(())
    }

    fn size_hint_x(&mut self) -> Result<i32> {
        let options = self.grid_options();
        grid::size_hint_x(&mut self.common.children, &options)
    }
    fn is_size_hint_x_fixed(&mut self) -> bool {
        let options = self.grid_options();
        grid::is_size_hint_x_fixed(&mut self.common.children, &options)
    }
    fn is_size_hint_y_fixed(&mut self) -> bool {
        let options = self.grid_options();
        grid::is_size_hint_y_fixed(&mut self.common.children, &options)
    }
    fn size_hint_y(&mut self, size_x: i32) -> Result<i32> {
        let options = self.grid_options();
        grid::size_hint_y(&mut self.common.children, &options, size_x)
    }
}
