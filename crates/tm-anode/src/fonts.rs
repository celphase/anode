use std::ffi::CString;

use const_cstr::const_cstr;
use font_kit::{
    family_name::FamilyName, handle::Handle, properties::Properties, source::SystemSource,
};
use machinery::{identifier, Identifier, RegistryStorage};
use machinery_api::{
    foundation::ApiRegistryApi,
    plugins::ui::{FontDescriptorT, FontProviderT, TtfRangeT, TM_FONT_PROVIDER_T_VERSION},
};

pub fn register(registry: &ApiRegistryApi, registry_storage: &mut RegistryStorage) {
    unsafe {
        let range = TtfRangeT { start: 32, n: 95 };

        // Lookup the font using font-kit
        let source = SystemSource::new();
        let font = source
            .select_best_match(
                &[
                    FamilyName::Title("Consolas".to_string()),
                    FamilyName::Monospace,
                ],
                &Properties::new(),
            )
            .unwrap();
        let font_path = if let Handle::Path { path, .. } = font {
            path
        } else {
            panic!("Unable to locate any font.");
        };

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
            const_cstr!("tm_font_provider_t").as_ptr(),
            TM_FONT_PROVIDER_T_VERSION,
            provider,
        );
    }
}

pub const ANODE_CODE_FONT: Identifier = identifier!("tm_anode_code_font");
