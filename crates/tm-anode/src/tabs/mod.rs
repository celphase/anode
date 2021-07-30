pub mod code_editor;

use std::ffi::CString;

use dwrote::{FontCollection, FontDescriptor, FontStretch, FontStyle, FontWeight};
use machinery::{identifier, Identifier, RegistryStorage};
use machinery_api::{
    foundation::ApiRegistryApi,
    plugins::ui::{
        FontDescriptorT, FontProviderT, TabVt, TtfRangeT, TM_FONT_PROVIDER_INTERFACE_NAME,
        TM_TAB_VT_INTERFACE_NAME,
    },
};

pub fn register(registry: &ApiRegistryApi, storage: &mut RegistryStorage) -> *const TabVt {
    // Register the code editor tab
    let tab = unsafe {
        let code_editor_tab_vtable = code_editor::create_vtable();
        storage.add_implementation(
            registry,
            TM_TAB_VT_INTERFACE_NAME.as_ptr() as *const i8,
            code_editor_tab_vtable,
        ) as *const TabVt
    };

    // Register the monospace font
    unsafe {
        let range = TtfRangeT { start: 32, n: 95 };

        // Lookup the font using DirectWrite
        let font_collection = FontCollection::system();
        let desc = FontDescriptor {
            family_name: "Consolas".to_string(),
            weight: FontWeight::Regular,
            stretch: FontStretch::Normal,
            style: FontStyle::Normal,
        };
        let font = font_collection.get_font_from_descriptor(&desc).unwrap();
        let font_file = &font.create_font_face().get_files()[0];
        let font_path = font_file.get_font_file_path().unwrap();
        let font_path_c = CString::new(font_path.to_str().unwrap()).unwrap();

        let descriptor = FontDescriptorT {
            path: font_path_c.as_ptr(),
            ranges: storage.add(range),
            num_ranges: 1,
            ..Default::default()
        };
        storage.add(font_path_c);

        let provider = FontProviderT {
            font_id: ANODE_CODE_FONT.hash,
            font_size: 10,
            descriptor: storage.add(descriptor),
            ..Default::default()
        };

        storage.add_implementation(
            registry,
            TM_FONT_PROVIDER_INTERFACE_NAME.as_ptr() as *const i8,
            provider,
        );
    }

    tab
}

const ANODE_CODE_FONT: Identifier = identifier!("tm_anode_code_font");
