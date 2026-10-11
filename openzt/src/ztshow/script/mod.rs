pub mod item;
pub mod mgr;
#[allow(clippy::module_inception)]
pub mod script;

pub use item::*;
pub use mgr::*;
#[cfg(test)]
pub(crate) use script::test_item;

pub fn init() {
    ztshowscriptmgr_detours::init();
    ztshowscript_detours::init();
}

mod ztshowscriptmgr_detours {
    use openzt_detour::generated::{
        ztshowscript::SAVE as SAVE_SCRIPT_OLD,
        ztshowscriptmgr::{CLEAR_ALL_SCRIPTS, GET_SCRIPT, LOAD, REGISTER_SCRIPT, SAVE, UNREGISTER_SCRIPT},
    };
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(REGISTER_SCRIPT)]
        unsafe extern "thiscall" fn register_script(_this: *const u32, script: *const u32) -> u32 {
            if script.is_null() {
                return 0;
            }
            let script_type = crate::util::get_from_memory::<u32>(script as u32 + 0xc);
            match super::super::mgr::register_script(script as u32, script_type) {
                Some(id) => {
                    crate::util::save_to_memory::<u16>(script as u32 + 4, id);
                    1
                }
                None => 0,
            }
        }

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(_this: *const u32, file: *const i8) -> bool {
            super::super::mgr::save_mgr(file as *const u32)
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(_this: *const u32, file: *const u32, version: u32) -> bool {
            super::super::mgr::load_mgr(file, version)
        }

        #[detour(GET_SCRIPT)]
        unsafe extern "thiscall" fn get_script(_this: *const u32, id: u16) -> u32 {
            super::super::mgr::get_script(id)
        }

        #[detour(UNREGISTER_SCRIPT)]
        unsafe extern "thiscall" fn unregister_script(_this: *const u32, script: *const u32) -> u32 {
            super::super::mgr::unregister_script(script as u32) as u32
        }

        #[detour(CLEAR_ALL_SCRIPTS)]
        unsafe extern "thiscall" fn clear_all_scripts(_this: *const u32) -> u32 {
            super::super::mgr::clear_all_scripts() as u32
        }

        #[detour(SAVE_SCRIPT_OLD)]
        unsafe extern "thiscall" fn save_script_old(this: *const u32, file: *const i8) -> bool {
            super::super::mgr::save_script(this as u32, file as *const u32)
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztshowscriptmgr detours: {e:?}");
        }
    }
}

mod ztshowscript_detours {
    use openzt_detour::generated::ztshowscript::{ADD_ITEM, CLEAR_ALL, GET_ITEM, GET_ITEM_BY_TRICK_ID, LOAD, REMOVE_ITEM, SIZE};
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(SIZE)]
        unsafe extern "thiscall" fn size(this: *const u32) -> i32 {
            super::super::mgr::size(this as u32)
        }

        #[detour(CLEAR_ALL)]
        unsafe extern "thiscall" fn clear_all(this: *const u32) {
            super::super::mgr::clear_all(this as u32);
        }

        #[detour(GET_ITEM)]
        unsafe extern "thiscall" fn get_item(this: *const u32, index: u16) -> u32 {
            super::super::mgr::get_item(this as u32, index)
        }

        #[detour(GET_ITEM_BY_TRICK_ID)]
        unsafe extern "thiscall" fn get_item_by_trick_id(this: *const u32, trick_id: u16) -> u32 {
            super::super::mgr::get_item_by_trick_id(this as u32, trick_id)
        }

        #[detour(REMOVE_ITEM)]
        unsafe extern "thiscall" fn remove_item(this: *const u32, index: u16) -> u32 {
            super::super::mgr::remove_item(this as u32, index) as u32
        }

        #[allow(improper_ctypes_definitions)]
        #[detour(ADD_ITEM)]
        unsafe extern "thiscall" fn add_item(this: *const u32, item: [u8; 0x7c]) -> u32 {
            let item = unsafe { &*(item.as_ptr() as *const super::super::item::ZTShowScriptItemRaw) };
            super::super::mgr::add_item(this as u32, item)
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            let Some((id, script)) = super::super::script::read_script(file, version) else { return false };
            let mut state = super::super::mgr::STATE.lock().unwrap();
            state.aliases.insert(this as u32, id);
            state.scripts.insert(id, script);
            crate::util::save_to_memory::<u16>(this as u32 + 4, id);
            true
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztshowscript detours: {e:?}");
        }
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) fn reset_state() {
        clear_all_scripts();
        STATE.lock().unwrap().next_id_counter = 0;
    }

    pub(crate) fn registered_script_count() -> usize {
        STATE.lock().unwrap().scripts.len()
    }

    pub(crate) fn all_script_ids() -> Vec<u16> {
        STATE.lock().unwrap().scripts.keys().copied().collect()
    }

    pub(crate) fn snapshot_encoded() -> Vec<u8> {
        mgr::encode_mgr(&STATE.lock().unwrap())
    }

    pub(crate) fn next_id_counter() -> u16 {
        STATE.lock().unwrap().next_id_counter
    }

    pub(crate) fn raw_item_matching_type(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: 0,
            visible: 1,
            id: trick_id,
            item_type,
            sentinel: 0xffff_ffff,
            name: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            anim: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_pre_trick: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_post_trick: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            building: 0,
            complexity: 1,
            return_to_keeper: 0,
            _pad: [0; 3],
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            grayed_icon: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
        }
    }

    pub(crate) fn raw_item_with_mirror(item_type: u32, trick_id: u16, satisfaction: u32, satisfaction_mirror: u32) -> ZTShowScriptItemRaw {
        let mut item = raw_item_matching_type(item_type, trick_id);
        item.satisfaction = satisfaction;
        item.satisfaction_mirror = satisfaction_mirror;
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::script::*;

    #[test]
    fn next_script_id_wraps_at_0xffff_not_0x10000() {
        let mut counter = 0xfffeu16;
        assert_eq!(next_script_id(&mut counter), 0xffff % 0xffff);
        assert_eq!(counter, 0xffff);
        assert_eq!(next_script_id(&mut counter), 0);
        assert_eq!(counter, 0);
    }

    #[test]
    fn register_get_unregister_roundtrip() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        let id = register_script(0x1000, 42).unwrap();
        let handle = get_script(id);
        assert_ne!(handle, 0);
        assert_eq!(size(handle), 0);
        assert_eq!(size(0x1000), 0);
        assert!(unregister_script(0x1000));
        assert_eq!(get_script(id), 0, "unregistered id should no longer resolve via getScript");
    }

    #[test]
    fn register_script_rejects_null() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        assert_eq!(register_script(0, 1), None);
    }

    #[test]
    fn add_item_only_inserts_matching_type() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        let _id = register_script(0x2000, 7).unwrap();
        let matching = raw_item(7, 5);
        let mismatched = raw_item(9, 6);
        assert_eq!(add_item(0x2000, &matching), 0);
        assert_eq!(add_item(0x2000, &mismatched), 0);
        assert_eq!(size(0x2000), 1, "only the type-matching item should have been inserted");
        let item_ptr = get_item(0x2000, 0);
        assert_ne!(item_ptr, 0, "found item should return a real, non-null pointer, not a boolean sentinel");
        let raw = unsafe { &*(item_ptr as *const ZTShowScriptItemRaw) };
        assert_eq!(raw.id, 5);
        assert_eq!(raw.item_type, 7);
        assert_eq!(get_item(0x2000, 1), 0, "out-of-range index should return 0");
        let found_ptr = get_item_by_trick_id(0x2000, 5);
        assert_ne!(found_ptr, 0, "found item should return a real, non-null pointer, not a boolean sentinel");
        let raw = unsafe { &*(found_ptr as *const ZTShowScriptItemRaw) };
        assert_eq!(raw.id, 5);
        assert_eq!(raw.item_type, 7);
        assert_eq!(get_item_by_trick_id(0x2000, 6), 0, "not found should return 0, matching vanilla's real getItemByTrickID");
    }

    #[test]
    fn remove_and_clear_items() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        register_script(0x3000, 1).unwrap();
        add_item(0x3000, &raw_item(1, 1));
        add_item(0x3000, &raw_item(1, 2));
        assert_eq!(size(0x3000), 2);
        assert!(remove_item(0x3000, 0));
        assert_eq!(size(0x3000), 1);
        assert!(!remove_item(0x3000, 5), "out-of-range index should fail");
        clear_all(0x3000);
        assert_eq!(size(0x3000), 0);
    }

    #[test]
    fn encode_decode_item_roundtrip_matches_wire_format() {
        let mut item = ShowScriptItem::default();
        item.id = 7;
        item.item_type = 3;
        item.name = "trick".to_string();
        item.satisfaction = 99;
        let bytes = encode_item(&item);
        assert_eq!(bytes[0], item.default_available as u8);
        assert_eq!(bytes[1], item.visible as u8);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), item.id);
        assert_eq!(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), item.item_type);
        assert_eq!(u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]), item.sentinel);
        let name_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        assert_eq!(name_len, item.name.len());
        assert_eq!(&bytes[16..16 + name_len], item.name.as_bytes());
    }

    #[test]
    fn encode_mgr_orders_scripts_ascending_by_id() {
        let mut state = mgr::ShowScriptMgrState::default();
        state.scripts.insert(5, ShowScriptData { sentinel: 0xffff_ffff, script_type: 1, items: Vec::new() });
        state.scripts.insert(2, ShowScriptData { sentinel: 0xffff_ffff, script_type: 1, items: Vec::new() });
        state.next_id_counter = 5;
        let bytes = mgr::encode_mgr(&state);
        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 2, "script count");
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 2);
    }

    fn raw_item(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: 0,
            visible: 1,
            id: trick_id,
            item_type,
            sentinel: 0xffff_ffff,
            name: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            anim: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_pre_trick: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_post_trick: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            building: 0,
            complexity: 1,
            return_to_keeper: 0,
            _pad: [0; 3],
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
            grayed_icon: crate::util::ZTBufferString::from_raw_parts(0, 0, 0),
        }
    }

    #[test]
    fn encode_item_truncates_overlong_strings_at_string_length_cap() {
        let mut item = ShowScriptItem::default();
        item.name = "x".repeat(STRING_LENGTH_CAP as usize + 50);
        let bytes = encode_item(&item);
        let name_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        assert_eq!(name_len, (STRING_LENGTH_CAP - 1) as usize, "encoded length prefix should be truncated to STRING_LENGTH_CAP - 1");
        assert_eq!(&bytes[16..16 + name_len], "x".repeat(name_len).as_bytes());
    }
}
