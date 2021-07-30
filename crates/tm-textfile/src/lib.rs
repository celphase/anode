use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    sync::Mutex,
};

use const_cstr::const_cstr;
use machinery::{
    export_singleton_fns, get_api, identifier, plugin, Identifier, Plugin, RegistryStorage,
    Singleton,
};
use machinery_api::{
    foundation::{
        ApiRegistryApi, ApplicationO, TheTruthApi, TheTruthO, TheTruthPropertyDefinitionT, TtIdT,
        TtUndoScopeT, UiO, TM_THE_TRUTH_CREATE_TYPES_INTERFACE_NAME,
        TM_THE_TRUTH_PROPERTY_TYPE_BUFFER, TM_TT_ASPECT__FILE_EXTENSION,
    },
    plugins::{
        editor_views::{
            AssetBrowserCreateAssetI, AssetBrowserCreateAssetO,
            TM_ASSET_BROWSER_CREATE_ASSET_INTERFACE_NAME,
        },
        the_machinery_shared::{AssetOpenAspectI, AssetOpenMode, TM_TT_ASPECT__ASSET_OPEN},
        ui::{DockingFindTabOptT, TabI},
    },
};
use tm_anode_api::{AnodeApi, AnodeAspectI, ASPECT_ANODE};

plugin!(TextFilePlugin);

#[derive(Singleton)]
struct TextFilePlugin {
    registry: *const ApiRegistryApi,
    registry_storage: Mutex<RegistryStorage>,
    truth: *const TheTruthApi,
    anode: *const AnodeApi,
}

unsafe impl Send for TextFilePlugin {}
unsafe impl Sync for TextFilePlugin {}

impl Plugin for TextFilePlugin {
    fn load(registry: *const ApiRegistryApi) -> Self {
        unsafe {
            let mut registry_storage = RegistryStorage::new();

            // Register type creation in the truth
            registry_storage.add_raw_implementation(
                &*registry,
                TM_THE_TRUTH_CREATE_TYPES_INTERFACE_NAME.as_ptr() as *const i8,
                Self::truth_create_types as *const c_void,
            );

            // Register the asset creation menu item
            let create_asset = AssetBrowserCreateAssetI {
                inst: null_mut(),
                menu_name: const_cstr!("New Text File").as_ptr(),
                asset_name: const_cstr!("textfile").as_ptr(),
                create: Some(TextFilePlugin::create_asset),
            };

            registry_storage.add_implementation(
                &*registry,
                TM_ASSET_BROWSER_CREATE_ASSET_INTERFACE_NAME.as_ptr() as *const i8,
                create_asset,
            );

            Self {
                registry,
                registry_storage: Mutex::new(registry_storage),
                truth: get_api(&*registry),
                anode: get_api(&*registry),
            }
        }
    }
}

impl Drop for TextFilePlugin {
    fn drop(&mut self) {
        unsafe {
            self.registry_storage.lock().unwrap().clear(&*self.registry);
        }
    }
}

#[export_singleton_fns]
impl TextFilePlugin {
    unsafe fn truth_create_types(&self, tt: *mut TheTruthO) {
        // Create the truth type for the asset
        let properties = vec![TheTruthPropertyDefinitionT {
            name: const_cstr!("data").as_ptr(),
            type_: TM_THE_TRUTH_PROPERTY_TYPE_BUFFER as u32,
            ..Default::default()
        }];

        let asset_type = (*self.truth).create_object_type(
            tt,
            TEXTFILE_ASSET.name.as_ptr(),
            properties.as_ptr(),
            properties.len() as u32,
        );

        // Mark this object type as an asset
        (*self.truth).set_aspect(
            tt,
            asset_type,
            TM_TT_ASPECT__FILE_EXTENSION,
            const_cstr!("txt").as_ptr() as *const c_void,
        );

        // Mark this object type as openable
        let mut registry_storage = self.registry_storage.lock().unwrap();
        let open_i = registry_storage.add(AssetOpenAspectI {
            open: Some(TextFilePlugin::open_asset),
        });
        (*self.truth).set_aspect(
            tt,
            asset_type,
            TM_TT_ASPECT__ASSET_OPEN,
            open_i as *const c_void,
        );

        // Register anode for this asset
        let anode = registry_storage.add(AnodeAspectI {
            property: 0,
            highlighting: null(),
        });
        (*self.truth).set_aspect(tt, asset_type, ASPECT_ANODE.hash, anode as *const c_void);
    }

    unsafe fn create_asset(
        &self,
        _inst: *mut AssetBrowserCreateAssetO,
        tt: *mut TheTruthO,
        undo_scope: TtUndoScopeT,
    ) -> TtIdT {
        let asset_type = (*self.truth).object_type_from_name_hash(tt, TEXTFILE_ASSET.hash);
        let asset = (*self.truth).create_object_of_type(tt, asset_type, undo_scope);

        // Default value
        let value = "".as_bytes();

        // Create a buffer holding the data
        let buffers = (*self.truth).buffers(tt);
        let buffer_ptr = (*buffers).allocate.unwrap()(
            (*buffers).inst,
            value.len() as u64,
            value.as_ptr() as *const c_void,
        );
        let buffer_id = (*buffers).add.unwrap()((*buffers).inst, buffer_ptr, value.len() as u64, 0);

        // Write the buffer to the truth data for the asset
        let object = (*self.truth).write(tt, asset);
        (*self.truth).set_buffer(tt, object, 0, buffer_id);
        (*self.truth).commit(tt, object, undo_scope);

        asset
    }

    unsafe fn open_asset(
        &self,
        app: *mut ApplicationO,
        ui: *mut UiO,
        from_tab: *mut TabI,
        tt: *mut TheTruthO,
        asset: TtIdT,
        _open_mode: AssetOpenMode,
    ) {
        let opt = DockingFindTabOptT {
            from_tab,
            in_ui: ui,
            find_asset_tt: tt,
            find_asset: asset,
            ..Default::default()
        };
        ((*self.anode).open_asset)(app, &opt);
    }
}

pub const TEXTFILE_ASSET: Identifier = identifier!("tm_textfile_asset");
