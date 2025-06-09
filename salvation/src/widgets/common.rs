use {
    super::{address, RawWidgetId, Widget, WidgetAddress, WidgetId, WidgetNotFound},
    crate::{
        callback::{widget_callback, Callback},
        event::Event,
        key::Key,
        layout::{LayoutItemOptions, SizeHints},
        shortcut::{Shortcut, ShortcutId, ShortcutScope},
        style::{
            computed::CommonComputedStyle,
            css::{Element, PseudoClass},
            get_style,
        },
        system::{
            register_address, request_children_update, unregister_address, with_system,
            ChildrenUpdateState, ReportError,
        },
        types::{PhysicalPixels, Point, Rect, Size},
        widgets::WidgetExt,
        window::{Window, WindowId},
    },
    anyhow::{Context, Result},
    derivative::Derivative,
    log::{error, warn},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        fmt::Debug,
        marker::PhantomData,
        mem,
        ops::{Deref, DerefMut},
        rc::Rc,
    },
    stringcase::kebab_case,
    winit::window::CursorIcon,
};

#[derive(Debug, Clone)]
pub struct WidgetCreationContext {
    pub parent_id: Option<RawWidgetId>,
    pub address: WidgetAddress,
    pub window: Option<Window>,
    pub parent_scale: f32,
    pub is_parent_enabled: bool,
    pub is_window_root: bool,
}

pub type EventFilterFn = dyn Fn(Event) -> Result<bool>;

/// Information about position, size and clipping of a widget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetGeometry {
    /// Rect of this widget in parent coordinates.
    rect_in_parent: Rect,
    /// Top left of the parent widget in window coordinates.
    parent_top_left_in_window: Point,
    /// Parent widget's visible rect in parent widget's coordinates.
    parent_visible_rect_in_parent: Rect,
}

impl WidgetGeometry {
    pub fn root(size: Size) -> Self {
        WidgetGeometry {
            rect_in_parent: Rect::from_pos_size(Point::default(), size),
            parent_top_left_in_window: Point::default(),
            parent_visible_rect_in_parent: Rect::from_pos_size(Point::default(), size),
        }
    }

    /// Returns widget geometry of the child widget given the parent widget geometry and the
    /// rect of the child in the parent's coordinates.
    pub fn new(parent: &WidgetGeometry, rect_in_parent: Rect) -> Self {
        Self {
            rect_in_parent,
            parent_top_left_in_window: parent.rect_in_parent.top_left()
                + parent.parent_top_left_in_window,
            parent_visible_rect_in_parent: parent.visible_rect_in_self(),
        }
    }

    /// Rect of this widget in this widget's coordinates (top left is always zero).
    pub fn rect_in_self(&self) -> Rect {
        Rect::from_pos_size(Point::default(), self.rect_in_parent.size())
    }

    /// Rect of this widget in parent coordinates.
    pub fn rect_in_parent(&self) -> Rect {
        self.rect_in_parent
    }

    /// Size of the widget.
    pub fn size(&self) -> Size {
        self.rect_in_parent.size()
    }

    pub fn size_x(&self) -> PhysicalPixels {
        self.rect_in_parent.size_x()
    }

    pub fn size_y(&self) -> PhysicalPixels {
        self.rect_in_parent.size_y()
    }

    /// Rect of this widget in the window coordinates.
    pub fn rect_in_window(&self) -> Rect {
        self.rect_in_parent
            .translate(self.parent_top_left_in_window)
    }

    /// Visible rect of this widget in this widget's coordinates.
    pub fn visible_rect_in_self(&self) -> Rect {
        self.parent_visible_rect_in_parent
            .translate(-self.rect_in_parent.top_left())
            .intersect(self.rect_in_self())
    }
}

macro_rules! auto_bitflags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $Name:ident : $T:ty { $( $flag:ident ),+ $(,)? }
    ) => {
        #[repr(u8)]
        #[allow(non_camel_case_types, clippy::upper_case_acronyms)]
        enum __Flag { $( $flag ),+ }
        bitflags::bitflags! {
            $(#[$meta])*
            $vis struct $Name: $T {
                $(
                    const $flag = 1 << __Flag::$flag as $T;
                )+
            }
        }
    }
}

auto_bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    struct Flags: u64 {
        // explicitly set for this widget using `set_focusable`
        self_focusable,
        // reported by widget internally using `set_supports_focus`
        supports_focus,
        // widget currently has focus
        focused,
        // whether IME is enabled when the widget has focus
        input_method_enabled,
        // If true, all mouse events from the parent propagate to this widget,
        // regardless of its boundaries.
        receives_all_mouse_events,
        // true if parent is enabled or if there is no parent
        parent_enabled,
        // true if this widget hasn't been explicitly disabled
        self_enabled,
        self_visible,
        window_root,
        mouse_over,
        accessible,
        registered_as_focusable,
        has_declare_children_override,
    }
}

// TODO: use bitflags?
#[derive(Derivative)]
#[derivative(Debug)]
pub struct WidgetCommon {
    id: RawWidgetId,
    type_name: &'static str,
    flags: Flags,
    pub cursor_icon: CursorIcon,

    pub parent_id: Option<RawWidgetId>,
    pub address: WidgetAddress,
    pub window: Option<Window>,

    pub parent_scale: f32,
    pub self_scale: Option<f32>,
    pub is_self_visible: bool,

    pub is_window_root: bool,

    pub is_mouse_over: bool,

    // Present if the widget is not hidden, and only after layout.
    pub geometry: Option<WidgetGeometry>,

    #[derivative(Debug = "ignore")]
    pub children: BTreeMap<Key, Box<dyn Widget>>,
    pub layout_item_options: LayoutItemOptions,

    pub size_hint_x_cache: Option<SizeHints>,
    // TODO: limit count
    pub size_hint_y_cache: HashMap<PhysicalPixels, SizeHints>,
    pub is_accessible: bool,

    pub is_registered_as_focusable: bool,
    // TODO: multiple filters?
    // TODO: accept/reject event from filter; option to run filter after on_event
    #[derivative(Debug = "ignore")]
    pub event_filter: Option<Box<EventFilterFn>>,

    pub shortcuts: Vec<Shortcut>,
    pub style_element: Element,
    pub common_style: Rc<CommonComputedStyle>,

    pub num_added_children: u32,
    pub has_declare_children_override: bool,
    // Direct and indirect children created by last call of this widget's
    // `handle_update_children`.
    pub declared_children: HashSet<RawWidgetId>,
}

impl Drop for WidgetCommon {
    fn drop(&mut self) {
        unregister_address(self.id);
        // Drop and unmount children before unmounting self.
        self.children.clear();
        self.unmount_accessible();
        for shortcut in &self.shortcuts {
            // TODO: deregister widget/window shortcuts
            if shortcut.scope == ShortcutScope::Application {
                with_system(|system| system.application_shortcuts.retain(|s| s.id != shortcut.id));
            }
        }
    }
}

fn last_path_part(str: &str) -> &str {
    str.rsplit("::")
        .next()
        .expect("rsplit always returns at least one element")
}

impl WidgetCommon {
    #[allow(clippy::new_ret_no_self)]
    pub fn new<T: Widget>(ctx: WidgetCreationContext) -> WidgetCommonTyped<T> {
        let id = ctx.address.widget_id();
        register_address(id, ctx.address.clone());

        let type_name = T::type_name();
        let style_element = Element::new(kebab_case(last_path_part(type_name)))
            .with_pseudo_class(PseudoClass::Enabled);
        let common_style = get_style(&style_element, ctx.parent_scale);
        let mut common = Self {
            id,
            type_name,
            flags: Flags::self_enabled
                | Flags::self_visible
                | Flags::accessible
                | Flags::has_declare_children_override
                | if ctx.is_parent_enabled {
                    Flags::parent_enabled
                } else {
                    Flags::empty()
                },
            parent_id: ctx.parent_id,
            address: ctx.address,
            window: ctx.window,
            parent_scale: ctx.parent_scale,
            self_scale: None,
            is_self_visible: true,
            is_mouse_over: false,
            geometry: None,
            cursor_icon: CursorIcon::Default,
            children: BTreeMap::new(),
            layout_item_options: LayoutItemOptions::default(),
            size_hint_x_cache: None,
            size_hint_y_cache: HashMap::new(),
            is_accessible: true,
            is_registered_as_focusable: false,
            event_filter: None,
            is_window_root: ctx.is_window_root,
            shortcuts: Vec::new(),
            style_element,
            common_style,
            has_declare_children_override: true,
            num_added_children: 0,
            declared_children: Default::default(),
        };

        if let Some(window) = &common.window {
            let root_widget_id = window.root_widget_id();
            window.accessible_mount(
                if common.id == root_widget_id {
                    None
                } else if let Some(parent_id) = common.parent_id {
                    Some(parent_id.into())
                } else {
                    warn!("widget is not a window root so it must have a parent");
                    None
                },
                common.id.into(),
                // TODO: calculate visual index instead
                common
                    .address
                    .path
                    .last()
                    .map(|(key, _id)| key.clone())
                    .unwrap_or_else(|| {
                        warn!("WidgetCommon::new: empty address encountered");
                        "".into()
                    }),
            );
        }
        common.update();
        common.enabled_changed();
        common.focusable_changed();
        common.refresh_common_style();

        WidgetCommonTyped {
            common,
            _marker: PhantomData,
        }
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    pub fn is_self_visible(&self) -> bool {
        self.is_self_visible
    }

    /// True if this widget hasn't been explicitly disabled.
    ///
    /// This method can be used to tell if the widget is disabled because `set_enabled(false)` was called on it
    /// or because its parent is disabled. In most cases it's sufficient to use [is_enabled](Self::is_enabled) instead.
    pub fn is_self_enabled(&self) -> bool {
        self.flags.contains(Flags::self_enabled)
    }

    pub(crate) fn is_parent_enabled(&self) -> bool {
        self.flags.contains(Flags::parent_enabled)
    }

    /// True if this widget is enabled.
    ///
    /// Disabled widgets do not receive input events and have an alternate (usually grayed out) appearance.
    /// Use [set_enabled](Self::set_enabled) to enable or disable a widget. If a widget is disabled,
    /// all its children are disabled as well.
    pub fn is_enabled(&self) -> bool {
        self.flags
            .contains(Flags::self_enabled | Flags::parent_enabled)
    }

    pub(crate) fn self_enabled_changed(&mut self, enabled: bool) -> &mut Self {
        if self.flags.contains(Flags::self_enabled) == enabled {
            return self;
        }
        let old_enabled = self.is_enabled();
        self.flags.set(Flags::self_enabled, enabled);
        let new_enabled = self.is_enabled();
        if old_enabled == new_enabled {
            return self;
        }
        self.enabled_changed();
        self
    }

    pub(crate) fn parent_enabled_changed(&mut self, enabled: bool) -> &mut Self {
        if self.flags.contains(Flags::parent_enabled) == enabled {
            return self;
        }
        let old_enabled = self.is_enabled();
        self.flags.set(Flags::parent_enabled, enabled);
        let new_enabled = self.is_enabled();
        if old_enabled == new_enabled {
            return self;
        }
        self.enabled_changed();
        self
    }

    /// Returns `true` if this widget can receive focus.
    ///
    /// The widget is focusable if it supports focus, is not disabled and hasn't been configured with `set_focusable(false)`.
    /// Widgets that support focus are focusable by default.
    pub fn is_focusable(&self) -> bool {
        self.flags
            .contains(Flags::self_focusable | Flags::supports_focus)
            && self.is_enabled()
    }

    /// Returns `true` if the widget currently has focus.
    pub fn is_focused(&self) -> bool {
        self.flags.contains(Flags::focused) && self.is_enabled()
    }

    /// Returns `true` if the widget's OS window is focused.
    pub fn is_window_focused(&self) -> bool {
        self.window.as_ref().is_some_and(|w| w.is_focused())
    }

    pub fn new_creation_context(
        &self,
        new_id: RawWidgetId,
        key: Key,
        root_of_window: Option<Window>,
    ) -> WidgetCreationContext {
        WidgetCreationContext {
            parent_id: Some(self.id),
            address: self.address.clone().join(key, new_id),
            is_window_root: root_of_window.is_some(),
            window: root_of_window.or_else(|| self.window.clone()),
            parent_scale: self.scale(),
            is_parent_enabled: self.is_enabled(),
        }
    }

    // Request redraw and accessible update
    pub fn update(&mut self) {
        if let Some(window) = &self.window {
            window.request_redraw();
            window.request_accessible_update(self.address.clone());
        };
        request_children_update(self.address.clone());
    }

    pub fn has_child(&self, key: impl Into<Key>) -> bool {
        self.children.contains_key(&key.into())
    }

    pub fn add_child<T: Widget>(&mut self) -> &mut T {
        let key = self.num_added_children;
        self.num_added_children += 1;
        self.add_child_common(key.into(), false, None)
    }

    pub fn add_child_with_key<T: Widget>(&mut self, key: impl Into<Key>) -> &mut T {
        self.add_child_common(key.into(), false, None)
    }

    pub fn declare_child<T: Widget>(&mut self) -> &mut T {
        let key = with_system(|system| {
            let Some(state) = system.current_children_update.as_mut() else {
                warn!("declare_child called outside of handle_update_children");
                return 0;
            };
            let num = state.num_declared_children.entry(self.id).or_default();
            let key = *num;
            *num += 1;
            key
        });
        self.add_child_common(key.into(), true, None)
    }

    pub fn declare_child_with_key<T: Widget>(&mut self, key: impl Into<Key>) -> &mut T {
        self.add_child_common(key.into(), true, None)
    }

    fn add_child_common<T: Widget>(
        &mut self,
        key: Key,
        declare: bool,
        new_id: Option<RawWidgetId>,
    ) -> &mut T {
        if declare {
            if let Some(old_widget) = self
                .children
                .get_mut(&key)
                .and_then(|c| c.downcast_mut::<T>())
            {
                with_system(|system| {
                    if let Some(state) = &mut system.current_children_update {
                        state.declared_children.insert(old_widget.common().id);
                    } else {
                        warn!("declare_child shouldn't be used outside of declare_children()");
                    }
                });
                // Should be `return old_widget` but borrow checker is not smart enough.
                // Should we use `polonius-the-crab` crate?
                return self
                    .children
                    .get_mut(&key)
                    .and_then(|c| c.downcast_mut::<T>())
                    .unwrap();
            }
        }

        let new_id = new_id.unwrap_or_else(RawWidgetId::new_unique);
        let ctx = if T::is_window_root_type() {
            let new_window = Window::new(new_id);
            self.new_creation_context(new_id, key.clone(), Some(new_window.clone()))
        } else {
            self.new_creation_context(new_id, key.clone(), None)
        };
        // This may delete the old widget.
        self.children
            .insert(key.clone(), Box::new(T::new(WidgetCommon::new::<T>(ctx))));
        self.size_hint_changed();
        let widget = self.children.get_mut(&key).unwrap();
        if declare {
            with_system(|system| {
                if let Some(state) = &mut system.current_children_update {
                    state.declared_children.insert(widget.common().id);
                } else {
                    warn!("declare_child shouldn't be used outside of declare_children()");
                }
            });
        }
        widget.downcast_mut().unwrap()
    }

    pub fn get_dyn_child(&self, key: impl Into<Key>) -> anyhow::Result<&dyn Widget> {
        Ok(self
            .children
            .get(&key.into())
            .context("no such key")?
            .as_ref())
    }

    pub fn get_dyn_child_mut(&mut self, key: impl Into<Key>) -> anyhow::Result<&mut dyn Widget> {
        Ok(self
            .children
            .get_mut(&key.into())
            .context("no such key")?
            .as_mut())
    }

    pub fn get_child<T: Widget>(&self, key: impl Into<Key>) -> anyhow::Result<&T> {
        self.children
            .get(&key.into())
            .context("no such key")?
            .downcast_ref()
            .context("child type mismatch")
    }

    pub fn get_child_mut<T: Widget>(&mut self, key: impl Into<Key>) -> anyhow::Result<&mut T> {
        self.children
            .get_mut(&key.into())
            .context("no such key")?
            .downcast_mut()
            .context("child type mismatch")
    }

    pub fn layout_item_options(&self) -> &LayoutItemOptions {
        &self.layout_item_options
    }

    pub fn set_layout_item_options(&mut self, options: LayoutItemOptions) -> &mut Self {
        if self.layout_item_options == options {
            return self;
        }
        self.layout_item_options = options;
        self.size_hint_changed();
        self
    }

    pub fn remove_child(&mut self, key: impl Into<Key>) -> Result<(), WidgetNotFound> {
        self.children.remove(&key.into()).ok_or(WidgetNotFound)?;
        self.size_hint_changed();
        Ok(())
    }

    pub fn remove_child_by_id(&mut self, id: RawWidgetId) -> Result<(), WidgetNotFound> {
        if id == self.id {
            warn!("remove_child_by_id: cannot delete self");
            return Err(WidgetNotFound);
        }
        let Some(address) = address(id) else {
            // Widget probably already deleted.
            return Err(WidgetNotFound);
        };
        if !address.starts_with(&self.address) {
            warn!("remove_child_by_id: address is not within widget");
            return Err(WidgetNotFound);
        }
        let Some(parent_id) = address.parent_widget_id() else {
            error!("remove_child_by_id: encountered root widget");
            return Err(WidgetNotFound);
        };
        if parent_id == self.id {
            return self.remove_child(&address.path.last().unwrap().0);
        }
        let remaining_path = &address.path[self.address.len()..];
        let Some(mut current_widget) = self
            .children
            .get_mut(&remaining_path[0].0)
            .map(|c| c.as_mut())
        else {
            return Err(WidgetNotFound);
        };
        for (key, _id) in &address.path[self.address.len() + 1..] {
            if current_widget.common().id == parent_id {
                return current_widget
                    .common_mut()
                    .remove_child(&address.path.last().unwrap().0);
            }
            current_widget = current_widget
                .common_mut()
                .children
                .get_mut(key)
                .ok_or(WidgetNotFound)?
                .as_mut();
        }
        error!("remove_child_by_id: did not reach parent widget");
        Err(WidgetNotFound)
    }

    pub fn size_hint_changed(&mut self) {
        self.clear_size_hint_cache();
        let Some(window) = &self.window else {
            return;
        };
        window.invalidate_size_hint(self.address.clone());
    }

    pub fn clear_size_hint_cache(&mut self) {
        self.size_hint_x_cache = None;
        self.size_hint_y_cache.clear();
    }

    pub fn window_or_err(&self) -> Result<&Window> {
        self.window.as_ref().context("no window")
    }

    pub fn address(&self) -> &WidgetAddress {
        &self.address
    }

    pub fn size(&self) -> Option<Size> {
        self.geometry.as_ref().map(|g| g.size())
    }

    pub fn rect_in_window(&self) -> Option<Rect> {
        self.geometry.as_ref().map(|g| g.rect_in_window())
    }

    pub fn rect_in_parent(&self) -> Option<Rect> {
        self.geometry.as_ref().map(|g| g.rect_in_parent)
    }

    pub fn visible_rect(&self) -> Option<Rect> {
        self.geometry.as_ref().map(|g| g.visible_rect_in_self())
    }

    pub fn geometry_or_err(&self) -> Result<&WidgetGeometry> {
        self.geometry.as_ref().context("no geometry")
    }

    pub fn rect_in_window_or_err(&self) -> Result<Rect> {
        Ok(self.geometry_or_err()?.rect_in_window())
    }

    pub fn size_or_err(&self) -> Result<Size> {
        Ok(self.geometry_or_err()?.size())
    }

    pub fn rect_in_self_or_err(&self) -> Result<Rect> {
        Ok(self.geometry_or_err()?.rect_in_self())
    }

    /// Processes the event before it's dispatched to the widget.
    ///
    /// Returns `true` if the event is consumed and shouldn't be dispatched to the widget.
    pub(crate) fn before_event(&mut self, event: &Event) -> bool {
        match &event {
            Event::FocusIn(_) => {
                self.flags.insert(Flags::focused);
            }
            Event::FocusOut(_) => {
                self.flags.remove(Flags::focused);
            }
            Event::WindowFocusChange(_) => {}
            Event::MouseInput(event) => {
                for child in self.children.values_mut().rev() {
                    if let Some(rect_in_parent) = child.common().rect_in_parent() {
                        if let Some(child_event) = event.map_to_child(
                            rect_in_parent,
                            child.common().receives_all_mouse_events(),
                        ) {
                            if child.dispatch(child_event.into()) {
                                return true;
                            }
                        }
                    }
                }
            }
            Event::MouseScroll(event) => {
                for child in self.children.values_mut().rev() {
                    if let Some(rect_in_parent) = child.common().rect_in_parent() {
                        if let Some(child_event) = event.map_to_child(
                            rect_in_parent,
                            child.common().receives_all_mouse_events(),
                        ) {
                            if child.dispatch(child_event.into()) {
                                return true;
                            }
                        }
                    }
                }
            }
            Event::MouseMove(event) => {
                for child in self.children.values_mut().rev() {
                    if let Some(rect_in_parent) = child.common().rect_in_parent() {
                        if let Some(child_event) = event.map_to_child(
                            rect_in_parent,
                            child.common().receives_all_mouse_events(),
                        ) {
                            if child.dispatch(child_event.into()) {
                                return true;
                            }
                        }
                    }
                }
            }
            Event::MouseLeave(_) => {
                self.is_mouse_over = false;
            }
            Event::StyleChange(_) => {
                self.refresh_common_style();
            }
            Event::Draw(event) => {
                let Some(size) = self.size_or_err().or_report_err() else {
                    return false;
                };

                event.stroke_and_fill_rounded_rect(
                    Rect::from_pos_size(Point::default(), size),
                    &self.common_style.border,
                    self.common_style.background.as_ref(),
                );
            }
            Event::MouseEnter(_)
            | Event::KeyboardInput(_)
            | Event::InputMethod(_)
            | Event::Layout(_)
            | Event::AccessibilityAction(_) => {}
        }
        false
    }

    fn focusable_changed(&mut self) {
        let is_focusable = self.is_focusable();
        if is_focusable != self.is_registered_as_focusable {
            if let Some(window) = &self.window {
                if is_focusable {
                    window.add_focusable_widget(self.address.clone(), self.id);
                } else {
                    window.remove_focusable_widget(self.address.clone(), self.id);
                }
                self.is_registered_as_focusable = is_focusable;
            } else {
                self.is_registered_as_focusable = false;
            }
        }
    }

    fn enabled_changed(&mut self) {
        self.focusable_changed();
        // TODO: widget should receive MouseLeave even if it's disabled
        let is_enabled = self.is_enabled();
        for child in self.children.values_mut() {
            child.set_parent_enabled(is_enabled);
        }
    }

    /// Enable or disable input method for this widget.
    ///
    /// IME will be enabled for the window if the currently focused widget has input method enabled.
    /// Default value is false.
    ///
    /// Call this function with `true` in your widget's `new` function if your widget receives text input
    /// (as opposed to e.g. hotkeys). Implement [handle_input_method](Widget::handle_input_method) to
    /// handle events from the input method.
    pub fn set_input_method_enabled(&mut self, enabled: bool) -> &mut Self {
        if self.flags.contains(Flags::input_method_enabled) == enabled {
            return self;
        }
        self.flags.set(Flags::input_method_enabled, enabled);
        if self.is_focused() {
            if let Some(window) = &self.window {
                window.set_ime_allowed(enabled);
            }
        }
        self
    }

    /// Returns true if input method is enabled for this widget.
    ///
    /// See also [set_input_method_enabled](Self::set_input_method_enabled).
    pub fn is_input_method_enabled(&self) -> bool {
        self.flags.contains(Flags::input_method_enabled)
    }

    fn unmount_accessible(&mut self) {
        // println!("unmount_accessible {:?}", self.id);
        // for child in self.children.values_mut() {
        //     child.common_mut().unmount_accessible();
        // }
        if let Some(window) = &self.window {
            let root_widget_id = window.root_widget_id();
            window.accessible_unmount(
                if self.id == root_widget_id {
                    None
                } else {
                    self.parent_id.map(|id| id.into())
                },
                self.id.into(),
            );
        }
    }

    pub fn widget<W: Widget>(&mut self, id: WidgetId<W>) -> Result<&mut W, WidgetNotFound> {
        let w = self.widget_raw(id.raw())?;
        Ok(w.downcast_mut::<W>().expect("widget downcast failed"))
    }

    pub fn widget_raw(&mut self, id: RawWidgetId) -> Result<&mut dyn Widget, WidgetNotFound> {
        // TODO: speed up
        for child in self.children.values_mut() {
            if child.common().id == id {
                return Ok(child.as_mut());
            }
            if let Ok(widget) = child.common_mut().widget_raw(id) {
                return Ok(widget);
            }
        }
        Err(WidgetNotFound)
    }

    pub fn add_shortcut(&mut self, shortcut: Shortcut) -> ShortcutId {
        let id = shortcut.id;
        if shortcut.scope == ShortcutScope::Application {
            with_system(|system| system.application_shortcuts.push(shortcut.clone()));
        }
        // TODO: register widget/window shortcuts
        self.shortcuts.push(shortcut);
        id
    }

    // TODO: remove_shortcut

    pub fn refresh_common_style(&mut self) {
        self.common_style = get_style(&self.style_element, self.scale());
        self.size_hint_changed();
        self.update();
    }

    pub fn style_element(&self) -> &Element {
        &self.style_element
    }

    pub fn scale(&self) -> f32 {
        self.self_scale.unwrap_or(self.parent_scale)
    }

    pub(crate) fn set_scale(&mut self, scale: Option<f32>) -> &mut Self {
        self.self_scale = scale;
        self
    }

    pub fn set_accessible(&mut self, value: bool) {
        if self.is_accessible == value {
            return;
        }
        self.is_accessible = value;
        self.update();
    }

    pub fn after_declare_children(&mut self, state: ChildrenUpdateState) {
        let previous = mem::take(&mut self.declared_children);
        let to_delete = previous.difference(&state.declared_children);
        for id in to_delete {
            if self.remove_child_by_id(*id).is_ok() {
                //                println!("deleted {:?}", id);
            }
        }
        self.declared_children = state.declared_children;
    }

    /// Returns untyped ID of this widget.
    ///
    /// To obtain a typed ID of the widget, use [WidgetExt::id] externally
    /// or `self.common.id()` internally.
    pub fn id(&self) -> RawWidgetId {
        self.id
    }

    /// Returns full path to the widget type as a string.
    ///
    /// Returns the same value as [Widget::type_name].
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// Returns `true` if this widget's implementation is able to receive focus.
    ///
    /// This value is set by the widget's implementation. Widgets cannot be focusable if they don't support focus.
    /// Most widgets that support focus are focusable by default, but can be made unfocusable using [set_focusable](Self::set_focusable).
    pub fn supports_focus(&self) -> bool {
        self.flags.contains(Flags::supports_focus)
    }

    /// Set focusability of the widget.
    ///
    /// Widgets that support focus are usually focusable by default, so usually you shouldn't use this function.
    /// Disabling focus can make your interface less accessible.
    pub fn set_focusable(&mut self, focusable: bool) -> &mut Self {
        if focusable == self.flags.contains(Flags::self_focusable) {
            return self;
        }
        if focusable && !self.supports_focus() {
            warn!("cannot do `set_focusable(true)` on a widget that doesn't support focus");
            return self;
        }
        self.flags.set(Flags::self_focusable, focusable);
        self.focusable_changed();
        self
    }

    /// If true, all mouse events from the parent propagate to this widget,
    /// regardless of its boundaries.
    pub fn set_receives_all_mouse_events(&mut self, enabled: bool) -> &mut Self {
        if self.flags.contains(Flags::receives_all_mouse_events) == enabled {
            return self;
        }
        self.flags.set(Flags::receives_all_mouse_events, enabled);
        self
    }

    /// If true, all mouse events from the parent propagate to this widget,
    /// regardless of its boundaries.
    pub fn receives_all_mouse_events(&self) -> bool {
        self.flags.contains(Flags::receives_all_mouse_events)
    }
}

#[derive(Debug)]
pub struct WidgetCommonTyped<T> {
    pub common: WidgetCommon,
    _marker: PhantomData<T>,
}

impl<W> WidgetCommonTyped<W> {
    pub fn id(&self) -> WidgetId<W> {
        WidgetId::new(self.common.id)
    }

    pub fn callback<E, F>(&self, func: F) -> Callback<E>
    where
        W: Widget,
        F: Fn(&mut W, E) -> Result<()> + 'static,
        E: 'static,
    {
        widget_callback(WidgetId::<W>::new(self.common.id), func)
    }

    pub fn add_child<T: Widget>(&mut self) -> &mut T {
        self.common.add_child::<T>()
    }
    pub fn add_child_with_key<T: Widget>(&mut self, key: impl Into<Key>) -> &mut T {
        self.common.add_child_with_key::<T>(key)
    }

    /// Report the widget as supporting (or not supporting) focus.
    ///
    /// This function should only be called by the widget itself and should never be called by other widgets.
    /// To control focusability of other widgets, use [set_focusable](WidgetCommon::set_focusable) instead.
    ///
    /// Call `set_supports_focus(true)` from the `new` function of your widget if it supports being focusable.
    /// In most cases it's not needed to ever call it again.
    ///
    /// Note that it also implies `set_focusable(true)`. In a rare case when you don't want your widget
    /// to be focusable by default,
    /// Call `set_focusable(false)` **after** `set_supports_focus(true)`.
    pub fn set_supports_focus(&mut self, supports_focus: bool) -> &mut Self {
        let old_flags = self.common.flags.bits();
        self.common.flags.set(
            Flags::supports_focus | Flags::self_focusable,
            supports_focus,
        );
        if self.common.flags.bits() == old_flags {
            return self;
        }
        self.focusable_changed();
        self
    }
}

impl<T> Deref for WidgetCommonTyped<T> {
    type Target = WidgetCommon;

    fn deref(&self) -> &Self::Target {
        &self.common
    }
}

impl<T> DerefMut for WidgetCommonTyped<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.common
    }
}

impl<T> From<WidgetCommonTyped<T>> for WidgetCommon {
    fn from(value: WidgetCommonTyped<T>) -> Self {
        value.common
    }
}
