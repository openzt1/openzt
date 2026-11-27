//! `ZTGuest`'s per-tick pipeline: `doConditionals`, the need and environment checks it runs, the status-variable
//! update and the leave-zoo predicates. Reads and writes guest memory in place through [`ZTGuest`].
//!
//! Every check keeps its countdown in a slot of the guest (`+0x2e8..+0x310`) and, when the countdown reaches
//! zero, reloads it from the guest type. A set force byte (`+0x33e`) skips the countdown so the check body
//! runs at once. Debug strings vanilla builds and discards (`nullsub`) are not built here.

use std::ffi::c_void;

use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfunit::GO_TO_DEST_0 as BFUNIT_GO_TO_DEST,
    msvc_std_basic_string::BASIC_STRING_RANGE,
    poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
    ztanimal::PREYS_ON_MAN,
    ztguest::{
        DO_AMBIENT_ANIMS, DO_BATHROOM_CHECK, DO_BUY_SOUVENIR_CHECK, DO_CHASE_CHECK, DO_CONDITIONALS, DO_ENERGY_CHECK, DO_ENVIRONMENT_EFFECT_CHECK, DO_HUNGER_CHECK, DO_LEAVE_ZOO_CHECK,
        DO_THIRSTY_CHECK, DO_TRASH_CHECK, DO_VIEWING_AREA_CHECK, FIND_ANIMAL_BY_HAPPINESS, F_CROWD_DENSITY, F_DIFFERENT_SPECIES, F_NUM_ANIMALS, F_NUM_SICK_ANIMALS,
        F_SICK_ANIMALS, FIND_PLACE_TO_SIT, FIND_TRASH_CAN, F_CROWD_DENSITY_MEGATILE, F_ESTHETIC_BONUS_MEGATILE,
        F_GUEST_THOUGHT, F_STINKY_MEGATILE, PICK_RANDOM_BUILDING_OF_TYPE, TRIGGER_GRANDSTAND_ESCAPE_1, CHECK_LEAVE_ZOO, CHECK_PREDATOR, LEAVE_OK,
        UPDATE_STATUS_VARIABLES,
    },
    zthabitat::GET_ALL_ANIMALS,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{
    guest_offsets, lifecycle_offsets, status_type_offsets, type_offsets, unit_fields, ZTGuest, AIMGR_RESET_INTERVAL_OFFSET,
};
use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, position_in_tile_xy, position_in_tile_z},
    zthabitat::{
        next_game_random,
        viewing_area::{recalculate_characteristics as recalculate_viewing_area, recalculate_observable_area},
    },
    ztkeeper::{call_slot_0, call_slot_1, unit_offsets},
    ztmegatilemgr::entity_type_matches,
};

/// `ZTGuestType` fields and guest slots `doViewingAreaCheck` uses.
mod viewing_check {
    pub const PERIOD: u32 = 0x1d8;
    pub const VARIETY_BONUS: u32 = 0x274;
    pub const VARIETY_MIN: u32 = 0x278;
    pub const SICK_BONUS: u32 = 0x27c;
    pub const CROWD_MIN: u32 = 0x280;
    pub const CROWD_BONUS: u32 = 0x284;
    pub const OVERJOYED: u32 = 0x28c;
    pub const HAPPY: u32 = 0x290;
    pub const PLEASED: u32 = 0x294;
    pub const ANGRY: u32 = 0x298;
    pub const FURIOUS: u32 = 0x29c;
    /// Animal the guest is looking at, remembered for the viewing thoughts.
    pub const TARGET: u32 = 0x330;
}

/// `ZTViewingArea` fields the statistics queries read; the embedded observable area starts at `+0x4`.
mod viewing_area_fields {
    pub const HABITAT: u32 = 0x0;
    pub const OBSERVABLE_AREA: u32 = 0x4;
    /// Observable-area species count (`OA+0xc`).
    pub const DIFFERENT_SPECIES: u32 = 0x10;
    /// Observable-area animal count (`OA+0x10`).
    pub const ANIMAL_COUNT: u32 = 0x14;
    /// Observable-area sick-animal count (`OA+0x18`).
    pub const SICK_COUNT: u32 = 0x1c;
    /// Observable area's characteristics-dirty byte (`OA+0x20`).
    pub const OA_DIRTY: u32 = 0x24;
    pub const CHARACTERISTICS_DIRTY: u32 = 0x4c;
    /// Vector (begin, end) whose length `fCrowdDensity` divides by.
    pub const CROWD_VECTOR_BEGIN: u32 = 0x40;
    pub const CROWD_VECTOR_END: u32 = 0x44;
    /// Count `fCrowdDensity` is a ratio of.
    pub const CROWD_COUNT: u32 = 0x54;
}

/// `DAT_00630d5c`: the vector length for which `fCrowdDensity` defers to its fallback path.
const CROWD_GUARD_RVA: u32 = 0x0023_0d5c;
/// `DAT_00630d60`: `fCrowdDensity`'s scale factor.
const CROWD_SCALE_RVA: u32 = 0x0023_0d60;

/// An animal's happiness, which `findAnimalByHappiness` ranges over.
const ANIMAL_HAPPINESS: u32 = 0x2a8;
/// Byte set on a guest that `doAmbientAnims` turns into the `+0x339` flag.
const AMBIENT_TRIGGER_FLAG: u32 = 0x33b;

/// `ZTGuest` fields the checks use beyond the layout table.
mod fields {
    /// Pending happiness change the status update folds into `+0x2a8`.
    pub const HAPPY_DELTA: u32 = 0x2ac;
    pub const HUNGER_DELTA: u32 = 0x2b4;
    pub const THIRST_DELTA: u32 = 0x2bc;
    pub const ENERGY_DELTA: u32 = 0x2c4;
    pub const BATHROOM_DELTA: u32 = 0x2cc;
    /// Per-need rate words.
    pub const HUNGER_RATE: u32 = 0x2d4;
    pub const THIRST_RATE: u32 = 0x2d8;
    pub const ENERGY_RATE: u32 = 0x2dc;
    pub const BATHROOM_RATE: u32 = 0x2e0;
    /// Check countdown slots.
    pub const HUNGER_COUNTDOWN: u32 = 0x2e8;
    pub const THIRST_COUNTDOWN: u32 = 0x2ec;
    pub const BATHROOM_COUNTDOWN: u32 = 0x2f0;
    pub const LEAVE_COUNTDOWN: u32 = 0x2f4;
    pub const SOUVENIR_COUNTDOWN: u32 = 0x2f8;
    pub const ENERGY_COUNTDOWN: u32 = 0x2fc;
    pub const CHASE_COUNTDOWN: u32 = 0x300;
    pub const TRASH_COUNTDOWN: u32 = 0x304;
    pub const VIEWING_COUNTDOWN: u32 = 0x30c;
    pub const ENVIRONMENT_COUNTDOWN: u32 = 0x310;
    /// Countdown to the next `initStatusVars` reset.
    pub const RESET_COUNTDOWN: u32 = 0x318;
    /// Byte set once the guest has decided to leave the zoo.
    pub const LEAVING: u32 = 0x339;
    /// Byte set once the guest has seen a predator.
    pub const PREDATOR_SEEN: u32 = 0x33a;
    /// Byte set when the guest wants to buy a souvenir.
    pub const WANTS_SOUVENIR: u32 = 0x33c;
    /// Byte that stops the need checks from searching again.
    pub const SEARCH_LOCK: u32 = 0x33f;
    /// Byte the check that raises "seeing a smell" keeps so it fires once.
    pub const SMELL_FLAG: u32 = 0x336;
    /// `ZTUnit` bytes.
    pub const TARGET_FLAG: u32 = 0x20c;
    pub const FLEEING: u32 = 0x24c;
    pub const NEED_INDEX: u32 = 0x24e;
}

/// Vtable slots the checks dispatch.
mod check_slots {
    /// `initStatusVars`.
    pub const INIT_STATUS_VARS: u32 = 0x10c;
    /// The unit's current target entity, or `0`.
    pub const TARGET: u32 = 0x24c;
    /// Predicate that suspends the need checks while it holds.
    pub const BUSY: u32 = 0x268;
    /// Takes `0`; returns the id of a predator in sight, or `0`.
    pub const PREDATOR_IN_SIGHT: u32 = 0x2fc;
    /// Takes a tile: walks the guest towards it.
    pub const GO_TO_TILE: u32 = 0x2e4;
    /// Takes a flag pointer and `1`; returns an entity pointer or `0`.
    pub const TRASH_TARGET: u32 = 0x2e8;
}

/// `ZTGuestType` fields the checks use beyond [`type_offsets`].
mod check_type {
    pub const ENERGY_THRESHOLD: u32 = 0x200;
    pub const ENERGY_RATE: u32 = 0x210;
    pub const ENERGY_HAPPY: u32 = 0x218;
    pub const BATHROOM_THRESHOLD: u32 = 0x208;
    pub const BATHROOM_RATE: u32 = 0x238;
    pub const BATHROOM_HAPPY: u32 = 0x240;
    pub const HUNGER_THRESHOLD: u32 = 0x1f0;
    pub const HUNGER_RATE: u32 = 0x21c;
    pub const HUNGER_HAPPY: u32 = 0x224;
    pub const THIRST_THRESHOLD: u32 = 0x1f8;
    pub const THIRST_RATE: u32 = 0x22c;
    pub const THIRST_HAPPY: u32 = 0x234;
    pub const TRASH_HAPPY: u32 = 0x264;
    /// Chances (percent) a guest in a happiness band picks up a souvenir.
    pub const SOUVENIR_CHANCE_HIGH: u32 = 0x260;
    pub const SOUVENIR_CHANCE_LOW: u32 = 0x25c;
    /// Chances (percent) a guest in each unhappy band leaves.
    pub const LEAVE_CHANCE_A: u32 = 0x24c;
    pub const LEAVE_CHANCE_B: u32 = 0x250;
    pub const LEAVE_CHANCE_C: u32 = 0x254;
    pub const LEAVE_CHANCE_CHECK: u32 = 0x258;
    pub const PREDATOR_HAPPY: u32 = 0x2a0;
    pub const CROWDED_THRESHOLD: u32 = 0x280;
    pub const CROWDED_HAPPY: u32 = 0x284;
    pub const ESTHETIC_THRESHOLD: u32 = 0x2a4;
    pub const ESTHETIC_HAPPY: u32 = 0x2a8;
    pub const STINK_THRESHOLD: u32 = 0x2b0;
    pub const STINK_HAPPY: u32 = 0x2b8;
}

/// Need names the checks resolve with `ZTWorldMgr::getNeedIndex`, as RVAs from the `zoo.exe` base.
pub(super) mod need_names {
    pub const THIRST: u32 = 0x0023_c4c8;
    pub const BATHROOM: u32 = 0x0023_c4d8;
    pub const HUNGER_A: u32 = 0x0023_c4e8;
    pub const HUNGER_B: u32 = 0x0023_c4f8;
    pub const TRASH: u32 = 0x0023_c508;
    pub const ENERGY: u32 = 0x0023_c518;
    pub const SOUVENIR: u32 = 0x0023_c528;
}

/// The goal the guest is in when the checks stand down.
const GOAL_STANDING_DOWN: u32 = 0x71;
const GOAL_GRANDSTAND: u32 = 0x75;
const GOAL_GRANDSTAND_ESCAPE: u32 = 0x7f;
/// `ZTAnimal`'s class tag (`CAST_ZTAnimal`, `0x00638690`) as an RVA.
const ANIMAL_TYPE_TAG_RVA: u32 = 0x0023_8690;
/// `DAT_00638fb0`: the global holding the sentinel of the list `doConditionals` tests for emptiness.
const ESCAPE_LIST_RVA: u32 = 0x0023_8fb0;
/// Argument `findNearestSatisfier` takes as its search radius.
const SATISFIER_SEARCH_RANGE: i32 = 0xb504;
/// `BFWorldMgr::getEntity(id, flag)`.
const WORLD_GET_ENTITY: u32 = 0x24;
/// `GLOBAL_ZTWorldMgr`.
const WORLD_MGR_RVA: u32 = 0x0023_8040;
/// Byte at a target entity `+0x2c` that the chase check tests.
const TARGET_ACTIVE_OFFSET: u32 = 0x2c;
/// Word at an entity `+0x19c` that `doTrashCheck` compares with `0x18`.
const TRASH_TARGET_RANGE_OFFSET: u32 = 0x19c;
const TRASH_TARGET_RANGE: i32 = 0x18;
/// Offsets a building contributes to the guest's position record.
const BUILDING_POSITION_X: u32 = 0x114;
const BUILDING_POSITION_Y: u32 = 0x118;
const BUILDING_POSITION_Z: u32 = 0x11c;

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// How a need check treats one of the hunger/thirst/bathroom needs.
struct NeedSpec {
    countdown: u32,
    period: u32,
    /// Need names; the happiness and rate steps run only when the guest's need index (`+0x24e`) equals none of them.
    names: &'static [u32],
    rate: u32,
    delta: u32,
    rate_type: u32,
    level: u32,
    level_type: u32,
    happy_type: u32,
    pressing_type: u32,
    /// Carried-item slot whose being filled ends the check.
    carried_item: Option<u32>,
    /// Thought when the guest starts searching, and when no building of the need's type exists at all.
    thought: u32,
    missing_thought: u32,
    /// The name searched for.
    search_name: u32,
}

const HUNGER: NeedSpec = NeedSpec {
    countdown: fields::HUNGER_COUNTDOWN,
    period: status_type_offsets::SLOT_PERIODS[0],
    names: &[need_names::HUNGER_A, need_names::HUNGER_B],
    rate: fields::HUNGER_RATE,
    delta: fields::HUNGER_DELTA,
    rate_type: check_type::HUNGER_RATE,
    level: guest_offsets::HUNGER,
    level_type: check_type::HUNGER_THRESHOLD,
    happy_type: check_type::HUNGER_HAPPY,
    pressing_type: type_offsets::HUNGER_PRESSING,
    carried_item: Some(0),
    thought: 0x275f,
    missing_thought: 0x2761,
    search_name: need_names::HUNGER_A,
};

const THIRST: NeedSpec = NeedSpec {
    countdown: fields::THIRST_COUNTDOWN,
    period: status_type_offsets::SLOT_PERIODS[1],
    names: &[need_names::THIRST],
    rate: fields::THIRST_RATE,
    delta: fields::THIRST_DELTA,
    rate_type: check_type::THIRST_RATE,
    level: guest_offsets::THIRST,
    level_type: check_type::THIRST_THRESHOLD,
    happy_type: check_type::THIRST_HAPPY,
    pressing_type: type_offsets::THIRST_PRESSING,
    carried_item: Some(1),
    thought: 0x2762,
    missing_thought: 0x2763,
    search_name: need_names::THIRST,
};

const BATHROOM: NeedSpec = NeedSpec {
    countdown: fields::BATHROOM_COUNTDOWN,
    period: status_type_offsets::SLOT_PERIODS[2],
    names: &[need_names::BATHROOM],
    rate: fields::BATHROOM_RATE,
    delta: fields::BATHROOM_DELTA,
    rate_type: check_type::BATHROOM_RATE,
    level: guest_offsets::BATHROOM,
    level_type: check_type::BATHROOM_THRESHOLD,
    happy_type: check_type::BATHROOM_HAPPY,
    pressing_type: type_offsets::BATHROOM_PRESSING,
    carried_item: None,
    thought: 0x2764,
    missing_thought: 0x2766,
    search_name: need_names::BATHROOM,
};

impl ZTGuest {
    fn type_word(self, guest_type: u32, offset: u32) -> i32 {
        get_from_memory(guest_type + offset)
    }

    fn add_happiness_delta(self, amount: i32) {
        let delta: i32 = self.read(fields::HAPPY_DELTA);
        self.write(fields::HAPPY_DELTA, delta.wrapping_add(amount));
    }

    pub(super) fn is_on_water(self) -> bool {
        crate::ztunit::ZTUnit(self.0).is_on_water()
    }

    fn goal_id(self) -> u32 {
        self.read(unit_offsets::GOAL_ID)
    }

    /// The need checks stand down on water, in goal `0x71` and while the busy predicate (`+0x268`) holds.
    fn checks_active(self) -> bool {
        !self.is_on_water() && self.goal_id() != GOAL_STANDING_DOWN && !low_byte_bool(call_slot_0(self.0, check_slots::BUSY))
    }

    pub(super) fn thought(self, id: u32, subject: u32) {
        unsafe { F_GUEST_THOUGHT.hooked()(self.0 as *const u32, id, subject as *const i32, std::ptr::null()) };
    }

    fn need_index_of(name: u32) -> u8 {
        Self::need_index(name)
    }

    fn world_entity(id: u32) -> u32 {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let slot = get_from_memory::<u32>(get_from_memory::<u32>(world) + WORLD_GET_ENTITY);
        let get_entity = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(slot) };
        get_entity(world, id, 1)
    }

    pub(super) fn position_record(self) -> [u32; 3] {
        [
            self.read(lifecycle_offsets::POSITION_RECORD),
            self.read(lifecycle_offsets::POSITION_RECORD + 4),
            self.read(lifecycle_offsets::POSITION_RECORD + 8),
        ]
    }

    /// `ZTUnit::isNeedOrFunSatisfier` on the building grid with a copy of the guest's position record.
    pub(super) fn standing_on_satisfier(self) -> bool {
        let record = self.position_record();
        let grid: i32 = self.read(lifecycle_offsets::BUILDING_GRID);
        crate::ztunit::ZTUnit(self.0).is_need_or_fun_satisfier(grid as u32, record.as_ptr() as *const i32)
    }

    fn carried_item(self, slot: u32) -> u32 {
        let begin: u32 = self.read(guest_offsets::ITEMS);
        if begin == 0 {
            return 0;
        }
        get_from_memory(begin + slot * 4)
    }

    fn force_check(self) -> bool {
        self.read::<u8>(guest_offsets::RANDOM_DONE) != 0
    }

    /// Advances a check's countdown. `None` when the check is not due; otherwise whether the countdown itself
    /// expired (and was reloaded from the type period), as opposed to the force byte being set.
    fn countdown_due(self, slot: u32, guest_type: u32, period: u32) -> Option<bool> {
        if self.force_check() {
            return Some(false);
        }
        let old: i32 = self.read(slot);
        self.write(slot, old.wrapping_sub(1));
        if old != 0 {
            return None;
        }
        self.write(slot, self.type_word(guest_type, period).wrapping_sub(1));
        Some(true)
    }

    /// Writes the building's position into the guest's position record: the first two words modulo 64 and the third
    /// modulo 16, as vanilla computes them.
    pub(super) fn take_building_position(self, building: u32) {
        let x: i32 = get_from_memory(building + BUILDING_POSITION_X);
        let y: i32 = get_from_memory(building + BUILDING_POSITION_Y);
        let z: i32 = get_from_memory(building + BUILDING_POSITION_Z);
        self.write(lifecycle_offsets::POSITION_RECORD, position_in_tile_xy(x));
        self.write(lifecycle_offsets::POSITION_RECORD + 4, position_in_tile_xy(y));
        self.write(lifecycle_offsets::POSITION_RECORD + 8, position_in_tile_z(z));
    }

    /// Searches for the nearest building of `name` and walks towards it. With none in range, a random building of the
    /// type gets `0x2803` and no building at all `missing_thought`.
    fn search_for_building(self, name: u32, missing_thought: u32) {
        let name_address = (base() + name) as *const i32;
        let mut found: u32 = 0;
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) };
        let reached = crate::ztunit::ZTUnit(self.0).find_nearest_satisfier(tile as u32, name_address as u32, &mut found as *mut u32 as u32, SATISFIER_SEARCH_RANGE, -1);
        if reached {
            if found == 0 {
                return;
            }
            let index = Self::need_index_of(name);
            if index != 0xff {
                self.write(fields::NEED_INDEX, index);
            }
            let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(found as *const u32) } as u32;
            call_slot_1(self.0, check_slots::GO_TO_TILE, building_tile);
            self.take_building_position(found);
        } else {
            let mut random: i32 = 0;
            unsafe { PICK_RANDOM_BUILDING_OF_TYPE.hooked()(name_address, &mut random as *mut i32 as *const i32) };
            if random != 0 {
                self.thought(0x2803, random as u32);
            } else {
                self.thought(missing_thought, 0);
            }
        }
    }

    fn run_need_check(self, spec: &NeedSpec) {
        if !self.checks_active() {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        match self.countdown_due(spec.countdown, guest_type, spec.period) {
            None => return,
            Some(true) => {
                let current = self.read::<u8>(fields::NEED_INDEX);
                if spec.names.iter().all(|&name| current != Self::need_index_of(name)) {
                    let rate: i32 = self.read(spec.rate);
                    let delta: i32 = self.read(spec.delta);
                    self.write(spec.delta, delta.wrapping_add(rate.wrapping_mul(self.type_word(guest_type, spec.rate_type))));
                    if self.read::<i32>(spec.level) >= self.type_word(guest_type, spec.level_type) {
                        self.add_happiness_delta(self.type_word(guest_type, spec.happy_type));
                    }
                }
            }
            Some(false) => {}
        }
        if let Some(slot) = spec.carried_item
            && self.carried_item(slot) != 0
        {
            return;
        }
        if self.standing_on_satisfier() || self.read::<u8>(fields::SEARCH_LOCK) != 0 {
            return;
        }
        if self.read::<i32>(spec.level) < self.type_word(guest_type, spec.pressing_type) {
            return;
        }
        self.thought(spec.thought, 0);
        self.search_for_building(spec.search_name, spec.missing_thought);
    }

    /// `ZTGuest::doBathroomCheck` (`0x0043acd4`).
    pub fn do_bathroom_check(self) {
        self.run_need_check(&BATHROOM);
    }

    /// `ZTGuest::doHungerCheck` (`0x0043ad57`).
    pub fn do_hunger_check(self) {
        self.run_need_check(&HUNGER);
    }

    /// `ZTGuest::doThirstyCheck` (`0x0043adda`).
    pub fn do_thirsty_check(self) {
        self.run_need_check(&THIRST);
    }

    /// `ZTGuest::doEnergyCheck` (`0x0043ae5d`): like the other needs, but a tired guest looks for a seat
    /// (`findPlaceToSit(1)`) and the search lock does not apply.
    pub fn do_energy_check(self) {
        if !self.checks_active() {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        match self.countdown_due(fields::ENERGY_COUNTDOWN, guest_type, status_type_offsets::SLOT_PERIODS[5]) {
            None => return,
            Some(true) => {
                if self.read::<u8>(fields::NEED_INDEX) != Self::need_index_of(need_names::ENERGY) {
                    let rate: i32 = self.read(fields::ENERGY_RATE);
                    let delta: i32 = self.read(fields::ENERGY_DELTA);
                    self.write(fields::ENERGY_DELTA, delta.wrapping_add(rate.wrapping_mul(self.type_word(guest_type, check_type::ENERGY_RATE))));
                    if self.read::<i32>(guest_offsets::ENERGY) >= self.type_word(guest_type, check_type::ENERGY_THRESHOLD) {
                        self.add_happiness_delta(self.type_word(guest_type, check_type::ENERGY_HAPPY));
                    }
                }
            }
            Some(false) => {}
        }
        if self.standing_on_satisfier() {
            return;
        }
        if self.read::<i32>(guest_offsets::ENERGY) < self.type_word(guest_type, type_offsets::ENERGY_PRESSING) {
            return;
        }
        self.thought(0x276c, 0);
        unsafe { FIND_PLACE_TO_SIT.hooked()(self.0 as *const u32, std::ptr::without_provenance::<u32>(1)) };
    }
}

/// A virtual call on `this` taking two arguments and returning `EAX`.
fn call_slot_2_returning(this: u32, offset: u32, first: u32, second: u32) -> u32 {
    let slot = get_from_memory::<u32>(get_from_memory::<u32>(this) + offset);
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(slot) };
    function(this, first, second)
}

/// One draw from the shared RNG reduced to a percentage.
fn percent_roll() -> i32 {
    (next_game_random() % 100) as i32
}

impl ZTGuest {
    /// `ZTGuest::doTrashCheck` (`0x0043aedc`): a guest carrying trash (carried-item slot 4) that is not standing on a
    /// satisfier looks for a trash can; a can that exists but is out of reach (`+0x19c` below `0x18` on the entity the
    /// trash-target slot returns) ends the check, any other outcome falls back to `findTrashCan(0)`.
    pub fn do_trash_check(self) {
        if !self.checks_active() {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        match self.countdown_due(fields::TRASH_COUNTDOWN, guest_type, status_type_offsets::SLOT_PERIODS[7]) {
            None => return,
            Some(true) => {
                if self.read::<u8>(fields::NEED_INDEX) != Self::need_index_of(need_names::TRASH) {
                    self.add_happiness_delta(self.type_word(guest_type, check_type::TRASH_HAPPY));
                }
            }
            Some(false) => {}
        }
        if self.carried_item(4) == 0 || self.standing_on_satisfier() || self.read::<u8>(fields::SEARCH_LOCK) != 0 {
            return;
        }
        self.thought(0x27fd, 0);
        if unsafe { FIND_TRASH_CAN.hooked()(self.0 as *const u32, 1) } {
            let mut flag: u8 = 0;
            let target = call_slot_2_returning(self.0, check_slots::TRASH_TARGET, &mut flag as *mut u8 as u32, 1);
            if target != 0 && get_from_memory::<i32>(target + TRASH_TARGET_RANGE_OFFSET) < TRASH_TARGET_RANGE {
                return;
            }
        }
        unsafe { FIND_TRASH_CAN.hooked()(self.0 as *const u32, 0) };
    }

    /// `ZTGuest::doBuySouvenirCheck` (`0x0043af5b`): with the souvenir slot (item 3) empty, a guest not standing on a
    /// satisfier rolls for wanting a souvenir by happiness band (`0x47..=0x64` against the high chance, `0x1f..=0x46`
    /// against the low one; one draw in whichever band holds), and a guest that wants one searches for the shop.
    pub fn do_buy_souvenir_check(self) {
        if !self.checks_active() {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        if self.countdown_due(fields::SOUVENIR_COUNTDOWN, guest_type, status_type_offsets::SLOT_PERIODS[4]).is_none() {
            return;
        }
        if self.carried_item(3) != 0 || self.standing_on_satisfier() || self.read::<u8>(fields::SEARCH_LOCK) != 0 {
            return;
        }
        self.write(fields::WANTS_SOUVENIR, 0u8);
        let happiness: i32 = self.read(guest_offsets::HAPPINESS);
        if (0x47..=0x64).contains(&happiness) {
            if self.type_word(guest_type, check_type::SOUVENIR_CHANCE_HIGH) > percent_roll() {
                self.write(fields::WANTS_SOUVENIR, 1u8);
            }
        } else if (0x1f..=0x46).contains(&happiness) && self.type_word(guest_type, check_type::SOUVENIR_CHANCE_LOW) > percent_roll() {
            self.write(fields::WANTS_SOUVENIR, 1u8);
        }
        if self.read::<u8>(fields::WANTS_SOUVENIR) == 0 {
            return;
        }
        self.thought(0x276a, 0);
        self.search_for_building(need_names::SOUVENIR, 0x276b);
    }

    /// `ZTGuest::doLeaveZooCheck` (`0x0043ac4f`): when due (and after a force-triggered pass too) the countdown is
    /// reloaded; an unhappy guest (three happiness bands, each with its own chance) decides to leave (`+0x339`) and
    /// thinks `0x2767`/`0x2768`/`0x2769`. A guest in goal `0x71` only reloads the countdown.
    pub fn do_leave_zoo_check(self) {
        if !self.checks_active() {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        if !self.force_check() {
            let old: i32 = self.read(fields::LEAVE_COUNTDOWN);
            self.write(fields::LEAVE_COUNTDOWN, old.wrapping_sub(1));
            if old != 0 {
                return;
            }
        }
        self.write(fields::LEAVE_COUNTDOWN, self.type_word(guest_type, status_type_offsets::SLOT_PERIODS[3]).wrapping_sub(1));
        if self.goal_id() == GOAL_STANDING_DOWN {
            return;
        }
        let bands: [(i32, i32, u32, u32); 3] = [
            (-0x28, -0x14, check_type::LEAVE_CHANCE_A, 0x2767),
            (-0x50, -0x29, check_type::LEAVE_CHANCE_B, 0x2768),
            (-0x64, -0x51, check_type::LEAVE_CHANCE_C, 0x2769),
        ];
        for (low, high, chance, thought) in bands {
            let happiness: i32 = self.read(guest_offsets::HAPPINESS);
            if (low..=high).contains(&happiness) && self.type_word(guest_type, chance) > percent_roll() {
                self.write(fields::LEAVING, 1u8);
                self.thought(thought, 0);
                return;
            }
        }
    }

    /// `ZTGuest::doChaseCheck` (`0x0043abf9`): when due, a guest not in a grandstand goal asks the predator slot
    /// (`+0x2fc`); with one in sight it flees (`triggerGrandstandEscape`), takes the type's happiness penalty, marks
    /// `+0x33a`/`+0x24c`, clears `+0x339` and thinks `0x2805` (with the predator, when the target slot's entity is
    /// active) or `0x2808`, then reloads the general countdown (`+0x314`). With none in sight `+0x24c` clears.
    pub fn do_chase_check(self) {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        let goal = self.goal_id();
        let old: i32 = self.read(fields::CHASE_COUNTDOWN);
        self.write(fields::CHASE_COUNTDOWN, old.wrapping_sub(1));
        if old != 0 {
            return;
        }
        self.write(fields::CHASE_COUNTDOWN, self.type_word(guest_type, status_type_offsets::SLOT_PERIODS[6]).wrapping_sub(1));
        if goal == GOAL_GRANDSTAND || goal == GOAL_GRANDSTAND_ESCAPE {
            return;
        }
        let predator = call_slot_1(self.0, check_slots::PREDATOR_IN_SIGHT, 0);
        if predator == 0 {
            self.write(fields::FLEEING, 0u8);
            return;
        }
        unsafe { TRIGGER_GRANDSTAND_ESCAPE_1.hooked()(self.0 as *const u32, predator as i32) };
        self.write(fields::PREDATOR_SEEN, 1u8);
        self.write(fields::FLEEING, 1u8);
        self.write(fields::LEAVING, 0u8);
        self.add_happiness_delta(self.type_word(guest_type, check_type::PREDATOR_HAPPY));
        let entity = Self::world_entity(predator);
        let target = call_slot_0(self.0, check_slots::TARGET);
        if target != 0 && entity != 0 && get_from_memory::<u8>(target + TARGET_ACTIVE_OFFSET) != 0 {
            self.thought(0x2805, entity);
        } else {
            self.thought(0x2808, 0);
        }
        self.write(guest_offsets::CHECK_COUNTDOWN, self.type_word(guest_type, status_type_offsets::CHECK_PERIOD).wrapping_sub(1));
    }

    /// `ZTGuest::doEnvironmentEffectCheck` (`0x0043abac`): when due, a guest whose tile's esthetic bonus reaches
    /// the type's threshold gains happiness (`0x2777`), one in a crowded area gains the crowded value (`0x2778`),
    /// and the smell test sets `+0x336` once (`0x2801`) and clears it again when the smell fades.
    #[allow(clippy::neg_cmp_op_on_partial_ord)]
    pub fn do_environment_effect_check(self) {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        let old: i32 = self.read(fields::ENVIRONMENT_COUNTDOWN);
        self.write(fields::ENVIRONMENT_COUNTDOWN, old.wrapping_sub(1));
        if old != 0 {
            return;
        }
        self.write(fields::ENVIRONMENT_COUNTDOWN, self.type_word(guest_type, status_type_offsets::SLOT_PERIODS[10]).wrapping_sub(1));
        let esthetic = unsafe { F_ESTHETIC_BONUS_MEGATILE.hooked()(self.0 as *const u32) };
        if !(self.type_word(guest_type, check_type::ESTHETIC_THRESHOLD) as f64 > esthetic as f64) {
            self.add_happiness_delta(self.type_word(guest_type, check_type::ESTHETIC_HAPPY));
            self.thought(0x2777, 0);
        }
        let crowd = unsafe { F_CROWD_DENSITY_MEGATILE.hooked()(self.0 as *const u32) };
        if crowd >= self.type_word(guest_type, check_type::CROWDED_THRESHOLD) {
            self.add_happiness_delta(self.type_word(guest_type, check_type::CROWDED_HAPPY));
            self.thought(0x2778, 0);
        }
        let stink = unsafe { F_STINKY_MEGATILE.hooked()(self.0 as *const u32) };
        if !(self.type_word(guest_type, check_type::STINK_THRESHOLD) as f64 > stink as f64) {
            if self.read::<u8>(fields::SMELL_FLAG) == 0 {
                self.write(fields::SMELL_FLAG, 1u8);
                self.add_happiness_delta(self.type_word(guest_type, check_type::STINK_HAPPY));
                self.thought(0x2801, 0);
            }
        } else {
            self.write(fields::SMELL_FLAG, 0u8);
        }
    }

    /// Sends the guest walking (`BFUnit::goToDest` with the animation name `"bWalk"`), then frees the name string
    /// the way its destructor does.
    fn go_to_walk_dest(self) {
        const NAME: &[u8; 6] = b"bWalk\0";
        let mut name = [0u32; 3];
        let start = NAME.as_ptr();
        unsafe {
            BASIC_STRING_RANGE.hooked()(name.as_mut_ptr() as *const c_void, start as *const u32, start.add(NAME.len() - 1) as i32);
            BFUNIT_GO_TO_DEST.hooked()(self.0 as *const u32, name.as_ptr(), 0, 1);
            if name[0] != 0 {
                POOLALLOC_DEALLOCATE.original()(name[0] as *const u32, name[2].wrapping_sub(name[0]));
            }
        }
    }

    fn escape_list_is_empty() -> bool {
        let sentinel: u32 = get_from_memory(base() + ESCAPE_LIST_RVA);
        get_from_memory::<u32>(sentinel) == sentinel
    }

    /// `ZTGuest::doConditionals` (`0x0043ab00`): with the AI manager's reset interval set, counts `+0x318` down and
    /// re-runs `initStatusVars` (virtually) when it runs out; counts `+0x314` down and, when due, reloads it and lets a
    /// guest that saw a predator (`+0x33a`) settle: the flag clears and the guest walks on (`+0x20c` set), unless the
    /// predator slot still reports one (or an active target is held and the escape list is non-empty). Then the ten
    /// checks run, `+0x33e` is cleared and the viewing-area and environment checks follow.
    pub fn do_conditionals(self) {
        let guest_type = self.guest_type();
        let aimgr = globals().ztaimgr_ptr() as u32;
        if aimgr != 0 && get_from_memory::<u32>(aimgr + AIMGR_RESET_INTERVAL_OFFSET) != 0 {
            let remaining = self.read::<i32>(fields::RESET_COUNTDOWN).wrapping_sub(1);
            self.write(fields::RESET_COUNTDOWN, remaining);
            if remaining < 1 {
                call_slot_0(self.0, check_slots::INIT_STATUS_VARS);
            }
        }
        let old: i32 = self.read(guest_offsets::CHECK_COUNTDOWN);
        self.write(guest_offsets::CHECK_COUNTDOWN, old.wrapping_sub(1));
        if old == 0 && guest_type != 0 {
            self.write(guest_offsets::CHECK_COUNTDOWN, self.type_word(guest_type, status_type_offsets::CHECK_PERIOD).wrapping_sub(1));
            if self.read::<u8>(fields::PREDATOR_SEEN) != 0 {
                let target = call_slot_0(self.0, check_slots::TARGET);
                let settled = if target != 0 && get_from_memory::<u8>(target + TARGET_ACTIVE_OFFSET) != 0 {
                    Self::escape_list_is_empty()
                } else {
                    call_slot_1(self.0, check_slots::PREDATOR_IN_SIGHT, 0) == 0
                };
                if settled {
                    self.write(fields::PREDATOR_SEEN, 0u8);
                    if self.read::<u8>(fields::TARGET_FLAG) != 0 {
                        self.go_to_walk_dest();
                    }
                }
            }
        }
        unsafe {
            DO_CHASE_CHECK.hooked()(self.0 as *const u32);
            DO_LEAVE_ZOO_CHECK.hooked()(self.0 as *const u32);
            DO_BATHROOM_CHECK.hooked()(self.0 as *const u32);
            DO_HUNGER_CHECK.hooked()(self.0 as *const u32);
            DO_THIRSTY_CHECK.hooked()(self.0 as *const u32);
            DO_ENERGY_CHECK.hooked()(self.0 as *const u32);
            DO_TRASH_CHECK.hooked()(self.0 as *const u32);
            DO_BUY_SOUVENIR_CHECK.hooked()(self.0 as *const u32);
        }
        self.write(guest_offsets::RANDOM_DONE, 0u8);
        unsafe {
            DO_VIEWING_AREA_CHECK.hooked()(self.0 as *const u32);
            DO_ENVIRONMENT_EFFECT_CHECK.hooked()(self.0 as *const u32);
        }
    }

    /// `ZTGuest::updateStatusVariables` (`0x0043b03a`): folds each pending change into its level, clamps the level
    /// to the type's range and clears the change (the happiness, hunger, thirst, energy and bathroom pairs).
    pub fn update_status_variables(self) {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        let pairs: [(u32, u32, u32, u32); 5] = [
            (guest_offsets::HAPPINESS, fields::HAPPY_DELTA, type_offsets::HAPPY_HIGH, type_offsets::HAPPY_LOW),
            (guest_offsets::HUNGER, fields::HUNGER_DELTA, type_offsets::HUNGER_HIGH, type_offsets::HUNGER_LOW),
            (guest_offsets::THIRST, fields::THIRST_DELTA, type_offsets::THIRST_HIGH, type_offsets::THIRST_LOW),
            (guest_offsets::ENERGY, fields::ENERGY_DELTA, type_offsets::ENERGY_HIGH, type_offsets::ENERGY_LOW),
            (guest_offsets::BATHROOM, fields::BATHROOM_DELTA, type_offsets::BATHROOM_HIGH, type_offsets::BATHROOM_LOW),
        ];
        for (level, change, high, low) in pairs {
            let mut value = self.read::<i32>(level).wrapping_add(self.read::<i32>(change));
            self.write(change, 0i32);
            if self.type_word(guest_type, high) < value {
                value = self.type_word(guest_type, high);
            }
            if value < self.type_word(guest_type, low) {
                value = self.type_word(guest_type, low);
            }
            self.write(level, value);
        }
    }

    /// `ZTGuest::checkPredator` (`0x004302ea`): whether `unit` is an animal that preys on people.
    pub fn check_predator(unit: u32) -> bool {
        if unit == 0 || !unsafe { entity_type_matches(unit, ANIMAL_TYPE_TAG_RVA) } {
            return false;
        }
        unsafe { PREYS_ON_MAN.hooked()(unit as *const u32) }
    }

    /// `ZTGuest::leaveOK` (`0x0043a7e9`): the guest has decided to leave (`+0x339`), is not fleeing (`+0x24c`), not
    /// busy (`+0x25c`), not in goal `0x71`/`0x75`/`0x7f`/`0x68`-`0x6a`, not on water, and its target entity's
    /// predicate (vtable `+0x20`) does not hold. `false` without a target entity, where vanilla faults.
    pub fn leave_ok(self) -> bool {
        let goal = self.goal_id();
        let target = call_slot_0(self.0, check_slots::TARGET);
        if target == 0 || low_byte_bool(call_slot_0(target, 0x20)) {
            return false;
        }
        if self.read::<u8>(fields::LEAVING) == 0 || self.read::<u8>(fields::FLEEING) != 0 || self.read::<u8>(unit_fields::FLAG_0X25C) != 0 {
            return false;
        }
        if goal == GOAL_STANDING_DOWN || goal == GOAL_GRANDSTAND || goal == GOAL_GRANDSTAND_ESCAPE {
            return false;
        }
        if (0x68..=0x6a).contains(&self.goal_id()) {
            return false;
        }
        !self.is_on_water()
    }

    /// `ZTGuest::doViewingAreaCheck` (`0x0043afde`): when its countdown (`+0x30c`) runs out it reloads from the type
    /// (`+0x1d8`), forgets the viewing area (`+0x328` = `-1`, `+0x32c` = `0`) and looks for one. With a viewing area
    /// holding animals: sick animals add the sick bonus (`+0x27c`) and think `0x2931` (and `0x2770` for several); the
    /// average animal happiness picks one of six bands, each adding its own bonus (`+0x28c`..`+0x29c`), remembering a
    /// matching animal (`+0x330`) and thinking about it; a variety of species and a crowded area add theirs too.
    pub fn do_viewing_area_check(self) {
        let guest_type = self.guest_type();
        let old: i32 = self.read(fields::VIEWING_COUNTDOWN);
        self.write(fields::VIEWING_COUNTDOWN, old.wrapping_sub(1));
        if old != 0 || guest_type == 0 {
            return;
        }
        self.write(fields::VIEWING_COUNTDOWN, self.type_word(guest_type, viewing_check::PERIOD).wrapping_sub(1));
        self.write(guest_offsets::VIEWING_AREA, 0u32);
        self.write(guest_offsets::VIEWING_AREA_ID, -1i32);
        if !self.find_viewing_area() {
            return;
        }
        let this = self.0 as *const c_void;
        if unsafe { F_NUM_ANIMALS.hooked()(this) } as i32 <= 0 {
            return;
        }
        if low_byte_bool(unsafe { F_SICK_ANIMALS.hooked()(this) }) {
            self.add_happiness_delta(self.type_word(guest_type, viewing_check::SICK_BONUS));
            if unsafe { F_NUM_SICK_ANIMALS.hooked()(this) } as i32 > 1 {
                self.thought(0x2770, 0);
            }
            let animal = self.find_animal(-100, 100);
            self.write(viewing_check::TARGET, animal);
            self.thought(0x2931, animal);
        }
        let happiness = self.animal_happy();
        // (low, high, bonus offset, thought) for the band the average happiness falls in.
        let band: Option<(i32, i32, u32, u32)> = if happiness >= 0x51 {
            Some((0x51, 100, viewing_check::OVERJOYED, 0x2772))
        } else if happiness >= 0x3d {
            Some((0x3d, 0x50, viewing_check::HAPPY, 0x2829))
        } else if (-0x3c..=0x3c).contains(&happiness) {
            if happiness >= 0 {
                Some((0, 0x3c, viewing_check::PLEASED, 0x282a))
            } else {
                Some((-0x3d, -1, viewing_check::PLEASED, 0x282c))
            }
        } else if happiness >= -0x50 {
            Some((-0x50, -0x3d, viewing_check::ANGRY, 0x282b))
        } else {
            Some((-100, -0x51, viewing_check::FURIOUS, 0x2773))
        };
        if let Some((low, high, bonus, thought)) = band {
            self.add_happiness_delta(self.type_word(guest_type, bonus));
            let animal = self.find_animal(low, high);
            self.write(viewing_check::TARGET, animal);
            if animal != 0 {
                self.thought(thought, animal);
            }
        }
        if unsafe { F_DIFFERENT_SPECIES.hooked()(this) } as i32 >= self.type_word(guest_type, viewing_check::VARIETY_MIN) {
            self.add_happiness_delta(self.type_word(guest_type, viewing_check::VARIETY_BONUS));
            self.thought(0x2774, 0);
        }
        if (unsafe { F_CROWD_DENSITY.hooked()(this) } as i32) >= self.type_word(guest_type, viewing_check::CROWD_MIN) {
            self.add_happiness_delta(self.type_word(guest_type, viewing_check::CROWD_BONUS));
            let animal = self.find_animal(-100, 100);
            if animal != 0 {
                self.thought(0x2776, animal);
            }
        }
    }

    fn find_animal(self, low: i32, high: i32) -> u32 {
        unsafe { FIND_ANIMAL_BY_HAPPINESS.hooked()(self.0 as *const u32, low, high) as u32 }
    }

    /// The viewing area `getViewingArea` returns, with its characteristics (and then its observable area) recalculated
    /// when dirty, as every statistics query does first. `0` without a viewing area.
    fn refreshed_viewing_area(self) -> u32 {
        let area = self.viewing_area();
        if area == 0 {
            return 0;
        }
        if get_from_memory::<u8>(area + viewing_area_fields::CHARACTERISTICS_DIRTY) != 0 {
            recalculate_viewing_area(area);
        }
        if get_from_memory::<u8>(area + viewing_area_fields::OA_DIRTY) != 0 {
            recalculate_observable_area(area + viewing_area_fields::OBSERVABLE_AREA);
        }
        area
    }

    fn area_statistic(self, offset: u32) -> u32 {
        match self.refreshed_viewing_area() {
            0 => 0,
            area => get_from_memory(area + offset),
        }
    }

    /// `ZTGuest::fNumAnimals` (`0x0042330b`): the animals in the viewing area's observable area, `0` without one.
    pub fn num_animals(self) -> u32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::NUM_ANIMALS.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        self.area_statistic(viewing_area_fields::ANIMAL_COUNT)
    }

    /// `ZTGuest::fSickAnimals` (`0x00425f2e`): whether the observable area holds any sick animal. Vanilla leaves the
    /// register's upper bytes undefined; callers test the low byte.
    pub fn sick_animals(self) -> u32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::SICK_ANIMALS.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        (self.area_statistic(viewing_area_fields::SICK_COUNT) as i32 > 0) as u32
    }

    /// `ZTGuest::fNumSickAnimals` (`0x00589693`): the number of sick animals in the observable area.
    pub fn num_sick_animals(self) -> u32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::NUM_SICK_ANIMALS.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        self.area_statistic(viewing_area_fields::SICK_COUNT)
    }

    /// `ZTGuest::fDifferentSpecies` (`0x00426013`): the number of species in the observable area.
    pub fn different_species(self) -> u32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::DIFFERENT_SPECIES.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        self.area_statistic(viewing_area_fields::DIFFERENT_SPECIES)
    }

    /// `ZTGuest::fCrowdDensity` (`0x00425f6a`): the viewing area's `+0x54` count over its `+0x40..+0x44` vector length,
    /// scaled and truncated to a 64-bit result (the x87 `FISTP` truncation, so an out-of-range value is `i64::MIN`).
    /// Callers read the low dword. The vector length is read before the dirty characteristics are recalculated, as
    /// vanilla does. `None` when there is no viewing area or the length equals the guard constant (`DAT_00630d5c`):
    /// vanilla then returns whatever `__ftol` and a CRT helper leave in `EDX:EAX`, which the detour leaves to the real
    /// body.
    pub fn crowd_density(self) -> Option<i64> {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::CROWD_DENSITY.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return Some(answer);
            }
        }
        let area = self.viewing_area();
        if area == 0 {
            return None;
        }
        let length = get_from_memory::<u32>(area + viewing_area_fields::CROWD_VECTOR_END).wrapping_sub(get_from_memory::<u32>(area + viewing_area_fields::CROWD_VECTOR_BEGIN));
        let slots = (length as i32 >> 2) as f32;
        if get_from_memory::<u8>(area + viewing_area_fields::CHARACTERISTICS_DIRTY) != 0 {
            recalculate_viewing_area(area);
        }
        if slots == get_from_memory::<f32>(base() + CROWD_GUARD_RVA) {
            return None;
        }
        let scaled = (get_from_memory::<i32>(area + viewing_area_fields::CROWD_COUNT) as f64 / slots as f64) * get_from_memory::<f32>(base() + CROWD_SCALE_RVA) as f64;
        Some(if scaled.is_nan() || scaled >= -(i64::MIN as f64) || scaled < i64::MIN as f64 { i64::MIN } else { scaled as i64 })
    }

    /// `ZTGuest::findAnimalByHappiness` (`0x00426049`): an animal of the viewing area's habitat whose happiness lies in
    /// `low..=high`, or `0`. One draw of the game RNG (only when the habitat has animals) is reduced by the animal
    /// count; the first matching animal at an index at or past the reduced draw is returned, else the last match.
    pub fn find_animal_by_happiness(self, low: i32, high: i32) -> u32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = super::check_overrides::FIND_ANIMAL.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0, low, high)) {
                return answer;
            }
        }
        let area = self.viewing_area();
        if area == 0 {
            return 0;
        }
        let habitat: u32 = get_from_memory(area + viewing_area_fields::HABITAT);
        let animals = unsafe { GET_ALL_ANIMALS.hooked()(habitat as *const u32, 0) } as u32;
        let begin: u32 = get_from_memory(animals);
        let bytes = get_from_memory::<u32>(animals + 4).wrapping_sub(begin);
        if (bytes & !3) as i32 <= 0 {
            return 0;
        }
        let draw = next_game_random();
        let count = bytes >> 2;
        let mut found = 0;
        for index in 0..count {
            let animal: u32 = get_from_memory(begin + index * 4);
            let happiness: i32 = get_from_memory(animal + ANIMAL_HAPPINESS);
            if low <= happiness && happiness <= high {
                found = animal;
                if draw % count <= index {
                    return animal;
                }
            }
        }
        found
    }

    /// `ZTGuest::doAmbientAnims` (`0x00422498`): a guest with `+0x33b` set also sets `+0x339`, then `ZTUnit`'s
    /// ambient animations run with no vector.
    pub fn do_ambient_anims(self) {
        if self.read::<u8>(AMBIENT_TRIGGER_FLAG) != 0 {
            self.write(guest_offsets::FLAG_0X339, 1u8);
        }
        crate::ztunit::ZTUnit(self.0).do_ambient_anims(0);
    }

    /// `ZTGuest::checkLeaveZoo` (`0x004a6acc`): with an AI manager and a guest type, and outside goal `0x71` and off the
    /// water, one draw against the type's leave chance (`+0x258`) decides: on success the guest is marked as leaving
    /// (`+0x339`) and thinks `0x27fe`.
    pub fn check_leave_zoo(self) -> bool {
        let aimgr = globals().ztaimgr_ptr() as u32;
        let guest_type = self.guest_type();
        if aimgr == 0 || guest_type == 0 || self.goal_id() == GOAL_STANDING_DOWN || self.is_on_water() {
            return false;
        }
        if self.type_word(guest_type, check_type::LEAVE_CHANCE_CHECK) > percent_roll() {
            self.write(fields::LEAVING, 1u8);
            self.thought(0x27fe, 0);
            return true;
        }
        false
    }
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(DO_VIEWING_AREA_CHECK)]
    unsafe extern "thiscall" fn do_viewing_area_check(this: *const u32) {
        ZTGuest(this as u32).do_viewing_area_check();
    }

    #[detour(DO_CONDITIONALS)]
    unsafe extern "thiscall" fn do_conditionals(this: *const u32) {
        ZTGuest(this as u32).do_conditionals();
    }

    #[detour(DO_ENVIRONMENT_EFFECT_CHECK)]
    unsafe extern "thiscall" fn do_environment_effect_check(this: *const u32) {
        ZTGuest(this as u32).do_environment_effect_check();
    }

    #[detour(DO_CHASE_CHECK)]
    unsafe extern "thiscall" fn do_chase_check(this: *const u32) {
        ZTGuest(this as u32).do_chase_check();
    }

    #[detour(DO_LEAVE_ZOO_CHECK)]
    unsafe extern "thiscall" fn do_leave_zoo_check(this: *const u32) {
        ZTGuest(this as u32).do_leave_zoo_check();
    }

    #[detour(DO_BATHROOM_CHECK)]
    unsafe extern "thiscall" fn do_bathroom_check(this: *const u32) {
        ZTGuest(this as u32).do_bathroom_check();
    }

    #[detour(DO_HUNGER_CHECK)]
    unsafe extern "thiscall" fn do_hunger_check(this: *const u32) {
        ZTGuest(this as u32).do_hunger_check();
    }

    #[detour(DO_THIRSTY_CHECK)]
    unsafe extern "thiscall" fn do_thirsty_check(this: *const u32) {
        ZTGuest(this as u32).do_thirsty_check();
    }

    #[detour(DO_ENERGY_CHECK)]
    unsafe extern "thiscall" fn do_energy_check(this: *const u32) {
        ZTGuest(this as u32).do_energy_check();
    }

    #[detour(DO_TRASH_CHECK)]
    unsafe extern "thiscall" fn do_trash_check(this: *const u32) {
        ZTGuest(this as u32).do_trash_check();
    }

    #[detour(DO_BUY_SOUVENIR_CHECK)]
    unsafe extern "thiscall" fn do_buy_souvenir_check(this: *const u32) {
        ZTGuest(this as u32).do_buy_souvenir_check();
    }

    #[detour(UPDATE_STATUS_VARIABLES)]
    unsafe extern "thiscall" fn update_status_variables(this: *const u32) {
        ZTGuest(this as u32).update_status_variables();
    }

    #[detour(CHECK_PREDATOR)]
    unsafe extern "thiscall" fn check_predator(_this: *const u32, unit: *const u32, _param: bool) -> bool {
        ZTGuest::check_predator(unit as u32)
    }

    #[detour(LEAVE_OK)]
    unsafe extern "thiscall" fn leave_ok(this: *const u32) -> bool {
        ZTGuest(this as u32).leave_ok()
    }

    #[detour(F_NUM_ANIMALS)]
    unsafe extern "fastcall" fn num_animals(this: *const c_void) -> u32 {
        ZTGuest(this as u32).num_animals()
    }

    #[detour(F_SICK_ANIMALS)]
    unsafe extern "fastcall" fn sick_animals(this: *const c_void) -> u32 {
        ZTGuest(this as u32).sick_animals()
    }

    #[detour(F_NUM_SICK_ANIMALS)]
    unsafe extern "fastcall" fn num_sick_animals(this: *const c_void) -> u32 {
        ZTGuest(this as u32).num_sick_animals()
    }

    #[detour(F_DIFFERENT_SPECIES)]
    unsafe extern "fastcall" fn different_species(this: *const c_void) -> u32 {
        ZTGuest(this as u32).different_species()
    }

    #[detour(F_CROWD_DENSITY)]
    unsafe extern "fastcall" fn crowd_density(this: *const c_void) -> i64 {
        match ZTGuest(this as u32).crowd_density() {
            Some(value) => value,
            None => unsafe { F_CROWD_DENSITY_DETOUR.call(this) },
        }
    }

    #[detour(FIND_ANIMAL_BY_HAPPINESS)]
    unsafe extern "thiscall" fn find_animal_by_happiness(this: *const u32, low: i32, high: i32) -> i32 {
        ZTGuest(this as u32).find_animal_by_happiness(low, high) as i32
    }

    #[detour(DO_AMBIENT_ANIMS)]
    unsafe extern "thiscall" fn do_ambient_anims(this: *const u32, _vector: *const u32) {
        ZTGuest(this as u32).do_ambient_anims();
    }

    #[detour(CHECK_LEAVE_ZOO)]
    unsafe extern "thiscall" fn check_leave_zoo(this: *const u32) -> bool {
        ZTGuest(this as u32).check_leave_zoo()
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn do_viewing_area_check(this: u32) {
            unsafe { super::DO_VIEWING_AREA_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_conditionals(this: u32) {
            unsafe { super::DO_CONDITIONALS_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_environment_effect_check(this: u32) {
            unsafe { super::DO_ENVIRONMENT_EFFECT_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_chase_check(this: u32) {
            unsafe { super::DO_CHASE_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_leave_zoo_check(this: u32) {
            unsafe { super::DO_LEAVE_ZOO_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_bathroom_check(this: u32) {
            unsafe { super::DO_BATHROOM_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_hunger_check(this: u32) {
            unsafe { super::DO_HUNGER_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_thirsty_check(this: u32) {
            unsafe { super::DO_THIRSTY_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_energy_check(this: u32) {
            unsafe { super::DO_ENERGY_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_trash_check(this: u32) {
            unsafe { super::DO_TRASH_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_buy_souvenir_check(this: u32) {
            unsafe { super::DO_BUY_SOUVENIR_CHECK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn update_status_variables(this: u32) {
            unsafe { super::UPDATE_STATUS_VARIABLES_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn check_predator(this: u32, unit: u32) -> bool {
            unsafe { super::CHECK_PREDATOR_DETOUR.call(this as *const u32, unit as *const u32, false) }
        }
        pub(crate) fn leave_ok(this: u32) -> bool {
            unsafe { super::LEAVE_OK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn check_leave_zoo(this: u32) -> bool {
            unsafe { super::CHECK_LEAVE_ZOO_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn num_animals(this: u32) -> u32 {
            unsafe { super::F_NUM_ANIMALS_DETOUR.call(this as *const std::ffi::c_void) }
        }
        pub(crate) fn sick_animals(this: u32) -> u32 {
            unsafe { super::F_SICK_ANIMALS_DETOUR.call(this as *const std::ffi::c_void) }
        }
        pub(crate) fn num_sick_animals(this: u32) -> u32 {
            unsafe { super::F_NUM_SICK_ANIMALS_DETOUR.call(this as *const std::ffi::c_void) }
        }
        pub(crate) fn different_species(this: u32) -> u32 {
            unsafe { super::F_DIFFERENT_SPECIES_DETOUR.call(this as *const std::ffi::c_void) }
        }
        pub(crate) fn crowd_density(this: u32) -> i64 {
            unsafe { super::F_CROWD_DENSITY_DETOUR.call(this as *const std::ffi::c_void) }
        }
        pub(crate) fn find_animal_by_happiness(this: u32, low: i32, high: i32) -> i32 {
            unsafe { super::FIND_ANIMAL_BY_HAPPINESS_DETOUR.call(this as *const u32, low, high) }
        }
        pub(crate) fn do_ambient_anims(this: u32) {
            unsafe { super::DO_AMBIENT_ANIMS_DETOUR.call(this as *const u32, std::ptr::null()) }
        }
    }
}

/// Registers the pipeline detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest check detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
