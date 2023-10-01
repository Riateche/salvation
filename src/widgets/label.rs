use std::fmt::Display;

use anyhow::Result;
use cosmic_text::Attrs;

use crate::{draw::DrawEvent, layout::SizeHint, text_editor::TextEditor, types::Point};

use super::{Widget, WidgetCommon};

pub struct Label {
    editor: TextEditor,
    common: WidgetCommon,
}

impl Label {
    pub fn new(text: impl Display) -> Self {
        let mut editor = TextEditor::new(&text.to_string());
        editor.set_cursor_hidden(true);
        Self {
            editor,
            common: WidgetCommon::new(),
        }
    }

    pub fn set_text(&mut self, text: impl Display) {
        self.editor.set_text(&text.to_string(), Attrs::new());
        self.common.size_hint_changed();
        self.common.update();
    }
}

impl Widget for Label {
    fn on_draw(&mut self, event: DrawEvent) -> Result<()> {
        event.draw_pixmap(Point::default(), self.editor.pixmap().as_ref());
        Ok(())
    }

    fn common(&self) -> &WidgetCommon {
        &self.common
    }
    fn common_mut(&mut self) -> &mut WidgetCommon {
        &mut self.common
    }

    fn size_hint_x(&mut self) -> Result<SizeHint> {
        Ok(SizeHint {
            min: self.editor.size().x,
            preferred: self.editor.size().x,
            is_fixed: true,
        })
    }

    fn size_hint_y(&mut self, _size_x: i32) -> Result<SizeHint> {
        // TODO: use size_x, handle multiple lines
        Ok(SizeHint {
            min: self.editor.size().y,
            preferred: self.editor.size().y,
            is_fixed: true,
        })
    }
}
