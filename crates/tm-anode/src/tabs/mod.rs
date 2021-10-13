pub mod code_editor;

use const_cstr::const_cstr;

use machinery::RegistryStorage;
use machinery_api::{
    foundation::ApiRegistryApi,
    plugins::ui::{TabVt, TM_TAB_VT_VERSION},
};

pub fn register(registry: &ApiRegistryApi, registry_storage: &mut RegistryStorage) -> *const TabVt {
    unsafe {
        // Register the code editor tab
        let code_editor_tab_vtable = code_editor::create_vtable();
        registry_storage.add_implementation(
            registry,
            const_cstr!("tm_tab_vt").as_ptr(),
            TM_TAB_VT_VERSION,
            code_editor_tab_vtable,
        ) as *const TabVt
    }
}
