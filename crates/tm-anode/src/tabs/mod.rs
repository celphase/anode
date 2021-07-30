pub mod code_editor;

use machinery::RegistryStorage;
use machinery_api::{
    foundation::ApiRegistryApi,
    plugins::ui::{TabVt, TM_TAB_VT_INTERFACE_NAME},
};

pub fn register(registry: &ApiRegistryApi, registry_storage: &mut RegistryStorage) -> *const TabVt {
    unsafe {
        // Register the code editor tab
        let code_editor_tab_vtable = code_editor::create_vtable();
        registry_storage.add_implementation(
            registry,
            TM_TAB_VT_INTERFACE_NAME.as_ptr() as *const i8,
            code_editor_tab_vtable,
        ) as *const TabVt
    }
}
