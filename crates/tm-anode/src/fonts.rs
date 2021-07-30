use std::ffi::CString;

use dwrote::{FontCollection, FontDescriptor, FontStretch, FontStyle, FontWeight};
use machinery::{identifier, Identifier, RegistryStorage};
use machinery_api::{
    foundation::ApiRegistryApi,
    plugins::ui::{FontDescriptorT, FontProviderT, TtfRangeT, TM_FONT_PROVIDER_INTERFACE_NAME},
};

pub fn register(registry: &ApiRegistryApi, registry_storage: &mut RegistryStorage) {
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
            ranges: registry_storage.add(range),
            num_ranges: 1,
            ..Default::default()
        };
        registry_storage.add(font_path_c);

        let provider = FontProviderT {
            font_id: ANODE_CODE_FONT.hash,
            font_size: 10,
            descriptor: registry_storage.add(descriptor),
            ..Default::default()
        };

        registry_storage.add_implementation(
            registry,
            TM_FONT_PROVIDER_INTERFACE_NAME.as_ptr() as *const i8,
            provider,
        );
    }
}

pub const ANODE_CODE_FONT: Identifier = identifier!("tm_anode_code_font");
