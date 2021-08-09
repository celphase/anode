use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
};

use eyre::{eyre, Result};
use machinery::{tt_id_eq, tt_id_type};
use machinery_api::{
    foundation::{TheTruthO, TtIdT, TtUndoScopeT, TM_TT_ASPECT__FILE_EXTENSION},
    plugins::editor_views::{AssetSaveI, TM_ASSET_SAVE_STATUS__SAVED},
};
use tm_anode_api::{AnodeAspectI, Highlighting, ASPECT_ANODE};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use crate::plugin::PluginData;

pub(crate) struct DocumentState {
    // Associated target asset
    asset: Option<(*mut TheTruthO, TtIdT, u32)>,

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

impl DocumentState {
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

    pub fn asset(&self) -> Option<(*mut TheTruthO, TtIdT, u32)> {
        self.asset
    }

    pub fn refresh_title(&mut self, data: &PluginData, save_interface: *mut AssetSaveI) -> &CStr {
        if let Some((tt, root, _property)) = self.asset {
            self.title = unsafe { title_from_asset(data, tt, root, save_interface) };
        } else {
            self.title = CString::new("untitled").unwrap();
        }

        self.title.as_c_str()
    }

    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn highlights(&self) -> &[HighlightEvent] {
        &self.highlights
    }

    pub fn caret_line_column(&self) -> (usize, usize) {
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

    pub unsafe fn load_from_asset(
        &mut self,
        data: &PluginData,
        tt: *mut TheTruthO,
        root: TtIdT,
    ) -> Result<()> {
        // If we've already got the same object, don't do anything
        if let Some(asset) = self.asset.as_ref() {
            if asset.0 == tt && tt_id_eq(asset.1, root) {
                return Ok(());
            }
        }

        // Get the aspect data out of the asset, which tells us how to open it
        let aspect_i = (*data.apis.truth).get_aspect(tt, tt_id_type(root), ASPECT_ANODE.hash)
            as *const AnodeAspectI;
        if aspect_i.is_null() {
            return Err(eyre!(
                "Asset does not have required tm_anode_aspect_i aspect"
            ));
        }

        // Reset data that's no longer valid
        let property = (*aspect_i).property;
        self.asset = Some((tt, root, property));
        self.caret = 0;

        // Get the data out of the asset
        let object = (*data.apis.truth).read(tt, root);
        let buffer = (*data.apis.truth).get_buffer(tt, object, property);

        let mut size = 0;
        let buffers = (*data.apis.truth).buffers(tt);
        let buffer_ptr = (*buffers).get.unwrap()((*buffers).inst, buffer.id, &mut size);

        let text_data = std::slice::from_raw_parts(buffer_ptr as *const u8, size as usize);
        self.text = String::from_utf8_lossy(text_data).to_string();

        // Trim carriage returns just in case git mangled the file
        self.text.retain(|v| v != '\r');

        // Set up code highlighting
        self.highlight_config = (*aspect_i)
            .highlighting
            .as_ref()
            .map(|v| higlight_config_from_raw(data, v));
        self.highlight();

        Ok(())
    }

    pub fn apply_input_left(&mut self, skip_word: bool) {
        if !skip_word {
            self.caret = (self.caret as i32 - 1).max(0) as usize;
        } else {
            let len = self.text.len();
            let mut iter = self.text.chars().rev().enumerate().skip(len - self.caret);

            // Skip to start of word
            for (i, c) in &mut iter {
                self.caret = len - i;

                if c.is_alphanumeric() {
                    break;
                }
            }

            // Skip to end of word
            for (i, c) in iter {
                self.caret = len - i;

                if !c.is_alphanumeric() {
                    break;
                }
            }
        }

        self.set_caret_column_to_current();
    }

    pub fn apply_input_right(&mut self, skip_word: bool) {
        if !skip_word {
            self.caret = (self.caret + 1).min(self.text.len());
        } else {
            let mut iter = self.text.chars().enumerate().skip(self.caret);

            // Skip to start of word
            for (i, c) in &mut iter {
                self.caret = i;

                if c.is_alphanumeric() {
                    break;
                }
            }

            // Skip to end of word
            for (i, c) in iter {
                self.caret = i;

                if !c.is_alphanumeric() {
                    break;
                }
            }
        }

        self.set_caret_column_to_current();
    }

    pub fn apply_input_up(&mut self) {
        let (line, _) = self.caret_line_column();
        if line > 0 {
            self.set_caret_line_column(line - 1, self.caret_column);
        } else {
            // If we're already on the first line, force to top
            self.caret = 0;
        }

        self.set_caret_column_to_current();
    }

    pub fn apply_input_down(&mut self) {
        let (line, _) = self.caret_line_column();
        let lines = self.text.split('\n').count();
        if line < lines - 1 {
            self.set_caret_line_column(line + 1, self.caret_column);
        } else {
            // If we're already on the last line, force to end
            self.caret = self.text.len();
        }

        self.set_caret_column_to_current();
    }

    pub fn apply_input_character(&mut self, data: &PluginData, character: char) {
        self.text.insert(self.caret, character);
        self.caret += 1;

        if character != '\n' {
            self.caret_column += 1;
        } else {
            self.caret_column = 0;
        }

        self.highlight();
        self.commit_to_asset(data);
    }

    pub fn apply_input_backspace(&mut self, data: &PluginData) {
        if self.caret == 0 {
            // Can't backspace at start of file
            return;
        }

        let removed = self.text.remove(self.caret - 1);
        self.caret -= 1;

        if removed != '\n' {
            self.caret_column -= 1;
        } else {
            self.set_caret_column_to_current();
        }

        self.highlight();
        self.commit_to_asset(data);
    }

    pub fn apply_input_delete(&mut self, data: &PluginData) {
        if self.caret == self.text.len() {
            // Can't delete at end of file
            return;
        }

        self.text.remove(self.caret);

        self.highlight();
        self.commit_to_asset(data);
    }

    pub fn apply_input_tab(&mut self, data: &PluginData) {
        // Pad to the nearest 4
        let (_, column) = self.caret_line_column();
        let count = 4 - (column % 4);
        for _ in 0..count {
            self.text.insert(self.caret, ' ');
        }
        self.caret += count;

        self.highlight();
        self.commit_to_asset(data);
    }

    fn highlight(&mut self) {
        if let Some(config) = &self.highlight_config {
            self.highlights = self
                .highlighter
                .highlight(config, self.text.as_bytes(), None, |_| None)
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

    fn commit_to_asset(&self, data: &PluginData) {
        let (tt, asset, property) = if let Some(asset) = self.asset {
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
            (*data.apis.truth).set_buffer(tt, object, property, buffer_id);
            (*data.apis.truth).commit(tt, object, TtUndoScopeT { u64_: 0 });
        }
    }
}

unsafe fn title_from_asset(
    data: &PluginData,
    tt: *mut TheTruthO,
    root: TtIdT,
    save_interface: *mut AssetSaveI,
) -> CString {
    // Fetch the name for the asset
    let mut buffer = vec![0u8; 128];
    (*data.apis.properties_view).get_display_name(tt, root, buffer.as_mut_ptr() as *mut i8, 128);

    // Check if this asset type has an extension defined, if so add that
    let extension_i =
        (*data.apis.truth).get_aspect(tt, tt_id_type(root), TM_TT_ASPECT__FILE_EXTENSION)
            as *const c_char;
    buffer.truncate(buffer.iter().position(|v| *v == 0).unwrap_or(128));

    if !extension_i.is_null() {
        let extension = CStr::from_ptr(extension_i);

        buffer.push(b'.');
        buffer.extend_from_slice(extension.to_bytes());

        // Add a star if unsaved
        let owner = (*data.apis.truth).owner(tt, root);
        let is_unsaved = (*save_interface).status.unwrap()((*save_interface).inst, owner)
            != TM_ASSET_SAVE_STATUS__SAVED;
        if is_unsaved {
            buffer.push(b'*');
        }
    }

    CString::new(buffer).unwrap()
}

unsafe fn higlight_config_from_raw(
    data: &PluginData,
    highlighting: &Highlighting,
) -> HighlightConfiguration {
    // Load the config from the aspect
    let highlight_query = std::slice::from_raw_parts(
        highlighting.highlight_query,
        highlighting.highlight_query_len,
    );
    let injection_query = std::slice::from_raw_parts(
        highlighting.injection_query,
        highlighting.injection_query_len,
    );
    let locals_query =
        std::slice::from_raw_parts(highlighting.locals_query, highlighting.locals_query_len);

    let mut highlight_config = HighlightConfiguration::new(
        std::mem::transmute(highlighting.language),
        std::str::from_utf8(highlight_query).unwrap(),
        std::str::from_utf8(injection_query).unwrap(),
        std::str::from_utf8(locals_query).unwrap(),
    )
    .unwrap();
    let highlight_names: Vec<String> = data
        .token_colors
        .iter()
        .map(|v| v.scope.to_string())
        .collect();
    highlight_config.configure(&highlight_names);

    highlight_config
}
