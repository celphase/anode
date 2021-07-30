use const_cstr::{const_cstr, ConstCStr};
use machinery::{identifier, Identifier};
use machinery_api::{foundation::ApplicationO, plugins::ui::DockingFindTabOptT, Api};

/// Anode editor API.
#[repr(C)]
pub struct AnodeApi {
    /// Open an asset in an editor tab.
    ///
    /// Creates a new tab with this asset's contents open, or focuses the existing tab if one
    /// already exists.
    pub open_asset: unsafe extern "C" fn(app: *mut ApplicationO, opt: *const DockingFindTabOptT),
}

impl Api for AnodeApi {
    const NAME: ConstCStr = const_cstr!("tm_anode_api");
}

/// Aspect for assets opened in an anode editor.
pub struct AnodeAspectI {
    /// Anode expects the asset's data to be a UTF-8 buffer **without nul terminator**.
    pub property: u32,
    /// Highlighting language description.
    pub highlighting: *const Highlighting,
}

pub const ASPECT_ANODE: Identifier = identifier!("tm_anode_aspect_i");

/// Highlighting language description.
///
/// Highlighting is provided by tree-sitter, see [tree-sitter's documentation][1] on how to define
/// new languages. String values are expected to be in UTF-8 arrays **without nul terminator**.
///
/// [1]: https://tree-sitter.github.io
pub struct Highlighting {
    pub language: *const TSLanguage,
    pub highlight_query: *const u8,
    pub highlight_query_len: usize,
    pub injection_query: *const u8,
    pub injection_query_len: usize,
    pub locals_query: *const u8,
    pub locals_query_len: usize,
}

/// Opaque raw TSLanguage.
///
/// A pointer to this can be transmuted to and from tree-sitter's `Language`.
/// This is provided so that tree-sitter doesn't have to be depended on directly.
#[repr(C)]
pub struct TSLanguage {
    _opaque: [u8; 0],
}
