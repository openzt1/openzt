use std::{
    collections::{HashMap, VecDeque},
    mem,
    sync::{LazyLock, Mutex},
};

use super::super::thought::{read_dword, resolve_object_own_habitat_ptr, write_dword, ZTThought};

/// The zoo's thought manager - owns the persistent, sentinel-terminated intrusive list of every
/// active `ZTThought`. Allocated as 16 bytes (`operator_new(0x10)`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTThoughtMgr {
    pub vtable: u32,       // 0x0
    pub flag: u8,           // 0x4 - inherited BFMgr field, not behaviorally relevant
    pub _pad: [u8; 3],      // ----- padding: 3 bytes
    pub sentinel_ptr: u32,  // 0x8 - pointer to the list's sentinel node (not embedded inline)
    pub max_thoughts: u32,  // 0xc - default 1000, the cap `addThought` trims the list to
}

const _: () = assert!(mem::size_of::<ZTThoughtMgr>() == 0x10);

/// Process-global registry backing every `ZTThoughtMgr` instance's persistent list, keyed by that
/// instance's own `sentinel_ptr` value.
pub(crate) static THOUGHT_STORES: LazyLock<Mutex<HashMap<u32, VecDeque<ZTThought>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

impl ZTThoughtMgr {
    pub fn max_thoughts(&self) -> u32 {
        self.max_thoughts
    }

    /// This instance's key into [`THOUGHT_STORES`] - its own `sentinel_ptr`, never dereferenced as a
    /// pointer by any of the methods below.
    pub fn store_key(&self) -> u32 {
        self.sentinel_ptr
    }

    /// Walks the persistent thought list front-to-back (most-recently-inserted first), yielding owned
    /// copies.
    pub fn iter(&self) -> impl Iterator<Item = ZTThought> {
        THOUGHT_STORES.lock().unwrap().get(&self.store_key()).into_iter().flatten().copied().collect::<Vec<_>>().into_iter()
    }

    pub fn len(&self) -> usize {
        THOUGHT_STORES.lock().unwrap().get(&self.store_key()).map_or(0, VecDeque::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Inserts `thought` at the front of the list (matching `addThought`'s own insertion point -
    /// most-recent-first), then trims from the back until the list is at most `max_thoughts` long.
    pub fn insert_front(&mut self, thought: ZTThought) {
        let mut stores = THOUGHT_STORES.lock().unwrap();
        let store = stores.entry(self.store_key()).or_default();
        store.push_front(thought);
        while store.len() > self.max_thoughts as usize {
            store.pop_back();
        }
    }

    /// Removes every thought matching `predicate`. Shared removal primitive for
    /// `removeThoughtsBy{Thinker,Habitat,Object}`.
    pub fn remove_where(&mut self, predicate: impl Fn(&ZTThought) -> bool) {
        if let Some(store) = THOUGHT_STORES.lock().unwrap().get_mut(&self.store_key()) {
            store.retain(|t| !predicate(t));
        }
    }

    /// An ordered, `max_count`-bounded sequence of matching `ZTThought`s.
    pub fn get_thoughts_by_thinker(&self, thinker_ptr: u32, max_count: usize) -> Vec<ZTThought> {
        let mut matches: Vec<ZTThought> = self.iter().filter(|t| t.thinker_ptr() == thinker_ptr).take(max_count).collect();
        matches.reverse();
        matches
    }

    /// Matches on `object_ptr`.
    pub fn get_thoughts_by_object(&self, object_ptr: u32, max_count: usize) -> Vec<ZTThought> {
        let mut matches: Vec<ZTThought> = self.iter().filter(|t| t.object_ptr() == object_ptr).take(max_count).collect();
        matches.reverse();
        matches
    }

    /// Matches on `habitat_ptr`.
    pub fn get_thoughts_by_habitat(&self, habitat_ptr: u32, max_count: usize) -> Vec<ZTThought> {
        let mut matches: Vec<ZTThought> = self.iter().filter(|t| t.habitat_ptr() == habitat_ptr).take(max_count).collect();
        matches.reverse();
        matches
    }

    /// Uses `insert_front`, which already trims to `max_thoughts` after every insert.
    pub fn add_thought(&mut self, string_id: u32, thinker_ptr: u32, object_ptr: u32, habitat_ptr: u32) {
        let habitat_arg = if object_ptr != 0 { resolve_object_own_habitat_ptr(object_ptr).unwrap_or(habitat_ptr) } else { habitat_ptr };
        self.insert_front(ZTThought::new(string_id, thinker_ptr, object_ptr, habitat_arg));
    }

    /// Matches on `thinker_ptr`.
    pub fn remove_thoughts_by_thinker(&mut self, thinker_ptr: u32) {
        self.remove_where(|t| t.thinker_ptr() == thinker_ptr);
    }

    /// Matches on `object_ptr`.
    pub fn remove_thoughts_by_object(&mut self, object_ptr: u32) {
        self.remove_where(|t| t.object_ptr() == object_ptr);
    }

    /// Removes thoughts for `habitat_ptr`.
    pub fn remove_thoughts_by_habitat(&mut self, habitat_ptr: u32, force: bool) {
        if let Some(store) = THOUGHT_STORES.lock().unwrap().get_mut(&self.store_key()) {
            store.retain_mut(|t| {
                if t.habitat_ptr != habitat_ptr {
                    return true;
                }
                if !force && t.object_ptr != 0 {
                    t.habitat_ptr = 0;
                    true
                } else {
                    false
                }
            });
        }
    }

    /// Writes the list's own length as a leading dword, then calls `ZTThought::save` on every thought.
    pub fn save(&self, file: *const u32) -> bool {
        let mut ok = write_dword(file, self.len() as u32);
        for thought in self.iter() {
            ok &= thought.save(file);
        }
        ok
    }

    /// Reads a leading dword count, then loads each record.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        let Some(count) = read_dword(file) else {
            return false;
        };
        let count = count as i32;
        if count <= 0 {
            return true;
        }

        let mut ok = true;
        let mut stores = THOUGHT_STORES.lock().unwrap();
        let store = stores.entry(self.store_key()).or_default();
        for _ in 0..count {
            let mut thought = ZTThought::new(0, 0, 0, 0);
            let loaded_ok = thought.load(file, version);
            ok &= loaded_ok;
            if loaded_ok && (thought.object_id == 0 || thought.object_ptr != 0) && (thought.thinker_id == 0 || thought.thinker_ptr != 0) {
                store.push_back(thought);
            }
            if !loaded_ok {
                break;
            }
        }
        ok
    }

    /// Calls `ZTThought::populate` on every thought in the list.
    pub fn populate_thoughts(&mut self) {
        if let Some(store) = THOUGHT_STORES.lock().unwrap().get_mut(&self.store_key()) {
            for thought in store.iter_mut() {
                thought.populate();
            }
        }
    }

    /// Clears the store for this instance.
    pub fn clear(&mut self) {
        if let Some(store) = THOUGHT_STORES.lock().unwrap().get_mut(&self.store_key()) {
            store.clear();
        }
    }
}
