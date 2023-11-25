use std::{
    cmp::{max, min},
    ops::RangeInclusive,
};

use self::grid::GridAxisOptions;

pub mod grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SizeHints {
    // TODO: PhysicalPixels
    pub preferred: i32,
    pub is_fixed: bool,
}

// impl SizeHint {
//     pub fn new_fallback() -> Self {
//         SizeHint {
//             min: 48,
//             preferred: 48,
//             is_fixed: true,
//         }
//     }
// }

pub const FALLBACK_SIZE_HINT: i32 = 48;

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
    pub(crate) size_hints: SizeHints,
    // TODO: params
}

pub(crate) struct SolveLayoutOutput {
    pub(crate) sizes: Vec<i32>,
    pub(crate) padding: i32,
    pub(crate) spacing: i32,
}

// TODO: chose min/preferred spacing and padding
// TODO: support spanned items
pub(crate) fn solve_layout(
    items: &[LayoutItem],
    mut total: i32,
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
    total = max(
        0,
        total
            - 2 * options.padding
            - items.len().saturating_sub(1) as i32 * (options.spacing - options.border_collapse),
    );
    let total_preferred: i32 = items.iter().map(|item| item.size_hints.preferred).sum();
    if total_preferred == total {
        output.sizes = items.iter().map(|item| item.size_hints.preferred).collect();
        return output;
    } else if total_preferred < total {
        let num_flexible = items
            .iter()
            .filter(|item| !item.size_hints.is_fixed)
            .count() as i32;
        let mut remaining = total;
        let mut extras = fare_split(num_flexible, max(0, total - total_preferred));
        for item in items {
            let item_size = if item.size_hints.is_fixed {
                item.size_hints.preferred
            } else {
                item.size_hints.preferred + extras.pop().unwrap()
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
