use std::{
    cmp::{max, min},
    collections::{BTreeMap, HashMap},
    ops::RangeInclusive,
};

use crate::{
    layout::{fare_split, solve_layout},
    types::{Rect, Size},
    widgets::{Child, WidgetExt},
};

use anyhow::Result;
use itertools::Itertools;
use log::warn;

pub struct GridOptions {
    pub x: GridAxisOptions,
    pub y: GridAxisOptions,
}

pub struct GridAxisOptions {
    pub padding: i32,
    pub spacing: i32,
    pub border_collapse: i32,
}

fn size_hint(items: &[(RangeInclusive<i32>, i32)], options: &GridAxisOptions) -> Result<i32> {
    let mut max_per_column = BTreeMap::new();
    let mut spanned = Vec::new();
    for (pos, hint) in items {
        if pos.start() == pos.end() {
            let value = max_per_column.entry(*pos.start()).or_default();
            *value = max(*value, *hint);
        } else if pos.start() > pos.end() {
            warn!("invalid pos_in_grid range");
            continue;
        } else {
            spanned.push((pos, *hint));
        }
    }
    for (range, hint) in spanned {
        let current: i32 = range
            .clone()
            .map(|pos| max_per_column.get(&pos).copied().unwrap_or(0))
            .sum();
        if hint > current {
            let extra_per_column = fare_split(*range.end() - *range.start() + 1, hint - current);
            for (pos, extra) in range.clone().zip(extra_per_column) {
                *max_per_column.entry(pos).or_default() += extra;
            }
        }
    }
    Ok(max_per_column.values().sum::<i32>()
        + 2 * options.padding
        + max_per_column.len().saturating_sub(1) as i32
            * (options.spacing - options.border_collapse))
}

pub fn size_hint_x(items: &mut [Child], options: &GridOptions) -> Result<i32> {
    // TODO: exclude hidden widgets
    let item_data = items
        .iter_mut()
        .filter(|item| item.options.is_in_grid())
        .map(|item| {
            (
                item.options.x.pos_in_grid.clone().unwrap(),
                item.widget.cached_size_hint_x(),
            )
        })
        .collect_vec();
    size_hint(&item_data, &options.x)
}

pub fn size_hint_y(items: &mut [Child], options: &GridOptions, size_x: i32) -> Result<i32> {
    let x_layout = x_layout(items, &options.x, size_x)?;
    let mut item_data = Vec::new();
    for (index, item) in items.iter_mut().enumerate() {
        if !item.options.is_in_grid() {
            continue;
        }
        let Some(item_size_x) = x_layout.child_sizes.get(&index) else {
            continue;
        };
        let pos = item.options.y.pos_in_grid.clone().unwrap();
        let hints = item.widget.cached_size_hint_y(*item_size_x);
        item_data.push((pos, hints));
    }
    size_hint(&item_data, &options.y)
}

// TODO: skip hidden, check item options
pub fn is_size_hint_x_fixed(items: &mut [Child], _options: &GridOptions) -> bool {
    items
        .iter_mut()
        .all(|item| !item.options.is_in_grid() || item.widget.cached_size_hint_x_fixed())
}
pub fn is_size_hint_y_fixed(items: &mut [Child], _options: &GridOptions) -> bool {
    items
        .iter_mut()
        .all(|item| !item.options.is_in_grid() || item.widget.cached_size_hint_y_fixed())
}

struct XLayout {
    padding: i32,
    spacing: i32,
    column_sizes: BTreeMap<i32, i32>,
    child_sizes: HashMap<usize, i32>,
}

fn x_layout(items: &mut [Child], options: &GridAxisOptions, size_x: i32) -> Result<XLayout> {
    let mut hints_per_column = BTreeMap::new();
    for item in items.iter_mut() {
        if !item.options.is_in_grid() {
            continue;
        }
        let Some(pos) = item.options.x.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let pos = *pos.start();
        let hints = item.widget.cached_size_hints_x();
        let column_hints = hints_per_column.entry(pos).or_insert(hints);
        column_hints.value = max(column_hints.value, hints.value);
        column_hints.is_fixed = column_hints.is_fixed && hints.is_fixed;
    }
    let layout_items = hints_per_column
        .values()
        .map(|hints| super::LayoutItem { size_hints: *hints })
        .collect_vec();
    let output = solve_layout(&layout_items, size_x, options);
    let column_sizes: BTreeMap<_, _> = hints_per_column.keys().copied().zip(output.sizes).collect();
    let mut child_sizes = HashMap::new();
    for (index, item) in items.iter_mut().enumerate() {
        if !item.options.is_in_grid() {
            continue;
        }
        let Some(pos) = item.options.x.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let Some(column_size) = column_sizes.get(pos.start()) else {
            warn!("missing column data for existing child");
            continue;
        };
        let child_size = if item.widget.cached_size_hint_x_fixed() {
            let hint = item.widget.cached_size_hint_x();
            min(hint, *column_size)
        } else {
            *column_size
        };
        child_sizes.insert(index, child_size);
    }
    Ok(XLayout {
        padding: output.padding,
        spacing: output.spacing,
        column_sizes,
        child_sizes,
    })
}

pub fn layout(
    items: &mut [Child],
    options: &GridOptions,
    size: Size,
) -> Result<BTreeMap<usize, Rect>> {
    let x_layout = x_layout(items, &options.x, size.x)?;
    let mut hints_per_row = BTreeMap::new();
    for (index, item) in items.iter_mut().enumerate() {
        if !item.options.is_in_grid() {
            continue;
        }
        let Some(pos) = item.options.y.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let Some(item_size_x) = x_layout.child_sizes.get(&index) else {
            continue;
        };
        let pos = *pos.start();
        let hints = item.widget.cached_size_hints_y(*item_size_x);
        let row_hints = hints_per_row.entry(pos).or_insert(hints);
        row_hints.value = max(row_hints.value, hints.value);
        row_hints.is_fixed = row_hints.is_fixed && hints.is_fixed;
        // TODO: deduplicate
    }
    let layout_items = hints_per_row
        .values()
        .map(|hints| super::LayoutItem { size_hints: *hints })
        .collect_vec();
    let output_y = solve_layout(&layout_items, size.y, &options.y);
    let row_sizes: BTreeMap<_, _> = hints_per_row.keys().copied().zip(output_y.sizes).collect();
    let positions_x = positions(&x_layout.column_sizes, x_layout.padding, x_layout.spacing);
    let positions_y = positions(&row_sizes, output_y.padding, output_y.spacing);
    let mut result = BTreeMap::new();
    for (index, item) in items.iter_mut().enumerate() {
        let Some(pos_x) = item.options.x.pos_in_grid.clone() else {
            continue;
        };
        let Some(pos_y) = item.options.y.pos_in_grid.clone() else {
            continue;
        };
        let Some(cell_pos_x) = positions_x.get(pos_x.start()) else {
            warn!("missing item in positions_x");
            continue;
        };
        let Some(cell_pos_y) = positions_y.get(pos_y.start()) else {
            warn!("missing item in positions_y");
            continue;
        };
        let Some(size_x) = x_layout.child_sizes.get(&index) else {
            warn!("missing item in x_layout.child_sizes");
            continue;
        };
        let Some(row_size) = row_sizes.get(pos_y.start()) else {
            warn!("missing item in row_sizes");
            continue;
        };
        let size_y = if item.widget.cached_size_hint_y_fixed() {
            min(*row_size, item.widget.cached_size_hint_y(*size_x))
        } else {
            *row_size
        };
        result.insert(
            index,
            Rect::from_xywh(*cell_pos_x, *cell_pos_y, *size_x, size_y),
        );
    }
    Ok(result)
}

fn positions(sizes: &BTreeMap<i32, i32>, padding: i32, spacing: i32) -> BTreeMap<i32, i32> {
    let mut pos = padding;
    let mut result = BTreeMap::new();
    for (num, size) in sizes {
        result.insert(*num, pos);
        pos += *size + spacing;
    }
    result
}
