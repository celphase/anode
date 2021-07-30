use std::{
    mem::size_of,
    ptr::null,
    sync::{Arc, Mutex},
};

use machinery::{
    export_singleton_fns, get_api, plugin, tt_id_eq, Plugin, RegistryStorage, Singleton,
};
use machinery_api::{
    foundation::{ApiRegistryApi, ApplicationO, TheTruthApi, TheTruthO, TtIdT},
    plugins::{
        editor_views::PropertiesViewApi,
        ui::{DockingApi, DockingFindTabOptT, Draw2dApi, FontApi, TabI, TabVt, UiApi},
    },
    the_machinery::TheMachineryApi,
    Api,
};
use tm_anode_api::AnodeApi;
use tracing::{event, Level};

use crate::{hex_token_color, tabs::code_editor::ANODE_CODE_EDITOR_TAB, TokenColor};

plugin!(AnodePlugin);

#[derive(Singleton)]
pub(crate) struct AnodePlugin {
    // TODO: Make not-public and expose public API instead.
    pub data: Arc<PluginData>,
}

impl Plugin for AnodePlugin {
    fn load(registry: *const ApiRegistryApi) -> Self {
        let registry = unsafe { &*registry };

        machinery::tracing::initialize(registry);
        event!(Level::INFO, "Loading anode.");

        let mut registry_storage = RegistryStorage::new();

        let code_editor_tab_vtable = crate::tabs::register(registry, &mut registry_storage);
        crate::fonts::register(registry, &mut registry_storage);

        let token_colors = vec![
            hex_token_color("comment", 0x6A9955FF),
            hex_token_color("function", 0xDCDCAAFF),
            hex_token_color("string", 0xCE9178FF),
            hex_token_color("number", 0xB5CEA8FF),
            hex_token_color("type", 0x4EC9B0FF),
            hex_token_color("variable", 0x9CDCFEFF),
            hex_token_color("property", 0x9CDCFEFF),
            hex_token_color("keyword", 0x569CD6FF),
        ];

        let apis = Apis {
            registry,
            truth: get_api(registry),
            ui: get_api(registry),
            docking: get_api(registry),
            draw2d: get_api(registry),
            font: get_api(registry),
            properties_view: get_api(registry),
            machinery: get_api(registry),
            code_editor_tab_vtable,
        };

        // Register the public API
        unsafe {
            let api = registry_storage.add(AnodeApi {
                open_asset: Self::open_asset,
            });
            registry.set(
                AnodeApi::NAME.as_ptr(),
                api as *const _,
                size_of::<AnodeApi>() as u32,
            );
        }

        let data = PluginData {
            apis,
            registry_storage: Mutex::new(registry_storage),
            token_colors,
        };

        Self {
            data: Arc::new(data),
        }
    }
}

impl Drop for AnodePlugin {
    fn drop(&mut self) {
        event!(Level::INFO, "Unloading anode.");

        unsafe {
            self.data
                .registry_storage
                .lock()
                .unwrap()
                .clear(&*self.data.apis.registry);
        }
    }
}

#[export_singleton_fns]
impl AnodePlugin {
    unsafe fn open_asset(&self, app: *mut ApplicationO, opt: *const DockingFindTabOptT) {
        // Try to find an existing tab
        let mut tab = (*self.data.apis.docking)
            .find_tab(ANODE_CODE_EDITOR_TAB.hash, opt)
            .tab;

        // If we couldn't find a tab that's already open with this script, create one
        if tab.is_null() || !is_open_in(&*tab, (*opt).find_asset_tt, (*opt).find_asset) {
            tab = (*self.data.apis.machinery).create_or_select_tab(
                app,
                (*opt).in_ui,
                ANODE_CODE_EDITOR_TAB.name.as_ptr(),
                null(),
            );
        }

        // Focus the tab and tell it to (re)open the file
        (*self.data.apis.docking).set_focus_tab((*opt).in_ui, tab);
        (*(*tab).vt).set_root.unwrap()((*tab).inst, (*opt).find_asset_tt, (*opt).find_asset);
    }
}

pub(crate) struct PluginData {
    pub apis: Apis,
    pub registry_storage: Mutex<RegistryStorage>,
    pub token_colors: Vec<TokenColor>,
}

pub struct Apis {
    pub registry: *const ApiRegistryApi,
    pub truth: *const TheTruthApi,
    pub ui: *const UiApi,
    pub docking: *const DockingApi,
    pub draw2d: *const Draw2dApi,
    pub font: *const FontApi,
    pub properties_view: *const PropertiesViewApi,
    pub machinery: *const TheMachineryApi,
    pub code_editor_tab_vtable: *const TabVt,
}

unsafe impl Send for Apis {}
unsafe impl Sync for Apis {}

pub unsafe fn is_open_in(tab: &TabI, tt: *mut TheTruthO, asset: TtIdT) -> bool {
    let root = (*tab.vt).root.unwrap()(tab.inst);
    root.tt == tt && tt_id_eq(root.root, asset)
}
