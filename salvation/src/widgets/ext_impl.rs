use {
    super::{common::WidgetGeometry, Widget, WidgetAddress, WidgetExt, WidgetId},
    crate::{
        callback::{widget_callback, Callback},
        event::{EnabledChangeEvent, Event, LayoutEvent, StyleChangeEvent},
        layout::{SizeHints, FALLBACK_SIZE_HINTS},
        style::{computed::ComputedStyle, css::MyPseudoClass, Style},
        system::{with_system, ReportError},
    },
    anyhow::Result,
    log::{error, warn},
    std::{marker::PhantomData, rc::Rc},
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
        let id = widget.common().id;
        window.accept_current_mouse_event(id).or_report_err();

        window.set_cursor(widget.common().cursor_icon);
        if is_enter {
            window.add_mouse_entered(rect_in_window, id);
            widget.common_mut().is_mouse_over = true;
            widget.common_mut().mouse_over_changed();
        }
    }
}

impl<W: Widget + ?Sized> WidgetExt for W {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized,
    {
        WidgetId(self.common().id, PhantomData)
    }

    // TODO: use classes instead?
    fn set_no_padding(&mut self, no_padding: bool) -> &mut Self {
        self.common_mut().set_no_padding(no_padding);
        self
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

    fn add_pseudo_class(&mut self, class: MyPseudoClass) -> &mut Self {
        self.common_mut().add_pseudo_class(class);
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
        let mut should_dispatch = true;
        match &event {
            Event::FocusIn(_) => {
                self.common_mut().is_focused = true;
                self.common_mut().focused_changed();
            }
            Event::FocusOut(_) => {
                self.common_mut().is_focused = false;
                self.common_mut().focused_changed();
            }
            Event::WindowFocusChange(e) => {
                self.common_mut().is_window_focused = e.is_window_focused;
                self.common_mut().focused_changed();
            }
            Event::MouseInput(event) => {
                should_dispatch = self.common().is_enabled();
                if should_dispatch {
                    for child in self.common_mut().children.values_mut().rev() {
                        if let Some(rect_in_parent) = child.common().rect_in_parent() {
                            if let Some(child_event) = event.map_to_child(
                                rect_in_parent,
                                child.common().receives_all_mouse_events,
                            ) {
                                if child.dispatch(child_event.into()) {
                                    accepted = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Event::MouseScroll(event) => {
                should_dispatch = self.common().is_enabled();
                if should_dispatch {
                    for child in self.common_mut().children.values_mut().rev() {
                        if let Some(rect_in_parent) = child.common().rect_in_parent() {
                            if let Some(child_event) = event.map_to_child(
                                rect_in_parent,
                                child.common().receives_all_mouse_events,
                            ) {
                                if child.dispatch(child_event.into()) {
                                    accepted = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Event::MouseEnter(_) | Event::KeyboardInput(_) | Event::InputMethod(_) => {
                should_dispatch = self.common().is_enabled();
            }
            Event::MouseMove(event) => {
                should_dispatch = self.common().is_enabled();
                if should_dispatch {
                    for child in self.common_mut().children.values_mut().rev() {
                        if let Some(rect_in_parent) = child.common().rect_in_parent() {
                            if let Some(child_event) = event.map_to_child(
                                rect_in_parent,
                                child.common().receives_all_mouse_events,
                            ) {
                                if child.dispatch(child_event.into()) {
                                    accepted = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !accepted {
                        let is_enter =
                            if let Some(window) = self.common().window_or_err().or_report_err() {
                                !window.is_mouse_entered(self.common().id)
                            } else {
                                false
                            };

                        if is_enter {
                            self.dispatch(event.create_enter_event().into());
                        }
                    }
                }
            }
            Event::MouseLeave(_) => {
                self.common_mut().is_mouse_over = false;
                self.common_mut().mouse_over_changed();
                should_dispatch = self.common().is_enabled();
            }
            Event::Layout(_) => {}
            Event::StyleChange(_) => {
                self.common_mut().refresh_common_style();
            }
            Event::EnabledChange(_) => {
                self.common_mut().enabled_changed();
            }
            Event::Draw(_) | Event::AccessibilityAction(_) | Event::ScrollToRect(_) => {}
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
                            window.accept_current_mouse_event(common.id).or_report_err();
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
            Event::ScrollToRect(event) => {
                if !accepted && event.address != self.common().address {
                    if event.address.starts_with(&self.common().address) {
                        if let Some((key, id)) = event.address.item_at(self.common().address.len())
                        {
                            if let Some(child) = self.common_mut().children.get_mut(key) {
                                if &child.common().id == id {
                                    child.dispatch(event.clone().into());
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
            }
            Event::EnabledChange(event) => {
                for child in self.common_mut().children.values_mut() {
                    let old_enabled = child.common_mut().is_enabled();
                    child.common_mut().is_parent_enabled = event.is_enabled;
                    let new_enabled = child.common_mut().is_enabled();
                    if old_enabled != new_enabled {
                        child.dispatch(
                            EnabledChangeEvent {
                                is_enabled: new_enabled,
                            }
                            .into(),
                        );
                        // TODO: do it when pseudo class changes instead
                        child.dispatch(StyleChangeEvent {}.into());
                    }
                }
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
                node.set_bounds(accesskit::Rect {
                    x0: rect.top_left.x as f64,
                    y0: rect.top_left.y as f64,
                    x1: rect.bottom_right().x as f64,
                    y1: rect.bottom_right().y as f64,
                });
            }
            node
        });
        window.accessible_update(self.common().id.0.into(), node);
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

    fn size_hint_y(&mut self, size_x: i32) -> SizeHints {
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

    fn set_style(&mut self, style: Option<Rc<Style>>) -> Result<()> {
        let scale = self.common().parent_style.0.scale;
        let style = if let Some(style) = style {
            Some(ComputedStyle::new(style, scale)?)
        } else {
            None
        };
        self.common_mut().self_style = style;
        self.dispatch(StyleChangeEvent {}.into());
        Ok(())
    }

    fn add_class(&mut self, class: &'static str) -> &mut Self {
        self.common_mut().style_element.add_class(class);
        self.dispatch(StyleChangeEvent {}.into());
        self
    }

    fn remove_class(&mut self, class: &'static str) {
        self.common_mut().style_element.remove_class(class);
        self.dispatch(StyleChangeEvent {}.into());
    }

    fn set_enabled(&mut self, enabled: bool) {
        let old_enabled = self.common().is_enabled();
        if self.common().is_self_enabled == enabled {
            return;
        }
        self.common_mut().is_self_enabled = enabled;
        let new_enabled = self.common().is_enabled();
        if old_enabled != new_enabled {
            self.dispatch(
                EnabledChangeEvent {
                    is_enabled: new_enabled,
                }
                .into(),
            );
            // TODO: do it when pseudo class changes instead
            self.dispatch(StyleChangeEvent {}.into());
        }
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

        /*
        let rect_in_window = if let Some(rect_in_window) = self.rect_in_window {
            rect_in_parent.map(|rect_in_parent| rect_in_parent.translate(rect_in_window.top_left))
        } else {
            None
        };
        let visible_rect = if let (Some(visible_rect), Some(rect_in_parent)) =
            (self.visible_rect, rect_in_parent)
        {
            Some(
                visible_rect
                    .translate(-rect_in_parent.top_left)
                    .intersect(Rect::from_pos_size(Point::default(), rect_in_parent.size)),
            )
            .filter(|r| r != &Rect::default())
        } else {
            None
        };
        child.common_mut().rect_in_parent = rect_in_parent;
        // println!(
        //     "rect_in_window {:?} -> {:?}",
        //     child.common().rect_in_window,
        //     rect_in_window
        // );
        // println!(
        //     "visible_rect {:?} -> {:?}",
        //     child.common().visible_rect,
        //     visible_rect
        // );*/
        // let rects_changed = self.common().geometry.as_ref() != Some(&geometry);
        // self.common_mut().geometry = Some(geometry);
        // if let Some(event) = &self.current_layout_event {
        //     if rects_changed || event.size_hints_changed_within(child.common().address()) {
        //         //println!("set_child_rect ok1");
        //         child.dispatch(
        //             LayoutEvent {
        //                 new_rect_in_window: rect_in_window,
        //                 new_visible_rect: visible_rect,
        //                 changed_size_hints: event.changed_size_hints.clone(),
        //             }
        //             .into(),
        //         );
        //     }
        // } else {
        //     if rects_changed {
        //         //println!("set_child_rect ok2");
        //         child.dispatch(
        //             LayoutEvent {
        //                 new_rect_in_window: rect_in_window,
        //                 new_visible_rect: visible_rect,
        //                 changed_size_hints: Vec::new(),
        //             }
        //             .into(),
        //         );
        //     }
        // }
        //println!("set_child_rect end");
        //Ok(())
    }

    fn boxed(self) -> Box<dyn Widget>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}
