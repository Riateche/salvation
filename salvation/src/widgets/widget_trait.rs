use {
    super::{WidgetCommon, WidgetCommonTyped},
    crate::{
        draw::DrawEvent,
        event::{
            AccessibilityActionEvent, Event, FocusInEvent, FocusOutEvent, InputMethodEvent,
            KeyboardInputEvent, LayoutEvent, MouseEnterEvent, MouseInputEvent, MouseLeaveEvent,
            MouseMoveEvent, MouseScrollEvent, ScrollToRectRequest, StyleChangeEvent,
            WindowFocusChangeEvent,
        },
        layout::{
            grid::{self, grid_layout},
            SizeHints,
        },
        types::PhysicalPixels,
    },
    anyhow::Result,
    log::warn,
    std::any::Any,
};

pub trait Widget: Any {
    /// Returns full path to the widget type as a string.
    ///
    /// It's recommended to use [impl_widget_common!](crate::impl_widget_common) macro
    /// to automatically implement this method.
    /// If not using the macro, it's recommended to return `std::any::type_name::<Self>()`
    /// from this function.
    fn type_name() -> &'static str
    where
        Self: Sized;

    /// Returns true if this widget type is a window root.
    ///
    /// Default implementation returns `false` which should always suffice unless you're extending
    /// [WindowWidget](super::window::WindowWidget).
    ///
    /// If `is_window_root_type() == true`, when a widget of this type
    /// is created, a new OS window will also be created that will contain this widget.
    /// If `is_window_root_type() == false`, when a widget of this type is created, it will be displayed within
    /// its parent widget. Default implementation returns `false`. The only built-in widget type that sets
    /// `is_window_root_type() == true` is [WindowWidget](super::window::WindowWidget).
    fn is_window_root_type() -> bool
    where
        Self: Sized,
    {
        false
    }

    /// Creates a new widget. The `common` argument provides all available information about the context in which
    /// the widget is being created.
    ///
    /// You don't need to call this function directly. It's automatically invoked when you create a widget using
    /// one of the following functions on `WidgetCommon` of the parent widget:
    /// - [add_child](crate::widgets::WidgetCommon::add_child)
    /// - [add_child_with_key](crate::widgets::WidgetCommon::add_child_with_key)
    /// - [declare_child](crate::widgets::WidgetCommon::declare_child)
    /// - [declare_child_with_key](crate::widgets::WidgetCommon::declare_child_with_key)
    ///
    /// When implementing this function, you should always store the `common` argument value inside your widget object.
    /// As a convention, you should store it in the widget's field named `common`.
    /// Your implementations of [common](Self::common) and [common_mut](Self::common_mut) must return a reference to that object.
    fn new(common: WidgetCommonTyped<Self>) -> Self
    where
        Self: Sized;

    /// Returns a non-unique, read-only reference to `WidgetCommon` object stored inside the widget.
    /// It's recommended to use [impl_widget_common!](crate::impl_widget_common) macro
    /// to automatically implement this function.
    fn common(&self) -> &WidgetCommon; // TODO: example+test for custom location of common object

    /// Returns a unique, read-write reference to `WidgetCommon` object stored inside the widget.
    /// It's recommended to use [impl_widget_common!](crate::impl_widget_common) macro
    /// to automatically implement this function.
    fn common_mut(&mut self) -> &mut WidgetCommon;

    /// Handles a draw event.
    ///
    /// You should not call this function directly.
    /// Call [WidgetCommon::update](crate::widgets::WidgetCommon::update) to request
    /// a repaint of a widget.
    ///
    /// Implement this function to perform custom drawing in your widget.
    /// You don't need to implement it if your widget doesn't need to draw.
    /// For example, many container types don't draw anything themselves and do not implement `handle_draw`.
    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }

    /// Handles a press or release of a mouse button.
    ///
    /// Widgets receive a mouse event when it occurs within their boundaries, unless it's intercepted
    /// by a sibling or a child widget.
    /// Note that child widgets receive mouse events before their parent.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to handle events from mouse buttons in your widget.
    ///
    /// If `false` is returned, the event will be propagated to overlapping sibling widgets (if any)
    /// and then to the parent widget.
    ///
    /// If `true` is returned, the event will be consumed. Additionally, if it's a `ElementState::Press`
    /// event, the widget becomes a *mouse grabber*, i.e. it will continue receiving all mouse events,
    /// even those outside its boundaries, until all mouse buttons have been released.
    /// Mouse grabber will not receive a mouse leave event even if the mouse pointer leaves its boundary.
    /// Instead, the mouse leave event will be delivered when all mouse buttons are released.
    ///
    /// Default implementation returns `false`, i.e. it doesn't consume the event.
    fn handle_mouse_input(&mut self, event: MouseInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles a mouse scroll wheel movement or a touchpad scroll gesture.
    ///
    /// Widgets receive a mouse event when it occurs within their boundaries, unless it's intercepted
    /// by a sibling or a child widget.
    /// Note that child widgets receive mouse events before their parent.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to handle scroll events in your widget.
    ///
    /// If `false` is returned, the event will be propagated to overlapping sibling widgets (if any)
    /// and then to the parent widget.
    ///
    /// If `true` is returned, the event will be consumed.
    ///
    /// Default implementation returns `false`, i.e. it doesn't consume the event.
    fn handle_mouse_scroll(&mut self, event: MouseScrollEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles a mouse enter event.
    ///
    /// This event is triggered when a mouse move event occurs for the first time
    /// (since widget creation or since last mouse leave event for that widget)
    /// within the widget's boundary that is not consumed by any of its child widgets.
    /// The mouse enter event is always delivered before the mouse move event that caused it.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to detect when the mouse moves over the widget.
    ///
    /// If `false` is returned, the event will be discarded.
    ///
    /// If `true` is returned, the event will be consumed, and the `is_mouse_over`
    /// status of the widget will change to `true`.
    ///
    /// Default implementation returns `false`, i.e. it doesn't consume the event.
    fn handle_mouse_enter(&mut self, event: MouseEnterEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles a mouse move event.
    ///
    /// Widgets receive a mouse event when it occurs within their boundaries, unless it's intercepted
    /// by a sibling or a child widget.
    /// Note that child widgets receive mouse events before their parent.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to handle mouse move events in your widget.
    ///
    /// If `false` is returned, the event will be propagated to overlapping sibling widgets (if any)
    /// and then to the parent widget.
    ///
    /// If `true` is returned, the event will be consumed. Additionally,
    /// `is_mouse_over` status of the widget will change to `true` if it was `false`.
    ///
    /// Default implementation returns `false`, i.e. it doesn't consume the event.
    fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles a mouse leave event.
    ///
    /// This event is triggered when the mouse pointer leaves the widget's boundary or when it leaves
    /// the OS window's boundary.
    /// In the widget is the current *mouse grabber*, the mouse leave event will only be
    /// delivered after it stops being the mouse grabber (i.e. when all mouse buttons are released).
    /// When this event is delivered, the `is_mouse_over` status of the widget will change to `false` if it was `true`.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to detect when the mouse is no longer over the widget.
    fn handle_mouse_leave(&mut self, event: MouseLeaveEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }

    /// Handles a press or release of a keyboard button.
    ///
    /// Only the currently focused widget receives keyboard events. Note that the widget can only become focused
    /// if it is [focusable](crate::widgets::WidgetCommon::set_focusable).
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to handle user input. Note that you should make your event focusable
    /// by calling [set_focusable](crate::widgets::WidgetCommon::set_focusable).
    /// If your widget handles text input (as opposed to e.g. hotkeys), you should also
    /// [enable input method editor](crate::widgets::WidgetCommon::set_ime_enabled) and implement
    /// [handle_input_method](Self::handle_input_method).
    fn handle_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles an event from the [input method](https://en.wikipedia.org/wiki/Input_method) (IME) provided by the OS.
    ///
    /// Only the currently focused widget receives input method events.
    /// Note that the widget can only become focused
    /// if it is [focusable](crate::widgets::WidgetCommon::set_focusable).
    /// The input method will be enabled for the window only when the currently focused widget has
    /// [set_ime_enabled(true)](crate::widgets::WidgetCommon::set_ime_enabled).
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to handle IME user input. Note that you should make your event focusable
    /// by calling [set_focusable](crate::widgets::WidgetCommon::set_focusable).
    /// See also [handle_keyboard_input](Self::handle_keyboard_input) for handling keyboard input
    /// in absence of input method.
    fn handle_input_method(&mut self, event: InputMethodEvent) -> Result<bool> {
        let _ = event;
        Ok(false)
    }

    /// Handles a layout event.
    ///
    /// Layout events are used to update position of widgets when the widget tree changes or when the OS window is resized.
    /// Generally speaking, layout events are handled by the parent widget first, and children receive their own layout events
    /// during or immediately after the parent's `handle_layout_event` execution.
    ///
    /// Layout events may originate from the root widget (e.g. when the OS window
    /// is resized or when a size hint of any widget changes), but may also originate from another widget if that widget
    /// is repositioned or resized.
    ///
    /// The widget receives a layout event when its geometry changes (i.e. its position within the window, its size,
    /// or its visible area changes), and also when size hint is changed for this widget or any of its direct and indirect children.
    ///
    /// You should not call this function directly. To trigger a re-layout, call
    /// [size_hint_changed](crate::widgets::WidgetCommon::size_hint_changed).
    ///
    /// Implement this function to achieve a custom positioning of this widget's children that is not achievable
    /// through the default grid layout, or if you want to perform an action when the geometry of your widget changes
    /// (i.e. its position within the window, its size, or its visible area changes).
    ///
    /// In this function, use [set_geometry](crate::widgets::WidgetExt::set_geometry)
    /// to position the direct children of this widget. Note that this will immediately trigger a layout event
    /// for the child widget if the conditions listed above are met.
    ///
    /// The default implementation calls [`grid_layout`](grid_layout)`(self, &event.changed_size_hints)`.
    /// This default layout logic will re-position all direct children that are not explicitly excluded from the grid.
    /// If you want to retain this behavior and do something extra, you can call `grid_layout` from
    /// your `handle_layout` implementation.
    ///
    /// You don't need to implement this function if the default grid layout is sufficient.
    /// There is also no need to implement it for widgets that do not have any children. For those widgets
    /// it's sufficient to implement [handle_size_hint_x_request](Self::handle_size_hint_x_request) and
    /// [handle_size_hint_y_request](Self::handle_size_hint_y_request).
    fn handle_layout(&mut self, event: LayoutEvent) -> Result<()> {
        grid_layout(self, &event.changed_size_hints);
        Ok(())
    }

    /// Handles a focus-in event.
    ///
    /// This event triggers when the widget gains focus. Only focusable widgets can receive this event.
    ///
    /// Focus can be changed in a variety of ways. It can happen on response to a keyboard or mouse input.
    /// Focus change can also be programmatically requested by any widget.
    /// If the window doesn't have a focused widget (e.g. immediately after the window is created or
    /// when the currently focused widget is disabled or deleted), the first focusable widget within the window
    /// will be automatically focused.
    ///
    /// Note that widget focus doesn't interact with OS window focus. When OS window gains or loses focus,
    /// the focused widget within that window is still considered focused.
    /// Use [handle_window_focus_change](Self::handle_window_focus_change) to track window focus.
    ///
    /// You should not call this function directly. Use [send_window_request](crate::system::send_window_request)
    /// to request focus for a widget.
    ///
    /// Implement this function to perform a custom action when your widget gains focus or if you're interested
    /// in the [reason](FocusReason) why it gained focus.
    ///
    /// Note that if you're only interested in the current state of
    /// the focus and do not need to perform custom actions when it changes,
    /// you can use [is_focused](crate::widgets::WidgetCommon::is_focused) instead. The widget is always updated
    /// (including a redraw) when it gains or loses focus.
    fn handle_focus_in(&mut self, event: FocusInEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }

    /// Handles a focus-out event.
    ///
    /// This event triggers when the widget loses focus. Only focusable widgets can receive this event.
    ///
    /// Focus can be changed in a variety of ways. It can happen on response to a keyboard or mouse input.
    /// Focus change can also be programmatically requested by any widget.
    /// If the window doesn't have a focused widget (e.g. immediately after the window is created or
    /// when the currently focused widget is disabled or deleted), the first focusable widget within the window
    /// will be automatically focused.
    ///
    /// Note that widget focus doesn't interact with OS window focus. When OS window gains or loses focus,
    /// the focused widget within that window is still considered focused.
    /// Use [handle_window_focus_change](Self::handle_window_focus_change) to track window focus.
    ///
    /// You should not call this function directly. Use [send_window_request](crate::system::send_window_request)
    /// to request focus for a widget.
    ///
    /// Implement this function to perform a custom action when your widget loses focus.
    ///
    /// Note that if you're only interested in the current state of
    /// the focus and do not need to perform custom actions when it changes,
    /// you can use [is_focused](crate::widgets::WidgetCommon::is_focused) instead. The widget is always updated
    /// (including a redraw) when it gains or loses focus.
    fn handle_focus_out(&mut self, event: FocusOutEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }

    // TODO: doc: recommended way to request focus for a window?

    /// Handles focus change of the OS window.
    ///
    /// This event triggers when the OS window that contains this widget gains or loses focus. This can happen
    /// when the window is minimized or hidden, when another OS window is clicked, by a keyboard shortcut
    /// or by other means offered by the OS.
    ///
    /// Note that widget focus doesn't interact with OS window focus. When OS window gains or loses focus,
    /// the focused widget within that window is still considered focused.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function to perform a custom action when your widget loses focus.
    ///
    /// Note that if you're only interested in the current state of
    /// the focus and do not need to perform custom actions when it changes,
    /// you can use [Window::is_focused](crate::window::Window::is_focused) on `self.common.window()` instead.
    /// The widget is always updated
    /// (including a redraw) when it gains or loses focus.
    fn handle_window_focus_change(&mut self, event: WindowFocusChangeEvent) -> Result<()> {
        let _ = event;
        // TODO: optimize: only deliver to widgets that requested it
        Ok(())
    }

    /// Handles an accessibility action.
    ///
    /// This event can be triggered by screen readers and other assistive technologies.
    /// It can only be triggered for widgets that implement [handle_accessible_node_request](Self::handle_accessible_node_request)
    /// to return an accessibility node that supports actions.
    ///
    /// You should not call this function directly.
    ///
    /// Implement this function if you're implementing a custom widget that handles user input
    /// (e.g. keyboard or mouse input). You should also implement
    /// [handle_accessible_node_request](Self::handle_accessible_node_request) accordingly to be able to
    /// receive these events.
    ///
    /// You don't need to implement this function if your widget is non-interactive. You also don't need to implement it
    /// if you're only composing or wrapping existing widgets and your widget only relies on the
    /// interactivity provided by those widgets.
    fn handle_accessibility_action(&mut self, event: AccessibilityActionEvent) -> Result<()> {
        warn!("unhandled event: {event:?}");
        let _ = event;
        Ok(())
    }

    /// Handles a style change.
    ///
    /// This event is triggered when a widget's style is changed using [set_style](crate::widgets::WidgetExt::set_style),
    /// when its classes or pseudoclasses change (e.g. when it's disabled/enabled, mouse hovered over, gained/lost focus),
    /// or when a parent's style is changed.
    ///
    /// This event is handled by the parent widget first, then propagates to all affected children.
    ///
    /// You should not call this function directly. You can manipulate the widget's style using
    /// [set_style](crate::widgets::WidgetExt::set_style), [add_class](crate::widgets::WidgetExt::add_class)
    /// and [remove_class](crate::widgets::WidgetExt::remove_class).
    ///
    /// Implement this function if your widget needs to react to a style change. Possible examples include
    /// updating cached margins or regenerating pixmaps that depend on style. You don't need to implement it
    /// if your implementation doesn't cache anything and always fetches the style data using
    /// [style](crate::widgets::WidgetCommon::style) and [common_style](crate::widgets::WidgetCommon::common_style).
    /// The widget is always updated (including a redraw) when its style changes.
    fn handle_style_change(&mut self, event: StyleChangeEvent) -> Result<()> {
        let _ = event;
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<bool> {
        match event {
            Event::MouseInput(e) => self.handle_mouse_input(e),
            Event::MouseScroll(e) => self.handle_mouse_scroll(e),
            Event::MouseEnter(e) => self.handle_mouse_enter(e),
            Event::MouseMove(e) => self.handle_mouse_move(e),
            Event::MouseLeave(e) => self.handle_mouse_leave(e).map(|()| true),
            Event::KeyboardInput(e) => self.handle_keyboard_input(e),
            Event::InputMethod(e) => self.handle_input_method(e),
            Event::Draw(e) => self.handle_draw(e).map(|()| true),
            Event::Layout(e) => self.handle_layout(e).map(|()| true),
            Event::FocusIn(e) => self.handle_focus_in(e).map(|()| true),
            Event::FocusOut(e) => self.handle_focus_out(e).map(|()| true),
            Event::WindowFocusChange(e) => self.handle_window_focus_change(e).map(|()| true),
            Event::AccessibilityAction(e) => self.handle_accessibility_action(e).map(|()| true),
            Event::StyleChange(e) => self.handle_style_change(e).map(|()| true),
        }
    }

    /// Updates state of the widget's children.
    ///
    /// This function allows the widget to update its children in a declarative way.
    /// It's called after the widget's update has been requested by calling
    /// [WidgetCommon::update](crate::widgets::WidgetCommon::update) or after
    /// a relevant built-in property has been changed (e.g. focus state, enabled state, or widget style).
    ///
    /// You should not call this function directly. Use
    /// [WidgetCommon::update](crate::widgets::WidgetCommon::update) to schedule an update of the widget.
    ///
    /// Implement this function to update the widget's children in a declarative way.
    /// Inside this implementation, you can use
    /// [declare_child](crate::widgets::WidgetCommon::declare_child) and
    /// [declare_child_with_key](crate::widgets::WidgetCommon::declare_child_with_key)
    /// functions to declare the children (note: these functions should only be used from within
    /// a `handle_declare_children_request` call). Both direct and indirect children can be declared this way.
    ///
    /// When you call `declare_*` functions, it can either return a reference to an existing child or
    /// create a new child and return a reference to it.
    /// Use the returned references to update **all** properties of the
    /// child that may change.
    ///
    /// After `handle_declare_children_request` has finished, any previously declared children that
    /// were not declared during that call will be deleted. This means that in you want a child widget
    /// to keep existing, you need to declare it in *every* call to `handle_declare_children_request`.
    ///
    /// Implementing this function is the easiest and the most convenient way to manage the content of your widget.
    /// However, it entails a performance cost of iterating over all the children you want to declare and
    /// recalculating all values for their dynamic properties. There is an alternative way of dealing with this task.
    /// You can explicitly create children using [add_child](crate::widgets::WidgetCommon::add_child) and
    /// [add_child_with_key](crate::widgets::WidgetCommon::add_child_with_key),
    /// get a reference to an existing child with [child](crate::widgets::WidgetCommon::child) and
    /// [child_mut](crate::widgets::WidgetCommon::child_mut), and explicitly remove children with
    /// [remove_child](crate::widgets::WidgetCommon::remove_child). These functions can be called at any time,
    /// from any function within your widget (or even from outside), so they don't have such restrictions as
    /// `declare_*` functions have. This approach can be more error-prone, but it can also be much more
    /// efficient if your widget has a lot of children or some properties are expensive to calculate.
    /// (Note however that if you need to present many objects in your UI, you should use *virtual rendering* instead of
    /// creating all the widgets at once.)
    ///
    /// You don't need to implement this function if your widget doesn't have any children or if you're
    /// managing its children explicitly.
    fn handle_declare_children_request(&mut self) -> Result<()> {
        self.common_mut().has_declare_children_override = false;
        Ok(())
    }

    /// Calculates size hint of this widget along the X axis.
    ///
    /// This function is typically called after widget creation and after
    /// [size_hint_changed](crate::widgets::WidgetCommon::size_hint_changed) has been called for this widget.
    /// The value is subsequently cached until `size_hint_changed` is called again.
    ///
    /// You should not call this function directly. Use
    /// [size_hint_x](crate::widgets::WidgetExt::size_hint_x) to get the current size hint
    /// (it can trigger a call to `handle_size_hint_x_request` internally if it hasn't been cached yet).
    ///
    /// Default implementation returns the size hint provided by the default grid layout.
    ///
    /// Implement this function if your widget uses a custom layout to position its children; if
    /// your widget doesn't have any children but needs to have non-zero size; or if you want it
    /// to be sized differently from what the default grid layout offers.
    ///
    /// Note that [set_layout_item_options](crate::widgets::WidgetCommon::set_layout_item_options)
    /// offers many options that alter the size of the widget, which in many cases is sufficient,
    /// so reimplementing size hint methods may not be necessary.
    fn handle_size_hint_x_request(&mut self) -> Result<SizeHints> {
        let options = self.common().common_style.grid.clone();
        Ok(grid::size_hint_x(&mut self.common_mut().children, &options))
    }

    /// Calculates size hint of this widget along the Y axis, given the X size.
    ///
    /// This function is typically called after widget creation and after
    /// [size_hint_changed](crate::widgets::WidgetCommon::size_hint_changed) has been called for this widget.
    /// The value is subsequently cached (separately for each `size_x`) until `size_hint_changed` is called again.
    ///
    /// Note that this function is typically called during layout calculation, and the value of `size_x`
    /// may not be the same as the final value chosen by the layout. Implement `handle_layout` if you want
    /// to be notified about the final size assigned to your widget.
    ///
    /// You should not call this function directly. Use
    /// [size_hint_y](crate::widgets::WidgetExt::size_hint_y) to get the current size hint
    /// (it can trigger a call to `handle_size_hint_y_request` internally if it hasn't been cached yet).
    ///
    /// Default implementation returns the size hint provided by the default grid layout.
    ///
    /// Implement this function if your widget uses a custom layout to position its children; if
    /// your widget doesn't have any children but needs to have non-zero size; or if you want it
    /// to be sized differently from what the default grid layout offers.
    ///
    /// Note that [set_layout_item_options](crate::widgets::WidgetCommon::set_layout_item_options)
    /// offers many options that alter the size of the widget, which in many cases is sufficient,
    /// so reimplementing size hint methods may not be necessary.
    fn handle_size_hint_y_request(&mut self, size_x: PhysicalPixels) -> Result<SizeHints> {
        let options = self.common().common_style.grid.clone();
        Ok(grid::size_hint_y(
            &mut self.common_mut().children,
            &options,
            size_x,
        ))
    }

    // TODO: track accesskit state and don't update nodes if it's disabled

    /// Calculates the accessibility node representing this widget.
    ///
    /// You should not call this function directly.
    /// Call [WidgetCommon::update](crate::widgets::WidgetCommon::update) to request
    /// an update of a widget. Note that `handle_accessibility_node_request` may not be called if
    /// no assistive technologies are enabled in the OS.
    ///
    /// Implement this function if your widget displays something or interacts with the user.
    /// If your accessibility node can receive any actions, you should also implement
    /// [handle_accessibility_action](Self::handle_accessibility_action) to handle those actions.
    ///
    /// Note that you don't need to set the node's bounds using `set_bounds`. This data will be
    /// filled automatically.
    ///
    /// You don't need to implement this function if your widget is non-interactive. You also don't need to implement it
    /// if you're only composing or wrapping existing widgets and your widget only relies on the
    /// interactivity provided by those widgets.
    fn handle_accessibility_node_request(&mut self) -> Result<Option<accesskit::Node>> {
        Ok(None)
    }

    /// Handles a request that asks for a certain area of a certain widget become visible.
    ///
    /// This request can be triggered by any widget. For example, it can be triggered when a text cursor
    /// is moved into an invisible area or an invisible cell of a table is selected.
    ///
    /// Any widget is always contained within the boundary of its parent widget. As such, the request
    /// is delivered to all the parents of the widget that requested it, starting from the root widget.
    ///
    /// You should not call this function directly. To trigger it,
    /// use [send_window_request](crate::system::send_window_request).
    ///
    /// Implement this function in a rare case that your widget implements a container with a custom layout which may
    /// position children outside of its boundary. You don't need to implement this function if you're
    /// simply composing existing widgets that implement scrolling (e.g. `ScrollArea` or `TextInput`).
    fn handle_scroll_to_rect_request(&mut self, request: ScrollToRectRequest) -> Result<bool> {
        // TODO: remove return value and always propagate to all parents?
        // (in case there are multiple nested scroll areas)
        let _ = request;
        Ok(false)
    }
}

impl dyn Widget {
    /// Returns `true` if the widget has type `T`.
    pub fn is<T: Widget>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }

    /// Returns a reference to the widget if it is of type `T`, or
    /// `None` if it isn't.
    pub fn downcast_ref<T: Widget>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }

    /// Returns a mutable reference to the widget if it is of type `T`, or
    /// `None` if it isn't.
    pub fn downcast_mut<T: Widget>(&mut self) -> Option<&mut T> {
        (self as &mut dyn Any).downcast_mut()
    }
}
