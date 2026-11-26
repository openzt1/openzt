//! `ZTMaint`: the maintenance worker entity, a `ZTStaff` subclass (`ZTUnit` -> `BFUnit` -> `BFEntity`), plus the
//! small `ZTMaintTaskPool` helpers that share its task nodes.
//!
//! Vanilla-layout-compatible (style 1): maintenance workers are heap entities that un-ported `ZTStaff`/`ZTUnit`/
//! `BFEntity` code, the goal classes and `ZTMaintTaskPool` read by raw offset, so the object stays vanilla memory
//! and this module reads and writes it in place. A worker is `0x2c4` bytes: the `ZTStaff` prefix (`0x000`-`0x293`)
//! followed by the fields below. Nothing here owns an allocation. Task nodes (`0x10` bytes) and the pool's list
//! nodes come from vanilla's allocator (`ZTMaintTaskPool::listen`); this module frees them only through the same
//! paths vanilla uses (`operator_delete` for a task, the 12-byte small-block freelist for a list node).
//!
//! The constructor and destructors stay vanilla: the constructor builds the task-availability bit vector through
//! vanilla's `vector<bool>` helpers (`0x4c5e75` and friends, not in `generated.rs`), and the destructor frees that
//! vector through the small-block freelist. Neither is hot, and a Rust copy would only duplicate them.
//!
//! `ZTMaint::doConditionals` (`0x439c6e`, a bare 5-byte `JMP ZTStaff::doConditionals`) and
//! `getRandomAssignedHabitat` (`0x4a95c9`, `XOR EAX,EAX; RET`, 3 bytes, referenced only by the vtable) are too short
//! for a detour and stay vanilla. `getTerrainCost` forwards to `ZTStaff::getTerrainCost` and is left as is.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfcoggoal::SET_ID as COG_GOAL_SET_ID,
    bfentity::UPDATE as BFENTITY_UPDATE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bfunit::IDLE as BFUNIT_IDLE,
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ztfence::IS_WORTH_FIXING as ZTFENCE_IS_WORTH_FIXING,
    ztguest::F_ROUTINE,
    ztmaint::{
        FIND_FENCE_TO_FIX_ON_TILE, FINISH_TASK, INIT_STATUS_VARS, IS_TARGETED, IS_TASK_AVAILABLE, LOAD, REMOVE_FROM_MAP, SAVE, SET_NEW_TASK, UPDATE,
        UPDATE_GOALS,
    },
    ztmainttaskpool::{CLEAR, GET_EVENTS, LISTEN},
    ztunit::UPDATE_GOALS as ZTUNIT_UPDATE_GOALS,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, save_to_memory},
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3, read_field, unit_offsets, write_field},
    ztmegatilemgr::entity_type_matches,
    vanilla_list,
    ztstaff::ZTStaff,
};

/// `ZTMaintType::createEntity` allocates this many bytes.
pub const MAINT_SIZE: u32 = 0x2c4;

/// Vtable address of `ZTMaint` (`0x0062ea54`) as an RVA from the `zoo.exe` base.
pub const MAINT_VTABLE_RVA: u32 = 0x0022_ea54;

/// Class tag (RVA) the entity type's `isCastClass` takes to test for a maintenance-worker type (`CAST_ZTMaint`).
pub const MAINT_TYPE_TAG_RVA: u32 = 0x0023_8780;

/// Own fields. `0x294`/`0x298` are set to `2` by the constructor and saved; nothing decompiled reads them.
pub const FIELD_0X294_OFFSET: u32 = 0x294;
pub const FIELD_0X298_OFFSET: u32 = 0x298;
/// Countdown the worker's `update` subtracts the frame time from; reloaded from the type's `+0x1fc`.
pub const TIMER_OFFSET: u32 = 0x29c;
/// Zeroed by `initStatusVars`; saved first.
pub const FIELD_0X2A0_OFFSET: u32 = 0x2a0;
/// Byte cleared by the constructor.
pub const FIELD_0X2A4_OFFSET: u32 = 0x2a4;
/// `vector<bool>` of per-task-kind "available" bits: begin word pointer, begin bit offset, end word pointer, end bit
/// offset, end-of-storage pointer. The constructor sizes it to 4 bits.
pub const TASK_BITS_BEGIN_OFFSET: u32 = 0x2a8;
pub const TASK_BITS_BEGIN_BIT_OFFSET: u32 = 0x2ac;
pub const TASK_BITS_END_OFFSET: u32 = 0x2b0;
pub const TASK_BITS_END_BIT_OFFSET: u32 = 0x2b4;
pub const TASK_BITS_CAPACITY_OFFSET: u32 = 0x2b8;
/// `u16`: number of available task kinds, counted by the constructor; saved.
pub const AVAILABLE_TASK_COUNT_OFFSET: u32 = 0x2bc;
/// The worker's current task node (see [`task_offsets`]); null when idle.
pub const CURRENT_TASK_OFFSET: u32 = 0x2c0;

/// Fields of a `0x10`-byte task node, allocated by `ZTMaintTaskPool::listen`.
pub mod task_offsets {
    pub const KIND: u32 = 0x0;
    pub const TARGET_ID: u32 = 0x4;
    /// The worker the task is assigned to; cleared when the worker lets go of the task.
    pub const WORKER: u32 = 0x8;
    pub const FLAG: u32 = 0xc;
}

/// `ZTMaintType`'s task-availability `vector<bool>`: begin word pointer, begin bit offset, end word pointer, end bit
/// offset.
mod type_offsets {
    pub const TASK_BITS_BEGIN: u32 = 0x200;
    pub const TASK_BITS_BEGIN_BIT: u32 = 0x204;
    pub const TASK_BITS_END: u32 = 0x208;
    pub const TASK_BITS_END_BIT: u32 = 0x20c;
    /// Initial value of the worker's countdown timer.
    pub const TIMER_RELOAD: u32 = 0x1fc;
    pub const BLOCK: u32 = 0x80;
    pub const BLOCK_GATE: u32 = 0x84;
}

/// Staff fields `update` and `ZTMaint::updateFenceList` treat as two `{begin, end}` pointer pairs.
mod staff_vectors {
    use crate::ztstaff::staff_offsets;

    pub const FIRST_BEGIN: u32 = staff_offsets::SKIP_ENTITIES_BEGIN;
    pub const FIRST_END: u32 = staff_offsets::SKIP_ENTITIES_END;
    pub const SECOND_BEGIN: u32 = staff_offsets::SKIP_TILES_BEGIN;
    pub const SECOND_END: u32 = staff_offsets::SKIP_TILES_END;
}

/// Vtable slots (byte offsets) dispatched through so subclass overrides are honoured.
mod slots {
    pub const GOAL_SET: u32 = 0x1d8;
    pub const FIND_PREDATOR: u32 = 0x2fc;
    /// `BFCogGoal::valid` (cog goal vtable).
    pub const COG_VALID: u32 = 0x0;
    /// `ZTMaintTaskPool::getEvents` three-argument form (pool vtable).
    pub const POOL_GET_EVENTS: u32 = 0xc;
    /// The pool's one-argument `getEvents(out)` (pool vtable), which `listen` calls.
    pub const POOL_EVENTS_OUT: u32 = 0x8;
}

/// `goalSet` ids.
const GOAL_PREDATOR: u32 = 0x75;
/// The goal `updateGoals` falls back to when the cog goal is invalid or its flag is set.
const GOAL_FALLBACK: u32 = 0x78;
/// Event kind the pool's `getEvents` asks for.
const EVENT_KIND_MAINT: u32 = 0x4d;

/// Globals, as RVAs from the `zoo.exe` base.
mod globals_rva {
    /// `GLOBAL_ZTWorldMgr`; the `BFMap` is embedded at `+0x8`.
    pub const WORLD_MGR: u32 = 0x0023_8040;
    /// Value is the sentinel node of the `ZTMaintTaskPool`'s list (the pool's `+0x4`).
    pub const TASK_LIST: u32 = 0x0023_8fa4;
    /// Value is the sentinel node of the world's entity list (node `+0x0` next, `+0x8` entity).
    pub const ENTITY_LIST: u32 = 0x0023_9474;
    /// `BFCogGoal::setID`'s trailing argument (`DAT_00639360`).
    pub const COG_GOAL_ARG: u32 = 0x0023_9360;
    /// `CAST_ZTFence`.
    pub const FENCE_TAG: u32 = 0x0023_8660;
}

/// Edge slots of a tile: the fence entities on its four edges at `+0x14 + (direction / 2) * 4`.
const TILE_EDGE_SLOTS_OFFSET: u32 = 0x14;
/// A fence's wear value `findFenceToFixOnTile` minimises.
const FENCE_WEAR_OFFSET: u32 = 0x15c;
/// Save versions that change what `load` reads.
mod save_version {
    pub const FIELD_0X2A0_AFTER: u32 = 0x22;
    pub const TASK_STATE_AFTER: u32 = 0x5b;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// A `vector<bool>` in vanilla memory, as the four words `{begin word pointer, begin bit offset, end word pointer,
/// end bit offset}` stored at `address`.
#[derive(Clone, Copy)]
struct BitVec {
    begin: u32,
    begin_bit: u32,
    end: u32,
    end_bit: u32,
}

impl BitVec {
    fn read(begin: u32, begin_bit: u32, end: u32, end_bit: u32) -> Self {
        BitVec { begin: get_from_memory(begin), begin_bit: get_from_memory(begin_bit), end: get_from_memory(end), end_bit: get_from_memory(end_bit) }
    }

    /// Number of bits, as vanilla computes it: whole words between the pointers times 32, minus the begin offset,
    /// plus the end offset.
    fn len(self) -> u32 {
        (((self.end.wrapping_sub(self.begin) as i32) >> 2) << 5).wrapping_sub(self.begin_bit as i32).wrapping_add(self.end_bit as i32) as u32
    }

    /// The word address and mask of bit `index`.
    fn locate(self, index: u32) -> (u32, u32) {
        let position = (self.begin_bit as i32).wrapping_add(index as i32);
        let word = self.begin.wrapping_add(((position >> 5) as u32).wrapping_mul(4));
        (word, 1u32 << (position & 31))
    }

    fn get(self, index: u32) -> bool {
        let (word, mask) = self.locate(index);
        get_from_memory::<u32>(word) & mask != 0
    }

    fn set(self, index: u32, value: bool) {
        let (word, mask) = self.locate(index);
        let current: u32 = get_from_memory(word);
        save_to_memory(word, if value { current | mask } else { current & !mask });
    }
}

/// A maintenance worker in vanilla memory. All reads and writes are volatile through the raw address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTMaint(pub u32);

#[allow(dead_code)]
impl ZTMaint {
    pub fn timer(self) -> i32 {
        get_from_memory(self.0 + TIMER_OFFSET)
    }

    pub fn set_timer(self, value: i32) {
        save_to_memory(self.0 + TIMER_OFFSET, value);
    }

    pub fn current_task(self) -> u32 {
        get_from_memory(self.0 + CURRENT_TASK_OFFSET)
    }

    pub fn set_current_task(self, task: u32) {
        save_to_memory(self.0 + CURRENT_TASK_OFFSET, task);
    }

    pub fn available_task_count(self) -> u16 {
        get_from_memory(self.0 + AVAILABLE_TASK_COUNT_OFFSET)
    }

    fn task_bits(self) -> BitVec {
        BitVec::read(
            self.0 + TASK_BITS_BEGIN_OFFSET,
            self.0 + TASK_BITS_BEGIN_BIT_OFFSET,
            self.0 + TASK_BITS_END_OFFSET,
            self.0 + TASK_BITS_END_BIT_OFFSET,
        )
    }

    /// Number of task-availability bits the worker carries (4 after construction).
    pub fn task_bit_count(self) -> u32 {
        self.task_bits().len()
    }

    pub fn task_bit(self, index: u32) -> bool {
        self.task_bits().get(index)
    }

    pub fn set_task_bit(self, index: u32, value: bool) {
        self.task_bits().set(index, value);
    }

    /// `ZTMaint::fRoutine` and `ZTGuest::fRoutine` (one folded body, `0x0042201e`): report whether the unit is
    /// stopped, then let `BFUnit::idle` run. Always `true`.
    pub fn f_routine(this: u32, done: *const bool) -> bool {
        let stopped: u8 = get_from_memory(this + unit_offsets::STOPPED);
        save_to_memory(done as u32, stopped);
        unsafe { BFUNIT_IDLE.original()(this as *const u32) };
        true
    }

    /// `ZTMaint::update` (`0x00439c06`): counts the timer down by `elapsed`; once it reaches zero the two staff
    /// pointer pairs at `+0x270` and `+0x27c` are emptied (end set to begin) and the timer reloads from the type.
    /// Then `BFEntity::update`.
    pub fn update(self, elapsed: u32) {
        let remaining = self.timer().wrapping_sub(elapsed as i32);
        self.set_timer(remaining);
        if remaining < 1 {
            save_to_memory(self.0 + staff_vectors::FIRST_END, get_from_memory::<u32>(self.0 + staff_vectors::FIRST_BEGIN));
            save_to_memory(self.0 + staff_vectors::SECOND_END, get_from_memory::<u32>(self.0 + staff_vectors::SECOND_BEGIN));
            let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
            if entity_type != 0 {
                self.set_timer(get_from_memory(entity_type + type_offsets::TIMER_RELOAD));
            }
        }
        // `BFEntity::update`; nothing detours it, so `.hooked()` runs the real body.
        unsafe { BFENTITY_UPDATE.hooked()(self.0 as *const u32, elapsed) };
    }

    fn finish_update_goals(self) {
        unsafe { ZTUNIT_UPDATE_GOALS.hooked()(self.0 as *const u32) };
    }

    /// `ZTMaint::updateGoals` (`0x00439c33`), from the `.asm`: a pending predator check (`+0x24c`) off the predator
    /// goal looks for a predator, switching to goal `0x75` and recording it at `+0x22c` when found, clears the check
    /// and hands over to `ZTUnit::updateGoals`. Otherwise an invalid cog goal, or a set goal flag (`+0x1a8`), goes to
    /// goal `0x78` first. Every path ends in `ZTUnit::updateGoals`.
    pub fn update_goals(self) {
        let cog_goal = self.0 + unit_offsets::COG_GOAL;
        if get_from_memory::<u8>(self.0 + unit_offsets::PREDATOR_CHECK_PENDING) != 0 && get_from_memory::<u32>(cog_goal + 0x14) != GOAL_PREDATOR {
            let predator = crate::ztkeeper::call_slot_1(self.0, slots::FIND_PREDATOR, 0);
            if predator != 0 {
                call_slot_2(self.0, slots::GOAL_SET, GOAL_PREDATOR, 3);
                let arg = (base() + globals_rva::COG_GOAL_ARG) as *const i32;
                unsafe { COG_GOAL_SET_ID.original()(cog_goal as *const u32, predator, arg) };
                save_to_memory(self.0 + unit_offsets::PREDATOR, predator);
            }
            save_to_memory(self.0 + unit_offsets::PREDATOR_CHECK_PENDING, 0u8);
            return self.finish_update_goals();
        }
        let cog_valid = call_slot_0(cog_goal, slots::COG_VALID) & 0xff != 0;
        if !cog_valid || get_from_memory::<u8>(self.0 + unit_offsets::COG_GOAL_FLAG) != 0 {
            call_slot_2(self.0, slots::GOAL_SET, GOAL_FALLBACK, 4);
        }
        self.finish_update_goals();
    }

    /// `ZTMaint::initStatusVars` (`0x004c6056`): the `ZTStaff` reset; then, when the unit has a type, `+0x2a0` is
    /// cleared, `+0x64` links to the unit itself, `+0x68` to the type's `+0x80` block (null when the type's `+0x84`
    /// is clear), and the timer reloads from the type.
    pub fn init_status_vars(self) {
        ZTStaff(self.0).init_status_vars();
        let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            return;
        }
        save_to_memory(self.0 + FIELD_0X2A0_OFFSET, 0u32);
        save_to_memory(self.0 + unit_offsets::SELF_POINTER, self.0);
        let block = if get_from_memory::<u32>(entity_type + type_offsets::BLOCK_GATE) == 0 { 0 } else { entity_type + type_offsets::BLOCK };
        save_to_memory(self.0 + unit_offsets::TYPE_BLOCK_POINTER, block);
        self.set_timer(get_from_memory(entity_type + type_offsets::TIMER_RELOAD));
    }

    /// `ZTMaint::isTaskAvailable` (`0x004a9742`): whether the worker's type allows task kind `index`. A missing type
    /// or an index past the type's bit vector is `false`.
    pub fn is_task_available(self, index: u32) -> bool {
        let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            return false;
        }
        let bits = BitVec::read(
            entity_type + type_offsets::TASK_BITS_BEGIN,
            entity_type + type_offsets::TASK_BITS_BEGIN_BIT,
            entity_type + type_offsets::TASK_BITS_END,
            entity_type + type_offsets::TASK_BITS_END_BIT,
        );
        index < bits.len() && bits.get(index)
    }

    /// `ZTMaint::setNewTask` (`0x004a97c0`): replaces the worker's current task. With no current task the new one is
    /// stored. Otherwise every pool list node that holds the old task is unlinked and pushed on vanilla's 12-byte
    /// freelist, the old task is freed with `operator_delete`, and the new one is stored.
    pub fn set_new_task(self, task: u32) {
        let old = self.current_task();
        if old == 0 {
            return self.set_current_task(task);
        }
        let sentinel: u32 = get_from_memory(base() + globals_rva::TASK_LIST);
        vanilla_list::remove_matching(sentinel, old);
        unsafe { OPERATOR_DELETE.original()(old) };
        self.set_current_task(task);
    }

    /// `ZTMaint::finishTask` (`0x004a97e5`): `setNewTask(0)`.
    pub fn finish_task(self) {
        self.set_new_task(0);
    }

    /// `ZTMaint::isTargeted` (`0x004a2768`): whether another maintenance worker in the world is already heading for
    /// `tile` (its `+0x234` target tile).
    pub fn is_targeted(self, tile: u32) -> bool {
        let sentinel: u32 = get_from_memory(base() + globals_rva::ENTITY_LIST);
        let mut node: u32 = get_from_memory(sentinel);
        while node != sentinel {
            let entity: u32 = get_from_memory(node + 8);
            if entity != 0
                && unsafe { entity_type_matches(entity, MAINT_TYPE_TAG_RVA) }
                && entity != self.0
                && get_from_memory::<u32>(entity + unit_offsets::TARGET_TILE) == tile
            {
                return true;
            }
            node = get_from_memory(node);
        }
        false
    }

    /// `ZTMaint::removeFromMap` (`0x00502078`): a worker removed for good finishes its task; one only being moved
    /// detaches its task node from itself without freeing it. Then the `ZTStaff` removal.
    pub fn remove_from_map(self, param: bool) {
        if param {
            self.finish_task();
        } else {
            let task = self.current_task();
            if task != 0 {
                save_to_memory(task + task_offsets::WORKER, 0u32);
                self.set_current_task(0);
            }
        }
        ZTStaff(self.0).remove_from_map(param);
    }

    /// `ZTMaint::save` (`0x004794be`): `+0x2a0`, `+0x298`, `+0x294`, the 16-bit task count, the bit count, one byte
    /// per task bit, then the `ZTStaff` data. Every write and the `ZTStaff` save run even after a failure; the
    /// result is their conjunction.
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = write_field(self.0 + FIELD_0X2A0_OFFSET, 4, file);
        ok &= write_field(self.0 + FIELD_0X298_OFFSET, 4, file);
        ok &= write_field(self.0 + FIELD_0X294_OFFSET, 4, file);
        ok &= write_field(self.0 + AVAILABLE_TASK_COUNT_OFFSET, 2, file);
        let bits = self.task_bits();
        let count = bits.len();
        ok &= write_field(&count as *const u32 as u32, 4, file);
        for index in 0..count {
            let byte = bits.get(index) as u32;
            ok &= write_field(&byte as *const u32 as u32, 1, file);
        }
        let staff_ok = ZTStaff(self.0).save(file);
        staff_ok && ok
    }

    /// `ZTMaint::load` (`0x004c60bc`): mirrors `save`, gated by the save `version`. Above `0x22` `+0x2a0` is read;
    /// above `0x5b` the rest follows, and each stored task bit is applied while it is inside the worker's own bit
    /// vector (stored bits beyond it are still consumed from the stream, but only up to the vector's length, as
    /// vanilla stops there). Any failed read returns `false` without running `ZTStaff::load`.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let mut ok = true;
        if version > save_version::FIELD_0X2A0_AFTER {
            ok = read_field(self.0 + FIELD_0X2A0_OFFSET, 4, file);
        }
        if version > save_version::TASK_STATE_AFTER {
            ok &= read_field(self.0 + FIELD_0X298_OFFSET, 4, file);
            ok &= read_field(self.0 + FIELD_0X294_OFFSET, 4, file);
            ok &= read_field(self.0 + AVAILABLE_TASK_COUNT_OFFSET, 2, file);
            let mut stored: u32 = 0;
            ok &= read_field(&mut stored as *mut u32 as u32, 4, file);
            if !ok {
                return false;
            }
            let bits = self.task_bits();
            let mut index = 0;
            while index < stored && index < bits.len() {
                let mut byte: u8 = 1;
                if !read_field(&mut byte as *mut u8 as u32, 1, file) {
                    return false;
                }
                bits.set(index, byte != 0);
                index += 1;
            }
        }
        if !ok {
            return false;
        }
        ZTStaff(self.0).load(file, version)
    }
}

/// `ZTMaint::findFenceToFixOnTile` (`0x004a1f99`): of the fences on `tile`'s four edges (read from the tile and from
/// the neighbour on the far side, so each shared edge is looked at from both sides), the one worth fixing with the
/// least wear. Returns the direction it was found in (`0xffffffff` when none) and stores the wear at `*out_wear`
/// (`0x7fffffff` when none).
pub fn find_fence_to_fix_on_tile(tile: u32, out_wear: u32) -> u32 {
    let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
    let map = world.wrapping_add(8);
    let mut best_direction: u32 = 0xffff_ffff;
    let mut best_wear: i32 = 0x7fff_ffff;
    let mut consider = |fence: u32, direction: u32| {
        if fence == 0 {
            return;
        }
        if !unsafe { entity_type_matches(fence, globals_rva::FENCE_TAG) } {
            return;
        }
        if unsafe { ZTFENCE_IS_WORTH_FIXING.original()(fence as *const u32) } & 0xff == 0 {
            return;
        }
        let wear: i32 = get_from_memory(fence + FENCE_WEAR_OFFSET);
        if wear < best_wear {
            best_wear = wear;
            best_direction = direction;
        }
    };
    for direction in [0u32, 2, 4, 6] {
        consider(get_from_memory(tile + TILE_EDGE_SLOTS_OFFSET + (direction / 2) * 4), direction);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.original()(map as *const u32, tile as *const u32, direction) };
        let far_edge = direction.wrapping_sub(4) & 7;
        // Vanilla reads the neighbour's edge slot unchecked, so repairing a fence on a map-edge tile is a known vanilla
        // crash (see `plans/vanilla-quirks.md`); the port treats the missing neighbour as no fence.
        if neighbour != 0 {
            consider(get_from_memory(neighbour + TILE_EDGE_SLOTS_OFFSET + (far_edge / 2) * 4), direction);
        }
    }
    save_to_memory(out_wear, best_wear);
    best_direction
}

/// `ZTMaintTaskPool::getEvents` (`0x00434e8b`): forwards to the pool's own three-argument `getEvents` (vtable
/// `+0xc`) with `(out, 0, 0x4d)`.
fn pool_get_events(pool: u32, out: u32) {
    call_slot_3(pool, slots::POOL_GET_EVENTS, out, 0, EVENT_KIND_MAINT);
}

/// Event-record layout `listen` scans: `0x1c` bytes each, the target entity id at `+0x10` and the message code (a
/// `u16`) at `+0x16`.
const EVENT_RECORD_BYTES: u32 = 0x1c;
const EVENT_TARGET_OFFSET: u32 = 0x10;
const EVENT_CODE_OFFSET: u32 = 0x16;
/// A task node is `{kind, target entity id, worker, flag byte}`.
const TASK_NODE_BYTES: u32 = 0x10;
const TASK_FLAG_OFFSET: u32 = 0xc;

/// `ZTMaintTaskPool::listen` (`0x00434e30`): asks the pool for its pending events (vtable `+0x8`) and turns the task
/// messages `0x272e..=0x2734` into task nodes queued on the pool's list (`pool + 0x4`). A message sets the current
/// kind (`0x272e` 9, `0x272f` 7, `0x2730` 0xb, `0x2731` and `0x2732` 0x10, `0x2733` and `0x2734` 0x1a); `0x2732` and
/// `0x2734` queue at the front, the others at the back. Any other message queues another back task of the current
/// kind once one has been set. The event vector is then freed (the small-block freelist up to `0x80` bytes,
/// `operator_delete` above). The decompile's "call a helper and return" for `0x272e` is Ghidra's split of the shared
/// create-node block: that case creates a kind 9 task and carries on.
fn pool_listen(pool: u32) {
    let mut events = [0u32; 3];
    call_slot_1(pool, slots::POOL_EVENTS_OUT, events.as_mut_ptr() as u32);
    let (begin, end, capacity) = (events[0], events[1], events[2]);
    let list = get_from_memory::<u32>(pool + 4);
    let mut kind = 0u32;
    if begin != end {
        let mut record = begin;
        loop {
            let target: u32 = get_from_memory(record + EVENT_TARGET_OFFSET);
            let mut front = false;
            let mut queue = true;
            match get_from_memory::<u16>(record + EVENT_CODE_OFFSET) {
                0x272e => kind = 9,
                0x272f => kind = 7,
                0x2730 => kind = 0xb,
                0x2731 => kind = 0x10,
                0x2732 => {
                    kind = 0x10;
                    front = true;
                }
                0x2733 => kind = 0x1a,
                0x2734 => {
                    kind = 0x1a;
                    front = true;
                }
                _ => queue = kind != 0,
            }
            if queue {
                let task = unsafe { OPERATOR_NEW.original()(TASK_NODE_BYTES) } as u32;
                if task != 0 {
                    save_to_memory(task, kind);
                    save_to_memory(task + 4, target);
                    save_to_memory(task + 8, 0u32);
                    save_to_memory(task + TASK_FLAG_OFFSET, front as u8);
                    if front {
                        vanilla_list::push_front(list, task);
                    } else {
                        vanilla_list::push_back(list, task);
                    }
                }
            }
            record = record.wrapping_add(EVENT_RECORD_BYTES);
            if record == end {
                break;
            }
        }
    }
    if begin != 0 {
        let bytes = (capacity.wrapping_sub(begin) / EVENT_RECORD_BYTES) * EVENT_RECORD_BYTES;
        if bytes > 0x80 {
            unsafe { OPERATOR_DELETE.original()(begin) };
        } else if bytes != 0 {
            let bucket = get_module_base("zoo.exe") as u32 + 0x0023_8000 + ((bytes - 1) >> 3) * 4;
            save_to_memory(begin, get_from_memory::<u32>(bucket));
            save_to_memory(bucket, begin);
        }
    }
}

/// `ZTMaintTaskPool::clear` (`0x005b11a2`): releases every queued task. A task assigned to a worker lets that worker
/// go of it, the task is freed with `operator_delete`, then every list node goes back on vanilla's 12-byte freelist
/// and the list is emptied.
fn pool_clear(pool: u32) {
    let sentinel: u32 = get_from_memory(pool + 4);
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let task: u32 = get_from_memory(node + 8);
        let worker: u32 = get_from_memory(task + task_offsets::WORKER);
        if worker != 0 && get_from_memory::<u32>(worker + CURRENT_TASK_OFFSET) != 0 {
            let current: u32 = get_from_memory(worker + CURRENT_TASK_OFFSET);
            save_to_memory(current + task_offsets::WORKER, 0u32);
            save_to_memory(worker + CURRENT_TASK_OFFSET, 0u32);
        }
        unsafe { OPERATOR_DELETE.original()(task) };
        node = get_from_memory(node);
    }
    vanilla_list::clear(sentinel);
}

#[detour_mod]
mod maint_detours {
    use super::*;

    /// The body `ZTGuest` and `ZTMaint` share (`0x0042201e`).
    #[detour(F_ROUTINE)]
    unsafe extern "thiscall" fn f_routine(this: *const u32, done: *const bool) -> bool {
        ZTMaint::f_routine(this as u32, done)
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const i32, elapsed: u32) {
        ZTMaint(this as u32).update(elapsed);
    }

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const i32) {
        ZTMaint(this as u32).update_goals();
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const i32) {
        ZTMaint(this as u32).init_status_vars();
    }

    #[detour(IS_TASK_AVAILABLE)]
    unsafe extern "thiscall" fn is_task_available(this: *const i32, index: u32) -> u32 {
        ZTMaint(this as u32).is_task_available(index) as u32
    }

    #[detour(SET_NEW_TASK)]
    unsafe extern "thiscall" fn set_new_task(this: *const i32, task: u32) {
        ZTMaint(this as u32).set_new_task(task);
    }

    #[detour(FINISH_TASK)]
    unsafe extern "fastcall" fn finish_task(this: *const c_void) {
        ZTMaint(this as u32).finish_task();
    }

    #[detour(IS_TARGETED)]
    unsafe extern "thiscall" fn is_targeted(this: *const i32, tile: i32) -> u32 {
        ZTMaint(this as u32).is_targeted(tile as u32) as u32
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const i32, param: bool) {
        ZTMaint(this as u32).remove_from_map(param);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const i32, file: *const u32) -> bool {
        ZTMaint(this as u32).save(file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const i32, file: *const u32, version: u32) -> bool {
        ZTMaint(this as u32).load(file, version)
    }

    #[detour(FIND_FENCE_TO_FIX_ON_TILE)]
    unsafe extern "stdcall" fn find_fence_to_fix_on_tile(tile: *const u32, out_wear: *const i32) -> u32 {
        super::find_fence_to_fix_on_tile(tile as u32, out_wear as u32)
    }

    #[detour(GET_EVENTS)]
    unsafe extern "thiscall" fn pool_get_events(this: *const i32, out: u32) {
        super::pool_get_events(this as u32, out);
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn pool_listen(this: *const i32) {
        super::pool_listen(this as u32);
    }

    #[detour(CLEAR)]
    unsafe extern "thiscall" fn pool_clear(this: *const c_void) {
        super::pool_clear(this as u32);
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn f_routine(this: u32, done: *const bool) -> bool {
            unsafe { super::F_ROUTINE_DETOUR.call(this as *const u32, done) }
        }

        pub(crate) fn update(this: u32, elapsed: u32) {
            unsafe { super::UPDATE_DETOUR.call(this as *const i32, elapsed) }
        }

        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const i32) }
        }

        pub(crate) fn init_status_vars(this: u32) {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const i32) }
        }

        pub(crate) fn is_task_available(this: u32, index: u32) -> u32 {
            unsafe { super::IS_TASK_AVAILABLE_DETOUR.call(this as *const i32, index) }
        }

        pub(crate) fn set_new_task(this: u32, task: u32) {
            unsafe { super::SET_NEW_TASK_DETOUR.call(this as *const i32, task) }
        }

        pub(crate) fn finish_task(this: u32) {
            unsafe { super::FINISH_TASK_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn is_targeted(this: u32, tile: u32) -> u32 {
            unsafe { super::IS_TARGETED_DETOUR.call(this as *const i32, tile as i32) }
        }

        pub(crate) fn remove_from_map(this: u32, param: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const i32, param) }
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const i32, file) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const i32, file, version) }
        }

        pub(crate) fn find_fence_to_fix_on_tile(tile: u32, out_wear: u32) -> u32 {
            unsafe { super::FIND_FENCE_TO_FIX_ON_TILE_DETOUR.call(tile as *const u32, out_wear as *const i32) }
        }

        pub(crate) fn pool_get_events(pool: u32, out: u32) {
            unsafe { super::GET_EVENTS_DETOUR.call(pool as *const i32, out) }
        }

        pub(crate) fn pool_listen(pool: u32) {
            unsafe { super::LISTEN_DETOUR.call(pool as *const i32) }
        }

        pub(crate) fn pool_clear(pool: u32) {
            unsafe { super::CLEAR_DETOUR.call(pool as *const c_void) }
        }
    }
}

/// Installs `ZTMaint`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTMAINT_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { maint_detours::init_detours() } {
        error!("Failed to initialise ztmaint detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::{standalone::OPERATOR_NEW, ztmaint::CONSTRUCTOR, ztmaint::DESTRUCTOR};

    use super::{ZTMaint, MAINT_SIZE, MAINT_TYPE_TAG_RVA};
    use crate::{
        globals::{get_module_base, globals},
        util::get_from_memory,
    };

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::maint_detours::status()
    }

    /// The first `ZTMaintType` the live world holds, if any.
    pub fn find_maint_type() -> Option<u32> {
        let world = globals().ztworldmgr();
        let tag = get_module_base("zoo.exe") as u32 + MAINT_TYPE_TAG_RVA;
        let mut slot = world.entity_type_array_start;
        while slot != world.entity_type_array_end {
            let ty: u32 = get_from_memory(slot);
            slot += 4;
            if ty == 0 {
                continue;
            }
            let vtable: u32 = get_from_memory(ty);
            let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
            if is_cast_class(ty, tag) {
                return Some(ty);
            }
        }
        None
    }

    /// Zeroed worker-sized memory from vanilla's allocator, unconstructed.
    pub fn allocate_zeroed_maint() -> ZTMaint {
        let memory = unsafe { OPERATOR_NEW.original()(MAINT_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, MAINT_SIZE as usize) };
        ZTMaint(memory)
    }

    /// A zeroed worker built by the real vanilla constructor over `maint_type`. The constructor is not detoured.
    pub fn build_standalone_maint(maint_type: u32) -> ZTMaint {
        let worker = allocate_zeroed_maint();
        unsafe { CONSTRUCTOR.original()(worker.0 as *const i32, maint_type as *const i32) };
        worker
    }

    /// Tears a worker down through the real scalar-deleting destructor (also not detoured).
    pub fn destroy_standalone_maint(worker: ZTMaint) {
        unsafe { DESTRUCTOR.original()(worker.0 as *const i32, 1) };
    }

    pub(crate) use super::maint_detours::test_real as real;
}
