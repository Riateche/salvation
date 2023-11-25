use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    fmt::{self, Debug},
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use accesskit::NodeId;
use anyhow::{bail, Context, Result};
use downcast_rs::{impl_downcast, Downcast};
use log::warn;
use thiserror::Error;
use winit::window::{CursorIcon, WindowId};

use crate::{
    callback::{widget_callback, Callback},
    draw::DrawEvent,
    event::{
        AccessibleActionEvent, Event, FocusInEvent, FocusOutEvent, ImeEvent, KeyboardInputEvent,
        LayoutEvent, MountEvent, MouseEnterEvent, MouseInputEvent, MouseLeaveEvent, MouseMoveEvent,
        UnmountEvent, WidgetScopeChangeEvent, WindowFocusChangeEvent,
    },
    layout::{LayoutItemOptions, SizeHints, FALLBACK_SIZE_HINT},
    style::{computed::ComputedStyle, Style},
    system::{address, register_address, unregister_address, with_system, ReportError},
    types::{Rect, Size},
    window::SharedWindowData,
};

pub mod button;
pub mod column;
pub mod image;
pub mod label;
pub mod padding_box;
pub mod scroll_area;
pub mod scroll_bar;
pub mod stack;
pub mod text_input;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawWidgetId(pub u64);

impl RawWidgetId {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

impl From<RawWidgetId> for NodeId {
    fn from(value: RawWidgetId) -> Self {
        value.0.into()
    }
}

pub struct WidgetId<T>(pub RawWidgetId, pub PhantomData<T>);

impl<T> WidgetId<T> {
    pub fn new(id: RawWidgetId) -> Self {
        Self(id, PhantomData)
    }
}

impl<T> Debug for WidgetId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl<T> Clone for WidgetId<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for WidgetId<T> {}

#[derive(Debug, Clone)]
pub struct WidgetScope {
    pub is_visible: bool,
    pub is_enabled: bool,
    pub style: Rc<ComputedStyle>,
}

impl Default for WidgetScope {
    fn default() -> Self {
        with_system(|s| Self {
            is_visible: true,
            is_enabled: true,
            style: s.default_style.clone(),
        })
    }
}

pub type EventFilterFn = dyn Fn(Event) -> Result<bool>;

pub struct WidgetCommon {
    pub id: RawWidgetId,
    pub is_focusable: bool,
    pub enable_ime: bool,
    pub cursor_icon: CursorIcon,

    pub is_focused: bool,
    // TODO: set initial value in mount event
    pub is_window_focused: bool,
    pub parent_scope: WidgetScope,

    pub is_mouse_over: bool,
    pub mount_point: Option<MountPoint>,
    // Present if the widget is mounted, not hidden, and only after layout.
    pub rect_in_window: Option<Rect>,

    pub children: Vec<Child>,
    pub current_layout_event: Option<LayoutEvent>,

    pub size_hint_x_cache: Option<i32>,
    // TODO: limit count
    pub size_hint_y_cache: HashMap<i32, i32>,
    pub size_hint_x_fixed_cache: Option<bool>,
    pub size_hint_y_fixed_cache: Option<bool>,

    pub pending_accessible_update: bool,

    pub is_explicitly_enabled: bool,
    pub is_explicitly_visible: bool,
    pub explicit_style: Option<Rc<ComputedStyle>>,

    pub is_registered_as_focusable: bool,
    // TODO: multiple filters?
    // TODO: accept/reject event from filter; option to run filter after on_event
    pub event_filter: Option<Box<EventFilterFn>>,
}

#[derive(Debug, Clone)]
pub struct MountPoint {
    pub address: WidgetAddress,
    pub window: SharedWindowData,
    // TODO: move out? unmounted widget can have parent
    pub parent_id: Option<RawWidgetId>,
    // Determines visual / accessible order.
    // TODO: remove, use address
    pub index_in_parent: usize,
}

impl WidgetCommon {
    pub fn new() -> Self {
        Self {
            id: RawWidgetId::new(),
            is_explicitly_enabled: true,
            is_explicitly_visible: true,
            explicit_style: None,
            is_focusable: false,
            is_focused: false,
            is_mouse_over: false,
            is_window_focused: false,
            enable_ime: false,
            mount_point: None,
            rect_in_window: None,
            cursor_icon: CursorIcon::Default,
            children: Vec::new(),
            size_hint_x_cache: None,
            size_hint_y_cache: HashMap::new(),
            size_hint_x_fixed_cache: None,
            size_hint_y_fixed_cache: None,
            pending_accessible_update: false,
            parent_scope: WidgetScope::default(),

            is_registered_as_focusable: false,
            event_filter: None,
            current_layout_event: None,
        }
    }

    //    let address = parent_address.join(self.id);

    pub fn mount(&mut self, mount_point: MountPoint) {
        if self.mount_point.is_some() {
            warn!("widget was already mounted");
        }
        let old = register_address(self.id, mount_point.address.clone());
        if old.is_some() {
            warn!("widget address was already registered");
        }
        mount_point.window.0.borrow_mut().accessible_nodes.mount(
            mount_point.parent_id.map(|id| id.into()),
            self.id.into(),
            mount_point.index_in_parent,
        );
        self.mount_point = Some(mount_point);
        self.update();
        self.register_focusable();
        // TODO: set is_window_focused
    }

    pub fn unmount(&mut self) {
        if let Some(mount_point) = self.mount_point.take() {
            if self.is_registered_as_focusable {
                mount_point
                    .window
                    .remove_focusable_widget(mount_point.address.clone(), self.id);
                self.is_registered_as_focusable = false;
            }
            unregister_address(self.id);
            mount_point
                .window
                .0
                .borrow_mut()
                .accessible_nodes
                .update(self.id.0.into(), None);
            mount_point
                .window
                .0
                .borrow_mut()
                .accessible_nodes
                .unmount(mount_point.parent_id.map(|id| id.into()), self.id.into());
        } else {
            warn!("widget was not mounted");
        }
        self.is_focused = false;
        self.is_window_focused = false;
    }

    pub fn is_visible(&self) -> bool {
        self.parent_scope.is_visible && self.is_explicitly_visible
    }

    pub fn is_enabled(&self) -> bool {
        self.parent_scope.is_enabled && self.is_explicitly_enabled
    }

    pub fn is_focusable(&self) -> bool {
        self.is_focusable && self.is_enabled()
    }

    pub fn is_focused(&self) -> bool {
        self.is_focused && self.is_window_focused
    }

    pub fn style(&self) -> &Rc<ComputedStyle> {
        self.explicit_style
            .as_ref()
            .unwrap_or(&self.parent_scope.style)
    }

    pub fn effective_scope(&self) -> WidgetScope {
        WidgetScope {
            is_visible: self.is_visible(),
            is_enabled: self.is_enabled(),
            // TODO: allow overriding scale?
            style: self.style().clone(),
        }
    }

    pub fn size(&self) -> Option<Size> {
        self.rect_in_window.as_ref().map(|g| g.size)
    }

    // Request redraw and accessible update
    pub fn update(&mut self) {
        let Some(mount_point) = &self.mount_point else {
            return;
        };
        mount_point.window.request_redraw();
        self.pending_accessible_update = true;
    }

    pub fn add_child(&mut self, widget: Box<dyn Widget>, options: LayoutItemOptions) -> usize {
        let index = self.children.len();
        self.insert_child(index, widget, options)
            .expect("should never fail with correct index");
        index
    }

    // TODO: fn replace_child
    pub fn insert_child(
        &mut self,
        index: usize,
        mut widget: Box<dyn Widget>,
        options: LayoutItemOptions,
    ) -> Result<()> {
        if index > self.children.len() {
            bail!("index out of bounds");
        }
        if let Some(mount_point) = &self.mount_point {
            let address = mount_point.address.clone().join(index);
            widget.dispatch(
                MountEvent(MountPoint {
                    address,
                    window: mount_point.window.clone(),
                    parent_id: Some(self.id),
                    index_in_parent: index,
                })
                .into(),
            );
        }
        widget.set_parent_scope(self.effective_scope());
        self.children.insert(
            index,
            Child {
                widget,
                options,
                rect_in_parent: None,
                rect_set_during_layout: false,
            },
        );
        self.remount_children(index + 1);
        self.size_hint_changed();
        Ok(())
    }

    pub fn set_child_options(&mut self, index: usize, options: LayoutItemOptions) -> Result<()> {
        if index >= self.children.len() {
            bail!("index out of bounds");
        }
        self.children[index].options = options;
        self.size_hint_changed();
        Ok(())
    }

    fn remount_children(&mut self, from_index: usize) {
        if let Some(mount_point) = &self.mount_point {
            for i in from_index..self.children.len() {
                self.children[i].widget.dispatch(UnmountEvent.into());
                self.children[i].widget.dispatch(
                    MountEvent(MountPoint {
                        address: mount_point.address.clone().join(i),
                        window: mount_point.window.clone(),
                        parent_id: Some(self.id),
                        index_in_parent: i,
                    })
                    .into(),
                );
            }
        }
    }

    pub fn remove_child(&mut self, index: usize) -> Result<Box<dyn Widget>> {
        if index >= self.children.len() {
            bail!("invalid child index");
        }
        let mut widget = self.children.remove(index).widget;
        if self.mount_point.is_some() {
            widget.dispatch(UnmountEvent.into());
        }
        widget.set_parent_scope(WidgetScope::default());
        self.remount_children(index);
        self.size_hint_changed();
        Ok(widget)
    }

    pub fn set_child_rect(&mut self, index: usize, rect_in_parent: Option<Rect>) -> Result<()> {
        let child = self
            .children
            .get_mut(index)
            .context("set_child_rect: invalid child index")?;

        let rect_in_window = if let Some(rect_in_window) = self.rect_in_window {
            rect_in_parent.map(|rect_in_parent| rect_in_parent.translate(rect_in_window.top_left))
        } else {
            None
        };
        child.rect_in_parent = rect_in_parent;
        let rect_changed = child.widget.common().rect_in_window != rect_in_window;
        if let Some(event) = &self.current_layout_event {
            let mount_point = child.widget.common().mount_point_or_err()?;
            if rect_changed || event.size_hints_changed_within(&mount_point.address) {
                child.widget.dispatch(
                    LayoutEvent {
                        new_rect_in_window: rect_in_window,
                        changed_size_hints: event.changed_size_hints.clone(),
                    }
                    .into(),
                );
            }
            child.rect_set_during_layout = true;
        } else {
            if rect_changed {
                child.widget.dispatch(
                    LayoutEvent {
                        new_rect_in_window: rect_in_window,
                        changed_size_hints: Vec::new(),
                    }
                    .into(),
                );
            }
        }
        Ok(())
    }

    pub fn set_child_rects(&mut self, rects: &BTreeMap<usize, Rect>) -> Result<()> {
        let len = self.children.len();
        for index in 0..len {
            self.set_child_rect(index, rects.get(&index).copied())?;
        }
        Ok(())
    }

    pub fn size_hint_changed(&mut self) {
        self.clear_size_hint_cache();
        let Some(mount_point) = &self.mount_point else {
            return;
        };
        mount_point
            .window
            .0
            .borrow_mut()
            .pending_size_hint_invalidations
            .push(mount_point.address.clone());
    }

    fn clear_size_hint_cache(&mut self) {
        self.size_hint_x_cache = None;
        self.size_hint_y_cache.clear();
        self.size_hint_x_fixed_cache = None;
        self.size_hint_y_fixed_cache = None;
    }

    pub fn mount_point_or_err(&self) -> Result<&MountPoint> {
        self.mount_point.as_ref().context("no mount point")
    }

    pub fn rect_in_window_or_err(&self) -> Result<Rect> {
        self.rect_in_window.context("no rect_in_window")
    }

    pub fn size_or_err(&self) -> Result<Size> {
        Ok(self.rect_in_window.context("no rect_in_window")?.size)
    }

    fn register_focusable(&mut self) {
        let is_focusable = self.is_focusable();
        if is_focusable != self.is_registered_as_focusable {
            if let Some(mount_point) = &self.mount_point {
                if is_focusable {
                    mount_point
                        .window
                        .add_focusable_widget(mount_point.address.clone(), self.id);
                } else {
                    mount_point
                        .window
                        .remove_focusable_widget(mount_point.address.clone(), self.id);
                }
                self.is_registered_as_focusable = is_focusable;
            } else {
                self.is_registered_as_focusable = false;
            }
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_explicitly_enabled = enabled;
        self.register_focusable();
    }

    pub fn set_focusable(&mut self, focusable: bool) {
        self.is_focusable = focusable;
        self.register_focusable();
    }

    pub fn set_parent_scope(&mut self, scope: WidgetScope) {
        self.parent_scope = scope;
        self.register_focusable();
    }

    /*pub fn only_child_size_hint_x(&mut self) -> Result<SizeHint> {
        if self.children.is_empty() {
            bail!("no children");
        }
        if self.children.len() > 1 {
            warn!("more than one child found, using first child's size hint");
        }
        Ok(self.children[0].widget.cached_size_hint_x())
    }

    pub fn only_child_size_hint_y(&mut self, size_x: i32) -> Result<SizeHint> {
        if self.children.is_empty() {
            bail!("no children");
        }
        if self.children.len() > 1 {
            warn!("more than one child found, using first child's size hint");
        }
        Ok(self.children[0].widget.cached_size_hint_y(size_x))
    }

    pub fn layout_child_as_self(&self) -> Vec<Option<Rect>> {
        let rect = self
            .rect_in_window_or_err()
            .or_report_err()
            .map(|rect| Rect {
                top_left: Point::default(),
                size: rect.size,
            });
        self.children.iter().map(|_| rect).collect()
    }*/
}

impl Default for WidgetCommon {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
#[error("widget not found")]
pub struct WidgetNotFound;

pub fn get_widget_by_address_mut<'a>(
    root_widget: &'a mut dyn Widget,
    address: &WidgetAddress,
) -> Result<&'a mut dyn Widget, WidgetNotFound> {
    let mut current_widget = root_widget;
    for &index in &address.path {
        current_widget = current_widget
            .common_mut()
            .children
            .get_mut(index)
            .ok_or(WidgetNotFound)?
            .widget
            .as_mut();
    }
    Ok(current_widget)
}

pub fn get_widget_by_id_mut(
    root_widget: &mut dyn Widget,
    id: RawWidgetId,
) -> Result<&mut dyn Widget, WidgetNotFound> {
    let address = address(id).ok_or(WidgetNotFound)?;
    get_widget_by_address_mut(root_widget, &address)
}

pub struct Child {
    pub widget: Box<dyn Widget>,
    pub options: LayoutItemOptions,
    pub rect_in_parent: Option<Rect>,
    pub rect_set_during_layout: bool,
}

pub trait Widget: Downcast {
    fn common(&self) -> &WidgetCommon;
    fn common_mut(&mut self) -> &mut WidgetCommon;
    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_mouse_input(&mut self, event: MouseInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn handle_mouse_enter(&mut self, event: MouseEnterEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn handle_mouse_leave(&mut self, event: MouseLeaveEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn handle_ime(&mut self, event: ImeEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn handle_layout(&mut self, event: LayoutEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_widget_scope_change(&mut self, event: WidgetScopeChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_mount(&mut self, event: MountEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_unmount(&mut self, event: UnmountEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_focus_in(&mut self, event: FocusInEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_focus_out(&mut self, event: FocusOutEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_window_focus_change(&mut self, event: WindowFocusChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_accessible_action(&mut self, event: AccessibleActionEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn handle_event(&mut self, event: Event) -> Result<bool> {
        match event {
            Event::MouseInput(e) => self.handle_mouse_input(e),
            Event::MouseEnter(e) => self.handle_mouse_enter(e),
            Event::MouseMove(e) => self.handle_mouse_move(e),
            Event::MouseLeave(e) => self.handle_mouse_leave(e).map(|()| true),
            Event::KeyboardInput(e) => self.handle_keyboard_input(e),
            Event::Ime(e) => self.handle_ime(e),
            Event::Draw(e) => self.handle_draw(e).map(|()| true),
            Event::Layout(e) => self.handle_layout(e).map(|()| true),
            Event::Mount(e) => self.handle_mount(e).map(|()| true),
            Event::Unmount(e) => self.handle_unmount(e).map(|()| true),
            Event::FocusIn(e) => self.handle_focus_in(e).map(|()| true),
            Event::FocusOut(e) => self.handle_focus_out(e).map(|()| true),
            Event::WindowFocusChange(e) => self.handle_window_focus_change(e).map(|()| true),
            Event::Accessible(e) => self.handle_accessible_action(e).map(|()| true),
            Event::WidgetScopeChange(e) => self.handle_widget_scope_change(e).map(|()| true),
        }
    }
    fn size_hint_x(&mut self) -> Result<i32>;
    fn size_hint_y(&mut self, size_x: i32) -> Result<i32>;
    fn is_size_hint_x_fixed(&mut self) -> bool {
        true
    }
    fn is_size_hint_y_fixed(&mut self) -> bool {
        true
    }

    // TODO: result?
    fn accessible_node(&mut self) -> Option<accesskit::NodeBuilder> {
        None
    }
}
impl_downcast!(Widget);

pub struct WidgetWithId<W> {
    pub id: WidgetId<W>,
    pub widget: W,
}

pub trait WidgetExt {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized;

    fn callback<F, E>(&self, func: F) -> Callback<E>
    where
        F: Fn(&mut Self, E) -> Result<()> + 'static,
        E: 'static,
        Self: Sized;

    fn split_id(self) -> WidgetWithId<Self>
    where
        Self: Sized,
    {
        WidgetWithId {
            id: self.id(),
            widget: self,
        }
    }

    fn dispatch(&mut self, event: Event) -> bool;
    fn update_accessible(&mut self);
    fn cached_size_hint_x(&mut self) -> i32;
    fn cached_size_hints_x(&mut self) -> SizeHints;
    fn cached_size_hint_y(&mut self, size_x: i32) -> i32;
    fn cached_size_hints_y(&mut self, size_x: i32) -> SizeHints;
    fn cached_size_hint_x_fixed(&mut self) -> bool;
    fn cached_size_hint_y_fixed(&mut self) -> bool;

    // TODO: private
    fn set_parent_scope(&mut self, scope: WidgetScope);
    fn set_enabled(&mut self, enabled: bool);
    fn set_visible(&mut self, visible: bool);
    fn set_style(&mut self, style: Option<Style>) -> Result<()>;

    fn boxed(self) -> Box<dyn Widget>
    where
        Self: Sized;
}

impl<W: Widget + ?Sized> WidgetExt for W {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized,
    {
        WidgetId(self.common().id, PhantomData)
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
            Event::Mount(event) => {
                let mount_point = event.0.clone();
                self.common_mut().mount(mount_point.clone());

                let id = self.common().id;
                for (i, child) in self.common_mut().children.iter_mut().enumerate() {
                    let child_address = mount_point.address.clone().join(i);
                    child.widget.dispatch(
                        MountEvent(MountPoint {
                            address: child_address,
                            parent_id: Some(id),
                            window: mount_point.window.clone(),
                            index_in_parent: i,
                        })
                        .into(),
                    );
                }
            }
            // TODO: before or after handler?
            Event::Unmount(_event) => {
                for child in &mut self.common_mut().children {
                    child.widget.dispatch(UnmountEvent.into());
                }
            }
            Event::FocusIn(_) => {
                self.common_mut().is_focused = true;
            }
            Event::FocusOut(_) => {
                self.common_mut().is_focused = false;
            }
            Event::WindowFocusChange(e) => {
                self.common_mut().is_window_focused = e.focused;
            }
            Event::MouseInput(event) => {
                should_dispatch = self.common().is_enabled();
                if should_dispatch {
                    for child in self.common_mut().children.iter_mut().rev() {
                        if let Some(rect_in_parent) = child.rect_in_parent {
                            if let Some(child_event) = event.map_to_child(rect_in_parent) {
                                if child.widget.dispatch(child_event.into()) {
                                    accepted = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            Event::MouseEnter(_) | Event::KeyboardInput(_) | Event::Ime(_) => {
                should_dispatch = self.common().is_enabled();
            }
            Event::MouseMove(event) => {
                should_dispatch = self.common().is_enabled();
                if should_dispatch {
                    for child in self.common_mut().children.iter_mut().rev() {
                        if let Some(rect_in_parent) = child.rect_in_parent {
                            if rect_in_parent.contains(event.pos) {
                                let event = MouseMoveEvent {
                                    pos: event.pos - rect_in_parent.top_left,
                                    pos_in_window: event.pos_in_window(),
                                    device_id: event.device_id,
                                    accepted_by: event.accepted_by.clone(),
                                };
                                if child.widget.dispatch(event.into()) {
                                    accepted = true;
                                    break;
                                }
                            }
                        }
                    }

                    if !accepted {
                        let is_enter = if let Some(mount_point) =
                            self.common().mount_point_or_err().or_report_err()
                        {
                            let self_id = self.common().id;
                            !mount_point
                                .window
                                .0
                                .borrow()
                                .mouse_entered_widgets
                                .iter()
                                .any(|(_, id)| *id == self_id)
                        } else {
                            false
                        };

                        if is_enter {
                            self.dispatch(
                                MouseEnterEvent {
                                    device_id: event.device_id,
                                    pos: event.pos,
                                    accepted_by: event.accepted_by.clone(),
                                }
                                .into(),
                            );
                        }
                    }
                }
            }
            Event::MouseLeave(_) => {
                self.common_mut().is_mouse_over = false;
                should_dispatch = self.common().is_enabled();
            }
            Event::Layout(event) => {
                self.common_mut().rect_in_window = event.new_rect_in_window;
                self.common_mut().current_layout_event = Some(event.clone());
                for child in &mut self.common_mut().children {
                    child.rect_set_during_layout = false;
                }
            }
            _ => {}
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
            Event::MouseInput(event) => {
                if event.accepted_by().is_none() && accepted {
                    event.set_accepted_by(self.common().id);
                }
            }
            Event::MouseEnter(event) => {
                accept_mouse_event(self, true, &event.accepted_by);
            }
            Event::MouseMove(event) => {
                accept_mouse_event(self, false, &event.accepted_by);
            }
            Event::Unmount(_) => {
                self.common_mut().unmount();
            }
            Event::Draw(event) => {
                for child in &mut self.common_mut().children {
                    if let Some(rect_in_parent) = child.rect_in_parent {
                        if let Some(child_event) = event.map_to_child(rect_in_parent) {
                            child.widget.dispatch(child_event.into());
                        }
                    }
                }
            }
            Event::WindowFocusChange(event) => {
                for child in &mut self.common_mut().children {
                    child.widget.dispatch(event.clone().into());
                }
            }
            Event::WidgetScopeChange(_) => {
                let scope = self.common().effective_scope();
                for child in &mut self.common_mut().children {
                    child.widget.as_mut().set_parent_scope(scope.clone());
                }
                self.common_mut().update();
            }
            Event::FocusIn(_) | Event::FocusOut(_) | Event::MouseLeave(_) => {
                self.common_mut().update();
            }
            Event::Layout(_) => {
                let len = self.common().children.len();
                for i in 0..len {
                    if !self.common().children[i].rect_set_during_layout {
                        let rect_in_parent = self.common().children[i].rect_in_parent;
                        self.common_mut()
                            .set_child_rect(i, rect_in_parent)
                            .or_report_err();
                    }
                    self.common_mut().children[i].rect_set_during_layout = false;
                }
                self.common_mut().current_layout_event = None;
                self.common_mut().update();
            }
            Event::KeyboardInput(_) | Event::Ime(_) | Event::Mount(_) | Event::Accessible(_) => {}
        }

        self.update_accessible();
        accepted
    }

    fn update_accessible(&mut self) {
        if !self.common().pending_accessible_update {
            return;
        }
        let node = self.accessible_node();
        let Some(mount_point) = self.common().mount_point.as_ref() else {
            return;
        };
        let rect = self.common().rect_in_window;
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
        mount_point
            .window
            .0
            .borrow_mut()
            .accessible_nodes
            .update(self.common().id.0.into(), node);
        self.common_mut().pending_accessible_update = false;
    }

    fn cached_size_hint_x(&mut self) -> i32 {
        if let Some(cached) = self.common().size_hint_x_cache {
            cached
        } else {
            let r = self
                .size_hint_x()
                .or_report_err()
                .unwrap_or(FALLBACK_SIZE_HINT);
            self.common_mut().size_hint_x_cache = Some(r);
            r
        }
    }
    fn cached_size_hints_x(&mut self) -> SizeHints {
        SizeHints {
            value: self.cached_size_hint_x(),
            is_fixed: self.cached_size_hint_x_fixed(),
        }
    }
    fn cached_size_hint_y(&mut self, size_x: i32) -> i32 {
        if let Some(cached) = self.common().size_hint_y_cache.get(&(size_x)) {
            *cached
        } else {
            let r = self
                .size_hint_y(size_x)
                .or_report_err()
                .unwrap_or(FALLBACK_SIZE_HINT);
            self.common_mut().size_hint_y_cache.insert(size_x, r);
            r
        }
    }

    fn cached_size_hints_y(&mut self, size_x: i32) -> SizeHints {
        SizeHints {
            value: self.cached_size_hint_y(size_x),
            is_fixed: self.cached_size_hint_y_fixed(),
        }
    }

    fn cached_size_hint_x_fixed(&mut self) -> bool {
        if let Some(cached) = self.common().size_hint_x_fixed_cache {
            cached
        } else {
            let r = self.is_size_hint_x_fixed();
            self.common_mut().size_hint_x_fixed_cache = Some(r);
            r
        }
    }
    fn cached_size_hint_y_fixed(&mut self) -> bool {
        if let Some(cached) = self.common().size_hint_y_fixed_cache {
            cached
        } else {
            let r = self.is_size_hint_y_fixed();
            self.common_mut().size_hint_y_fixed_cache = Some(r);
            r
        }
    }

    fn set_parent_scope(&mut self, scope: WidgetScope) {
        self.common_mut().set_parent_scope(scope);
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.common_mut().set_enabled(enabled);
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_visible(&mut self, visible: bool) {
        self.common_mut().is_explicitly_visible = visible;
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_style(&mut self, style: Option<Style>) -> Result<()> {
        let scale = self.common().parent_scope.style.scale;
        let style = if let Some(style) = style {
            Some(Rc::new(ComputedStyle::new(&style, scale)?))
        } else {
            None
        };
        self.common_mut().explicit_style = style;
        self.dispatch(WidgetScopeChangeEvent.into());
        Ok(())
    }

    fn boxed(self) -> Box<dyn Widget>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

fn accept_mouse_event(
    widget: &mut (impl Widget + ?Sized),
    is_enter: bool,
    accepted_by: &Rc<Cell<Option<RawWidgetId>>>,
) {
    if accepted_by.get().is_none() {
        let Some(rect_in_window) = widget.common().rect_in_window_or_err().or_report_err() else {
            return;
        };
        let Some(mount_point) = widget.common().mount_point_or_err().or_report_err() else {
            return;
        };
        let id = widget.common().id;
        accepted_by.set(Some(id));

        mount_point
            .window
            .0
            .borrow()
            .winit_window
            .set_cursor_icon(widget.common().cursor_icon);
        if is_enter {
            mount_point
                .window
                .0
                .borrow_mut()
                .mouse_entered_widgets
                .push((rect_in_window, id));

            widget.common_mut().is_mouse_over = true;
            widget.common_mut().update();
        }
    }
}

pub fn invalidate_size_hint_cache(widget: &mut dyn Widget, pending: &[WidgetAddress]) {
    let common = widget.common_mut();
    let Some(mount_point) = &common.mount_point else {
        return;
    };
    for pending_addr in pending {
        if pending_addr.starts_with(&mount_point.address) {
            common.clear_size_hint_cache();
            for child in &mut common.children {
                invalidate_size_hint_cache(child.widget.as_mut(), pending);
            }
            return;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WidgetAddress {
    pub window_id: WindowId,
    pub path: Vec<usize>,
}

impl WidgetAddress {
    pub fn window_root(window_id: WindowId) -> Self {
        Self {
            window_id,
            path: Vec::new(),
        }
    }
    pub fn join(mut self, index: usize) -> Self {
        self.path.push(index);
        self
    }
    pub fn starts_with(&self, base: &WidgetAddress) -> bool {
        self.window_id == base.window_id
            && base.path.len() <= self.path.len()
            && base.path == self.path[..base.path.len()]
    }
}
