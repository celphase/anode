use std::{
    ops::Rem,
    ptr::null_mut,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use machinery::{export_instance_fns, export_singleton_fns, identifier, Identifier};
use machinery_api::{foundation::{ColorSrgbT, RectT, TheTruthO, TtIdT, UiO, Vec2T}, plugins::{editor_views::AssetSaveI, ui::{Draw2dIbufferT, Draw2dStyleT, TM_UI_CURSOR_TEXT, TM_UI_EDIT_KEY_DELETE, TM_UI_EDIT_KEY_DOWN, TM_UI_EDIT_KEY_LEFT, TM_UI_EDIT_KEY_RIGHT, TM_UI_EDIT_KEY_UP, TM_UI_METRIC_SCROLLBAR_WIDTH, TabI, TabO, TabVt, TabVtRootT, UiApi, UiBuffersT, UiFontT, UiInputStateT, UiScrollbarT, UiStyleT}}, the_machinery::{TabCreateContextT, TheMachineryTabVt}};
use tracing::{event, Level};
use tree_sitter_highlight::HighlightEvent;
use ultraviolet::IVec2;

use crate::{
    document::{CaretDirection, DocumentState, TextChange},
    fonts::ANODE_CODE_FONT,
    plugin::{AnodePlugin, PluginData},
};

pub fn create_vtable() -> TheMachineryTabVt {
    TheMachineryTabVt {
        super_: TabVt {
            name: ANODE_CODE_EDITOR_TAB.name.as_ptr(),
            name_hash: ANODE_CODE_EDITOR_TAB.hash,
            create: Some(AnodePlugin::code_editor_create),
            destroy: Some(code_editor_destroy),
            title: Some(CodeEditorTab::title),
            ui: Some(CodeEditorTab::ui),
            set_root: Some(CodeEditorTab::set_root),
            root: Some(CodeEditorTab::root),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[export_singleton_fns]
impl AnodePlugin {
    unsafe fn code_editor_create(
        &self,
        context: *mut TabCreateContextT,
        _ui: *mut UiO,
    ) -> *mut TabI {
        let mut tab = Box::new(CodeEditorTab::new(self.data.clone(), context));
        tab.interface.inst = tab.as_ref() as *const _ as *mut TabO;

        let tab = Box::into_raw(tab);
        &mut (*tab).interface
    }
}

unsafe extern "C" fn code_editor_destroy(inst: *mut TabO) {
    let _tab: Box<CodeEditorTab> = Box::from_raw(inst as *mut _);
}

pub struct CodeEditorTab {
    interface: TabI,
    data: Arc<PluginData>,
    save_interface: *mut AssetSaveI,
    auto_activate: AtomicBool,
    state: Mutex<DocumentState>,
}

impl CodeEditorTab {
    unsafe fn new(data: Arc<PluginData>, context: *mut TabCreateContextT) -> Self {
        let interface = TabI {
            vt: data.apis.code_editor_tab_vtable as *mut TabVt,
            inst: null_mut(),
            root_id: *(*context).id,
        };

        *(*context).id += 1000000;

        Self {
            interface,
            data,
            save_interface: (*context).save_interface,
            auto_activate: AtomicBool::new(false),
            state: Mutex::new(DocumentState::new()),
        }
    }
}

#[export_instance_fns(TabO)]
impl CodeEditorTab {
    fn title(&self, _ui: *mut UiO) -> *const i8 {
        self.state
            .lock()
            .unwrap()
            .refresh_title(&self.data, self.save_interface)
            .as_ptr()
    }

    unsafe fn ui(&self, ui: *mut UiO, ui_style: *const UiStyleT, rect: RectT) {
        let ui_api = &*self.data.apis.ui;
        let mut state = self.state.lock().unwrap();

        let buffers = (*self.data.apis.ui).buffers(ui);
        let ibuffer = *buffers.ibuffers.offset((*ui_style).buffer as isize);
        let code_font = ui_api.font(ui, ANODE_CODE_FONT.hash, 10);

        let scrollbar_width = *buffers.metrics.offset(TM_UI_METRIC_SCROLLBAR_WIDTH as isize);
        let metrics = EditorMetrics::calculate(rect, &code_font, scrollbar_width);
        let active =
            self.handle_input(ui_api, ui, (*ui_style).clip, &buffers, &mut state, &metrics);

        // Fill the style for drawing
        let mut style = Draw2dStyleT {
            font: code_font.font,
            clip: (*ui_style).clip,
            font_scale: 1.0,
            ..Default::default()
        };

        // Draw the background
        style.color = ColorSrgbT {
            r: 30,
            g: 30,
            b: 30,
            a: 255,
        };
        (*self.data.apis.draw2d).fill_rect(buffers.vbuffer, ibuffer, &style, rect);

        // Draw parts
        let mut glyphs = Vec::new();
        self.draw_decorations(&buffers, ibuffer, &mut style, &mut glyphs, &metrics, &state);
        self.draw_code(
            ui_api,
            ui,
            &buffers,
            ibuffer,
            &mut style,
            &mut glyphs,
            &metrics,
            &state,
        );

        if active {
            self.draw_caret(&buffers, ibuffer, &metrics, &state, (*ui_style).clip);
        }

        // Draw text area scrollbars
        let mut scroll_y = 50.0;
        let scrollbar = UiScrollbarT {
            rect: RectT {
                x: metrics.rect.x + metrics.rect.w - scrollbar_width,
                y: metrics.rect.y,
                w: scrollbar_width,
                h: metrics.rect.h,
            },
            min: 0.0,
            max: 100.0,
            size: 10.0,
            ..Default::default()
        };
        ui_api.scrollbar_y(ui, ui_style, &scrollbar, &mut scroll_y);
    }

    unsafe fn set_root(&self, tt: *mut TheTruthO, root: TtIdT) {
        let mut state = self.state.lock().unwrap();
        let result = state.load_from_asset(&self.data, tt, root);

        if let Err(error) = result {
            event!(Level::ERROR, "{}", error);
            (*self.data.apis.docking).remove_tab(&self.interface as *const _ as *mut _);

            // This should be safe as long as we don't access the struct after this
            (*self.interface.vt).destroy.unwrap()(self.interface.inst);
        }
    }

    fn root(&self) -> TabVtRootT {
        let state = self.state.lock().unwrap();
        state
            .asset()
            .map(|asset| TabVtRootT {
                tt: asset.0,
                root: asset.1,
                internal_root: asset.1,
                counter: 0,
            })
            .unwrap_or_default()
    }
}

impl CodeEditorTab {
    unsafe fn handle_input(
        &self,
        ui_api: &UiApi,
        ui: *mut UiO,
        clip: u32,
        buffers: &UiBuffersT,
        state: &mut DocumentState,
        metrics: &EditorMetrics,
    ) -> bool {
        let input = &*buffers.input;

        let id = ui_api.make_id(ui);
        let mut active = ui_api.is_active(ui, id, ANODE_CODE_EDITOR_ACTIVE_DATA.hash);

        // Handle mouse input
        if ui_api.is_hovering(ui, metrics.textarea_rect, clip) {
            (*buffers.activation).next_hover = id;
        }

        let is_hovering = (*buffers.activation).hover == id;
        let mut should_activate = self.auto_activate.swap(false, Ordering::SeqCst);

        if is_hovering {
            ui_api.set_cursor(ui, TM_UI_CURSOR_TEXT);
        }

        // Activate or de-activate the component on mouse press
        if input.left_mouse_pressed || input.right_mouse_pressed {
            if is_hovering {
                should_activate = true;
            } else if (*buffers.activation).active == id {
                ui_api.clear_active(ui);
                active = null_mut();
            }
        }

        // If this component should be activated, check if it isn't already and then activate
        if should_activate && active.is_null() {
            active = ui_api.set_active(ui, id, ANODE_CODE_EDITOR_ACTIVE_DATA.hash);
            ui_api.set_responder_chain(ui, id);
        }

        // If the text area is active
        if !active.is_null() {
            self.handle_active_input(state, metrics, input);
        }

        !active.is_null()
    }

    unsafe fn handle_active_input(
        &self,
        state: &mut DocumentState,
        metrics: &EditorMetrics,
        input: &UiInputStateT,
    ) {
        if input.left_mouse_pressed {
            // Move the caret to the position the cursor is hovering over
            let relative_x = input.mouse_pos.x - metrics.textarea_rect.x;
            let relative_y = input.mouse_pos.y - metrics.textarea_rect.y;
            let line = ((relative_y - metrics.caret_start) / metrics.line_stride)
                .floor()
                .max(0.0) as usize;
            let offset = 4.0; // Feels just a bit better to have it offset a little
            let column = ((relative_x + offset) / metrics.char_width)
                .floor()
                .max(0.0) as usize;

            state.set_caret_line_column(line, column);
            state.set_caret_column_to_current();
        }

        // Handle text input
        let end = input.num_text_input as usize;
        for codepoint in &input.text_input[0..end] {
            match *codepoint {
                // Newline
                13 => state.apply_text_change(&self.data, TextChange::Character('\n')),
                // Backspace
                8 => state.apply_text_change(&self.data, TextChange::Backspace),
                // Ignore all other control characters
                v if v < 32 => continue,
                // Any text input
                _ => {
                    let character = std::char::from_u32(*codepoint).unwrap_or(' ');
                    state.apply_text_change(&self.data, TextChange::Character(character));
                }
            }
        }

        // Handle special edit input
        if input.edit_key_pressed[TM_UI_EDIT_KEY_LEFT as usize] {
            state.move_caret(CaretDirection::Left);
        }
        if input.edit_key_pressed[TM_UI_EDIT_KEY_RIGHT as usize] {
            state.move_caret(CaretDirection::Right);
        }
        if input.edit_key_pressed[TM_UI_EDIT_KEY_UP as usize] {
            state.move_caret(CaretDirection::Up);
        }
        if input.edit_key_pressed[TM_UI_EDIT_KEY_DOWN as usize] {
            state.move_caret(CaretDirection::Down);
        }

        if input.edit_key_pressed[TM_UI_EDIT_KEY_DELETE as usize] {
            state.apply_text_change(&self.data, TextChange::Delete);
        }
    }

    unsafe fn draw_decorations(
        &self,
        buffers: &UiBuffersT,
        ibuffer: *mut Draw2dIbufferT,
        style: &mut Draw2dStyleT,
        glyphs: &mut Vec<u16>,
        metrics: &EditorMetrics,
        state: &DocumentState,
    ) {
        style.color = ColorSrgbT {
            r: 120,
            g: 120,
            b: 120,
            a: 255,
        };

        for i in 0..state.text().split('\n').count() {
            // Draw the gutter (left side line numbers)
            let digits = digits(i as u32 + 1);
            let pos = Vec2T {
                x: metrics.rect.x,
                y: metrics.rect.y + metrics.first_baseline + (metrics.line_stride * i as f32),
            };
            self.draw_text(buffers, ibuffer, style, pos, glyphs, &digits);
        }

        // Draw the right side ruler
        style.color = ColorSrgbT {
            r: 80,
            g: 80,
            b: 80,
            a: 255,
        };
        let rect = RectT {
            x: metrics.textarea_rect.x + (metrics.char_width * 100.0).round(),
            y: metrics.textarea_rect.y,
            w: 1.0,
            h: metrics.textarea_rect.h,
        };
        (*self.data.apis.draw2d).fill_rect(buffers.vbuffer, ibuffer, style, rect);
    }

    unsafe fn draw_caret(
        &self,
        buffers: &UiBuffersT,
        ibuffer: *mut Draw2dIbufferT,
        metrics: &EditorMetrics,
        state: &DocumentState,
        clip: u32,
    ) {
        let (line, column) = state.caret_line_column();

        let pos = Vec2T {
            x: metrics.textarea_rect.x + (column as f32 * metrics.char_width),
            y: metrics.textarea_rect.y + metrics.caret_start + (metrics.line_stride * line as f32),
        };

        let caret = RectT {
            x: pos.x - 1.0,
            y: pos.y,
            w: 2.0,
            h: metrics.line_stride,
        };
        let style = Draw2dStyleT {
            color: CARET_COLOR,
            clip,
            ..Default::default()
        };
        (*self.data.apis.draw2d).fill_rect(buffers.vbuffer, ibuffer, &style, caret);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_code(
        &self,
        ui_api: &UiApi,
        ui: *mut UiO,
        buffers: &UiBuffersT,
        ibuffer: *mut Draw2dIbufferT,
        style: &mut Draw2dStyleT,
        glyphs: &mut Vec<u16>,
        metrics: &EditorMetrics,
        state: &DocumentState,
    ) {
        let mut codepoints = Vec::new();
        style.color = BASE_CODE_COLOR;

        // Indexing ranges into the string repeatedly is slow as it's not an O(1) operation, instead
        // we'll progressively step through the string's iterator
        let mut chars = state.text().chars();

        // Text position cursor for rendering, this is how we layout the text
        let mut position = IVec2::new(0, 0);

        for event in state.highlights() {
            match event {
                HighlightEvent::Source { start, end } => {
                    let segment = (&mut chars).take(end - start);
                    self.draw_segment(
                        buffers,
                        ibuffer,
                        style,
                        metrics,
                        glyphs,
                        &mut codepoints,
                        &mut position,
                        segment,
                    );
                    ui_api.reserve_draw_memory(ui);
                }
                HighlightEvent::HighlightStart(higlight) => {
                    style.color = self.data.token_colors[higlight.0].color;
                }
                HighlightEvent::HighlightEnd => {
                    style.color = BASE_CODE_COLOR;
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_segment(
        &self,
        buffers: &UiBuffersT,
        ibuffer: *mut Draw2dIbufferT,
        style: &mut Draw2dStyleT,
        metrics: &EditorMetrics,
        glyphs: &mut Vec<u16>,
        codepoints: &mut Vec<u32>,
        position: &mut IVec2,
        mut segment: impl Iterator<Item = char>,
    ) {
        let mut had_newline = true;
        while had_newline {
            // Accumulate until codepoints until we hit either the end of the segment, or a newline
            had_newline = false;
            'inner: for c in &mut segment {
                // If the character is a newline, don't include it and draw a sub-segment of text
                if c == '\n' {
                    had_newline = true;
                    break 'inner;
                }

                codepoints.push(c as u32);
            }

            if !codepoints.is_empty() {
                // Draw the text
                self.draw_text(
                    buffers,
                    ibuffer,
                    style,
                    Vec2T {
                        x: metrics.textarea_rect.x + (position.x as f32 * metrics.char_width),
                        y: metrics.textarea_rect.y
                            + metrics.first_baseline
                            + (position.y as f32 * metrics.line_stride),
                    },
                    glyphs,
                    codepoints,
                );

                // Add to the positions
                position.x += codepoints.len() as i32;
                codepoints.clear();
            }

            if had_newline {
                position.x = 0;
                position.y += 1;
            }
        }
    }

    unsafe fn draw_text(
        &self,
        buffers: &UiBuffersT,
        ibuffer: *mut Draw2dIbufferT,
        style: &Draw2dStyleT,
        mut pos: Vec2T,
        glyphs: &mut Vec<u16>,
        codepoints: &[u32],
    ) {
        // Hack to improve blurryness issues
        pos.y = pos.y.round();

        // Convert codepoints into glyph IDs
        glyphs.resize(codepoints.len(), 0);
        (*self.data.apis.font).glyphs(
            (*style.font).info,
            glyphs.as_mut_ptr(),
            codepoints.as_ptr(),
            codepoints.len() as u32,
        );

        // Draw the glyphs
        (*self.data.apis.draw2d).draw_glyphs(
            buffers.vbuffer,
            ibuffer,
            style,
            pos,
            glyphs.as_ptr(),
            glyphs.len() as u32,
        );
    }
}

/// Convert digits of an integer to unicode codepoints, right aligned.
fn digits(value: u32) -> [u32; 5] {
    let mut codepoints = [32u32; 5];

    let mut write = false;
    for (i, codepoint) in codepoints.iter_mut().enumerate() {
        let div = 10u32.pow(4 - i as u32);
        let digit = (value / div).rem(10);

        if digit != 0 || write {
            *codepoint = 48 + digit;
            write = true;
        }
    }

    codepoints
}

struct EditorMetrics {
    first_baseline: f32,
    line_stride: f32,
    char_width: f32,
    caret_start: f32,
    rect: RectT,
    textarea_rect: RectT,
}

impl EditorMetrics {
    pub unsafe fn calculate(rect: RectT, font: &UiFontT, scrolbar_width: f32) -> Self {
        let font_info = &*(*font.font).info;

        // Font metrics
        let padding = 4.0;
        let first_line = padding + font_info.ascent[0];
        let line_stride = font_info.ascent[0] + font_info.descent[0] + font_info.line_gap[0];
        let char_width = (*font_info.glyphs).xadvance;
        let caret_start = padding - (font_info.line_gap[0] * 0.5);

        // Layouting sizes
        let line_offset = char_width * 7.0;
        let mut textarea_rect = rect;
        textarea_rect.x += line_offset;
        textarea_rect.w -= line_offset + scrolbar_width;

        Self {
            first_baseline: first_line,
            line_stride,
            char_width,
            caret_start,
            rect,
            textarea_rect,
        }
    }
}

const BASE_CODE_COLOR: ColorSrgbT = ColorSrgbT {
    r: 220,
    g: 220,
    b: 220,
    a: 255,
};

const CARET_COLOR: ColorSrgbT = ColorSrgbT {
    r: 200,
    g: 200,
    b: 200,
    a: 255,
};

pub const ANODE_CODE_EDITOR_TAB: Identifier = identifier!("tm_anode_code_editor_tab");

const ANODE_CODE_EDITOR_ACTIVE_DATA: Identifier = identifier!("tm_anode_code_editor_data_t");
