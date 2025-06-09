use {
    super::{Alignment, SizeHintMode, SizeHints},
    crate::{
        key::Key,
        layout::{fair_split, solve_layout},
        types::{PhysicalPixels, PpxSuffix, Rect},
        widgets::{Widget, WidgetAddress, WidgetExt, WidgetGeometry},
    },
    itertools::Itertools,
    log::warn,
    std::{
        cmp::{max, min},
        collections::{BTreeMap, HashMap},
        ops::RangeInclusive,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GridOptions {
    pub x: GridAxisOptions,
    pub y: GridAxisOptions,
}

impl GridOptions {
    pub const ZERO: Self = GridOptions {
        x: GridAxisOptions {
            min_padding: PhysicalPixels::ZERO,
            min_spacing: PhysicalPixels::ZERO,
            preferred_padding: PhysicalPixels::ZERO,
            preferred_spacing: PhysicalPixels::ZERO,
            border_collapse: PhysicalPixels::ZERO,
            alignment: Alignment::Start,
        },
        y: GridAxisOptions {
            min_padding: PhysicalPixels::ZERO,
            min_spacing: PhysicalPixels::ZERO,
            preferred_padding: PhysicalPixels::ZERO,
            preferred_spacing: PhysicalPixels::ZERO,
            border_collapse: PhysicalPixels::ZERO,
            alignment: Alignment::Start,
        },
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GridAxisOptions {
    pub min_padding: PhysicalPixels,
    pub min_spacing: PhysicalPixels,
    pub preferred_padding: PhysicalPixels,
    pub preferred_spacing: PhysicalPixels,
    pub border_collapse: PhysicalPixels,
    pub alignment: Alignment,
}

fn size_hint(
    items: &[(RangeInclusive<i32>, PhysicalPixels)],
    options: &GridAxisOptions,
    mode: SizeHintMode,
) -> PhysicalPixels {
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
        let current: PhysicalPixels = range
            .clone()
            .map(|pos| max_per_column.get(&pos).copied().unwrap_or(0.ppx()))
            .sum();
        if hint > current {
            let extra_per_column = fair_split(*range.end() - *range.start() + 1, hint - current);
            for (pos, extra) in range.clone().zip(extra_per_column) {
                *max_per_column.entry(pos).or_default() += extra;
            }
        }
    }
    let (padding, spacing) = match mode {
        SizeHintMode::Min => (options.min_padding, options.min_spacing),
        SizeHintMode::Preferred => (options.preferred_padding, options.preferred_spacing),
    };
    max_per_column.values().sum::<PhysicalPixels>()
        + 2 * padding
        + max_per_column.len().saturating_sub(1) as i32 * (spacing - options.border_collapse)
}

pub fn size_hint_x(items: &mut BTreeMap<Key, Box<dyn Widget>>, options: &GridOptions) -> SizeHints {
    let mut min_items = Vec::new();
    let mut preferred_items = Vec::new();
    let mut all_fixed = true;
    for item in items.values_mut() {
        if !(item.common().layout_item_options.is_in_grid()
            && !item.common().is_window_root
            && item.common().is_self_visible)
        {
            continue;
        }

        let hints = item.size_hint_x();
        let pos_in_grid = item
            .common()
            .layout_item_options
            .x
            .pos_in_grid
            .clone()
            .unwrap();
        min_items.push((pos_in_grid.clone(), hints.min));
        preferred_items.push((pos_in_grid, hints.preferred));

        let is_fixed = item
            .common()
            .layout_item_options
            .x
            .is_fixed
            .unwrap_or(hints.is_fixed);

        if !is_fixed {
            all_fixed = false;
        }
    }
    SizeHints {
        min: size_hint(&min_items, &options.x, SizeHintMode::Min),
        preferred: size_hint(&preferred_items, &options.x, SizeHintMode::Preferred),
        is_fixed: all_fixed,
    }
}

pub fn size_hint_y(
    items: &mut BTreeMap<Key, Box<dyn Widget>>,
    options: &GridOptions,
    size_x: PhysicalPixels,
) -> SizeHints {
    let x_layout = x_layout(items, &options.x, size_x);
    let mut min_items = Vec::new();
    let mut preferred_items = Vec::new();
    let mut all_fixed = true;
    for (key, item) in items.iter_mut() {
        if !item.common().layout_item_options.is_in_grid()
            || item.common().is_window_root
            || !item.common().is_self_visible
        {
            continue;
        }
        let Some(item_size_x) = x_layout.child_sizes.get(key) else {
            continue;
        };
        let pos_in_grid = item
            .common()
            .layout_item_options
            .y
            .pos_in_grid
            .clone()
            .unwrap();
        let hints = item.size_hint_y(*item_size_x);

        min_items.push((pos_in_grid.clone(), hints.min));
        preferred_items.push((pos_in_grid, hints.preferred));

        let is_fixed = item
            .common()
            .layout_item_options
            .x
            .is_fixed
            .unwrap_or(hints.is_fixed);

        if !is_fixed {
            all_fixed = false;
        }
    }
    SizeHints {
        min: size_hint(&min_items, &options.y, SizeHintMode::Min),
        preferred: size_hint(&preferred_items, &options.y, SizeHintMode::Preferred),
        is_fixed: all_fixed,
    }
}

struct XLayout {
    padding: PhysicalPixels,
    spacing: PhysicalPixels,
    column_sizes: BTreeMap<i32, PhysicalPixels>,
    child_sizes: HashMap<Key, PhysicalPixels>,
}

fn x_layout(
    items: &mut BTreeMap<Key, Box<dyn Widget>>,
    options: &GridAxisOptions,
    size_x: PhysicalPixels,
) -> XLayout {
    let mut hints_per_column = BTreeMap::new();
    for item in items.values_mut() {
        if !item.common().layout_item_options.is_in_grid()
            || item.common().is_window_root
            || !item.common().is_self_visible
        {
            continue;
        }
        let Some(pos) = item.common().layout_item_options.x.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let pos = *pos.start();
        let mut hints = item.size_hint_x();
        if let Some(is_fixed) = item.common().layout_item_options.x.is_fixed {
            hints.is_fixed = is_fixed;
        }
        let column_hints = hints_per_column.entry(pos).or_insert(hints);
        column_hints.min = max(column_hints.min, hints.min);
        column_hints.preferred = max(column_hints.preferred, hints.preferred);
        column_hints.is_fixed = column_hints.is_fixed && hints.is_fixed;
    }
    let layout_items = hints_per_column
        .values()
        .map(|hints| super::LayoutItem { size_hints: *hints })
        .collect_vec();
    let output = solve_layout(&layout_items, size_x, options);
    let column_sizes: BTreeMap<_, _> = hints_per_column.keys().copied().zip(output.sizes).collect();
    let mut child_sizes = HashMap::new();
    for (key, item) in items.iter_mut() {
        if !item.common().layout_item_options.is_in_grid()
            || item.common().is_window_root
            || !item.common().is_self_visible
        {
            continue;
        }
        let Some(pos) = item.common().layout_item_options.x.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let Some(column_size) = column_sizes.get(pos.start()) else {
            warn!("missing column data for existing child");
            continue;
        };
        let child_size = if item
            .common()
            .layout_item_options
            .x
            .is_fixed
            .unwrap_or_else(|| item.size_hint_x().is_fixed)
        {
            let hint = item.size_hint_x().preferred;
            min(hint, *column_size)
        } else {
            *column_size
        };
        child_sizes.insert(key.clone(), child_size);
    }
    XLayout {
        padding: output.padding,
        spacing: output.spacing,
        column_sizes,
        child_sizes,
    }
}

pub fn grid_layout<W: Widget + ?Sized>(widget: &mut W, changed_size_hints: &[WidgetAddress]) {
    let Some(geometry) = widget.common().geometry.clone() else {
        for child in widget.common_mut().children.values_mut() {
            child.set_geometry(None, changed_size_hints);
        }
        return;
    };
    let options = widget.common().common_style.grid.clone();
    let x_layout = x_layout(
        &mut widget.common_mut().children,
        &options.x,
        geometry.size_x(),
    );
    let mut hints_per_row = BTreeMap::new();
    for (key, item) in &mut widget.common_mut().children {
        if !item.common().layout_item_options.is_in_grid() || item.common().is_window_root {
            //|| !item.common().is_self_visible {
            continue;
        }
        let Some(pos) = item.common().layout_item_options.y.pos_in_grid.clone() else {
            continue;
        };
        if pos.start() != pos.end() {
            warn!("spanned items are not supported yet");
        }
        let Some(item_size_x) = x_layout.child_sizes.get(key) else {
            continue;
        };
        let pos = *pos.start();
        let mut hints = item.size_hint_y(*item_size_x);
        if let Some(is_fixed) = item.common().layout_item_options.y.is_fixed {
            hints.is_fixed = is_fixed;
        }
        let row_hints = hints_per_row.entry(pos).or_insert(hints);
        row_hints.min = max(row_hints.min, hints.min);
        row_hints.preferred = max(row_hints.preferred, hints.preferred);
        row_hints.is_fixed = row_hints.is_fixed && hints.is_fixed;
        // TODO: deduplicate
    }
    let layout_items = hints_per_row
        .values()
        .map(|hints| super::LayoutItem { size_hints: *hints })
        .collect_vec();
    let output_y = solve_layout(&layout_items, geometry.size_y(), &options.y);
    let row_sizes: BTreeMap<_, _> = hints_per_row.keys().copied().zip(output_y.sizes).collect();
    let positions_x = positions(
        &x_layout.column_sizes,
        x_layout.padding,
        x_layout.spacing,
        geometry.size_x(),
        options.x.alignment,
    );
    let positions_y = positions(
        &row_sizes,
        output_y.padding,
        output_y.spacing,
        geometry.size_y(),
        options.y.alignment,
    );
    for (key, item) in &mut widget.common_mut().children {
        // if !item.common().is_self_visible {
        //     continue;
        // }
        let Some(pos_x) = item.common().layout_item_options.x.pos_in_grid.clone() else {
            continue;
        };
        let Some(pos_y) = item.common().layout_item_options.y.pos_in_grid.clone() else {
            continue;
        };
        let Some(cell_pos_x) = positions_x.get(pos_x.start()) else {
            continue;
        };
        let Some(cell_pos_y) = positions_y.get(pos_y.start()) else {
            warn!("missing item in positions_y");
            continue;
        };
        let Some(size_x) = x_layout.child_sizes.get(key) else {
            warn!("missing item in x_layout.child_sizes");
            continue;
        };
        let Some(row_size) = row_sizes.get(pos_y.start()) else {
            warn!("missing item in row_sizes");
            continue;
        };
        let size_hint_y = item.size_hint_y(*size_x);
        let size_y = if item
            .common()
            .layout_item_options
            .y
            .is_fixed
            .unwrap_or(size_hint_y.is_fixed)
        {
            min(*row_size, size_hint_y.preferred)
        } else {
            *row_size
        };
        item.set_geometry(
            Some(WidgetGeometry::new(
                &geometry,
                Rect::from_xywh(*cell_pos_x, *cell_pos_y, *size_x, size_y),
            )),
            changed_size_hints,
        );
    }
}

fn positions(
    sizes: &BTreeMap<i32, PhysicalPixels>,
    padding: PhysicalPixels,
    spacing: PhysicalPixels,
    total_available: PhysicalPixels,
    alignment: Alignment,
) -> BTreeMap<i32, PhysicalPixels> {
    let mut pos = padding;
    let total_taken: PhysicalPixels = sizes.values().sum();
    let available_for_items =
        total_available - 2 * padding - spacing * sizes.len().saturating_sub(1) as i32;
    match alignment {
        Alignment::Start => {}
        Alignment::Middle => {
            pos += (available_for_items - total_taken) / 2;
        }
        Alignment::End => {
            pos += available_for_items - total_taken;
        }
    }
    let mut result = BTreeMap::new();
    for (num, size) in sizes {
        result.insert(*num, pos);
        pos += *size + spacing;
    }
    result
}
