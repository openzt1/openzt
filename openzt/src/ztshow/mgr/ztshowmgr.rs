use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};

use crate::bfconfigfile;
use crate::globals::get_module_base;
use crate::util::get_from_memory;
use openzt_detour::generated::bfapp::GET_INSTALLED_EXPANSION;
use openzt_detour::generated::standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE};
use openzt_detour::generated::ztshow::CLEAR_SHOW_SCRIPT_STATES;
use openzt_detour::generated::ztshowscriptmgr::{LOAD as ZTSHOWSCRIPTMGR_LOAD, SAVE as ZTSHOWSCRIPTMGR_SAVE};

pub(crate) const GLOBAL_ZTAPP_RVA: u32 = 0x00638154 - 0x400000;
pub(crate) const SHOW_ID_COUNTER_RVA: u32 = 0x0063e480 - 0x400000;
pub(crate) const ZTSHOWMGR_VTABLE_RVA: u32 = 0x00635120 - 0x400000;
pub(crate) const ZTSHOWSCRIPTMGR_VTABLE_RVA: u32 = 0x00630d28 - 0x400000;

#[repr(C)]
pub struct ZTShowScriptMgrSlot {
    pub vtable: u32,
    pub tree_header: u32,
    pub tree_size: u32,
    pub tag_byte: u8,
    _pad_0x41: [u8; 3],
}

const _: () = assert!(std::mem::size_of::<ZTShowScriptMgrSlot>() == 0x10);

#[repr(C)]
pub struct ZTShowMgr {
    pub vtable: u32,
    pub field_0x4: u8,
    _pad_0x5: [u8; 3],
    pub threshold_a: u32,
    pub threshold_b: u32,
    pub threshold_c: u32,
    pub bad_show: u32,
    pub good_show: u32,
    pub great_show: u32,
    pub min_ideal_length: u32,
    pub max_ideal_length: u32,
    pub tree_header: u32,
    pub tree_size: u32,
    pub tag_byte: u8,
    _pad_0x31: [u8; 3],
    pub show_script_mgr: ZTShowScriptMgrSlot,
}

const _: () = assert!(std::mem::size_of::<ZTShowMgr>() == 0x44);

impl ZTShowMgr {
    pub fn construct(&mut self) -> &mut Self {
        let base = get_module_base("zoo.exe") as u32;
        let this_addr = self as *const ZTShowMgr as u32;

        self.vtable = base + ZTSHOWMGR_VTABLE_RVA;
        self.field_0x4 = 0;
        self.threshold_a = 0;
        self.threshold_b = 3;
        self.threshold_c = 6;
        self.bad_show = 0x19;
        self.good_show = 0x32;
        self.great_show = 0x4b;
        self.min_ideal_length = 6;
        self.max_ideal_length = 6;
        self.tree_header = 0;
        self.tree_size = 0;
        self.tag_byte = (this_addr >> 24) as u8;

        self.show_script_mgr.vtable = base + ZTSHOWSCRIPTMGR_VTABLE_RVA;
        self.show_script_mgr.tree_header = 0;
        self.show_script_mgr.tree_size = 0;
        self.show_script_mgr.tag_byte = (this_addr >> 24) as u8;

        self
    }

    pub fn init_show_params(&mut self) -> u32 {
        self.threshold_a = 0;
        self.threshold_b = 3;
        self.threshold_c = 6;
        self.bad_show = 0x19;
        self.good_show = 0x32;
        self.great_show = 0x4b;
        self.min_ideal_length = 6;
        self.max_ideal_length = 6;

        let base = get_module_base("zoo.exe") as u32;
        let ztapp_ptr: u32 = get_from_memory(base + GLOBAL_ZTAPP_RVA);
        let expansion_2_installed = ztapp_ptr != 0 && unsafe { GET_INSTALLED_EXPANSION.original()(ztapp_ptr as *const u32, 2) };

        if expansion_2_installed
            && let Some(ini) = bfconfigfile::ini_compat::read_cfg("shows.cfg")
        {
            use bfconfigfile::ini_compat::first_parse;
            self.threshold_a = first_parse(&ini, "trickSatisfactionThresholds", "badTrick").unwrap_or(self.threshold_a);
            self.threshold_b = first_parse(&ini, "trickSatisfactionThresholds", "goodTrick").unwrap_or(self.threshold_b);
            self.threshold_c = first_parse(&ini, "trickSatisfactionThresholds", "greatTrick").unwrap_or(self.threshold_c);
            self.min_ideal_length = first_parse(&ini, "show", "minIdealLength").unwrap_or(self.min_ideal_length);
            self.max_ideal_length = first_parse(&ini, "show", "maxIdealLength").unwrap_or(self.max_ideal_length);
            self.bad_show = first_parse(&ini, "showSatisfactionThresholds", "badShow").unwrap_or(self.bad_show);
            self.good_show = first_parse(&ini, "showSatisfactionThresholds", "goodShow").unwrap_or(self.good_show);
            self.great_show = first_parse(&ini, "showSatisfactionThresholds", "greatShow").unwrap_or(self.great_show);
        }

        1
    }

    pub fn register_show(&mut self, show: *const u32, force: bool) -> u32 {
        if show.is_null() {
            return 0;
        }
        let show_addr = show as u32;
        let current_id = get_from_memory::<u16>(show_addr + 0x70);
        let fresh_id = {
            let mut store = SHOW_STORE.lock().unwrap();
            if store.registered_shows.contains_key(&current_id) {
                return 0;
            }
            if force || current_id == 0 {
                store.show_id_counter = store.show_id_counter.wrapping_add(1);
                Some(store.show_id_counter % 0xffff)
            } else {
                None
            }
        };
        let id = match fresh_id {
            Some(id) => {
                crate::ztshowinfo::set_show_info_id(show_addr, id);
                id
            }
            None => current_id,
        };
        SHOW_STORE.lock().unwrap().registered_shows.insert(id, show_addr);
        1
    }

    pub fn unregister_show(&mut self, id: u16, show: *const u32, clear: bool) -> u32 {
        if show.is_null() && id == 0 {
            return 0;
        }
        let effective_id = if id != 0 {
            id
        } else {
            get_from_memory::<u16>(show as u32 + 0x70)
        };
        if clear {
            let target = if id != 0 { Self::get_show_info(id) } else { show as u32 };
            if target != 0 {
                unsafe { CLEAR_SHOW_SCRIPT_STATES.hooked()((target + 0x4) as *const u32) };
            }
        }
        SHOW_STORE.lock().unwrap().registered_shows.remove(&effective_id);
        1
    }

    pub fn get_show_info(id: u16) -> u32 {
        SHOW_STORE.lock().unwrap().registered_shows.get(&id).copied().unwrap_or(0)
    }

    pub fn get_script_id(id: u16) -> u32 {
        let show_info = Self::get_show_info(id);
        if show_info != 0 {
            get_from_memory::<u16>(show_info + 0x8) as u32
        } else {
            0
        }
    }

    pub fn enter_new_month() {
        for show in Self::registered_show_values() {
            if show != 0 {
                crate::ztshowinfo::enter_new_month(show);
            }
        }
    }

    pub fn update() {
        let shows = Self::registered_show_values();
        if shows.is_empty() {
            return;
        }
        for show in shows {
            if show != 0 {
                let vtable: u32 = get_from_memory(show);
                let slot: u32 = get_from_memory(vtable + 0x20);
                let update_fn: unsafe extern "thiscall" fn(*const u32) = unsafe { std::mem::transmute(slot) };
                unsafe { update_fn(show as *const u32) };
            }
        }
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn save(&mut self, file: *const i8) -> u32 {
        let script_ok = unsafe { ZTSHOWSCRIPTMGR_SAVE.hooked()(&raw const self.show_script_mgr as *const u32, file) };
        let counter: u16 = SHOW_STORE.lock().unwrap().show_id_counter;
        let write_ok = unsafe { WRITE_BYTES_TO_FILE.hooked()(&raw const counter as *const u32, 2, 1, file) } == 1;
        (script_ok & write_ok) as u32
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    pub fn load(&mut self, file: *const u32, version: u32) -> u32 {
        let mut ok = unsafe { ZTSHOWSCRIPTMGR_LOAD.hooked()(&raw const self.show_script_mgr as *const u32, file, version) };
        if version > 0x60 {
            let mut counter: u16 = 0;
            let read_ok =
                unsafe { DEALLOCATE.hooked()(&raw mut counter as *const u32, 2, 1, file as *const u8) } == 1;
            if read_ok {
                SHOW_STORE.lock().unwrap().show_id_counter = counter;
            }
            ok &= read_ok;
        }
        ok as u32
    }

    pub fn is_doing_show(unit_id: u32, show_id: u16) -> u32 {
        let show_info = Self::get_show_info(show_id);
        if show_info != 0 {
            let state = crate::ztshow::get_show_script_state(show_info + 0x4, unit_id);
            (state != 0) as u32
        } else {
            0
        }
    }

    pub fn is_show_script_done(script_id: u32, show_id: u16) -> u32 {
        let show_info = Self::get_show_info(show_id);
        if show_info != 0 {
            let state = crate::ztshow::get_show_script_state(show_info + 0x4, script_id);
            if state != 0 {
                get_from_memory::<u8>(state + 0x13) as u32
            } else {
                0
            }
        } else {
            0
        }
    }

    fn registered_show_values() -> Vec<u32> {
        SHOW_STORE.lock().unwrap().registered_shows.values().copied().collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct ZTShowMgrState {
    pub(crate) registered_shows: BTreeMap<u16, u32>,
    pub(crate) show_id_counter: u16,
}

pub(crate) static SHOW_STORE: LazyLock<Mutex<ZTShowMgrState>> = LazyLock::new(|| Mutex::new(ZTShowMgrState::default()));

#[cfg(feature = "reimplementation-tests")]
pub(crate) fn registered_show_count() -> usize {
    SHOW_STORE.lock().unwrap().registered_shows.len()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) fn registered_show_for_id(id: u16) -> Option<u32> {
    SHOW_STORE.lock().unwrap().registered_shows.get(&id).copied()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) fn all_registered_shows() -> Vec<(u16, u32)> {
    SHOW_STORE.lock().unwrap().registered_shows.iter().map(|(&id, &addr)| (id, addr)).collect()
}
