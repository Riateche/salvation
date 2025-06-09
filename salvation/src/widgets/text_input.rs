use {
    super::{Widget, WidgetAddress, WidgetCommonTyped, WidgetExt, WidgetGeometry},
    crate::{
        event::{
            FocusInEvent, FocusOutEvent, InputMethodEvent, KeyboardInputEvent, LayoutEvent,
            ScrollToRectRequest, StyleChangeEvent,
        },
        impl_widget_common,
        layout::{grid::grid_layout, SizeHints},
        style::{get_style, text_input::TextInputStyle},
        text_editor::Text,
        types::{PhysicalPixels, Point, PpxSuffix, Rect},
    },
    anyhow::Result,
    cosmic_text::Attrs,
    log::warn,
    std::{cmp::max, fmt::Display, rc::Rc},
    winit::window::CursorIcon,
};

struct Viewport {
    common: WidgetCommonTyped<Self>,
}

impl Widget for Viewport {
    impl_widget_common!();

    fn new(common: WidgetCommonTyped<Self>) -> Self {
        Self { common }
    }

    fn handle_size_hint_x_request(&mut self) -> Result<crate::layout::SizeHints> {
        Ok(SizeHints {
            min: 0.ppx(),
            preferred: 0.ppx(),
            is_fixed: false,
        })
    }

    fn handle_size_hint_y_request(&mut self, _size_x: PhysicalPixels) -> Result<SizeHints> {
        let size = PhysicalPixels::from_i32(
            self.common.get_child::<Text>(0).unwrap().line_height() as i32,
        );
        Ok(SizeHints {
            min: size,
            preferred: size,
            is_fixed: true,
        })
    }
}

pub struct TextInput {
    common: WidgetCommonTyped<Self>,
    style: Rc<TextInputStyle>,
}

impl TextInput {
    fn text_widget(&self) -> &Text {
        self.common
            .get_dyn_child(0)
            .unwrap()
            .common()
            .get_child::<Text>(0)
            .unwrap()
    }

    fn text_widget_mut(&mut self) -> &mut Text {
        self.common
            .get_dyn_child_mut(0)
            .unwrap()
            .common_mut()
            .get_child_mut::<Text>(0)
            .unwrap()
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.text_widget_mut().set_text(text, Attrs::new());
    }

    fn adjust_scroll(&mut self, changed_size_hints: &[WidgetAddress]) {
        let Some(editor_viewport_rect) = self
            .common
            .children
            .get(&0.into())
            .unwrap()
            .common()
            .rect_in_parent()
        else {
            return;
        };
        let text_size = self.text_widget().size();
        let cursor_position = self.text_widget().cursor_position();
        let mut scroll_x = self
            .common
            .get_dyn_child(0)
            .unwrap()
            .common()
            .children
            .get(&0.into())
            .unwrap()
            .common()
            .rect_in_parent()
            .map_or(0.ppx(), |rect| -rect.left());
        let max_scroll = max(0.ppx(), text_size.x() - editor_viewport_rect.size_x());
        if let Some(cursor_position) = cursor_position {
            let cursor_x_in_viewport = cursor_position.x() - scroll_x;
            if cursor_x_in_viewport < 0.ppx() {
                scroll_x -= -cursor_x_in_viewport;
            } else if cursor_x_in_viewport > editor_viewport_rect.size_x() - 1.ppx() {
                scroll_x += cursor_x_in_viewport - (editor_viewport_rect.size_x() - 1.ppx());
            }
        }
        scroll_x = scroll_x.clamp(0.ppx(), max_scroll);
        let new_rect = Rect::from_pos_size(Point::new(-scroll_x, 0.ppx()), text_size);
        if self
            .common
            .get_dyn_child(0)
            .unwrap()
            .common()
            .children
            .get(&0.into())
            .unwrap()
            .common()
            .rect_in_parent()
            != Some(new_rect)
        {
            let Some(geometry) = self
                .common
                .get_dyn_child(0)
                .unwrap()
                .common()
                .geometry
                .clone()
            else {
                return;
            };

            self.common
                .get_dyn_child_mut(0)
                .unwrap()
                .common_mut()
                .children
                .get_mut(&0.into())
                .unwrap()
                .set_geometry(
                    Some(WidgetGeometry::new(&geometry, new_rect)),
                    changed_size_hints,
                );
        }
    }
}

impl Widget for TextInput {
    impl_widget_common!();

    fn new(mut common: WidgetCommonTyped<Self>) -> Self {
        common.set_supports_focus(true);
        common.cursor_icon = CursorIcon::Text;
        let host_id = common.id();
        let viewport = common
            .add_child_with_key::<Viewport>(0)
            .set_column(0)
            .set_row(0);
        viewport.common_mut().set_receives_all_mouse_events(true);
        viewport.common_mut().cursor_icon = CursorIcon::Text;
        let editor = viewport
            .common_mut()
            .add_child_with_key::<Text>(0)
            .set_multiline(false)
            .set_editable(true)
            .set_host_id(host_id.into());
        editor.common_mut().set_receives_all_mouse_events(true);
        Self {
            style: get_style(common.style_element(), common.scale()),
            common,
        }
    }

    fn handle_focus_in(&mut self, event: FocusInEvent) -> Result<()> {
        self.text_widget_mut().handle_host_focus_in(event.reason)
    }

    fn handle_focus_out(&mut self, _event: FocusOutEvent) -> Result<()> {
        self.text_widget_mut().handle_host_focus_out()
    }

    fn handle_layout(&mut self, event: LayoutEvent) -> Result<()> {
        grid_layout(self, &event.changed_size_hints);
        self.adjust_scroll(&event.changed_size_hints);
        Ok(())
    }

    fn handle_style_change(&mut self, _event: StyleChangeEvent) -> Result<()> {
        self.style = get_style(self.common().style_element(), self.common().scale());
        Ok(())
    }

    fn handle_size_hint_x_request(&mut self) -> Result<SizeHints> {
        Ok(SizeHints {
            min: self.style.min_width,
            preferred: self.style.preferred_width,
            is_fixed: false,
        })
    }

    fn handle_keyboard_input(&mut self, event: KeyboardInputEvent) -> Result<bool> {
        self.text_widget_mut().handle_host_keyboard_input(event)
    }

    fn handle_input_method(&mut self, event: InputMethodEvent) -> Result<bool> {
        self.text_widget_mut().handle_host_ime(event)
    }

    fn handle_scroll_to_rect_request(&mut self, event: ScrollToRectRequest) -> Result<bool> {
        if self.text_widget().common().id() != event.address.widget_id() {
            warn!("TextInput received unexpected ScrollToRectEvent");
            return Ok(false);
        }

        self.adjust_scroll(&[]);

        Ok(true)
    }
}
