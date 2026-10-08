//! GPUI single-line text field (IME, clipboard, selection).

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use gpui::{
    actions, div, point, px, App, Bounds, ClipboardItem, Context, CursorStyle, Element,
    ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, ParentElement, Pixels, Point, Render, ShapedLine, SharedString,
    Styled, TextAlign, TextRun, UnderlineStyle, UTF16Selection, Window,
};

use crate::{
    components::focus::focus_visible_ring,
    theme::ResolvedTheme,
};

use super::editor::SingleLineEditor;

actions!(
    wisp_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Undo,
        Redo,
    ]
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputKind {
    SingleLine,
    Password,
    Mono,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    editor: SingleLineEditor,
    placeholder: SharedString,
    kind: TextInputKind,
    theme: ResolvedTheme,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        placeholder: impl Into<SharedString>,
        kind: TextInputKind,
        theme: ResolvedTheme,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            editor: SingleLineEditor::default(),
            placeholder: placeholder.into(),
            kind,
            theme,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
        }
    }

    pub fn content(&self) -> &str {
        self.editor.content()
    }

    pub fn set_content(&mut self, value: &str) {
        self.editor = SingleLineEditor::new(value);
    }

    pub fn set_theme(&mut self, theme: ResolvedTheme) {
        self.theme = theme;
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.selection().is_empty() {
            self.editor
                .move_to(self.editor.previous_boundary(self.editor.cursor()));
        } else {
            self.editor.move_to(self.editor.selection().start);
        }
        cx.notify();
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.selection().is_empty() {
            self.editor
                .move_to(self.editor.next_boundary(self.editor.selection().end));
        } else {
            self.editor.move_to(self.editor.selection().end);
        }
        cx.notify();
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .select_to(self.editor.previous_boundary(self.editor.cursor()));
        cx.notify();
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.editor
            .select_to(self.editor.next_boundary(self.editor.cursor()));
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.select_all();
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_to(0);
        cx.notify();
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.move_to(self.editor.content().len());
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.delete_backward();
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.delete_forward();
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.editor.insert(&text);
            cx.notify();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.editor.selection();
        if !range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.editor.content()[range].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.editor.selection();
        if !range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.editor.content()[range].to_string(),
            ));
            self.editor.delete_backward();
            cx.notify();
        }
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.undo() {
            cx.notify();
        }
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        if self.editor.redo() {
            cx.notify();
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let index = self.index_for_mouse_position(event.position);
        if event.modifiers.shift {
            self.editor.select_to(index);
        } else {
            self.editor.move_to(index);
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.editor
                .select_to(self.index_for_mouse_position(event.position));
            cx.notify();
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.editor.content().is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.editor.content().len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn display_text(&self) -> SharedString {
        let content = self.editor.content();
        if self.kind == TextInputKind::Password && !content.is_empty() {
            return "•".repeat(content.graphemes(true).count()).into();
        }
        content.into()
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.editor.range_from_utf16(&range_utf16);
        actual_range.replace(self.editor.range_to_utf16(&range));
        Some(self.editor.content()[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.editor.range_to_utf16(&self.editor.selection()),
            reversed: self.editor.selection_reversed(),
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.editor
            .marked_range()
            .as_ref()
            .map(|r| self.editor.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.clear_marked_range();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.editor.range_from_utf16(r))
            .or_else(|| self.editor.marked_range())
            .unwrap_or_else(|| self.editor.selection());
        self.editor.replace_range(range, new_text, true);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|r| self.editor.range_from_utf16(r))
            .or_else(|| self.editor.marked_range())
            .unwrap_or_else(|| self.editor.selection());
        let inner = new_selected_range_utf16
            .as_ref()
            .map(|r| self.editor.range_from_utf16(r));
        self.editor.replace_for_ime(range, new_text, inner);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.editor.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(
                bounds.left() + line.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + line.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let utf8_index = line.index_for_x(point.x - line_point.x)?;
        Some(self.editor.offset_to_utf16(utf8_index))
    }
}

struct TextInputElement {
    input: Entity<TextInput>,
}

impl IntoElement for TextInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl Element for TextInputElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let mut style = gpui::Style::default();
        style.size.width = gpui::relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let c = &input.theme.colors;
        let display = input.display_text();
        let selected = input.editor.selection();
        let cursor = input.editor.cursor();
        let font = match input.kind {
            TextInputKind::Mono => input.theme.typography.gpui_mono_font(),
            _ => input.theme.typography.gpui_ui_font(),
        };
        let font_size = match input.kind {
            TextInputKind::Mono => input.theme.mono_font_size,
            _ => input.theme.ui_font_size,
        };

        let (text, color) = if display.is_empty() {
            (input.placeholder.clone(), c.ink3.into())
        } else {
            (display, c.ink1.into())
        };

        let run = TextRun {
            len: text.len(),
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let runs = if let Some(marked) = input.editor.marked_range() {
            vec![
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(c.accent.into()),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len().saturating_sub(marked.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let line = window
            .text_system()
            .shape_line(text, font_size, &runs, None);

        let cursor_pos = line.x_for_index(cursor);
        let (selection, cursor) = if selected.is_empty() {
            (
                None,
                Some(gpui::fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top()),
                        gpui::size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    c.accent,
                )),
            )
        } else {
            (
                Some(gpui::fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected.end),
                            bounds.bottom(),
                        ),
                    ),
                    c.accent_soft,
                )),
                None,
            )
        };

        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection);
        }
        let line = prepaint.line.take().unwrap();
        line.paint(bounds.origin, window.line_height(), TextAlign::Left, None, window, cx)
            .ok();
        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }
        self.input.update(cx, |input, _| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = &self.theme.colors;
        let row_h = self.theme.density.row_height();
        div()
            .id("wisp-text-input")
            .key_context("WispTextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .tab_index(0)
            .focus_visible(focus_visible_ring(c))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .h(row_h)
            .px_2()
            .flex()
            .items_center()
            .bg(c.panel)
            .border_1()
            .border_color(c.line)
            .rounded_md()
            .child(TextInputElement {
                input: cx.entity().clone(),
            })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

pub fn bind_text_input_keys(cx: &mut App) {
    use gpui::KeyBinding;
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("WispTextInput")),
        KeyBinding::new("delete", Delete, Some("WispTextInput")),
        KeyBinding::new("left", Left, Some("WispTextInput")),
        KeyBinding::new("right", Right, Some("WispTextInput")),
        KeyBinding::new("shift-left", SelectLeft, Some("WispTextInput")),
        KeyBinding::new("shift-right", SelectRight, Some("WispTextInput")),
        KeyBinding::new("cmd-a", SelectAll, Some("WispTextInput")),
        KeyBinding::new("cmd-v", Paste, Some("WispTextInput")),
        KeyBinding::new("cmd-c", Copy, Some("WispTextInput")),
        KeyBinding::new("cmd-x", Cut, Some("WispTextInput")),
        KeyBinding::new("cmd-z", Undo, Some("WispTextInput")),
        KeyBinding::new("cmd-shift-z", Redo, Some("WispTextInput")),
        KeyBinding::new("home", Home, Some("WispTextInput")),
        KeyBinding::new("end", End, Some("WispTextInput")),
    ]);
}
