use {
    super::{common::WidgetGeometry, Widget, WidgetAddress, WidgetId},
    crate::{
        callback::{widget_callback, Callback},
        event::{Event, LayoutEvent, ScrollToRectRequest, StyleChangeEvent},
        layout::{SizeHints, FALLBACK_SIZE_HINTS},
        style::css::PseudoClass,
        system::{with_system, ReportError},
        types::PhysicalPixels,
    },
    anyhow::Result,
    log::{error, warn},
    std::borrow::Cow,
};

fn accept_mouse_move_or_enter_event(widget: &mut (impl Widget + ?Sized), is_enter: bool) {
    let Some(window) = widget.common_mut().window_or_err().or_report_err() else {
        return;
    };
    if window
        .current_mouse_event_state()
        .or_report_err()
        .is_some_and(|e| !e.is_accepted())
    {
        let Some(rect_in_window) = widget.common().rect_in_window_or_err().or_report_err() else {
            return;
        };
        let Some(window) = widget.common().window_or_err().or_report_err() else {
            return;
        };
        let id = widget.common().id();
        window.accept_current_mouse_event(id).or_report_err();

        window.set_cursor(widget.common().cursor_icon);
        if is_enter {
            window.add_mouse_entered(rect_in_window, id);
            widget.common_mut().is_mouse_over = true;
        }
    }
}

pub trait WidgetExt: Widget {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized,
    {
        WidgetId::new(self.common().id())
    }

    fn set_visible(&mut self, value: bool) -> &mut Self {
        if self.common_mut().is_self_visible == value {
            return self;
        }
        self.common_mut().is_self_visible = value;
        self.common_mut().size_hint_changed(); // trigger layout
        self
    }

    fn set_focusable(&mut self, value: bool) -> &mut Self {
        self.common_mut().set_focusable(value);
        self
    }
    fn set_accessible(&mut self, value: bool) -> &mut Self {
        self.common_mut().set_accessible(value);
        self
    }

    fn callback<F, E>(&self, func: F) -> Callback<E>
    where
        F: Fn(&mut Self, E) -> Result<()> + 'static,
        E: 'static,
        Self: Sized,
    {
        widget_callback(self.id(), func)
    }

    fn dispatch(&mut self, event: Event) -> bool {
        let mut accepted = false;
        let should_dispatch = if self.common().is_enabled() {
            true
        } else {
            match &event {
                Event::MouseInput(_)
                | Event::MouseScroll(_)
                | Event::MouseEnter(_)
                | Event::MouseMove(_)
                | Event::MouseLeave(_)
                | Event::KeyboardInput(_)
                | Event::InputMethod(_) => false,
                Event::Draw(_)
                | Event::Layout(_)
                | Event::FocusIn(_)
                | Event::FocusOut(_)
                | Event::WindowFocusChange(_)
                | Event::AccessibilityAction(_)
                | Event::StyleChange(_) => true,
            }
        };
        if should_dispatch {
            if self.common_mut().before_event(&event) {
                accepted = true;
            }
        }
        match &event {
            Event::MouseMove(event) => {
                if !accepted && should_dispatch {
                    let is_enter =
                        if let Some(window) = self.common().window_or_err().or_report_err() {
                            !window.is_mouse_entered(self.common().id())
                        } else {
                            false
                        };

                    if is_enter {
                        self.dispatch(event.create_enter_event().into());
                    }
                }
            }
            Event::MouseEnter(_) => {
                self.add_pseudo_class(PseudoClass::Hover);
            }
            Event::MouseLeave(_) => {
                self.remove_pseudo_class(PseudoClass::Hover);
            }
            Event::FocusIn(_) => {
                self.set_pseudo_class(PseudoClass::Focus, self.common().is_window_focused());
            }
            Event::FocusOut(_) => {
                self.remove_pseudo_class(PseudoClass::Focus);
            }
            Event::WindowFocusChange(event) => {
                self.set_pseudo_class(
                    PseudoClass::Focus,
                    event.is_window_focused && self.common().is_focused(),
                );
            }
            _ => (),
        }
        if !accepted && should_dispatch {
            if let Some(event_filter) = &mut self.common_mut().event_filter {
                accepted = event_filter(event.clone()).or_report_err().unwrap_or(false);
            }
            if !accepted {
                accepted = self
                    .handle_event(event.clone())
                    .or_report_err()
                    .unwrap_or(false);
            }
        }
        match event {
            Event::MouseInput(_) | Event::MouseScroll(_) => {
                if accepted {
                    let common = self.common_mut();
                    if let Some(window) = common.window_or_err().or_report_err() {
                        if window
                            .current_mouse_event_state()
                            .or_report_err()
                            .is_some_and(|e| !e.is_accepted())
                        {
                            window
                                .accept_current_mouse_event(common.id())
                                .or_report_err();
                        }
                    }
                }
            }
            Event::MouseEnter(_) => {
                // TODO: rename or rework to only accept if handler returned true
                accept_mouse_move_or_enter_event(self, true);
            }
            Event::MouseMove(_) => {
                accept_mouse_move_or_enter_event(self, false);
            }
            Event::Draw(event) => {
                for child in self.common_mut().children.values_mut() {
                    if let Some(rect_in_parent) = child.common().rect_in_parent() {
                        if let Some(child_event) = event.map_to_child(rect_in_parent) {
                            child.dispatch(child_event.into());
                        }
                    }
                }
            }
            Event::WindowFocusChange(event) => {
                for child in self.common_mut().children.values_mut() {
                    child.dispatch(event.clone().into());
                }
            }
            Event::FocusIn(_) | Event::FocusOut(_) | Event::MouseLeave(_) => {
                self.common_mut().update();
            }
            Event::Layout(_) => {
                self.common_mut().update();
            }
            Event::StyleChange(event) => {
                for child in self.common_mut().children.values_mut() {
                    // TODO: only if really changed
                    child.dispatch(event.clone().into());
                }
            }
            Event::KeyboardInput(_) | Event::InputMethod(_) | Event::AccessibilityAction(_) => {}
        }

        self.update_accessible();
        accepted
    }

    fn scroll_to_rect(&mut self, request: ScrollToRectRequest) -> bool {
        let accepted = self
            .handle_scroll_to_rect_request(request.clone())
            .or_report_err()
            .unwrap_or(false);
        if accepted {
            // TODO: propagate to inner widgets anyway? how does it work with two scroll areas?
            return true;
        }
        if request.address != self.common().address {
            if request.address.starts_with(&self.common().address) {
                if let Some((key, id)) = request.address.item_at(self.common().address.len()) {
                    if let Some(child) = self.common_mut().children.get_mut(key) {
                        if &child.common().id() == id {
                            child.scroll_to_rect(request.clone());
                        } else {
                            warn!("child id mismatch while dispatching ScrollToRectEvent");
                        }
                    } else {
                        warn!("invalid child index while dispatching ScrollToRectEvent");
                    }
                } else {
                    warn!("couldn't get child index while dispatching ScrollToRectEvent");
                }
            } else {
                warn!("ScrollToRectEvent dispatched to unrelated widget");
            }
        }

        false
    }

    fn update_accessible(&mut self) {
        let node = if self.common().is_accessible {
            self.handle_accessibility_node_request()
                .or_report_err()
                .flatten()
        } else {
            None
        };

        let Some(window) = self.common().window.as_ref() else {
            return;
        };
        // TODO: refresh after layout event
        let rect = self.common().rect_in_window();
        let node = node.map(|mut node| {
            if let Some(rect) = rect {
                node.set_bounds(rect.into());
            }
            node
        });
        window.accessible_update(self.common().id().into(), node);
    }

    fn update_children(&mut self) {
        if !self.common().has_declare_children_override {
            return;
        }
        let in_progress = with_system(|system| system.current_children_update.is_some());
        if in_progress {
            error!("attempted to call update_children while another update_children is running");
            return;
        }
        with_system(|system| {
            system.current_children_update = Some(Default::default());
        });
        self.handle_declare_children_request().or_report_err();
        let Some(state) = with_system(|system| system.current_children_update.take()) else {
            error!("missing widgets_created_in_current_children_update after handle_declare_children_request");
            return;
        };
        self.common_mut().after_declare_children(state);
    }

    fn size_hint_x(&mut self) -> SizeHints {
        if let Some(cached) = &self.common().size_hint_x_cache {
            *cached
        } else {
            let r = self
                .handle_size_hint_x_request()
                .or_report_err()
                .unwrap_or(FALLBACK_SIZE_HINTS);
            self.common_mut().size_hint_x_cache = Some(r);
            r
        }
    }

    fn size_hint_y(&mut self, size_x: PhysicalPixels) -> SizeHints {
        if let Some(cached) = self.common().size_hint_y_cache.get(&size_x) {
            *cached
        } else {
            let r = self
                .handle_size_hint_y_request(size_x)
                .or_report_err()
                .unwrap_or(FALLBACK_SIZE_HINTS);
            self.common_mut().size_hint_y_cache.insert(size_x, r);
            r
        }
    }

    fn add_class(&mut self, class: Cow<'static, str>) -> &mut Self {
        if self.common().style_element.has_class(&class) {
            return self;
        }
        self.common_mut().style_element.add_class(class);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn remove_class(&mut self, class: Cow<'static, str>) -> &mut Self {
        if !self.common().style_element.has_class(&class) {
            return self;
        }
        self.common_mut().style_element.remove_class(class);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn has_class(&self, class: &str) -> bool {
        self.common().style_element.has_class(class)
    }

    fn set_class(&mut self, class: Cow<'static, str>, present: bool) -> &mut Self {
        if self.common().style_element.has_class(&class) == present {
            return self;
        }
        self.common_mut().style_element.set_class(class, present);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn add_pseudo_class(&mut self, class: PseudoClass) -> &mut Self {
        if self.common().style_element.has_pseudo_class(class.clone()) {
            return self;
        }
        self.common_mut().style_element.add_pseudo_class(class);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn remove_pseudo_class(&mut self, class: PseudoClass) -> &mut Self {
        if !self.common().style_element.has_pseudo_class(class.clone()) {
            return self;
        }
        self.common_mut().style_element.remove_pseudo_class(class);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn has_pseudo_class(&self, class: PseudoClass) -> bool {
        self.common().style_element.has_pseudo_class(class)
    }

    fn set_pseudo_class(&mut self, class: PseudoClass, present: bool) -> &mut Self {
        if self.common().style_element.has_pseudo_class(class.clone()) == present {
            return self;
        }
        self.common_mut()
            .style_element
            .set_pseudo_class(class, present);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn set_enabled(&mut self, enabled: bool) -> &mut Self {
        let new_enabled = enabled && self.common().is_parent_enabled();
        self.set_pseudo_class(PseudoClass::Enabled, new_enabled);
        self.set_pseudo_class(PseudoClass::Disabled, !new_enabled);
        self.common_mut().self_enabled_changed(enabled);
        self
    }

    fn set_parent_enabled(&mut self, enabled: bool) -> &mut Self {
        let new_enabled = enabled && self.common().is_self_enabled();
        self.set_pseudo_class(PseudoClass::Enabled, new_enabled);
        self.set_pseudo_class(PseudoClass::Disabled, !new_enabled);
        self.common_mut().parent_enabled_changed(enabled);
        self
    }

    fn set_scale(&mut self, scale: Option<f32>) -> &mut Self {
        if self.common().self_scale == scale {
            return self;
        }
        self.common_mut().set_scale(scale);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    // TODO: check for row/column conflict
    fn set_row(&mut self, row: i32) -> &mut Self {
        let mut options = self.common().layout_item_options().clone();
        options.y.pos_in_grid = Some(row..=row);
        self.common_mut().set_layout_item_options(options);
        self
    }
    fn set_column(&mut self, column: i32) -> &mut Self {
        let mut options = self.common().layout_item_options().clone();
        options.x.pos_in_grid = Some(column..=column);
        self.common_mut().set_layout_item_options(options);
        self
    }

    fn set_size_x_fixed(&mut self, fixed: bool) -> &mut Self {
        let mut options = self.common().layout_item_options().clone();
        options.x.is_fixed = Some(fixed);
        self.common_mut().set_layout_item_options(options);
        self
    }
    fn set_size_y_fixed(&mut self, fixed: bool) -> &mut Self {
        let mut options = self.common().layout_item_options().clone();
        options.y.is_fixed = Some(fixed);
        self.common_mut().set_layout_item_options(options);
        self
    }

    fn set_geometry(
        &mut self,
        geometry: Option<WidgetGeometry>,
        changed_size_hints: &[WidgetAddress],
    ) {
        let geometry_changed = self.common().geometry != geometry;
        self.common_mut().geometry = geometry;
        if geometry_changed
            || changed_size_hints
                .iter()
                .any(|changed| changed.starts_with(self.common().address()))
        {
            self.dispatch(
                LayoutEvent {
                    new_geometry: None,
                    changed_size_hints: changed_size_hints.to_vec(),
                }
                .into(),
            );
        }
    }

    fn boxed(self) -> Box<dyn Widget>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

impl<W: Widget + ?Sized> WidgetExt for W {}
