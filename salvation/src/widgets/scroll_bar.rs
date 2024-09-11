use std::{
    cmp::{max, min},
    ops::RangeInclusive,
};

use crate::{
    callback::Callback,
    event::{Event, LayoutEvent},
    impl_widget_common,
    layout::{
        grid::{self, GridAxisOptions, GridOptions},
        LayoutItemOptions, SizeHintMode,
    },
    types::{Axis, Point, Rect, Size},
};
use anyhow::Result;
use log::warn;
use salvation_macros::impl_with;
use winit::event::{ElementState, MouseButton};

use super::{
    button::{self, Button},
    Widget, WidgetCommon, WidgetExt,
};

pub struct ScrollBar {
    common: WidgetCommon,
    axis: Axis,
    current_grip_pos: i32,
    max_slider_pos: i32,
    grip_size: Size,
    value_range: RangeInclusive<i32>,
    current_value: i32,
    step: i32,
    slider_grab_pos: Option<(Point, i32)>,
    value_changed: Option<Callback<i32>>,
}

mod names {
    pub const SCROLL_LEFT: &str = "scroll left";
    pub const SCROLL_RIGHT: &str = "scroll right";
    pub const SCROLL_UP: &str = "scroll up";
    pub const SCROLL_DOWN: &str = "scroll down";
    pub const SCROLL_GRIP: &str = "scroll grip";
}

const INDEX_DECREASE: usize = 0;
const INDEX_PAGER: usize = 1;
const INDEX_INCREASE: usize = 2;
const INDEX_GRIP_IN_PAGER: usize = 0;

#[impl_with]
impl ScrollBar {
    pub fn new() -> Self {
        let mut common = WidgetCommon::new();
        let border_collapse = common.style().scroll_bar.border_collapse.get();
        // TODO: update when style changes
        common.set_grid_options(Some(GridOptions {
            x: GridAxisOptions {
                min_padding: 0,
                min_spacing: 0,
                preferred_padding: 0,
                preferred_spacing: 0,
                border_collapse,
            },
            y: GridAxisOptions {
                min_padding: 0,
                min_spacing: 0,
                preferred_padding: 0,
                preferred_spacing: 0,
                border_collapse,
            },
        }));
        // TODO: localized name
        common.add_child(
            Button::new(names::SCROLL_LEFT)
                .with_role(button::Role1::ScrollLeft)
                .with_text_visible(false)
                .with_auto_repeat(true)
                .with_trigger_on_press(true)
                .boxed(),
            LayoutItemOptions::from_pos_in_grid(0, 0),
        );

        let axis = Axis::X;
        let mut pager = Pager::new(axis);
        pager.common.add_child(
            Button::new(names::SCROLL_GRIP)
                .with_role(button::Role1::ScrollGripX)
                .with_text_visible(false)
                .with_auto_repeat(true)
                .boxed(),
            LayoutItemOptions::default(),
        );
        common.add_child(pager.boxed(), LayoutItemOptions::from_pos_in_grid(1, 0));
        common.add_child(
            Button::new(names::SCROLL_RIGHT)
                .with_role(button::Role1::ScrollRight)
                .with_text_visible(false)
                .with_auto_repeat(true)
                .with_trigger_on_press(true)
                .boxed(),
            LayoutItemOptions::from_pos_in_grid(2, 0),
        );
        let mut this = Self {
            common,
            axis,
            current_grip_pos: 0,
            max_slider_pos: 0,
            grip_size: Size::default(),
            value_range: 0..=100,
            current_value: 0,
            step: 5,
            slider_grab_pos: None,
            value_changed: None,
        };

        let slider_pressed = this.callback(Self::slider_pressed);
        let slider_moved = this.callback(Self::slider_moved);
        this.common.children[INDEX_PAGER]
            .widget
            .common_mut()
            .children[INDEX_GRIP_IN_PAGER]
            .widget
            .common_mut()
            .event_filter = Some(Box::new(move |event| {
            match event {
                Event::MouseInput(e) => {
                    if e.button() == MouseButton::Left {
                        slider_pressed.invoke((e.pos_in_window(), e.state()));
                    }
                }
                Event::MouseMove(e) => slider_moved.invoke(e.pos_in_window()),
                _ => {}
            }
            Ok(false)
        }));

        let decrease_callback = this.callback(|this, _| {
            this.set_value(max(*this.value_range().start(), this.value() - this.step));
            Ok(())
        });
        this.common.children[INDEX_DECREASE]
            .widget
            .downcast_mut::<Button>()
            .unwrap()
            .on_triggered(decrease_callback);

        let increase_callback = this.callback(|this, _| {
            this.set_value(min(*this.value_range().end(), this.value() + this.step));
            Ok(())
        });
        this.common.children[INDEX_INCREASE]
            .widget
            .downcast_mut::<Button>()
            .unwrap()
            .on_triggered(increase_callback);

        this.update_decrease_increase();
        this
    }

    pub fn set_axis(&mut self, axis: Axis) {
        self.axis = axis;
        match axis {
            Axis::X => {
                let decrease = self.common.children[INDEX_DECREASE]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap();
                decrease.set_text(names::SCROLL_LEFT);
                decrease.set_role(button::Role1::ScrollLeft);

                let increase = self.common.children[INDEX_INCREASE]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap();
                increase.set_text(names::SCROLL_RIGHT);
                increase.set_role(button::Role1::ScrollRight);

                self.common.children[INDEX_PAGER]
                    .widget
                    .common_mut()
                    .children[INDEX_GRIP_IN_PAGER]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap()
                    .set_role(button::Role1::ScrollGripX);

                self.common
                    .set_child_options(INDEX_PAGER, LayoutItemOptions::from_pos_in_grid(1, 0))
                    .unwrap();
                self.common
                    .set_child_options(INDEX_INCREASE, LayoutItemOptions::from_pos_in_grid(2, 0))
                    .unwrap();
            }
            Axis::Y => {
                let decrease = self.common.children[INDEX_DECREASE]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap();
                decrease.set_text(names::SCROLL_UP);
                decrease.set_role(button::Role1::ScrollUp);

                let increase = self.common.children[INDEX_INCREASE]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap();
                increase.set_text(names::SCROLL_DOWN);
                increase.set_role(button::Role1::ScrollDown);

                self.common.children[INDEX_PAGER]
                    .widget
                    .common_mut()
                    .children[INDEX_GRIP_IN_PAGER]
                    .widget
                    .downcast_mut::<Button>()
                    .unwrap()
                    .set_role(button::Role1::ScrollGripY);

                self.common
                    .set_child_options(INDEX_PAGER, LayoutItemOptions::from_pos_in_grid(0, 1))
                    .unwrap();
                self.common
                    .set_child_options(INDEX_INCREASE, LayoutItemOptions::from_pos_in_grid(0, 2))
                    .unwrap();
            }
        }
        self.common.children[INDEX_PAGER]
            .widget
            .downcast_mut::<Pager>()
            .unwrap()
            .set_axis(axis);
    }

    pub fn on_value_changed(&mut self, callback: Callback<i32>) {
        self.value_changed = Some(callback);
    }

    fn slider_pressed(&mut self, (pos_in_window, state): (Point, ElementState)) -> Result<()> {
        match state {
            ElementState::Pressed => {
                self.slider_grab_pos = Some((pos_in_window, self.current_grip_pos));
            }
            ElementState::Released => {
                self.slider_grab_pos = None;
            }
        }
        Ok(())
    }

    fn slider_moved(&mut self, pos_in_window: Point) -> Result<()> {
        if let Some((start_mouse_pos, start_slider_pos)) = &self.slider_grab_pos {
            match self.axis {
                Axis::X => {
                    let new_pos = start_slider_pos - start_mouse_pos.x + pos_in_window.x;
                    self.current_grip_pos = new_pos.clamp(0, self.max_slider_pos);
                    let new_value = if self.max_slider_pos == 0 {
                        *self.value_range.start()
                    } else {
                        ((self.current_grip_pos as f32) / (self.max_slider_pos as f32)
                            * (*self.value_range.end() - *self.value_range.start()) as f32)
                            .round() as i32
                            + self.value_range.start()
                    };
                    self.set_value(new_value);
                }
                Axis::Y => {
                    // TODO: deduplicate
                    let new_pos = start_slider_pos - start_mouse_pos.y + pos_in_window.y;
                    self.current_grip_pos = new_pos.clamp(0, self.max_slider_pos);
                    let new_value = if self.max_slider_pos == 0 {
                        *self.value_range.start()
                    } else {
                        ((self.current_grip_pos as f32) / (self.max_slider_pos as f32)
                            * (*self.value_range.end() - *self.value_range.start()) as f32)
                            .round() as i32
                            + *self.value_range.start()
                    };
                    self.set_value(new_value);
                }
            }
        }
        Ok(())
    }

    pub fn set_value_range(&mut self, mut range: RangeInclusive<i32>) {
        if range.end() < range.start() {
            warn!("invalid scroll bar range");
            range = *range.start()..=*range.start();
        }
        if self.value_range == range {
            return;
        }
        self.value_range = range;
        if self.current_value < *self.value_range.start()
            || self.current_value > *self.value_range.end()
        {
            self.set_value(
                self.current_value
                    .clamp(*self.value_range.start(), *self.value_range.end()),
            );
        } else {
            self.update_grip_pos();
        }
        self.update_decrease_increase();
    }

    pub fn value_range(&self) -> RangeInclusive<i32> {
        self.value_range.clone()
    }

    pub fn set_step(&mut self, step: i32) {
        self.step = step;
    }

    pub fn set_value(&mut self, mut value: i32) {
        if value < *self.value_range.start() || value > *self.value_range.end() {
            warn!("scroll bar value out of bounds");
            value = value.clamp(*self.value_range.start(), *self.value_range.end());
        }
        if self.current_value == value {
            return;
        }
        self.current_value = value;
        if let Some(value_changed) = &self.value_changed {
            value_changed.invoke(self.current_value);
        }
        self.update_grip_pos();
        self.update_decrease_increase();
    }

    fn update_decrease_increase(&mut self) {
        let decrease = self.common.children[INDEX_DECREASE]
            .widget
            .downcast_mut::<Button>()
            .unwrap();
        decrease.set_enabled(self.current_value > *self.value_range.start());

        let increase = self.common.children[INDEX_INCREASE]
            .widget
            .downcast_mut::<Button>()
            .unwrap();
        increase.set_enabled(self.current_value < *self.value_range.end());
    }

    fn update_grip_pos(&mut self) {
        self.current_grip_pos = self.value_to_slider_pos();
        let shift = match self.axis {
            Axis::X => Point::new(self.current_grip_pos, 0),
            Axis::Y => Point::new(0, self.current_grip_pos),
        };
        let rect = if self.value_range.start() == self.value_range.end() {
            None
        } else {
            Some(Rect::from_pos_size(shift, self.grip_size))
        };
        self.common.children[INDEX_PAGER]
            .widget
            .common_mut()
            .set_child_rect(INDEX_GRIP_IN_PAGER, rect)
            .unwrap();
    }

    pub fn value(&self) -> i32 {
        self.current_value
    }

    fn value_to_slider_pos(&self) -> i32 {
        if *self.value_range.start() > *self.value_range.end() {
            warn!("invalid scroll bar range");
            return 0;
        }
        if *self.value_range.start() == *self.value_range.end() {
            return 0;
        }
        let pos = (self.current_value - *self.value_range.start()) as f32
            / (*self.value_range.end() - *self.value_range.start()) as f32
            * self.max_slider_pos as f32;
        pos.round() as i32
    }
}

impl Default for ScrollBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ScrollBar {
    fn common(&self) -> &super::WidgetCommon {
        &self.common
    }

    fn common_mut(&mut self) -> &mut super::WidgetCommon {
        &mut self.common
    }

    fn handle_draw(&mut self, event: crate::draw::DrawEvent) -> Result<()> {
        let size = self.common.size_or_err()?;
        let style = &self.common.style().scroll_bar;
        event.stroke_and_fill_rounded_rect(
            Rect {
                top_left: Point::default(),
                size,
            },
            &style.border,
            style.background.as_ref(),
        );
        Ok(())
    }

    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        let options = self.common.grid_options();
        let size = self.common.size_or_err()?;
        let rects = grid::layout(&mut self.common.children, &options, size)?;
        self.common.set_child_rects(&rects)?;
        let pager_rect = rects.get(&INDEX_PAGER).unwrap();
        let grip_size_hint_x = self.common.children[INDEX_PAGER]
            .widget
            .common_mut()
            .children[INDEX_GRIP_IN_PAGER]
            .widget
            .size_hint_x(SizeHintMode::Preferred);
        let grip_size_hint_y = self.common.children[INDEX_PAGER]
            .widget
            .common_mut()
            .children[INDEX_GRIP_IN_PAGER]
            .widget
            .size_hint_y(grip_size_hint_x, SizeHintMode::Preferred);

        let (size_along_axis, grip_min_size_along_axis, pager_size_along_axis) = match self.axis {
            Axis::X => (size.x, grip_size_hint_x, pager_rect.size.x),
            Axis::Y => (size.y, grip_size_hint_y, pager_rect.size.y),
        };
        let size_plus_range = self.value_range.end() - self.value_range.start() + size_along_axis;
        let visible_ratio = if size_plus_range == 0 {
            0.0
        } else {
            (size_along_axis as f32) / (size_plus_range as f32)
        };
        let grip_len = max(
            grip_min_size_along_axis,
            (pager_size_along_axis as f32 * visible_ratio).round() as i32,
        );

        match self.axis {
            Axis::X => {
                self.grip_size = Size::new(grip_len, pager_rect.size.y);
                self.max_slider_pos = pager_rect.size.x - self.grip_size.x;
            }
            Axis::Y => {
                self.grip_size = Size::new(pager_rect.size.x, grip_len);
                self.max_slider_pos = pager_rect.size.y - self.grip_size.y;
            }
        }
        self.update_grip_pos();
        Ok(())
    }
}

struct Pager {
    common: WidgetCommon,
    axis: Axis,
}

impl Pager {
    pub fn new(axis: Axis) -> Self {
        Self {
            common: WidgetCommon::new(),
            axis,
        }
    }

    pub fn set_axis(&mut self, axis: Axis) {
        self.axis = axis;
        self.common.size_hint_changed();
    }
}

const PAGER_SIZE_HINT_MULTIPLIER: i32 = 2;

impl Widget for Pager {
    impl_widget_common!();

    fn recalculate_size_hint_x(&mut self, mode: SizeHintMode) -> Result<i32> {
        let grip_hint = self.common.children[INDEX_GRIP_IN_PAGER]
            .widget
            .size_hint_x(mode);
        match self.axis {
            Axis::X => Ok(grip_hint * PAGER_SIZE_HINT_MULTIPLIER),
            Axis::Y => Ok(grip_hint),
        }
    }
    fn recalculate_size_hint_y(&mut self, size_x: i32, mode: SizeHintMode) -> Result<i32> {
        let grip_hint = self.common.children[INDEX_GRIP_IN_PAGER]
            .widget
            .size_hint_y(size_x, mode);
        match self.axis {
            Axis::X => Ok(grip_hint),
            Axis::Y => Ok(grip_hint * PAGER_SIZE_HINT_MULTIPLIER),
        }
    }
    fn recalculate_size_x_fixed(&mut self) -> bool {
        self.axis == Axis::Y
    }
    fn recalculate_size_y_fixed(&mut self) -> bool {
        self.axis == Axis::X
    }
}
