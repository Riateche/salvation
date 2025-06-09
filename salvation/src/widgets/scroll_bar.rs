use {
    super::{button::Button, Widget, WidgetAddress, WidgetCommonTyped, WidgetExt, WidgetGeometry},
    crate::{
        callback::{Callback, Callbacks},
        event::{
            Event, FocusInEvent, FocusOutEvent, KeyboardInputEvent, LayoutEvent, MouseScrollEvent,
        },
        impl_widget_common,
        layout::{grid::grid_layout, SizeHints},
        system::ReportError,
        types::{Axis, PhysicalPixels, Point, PpxSuffix, Rect, Size},
    },
    anyhow::Result,
    log::warn,
    ordered_float::NotNan,
    salvation_macros::impl_with,
    std::{
        cmp::{max, min},
        ops::RangeInclusive,
    },
    winit::{
        event::{ElementState, MouseButton},
        keyboard::{Key, NamedKey},
    },
};

pub struct ScrollBar {
    common: WidgetCommonTyped<Self>,
    axis: Axis,
    current_grip_pos: PhysicalPixels,
    max_slider_pos: PhysicalPixels,
    grip_size: Size,
    value_range: RangeInclusive<i32>,
    current_value: i32,
    step: i32,
    slider_grab_pos: Option<(Point, PhysicalPixels)>,
    value_changed: Callbacks<i32>,
    pager_direction: i32,
    pager_mouse_pos_in_window: Point,
}

mod names {
    pub const SCROLL_LEFT: &str = "scroll left";
    pub const SCROLL_RIGHT: &str = "scroll right";
    pub const SCROLL_UP: &str = "scroll up";
    pub const SCROLL_DOWN: &str = "scroll down";
    pub const SCROLL_GRIP: &str = "scroll grip";
    pub const SCROLL_PAGER: &str = "scroll pager";
}

const INDEX_DECREASE: u64 = 0;
const INDEX_PAGER: u64 = 1;
const INDEX_INCREASE: u64 = 2;

const INDEX_BUTTON_IN_PAGER: u64 = 0;
const INDEX_GRIP_IN_PAGER: u64 = 1;

// TODO: support other value types
#[impl_with]
impl ScrollBar {
    pub fn increase(&mut self) {
        self.increase_internal(true)
    }

    fn increase_internal(&mut self, from_setter: bool) {
        self.set_value_internal(
            min(*self.value_range().end(), self.value() + self.step),
            from_setter,
        );
    }

    pub fn decrease(&mut self) {
        self.decrease_internal(true)
    }

    fn decrease_internal(&mut self, from_setter: bool) {
        self.set_value_internal(
            min(*self.value_range().end(), self.value() - self.step),
            from_setter,
        );
    }

    pub fn axis(&mut self) -> Axis {
        self.axis
    }

    pub fn set_axis(&mut self, axis: Axis) -> &mut Self {
        if self.axis == axis {
            return self;
        }
        self.axis = axis;
        match axis {
            Axis::X => {
                let decrease = self.common.get_child_mut::<Button>(INDEX_DECREASE).unwrap();
                decrease.set_text(names::SCROLL_LEFT);
                decrease.add_class("scroll_left".into());
                decrease.remove_class("scroll_up".into());

                let increase = self.common.get_child_mut::<Button>(INDEX_INCREASE).unwrap();
                increase.set_text(names::SCROLL_RIGHT);
                increase.add_class("scroll_right".into());
                increase.remove_class("scroll_down".into());

                let grip = self
                    .common
                    .get_dyn_child_mut(INDEX_PAGER)
                    .unwrap()
                    .common_mut()
                    .get_child_mut::<Button>(INDEX_GRIP_IN_PAGER)
                    .unwrap();
                grip.add_class("scroll_grip_x".into());
                grip.remove_class("scroll_grip_y".into());

                self.common
                    .get_dyn_child_mut(INDEX_PAGER)
                    .unwrap()
                    .set_column(1)
                    .set_row(0);
                self.common
                    .get_dyn_child_mut(INDEX_INCREASE)
                    .unwrap()
                    .set_column(2)
                    .set_row(0);
            }
            Axis::Y => {
                let decrease = self.common.get_child_mut::<Button>(INDEX_DECREASE).unwrap();
                decrease.set_text(names::SCROLL_UP);
                decrease.remove_class("scroll_left".into());
                decrease.add_class("scroll_up".into());

                let increase = self.common.get_child_mut::<Button>(INDEX_INCREASE).unwrap();
                increase.set_text(names::SCROLL_DOWN);
                increase.remove_class("scroll_right".into());
                increase.add_class("scroll_down".into());

                let grip = self
                    .common
                    .get_dyn_child_mut(INDEX_PAGER)
                    .unwrap()
                    .common_mut()
                    .get_child_mut::<Button>(INDEX_GRIP_IN_PAGER)
                    .unwrap();

                grip.remove_class("scroll_grip_x".into());
                grip.add_class("scroll_grip_y".into());

                self.common
                    .get_dyn_child_mut(INDEX_PAGER)
                    .unwrap()
                    .set_column(0)
                    .set_row(1);
                self.common
                    .get_dyn_child_mut(INDEX_INCREASE)
                    .unwrap()
                    .set_column(0)
                    .set_row(2);
            }
        }
        self.common
            .get_child_mut::<Pager>(INDEX_PAGER)
            .unwrap()
            .set_axis(axis);
        self
    }

    pub fn on_value_changed(&mut self, callback: Callback<i32>) -> &mut Self {
        self.value_changed.add(callback);
        self
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
        if let Some((start_mouse_pos, start_slider_pos)) = self.slider_grab_pos {
            match self.axis {
                Axis::X => {
                    let new_pos = start_slider_pos - start_mouse_pos.x() + pos_in_window.x();
                    self.current_grip_pos = new_pos.clamp(0.ppx(), self.max_slider_pos);
                    let new_value = if self.max_slider_pos == 0.ppx() {
                        *self.value_range.start()
                    } else {
                        ((self.current_grip_pos.to_i32() as f32)
                            / (self.max_slider_pos.to_i32() as f32)
                            * (*self.value_range.end() - *self.value_range.start()) as f32)
                            .round() as i32
                            + self.value_range.start()
                    };
                    self.set_value_internal(new_value, false);
                }
                Axis::Y => {
                    // TODO: deduplicate
                    let new_pos = start_slider_pos - start_mouse_pos.y() + pos_in_window.y();
                    self.current_grip_pos = new_pos.clamp(0.ppx(), self.max_slider_pos);
                    let new_value = if self.max_slider_pos == 0.ppx() {
                        *self.value_range.start()
                    } else {
                        ((self.current_grip_pos.to_i32() as f32)
                            / (self.max_slider_pos.to_i32() as f32)
                            * (*self.value_range.end() - *self.value_range.start()) as f32)
                            .round() as i32
                            + *self.value_range.start()
                    };
                    self.set_value_internal(new_value, false);
                }
            }
        }
        Ok(())
    }

    fn pager_pressed(&mut self, pos_in_window: Point) -> Result<()> {
        let Some(grip_rect_in_window) = self
            .common
            .get_dyn_child(INDEX_PAGER)
            .unwrap()
            .common()
            .get_dyn_child(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .common()
            .rect_in_window()
        else {
            return Ok(());
        };
        self.pager_direction = match self.axis {
            Axis::X => {
                if grip_rect_in_window.right() < pos_in_window.x() {
                    1
                } else {
                    -1
                }
            }
            Axis::Y => {
                if grip_rect_in_window.bottom() < pos_in_window.y() {
                    1
                } else {
                    -1
                }
            }
        };

        Ok(())
    }

    fn pager_mouse_move(&mut self, pos_in_window: Point) -> Result<()> {
        self.pager_mouse_pos_in_window = pos_in_window;
        Ok(())
    }

    fn pager_triggered(&mut self) -> Result<()> {
        let Some(grip_rect_in_window) = self
            .common
            .get_dyn_child(INDEX_PAGER)
            .unwrap()
            .common()
            .get_dyn_child(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .common()
            .rect_in_window()
        else {
            return Ok(());
        };

        if self.pager_direction > 0 {
            let condition = match self.axis {
                Axis::X => grip_rect_in_window.right() < self.pager_mouse_pos_in_window.x(),
                Axis::Y => grip_rect_in_window.bottom() < self.pager_mouse_pos_in_window.y(),
            };
            if condition {
                self.page_forward_internal(false);
            }
        } else {
            let condition = match self.axis {
                Axis::X => grip_rect_in_window.left() > self.pager_mouse_pos_in_window.x(),
                Axis::Y => grip_rect_in_window.top() > self.pager_mouse_pos_in_window.y(),
            };
            if condition {
                self.page_back_internal(false);
            }
        }
        Ok(())
    }

    fn page_step(&self) -> i32 {
        let Some(size) = self.common.size() else {
            return 1;
        };
        match self.axis {
            Axis::X => size.x().to_i32(),
            Axis::Y => size.y().to_i32(),
        }
    }

    pub fn page_forward(&mut self) {
        self.page_forward_internal(true);
    }

    pub fn page_forward_internal(&mut self, from_setter: bool) {
        self.set_value_internal(
            min(
                *self.value_range.end(),
                self.current_value + self.page_step(),
            ),
            from_setter,
        );
    }

    pub fn page_back(&mut self) {
        self.page_back_internal(true);
    }

    pub fn page_back_internal(&mut self, from_setter: bool) {
        self.set_value_internal(
            max(
                *self.value_range.start(),
                self.current_value - self.page_step(),
            ),
            from_setter,
        );
    }

    pub fn set_value_range(&mut self, mut range: RangeInclusive<i32>) -> &mut Self {
        if range.end() < range.start() {
            warn!("invalid scroll bar range");
            range = *range.start()..=*range.start();
        }
        if self.value_range == range {
            return self;
        }
        self.value_range = range;
        self.update_grip_size(&[]).or_report_err();
        if self.current_value < *self.value_range.start()
            || self.current_value > *self.value_range.end()
        {
            self.set_value_internal(
                self.current_value
                    .clamp(*self.value_range.start(), *self.value_range.end()),
                true,
            );
        } else {
            self.update_grip_pos(&[]);
        }
        self.update_decrease_increase();
        self
    }

    pub fn value_range(&self) -> RangeInclusive<i32> {
        self.value_range.clone()
    }

    pub fn set_step(&mut self, step: i32) {
        self.step = step;
    }

    pub fn set_value(&mut self, value: i32) -> &mut Self {
        self.set_value_internal(value, true)
    }

    fn set_value_internal(&mut self, mut value: i32, from_setter: bool) -> &mut Self {
        if value < *self.value_range.start() || value > *self.value_range.end() {
            warn!("scroll bar value out of bounds");
            value = value.clamp(*self.value_range.start(), *self.value_range.end());
        }
        if self.current_value == value {
            return self;
        }
        self.current_value = value;
        self.value_changed.invoke(self.current_value, from_setter);
        self.update_grip_pos(&[]);
        self.update_decrease_increase();
        self
    }

    fn update_decrease_increase(&mut self) {
        let decrease = self.common.get_child_mut::<Button>(INDEX_DECREASE).unwrap();
        decrease.set_enabled(self.current_value > *self.value_range.start());

        let increase = self.common.get_child_mut::<Button>(INDEX_INCREASE).unwrap();
        increase.set_enabled(self.current_value < *self.value_range.end());
    }

    fn update_grip_pos(&mut self, changed_size_hints: &[WidgetAddress]) {
        self.current_grip_pos = self.value_to_slider_pos();
        let shift = match self.axis {
            Axis::X => Point::new(self.current_grip_pos, 0.ppx()),
            Axis::Y => Point::new(0.ppx(), self.current_grip_pos),
        };
        let can_scroll = self.value_range.start() != self.value_range.end();
        let rect = if can_scroll {
            Some(Rect::from_pos_size(shift, self.grip_size))
        } else {
            None
        };

        let Some(pager_geometry) = self
            .common
            .get_dyn_child(INDEX_PAGER)
            .unwrap()
            .common()
            .geometry
            .clone()
        else {
            return;
        };
        self.common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .set_geometry(
                rect.map(|rect| WidgetGeometry::new(&pager_geometry, rect)),
                changed_size_hints,
            );

        let pager_button = self
            .common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_child_mut::<Button>(INDEX_BUTTON_IN_PAGER)
            .unwrap();
        pager_button.set_enabled(can_scroll);
    }

    pub fn value(&self) -> i32 {
        self.current_value
    }

    fn value_to_slider_pos(&self) -> PhysicalPixels {
        if *self.value_range.start() > *self.value_range.end() {
            warn!("invalid scroll bar range");
            return 0.ppx();
        }
        if *self.value_range.start() == *self.value_range.end() {
            return 0.ppx();
        }
        let pos = (self.current_value - *self.value_range.start()) as f32
            / (*self.value_range.end() - *self.value_range.start()) as f32
            * self.max_slider_pos.to_i32() as f32;
        PhysicalPixels::from_i32(pos.round() as i32)
    }

    fn update_grip_size(&mut self, changed_size_hints: &[WidgetAddress]) -> Result<()> {
        let Some(size) = self.common.size() else {
            return Ok(());
        };
        grid_layout(self, changed_size_hints);
        let pager_rect = self
            .common
            .get_dyn_child(INDEX_PAGER)
            .unwrap()
            .common()
            .rect_in_parent()
            .unwrap();
        let grip_size_hint_x = self
            .common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .size_hint_x()
            .preferred;
        let grip_size_hint_y = self
            .common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .size_hint_y(grip_size_hint_x)
            .preferred;

        let (size_along_axis, grip_min_size_along_axis, pager_size_along_axis) = match self.axis {
            Axis::X => (size.x(), grip_size_hint_x, pager_rect.size_x()),
            Axis::Y => (size.y(), grip_size_hint_y, pager_rect.size_y()),
        };
        // For a scroll bar that's a part of a scroll area, we assume the following:
        // min value is 0; max value is the max displacement of the content in pixels;
        // size_along_axis == viewport_size;
        // max_value == content_size - viewport_size;
        // visible_ratio = viewport_size / content_size;
        // visible_ratio = viewport_size / (max_value + viewport_size);
        // we may consider adding a page_step property instead of always using size_along_axis.
        let size_plus_range =
            PhysicalPixels::from_i32(self.value_range.end() - self.value_range.start())
                + size_along_axis;
        let visible_ratio = if size_plus_range == 0.ppx() {
            0.0
        } else {
            (size_along_axis.to_i32() as f32) / (size_plus_range.to_i32() as f32)
        };
        let grip_len = max(
            grip_min_size_along_axis,
            pager_size_along_axis.mul_f32_round(visible_ratio),
        );

        match self.axis {
            Axis::X => {
                self.grip_size = Size::new(grip_len, pager_rect.size_y());
                self.max_slider_pos = pager_rect.size_x() - self.grip_size.x();
            }
            Axis::Y => {
                self.grip_size = Size::new(pager_rect.size_x(), grip_len);
                self.max_slider_pos = pager_rect.size_y() - self.grip_size.y();
            }
        }
        Ok(())
    }
}

impl Widget for ScrollBar {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        common.set_supports_focus(true).set_focusable(false);
        // TODO: localized name

        common
            .add_child_with_key::<Button>(INDEX_DECREASE)
            .set_column(0)
            .set_row(0)
            .set_text(names::SCROLL_LEFT)
            .set_accessible(false)
            .set_focusable(false)
            .add_class("scroll_left".into())
            .set_text_visible(false)
            .set_auto_repeat(true)
            .set_trigger_on_press(true);

        let axis = Axis::X;
        let pager = common
            .add_child_with_key::<Pager>(INDEX_PAGER)
            .set_column(1)
            .set_row(0)
            .set_axis(axis);
        pager
            .common
            .add_child_with_key::<Button>(INDEX_BUTTON_IN_PAGER)
            .set_column(0)
            .set_row(0)
            .set_size_x_fixed(false)
            .set_size_y_fixed(false)
            .set_accessible(false)
            .set_focusable(false)
            .add_class("scroll_pager".into())
            .set_text(names::SCROLL_PAGER)
            .set_text_visible(false)
            .set_auto_repeat(true)
            .set_trigger_on_press(true);
        pager
            .common
            .add_child_with_key::<Button>(INDEX_GRIP_IN_PAGER)
            .set_text(names::SCROLL_GRIP)
            .set_accessible(false)
            .set_focusable(false)
            .add_class("scroll_grip_x".into())
            .set_text_visible(false)
            .set_mouse_leave_sensitive(false);

        common
            .add_child_with_key::<Button>(INDEX_INCREASE)
            .set_column(2)
            .set_row(0)
            .set_text(names::SCROLL_RIGHT)
            .set_accessible(false)
            .set_focusable(false)
            .add_class("scroll_right".into())
            .set_text_visible(false)
            .set_auto_repeat(true)
            .set_trigger_on_press(true);

        let mut this = Self {
            common,
            axis,
            current_grip_pos: 0.ppx(),
            max_slider_pos: 0.ppx(),
            grip_size: Size::default(),
            value_range: 0..=100,
            current_value: 0,
            step: 5,
            slider_grab_pos: None,
            value_changed: Default::default(),
            pager_direction: 0,
            pager_mouse_pos_in_window: Point::default(),
        };

        let slider_pressed = this.callback(Self::slider_pressed);
        let slider_moved = this.callback(Self::slider_moved);
        this.common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .common_mut()
            .event_filter = Some(Box::new(move |event| {
            match event {
                Event::MouseInput(e) => {
                    if e.button == MouseButton::Left {
                        slider_pressed.invoke((e.pos_in_window, e.state));
                    }
                }
                Event::MouseMove(e) => slider_moved.invoke(e.pos_in_window),
                _ => {}
            }
            Ok(false)
        }));

        let decrease_callback = this.callback(|this, _| {
            this.decrease_internal(false);
            Ok(())
        });
        this.common
            .get_child_mut::<Button>(INDEX_DECREASE)
            .unwrap()
            .on_triggered(decrease_callback);

        let increase_callback = this.callback(|this, _| {
            this.increase_internal(false);
            Ok(())
        });
        this.common
            .get_child_mut::<Button>(INDEX_INCREASE)
            .unwrap()
            .on_triggered(increase_callback);

        let pager_triggered_callback = this.callback(|this, _| this.pager_triggered());
        let pager_pressed = this.callback(Self::pager_pressed);
        let pager_mouse_moved = this.callback(Self::pager_mouse_move);
        let pager_button = this
            .common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_child_mut::<Button>(INDEX_BUTTON_IN_PAGER)
            .unwrap();
        pager_button.on_triggered(pager_triggered_callback);
        pager_button.common_mut().event_filter = Some(Box::new(move |event| {
            match event {
                Event::MouseInput(e) => {
                    if e.button == MouseButton::Left && e.state == ElementState::Pressed {
                        pager_pressed.invoke(e.pos_in_window);
                    }
                }
                Event::MouseMove(e) => pager_mouse_moved.invoke(e.pos_in_window),
                _ => {}
            }
            Ok(false)
        }));

        this.update_decrease_increase();
        this
    }

    fn handle_layout(&mut self, event: LayoutEvent) -> Result<()> {
        self.update_grip_size(&event.changed_size_hints)?;
        self.update_grip_pos(&event.changed_size_hints);
        Ok(())
    }

    fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> Result<bool> {
        let delta = event.unified_delta(&self.common);
        let max_delta = if NotNan::new(delta.x.abs())? > NotNan::new(delta.y.abs())? {
            delta.x
        } else {
            delta.y
        };
        let new_value = self.value() - max_delta.round() as i32;
        self.set_value_internal(
            new_value.clamp(*self.value_range.start(), *self.value_range.end()),
            false,
        );
        Ok(true)
    }

    fn handle_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        if !event.info.state.is_pressed() {
            return Ok(false);
        }
        if let Key::Named(key) = event.info.logical_key {
            match key {
                NamedKey::ArrowDown => {
                    self.increase_internal(false);
                    Ok(true)
                }
                NamedKey::ArrowLeft => {
                    self.decrease_internal(false);
                    Ok(true)
                }
                NamedKey::ArrowRight => {
                    self.increase_internal(false);
                    Ok(true)
                }
                NamedKey::ArrowUp => {
                    self.decrease_internal(false);
                    Ok(true)
                }
                NamedKey::End => {
                    self.set_value_internal(*self.value_range().end(), false);
                    Ok(true)
                }
                NamedKey::Home => {
                    self.set_value_internal(*self.value_range().start(), false);
                    Ok(true)
                }
                NamedKey::PageDown => {
                    self.page_forward_internal(false);
                    Ok(true)
                }
                NamedKey::PageUp => {
                    self.page_back_internal(false);
                    Ok(true)
                }
                _ => Ok(false),
            }
        } else {
            Ok(false)
        }
    }

    fn handle_focus_in(&mut self, _event: FocusInEvent) -> Result<()> {
        self.common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .add_class("scroll_bar_focused".into());
        Ok(())
    }

    fn handle_focus_out(&mut self, _event: FocusOutEvent) -> Result<()> {
        self.common
            .get_dyn_child_mut(INDEX_PAGER)
            .unwrap()
            .common_mut()
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .remove_class("scroll_bar_focused".into());
        Ok(())
    }
}

struct Pager {
    common: WidgetCommonTyped<Self>,
    axis: Axis,
}

impl Pager {
    pub fn set_axis(&mut self, axis: Axis) -> &mut Self {
        self.axis = axis;
        self.common.size_hint_changed();
        self
    }
}

const PAGER_SIZE_HINT_MULTIPLIER: i32 = 2;

impl Widget for Pager {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        let mut t = Self {
            common,
            axis: Axis::X,
        };
        t.add_class("no_padding".into());
        t
    }

    fn handle_size_hint_x_request(&mut self) -> Result<SizeHints> {
        let grip = self.common.get_dyn_child_mut(INDEX_GRIP_IN_PAGER).unwrap();
        let grip_hint = grip.size_hint_x();
        let min_size = match self.axis {
            Axis::X => grip_hint.min * PAGER_SIZE_HINT_MULTIPLIER,
            Axis::Y => grip_hint.min,
        };
        let preferred_size = match self.axis {
            Axis::X => grip_hint.preferred * PAGER_SIZE_HINT_MULTIPLIER,
            Axis::Y => grip_hint.preferred,
        };
        Ok(SizeHints {
            min: min_size,
            preferred: preferred_size,
            is_fixed: self.axis == Axis::Y,
        })
    }
    fn handle_size_hint_y_request(&mut self, size_x: PhysicalPixels) -> Result<SizeHints> {
        let grip_hint = self
            .common
            .get_dyn_child_mut(INDEX_GRIP_IN_PAGER)
            .unwrap()
            .size_hint_y(size_x);
        let min_size = match self.axis {
            Axis::X => grip_hint.min,
            Axis::Y => grip_hint.min * PAGER_SIZE_HINT_MULTIPLIER,
        };
        let preferred_size = match self.axis {
            Axis::X => grip_hint.preferred,
            Axis::Y => grip_hint.preferred * PAGER_SIZE_HINT_MULTIPLIER,
        };
        Ok(SizeHints {
            min: min_size,
            preferred: preferred_size,
            is_fixed: self.axis == Axis::X,
        })
    }
}
