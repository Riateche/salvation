use std::{
    cmp::{max, min},
    ops::RangeInclusive,
};

use self::grid::GridAxisOptions;

pub mod grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizeHint {
    // TODO: PhysicalPixels
    pub value: i32,
    pub is_fixed: bool,
}

const FALLBACK_SIZE_HINT: i32 = 48;

impl SizeHint {
    pub fn new(value: i32, is_fixed: bool) -> Self {
        Self { value, is_fixed }
    }

    pub fn new_fixed(value: i32) -> Self {
        Self {
            value,
            is_fixed: true,
        }
    }

    pub fn new_expanding(value: i32) -> Self {
        Self {
            value,
            is_fixed: false,
        }
    }

    pub fn new_fallback() -> Self {
        SizeHint {
            value: FALLBACK_SIZE_HINT,
            is_fixed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LayoutItemOptions {
    pub x: LayoutItemAxisOptions,
    pub y: LayoutItemAxisOptions,
}

impl LayoutItemOptions {
    pub fn from_pos_in_grid(pos_x: i32, pos_y: i32) -> Self {
        Self {
            x: LayoutItemAxisOptions::new(pos_x),
            y: LayoutItemAxisOptions::new(pos_y),
        }
    }

    pub fn is_in_grid(&self) -> bool {
        self.x.pos_in_grid.is_some() && self.y.pos_in_grid.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct LayoutItemAxisOptions {
    // row or column
    pub pos_in_grid: Option<RangeInclusive<i32>>,
    pub alignment: Option<Alignment>,
    // TODO: alignment, priority, stretch, etc.
}

impl LayoutItemAxisOptions {
    pub fn new(pos: i32) -> Self {
        Self {
            pos_in_grid: Some(pos..=pos),
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Alignment {
    Start,
    Middle,
    End,
}

pub(crate) fn fare_split(count: i32, total: i32) -> Vec<i32> {
    if count == 0 {
        return Vec::new();
    }
    let per_item = (total as f32) / (count as f32);
    let mut prev = 0;
    let mut results = Vec::new();
    for i in 1..=count {
        let next = (per_item * (i as f32)).round() as i32;
        results.push(next - prev);
        prev = next;
    }
    results
}

pub(crate) struct LayoutItem {
    pub(crate) size_hints: SizeHint,
    // TODO: params
}

pub(crate) struct SolveLayoutOutput {
    pub(crate) sizes: Vec<i32>,
    pub(crate) padding: i32,
    pub(crate) spacing: i32,
}

// TODO: support spanned items
pub(crate) fn solve_layout(
    items: &[LayoutItem],
    mut total_available: i32,
    options: &GridAxisOptions,
) -> SolveLayoutOutput {
    let mut output = SolveLayoutOutput {
        sizes: Vec::new(),
        padding: options.padding,
        spacing: options.spacing - options.border_collapse,
    };
    if items.is_empty() {
        return output;
    }
    total_available = max(
        0,
        total_available
            - 2 * options.padding
            - items.len().saturating_sub(1) as i32 * (options.spacing - options.border_collapse),
    );
    let total_min: i32 = items.iter().map(|item| item.size_hints.value).sum();
    if total_min == total_available {
        output.sizes = items.iter().map(|item| item.size_hints.value).collect();
        return output;
    } else if total_min < total_available {
        let num_flexible = items
            .iter()
            .filter(|item| !item.size_hints.is_fixed)
            .count() as i32;
        let mut remaining = total_available;
        let mut extras = fare_split(num_flexible, max(0, total_available - total_min));
        for item in items {
            let item_size = if item.size_hints.is_fixed {
                item.size_hints.value
            } else {
                item.size_hints.value + extras.pop().unwrap()
            };
            let item_size = min(item_size, remaining);
            output.sizes.push(item_size);
            remaining -= item_size;
            if remaining == 0 {
                break;
            }
        }
    }
    while output.sizes.len() < items.len() {
        output.sizes.push(0);
    }

    output
}
