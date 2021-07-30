use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
};

use machinery::{tt_id_eq, tt_id_type};
use machinery_api::foundation::{TheTruthO, TtIdT, TtUndoScopeT, TM_TT_ASPECT__FILE_EXTENSION};
use tm_anode_api::{AnodeHighlightingAspectI, ASPECT_ANODE_HIGHLIGHTING};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::plugin::PluginData;

pub(crate) struct EditorState {
    // Associated target asset
    asset: Option<(*mut TheTruthO, TtIdT)>,

    // Metadata
    title: CString,

    // Highlighting utilities
    highlighter: Highlighter,
    highlight_config: Option<HighlightConfiguration>,

    // Current text state
    text: String,
    highlights: Vec<HighlightEvent>,
    caret: usize,
    /// The caret column position will be preserved when moving up/down.
    caret_column: usize,
}

impl EditorState {
    pub fn new() -> Self {
        Self {
            asset: None,
            title: CString::new("untitled").unwrap(),
            highlighter: Highlighter::new(),
            highlight_config: None,
            text: String::new(),
            highlights: Vec::new(),
            caret: 0,
            caret_column: 0,
        }
    }

    pub fn asset(&self) -> Option<(*mut TheTruthO, TtIdT)> {
        self.asset
    }

    pub fn title(&self) -> &CStr {
        self.title.as_c_str()
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn highlights(&self) -> &[HighlightEvent] {
        &self.highlights
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    fn caret_line_column(&self) -> (usize, usize) {
        // Find the right line
        let mut last_line_index = 0;
        let mut last_length = 0;
        let mut index = 0;
        for (line_index, line) in self.text.split('\n').enumerate() {
            // If the caret is within this line, return the value
            if self.caret < index + line.len() + 1 {
                return (line_index, self.caret - index);
            }

            last_line_index = line_index;
            last_length = line.len();
            index += line.len() + 1;
        }

        // Fall back to end of the last line
        (last_line_index, last_length - 1)
    }

    fn set_caret(&mut self, index: usize) {
        self.caret = index;
        self.set_caret_column_to_current();
    }

    pub fn set_caret_column_to_current(&mut self) {
        let (_, column) = self.caret_line_column();
        self.caret_column = column;
    }

    /// Sets the caret to a given line and column, clamping to end of line where necessary.
    ///
    /// Caret column isn't set, so up/down movement will be preserved.
    /// If you need to set this as well, call [`Self::set_caret_column_to_current`].
    pub fn set_caret_line_column(&mut self, line: usize, column: usize) {
        // Find the starting index of the line
        let mut index = 0;
        let mut current_line = 0;
        let mut chars = self.text.chars();
        loop {
            if current_line == line {
                // Continue until we find the column
                let mut current_column = 0;
                'inner: loop {
                    if current_column == column {
                        self.caret = index;
                        return;
                    }

                    match chars.next() {
                        Some('\n') => break 'inner,
                        None => break 'inner,
                        _ => {}
                    }

                    current_column += 1;
                    index += 1;
                }

                // Default to end of line
                self.caret = index;
                return;
            }

            match chars.next() {
                Some('\n') => current_line += 1,
                None => break,
                _ => {}
            }

            index += 1;
        }

        // Default to end of file
        self.caret = self.text.len();
    }

    pub fn move_caret(&mut self, direction: CaretDirection) {
        match direction {
            CaretDirection::Left => self.set_caret((self.caret as i32 - 1).max(0) as usize),
            CaretDirection::Right => self.set_caret((self.caret + 1).min(self.text.len())),
            CaretDirection::Up => {
                let (line, _) = self.caret_line_column();
                if line > 0 {
                    self.set_caret_line_column(line - 1, self.caret_column);
                } else {
                    // If we're already on the first line, force to top
                    self.set_caret(0);
                }
            }
            CaretDirection::Down => {
                let (line, _) = self.caret_line_column();
                let lines = self.text.split('\n').count();
                if line < lines - 1 {
                    self.set_caret_line_column(line + 1, self.caret_column);
                } else {
                    // If we're already on the last line, force to end
                    self.set_caret(self.text.len());
                }
            }
        }
    }

    pub unsafe fn load_from_asset(&mut self, data: &PluginData, tt: *mut TheTruthO, root: TtIdT) {
        // If we've already got the same object, don't do anything
        if let Some(asset) = self.asset.as_ref() {
            if asset.0 == tt && tt_id_eq(asset.1, root) {
                return;
            }
        }

        // Reset data that's no longer valid
        self.asset = Some((tt, root));
        self.caret = 0;

        // Get the data out of the asset
        let object = (*data.apis.truth).read(tt, root);
        let buffer = (*data.apis.truth).get_buffer(tt, object, 0);

        let mut size = 0;
        let buffers = (*data.apis.truth).buffers(tt);
        let buffer_ptr = (*buffers).get.unwrap()((*buffers).inst, buffer.id, &mut size);

        let text_data = std::slice::from_raw_parts(buffer_ptr as *const u8, size as usize);
        self.text = String::from_utf8_lossy(text_data).to_string();

        // Trim carriage returns just in case git mangled the file
        self.text.retain(|v| v != '\r');

        // Get the title out of the asset
        self.title = title_from_asset(data, tt, root);

        // Set up code highlighting
        self.highlight_config = higlight_config_from_asset(data, tt, root);
        self.highlight();
    }

    pub fn apply_text_change(&mut self, data: &PluginData, change: TextChange) {
        match change {
            TextChange::Character(character) => {
                self.text.insert(self.caret, character);
                self.caret += 1;
            }
            TextChange::Backspace => {
                if self.caret >= 1 {
                    self.text.remove(self.caret - 1);
                    self.caret -= 1;
                } else {
                    // Can't backspace at start of file
                    return;
                }
            }
            TextChange::Delete => {
                if self.caret < self.text.len() {
                    self.text.remove(self.caret);
                } else {
                    // Can't delete at end of file
                    return;
                }
            }
        }

        // Re-highlight changed text
        self.highlight();

        // Save the changes to the asset
        self.commit_to_asset(data);
    }

    fn commit_to_asset(&self, data: &PluginData) {
        let (tt, asset) = if let Some(asset) = self.asset {
            asset
        } else {
            return;
        };

        let bytes = self.text.as_bytes();

        // Create a buffer holding the data
        unsafe {
            let buffers = (*data.apis.truth).buffers(tt);
            let buffer_ptr = (*buffers).allocate.unwrap()(
                (*buffers).inst,
                bytes.len() as u64,
                bytes.as_ptr() as *const c_void,
            );
            let buffer_id =
                (*buffers).add.unwrap()((*buffers).inst, buffer_ptr, bytes.len() as u64, 0);

            // Write the buffer to the truth data for the asset
            let object = (*data.apis.truth).write(tt, asset);
            (*data.apis.truth).set_buffer(tt, object, 0, buffer_id);
            (*data.apis.truth).commit(tt, object, TtUndoScopeT { u64_: 0 });
        }
    }

    fn highlight(&mut self) {
        if let Some(config) = &self.highlight_config {
            self.highlights = self
                .highlighter
                .highlight(config, &self.text.as_bytes(), None, |_| None)
                .unwrap()
                .map(|v| v.unwrap())
                .collect()
        } else {
            // No highlighting dummy event
            self.highlights = vec![HighlightEvent::Source {
                start: 0,
                end: self.text.len(),
            }];
        }
    }
}

pub enum CaretDirection {
    Left,
    Right,
    Up,
    Down,
}

pub enum TextChange {
    Character(char),
    Backspace,
    Delete,
}

unsafe fn title_from_asset(data: &PluginData, tt: *mut TheTruthO, root: TtIdT) -> CString {
    // Fetch the name for the asset
    let mut buffer = vec![0u8; 128];
    (*data.apis.properties_view).get_display_name(tt, root, buffer.as_mut_ptr() as *mut i8, 128);

    // Check if this asset type has an extension defined, if so add that
    let extension_i =
        (*data.apis.truth).get_aspect(tt, tt_id_type(root), TM_TT_ASPECT__FILE_EXTENSION)
            as *const c_char;

    if !extension_i.is_null() {
        let extension = CStr::from_ptr(extension_i);

        buffer.truncate(buffer.iter().position(|v| *v == 0).unwrap_or(128));
        buffer.push(b'.');
        buffer.extend_from_slice(extension.to_bytes());
    }

    CString::new(buffer).unwrap()
}

unsafe fn higlight_config_from_asset(
    data: &PluginData,
    tt: *mut TheTruthO,
    root: TtIdT,
) -> Option<HighlightConfiguration> {
    // Get the highlighting interface from the object
    let highlighting_i =
        (*data.apis.truth).get_aspect(tt, tt_id_type(root), ASPECT_ANODE_HIGHLIGHTING.hash)
            as *const AnodeHighlightingAspectI;

    if highlighting_i.is_null() {
        return None;
    }

    let interface = &*highlighting_i;

    // Load the config from the aspect
    let highlight_query =
        std::slice::from_raw_parts(interface.highlight_query, interface.highlight_query_len);
    let injection_query =
        std::slice::from_raw_parts(interface.injection_query, interface.injection_query_len);
    let locals_query =
        std::slice::from_raw_parts(interface.locals_query, interface.locals_query_len);

    let mut highlight_config = HighlightConfiguration::new(
        interface.language,
        std::str::from_utf8(&highlight_query).unwrap(),
        std::str::from_utf8(&injection_query).unwrap(),
        std::str::from_utf8(&locals_query).unwrap(),
    )
    .unwrap();
    let highlight_names: Vec<String> = data
        .token_colors
        .iter()
        .map(|v| v.scope.to_string())
        .collect();
    highlight_config.configure(&highlight_names);

    Some(highlight_config)
}
