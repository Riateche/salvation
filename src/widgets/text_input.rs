use std::{
    cmp::{max, min},
    fmt::Display,
    time::Duration,
};

use accesskit::{ActionData, DefaultActionVerb, NodeBuilder, NodeId, Role};
use anyhow::Result;
use cosmic_text::{Action, Attrs, Wrap};
use log::warn;
use winit::{
    event::{ElementState, Ime, MouseButton},
    keyboard::Key,
    window::CursorIcon,
};

use crate::{
    accessible,
    draw::DrawEvent,
    event::{
        AccessibleActionEvent, FocusInEvent, FocusOutEvent, FocusReason, ImeEvent,
        KeyboardInputEvent, LayoutEvent, MountEvent, MouseInputEvent, MouseMoveEvent, UnmountEvent,
        WidgetScopeChangeEvent, WindowFocusChangeEvent,
    },
    layout::SizeHint,
    shortcut::standard_shortcuts,
    style::text_input::{ComputedVariantStyle, TextInputState},
    system::{add_interval, report_error, send_window_request, with_system, ReportError},
    text_editor::TextEditor,
    timer::TimerId,
    types::{Point, Rect, Size},
    window::SetFocusRequest,
};

use super::{Widget, WidgetCommon, WidgetExt};

pub struct TextInput {
    editor: TextEditor,
    editor_viewport_rect: Rect,
    scroll_x: i32,
    common: WidgetCommon,
    blink_timer: Option<TimerId>,
    selected_text: String,
    accessible_line_id: NodeId,
}

// TODO: get system setting
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

fn sanitize(text: &str) -> String {
    text.replace('\n', " ")
}

impl TextInput {
    pub fn new(text: impl Display) -> Self {
        let mut common = WidgetCommon::new();
        common.is_focusable = true;
        common.enable_ime = true;
        common.cursor_icon = CursorIcon::Text;
        let mut editor = TextEditor::new(&sanitize(&text.to_string()));
        editor.set_wrap(Wrap::None);
        Self {
            editor,
            common,
            editor_viewport_rect: Rect::default(),
            scroll_x: 0,
            blink_timer: None,
            selected_text: String::new(),
            accessible_line_id: accessible::new_id(),
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        // TODO: replace line breaks to avoid multiple lines in buffer
        self.editor
            .set_text(&sanitize(&text.to_string()), Attrs::new());
        self.after_change();
        self.reset_blink_timer();
        self.common.update();
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
    ))]
    fn copy_selection(&self) {
        use arboard::{LinuxClipboardKind, SetExtLinux};

        if !self.selected_text.is_empty() {
            with_system(|system| {
                system
                    .clipboard
                    .set()
                    .clipboard(LinuxClipboardKind::Primary)
                    .text(&self.selected_text)
            })
            .or_report_err();
        }
    }

    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
    ))]
    fn paste_selection(&mut self) {
        use arboard::{GetExtLinux, LinuxClipboardKind};

        if self.editor.is_mouse_interaction_forbidden() {
            return;
        }
        let text = with_system(|system| {
            system
                .clipboard
                .get()
                .clipboard(LinuxClipboardKind::Primary)
                .text()
        })
        .or_report_err();
        if let Some(text) = text {
            self.editor.insert_string(&sanitize(&text), None);
            self.common.update();
        }
    }

    fn after_change(&mut self) {
        self.adjust_scroll();
        let new_selected_text = self.editor.selected_text().unwrap_or_default();
        if new_selected_text != self.selected_text {
            self.selected_text = new_selected_text;
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "android", target_os = "emscripten"))
            ))]
            self.copy_selection();
        }
    }

    fn adjust_scroll(&mut self) {
        let max_scroll = max(0, self.editor.size().x - self.editor_viewport_rect.size.x);
        if let Some(cursor_position) = self.editor.cursor_position() {
            let cursor_x_in_viewport = cursor_position.x - self.scroll_x;
            if cursor_x_in_viewport < 0 {
                self.scroll_x -= -cursor_x_in_viewport;
            } else if cursor_x_in_viewport > self.editor_viewport_rect.size.x - 1 {
                self.scroll_x += cursor_x_in_viewport - (self.editor_viewport_rect.size.x - 1);
            }
        }
        let new_scroll = self.scroll_x.clamp(0, max_scroll);
        if self.scroll_x != new_scroll {
            self.scroll_x = new_scroll;
            self.common.update();
        }
    }

    fn reset_blink_timer(&mut self) {
        if let Some(id) = self.blink_timer.take() {
            id.cancel();
        }
        let focused = self.common.is_focused();
        self.editor.set_cursor_hidden(!focused);
        if focused {
            let id = add_interval(
                CURSOR_BLINK_INTERVAL,
                self.callback(|this, _| this.toggle_cursor_hidden()),
            );
            self.blink_timer = Some(id);
        }
        self.common.update();
    }

    fn toggle_cursor_hidden(&mut self) -> Result<()> {
        self.editor
            .set_cursor_hidden(!self.editor.is_cursor_hidden());
        if !self.editor.has_selection() {
            self.common.update();
        }
        Ok(())
    }

    fn copy_to_clipboard(&mut self) {
        if let Some(text) = self.editor.selected_text() {
            with_system(|system| system.clipboard.set_text(text)).or_report_err();
        }
    }

    fn handle_main_click(&mut self, event: MouseInputEvent) -> Result<()> {
        let mount_point = self.common.mount_point_or_err()?;

        if !self.common.is_focused {
            send_window_request(
                mount_point.address.window_id,
                SetFocusRequest {
                    widget_id: self.common.id,
                    reason: FocusReason::Mouse,
                },
            );
        }
        self.editor.on_mouse_input(
            event.pos() - self.editor_viewport_rect.top_left + Point::new(self.scroll_x, 0),
            event.num_clicks(),
            mount_point.window.0.borrow().modifiers_state.shift_key(),
        );
        self.common.update();
        Ok(())
    }

    fn current_variant_style(&self) -> &ComputedVariantStyle {
        let state = if self.common.is_enabled() {
            TextInputState::Enabled {
                focused: self.common.is_focused(),
                mouse_over: self.common.is_mouse_over,
            }
        } else {
            TextInputState::Disabled
        };
        self.common.style().text_input.variants.get(&state).unwrap()
    }

    fn style_changed(&mut self) {
        let style = &self.common.style().text_input;
        self.editor.set_font_metrics(style.font_metrics);
        let style = self.current_variant_style().clone();
        // TODO: support color changes based on state
        self.editor.set_text_color(style.text_color);
        self.editor
            .set_selected_text_color(style.selected_text_color);
        self.editor
            .set_selected_text_background(style.selected_text_background);
        self.update_viewport_rect();
        self.common.update();
    }

    fn update_viewport_rect(&mut self) {
        let style = &self.common.style().text_input;
        if let Some(rect_in_window) = self.common.rect_in_window {
            let offset_y = max(0, rect_in_window.size.y - self.editor.size().y) / 2;
            self.editor_viewport_rect = Rect {
                top_left: Point {
                    x: style.padding_with_border.x,
                    y: offset_y,
                },
                size: Size {
                    x: max(0, rect_in_window.size.x - 2 * style.padding_with_border.x),
                    y: min(rect_in_window.size.y, self.editor.size().y),
                },
            };
            self.adjust_scroll();
            self.reset_blink_timer();
        }
    }
}

impl Widget for TextInput {
    fn handle_layout(&mut self, _event: LayoutEvent) -> Result<()> {
        self.update_viewport_rect();
        Ok(())
    }

    fn handle_widget_scope_change(&mut self, _event: WidgetScopeChangeEvent) -> Result<()> {
        self.style_changed();
        Ok(())
    }

    fn handle_draw(&mut self, event: DrawEvent) -> Result<()> {
        let rect_in_window = self.common.rect_in_window_or_err()?;
        let mount_point = self.common.mount_point_or_err()?;
        let is_focused = self.common.is_focused();
        let style = self.current_variant_style().clone();

        // TODO: stroke and fill instead
        event.stroke_rounded_rect(
            Rect {
                top_left: Point::default(),
                size: rect_in_window.size,
            },
            style.border.radius.get() as f32,
            style.border.color,
            style.border.width.get() as f32,
        );

        let mut target_rect = self.editor_viewport_rect;
        target_rect.size.x = min(target_rect.size.x, self.editor.size().x);

        let scroll = Point::new(self.scroll_x, 0);
        event.draw_subpixmap(target_rect, self.editor.pixmap().as_ref(), scroll);
        if is_focused {
            if let Some(editor_cursor) = self.editor.cursor_position() {
                // We specify an area below the input because on Windows
                // the IME window obscures the specified area.
                let top_left = rect_in_window.top_left + self.editor_viewport_rect.top_left
                    - scroll
                    + editor_cursor
                    + Point {
                        x: 0,
                        y: self.editor.line_height().ceil() as i32,
                    };
                let size = rect_in_window.size; // TODO: self.editor_viewport_rect.size
                mount_point
                    .window
                    .set_ime_cursor_area(Rect { top_left, size });
            }
        }
        Ok(())
    }

    fn handle_mouse_input(&mut self, event: MouseInputEvent) -> Result<bool> {
        if event.state() == ElementState::Pressed {
            match event.button() {
                MouseButton::Left => {
                    self.handle_main_click(event)?;
                }
                MouseButton::Right => {
                    // TODO: context menu
                }
                MouseButton::Middle => {
                    #[cfg(all(
                        unix,
                        not(any(
                            target_os = "macos",
                            target_os = "android",
                            target_os = "emscripten"
                        ))
                    ))]
                    {
                        self.handle_main_click(event)?;
                        self.paste_selection();
                    }
                }
                _ => {}
            }
        }
        let is_released = self
            .common
            .mount_point
            .as_ref()
            .map_or(false, |mount_point| {
                mount_point
                    .window
                    .0
                    .borrow()
                    .pressed_mouse_buttons
                    .is_empty()
            });
        if is_released {
            self.editor.mouse_released();
        }
        self.after_change();
        self.reset_blink_timer();
        Ok(true)
    }

    fn handle_mouse_move(&mut self, event: MouseMoveEvent) -> Result<bool> {
        let mount_point = self.common.mount_point_or_err()?;
        if mount_point
            .window
            .0
            .borrow()
            .pressed_mouse_buttons
            .contains(&MouseButton::Left)
        {
            let pos = event.pos - self.editor_viewport_rect.top_left + Point::new(self.scroll_x, 0);
            let old_selection = (self.editor.select_opt(), self.editor.cursor());
            self.editor
                .action(Action::Drag { x: pos.x, y: pos.y }, true);
            let new_selection = (self.editor.select_opt(), self.editor.cursor());
            if old_selection != new_selection {
                self.after_change();
                self.common.update();
            }
        }
        Ok(true)
    }

    #[allow(clippy::if_same_then_else)]
    fn handle_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        if event.event.state == ElementState::Released {
            return Ok(true);
        }

        let shortcuts = standard_shortcuts();
        if shortcuts.move_to_next_char.matches(&event) {
            self.editor.action(Action::Next, false);
        } else if shortcuts.move_to_previous_char.matches(&event) {
            self.editor.action(Action::Previous, false);
        } else if shortcuts.delete.matches(&event) {
            self.editor.action(Action::Delete, false);
        } else if shortcuts.backspace.matches(&event) {
            self.editor.action(Action::Backspace, false);
        } else if shortcuts.cut.matches(&event) {
            self.copy_to_clipboard();
            self.editor.action(Action::Delete, false);
        } else if shortcuts.copy.matches(&event) {
            self.copy_to_clipboard();
        } else if shortcuts.paste.matches(&event) {
            let r = with_system(|system| system.clipboard.get_text());
            match r {
                Ok(text) => self.editor.insert_string(&sanitize(&text), None),
                Err(err) => report_error(err),
            }
        } else if shortcuts.undo.matches(&event) {
            // TODO
        } else if shortcuts.redo.matches(&event) {
            // TODO
        } else if shortcuts.select_all.matches(&event) {
            self.editor.action(Action::SelectAll, false);
        } else if shortcuts.deselect.matches(&event) {
            // TODO: why Escape?
            self.editor.action(Action::Escape, false);
        } else if shortcuts.move_to_next_word.matches(&event) {
            self.editor.action(Action::NextWord, false);
        } else if shortcuts.move_to_previous_word.matches(&event) {
            self.editor.action(Action::PreviousWord, false);
        } else if shortcuts.move_to_start_of_line.matches(&event) {
            self.editor.action(Action::Home, false);
        } else if shortcuts.move_to_end_of_line.matches(&event) {
            self.editor.action(Action::End, false);
        } else if shortcuts.select_next_char.matches(&event) {
            self.editor.action(Action::Next, true);
        } else if shortcuts.select_previous_char.matches(&event) {
            self.editor.action(Action::Previous, true);
        } else if shortcuts.select_next_word.matches(&event) {
            self.editor.action(Action::NextWord, true);
        } else if shortcuts.select_previous_word.matches(&event) {
            self.editor.action(Action::PreviousWord, true);
        } else if shortcuts.select_start_of_line.matches(&event) {
            self.editor.action(Action::Home, true);
        } else if shortcuts.select_end_of_line.matches(&event) {
            self.editor.action(Action::End, true);
        } else if shortcuts.delete_start_of_word.matches(&event) {
            self.editor.action(Action::DeleteStartOfWord, false);
        } else if shortcuts.delete_end_of_word.matches(&event) {
            self.editor.action(Action::DeleteEndOfWord, false);
        } else if let Some(text) = event.event.text {
            if [Key::Tab, Key::Enter, Key::Escape].contains(&event.event.logical_key) {
                return Ok(false);
            }
            self.editor.insert_string(&sanitize(&text), None);
        } else {
            return Ok(false);
        }
        self.after_change();
        self.common.update();
        self.reset_blink_timer();
        Ok(true)
    }

    fn handle_ime(&mut self, event: ImeEvent) -> Result<bool> {
        match event.0.clone() {
            Ime::Enabled => {}
            Ime::Preedit(preedit, cursor) => {
                // TODO: can pretext have line breaks?
                self.editor.action(
                    Action::SetPreedit {
                        preedit: sanitize(&preedit),
                        cursor,
                        attrs: None,
                    },
                    false,
                );
            }
            Ime::Commit(string) => {
                self.editor.insert_string(&sanitize(&string), None);
            }
            Ime::Disabled => {}
        }
        self.after_change();
        self.common.update();
        self.reset_blink_timer();
        Ok(true)
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn handle_mount(&mut self, event: MountEvent) -> Result<()> {
        self.editor.set_window(Some(event.0.window.clone()));
        self.reset_blink_timer();

        event.0.window.0.borrow_mut().accessible_nodes.mount(
            Some(self.common.id.into()),
            self.accessible_line_id,
            0,
        );
        //self.style_changed();
        Ok(())
    }
    fn handle_unmount(&mut self, _event: UnmountEvent) -> Result<()> {
        let mount_point = self.common.mount_point_or_err()?;
        mount_point
            .window
            .0
            .borrow_mut()
            .accessible_nodes
            .unmount(Some(self.common.id.into()), self.accessible_line_id);
        mount_point
            .window
            .0
            .borrow_mut()
            .accessible_nodes
            .update(self.accessible_line_id, None);
        self.editor.set_window(None);
        self.reset_blink_timer();
        Ok(())
    }
    fn handle_focus_in(&mut self, event: FocusInEvent) -> Result<()> {
        self.editor.on_focus_in(event.reason);
        self.common.update();
        self.reset_blink_timer();
        Ok(())
    }
    fn handle_focus_out(&mut self, _event: FocusOutEvent) -> Result<()> {
        self.editor.on_focus_out();
        self.common.update();
        self.reset_blink_timer();
        Ok(())
    }
    fn handle_window_focus_change(&mut self, event: WindowFocusChangeEvent) -> Result<()> {
        self.editor.on_window_focus_changed(event.focused);
        self.common.update();
        self.reset_blink_timer();
        Ok(())
    }
    fn handle_accessible_action(&mut self, event: AccessibleActionEvent) -> Result<()> {
        let mount_point = self.common.mount_point_or_err()?;

        match event.action {
            accesskit::Action::Default | accesskit::Action::Focus => {
                send_window_request(
                    mount_point.address.window_id,
                    SetFocusRequest {
                        widget_id: self.common.id,
                        // TODO: separate reason?
                        reason: FocusReason::Mouse,
                    },
                );
            }
            accesskit::Action::SetTextSelection => {
                let Some(ActionData::SetTextSelection(data)) = event.data else {
                    warn!("expected SetTextSelection in data, got {:?}", event.data);
                    return Ok(());
                };
                self.editor.set_accessible_selection(data);
                self.after_change();
                self.common.update();
                self.reset_blink_timer();
            }
            _ => {}
        }
        Ok(())
    }
    fn accessible_node(&mut self) -> Option<accesskit::NodeBuilder> {
        let Some(mount_point) = self.common.mount_point.as_ref() else {
            return None;
        };
        let mut line_node = NodeBuilder::new(Role::InlineTextBox);
        let mut line = self.editor.acccessible_line();
        for pos in &mut line.character_positions {
            *pos -= self.scroll_x as f32;
        }
        line_node.set_text_direction(line.text_direction);
        line_node.set_value(line.text);
        line_node.set_character_lengths(line.character_lengths);
        line_node.set_character_positions(line.character_positions);
        line_node.set_character_widths(line.character_widths);
        line_node.set_word_lengths(line.word_lengths);

        if let Some(rect_in_window) = self.common.rect_in_window {
            let rect = self.editor_viewport_rect.translate(rect_in_window.top_left);
            line_node.set_bounds(accesskit::Rect {
                x0: rect.top_left.x as f64,
                y0: rect.top_left.y as f64,
                x1: rect.bottom_right().x as f64,
                y1: rect.bottom_right().y as f64,
            });
        }

        mount_point
            .window
            .0
            .borrow_mut()
            .accessible_nodes
            .update(self.accessible_line_id, Some(line_node));

        let mut node = NodeBuilder::new(Role::TextInput);
        // TODO: use label
        node.set_name("some input");
        node.add_action(accesskit::Action::Focus);
        node.set_default_action_verb(DefaultActionVerb::Click);
        node.set_text_selection(self.editor.accessible_selection(self.accessible_line_id));
        Some(node)
    }

    fn size_hint_x(&mut self) -> Result<SizeHint> {
        let style = &self.common.style().text_input;
        Ok(SizeHint::new_expanding(style.min_width.get()))
    }

    fn size_hint_y(&mut self, _size_x: i32) -> Result<SizeHint> {
        let style = &self.common.style().text_input;
        let value = self.editor.size().y + 2 * style.padding_with_border.y;
        Ok(SizeHint::new_fixed(value))
    }
}
