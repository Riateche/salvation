use {
    crate::{
        event::FocusReason,
        system::with_system,
        types::{Point, Size},
        window::Window,
    },
    accesskit::{NodeId, TextDirection, TextPosition, TextSelection},
    line_straddler::{GlyphStyle, LineGenerator, LineType},
    log::warn,
    range_ext::intersect::Intersect,
    salvation_cosmic_text::{
        Action, Affinity, Attrs, AttrsList, AttrsOwned, BorrowedWithFontSystem, Buffer, Cursor,
        Edit, Editor, Shaping, Wrap,
    },
    std::{
        cmp::{max, min},
        ops::Range,
    },
    strict_num::FiniteF32,
    tiny_skia::{Color, Paint, PathBuilder, Pixmap, Shader, Stroke, Transform},
    unicode_segmentation::UnicodeSegmentation,
};

pub struct TextEditor {
    editor: Editor<'static>,
    pixmap: Option<Pixmap>,
    text_color: Color,
    selected_text_color: Color,
    selected_text_background: Color,
    size: Size,
    window: Option<Window>,
    is_cursor_hidden: bool,
    forbid_mouse_interaction: bool,
}

#[derive(Debug)]
pub struct AccessibleLine {
    pub text: String,
    pub text_direction: TextDirection,
    pub character_lengths: Vec<u8>,
    pub character_positions: Vec<f32>,
    pub character_widths: Vec<f32>,
    pub word_lengths: Vec<u8>,
    // pub line_top: f32,
    // pub line_bottom: f32,
}

impl TextEditor {
    pub fn new(text: &str) -> Self {
        let mut e = with_system(|system| Self {
            editor: Editor::new(Buffer::new(
                &mut system.font_system,
                system.default_style.0.font_metrics,
            )),
            pixmap: None,
            text_color: Color::BLACK,
            selected_text_color: Color::TRANSPARENT,
            selected_text_background: Color::TRANSPARENT,
            size: Size::default(),
            window: None,
            is_cursor_hidden: false,
            forbid_mouse_interaction: false,
        });
        e.set_text(text, Attrs::new());
        e.adjust_size();
        e
    }

    pub fn set_font_metrics(&mut self, metrics: salvation_cosmic_text::Metrics) {
        with_system(|system| {
            self.editor
                .with_buffer_mut(|buffer| buffer.set_metrics(&mut system.font_system, metrics));
        });
        self.adjust_size();
    }

    pub fn set_window(&mut self, window: Option<Window>) {
        self.window = window;
    }

    pub fn set_wrap(&mut self, wrap: Wrap) {
        with_system(|system| {
            self.editor
                .with_buffer_mut(|buffer| buffer.set_wrap(&mut system.font_system, wrap));
        });
    }

    pub fn set_text(&mut self, text: &str, attrs: Attrs) {
        with_system(|system| {
            self.editor.with_buffer_mut(|buffer| {
                buffer.set_text(&mut system.font_system, text, attrs, Shaping::Advanced)
            });
        });
        self.adjust_size();
    }

    pub fn text(&self) -> String {
        self.editor
            .with_buffer(|buffer| buffer.text_without_preedit())
    }

    pub fn acccessible_line(&mut self) -> AccessibleLine {
        #[derive(Debug)]
        struct CharStats {
            bytes: Range<usize>,
            pixels: Option<Range<FiniteF32>>,
        }

        self.shape_as_needed();
        // TODO: extend for multiline
        // TODO: take ref
        let text = self
            .editor
            .with_buffer(|buffer| buffer.lines[0].text().to_owned());

        let mut character_lengths = Vec::new();
        let mut character_stats = Vec::new();
        for (i, c) in text.grapheme_indices(true) {
            character_lengths.push(c.len() as u8);
            character_stats.push(CharStats {
                bytes: i..i + c.len(),
                pixels: None,
            });
        }
        let mut word_lengths = Vec::new();
        // TODO: expose from cosmic-text
        let mut prev_index_in_chars = None;
        let mut total_chars_in_words = 0;
        for (i, word) in text.unicode_word_indices() {
            let end_i = i + word.len();
            let index_in_chars = character_stats
                .iter()
                .take_while(|s| s.bytes.start < end_i)
                .count();
            // TODO: checked_sub?
            let len_in_chars = index_in_chars - prev_index_in_chars.unwrap_or(0);
            word_lengths.push(len_in_chars as u8);
            prev_index_in_chars = Some(index_in_chars);
            total_chars_in_words += len_in_chars;
        }
        if total_chars_in_words < character_stats.len() {
            word_lengths.push((character_stats.len() - total_chars_in_words) as u8);
        }
        let text_direction = self.editor.with_buffer(|buffer| {
            let mut runs = buffer.layout_runs();
            let run = runs.next().expect("missing layout run");
            if runs.next().is_some() {
                warn!("multiple layout_runs in single line edit");
            }

            if run.line_i != 0 {
                warn!("invalid line_i in single line layout_runs");
            }
            for glyph in run.glyphs {
                if let Some(stats) = character_stats
                    .iter_mut()
                    .find(|s| s.bytes.does_intersect(&(glyph.start..glyph.end)))
                {
                    let new_start = FiniteF32::new(glyph.x).unwrap();
                    let new_end = FiniteF32::new(glyph.x + glyph.w).unwrap();
                    if let Some(pixels) = &mut stats.pixels {
                        pixels.start = min(pixels.start, new_start);
                        pixels.end = max(pixels.end, new_end);
                    } else {
                        stats.pixels = Some(new_start..new_end);
                    }
                } else {
                    warn!("no char found for glyph: {glyph:?}");
                }
            }
            if run.rtl {
                TextDirection::RightToLeft
            } else {
                TextDirection::LeftToRight
            }
        });

        AccessibleLine {
            text_direction,
            // line_top: run.line_top,
            // line_bottom: run.line_top + self.editor.buffer().metrics().line_height,
            text,
            character_lengths,
            character_positions: character_stats
                .iter()
                .map(|s| {
                    s.pixels.as_ref().map_or_else(
                        || {
                            warn!("glyph for char not found");
                            0.0
                        },
                        |range| range.start.get(),
                    )
                })
                .collect(),
            character_widths: character_stats
                .iter()
                .map(|s| {
                    s.pixels.as_ref().map_or_else(
                        || {
                            warn!("glyph for char not found;");
                            0.0
                        },
                        |range| range.end.get() - range.start.get(),
                    )
                })
                .collect(),
            // TODO: real words
            word_lengths,
        }
    }

    pub fn set_accessible_selection(&mut self, data: TextSelection) {
        let text = self
            .editor
            .with_buffer(|buffer| buffer.lines[0].text().to_string());
        let char_to_byte_index =
            |char_index| text.grapheme_indices(true).nth(char_index).map(|(i, _)| i);
        if data.anchor == data.focus {
            self.set_select_opt(None);
        } else {
            let Some(index) = char_to_byte_index(data.anchor.character_index) else {
                warn!("char index is too large");
                return;
            };
            self.set_select_opt(Some(Cursor {
                line: 0,
                index,
                affinity: Affinity::Before,
            }));
        }
        let Some(index) = char_to_byte_index(data.focus.character_index) else {
            warn!("char index is too large");
            return;
        };
        self.set_cursor(Cursor {
            line: 0,
            index,
            affinity: Affinity::Before,
        });
    }

    pub fn accessible_selection(&mut self, id: NodeId) -> TextSelection {
        let text = self
            .editor
            .with_buffer(|buffer| buffer.lines[0].text().to_string());
        let byte_to_char_index = |byte_index| {
            text.grapheme_indices(true)
                .take_while(|(i, _)| *i < byte_index)
                .count()
        };
        let focus = TextPosition {
            node: id,
            character_index: byte_to_char_index(self.cursor().index),
        };
        let anchor = if let Some(select) = self.select_opt() {
            TextPosition {
                node: id,
                character_index: byte_to_char_index(select.index),
            }
        } else {
            focus
        };
        TextSelection { anchor, focus }
    }

    pub fn insert_string(&mut self, text: &str, attrs_list: Option<AttrsList>) {
        self.editor.insert_string(text, attrs_list);
        self.adjust_size();
    }

    fn set_size(&mut self, size: Size) {
        with_system(|system| {
            self.editor.with_buffer_mut(|buffer| {
                buffer.set_size(&mut system.font_system, size.x as f32, size.y as f32)
            });
        });
        self.size = size;
    }

    pub fn size(&self) -> Size {
        self.size
    }

    pub fn set_text_color(&mut self, color: Color) {
        if self.text_color != color {
            self.text_color = color;
            self.editor.set_redraw(true);
        }
    }

    pub fn set_selected_text_color(&mut self, color: Color) {
        if self.selected_text_color != color {
            self.selected_text_color = color;
            self.editor.set_redraw(true);
        }
    }

    pub fn set_selected_text_background(&mut self, color: Color) {
        if self.selected_text_background != color {
            self.selected_text_background = color;
            self.editor.set_redraw(true);
        }
    }

    pub fn shape_as_needed(&mut self) {
        with_system(|system| self.editor.shape_as_needed(&mut system.font_system, false));
    }

    pub fn needs_redraw(&mut self) -> bool {
        self.shape_as_needed();
        self.editor.redraw()
    }

    pub fn is_mouse_interaction_forbidden(&self) -> bool {
        self.forbid_mouse_interaction
    }

    pub fn pixmap(&mut self) -> &Pixmap {
        if self.pixmap.is_none() || self.needs_redraw() {
            let buffer_size = self.editor.with_buffer(|buffer| buffer.size());
            let size = Size {
                x: max(1, buffer_size.0.ceil() as i32),
                y: max(1, buffer_size.1.ceil() as i32),
            };
            let mut pixmap =
                Pixmap::new(size.x as u32, size.y as u32).expect("failed to create pixmap");
            with_system(|system| {
                self.editor.draw(
                    &mut system.font_system,
                    &mut system.swash_cache,
                    convert_color(self.text_color),
                    convert_color(self.text_color), // TODO: cursor color
                    convert_color(self.selected_text_background),
                    convert_color(self.selected_text_color),
                    |x, y, w, h, c| {
                        let color = Color::from_rgba8(c.r(), c.g(), c.b(), c.a());
                        let paint = Paint {
                            shader: Shader::SolidColor(color),
                            ..Paint::default()
                        };
                        pixmap.fill_rect(
                            tiny_skia::Rect::from_xywh(x as f32, y as f32, w as f32, h as f32)
                                .unwrap(),
                            &paint,
                            Transform::default(),
                            None,
                        );
                    },
                );
            });
            let mut alg = LineGenerator::new(LineType::Underline);
            let mut lines = Vec::new();
            let line_height = self
                .editor
                .with_buffer(|buffer| buffer.metrics().line_height);
            // TODO: determine from glyph width?
            let stroke_width = 1.0;
            self.editor.with_buffer(|buffer| {
                for run in buffer.layout_runs() {
                    let underline_space = line_height - run.line_y;
                    let line_y = run.line_top + underline_space / 2.0;
                    let line_y = (line_y + stroke_width / 2.0).round() - stroke_width / 2.0;
                    for glyph in run.glyphs {
                        if glyph.metadata & 0x1 != 0 {
                            let color = glyph.color_opt.unwrap_or(convert_color(self.text_color));
                            let glyph = line_straddler::Glyph {
                                line_y,
                                font_size: glyph.font_size,
                                width: glyph.w,
                                x: glyph.x,
                                style: GlyphStyle {
                                    boldness: 1,
                                    color: line_straddler::Color::rgba(
                                        color.r(),
                                        color.g(),
                                        color.b(),
                                        color.a(),
                                    ),
                                },
                            };
                            lines.extend(alg.add_glyph(glyph));
                        }
                    }
                }
            });
            lines.extend(alg.pop_line());
            for line in lines {
                let mut path = PathBuilder::new();
                path.move_to(line.start_x, line.y);
                path.line_to(line.end_x, line.y);
                pixmap.stroke_path(
                    &path.finish().unwrap(),
                    &Paint {
                        shader: Shader::SolidColor(tiny_skia::Color::from_rgba8(
                            line.style.color.red(),
                            line.style.color.green(),
                            line.style.color.blue(),
                            line.style.color.alpha(),
                        )),
                        ..Paint::default()
                    },
                    &Stroke {
                        width: stroke_width,
                        ..Stroke::default()
                    },
                    Transform::default(),
                    None,
                );
            }
            self.pixmap = Some(pixmap);
            self.editor.set_redraw(false);
        }
        self.pixmap.as_ref().expect("created above")
    }

    pub fn cursor_position(&mut self) -> Option<Point> {
        self.editor.cursor_position().map(|(x, y)| Point { x, y })
    }

    pub fn line_height(&self) -> f32 {
        self.editor
            .with_buffer(|buffer| buffer.metrics().line_height)
    }

    // TODO: remove
    pub fn action(&mut self, mut action: Action) {
        match &mut action {
            Action::SetPreedit { attrs, .. } => {
                if attrs.is_none() {
                    let mut new_attrs = self.attrs_at_cursor();
                    new_attrs.metadata = 1;
                    *attrs = Some(new_attrs);
                }
            }
            Action::Drag { .. } => {
                if self.forbid_mouse_interaction {
                    return;
                }
            }
            _ => (),
        }
        with_system(|system| self.editor.action(&mut system.font_system, action));
        self.adjust_size();
    }

    pub fn cursor(&self) -> Cursor {
        self.editor.cursor()
    }
    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.editor.set_cursor(cursor);
    }
    pub fn has_selection(&self) -> bool {
        self.editor.has_selection()
    }
    // TODO: update API
    pub fn select_opt(&self) -> Option<Cursor> {
        if let salvation_cosmic_text::Selection::Normal(value) = self.editor.selection() {
            Some(value)
        } else {
            None
        }
    }
    pub fn set_select_opt(&mut self, select_opt: Option<Cursor>) {
        self.editor.set_selection(if let Some(cursor) = select_opt {
            salvation_cosmic_text::Selection::Normal(cursor)
        } else {
            salvation_cosmic_text::Selection::None
        })
    }

    fn interrupt_preedit(&mut self) {
        if let Some(text) = self.editor.preedit_text() {
            let text = text.to_owned();
            self.action(Action::SetPreedit {
                preedit: String::new(),
                cursor: None,
                attrs: None,
            });
            self.insert_string(&text, None);
            if let Some(window) = &self.window {
                window.cancel_ime_preedit();
            } else {
                warn!("no window id in text editor event handler");
            }
        }
    }

    pub fn on_focus_in(&mut self, reason: FocusReason) {
        if reason == FocusReason::Tab {
            self.action(Action::SelectAll);
        }
    }

    pub fn on_focus_out(&mut self) {
        self.interrupt_preedit();
        self.action(Action::Escape);
    }

    pub fn on_window_focus_changed(&mut self, focused: bool) {
        if !focused {
            self.interrupt_preedit();
        }
    }

    pub fn on_mouse_input(&mut self, pos: Point, num_clicks: u32, select: bool) {
        let old_cursor = self.editor.cursor();
        let preedit_range = self.editor.preedit_range();
        let click_cursor = self
            .editor
            .with_buffer(|buffer| buffer.hit(pos.x as f32, pos.y as f32));
        if let Some(click_cursor) = click_cursor {
            if click_cursor.line == old_cursor.line
                && preedit_range
                    .as_ref()
                    .map_or(false, |ime_range| ime_range.contains(&click_cursor.index))
            {
                // Click is inside IME preedit, so we ignore it.
                self.forbid_mouse_interaction = true;
            } else {
                // Click is outside IME preedit, so we insert the preedit text
                // as real text and cancel IME preedit.
                self.interrupt_preedit();
                self.shape_as_needed();
                let x = pos.x;
                let y = pos.y;
                match ((num_clicks - 1) % 3) + 1 {
                    1 => self.action(Action::Click { x, y, select }),
                    2 => self.action(Action::DoubleClick { x, y }),
                    3 => self.action(Action::TripleClick { x, y }),
                    _ => {}
                }
            }
        }
    }

    pub fn mouse_released(&mut self) {
        self.forbid_mouse_interaction = false;
    }

    fn attrs_at_cursor(&self) -> AttrsOwned {
        // TODO: use lines.get() everywhere to be safe
        self.editor.with_buffer(|buffer| {
            let line = &buffer.lines[self.editor.cursor().line];
            AttrsOwned::new(line.attrs_list().get_span(self.editor.cursor().index))
        })
    }

    pub fn unrestricted_text_size(&mut self) -> Size {
        with_system(|system| {
            self.editor.with_buffer_mut(|buffer| {
                unrestricted_text_size(&mut buffer.borrow_with(&mut system.font_system))
            })
        })
    }

    // TODO: adapt for multiline text
    fn adjust_size(&mut self) {
        let unrestricted_size = self.unrestricted_text_size();
        self.set_size(unrestricted_size);
    }

    pub fn set_cursor_hidden(&mut self, hidden: bool) {
        self.editor.set_cursor_hidden(hidden);
        self.is_cursor_hidden = hidden;
    }

    pub fn is_cursor_hidden(&self) -> bool {
        self.is_cursor_hidden
    }

    pub fn selection_bounds(&self) -> Option<(Cursor, Cursor)> {
        if self.editor.has_selection() {
            self.editor.selection_bounds()
        } else {
            None
        }
    }

    pub fn selected_text(&mut self) -> Option<String> {
        // TODO: patch cosmic-text to remove mut and don't return empty selection
        self.editor.copy_selection().filter(|s| !s.is_empty())
    }
}

impl Default for TextEditor {
    fn default() -> Self {
        Self::new("")
    }
}

const MEASURE_MAX_SIZE: f32 = 10_000.;

fn unrestricted_text_size(buffer: &mut BorrowedWithFontSystem<'_, Buffer>) -> Size {
    buffer.set_size(MEASURE_MAX_SIZE, MEASURE_MAX_SIZE);
    buffer.shape_until_scroll(false);
    let height = (buffer.lines.len() as f32 * buffer.metrics().line_height).ceil() as i32;
    let width = buffer
        .layout_runs()
        .map(|run| run.line_w.ceil() as i32)
        .max()
        .unwrap_or(0);

    Size {
        x: width,
        y: height,
    }
}

fn convert_color(color: Color) -> salvation_cosmic_text::Color {
    let c = color.to_color_u8();
    salvation_cosmic_text::Color::rgba(c.red(), c.green(), c.blue(), c.alpha())
}
