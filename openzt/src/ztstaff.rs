//! `ZTStaff`: the staff entity base under `ZTKeeper`, `ZTGuide`, `ZTMaint` and `ZTHelicopter` (`ZTUnit` -> `BFUnit` ->
//! `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): staff are heap entities that un-ported `ZTUnit`/`BFEntity` code, `ZTHabitat`,
//! `ZTHabitatMgr` and the goal classes read by raw offset, so the object stays vanilla memory and this module reads and
//! writes it in place. A `ZTStaff` is `0x294` bytes: the `ZTUnit` part (`0x000`-`0x25f`) followed by the fields in
//! [`staff_offsets`]; each subclass appends its own fields at `0x294`. Nothing here owns an allocation. The assigned-habitat
//! list and the unit's two vectors use vanilla's 12-byte small-block freelist (`DAT_00638004`), reached through
//! [`crate::vanilla_list`]; nothing here frees through Rust's allocator.
//!
//! Every method on [`ZTStaff`] is the base behaviour: the subclasses call it, directly or through the vtable, after their
//! own part. `ZTHelicopter` is entirely vanilla and reaches these bodies through its inherited vtable slots.

use std::{ffi::c_void, mem::transmute};

use glam::i32::IVec3;
use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bfunit::{IS_MAX_COST_FOR_ALL, IS_NON_PATH_STEEP, IS_SAME_TILE},
    msvc_std_listuint::LIST_UINT_CTOR,
    poolalloc::{DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_12 as POOLALLOC_DEALLOCATE_N_12},
    standalone::OPERATOR_DELETE,
    zthabitat::{GET_GATE_TILE_IN, GET_GATE_TILE_OUT, GET_GATE_TILE_PASS_OUT},
    ztstaff::{
        ADD_TO_MAP, ASSIGN_HABITAT, CLEAR_ASSIGNED_HABITATS, CONSTRUCTOR, DESTRUCTOR_0, DESTRUCTOR_1, DO_CONDITIONALS, DONE_CAPTURE, DROP, FENCES_OK,
        FOUND_ESCAPED_ANIMAL, GET_ASSIGNED_HABITAT, GET_MONTHLY_COST, GET_RANDOM_ASSIGNED_HABITAT, GET_TERRAIN_COST, INIT_AFTER_LOAD, INIT_STATUS_VARS,
        IS_HABITAT_ASSIGNED, LOAD, PICK_RANDOM_DEST, REASSIGN_HABITATS, REMOVE_ASSIGNED_HABITAT, REMOVE_ASSIGNED_HABITAT_AT, REMOVE_FROM_MAP, SAVE,
        SET_TARGET_TILE,
    },
    ztui_multistaff::ADD_ENTITY as MULTISTAFF_ADD_ENTITY,
    ztunit::{
        ADD_TO_MAP as ZTUNIT_ADD_TO_MAP, CONSTRUCTOR as ZTUNIT_CONSTRUCTOR, DESTRUCTOR_0 as ZTUNIT_DESTRUCTOR_0, INIT_AFTER_LOAD as ZTUNIT_INIT_AFTER_LOAD,
        INIT_STATUS_VARS as ZTUNIT_INIT_STATUS_VARS, LOAD as ZTUNIT_LOAD, PICK_RANDOM_DEST as ZTUNIT_PICK_RANDOM_DEST,
        REMOVE_FROM_MAP as ZTUNIT_REMOVE_FROM_MAP, SAVE as ZTUNIT_SAVE, SET_TARGET_TILE as ZTUNIT_SET_TARGET_TILE,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, save_to_memory},
    vanilla_list, vanilla_vector,
    ztkeeper::{read_field, write_field},
    zthabitat::{next_game_random, MAX_PATH_COST_RVA},
    ztmegatilemgr::entity_type_matches,
    ztshow::RVA_ANIMAL_TYPE_CHECK,
};

/// A `ZTStaff` (and so every subclass) is this many bytes before the subclass's own fields.
pub const STAFF_SIZE: u32 = 0x294;

/// Vtable address of `ZTStaff` (`0x0062ed84`) as an RVA from the `zoo.exe` base.
pub const STAFF_VTABLE_RVA: u32 = 0x0022_ed84;

/// Class tag (RVA) the entity type's `isCastClass` takes to test for a staff type (`CAST_ZTStaff`).
pub const STAFF_TYPE_TAG_RVA: u32 = 0x0023_8710;

/// Offsets of the `ZTStaff` fields, from the start of the unit.
pub mod staff_offsets {
    /// Assigned-habitat `list<uint>`: a pointer to the sentinel node. Nodes are `{next +0x0, previous +0x4, habitat +0x8}`.
    pub const ASSIGNED_HABITATS: u32 = 0x260;
    /// `vector<(x, y, z)>` of 12-byte records: the gate-tile positions of the habitats saved with the unit. `load` fills
    /// it and `reassignHabitats` resolves each record to a habitat and empties it.
    pub const GATE_TILES_BEGIN: u32 = 0x264;
    pub const GATE_TILES_END: u32 = 0x268;
    pub const GATE_TILES_CAPACITY: u32 = 0x26c;
    /// `vector<u32>` of entity ids the unit skips; `drop` empties it by resetting the end pointer to the begin pointer.
    pub const SKIP_ENTITIES_BEGIN: u32 = 0x270;
    pub const SKIP_ENTITIES_END: u32 = 0x274;
    pub const SKIP_ENTITIES_CAPACITY: u32 = 0x278;
    /// `vector<BFTile*>` of tiles the unit skips; `drop` empties it the same way.
    pub const SKIP_TILES_BEGIN: u32 = 0x27c;
    pub const SKIP_TILES_END: u32 = 0x280;
    pub const SKIP_TILES_CAPACITY: u32 = 0x284;
    /// Initial countdown: `-1` when the type's first staff dword is `0`, otherwise an RNG draw modulo it. Saved first.
    pub const INITIAL_COUNTDOWN: u32 = 0x288;
    /// Countdown `doConditionals` decrements each call; reloaded from the type's second staff dword minus one.
    pub const CONDITIONALS_COUNTDOWN: u32 = 0x28c;
    /// Id of the escaped animal being chased. `foundEscapedAnimal` writes it; `doneCapture` and `removeFromMap` clear it.
    pub const ESCAPED_ANIMAL: u32 = 0x290;
}

/// `ZTHabitat::unknown_flag_0x2c`: set on the "world" habitat, which staff are never assigned to.
const HABITAT_IS_WORLD_OFFSET: u32 = 0x2c;
/// The animal field `foundEscapedAnimal` sets to the chasing staff member's id and `doneCapture` clears.
const ANIMAL_CAPTURED_BY_OFFSET: u32 = 0x374;
/// `ZTStaffType`'s monthly wage (a `f32`).
const TYPE_MONTHLY_COST_OFFSET: u32 = 0x1bc;
/// The entity type pointer every `BFEntity` holds.
const ENTITY_TYPE_OFFSET: u32 = 0x128;
/// `BFTile` byte tested against `0x8` by `getTerrainCost`.
const TILE_FLAGS_83_OFFSET: u32 = 0x83;
/// Vtable slot of `BFWorldMgr::getEntity(id, flag)`.
const WORLD_GET_ENTITY_SLOT: u32 = 0x24;
/// `GLOBAL_ZTWorldMgr` as an RVA from the `zoo.exe` base.
const WORLD_MGR_RVA: u32 = 0x0023_8040;

/// `DAT_00639474`: the variable holding the sentinel of the list of every staff member on the map, as an RVA.
const STAFF_LIST_RVA: u32 = 0x0023_9474;
/// `ZTUI::multistaff::removeEntity` (`0x00501f2b`, `cdecl`): drops the staff member's row from the staff panel's list box.
const MULTISTAFF_REMOVE_ENTITY: u32 = 0x0050_1f2b;
/// `BFEntity` bytes that exclude a unit from the staff panel and the global staff list.
const EXCLUDED_FLAG_A_OFFSET: u32 = 0x142;
const EXCLUDED_FLAG_B_OFFSET: u32 = 0x143;
/// `ZTStaffType`'s two countdown periods: the first seeds `+0x288`, the second `+0x28c`.
const TYPE_INITIAL_PERIOD_OFFSET: u32 = 0x1b4;
const TYPE_CONDITIONALS_PERIOD_OFFSET: u32 = 0x1b8;
/// Save versions at which `load` starts reading each `ZTStaff` field.
const VERSION_INITIAL_COUNTDOWN: u32 = 0x10;
const VERSION_CONDITIONALS_COUNTDOWN: u32 = 0x16;
const VERSION_GATE_TILES: u32 = 0x1a;
const VERSION_ESCAPED_ANIMAL: u32 = 0x4b;
/// `BFTile` position fields a gate tile is saved by.
const TILE_X_OFFSET: u32 = 0x34;
const TILE_Y_OFFSET: u32 = 0x38;
/// `BFTile` bytes: `+0x85` bit `0x4` marks the tile a staff member is walking to; the edge entity slots start at `+0x14`.
const TILE_FLAGS_85_OFFSET: u32 = 0x85;
const TILE_EDGE_SLOTS_OFFSET: u32 = 0x14;
/// The `ZTUnit` fields the behaviour methods read: the goal id, the pending predator check, the tile being walked to.
const UNIT_GOAL_ID_OFFSET: u32 = 0x170;
const UNIT_PREDATOR_CHECK_PENDING_OFFSET: u32 = 0x24c;
const UNIT_TARGET_TILE_OFFSET: u32 = 0x234;
/// The goal id of a unit reacting to a predator; `doConditionals` leaves the predator check alone while it is set.
const GOAL_PREDATOR: u32 = 0x75;
/// `BFEntity` id.
const ENTITY_ID_OFFSET: u32 = 0x124;
/// Virtual slots the behaviour methods dispatch through (subclasses override them).
const SLOT_FIND_PREDATOR: u32 = 0x2fc;
const SLOT_GET_HABITAT_FROM_TILE: u32 = 0x248;
const SLOT_GET_RANDOM_ASSIGNED_HABITAT: u32 = 0x328;
/// `DAT_00638fb0`: the variable holding the sentinel of the escaped-animal list, as an RVA.
const ESCAPED_ANIMAL_LIST_RVA: u32 = 0x0023_8fb0;
/// `CAST_ZTFence` and `TANKWALL_CLASS_NAME` as RVAs.
const FENCE_TAG_RVA: u32 = 0x0023_8660;
const TANKWALL_TAG_RVA: u32 = 0x0023_8720;
/// `ZTFenceType` bytes `fencesOK` reads: a flag set on tank-wall types, one set on types a unit may always cross, and the
/// type's class name string pointer (`'g'` first for a gate). On the fence itself: a flag set while it is not blocking,
/// and the tank wall's own flag byte.
const FENCE_TYPE_TANK_WALL_FLAG_OFFSET: u32 = 0x193;
const FENCE_TYPE_PASSABLE_FLAG_OFFSET: u32 = 0x191;
const FENCE_TYPE_NAME_OFFSET: u32 = 0xa4;
const FENCE_NOT_BLOCKING_OFFSET: u32 = 0x164;
const TANK_WALL_FLAG_OFFSET: u32 = 0x464;

/// A staff member in vanilla memory, addressed by its raw pointer. All reads and writes are volatile through the address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTStaff(pub u32);

impl ZTStaff {
    fn read<T>(self, offset: u32) -> T {
        get_from_memory(self.0 + offset)
    }

    fn write<T>(self, offset: u32, value: T) {
        save_to_memory(self.0 + offset, value);
    }

    /// The sentinel node of the assigned-habitat list.
    pub fn assigned_list(self) -> u32 {
        self.read(staff_offsets::ASSIGNED_HABITATS)
    }

    /// The habitats this unit is assigned to, in list order.
    pub fn assigned_habitats(self) -> Vec<u32> {
        vanilla_list::nodes(self.assigned_list()).map(vanilla_list::value).collect()
    }

    /// `ZTStaff::isHabitatAssigned` (`0x004467ec`): whether `habitat` is in the assigned list.
    pub fn is_habitat_assigned(self, habitat: u32) -> bool {
        vanilla_list::nodes(self.assigned_list()).any(|node| vanilla_list::value(node) == habitat)
    }

    /// `ZTStaff::removeAssignedHabitat` (`0x0050c884`): unlinks every node holding `habitat` and pushes it on the freelist.
    pub fn remove_assigned_habitat(self, habitat: u32) {
        vanilla_list::remove_matching(self.assigned_list(), habitat);
    }

    /// `ZTStaff::assignHabitat` (`0x005a168f`): appends `habitat` unless it is null, the world habitat or already assigned.
    pub fn assign_habitat(self, habitat: u32) {
        if habitat == 0 || get_from_memory::<u8>(habitat + HABITAT_IS_WORLD_OFFSET) != 0 {
            return;
        }
        if !self.is_habitat_assigned(habitat) {
            vanilla_list::push_back(self.assigned_list(), habitat);
        }
    }

    /// `ZTStaff::clearAssignedHabitats` (`0x005017ac`): frees every node and leaves the list empty.
    pub fn clear_assigned_habitats(self) {
        vanilla_list::clear(self.assigned_list());
    }

    /// `ZTStaff::getAssignedHabitat` (`0x0061108f`): the habitat at list position `index`, or `0` past the end. A negative
    /// index counts away from zero and so also walks to the end.
    pub fn get_assigned_habitat(self, index: i32) -> u32 {
        let sentinel = self.assigned_list();
        let mut node: u32 = get_from_memory(sentinel);
        let mut remaining = index;
        while remaining != 0 && node != sentinel {
            node = get_from_memory(node);
            remaining = remaining.wrapping_sub(1);
        }
        if node == sentinel {
            0
        } else {
            vanilla_list::value(node)
        }
    }

    /// `ZTStaff::removeAssignedHabitatAt` (`0x00611058`): erases the node at list position `index`, if there is one.
    pub fn remove_assigned_habitat_at(self, index: i32) {
        let sentinel = self.assigned_list();
        let mut node: u32 = get_from_memory(sentinel);
        let mut remaining = index;
        while remaining != 0 {
            if node == sentinel {
                return;
            }
            node = get_from_memory(node);
            remaining = remaining.wrapping_sub(1);
        }
        if node != sentinel {
            vanilla_list::unlink_and_free(node);
        }
    }

    /// `ZTStaff::getRandomAssignedHabitat` (`0x0061101d`): a random habitat from the habitat manager's non-world list
    /// (`ZTHabitatMgr::getNonWorldHabitats`, its `+0x1c` array), regardless of this unit's assignments. Null when the
    /// manager is absent or the list is empty; the game RNG advances only when there is a choice.
    pub fn get_random_assigned_habitat(self) -> u32 {
        if globals().zthabitatmgr_ptr().is_null() {
            return 0;
        }
        let habitats = globals().zthabitatmgr().exhibit_array();
        let count = habitats.len();
        if count == 0 {
            return 0;
        }
        habitats.get_ptr(next_game_random() as usize % count)
    }

    /// `ZTStaff::getMonthlyCost` (`0x005ad1e8`): the staff type's wage, `0.0` for a unit whose type is missing or not a
    /// staff type (vanilla reads address `0x1bc` there and faults).
    pub fn get_monthly_cost(self) -> f32 {
        if unsafe { entity_type_matches(self.0, STAFF_TYPE_TAG_RVA) } {
            get_from_memory(self.read::<u32>(ENTITY_TYPE_OFFSET) + TYPE_MONTHLY_COST_OFFSET)
        } else {
            0.0
        }
    }

    /// `ZTStaff::drop` (`0x004e766a`): empties the skipped-entity and skipped-tile vectors.
    pub fn drop_staff(self) {
        self.write(staff_offsets::SKIP_ENTITIES_END, self.read::<u32>(staff_offsets::SKIP_ENTITIES_BEGIN));
        self.write(staff_offsets::SKIP_TILES_END, self.read::<u32>(staff_offsets::SKIP_TILES_BEGIN));
    }

    /// `ZTStaff::doneCapture` (`0x0050d3e0`): releases the escaped animal being chased (its capturing-staff field is
    /// cleared when the id still resolves to an animal) and forgets its id.
    pub fn done_capture(self) {
        let animal_id: u32 = self.read(staff_offsets::ESCAPED_ANIMAL);
        if animal_id != 0 {
            let world: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA);
            let get_entity = unsafe {
                transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(world) + WORLD_GET_ENTITY_SLOT))
            };
            let animal = get_entity(world, animal_id, 1);
            if animal != 0 && unsafe { entity_type_matches(animal, RVA_ANIMAL_TYPE_CHECK) } {
                save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, 0u32);
            }
        }
        self.write(staff_offsets::ESCAPED_ANIMAL, 0u32);
    }

    /// `ZTStaff::ZTStaff` (`0x00457a0c`): the `ZTUnit` constructor, an empty assigned-habitat list (its sentinel from the
    /// 12-byte freelist) and cleared vectors, then the `ZTStaff` vtable.
    pub fn construct(self, staff_type: u32) {
        unsafe { ZTUNIT_CONSTRUCTOR.original()(self.0 as *const u32, staff_type) };
        unsafe { LIST_UINT_CTOR.original()((self.0 + staff_offsets::ASSIGNED_HABITATS) as *const c_void, 0) };
        for offset in (staff_offsets::GATE_TILES_BEGIN..=staff_offsets::SKIP_TILES_CAPACITY).step_by(4) {
            self.write(offset, 0u32);
        }
        self.write(0, get_module_base("zoo.exe") as u32 + STAFF_VTABLE_RVA);
    }

    /// `ZTStaff::~ZTStaff` (`0x004f9d09`, non-deleting): restores the `ZTStaff` vtable, returns the two skip vectors'
    /// and the gate-tile vector's buffers and every list node to the pool, then runs the `ZTUnit` destructor. Returns
    /// what that destructor returns.
    pub fn destruct(self) -> *const u32 {
        self.write(0, get_module_base("zoo.exe") as u32 + STAFF_VTABLE_RVA);
        for (begin_offset, capacity_offset, element_bytes) in [
            (staff_offsets::SKIP_TILES_BEGIN, staff_offsets::SKIP_TILES_CAPACITY, 4i32),
            (staff_offsets::SKIP_ENTITIES_BEGIN, staff_offsets::SKIP_ENTITIES_CAPACITY, 4),
            (staff_offsets::GATE_TILES_BEGIN, staff_offsets::GATE_TILES_CAPACITY, 12),
        ] {
            let begin: u32 = self.read(begin_offset);
            if begin != 0 {
                let bytes = (self.read::<u32>(capacity_offset).wrapping_sub(begin) as i32) / element_bytes * element_bytes;
                unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes as u32) };
            }
        }
        let sentinel = self.assigned_list();
        for node in vanilla_list::nodes(sentinel) {
            unsafe { POOLALLOC_DEALLOCATE_N_12.original()(node as *const u32, 1) };
        }
        save_to_memory(sentinel, sentinel);
        save_to_memory(sentinel + 4, sentinel);
        if sentinel != 0 {
            unsafe { POOLALLOC_DEALLOCATE.original()(sentinel as *const u32, 0xc) };
        }
        unsafe { ZTUNIT_DESTRUCTOR_0.original()(self.0 as *const u32) }
    }

    /// The `ZTStaffType` this unit's entity type is, or `0` when it is missing or not a staff type.
    fn staff_type(self) -> u32 {
        if unsafe { entity_type_matches(self.0, STAFF_TYPE_TAG_RVA) } {
            self.read(ENTITY_TYPE_OFFSET)
        } else {
            0
        }
    }

    /// `ZTStaff::initStatusVars` (`0x00457a83`): the `ZTUnit` reset, then the two countdowns: `+0x288` is `-1` when the
    /// staff type's first period (`+0x1b4`) is `0`, else one shared-RNG draw modulo it; `+0x28c` likewise from the second
    /// period (`+0x1b8`), and `+0x290` is cleared. Returns vanilla's `EAX`: `-1` when the second period is `0`, otherwise
    /// the second draw's quotient. A unit whose type is not a staff type is treated as having both periods `0`
    /// (vanilla reads through a null type there and faults).
    pub fn init_status_vars(self) -> u32 {
        unsafe { ZTUNIT_INIT_STATUS_VARS.original()(self.0 as *const u32) };
        let staff_type = self.staff_type();
        let period = |offset: u32| if staff_type == 0 { 0 } else { get_from_memory::<i32>(staff_type + offset) };
        let first = period(TYPE_INITIAL_PERIOD_OFFSET);
        let initial = if first == 0 { -1 } else { next_game_random() as i32 % first };
        self.write(staff_offsets::INITIAL_COUNTDOWN, initial);
        let second = period(TYPE_CONDITIONALS_PERIOD_OFFSET);
        if second == 0 {
            self.write(staff_offsets::ESCAPED_ANIMAL, 0u32);
            self.write(staff_offsets::CONDITIONALS_COUNTDOWN, -1i32);
            return u32::MAX;
        }
        let draw = next_game_random() as i32;
        self.write(staff_offsets::ESCAPED_ANIMAL, 0u32);
        self.write(staff_offsets::CONDITIONALS_COUNTDOWN, draw % second);
        (draw / second) as u32
    }

    /// Whether the unit is kept out of the staff panel and the global staff list.
    fn excluded_from_panel(self) -> bool {
        self.read::<u8>(EXCLUDED_FLAG_B_OFFSET) != 0 || self.read::<u8>(EXCLUDED_FLAG_A_OFFSET) != 0
    }

    /// The sentinel of the global staff list (`DAT_00639474`'s value).
    fn global_staff_list() -> u32 {
        get_from_memory(get_module_base("zoo.exe") as u32 + STAFF_LIST_RVA)
    }

    /// `ZTStaff::addToMap` (`0x00449ed0`): `ZTUnit::addToMap`; on success a unit not excluded by `+0x142`/`+0x143` is
    /// appended to the global staff list and handed to `ZTUI::multistaff::addEntity`. Returns `ZTUnit`'s result.
    pub fn add_to_map(self) -> bool {
        let added = unsafe { ZTUNIT_ADD_TO_MAP.original()(self.0 as *const u32) };
        if added && !self.excluded_from_panel() {
            vanilla_list::push_back(Self::global_staff_list(), self.0);
            unsafe { MULTISTAFF_ADD_ENTITY.original()(self.0 as *const u32) };
        }
        added
    }

    /// `ZTStaff::removeFromMap` (`0x004586f6`): a unit not excluded by `+0x142`/`+0x143` leaves the staff panel and the
    /// global staff list; unless `keep_escaped_animal`, the escaped animal being chased is released; then
    /// `ZTUnit::removeFromMap`.
    pub fn remove_from_map(self, keep_escaped_animal: bool) {
        if !self.excluded_from_panel() {
            let remove_entity = unsafe { transmute::<u32, unsafe extern "cdecl" fn(u32, u32)>(MULTISTAFF_REMOVE_ENTITY) };
            unsafe { remove_entity(self.0, 0) };
            vanilla_list::remove_matching(Self::global_staff_list(), self.0);
        }
        if !keep_escaped_animal {
            self.done_capture();
        }
        unsafe { ZTUNIT_REMOVE_FROM_MAP.original()(self.0 as *const u32, keep_escaped_animal) };
    }

    /// `ZTStaff::save` (`0x004795f9`): the two countdowns, the assigned-habitat count, the gate-tile `x` and `y` of each
    /// assigned habitat (`z` is not saved), the escaped-animal id, then `ZTUnit::save`. Every part is written whatever
    /// earlier writes did; the result is all of them together. A habitat with no gate tile saves `(-1, -1)`, which
    /// resolves to no habitat on load (vanilla reads through the null tile and faults).
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = write_field(self.0 + staff_offsets::INITIAL_COUNTDOWN, 4, file);
        ok &= write_field(self.0 + staff_offsets::CONDITIONALS_COUNTDOWN, 4, file);
        let habitats = self.assigned_habitats();
        let count = habitats.len() as u32;
        ok &= write_field(&count as *const u32 as u32, 4, file);
        for habitat in habitats {
            let tile = unsafe { GET_GATE_TILE_IN.hooked()(habitat as *const u32) } as u32;
            let (x, y): (i32, i32) = if tile == 0 { (-1, -1) } else { (get_from_memory(tile + TILE_X_OFFSET), get_from_memory(tile + TILE_Y_OFFSET)) };
            let wrote_x = write_field(&x as *const i32 as u32, 4, file);
            let wrote_y = write_field(&y as *const i32 as u32, 4, file);
            ok &= wrote_x & wrote_y;
        }
        ok &= write_field(self.0 + staff_offsets::ESCAPED_ANIMAL, 4, file);
        let unit_ok = unsafe { ZTUNIT_SAVE.original()(self.0 as *const u32, file) };
        unit_ok & ok
    }

    /// `ZTStaff::load` (`0x00455370`): the counterpart of [`Self::save`], each field present from its save version
    /// (`+0x288` from `0x10`, `+0x28c` from `0x16`, the gate-tile records from `0x1a`, the escaped-animal id from `0x4b`).
    /// Records go into the gate-tile vector for `reassignHabitats` to resolve. A failed read ends the load without running
    /// `ZTUnit::load`.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let mut ok = true;
        if version >= VERSION_INITIAL_COUNTDOWN {
            ok = read_field(self.0 + staff_offsets::INITIAL_COUNTDOWN, 4, file);
        }
        if version >= VERSION_CONDITIONALS_COUNTDOWN {
            ok &= read_field(self.0 + staff_offsets::CONDITIONALS_COUNTDOWN, 4, file);
        }
        if version >= VERSION_GATE_TILES {
            let mut count: i32 = 0;
            ok &= read_field(&mut count as *mut i32 as u32, 4, file);
            if !ok {
                return false;
            }
            if count != 0 {
                let mut remaining = count.wrapping_sub(1);
                loop {
                    let (mut x, mut y): (u32, u32) = (0, 0);
                    let read_x = read_field(&mut x as *mut u32 as u32, 4, file);
                    let read_y = read_field(&mut y as *mut u32 as u32, 4, file);
                    ok &= read_x & read_y;
                    if !ok {
                        return false;
                    }
                    vanilla_vector::push_words(self.0 + staff_offsets::GATE_TILES_BEGIN, [x, y, 0], |begin, byte_capacity| {
                        if begin != 0 {
                            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, byte_capacity / 12 * 12) };
                        }
                    });
                    let more = remaining != 0;
                    remaining = remaining.wrapping_sub(1);
                    if !more {
                        break;
                    }
                }
            }
        }
        if version >= VERSION_ESCAPED_ANIMAL {
            ok &= read_field(self.0 + staff_offsets::ESCAPED_ANIMAL, 4, file);
        }
        if !ok {
            return false;
        }
        let unit_ok = unsafe { ZTUNIT_LOAD.original()(self.0 as *const u32, file, version) };
        unit_ok & ok
    }

    /// `ZTStaff::reassignHabitats` (`0x00595237`): resolves each saved gate-tile record to the habitat on that tile,
    /// assigns it (the world habitat and absent tiles are skipped) and empties the record vector. Always succeeds; the
    /// returned `1` is vanilla's `AL`.
    pub fn reassign_habitats(self) -> u32 {
        let begin: u32 = self.read(staff_offsets::GATE_TILES_BEGIN);
        let end: u32 = self.read(staff_offsets::GATE_TILES_END);
        if !globals().zthabitatmgr_ptr().is_null() {
            for record in (0..(end.wrapping_sub(begin) as i32) / 12).map(|index| begin + index as u32 * 12) {
                let habitat = globals().zthabitatmgr().get_habitat_ptr(get_from_memory(record), get_from_memory(record + 4));
                self.assign_habitat(habitat);
            }
        }
        self.write(staff_offsets::GATE_TILES_END, begin);
        1
    }

    /// `ZTStaff::initAfterLoad` (`0x005952dc`): `reassignHabitats`, then `ZTUnit::initAfterLoad`; true when both succeed.
    pub fn init_after_load(self, param: u32) -> bool {
        let reassigned = self.reassign_habitats();
        let unit_ok = unsafe { ZTUNIT_INIT_AFTER_LOAD.original()(self.0 as *const u32, param) };
        unit_ok & low_byte_bool(reassigned)
    }

    /// `ZTStaff::doConditionals` (`0x00440602`): counts `+0x28c` down; when it was `0` it reloads from the staff type's
    /// second period minus one and, unless the unit is on the predator goal (`0x75`), records whether the virtual
    /// `findPredator` (`+0x2fc`, argument `0`) found something in the pending-predator byte `+0x24c`. Vanilla then builds
    /// and frees a `"Saw escaped animal"` string that nothing reads; that allocation round trip leaves the pool as it was
    /// and is not reproduced. A unit with no staff type reloads `-1` (vanilla reads through the null type and faults).
    pub fn do_conditionals(self) {
        #[cfg(feature = "reimplementation-tests")]
        if live_support::conditionals_probe::intercept() {
            return;
        }
        let remaining: i32 = self.read(staff_offsets::CONDITIONALS_COUNTDOWN);
        self.write(staff_offsets::CONDITIONALS_COUNTDOWN, remaining.wrapping_sub(1));
        if remaining != 0 {
            return;
        }
        let staff_type = self.staff_type();
        let period = if staff_type == 0 { 0 } else { get_from_memory::<i32>(staff_type + TYPE_CONDITIONALS_PERIOD_OFFSET) };
        self.write(staff_offsets::CONDITIONALS_COUNTDOWN, period.wrapping_sub(1));
        if self.read::<u32>(UNIT_GOAL_ID_OFFSET) == GOAL_PREDATOR {
            return;
        }
        let found = call_slot_1(self.0, SLOT_FIND_PREDATOR, 0) != 0;
        self.write(UNIT_PREDATOR_CHECK_PENDING_OFFSET, found as u8);
    }

    /// `ZTStaff::setTargetTile` (`0x00416692`): clears the walking-to mark (`+0x85` bit `0x4`) on the old target tile, runs
    /// `ZTUnit::setTargetTile`, then marks the new target tile.
    pub fn set_target_tile(self, tile: u32, flag: bool) {
        let mark = |set: bool| {
            let target: u32 = self.read(UNIT_TARGET_TILE_OFFSET);
            if target != 0 {
                let flags: u8 = get_from_memory(target + TILE_FLAGS_85_OFFSET);
                save_to_memory(target + TILE_FLAGS_85_OFFSET, if set { flags | 0x4 } else { flags & 0xfb });
            }
        };
        mark(false);
        unsafe { ZTUNIT_SET_TARGET_TILE.original()(self.0 as *const u32, tile as *const u32, flag) };
        mark(true);
    }

    /// `ZTStaff::pickRandomDest` (`0x00423a7e`): `position` receives a destination tile's position. A unit in a non-exhibit
    /// habitat (the habitat of its own tile, virtual `+0x248`, with `+0x2c` clear) goes to that habitat's pass-out gate
    /// tile; one in an exhibit goes to the exit gate of the habitat its virtual `getRandomAssignedHabitat` (`+0x328`)
    /// picks. Without such a gate, or when the gate is the unit's own tile, `ZTUnit::pickRandomDest` decides. Returns `1`
    /// for a gate destination, `0` when the pass-out gate does not exist, else `ZTUnit`'s result. A unit whose own tile has
    /// no habitat defers to `ZTUnit` (vanilla reads through the null habitat and faults).
    pub fn pick_random_dest(self, position: u32) -> u32 {
        let own_tile = Self::tile_of(self.0);
        let habitat = call_slot_1(self.0, SLOT_GET_HABITAT_FROM_TILE, own_tile);
        let defer = || unsafe { ZTUNIT_PICK_RANDOM_DEST.original()(self.0 as *const u32, position as *const IVec3) };
        if habitat == 0 {
            return defer();
        }
        let gate = if get_from_memory::<u8>(habitat + HABITAT_IS_WORLD_OFFSET) == 0 {
            unsafe { GET_GATE_TILE_PASS_OUT.hooked()(habitat as *const u32, self.0 as *const u32) as u32 }
        } else {
            let assigned = call_slot_0(self.0, SLOT_GET_RANDOM_ASSIGNED_HABITAT);
            let exit = if assigned == 0 { 0 } else { unsafe { GET_GATE_TILE_OUT.hooked()(assigned as *const u32) as u32 } };
            if exit == 0 || exit == own_tile {
                return defer();
            }
            exit
        };
        if gate == 0 {
            return 0;
        }
        for word in 0..3u32 {
            save_to_memory(position + word * 4, get_from_memory::<u32>(gate + TILE_X_OFFSET + word * 4));
        }
        1
    }

    fn tile_of(entity: u32) -> u32 {
        unsafe { BFENTITY_GET_TILE.original()(entity as *const u32) as u32 }
    }

    /// The entity with `id` in the live world, `0` when it is gone.
    fn world_entity(id: u32) -> u32 {
        let world: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA);
        let get_entity = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(world) + WORLD_GET_ENTITY_SLOT)) };
        get_entity(world, id, 1)
    }

    /// `ZTStaff::foundEscapedAnimal` (`0x00423dae`): picks the escaped animal nearest (Manhattan distance between tiles) to
    /// this unit among those nobody else is chasing (`+0x374` clear, or naming an entity that no longer exists) and that
    /// `ZTAnimal::canCapture` accepts, stores its id at `+0x290` and writes this unit's id into the animal's `+0x374`.
    /// Returns whether one was found. A chosen animal that is gone by the time it is claimed is left alone (vanilla writes
    /// through a null pointer there).
    pub fn found_escaped_animal(self) -> bool {
        self.write(staff_offsets::ESCAPED_ANIMAL, 0u32);
        let own_tile = Self::tile_of(self.0);
        let sentinel: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + ESCAPED_ANIMAL_LIST_RVA);
        let mut best = 0i32;
        let mut node: u32 = get_from_memory(sentinel);
        while node != sentinel {
            let animal = vanilla_list::value(node);
            let chaser: u32 = get_from_memory(animal + ANIMAL_CAPTURED_BY_OFFSET);
            let taken = chaser != 0 && Self::world_entity(chaser) != 0;
            if !taken && crate::ztanimal::ZTAnimal(animal).can_capture(self.0) {
                let animal_tile = Self::tile_of(animal);
                let distance = if own_tile == 0 || animal_tile == 0 {
                    i32::MAX
                } else {
                    let delta = |offset: u32| get_from_memory::<i32>(animal_tile + offset).wrapping_sub(get_from_memory::<i32>(own_tile + offset)).wrapping_abs();
                    delta(TILE_X_OFFSET).wrapping_add(delta(TILE_Y_OFFSET))
                };
                if self.read::<u32>(staff_offsets::ESCAPED_ANIMAL) == 0 || distance < best {
                    best = distance;
                    self.write(staff_offsets::ESCAPED_ANIMAL, get_from_memory::<u32>(animal + ENTITY_ID_OFFSET));
                }
            }
            node = get_from_memory(node);
        }
        let chosen: u32 = self.read(staff_offsets::ESCAPED_ANIMAL);
        if chosen == 0 {
            return false;
        }
        let animal = Self::world_entity(chosen);
        if animal != 0 && unsafe { entity_type_matches(animal, RVA_ANIMAL_TYPE_CHECK) } {
            save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, self.read::<u32>(ENTITY_ID_OFFSET));
        }
        true
    }

    /// `ZTStaff::fencesOK` (`0x00440524`): whether the step from `tile` in `direction` crosses no fence a staff member
    /// cannot pass. Both bytes at `out_exit_fence`/`out_unused` are cleared first (the second is never set). A missing tile
    /// or neighbour is `false`; direction `-1` is `true`. A fence on the tile's edge `direction` or the neighbour's edge
    /// `(direction - 4) & 7` blocks the step unless it is not blocking (`+0x164`), its type is always passable (`+0x191`),
    /// or it is a gate (type name starting with `'g'`) that is not a tank wall with its flag (`+0x464`) set. When the step
    /// is allowed and either fence's type is always passable, `*out_exit_fence` is set.
    pub fn fences_ok(self, tile: u32, direction: i32, out_exit_fence: u32, out_unused: u32) -> bool {
        for out in [out_exit_fence, out_unused] {
            if out != 0 {
                save_to_memory(out, 0u8);
            }
        }
        if tile == 0 {
            return false;
        }
        if direction == -1 {
            return true;
        }
        let world: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.original()((world + 8) as *const u32, tile as *const u32, direction as u32) };
        if neighbour == 0 {
            return false;
        }
        let near = edge_fence(tile, direction);
        let far = edge_fence(neighbour, direction.wrapping_sub(4) & 7);
        if near == 0 && far == 0 {
            return true;
        }
        let fence_type = |fence: u32| get_from_memory::<u32>(fence + ENTITY_TYPE_OFFSET);
        let tank_flag = |fence: u32| {
            fence != 0
                && get_from_memory::<u8>(fence_type(fence) + FENCE_TYPE_TANK_WALL_FLAG_OFFSET) != 0
                && unsafe { entity_type_matches(fence, TANKWALL_TAG_RVA) }
                && get_from_memory::<u8>(fence + TANK_WALL_FLAG_OFFSET) != 0
        };
        let always_passable = |fence: u32| fence != 0 && get_from_memory::<u8>(fence_type(fence) + FENCE_TYPE_PASSABLE_FLAG_OFFSET) != 0;
        let blocks = |fence: u32| {
            fence != 0
                && get_from_memory::<u8>(fence + FENCE_NOT_BLOCKING_OFFSET) == 0
                && !always_passable(fence)
                && !(is_gate(fence) && !tank_flag(fence))
        };
        if blocks(near) || blocks(far) {
            return false;
        }
        if out_exit_fence != 0 && (always_passable(near) || always_passable(far)) {
            save_to_memory(out_exit_fence, 1u8);
        }
        true
    }

    /// `ZTStaff::getTerrainCost` (`0x00439c80`): `1` for the tile the unit stands on, the path-cost sentinel for a tile
    /// that is maximally costly for everyone, the sentinel minus 4 for a steep tile, otherwise `1` when the tile's flag
    /// `+0x83 & 0x8` is set and `2` when it is clear.
    pub fn get_terrain_cost(self, tile: u32) -> i32 {
        let this = self.0 as *const u32;
        let tile_ptr = tile as *const u32;
        let max_cost = || get_from_memory::<i32>(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        if unsafe { IS_SAME_TILE.original()(this, tile_ptr) } != 0 {
            return 1;
        }
        if low_byte_bool(unsafe { IS_MAX_COST_FOR_ALL.original()(this, tile_ptr) }) {
            return max_cost();
        }
        if low_byte_bool(unsafe { IS_NON_PATH_STEEP.original()(tile as i32) }) {
            return max_cost() - 4;
        }
        if get_from_memory::<u8>(tile + TILE_FLAGS_83_OFFSET) & 0x8 != 0 {
            1
        } else {
            2
        }
    }
}

/// A virtual call on `this` taking no argument and returning `EAX`.
fn call_slot_0(this: u32, offset: u32) -> u32 {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(this) + offset)) };
    function(this)
}

/// A virtual call on `this` taking one argument and returning `EAX`.
fn call_slot_1(this: u32, offset: u32, argument: u32) -> u32 {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(this) + offset)) };
    function(this, argument)
}

/// The fence on `tile`'s edge `direction` (the edge slots hold two directions each), `0` when the slot is empty or holds
/// something other than a fence.
fn edge_fence(tile: u32, direction: i32) -> u32 {
    let slot = tile.wrapping_add(TILE_EDGE_SLOTS_OFFSET).wrapping_add(((direction / 2) as u32).wrapping_mul(4));
    let fence: u32 = get_from_memory(slot);
    if fence != 0 && unsafe { entity_type_matches(fence, FENCE_TAG_RVA) } {
        fence
    } else {
        0
    }
}

/// Whether a fence's type name starts with `'g'`: a gate.
fn is_gate(fence: u32) -> bool {
    let name: u32 = get_from_memory(get_from_memory::<u32>(fence + ENTITY_TYPE_OFFSET) + FENCE_TYPE_NAME_OFFSET);
    name != 0 && get_from_memory::<u8>(name) == b'g'
}

#[detour_mod]
mod staff_detours {
    use super::*;

    #[detour(IS_HABITAT_ASSIGNED)]
    unsafe extern "thiscall" fn is_habitat_assigned(this: *const u32, habitat: *const u32) -> bool {
        ZTStaff(this as u32).is_habitat_assigned(habitat as u32)
    }

    #[detour(REMOVE_ASSIGNED_HABITAT)]
    unsafe extern "thiscall" fn remove_assigned_habitat(this: *const u32, habitat: *const u32) {
        ZTStaff(this as u32).remove_assigned_habitat(habitat as u32);
    }

    #[detour(ASSIGN_HABITAT)]
    unsafe extern "thiscall" fn assign_habitat(this: *const u32, habitat: *const u32) {
        ZTStaff(this as u32).assign_habitat(habitat as u32);
    }

    #[detour(CLEAR_ASSIGNED_HABITATS)]
    unsafe extern "thiscall" fn clear_assigned_habitats(this: *const u32) {
        ZTStaff(this as u32).clear_assigned_habitats();
    }

    #[detour(GET_ASSIGNED_HABITAT)]
    unsafe extern "thiscall" fn get_assigned_habitat(this: *const u32, index: i32) -> *const u32 {
        ZTStaff(this as u32).get_assigned_habitat(index) as *const u32
    }

    #[detour(REMOVE_ASSIGNED_HABITAT_AT)]
    unsafe extern "thiscall" fn remove_assigned_habitat_at(this: *const u32, index: i32) {
        ZTStaff(this as u32).remove_assigned_habitat_at(index);
    }

    #[detour(GET_RANDOM_ASSIGNED_HABITAT)]
    unsafe extern "thiscall" fn get_random_assigned_habitat(this: *const u32) -> u32 {
        ZTStaff(this as u32).get_random_assigned_habitat()
    }

    #[detour(GET_MONTHLY_COST)]
    unsafe extern "thiscall" fn get_monthly_cost(this: *const u32) -> f32 {
        ZTStaff(this as u32).get_monthly_cost()
    }

    #[detour(DROP)]
    unsafe extern "thiscall" fn drop_staff(this: *const u32) {
        ZTStaff(this as u32).drop_staff();
    }

    #[detour(DONE_CAPTURE)]
    unsafe extern "thiscall" fn done_capture(this: *const u32) {
        ZTStaff(this as u32).done_capture();
    }

    #[detour(GET_TERRAIN_COST)]
    unsafe extern "thiscall" fn get_terrain_cost(this: *const u32, tile: *const u32) -> i32 {
        ZTStaff(this as u32).get_terrain_cost(tile as u32)
    }

    #[detour(DO_CONDITIONALS)]
    unsafe extern "thiscall" fn do_conditionals(this: *const u32) {
        ZTStaff(this as u32).do_conditionals()
    }

    #[detour(SET_TARGET_TILE)]
    unsafe extern "thiscall" fn set_target_tile(this: *const u32, tile: *const u32, flag: bool) {
        ZTStaff(this as u32).set_target_tile(tile as u32, flag)
    }

    #[detour(PICK_RANDOM_DEST)]
    unsafe extern "thiscall" fn pick_random_dest(this: *const u32, position: *const IVec3) -> u32 {
        ZTStaff(this as u32).pick_random_dest(position as u32)
    }

    #[detour(FOUND_ESCAPED_ANIMAL)]
    unsafe extern "thiscall" fn found_escaped_animal(this: *const u32) -> u32 {
        ZTStaff(this as u32).found_escaped_animal() as u32
    }

    #[detour(FENCES_OK)]
    unsafe extern "thiscall" fn fences_ok(this: *const u32, tile: *const u32, direction: u32, out_exit_fence: *const bool, out_unused: *const bool) -> u32 {
        ZTStaff(this as u32).fences_ok(tile as u32, direction as i32, out_exit_fence as u32, out_unused as u32) as u32
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const u32, staff_type: u32) -> *const u32 {
        ZTStaff(this as u32).construct(staff_type);
        this
    }

    #[detour(DESTRUCTOR_0)]
    unsafe extern "thiscall" fn destructor(this: *const u32) -> *const u32 {
        ZTStaff(this as u32).destruct()
    }

    /// The scalar-deleting destructor: destroys, then frees when bit 0 of `flags` is set.
    #[detour(DESTRUCTOR_1)]
    unsafe extern "thiscall" fn destructor_deleting(this: *const u32, flags: u8) -> *const u32 {
        ZTStaff(this as u32).destruct();
        if flags & 1 != 0 {
            unsafe { OPERATOR_DELETE.original()(this as u32) };
        }
        this
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) -> u32 {
        ZTStaff(this as u32).init_status_vars()
    }

    #[detour(ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> bool {
        ZTStaff(this as u32).add_to_map()
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, keep_escaped_animal: bool) {
        ZTStaff(this as u32).remove_from_map(keep_escaped_animal);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTStaff(this as u32).save(file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTStaff(this as u32).load(file, version)
    }

    #[detour(REASSIGN_HABITATS)]
    unsafe extern "thiscall" fn reassign_habitats(this: *const u32) -> u32 {
        ZTStaff(this as u32).reassign_habitats()
    }

    #[detour(INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn init_after_load(this: *const u32, param: u32) -> bool {
        ZTStaff(this as u32).init_after_load(param)
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn is_habitat_assigned(this: u32, habitat: u32) -> bool {
            unsafe { super::IS_HABITAT_ASSIGNED_DETOUR.call(this as *const u32, habitat as *const u32) }
        }

        pub(crate) fn remove_assigned_habitat(this: u32, habitat: u32) {
            unsafe { super::REMOVE_ASSIGNED_HABITAT_DETOUR.call(this as *const u32, habitat as *const u32) }
        }

        pub(crate) fn assign_habitat(this: u32, habitat: u32) {
            unsafe { super::ASSIGN_HABITAT_DETOUR.call(this as *const u32, habitat as *const u32) }
        }

        pub(crate) fn clear_assigned_habitats(this: u32) {
            unsafe { super::CLEAR_ASSIGNED_HABITATS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_assigned_habitat(this: u32, index: i32) -> u32 {
            unsafe { super::GET_ASSIGNED_HABITAT_DETOUR.call(this as *const u32, index) as u32 }
        }

        pub(crate) fn remove_assigned_habitat_at(this: u32, index: i32) {
            unsafe { super::REMOVE_ASSIGNED_HABITAT_AT_DETOUR.call(this as *const u32, index) }
        }

        pub(crate) fn get_random_assigned_habitat(this: u32) -> u32 {
            unsafe { super::GET_RANDOM_ASSIGNED_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_monthly_cost(this: u32) -> f32 {
            unsafe { super::GET_MONTHLY_COST_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn drop_staff(this: u32) {
            unsafe { super::DROP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn done_capture(this: u32) {
            unsafe { super::DONE_CAPTURE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn constructor(this: u32, staff_type: u32) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, staff_type) }
        }

        pub(crate) fn destructor(this: u32) -> *const u32 {
            unsafe { super::DESTRUCTOR_0_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn destructor_deleting(this: u32, flags: u8) -> *const u32 {
            unsafe { super::DESTRUCTOR_1_DETOUR.call(this as *const u32, flags) }
        }

        pub(crate) fn init_status_vars(this: u32) -> u32 {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn add_to_map(this: u32) -> bool {
            unsafe { super::ADD_TO_MAP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn remove_from_map(this: u32, keep_escaped_animal: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, keep_escaped_animal) }
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }

        pub(crate) fn reassign_habitats(this: u32) -> u32 {
            unsafe { super::REASSIGN_HABITATS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn init_after_load(this: u32, param: u32) -> bool {
            unsafe { super::INIT_AFTER_LOAD_DETOUR.call(this as *const u32, param) }
        }

        pub(crate) fn do_conditionals(this: u32) {
            unsafe { super::DO_CONDITIONALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_target_tile(this: u32, tile: u32, flag: bool) {
            unsafe { super::SET_TARGET_TILE_DETOUR.call(this as *const u32, tile as *const u32, flag) }
        }

        pub(crate) fn pick_random_dest(this: u32, position: u32) -> u32 {
            unsafe { super::PICK_RANDOM_DEST_DETOUR.call(this as *const u32, position as *const glam::i32::IVec3) }
        }

        pub(crate) fn found_escaped_animal(this: u32) -> u32 {
            unsafe { super::FOUND_ESCAPED_ANIMAL_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn fences_ok(this: u32, tile: u32, direction: u32, out_exit_fence: u32, out_unused: u32) -> u32 {
            unsafe { super::FENCES_OK_DETOUR.call(this as *const u32, tile as *const u32, direction, out_exit_fence as *const bool, out_unused as *const bool) }
        }

        pub(crate) fn get_terrain_cost(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_TERRAIN_COST_DETOUR.call(this as *const u32, tile as *const u32) }
        }
    }
}

/// Installs `ZTStaff`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTSTAFF_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { staff_detours::init_detours() } {
        error!("Failed to initialise ztstaff detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_NEW;

    use super::{staff_detours::test_real, ZTStaff, STAFF_SIZE};

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::staff_detours::status()
    }

    /// Zeroed staff-sized memory from vanilla's allocator, unconstructed.
    pub fn allocate_zeroed_staff() -> ZTStaff {
        let memory = unsafe { OPERATOR_NEW.original()(STAFF_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, STAFF_SIZE as usize) };
        ZTStaff(memory)
    }

    /// A `ZTStaff` built by the real vanilla constructor over `staff_type` (any `ZTStaffType` pointer). Its vtable is the
    /// base `ZTStaff` one.
    pub fn build_standalone_staff(staff_type: u32) -> ZTStaff {
        let staff = allocate_zeroed_staff();
        test_real::constructor(staff.0, staff_type);
        staff
    }

    /// A `ZTStaff` built by the Rust constructor over `staff_type`.
    pub fn build_standalone_staff_port(staff_type: u32) -> ZTStaff {
        let staff = allocate_zeroed_staff();
        staff.construct(staff_type);
        staff
    }

    /// Tears a staff down through the real scalar-deleting destructor, which also frees the memory.
    pub fn destroy_standalone_staff(staff: ZTStaff) {
        test_real::destructor_deleting(staff.0, 1);
    }

    pub(crate) use super::staff_detours::test_real as real;

    /// While active, `ZTStaff::do_conditionals` only counts its calls: the subclasses' conditionals end in it, and
    /// running it over a bare fixture reaches live-game state the fixture does not have.
    pub(crate) mod conditionals_probe {
        use std::sync::atomic::{AtomicU32, Ordering};

        static ACTIVE: AtomicU32 = AtomicU32::new(0);
        static CALLS: AtomicU32 = AtomicU32::new(0);

        pub(crate) fn set_active(active: bool) {
            ACTIVE.store(active as u32, Ordering::SeqCst);
        }

        /// Zeroes the call count.
        pub(crate) fn reset() {
            CALLS.store(0, Ordering::SeqCst);
        }

        /// How many calls were swallowed since [`reset`].
        pub(crate) fn calls() -> u32 {
            CALLS.load(Ordering::SeqCst)
        }

        pub(in crate::ztstaff) fn intercept() -> bool {
            if ACTIVE.load(Ordering::SeqCst) == 0 {
                return false;
            }
            CALLS.fetch_add(1, Ordering::SeqCst);
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::staff_offsets::*;

    #[test]
    fn layout_is_contiguous() {
        assert_eq!(ASSIGNED_HABITATS + 4, GATE_TILES_BEGIN);
        assert_eq!(GATE_TILES_CAPACITY + 4, SKIP_ENTITIES_BEGIN);
        assert_eq!(SKIP_ENTITIES_CAPACITY + 4, SKIP_TILES_BEGIN);
        assert_eq!(SKIP_TILES_CAPACITY + 4, INITIAL_COUNTDOWN);
        assert_eq!(ESCAPED_ANIMAL + 4, super::STAFF_SIZE);
    }
}
