use std::{
    collections::{BTreeMap, HashMap},
    sync::{LazyLock, Mutex},
};

use openzt_detour::generated::standalone::WRITE_BYTES_TO_FILE;

use super::{
    item::{GET_ITEM_BY_TRICK_ID_POOL, GET_ITEM_POOL, ItemSnapshot, ShowScriptItem, ZTShowScriptItemRaw},
    script::{
        encode_script, next_script_id, read_script, synthetic_script_handle, MAX_SCRIPT_COUNT,
        ShowScriptData,
    },
};

#[derive(Default)]
pub(crate) struct ShowScriptMgrState {
    pub(crate) scripts: BTreeMap<u16, ShowScriptData>,
    pub(crate) aliases: HashMap<u32, u16>,
    pub(crate) next_id_counter: u16,
}

pub(crate) static STATE: LazyLock<Mutex<ShowScriptMgrState>> =
    LazyLock::new(|| Mutex::new(ShowScriptMgrState::default()));

#[cfg(test)]
pub(crate) static SHOW_SCRIPT_STORE_TEST_LOCK: Mutex<()> = Mutex::new(());

#[cfg(test)]
pub(crate) fn reset_store_for_test() {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    state.next_id_counter = 0;
}

pub fn register_script(ctor_ptr: u32, script_type: u32) -> Option<u16> {
    if ctor_ptr == 0 {
        return None;
    }
    let mut state = STATE.lock().unwrap();
    let id = next_script_id(&mut state.next_id_counter);
    state.scripts.insert(id, ShowScriptData { sentinel: 0xffff_ffff, script_type, items: Vec::new() });
    state.aliases.insert(ctor_ptr, id);
    Some(id)
}

pub fn get_script(id: u16) -> u32 {
    let mut state = STATE.lock().unwrap();
    if !state.scripts.contains_key(&id) {
        return 0;
    }
    let handle = synthetic_script_handle(id);
    state.aliases.insert(handle, id);
    handle
}

pub(crate) fn resolve(state: &ShowScriptMgrState, this_ptr: u32) -> Option<u16> {
    state.aliases.get(&this_ptr).copied()
}

pub fn unregister_script(this_ptr: u32) -> bool {
    let mut state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    state.scripts.remove(&id);
    state.aliases.retain(|_, v| *v != id);
    true
}

pub fn clear_all_scripts() -> bool {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    true
}

pub fn size(this_ptr: u32) -> i32 {
    let state = STATE.lock().unwrap();
    resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).map(|s| s.items.len() as i32).unwrap_or(0)
}

pub fn clear_all(this_ptr: u32) {
    let mut state = STATE.lock().unwrap();
    if let Some(id) = resolve(&state, this_ptr)
        && let Some(script) = state.scripts.get_mut(&id)
    {
        script.items.clear();
    }
}

pub fn get_item(this_ptr: u32, index: u16) -> u32 {
    let state = STATE.lock().unwrap();
    let Some(item) = resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).and_then(|s| s.items.get(index as usize)) else {
        return 0;
    };
    GET_ITEM_POOL.with(|pool| pool.write(item))
}

pub fn get_item_by_trick_id(this_ptr: u32, trick_id: u16) -> u32 {
    let state = STATE.lock().unwrap();
    let Some(item) = resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).and_then(|s| s.items.iter().find(|it| it.id == trick_id))
    else {
        return 0;
    };
    GET_ITEM_BY_TRICK_ID_POOL.with(|pool| pool.write(item))
}

pub fn remove_item(this_ptr: u32, index: u16) -> bool {
    let mut state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    let Some(script) = state.scripts.get_mut(&id) else { return false };
    if (index as usize) < script.items.len() {
        script.items.remove(index as usize);
        true
    } else {
        false
    }
}

pub fn item_snapshot_by_id(id: u16, index: u16) -> Option<ItemSnapshot> {
    let state = STATE.lock().unwrap();
    let item = state.scripts.get(&id)?.items.get(index as usize)?;
    Some(ItemSnapshot { id: item.id, item_type: item.item_type, satisfaction: item.satisfaction, satisfaction_mirror: item.satisfaction_mirror })
}

pub fn script_item_count_by_id(id: u16) -> usize {
    STATE.lock().unwrap().scripts.get(&id).map(|s| s.items.len()).unwrap_or(0)
}

pub fn script_type_by_id(id: u16) -> Option<u32> {
    STATE.lock().unwrap().scripts.get(&id).map(|s| s.script_type)
}

pub fn script_exists_by_id(id: u16) -> bool {
    STATE.lock().unwrap().scripts.contains_key(&id)
}

pub(crate) fn item_full_by_id(id: u16, index: u16) -> Option<ShowScriptItem> {
    let state = STATE.lock().unwrap();
    state.scripts.get(&id)?.items.get(index as usize).cloned()
}

pub fn unregister_script_by_id(id: u16) -> bool {
    let mut state = STATE.lock().unwrap();
    if state.scripts.remove(&id).is_some() {
        state.aliases.retain(|_, v| *v != id);
        true
    } else {
        false
    }
}

pub fn add_item(this_ptr: u32, item: &ZTShowScriptItemRaw) -> u32 {
    let mut state = STATE.lock().unwrap();
    if let Some(id) = resolve(&state, this_ptr)
        && let Some(script) = state.scripts.get_mut(&id)
        && item.item_type == script.script_type
    {
        script.items.push(item.to_owned_item());
    }
    0
}

pub(crate) fn encode_mgr(state: &ShowScriptMgrState) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(state.scripts.len() as u32).to_le_bytes());
    for (&id, script) in &state.scripts {
        buf.extend_from_slice(&encode_script(id, script));
    }
    buf.extend_from_slice(&state.next_id_counter.to_le_bytes());
    buf
}

pub fn save_mgr(file: *const u32) -> bool {
    let state = STATE.lock().unwrap();
    let bytes = encode_mgr(&state);
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) }) == 1
}

pub fn save_script(this_ptr: u32, file: *const u32) -> bool {
    let state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    let Some(script) = state.scripts.get(&id) else { return false };
    let bytes = encode_script(id, script);
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) }) == 1
}

pub fn load_mgr(file: *const u32, version: u32) -> bool {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    if version <= 0x58 {
        return true;
    }
    let Some(count) = super::script::read_u32(file) else {
        return false;
    };
    if count > MAX_SCRIPT_COUNT {
        return false;
    }
    for _ in 0..count {
        let Some((id, script)) = read_script(file, version) else {
            return false;
        };
        state.scripts.insert(id, script);
    }
    if version > 0x60 {
        let Some(counter) = super::script::read_u16(file) else {
            return false;
        };
        state.next_id_counter = counter;
    }
    true
}
