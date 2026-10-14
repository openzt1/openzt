use std::mem;

use openzt_detour::generated::standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE};

use crate::{
    globals::{get_module_base, globals},
    string_registry::load_string_by_id,
    util::{get_from_memory, ref_from_memory, ZTString},
    zthabitatmgr::ZTHabitat,
    ztworldmgr::BFEntity,
};

/// One thought bubble entry. Own 2-entry vtable (`save`/`load`), no base class. Total size `0x24`
/// (36 bytes = 9 x u32/i32 fields). The persistent list's nodes are allocated at `0x30` - 4 bytes
/// more than `0x24`, likely the block allocator's size-class rounding.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ZTThought {
    pub vtable: u32,      // 0x0
    pub string_id: u32,   // 0x4 - message/template id, e.g. 0x280a for "caught prey"
    pub thinker_id: u32,  // 0x8 - not a raw pointer; copied from thinker_entity's own BFEntity::id at construction
    pub object_id: u32,   // 0xc - same mechanism, from object_entity's own BFEntity::id
    pub tile_x: i32,      // 0x10 - -1 sentinel = "none"
    pub tile_y: i32,      // 0x14 - -1 sentinel = "none"
    pub thinker_ptr: u32, // 0x18 - resolved live pointer, populated by resolving thinker_id via ZTWorldMgr::resolve_entity_by_id; null if unresolved
    pub object_ptr: u32,  // 0x1c - same resolution mechanism for object_id
    pub habitat_ptr: u32, // 0x20 - *ZTHabitat; set directly at construction, or recomputed on populate/load
}

const _: () = assert!(mem::size_of::<ZTThought>() == 0x24);

impl ZTThought {
    pub fn string_id(&self) -> u32 {
        self.string_id
    }

    pub fn thinker_id(&self) -> u32 {
        self.thinker_id
    }

    pub fn object_id(&self) -> u32 {
        self.object_id
    }

    pub fn tile_x(&self) -> i32 {
        self.tile_x
    }

    pub fn tile_y(&self) -> i32 {
        self.tile_y
    }

    pub fn thinker_ptr(&self) -> u32 {
        self.thinker_ptr
    }

    pub fn object_ptr(&self) -> u32 {
        self.object_ptr
    }

    pub fn habitat_ptr(&self) -> u32 {
        self.habitat_ptr
    }

    /// Called after a save-file load to re-derive the live, non-persistent pointer fields
    /// (`thinker_ptr`/`object_ptr`/`habitat_ptr`, plus a possible `tile_x`/`tile_y` refresh) from the
    /// persisted `thinker_id`/`object_id`/`tile_x`/`tile_y` alone.
    pub fn populate(&mut self) {
        let world_mgr = globals().ztworldmgr();
        self.thinker_ptr = world_mgr.resolve_entity_by_id(self.thinker_id) as u32;
        self.object_ptr = world_mgr.resolve_entity_by_id(self.object_id) as u32;

        if self.object_ptr != 0
            && let Some(habitat_ptr) = resolve_object_own_habitat_ptr(self.object_ptr)
        {
            self.habitat_ptr = habitat_ptr;
            if self.habitat_ptr != 0 {
                let habitat = unsafe { ref_from_memory::<ZTHabitat>(self.habitat_ptr) };
                if let Some(tile) = habitat.get_gate_tile_in() {
                    self.tile_x = tile.pos.x;
                    self.tile_y = tile.pos.y;
                }
            }
        }

        if self.habitat_ptr == 0 && self.tile_x != -1 && self.tile_y != -1 {
            self.habitat_ptr = globals().zthabitatmgr().get_habitat_ptr(self.tile_x, self.tile_y);
        }
    }

    /// `habitat_arg` is only accepted into `habitat_ptr` if the pointed-to `ZTHabitat`'s own flag at
    /// `+0x2c` is unset, and, when accepted, `tile_x`/`tile_y` are immediately refreshed from that
    /// habitat's own gate tile. `thinker_id`/`object_id` are resolved from `thinker_ptr`/`object_ptr`'s
    /// own `BFEntity::id` whenever those pointers are non-null.
    pub fn new(string_id: u32, thinker_ptr: u32, object_ptr: u32, habitat_arg: u32) -> ZTThought {
        let vtable = get_module_base("zoo.exe") as u32 + 0x0023_5400;
        let mut thought =
            ZTThought { vtable, string_id, thinker_id: 0, object_id: 0, tile_x: -1, tile_y: -1, thinker_ptr, object_ptr, habitat_ptr: 0 };

        if thinker_ptr != 0 {
            thought.thinker_id = *unsafe { ref_from_memory::<BFEntity>(thinker_ptr) }.id();
        }
        if object_ptr != 0 {
            thought.object_id = *unsafe { ref_from_memory::<BFEntity>(object_ptr) }.id();
        }
        if habitat_arg != 0 {
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_arg) };
            if *habitat.unknown_flag_0x2c() == 0 {
                thought.habitat_ptr = habitat_arg;
            }
            if thought.habitat_ptr != 0
                && let Some(tile) = unsafe { ref_from_memory::<ZTHabitat>(thought.habitat_ptr) }.get_gate_tile_in()
            {
                thought.tile_x = tile.pos.x;
                thought.tile_y = tile.pos.y;
            }
        }
        thought
    }

    /// Writes `string_id`, `thinker_id`, `object_id`, `tile_x`, `tile_y` as five little-endian dwords,
    /// in that order.
    pub fn save(&self, file: *const u32) -> bool {
        let mut ok = write_dword(file, self.string_id);
        ok &= write_dword(file, self.thinker_id);
        ok &= write_dword(file, self.object_id);
        ok &= write_dword(file, self.tile_x as u32);
        ok &= write_dword(file, self.tile_y as u32);
        ok
    }

    /// Reads `string_id` first, unconditionally, then branches on `version` for the remaining fields' read order.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        let string_id_ok = match read_dword(file) {
            Some(v) => {
                self.string_id = v;
                true
            }
            None => false,
        };

        let ok = if version < 0x1e {
            let object_id_ok = match read_dword(file) {
                Some(v) => {
                    self.object_id = v;
                    true
                }
                None => false,
            };
            let thinker_id_ok = match read_dword(file) {
                Some(v) => {
                    self.thinker_id = v;
                    true
                }
                None => false,
            };
            string_id_ok && object_id_ok && thinker_id_ok
        } else {
            let thinker_id_ok = match read_dword(file) {
                Some(v) => {
                    self.thinker_id = v;
                    true
                }
                None => false,
            };
            let object_id_ok = match read_dword(file) {
                Some(v) => {
                    self.object_id = v;
                    true
                }
                None => false,
            };
            let tile_x_ok = match read_dword(file) {
                Some(v) => {
                    self.tile_x = v as i32;
                    true
                }
                None => false,
            };
            let tile_y_ok = match read_dword(file) {
                Some(v) => {
                    self.tile_y = v as i32;
                    true
                }
                None => false,
            };
            string_id_ok && thinker_id_ok && object_id_ok && tile_x_ok && tile_y_ok
        };

        if ok && version > 0x1d {
            let world_mgr = globals().ztworldmgr();
            self.thinker_ptr = world_mgr.resolve_entity_by_id(self.thinker_id) as u32;
            self.object_ptr = world_mgr.resolve_entity_by_id(self.object_id) as u32;
            if self.tile_x != -1 && self.tile_y != -1 {
                self.habitat_ptr = globals().zthabitatmgr().get_habitat_ptr(self.tile_x, self.tile_y);
            }
        }
        ok
    }

    /// Loads the thought's own template string, then applies at most one `%s` substitution.
    pub fn get_string(&self) -> String {
        let template = load_string_by_id(self.string_id);
        let substitution: Option<String> = if self.object_ptr != 0 {
            Some(unsafe { ref_from_memory::<BFEntity>(self.object_ptr) }.name().copy_to_string())
        } else if self.habitat_ptr != 0 {
            Some(unsafe { ref_from_memory::<ZTHabitat>(self.habitat_ptr) }.exhibit_name().copy_to_string())
        } else {
            None
        };
        substitute_thought_string(template, substitution.as_deref())
    }
}

/// Pure substitution logic `ZTThought::get_string` delegates to, isolated for testing without touching
/// real memory. A missing template returns an empty string; an empty template is returned as-is.
pub fn substitute_thought_string(template: Option<String>, substitution: Option<&str>) -> String {
    let Some(template) = template else {
        return String::new();
    };
    if template.is_empty() {
        return template;
    }
    match substitution {
        Some(name) => template.replacen("%s", name, 1),
        None => template,
    }
}

/// Shared logic behind `ZTThought::populate`'s object-vtable-driven habitat resolution and
/// `ZTThoughtMgr::addThought`'s override of the caller-supplied habitat argument.
pub fn resolve_object_own_habitat_ptr(object_ptr: u32) -> Option<u32> {
    let object = unsafe { ref_from_memory::<BFEntity>(object_ptr) };
    let entity_type_ptr = *object.inner_class_ptr();
    let entity_type_vtable = get_from_memory::<u32>(entity_type_ptr);
    let type_check_fn =
        unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(entity_type_vtable + 0x1c)) };
    let type_check_arg = get_module_base("zoo.exe") as u32 + 0x00238690;
    if !type_check_fn(entity_type_ptr, type_check_arg) {
        return None;
    }
    let object_vtable = *object.vtable();
    let resolve_habitat_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(object_vtable + 0x24c)) };
    Some(resolve_habitat_fn(object_ptr))
}

/// Writes `value` as a single little-endian dword via whatever is installed at the vanilla
/// `WriteBytesToFile` address (`.hooked()`).
pub fn write_dword(file: *const u32, value: u32) -> bool {
    let bytes = value.to_le_bytes();
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, 4, 1, file as *const i8) }) == 1
}

/// Reads a single little-endian dword via whatever is installed at the vanilla read-primitive address
/// (`.hooked()`). `None` on a short/failed read.
pub fn read_dword(file: *const u32) -> Option<u32> {
    let mut buf = 0u32;
    let ok = unsafe { DEALLOCATE.hooked()(&mut buf as *mut u32 as *const u32, 4, 1, file as *const u8) };
    (ok == 1).then_some(buf)
}
