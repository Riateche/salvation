use std::{
    cell::Cell,
    collections::HashMap,
    fmt::{self, Debug},
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use accesskit::NodeId;
use anyhow::{Context, Result};
use downcast_rs::{impl_downcast, Downcast};
use log::warn;
use winit::window::{CursorIcon, WindowId};

use crate::{
    draw::DrawEvent,
    event::{
        AccessibleEvent, Event, FocusInEvent, FocusOutEvent, GeometryChangeEvent, ImeEvent,
        KeyboardInputEvent, MountEvent, MouseEnterEvent, MouseInputEvent, MouseLeaveEvent,
        MouseMoveEvent, UnmountEvent, WidgetScopeChangeEvent, WindowFocusChangeEvent,
    },
    layout::SizeHint,
    style::{computed::ComputedStyle, Style},
    system::{
        address, register_address, report_error, unregister_address, with_system, ReportError,
    },
    types::{Rect, Size},
    window::SharedWindowData,
};

pub mod button;
pub mod column;
pub mod image;
pub mod label;
pub mod padding_box;
pub mod stack;
pub mod text_input;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub size_hint_x_cache: Option<SizeHint>,
    // TODO: limit count
    pub size_hint_y_cache: HashMap<i32, SizeHint>,

    pub pending_accessible_update: bool,

    pub is_explicitly_enabled: bool,
    pub is_explicitly_visible: bool,
    pub explicit_style: Option<Rc<ComputedStyle>>,
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
            pending_accessible_update: false,
            parent_scope: WidgetScope::default(),
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
        mount_point.window.0.borrow_mut().widget_tree_changed = true;
        mount_point.window.0.borrow_mut().accessible_nodes.mount(
            mount_point.parent_id.map(|id| id.into()),
            self.id.into(),
            mount_point.index_in_parent,
        );
        self.mount_point = Some(mount_point);
        self.update();
        // TODO: set is_window_focused
    }

    pub fn unmount(&mut self) {
        if let Some(mount_point) = self.mount_point.take() {
            unregister_address(self.id);
            mount_point.window.0.borrow_mut().widget_tree_changed = true;
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

    pub fn add_child(&mut self, index: usize, mut widget: Box<dyn Widget>) {
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
            widget.set_parent_scope(self.effective_scope());
        }
        self.children.insert(
            index,
            Child {
                widget,
                rect_in_parent: None,
            },
        );
        self.remount_children(index + 1);
        self.size_hint_changed();
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

    pub fn remove_child(&mut self, index: usize) -> Box<dyn Widget> {
        let mut widget = self.children.remove(index).widget;
        if self.mount_point.is_some() {
            widget.dispatch(UnmountEvent.into());
            widget.set_parent_scope(WidgetScope::default());
        }
        self.remount_children(index);
        self.size_hint_changed();
        widget
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
    }

    pub fn mount_point_or_err(&self) -> Result<&MountPoint> {
        self.mount_point.as_ref().context("no mount point")
    }

    pub fn rect_in_window_or_err(&self) -> Result<Rect> {
        self.rect_in_window.context("no rect_in_window")
    }
}

impl Default for WidgetCommon {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
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
    pub rect_in_parent: Option<Rect>,
}

pub trait Widget: Downcast {
    fn common(&self) -> &WidgetCommon;
    fn common_mut(&mut self) -> &mut WidgetCommon;
    fn on_draw(&mut self, event: DrawEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_mouse_input(&mut self, event: MouseInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn on_mouse_enter(&mut self, event: MouseEnterEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn on_mouse_move(&mut self, event: MouseMoveEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn on_mouse_leave(&mut self, event: MouseLeaveEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    fn on_ime(&mut self, event: ImeEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }
    // TODO: we don't need accept/reject for some event types
    fn on_geometry_change(&mut self, event: GeometryChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_widget_scope_change(&mut self, event: WidgetScopeChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_mount(&mut self, event: MountEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_unmount(&mut self, event: UnmountEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_focus_in(&mut self, event: FocusInEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_focus_out(&mut self, event: FocusOutEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_window_focus_change(&mut self, event: WindowFocusChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_accessible(&mut self, event: AccessibleEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }
    fn on_event(&mut self, event: Event) -> Result<bool> {
        match event {
            Event::MouseInput(e) => self.on_mouse_input(e),
            Event::MouseEnter(e) => self.on_mouse_enter(e),
            Event::MouseMove(e) => self.on_mouse_move(e),
            Event::MouseLeave(e) => self.on_mouse_leave(e).map(|()| true),
            Event::KeyboardInput(e) => self.on_keyboard_input(e),
            Event::Ime(e) => self.on_ime(e),
            Event::Draw(e) => self.on_draw(e).map(|()| true),
            Event::GeometryChange(e) => self.on_geometry_change(e).map(|()| true),
            Event::Mount(e) => self.on_mount(e).map(|()| true),
            Event::Unmount(e) => self.on_unmount(e).map(|()| true),
            Event::FocusIn(e) => self.on_focus_in(e).map(|()| true),
            Event::FocusOut(e) => self.on_focus_out(e).map(|()| true),
            Event::WindowFocusChange(e) => self.on_window_focus_change(e).map(|()| true),
            Event::Accessible(e) => self.on_accessible(e).map(|()| true),
            Event::WidgetScopeChange(e) => self.on_widget_scope_change(e).map(|()| true),
        }
    }
    // TODO: result?
    fn size_hint_x(&mut self) -> SizeHint;
    fn size_hint_y(&mut self, size_x: i32) -> SizeHint;

    // TODO: result?
    #[must_use]
    fn layout(&mut self) -> Vec<Option<Rect>> {
        if !self.common().children.is_empty() {
            warn!("no layout impl for widget with children");
        }
        Vec::new()
    }
    // TODO: result?
    fn accessible_node(&mut self) -> Option<accesskit::NodeBuilder> {
        None
    }
}
impl_downcast!(Widget);

pub trait WidgetExt {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized;
    fn dispatch(&mut self, event: Event) -> bool;
    fn update_accessible(&mut self);
    fn apply_layout(&mut self);
    fn cached_size_hint_x(&mut self) -> SizeHint;
    fn cached_size_hint_y(&mut self, size_x: i32) -> SizeHint;

    // TODO: private
    fn set_parent_scope(&mut self, scope: WidgetScope);
    fn set_enabled(&mut self, enabled: bool);
    fn set_visible(&mut self, visible: bool);
    fn set_style(&mut self, style: Option<Style>);
}

impl<W: Widget + ?Sized> WidgetExt for W {
    fn id(&self) -> WidgetId<Self>
    where
        Self: Sized,
    {
        WidgetId(self.common().id, PhantomData)
    }

    fn dispatch(&mut self, event: Event) -> bool {
        let mut accepted = false;
        match &event {
            Event::GeometryChange(event) => {
                self.common_mut().rect_in_window = event.new_rect_in_window;
            }
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
            Event::MouseLeave(_) => {
                self.common_mut().is_mouse_over = false;
            }
            Event::WindowFocusChange(e) => {
                self.common_mut().is_window_focused = e.focused;
            }
            Event::MouseInput(event) => {
                for child in &mut self.common_mut().children {
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
            Event::MouseMove(event) => {
                for child in &mut self.common_mut().children {
                    if let Some(rect_in_parent) = child.rect_in_parent {
                        if rect_in_parent.contains(event.pos) {
                            let event = MouseMoveEvent {
                                pos: event.pos - rect_in_parent.top_left,
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
            _ => {}
        }
        if !accepted {
            accepted = match self.on_event(event.clone()) {
                Ok(r) => r,
                Err(err) => {
                    report_error(err);
                    false
                }
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
                        let child_event = event.map_to_child(rect_in_parent);
                        child.widget.dispatch(child_event.into());
                    }
                }
            }
            Event::WindowFocusChange(event) => {
                for child in &mut self.common_mut().children {
                    child.widget.dispatch(event.clone().into());
                }
            }
            Event::GeometryChange(_) => {
                self.apply_layout();
                self.common_mut().update();
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
            Event::KeyboardInput(_) | Event::Ime(_) | Event::Mount(_) | Event::Accessible(_) => {}
        }

        self.update_accessible();
        accepted
    }

    fn apply_layout(&mut self) {
        let mut rects = self.layout();
        if rects.is_empty() {
            rects = self.common().children.iter().map(|_| None).collect();
        }
        if rects.len() != self.common().children.len() {
            warn!("invalid length in layout output");
            return;
        }
        let rect_in_window = self.common().rect_in_window;
        for (rect_in_parent, child) in rects.into_iter().zip(self.common_mut().children.iter_mut())
        {
            child.rect_in_parent = rect_in_parent;
            let child_rect_in_window = if let Some(rect_in_window) = rect_in_window {
                rect_in_parent
                    .map(|rect_in_parent| rect_in_parent.translate(rect_in_window.top_left))
            } else {
                None
            };
            child.widget.dispatch(
                GeometryChangeEvent {
                    new_rect_in_window: child_rect_in_window,
                }
                .into(),
            );
        }
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

    fn cached_size_hint_x(&mut self) -> SizeHint {
        if let Some(cached) = &self.common().size_hint_x_cache {
            *cached
        } else {
            let r = self.size_hint_x();
            self.common_mut().size_hint_x_cache = Some(r);
            r
        }
    }
    fn cached_size_hint_y(&mut self, size_x: i32) -> SizeHint {
        if let Some(cached) = self.common().size_hint_y_cache.get(&size_x) {
            *cached
        } else {
            let r = self.size_hint_y(size_x);
            self.common_mut().size_hint_y_cache.insert(size_x, r);
            r
        }
    }

    fn set_parent_scope(&mut self, scope: WidgetScope) {
        self.common_mut().parent_scope = scope;
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.common_mut().is_explicitly_enabled = enabled;
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_visible(&mut self, visible: bool) {
        self.common_mut().is_explicitly_visible = visible;
        self.dispatch(WidgetScopeChangeEvent.into());
    }

    fn set_style(&mut self, style: Option<Style>) {
        let scale = self.common().parent_scope.style.scale;
        let style = style.map(|style| Rc::new(ComputedStyle::new(style, scale)));
        self.common_mut().explicit_style = style;
        self.dispatch(WidgetScopeChangeEvent.into());
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

#[derive(Debug, Clone)]
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
