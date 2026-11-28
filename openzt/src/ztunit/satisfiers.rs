//! `ZTUnit` target, predator and satisfier methods (group E of `plans/ztunit-scoping-plan.md`).

use std::mem::offset_of;

use openzt_detour::generated::{
    bfcoggoal::SET_ID as BFCOGGOAL_SET_ID,
    bfentity::{GET_GRID_POS, GET_TILE as BFENTITY_GET_TILE, SET_BS, SET_DIRECTION as BFENTITY_SET_DIRECTION},
    poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
    ztunit::{
        AVOID_DONE, CAUGHT_DONE, CHASE_DONE, CHECK_PREDATOR, FIND_NEARBY_NEED_SATISFIER, F_WAS_CAUGHT, FIND_ANY_SATISFIER, FIND_NEAREST_SATISFIER, FIND_PREDATOR, GET_PREDATOR_UNIT, GET_TARGET_UNIT, IN_FRIENDLY_CHASE, IS_FUN_SATISFIER, IS_NEED_OR_FUN_SATISFIER,
        RELEASE_PREDATOR,
    },
    ztworldmgr::SATISFIES,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{building_list::BuildingList, base, call_slot_2_returning, game_rand, globals, goal_slots, map_tile, GOAL_SET_DATA_RVA, tile_offsets, unit_offsets, unit_slots, ZTUnit, ANIMAL_TAG_RVA, BUILDING_TAG_RVA, BUILDING_NEED_NAME_RVA, UNIT_TYPE_TAG_RVA, WORLD_MGR_RVA};
use crate::{
    util::{get_from_memory, low_byte_bool, save_to_memory},
    bfentity::new_owned_name,
    bfentitytype::ZTBuildingType,
    ztbuilding::ZTBuilding,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2},
    ztanimal::ZTAnimal,
    ztmegatilemgr::entity_type_matches,
    vanilla_string::{object_bytes, range_bytes},
    vanilla_vector,
};

/// Vtable slots on the unit and on its entity type these methods dispatch through.
mod slots {
    /// `BFWorldMgr::getEntity(id, flag)`.
    pub const GET_ENTITY: u32 = 0x24;
    /// `BFEntity::getBlockingRect(out)`.
    pub const GET_BLOCKING_RECT: u32 = 0x24;
    /// `ZTUnit::isNeedSatisfier(tile, position)`.
    pub const IS_NEED_SATISFIER: u32 = 0x278;
    /// `ZTUnit::findPredator(flag)`.
    pub const FIND_PREDATOR: u32 = 0x2fc;
    /// `BFEntityType` slot answering the type id.
    pub const TYPE_ID: u32 = 0x20;
}

/// The two `BFEntity` bytes that mark an entity as removed or locked; such units never count as targets.
const REMOVED: u32 = 0x142;
const REMOVED_B: u32 = 0x143;
/// `BFEntity` word `chaseDone` and `avoidDone` compare against the AI manager's tick counter.
const LAST_THREAT_TICK: u32 = 0x138;
/// `GLOBAL_ZTAIMgr` and its tick counter, and the number of ticks a threat is remembered (`DAT_00639524`).
const AIMGR_RVA: u32 = 0x0023_8098;
const AIMGR_TICK: u32 = 0xec;
const THREAT_MEMORY_RVA: u32 = 0x0023_9524;
/// `ZTAnimal` fields `releasePredator` clears.
const ANIMAL_CAUGHT_FLAG: u32 = 0x3a5;
/// `ZTWorldMgr::satisfies` needs: the need name `isFunSatisfier` tests first, then the building need (`BUILDING_NEED_NAME_RVA`).
const FUN_NEED_NAME_RVA: u32 = 0x0023_af90;
/// `ZTAnimal` bytes `checkPredator` reads: two flags that rule an animal out, the one the staff check consults, and the
/// id of the unit it is chasing (`+0x2e4`).
const ANIMAL_RULED_OUT_A: u32 = 0x394;
const ANIMAL_RULED_OUT_B: u32 = 0x395;
const ANIMAL_STAFF_EXEMPT: u32 = 0x393;
const ANIMAL_PREY_ID: u32 = 0x2e4;
/// `ZTAnimalType` fields: the name string pointer (first letter `m`/`f` marks a man- or female-hunting type) and the byte
/// that lets any other type hunt too.
const TYPE_NAME: u32 = 0xa4;
const TYPE_HUNTS_ANYWAY: u32 = 0x3d4;
/// `isCastClass` tags: `ZTStaff` (`DAT_00638710`) and `ZTAnimal` as `ZTAnimalType` accepts it.
const STAFF_TAG_RVA: u32 = 0x0023_8710;
/// `BFEntityType` slot that tests a class tag.
const TYPE_IS_CAST_CLASS: u32 = 0x1c;
/// `findPredator`: the escaped-animal list's global (`DAT_00638fb0`, holding the sentinel; nodes are `{next, prev, entity}`),
/// the sight radius in tiles (`DAT_006394f4`) and the `checkPredator` slot it asks of each candidate.
const ESCAPE_LIST_RVA: u32 = 0x0023_8fb0;
const SIGHT_RADIUS_RVA: u32 = 0x0023_94f4;
const CHECK_PREDATOR_SLOT: u32 = 0x304;
/// `findNearestSatisfier`: the shared candidate vector (`DAT_0063c538`, `{begin, end, cap}` words), the building type's
/// commerce flag and default cost (compared with the building's price), and the two thresholds in `ZooStatus` (`ZTGameMgr +
/// 0x114` and `+ 0x118`).
const CANDIDATE_VECTOR_RVA: u32 = 0x0023_c538;
const TYPE_COMMERCE: u32 = offset_of!(ZTBuildingType, commerce) as u32;
const TYPE_DEFAULT_COST: u32 = offset_of!(ZTBuildingType, default_cost) as u32;
const GAME_MGR_LOW_THRESHOLD: u32 = 0x114;
const GAME_MGR_HIGH_THRESHOLD: u32 = 0x118;
/// `fWasCaught`: the goal that marks "caught by a predator" and the building-use goal range it leaves alone, the animal type's
/// two name strings (`{begin, end}` pairs), the `BFEntity` word holding the facing, and the behaviour sets it queues.
const CAUGHT_GOAL: u32 = 0x75;
const ANIMAL_TYPE_FIRST_NAME: u32 = 0x98;
const ANIMAL_TYPE_SECOND_NAME: u32 = 0xa4;
const FACING: u32 = 0x12c;
const CAUGHT_BY_PREFIX: &[u8] = b"bCaughtBy";
const CAUGHT_BY_LION_FEMALE: &[u8] = b"bCaughtBylionf";
const CAUGHT_BY_LION_MALE: &[u8] = b"bCaughtBylionm";
/// `BFBSCall` arguments `fWasCaught` passes `setBS`: call flag, force and clear mode.
const CAUGHT_BS_FLAGS: (i8, i8, i32) = (0, 1, 2);
/// `ZTHabitat` byte `checkPredator` tests for a staff member standing in a habitat.
const HABITAT_BYTE_2C: u32 = 0x2c;
/// The two behaviour names a caught unit plays, which `caughtDone` waits out.
const CAUGHT_ANIMATIONS: [&[u8]; 2] = [b"bCaughtBy", b"bDefense"];

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

/// The unit `id` names when it is a live, unlocked unit, else `0`.
fn live_unit_by_id(id: u32) -> u32 {
    if id == 0 {
        return 0;
    }
    let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
    let entity = call_slot_2_returning(world, slots::GET_ENTITY, id, 1);
    if entity == 0 || !unsafe { entity_type_matches(entity, UNIT_TYPE_TAG_RVA) } {
        return 0;
    }
    if get_from_memory::<u8>(entity + REMOVED_B) == 0 && get_from_memory::<u8>(entity + REMOVED) == 0 {
        entity
    } else {
        0
    }
}

/// The squared distance between two tiles, `i32::MAX` when either is missing.
fn tile_distance_squared(from: u32, to: u32) -> i32 {
    if from == 0 || to == 0 {
        return i32::MAX;
    }
    let dx = get_from_memory::<i32>(to + tile_offsets::X).wrapping_sub(get_from_memory::<i32>(from + tile_offsets::X));
    let dy = get_from_memory::<i32>(to + tile_offsets::Y).wrapping_sub(get_from_memory::<i32>(from + tile_offsets::Y));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// Appends `building` to the `{begin, end, cap}` word vector at `vector`, growing as vanilla's `vector<int>::insert_n`
/// does (the old buffer is returned to the pool only when there is one).
fn push_candidate(vector: u32, building: u32) {
    vanilla_vector::push_word(vector, building, |begin, bytes| {
        if begin != 0 {
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
        }
    });
}

impl ZTUnit {
    /// `ZTUnit::getTargetUnit` (`0x004a4b53`): the unit being chased or fled from, when it is still live.
    pub fn get_target_unit(self) -> u32 {
        live_unit_by_id(self.read(unit_offsets::TARGET_UNIT_ID))
    }

    /// `ZTUnit::getPredatorUnit` (`0x00412259`): the recorded predator, when it is still live.
    pub fn get_predator_unit(self) -> u32 {
        live_unit_by_id(self.read(unit_offsets::PREDATOR))
    }

    /// `ZTUnit::releasePredator` (`0x00431af4`): makes the predator visible again, resets an animal predator's chase, and
    /// forgets it.
    pub fn release_predator(self) {
        let predator = self.get_predator_unit();
        if predator != 0 {
            call_slot_1(predator, goal_slots::SET_VISIBLE, 1);
            if unsafe { entity_type_matches(predator, ANIMAL_TAG_RVA) } {
                save_to_memory(predator + ANIMAL_CAUGHT_FLAG, 0u8);
                save_to_memory(predator + unit_offsets::BYTE_20D, 0u8);
                call_slot_0(predator, goal_slots::CLEAR_GOAL);
            }
        }
        self.write(unit_offsets::PREDATOR, 0u32);
    }

    /// `ZTUnit::isFunSatisfier` (`0x004373eb`): whether the building in `position`'s quadrant of the `0x40`-tile grid
    /// `quadrants` (four entity words, `2 x 2`) satisfies the fun need or the building need.
    pub fn is_fun_satisfier(quadrants: u32, position: *const i32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::is_fun_satisfier(quadrants, position) {
            return answer;
        }
        if quadrants == 0 {
            return false;
        }
        let (x, y): (i32, i32) = (get_from_memory(position as u32), get_from_memory(position as u32 + 4));
        if x < 0 || y < 0 || x >= 0x40 || y >= 0x40 {
            return false;
        }
        let building: u32 = get_from_memory(quadrants + 4 + ((y >> 5) + (x >> 5) * 2) as u32 * 4);
        if building == 0 || !unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } {
            return false;
        }
        let satisfies = |name_rva: u32| low_byte_bool(unsafe { SATISFIES.hooked()(building as i32, (base() + name_rva) as *const i8, std::ptr::null()) });
        satisfies(FUN_NEED_NAME_RVA) || satisfies(BUILDING_NEED_NAME_RVA)
    }

    /// `ZTUnit::isNeedOrFunSatisfier` (`0x0043735c`).
    pub fn is_need_or_fun_satisfier(self, quadrants: u32, position: *const i32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::is_need_or_fun_satisfier(self.0, quadrants, position) {
            return answer;
        }
        low_byte_bool(call_slot_2_returning(self.0, slots::IS_NEED_SATISFIER, quadrants, position as u32)) || Self::is_fun_satisfier(quadrants, position)
    }

    /// `ZTUnit::inFriendlyChase` (`0x004a4bb7`): the target is an animal of the unit's own type.
    pub fn in_friendly_chase(self) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::in_friendly_chase(self.0) {
            return answer;
        }
        let target = self.get_target_unit();
        if target == 0 || !unsafe { entity_type_matches(target, ANIMAL_TAG_RVA) } {
            return false;
        }
        let target_type: u32 = get_from_memory(target + unit_offsets::ENTITY_TYPE);
        let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        call_slot_0(target_type, slots::TYPE_ID) == call_slot_0(own_type, slots::TYPE_ID)
    }

    /// Whether the unit's last threat tick is at least `DAT_00639524` ticks old (signed, as vanilla compares).
    fn threat_expired(self) -> bool {
        let aimgr: u32 = get_from_memory(base() + AIMGR_RVA);
        let now: i32 = get_from_memory(aimgr + AIMGR_TICK);
        let last: i32 = self.read(LAST_THREAT_TICK);
        let memory: i32 = get_from_memory(base() + THREAT_MEMORY_RVA);
        now.wrapping_sub(last) >= memory
    }

    /// `ZTUnit::avoidDone` (`0x004a5508`): a friendly chase is over once the threat expired; any other avoidance is over
    /// when the unit finds no predator (checked both ways).
    pub fn avoid_done(self) -> u32 {
        if self.in_friendly_chase() && self.threat_expired() {
            return 1;
        }
        if call_slot_1(self.0, slots::FIND_PREDATOR, 0) != 0 || call_slot_1(self.0, slots::FIND_PREDATOR, 1) != 0 {
            return 0;
        }
        1
    }

    /// `ZTUnit::chaseDone` (`0x004a4ca0`): a friendly chase is done once the two blocking rectangles touch or the threat
    /// expired; any other chase once the units share a grid position.
    pub fn chase_done(self, other: u32) -> bool {
        let expired = self.threat_expired();
        if self.in_friendly_chase() {
            let rect = |unit: u32| -> [i32; 4] {
                let mut out = [0i32; 4];
                let rect = call_slot_1(unit, slots::GET_BLOCKING_RECT, out.as_mut_ptr() as u32);
                [get_from_memory(rect), get_from_memory(rect + 4), get_from_memory(rect + 8), get_from_memory(rect + 12)]
            };
            let (a, b) = (rect(self.0), rect(other));
            let intersects = a[0].max(b[0]) < a[2].min(b[2]) && a[1].max(b[1]) < a[3].min(b[3]);
            return intersects || expired;
        }
        let grid = |unit: u32| -> [i32; 3] {
            let mut out = [0i32; 3];
            unsafe { GET_GRID_POS.hooked()(unit as *const u32, out.as_mut_ptr()) };
            out
        };
        grid(other) == grid(self.0)
    }

    /// `ZTUnit::caughtDone` (`0x004a60b9`): `1` unless the unit's current behaviour name holds `bCaughtBy` or `bDefense`.
    /// Vanilla copies the name into a `0x80`-byte stack buffer first; the port searches it in place.
    pub fn caught_done(self) -> u8 {
        let mut name = range_bytes(self.read(0x54), self.read(0x58));
        if let Some(nul) = name.iter().position(|&byte| byte == 0) {
            name.truncate(nul);
        }
        u8::from(!CAUGHT_ANIMATIONS.iter().any(|needle| contains(&name, needle)))
    }

    /// `ZTUnit::checkPredator` (`0x004391a2`): whether `candidate` is a predator of this unit (`chasing` false) or is
    /// chasing this unit (`chasing` true). Vanilla dereferences a null animal type, null habitat and the like; the port
    /// answers `false` there.
    pub fn check_predator(self, candidate: u32, chasing: bool) -> bool {
        if candidate == 0 || !unsafe { entity_type_matches(candidate, ANIMAL_TAG_RVA) } {
            return false;
        }
        if get_from_memory::<u8>(candidate + ANIMAL_RULED_OUT_A) != 0 || get_from_memory::<u8>(candidate + ANIMAL_RULED_OUT_B) != 0 {
            return false;
        }
        if !unsafe { entity_type_matches(self.0, UNIT_TYPE_TAG_RVA) } {
            return false;
        }
        let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let own_type_id = call_slot_0(own_type, slots::TYPE_ID);
        let candidate_type: u32 = get_from_memory(candidate + unit_offsets::ENTITY_TYPE);
        let candidate_is_animal_type = candidate_type != 0 && low_byte_bool(call_slot_1(candidate_type, TYPE_IS_CAST_CLASS, base() + ANIMAL_TAG_RVA));
        if chasing {
            if !candidate_is_animal_type {
                return false;
            }
            return own_type_id == call_slot_0(candidate_type, slots::TYPE_ID) && get_from_memory::<u32>(candidate + ANIMAL_PREY_ID) == self.read::<u32>(unit_offsets::ENTITY_ID);
        }
        if !candidate_is_animal_type {
            return false;
        }
        let name: u32 = get_from_memory(candidate_type + TYPE_NAME);
        let initial = if name == 0 { 0 } else { get_from_memory::<u8>(name) };
        if initial != b'm' && initial != b'f' && get_from_memory::<u8>(candidate_type + TYPE_HUNTS_ANYWAY) == 0 {
            return false;
        }
        let mut hunts = ZTAnimal(candidate).is_in_prey_list(own_type_id as i32);
        if hunts && unsafe { entity_type_matches(self.0, STAFF_TAG_RVA) } {
            let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
            if habitat == 0 || get_from_memory::<u8>(habitat + HABITAT_BYTE_2C) == 0 || get_from_memory::<u8>(candidate + ANIMAL_STAFF_EXEMPT) != 0 {
                hunts = false;
            }
        }
        hunts
    }

    /// `ZTUnit::findNearestSatisfier` (`0x004290f3`): among the buildings of `name`'s building list (those of the type id
    /// `filter` unless it is `-1`) that stand in the same habitat as `tile` and have an open slot for the unit, collects
    /// those at the smallest squared distance from `tile` (within `range`) in the shared candidate vector and stores a
    /// random one at `out_building`. A building type that scales by happiness (`+0x23d`) has the distance replaced by
    /// `(int)(sqrt(distance) * threshold)` squared, the threshold being the high one at or above the building's fill
    /// against `high * scale`, the low one at or below `low * scale`, and no change in between. Vanilla dereferences a
    /// type-less building when filtering; the port skips it.
    pub fn find_nearest_satisfier(self, tile: u32, name: u32, out_building: u32, range: i32, filter: i32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::find_nearest_satisfier(self.0, tile, name, out_building, range, filter) {
            return answer;
        }
        let buildings = BuildingList::named(name as *const i32);
        let candidates = base() + CANDIDATE_VECTOR_RVA;
        save_to_memory(candidates + 4, get_from_memory::<u32>(candidates));
        let game_manager = globals().ztgamemgr() as *const _ as u32;
        let mut best = range.wrapping_mul(range);
        let mut found = false;
        for building in buildings.buildings() {
            if building == 0 {
                continue;
            }
            let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
            if filter != -1 && (building_type == 0 || call_slot_0(building_type, slots::TYPE_ID) as i32 != filter) {
                continue;
            }
            let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
            let building_habitat = call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, building_tile);
            if building_habitat != call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, tile) {
                continue;
            }
            if !ZTBuilding(building).has_open_slot(self.0) {
                continue;
            }
            let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
            let mut distance = tile_distance_squared(tile, building_tile);
            if building_type != 0 && unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } && get_from_memory::<u8>(building_type + TYPE_COMMERCE) != 0 {
                let default_cost = f64::from(get_from_memory::<f32>(building_type + TYPE_DEFAULT_COST));
                let price = f64::from(ZTBuilding(building).price());
                let low = f64::from(get_from_memory::<f32>(game_manager + GAME_MGR_LOW_THRESHOLD));
                let high = f64::from(get_from_memory::<f32>(game_manager + GAME_MGR_HIGH_THRESHOLD));
                // The x87 compares treat a NaN as "below"/"not above", which the negated comparisons reproduce.
                let threshold = if !(price >= high * default_cost) {
                    (!(price > low * default_cost)).then_some(low)
                } else {
                    Some(high)
                };
                if let Some(threshold) = threshold {
                    let scaled = crate::util::ftol_low32_f64(f64::from(distance).sqrt() * threshold);
                    distance = scaled.wrapping_mul(scaled);
                }
            }
            if distance <= best {
                found = true;
                if distance < best {
                    save_to_memory(candidates + 4, get_from_memory::<u32>(candidates));
                }
                push_candidate(candidates, building);
                best = distance;
            }
        }
        if found {
            let begin: u32 = get_from_memory(candidates);
            let count = (get_from_memory::<u32>(candidates + 4).wrapping_sub(begin) as i32) >> 2;
            let pick: u32 = get_from_memory(begin + (game_rand().wrapping_rem(count) as u32).wrapping_mul(4));
            save_to_memory(out_building, pick);
        }
        found
    }

    /// `ZTUnit::findAnySatisfier` (`0x005a033a`): [`Self::find_nearest_satisfier`] without the nearness contest: every
    /// building of `name`'s list (of the type id `filter` unless it is `-1`) in the habitat of `tile`, with an open slot
    /// for the unit and within `range` (squared tile distance) is appended to the shared candidate vector, and one of them
    /// chosen at random is stored at `out_building`. Vanilla dereferences a type-less building when filtering; the port
    /// skips it.
    pub fn find_any_satisfier(self, tile: u32, name: u32, out_building: u32, range: i32, filter: i32) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::find_any_satisfier(self.0, tile, name, out_building, range, filter) {
            return answer;
        }
        let buildings = BuildingList::named(name as *const i32);
        let candidates = base() + CANDIDATE_VECTOR_RVA;
        save_to_memory(candidates + 4, get_from_memory::<u32>(candidates));
        let limit = range.wrapping_mul(range);
        let mut found = false;
        for building in buildings.buildings() {
            if building == 0 {
                continue;
            }
            if filter != -1 {
                let building_type: u32 = get_from_memory(building + unit_offsets::ENTITY_TYPE);
                if building_type == 0 || call_slot_0(building_type, slots::TYPE_ID) as i32 != filter {
                    continue;
                }
            }
            let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
            let building_habitat = call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, building_tile);
            if building_habitat != call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, tile) {
                continue;
            }
            if !ZTBuilding(building).has_open_slot(self.0) {
                continue;
            }
            let building_tile = unsafe { BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
            if tile_distance_squared(tile, building_tile) <= limit {
                found = true;
                push_candidate(candidates, building);
            }
        }
        if found {
            let begin: u32 = get_from_memory(candidates);
            let count = (get_from_memory::<u32>(candidates + 4).wrapping_sub(begin) as i32) >> 2;
            let pick: u32 = get_from_memory(begin + (game_rand().wrapping_rem(count) as u32).wrapping_mul(4));
            save_to_memory(out_building, pick);
        }
        found
    }

    /// `ZTUnit::fWasCaught` (`0x0050ed8f`): when the recorded predator is a live animal, the unit takes on the "caught"
    /// goal (unless it is already `0x75`, or in the building-use goals `0x68`-`0x6a`: it clears its goal, sets `0x75` with
    /// `3`, points the goal at the predator and records the predator's id), turns to the predator's facing and queues the
    /// behaviour set `bCaughtBy` + the predator type's two name strings (`+0x98`, `+0xa4`). A failed queue falls back to
    /// `bCaughtBylionf` for a type whose second name starts with `f`, else `bCaughtBylionm`. When one is queued the unit is
    /// marked busy (`+0x25c`) and the goal's flag byte (`+0x1a8`) cleared. Returns whether it was queued.
    pub fn f_was_caught(self) -> bool {
        let predator = self.get_predator_unit();
        if predator == 0 || !unsafe { entity_type_matches(predator, ANIMAL_TAG_RVA) } {
            return false;
        }
        let goal: u32 = self.read(unit_offsets::GOAL_ID);
        if goal != CAUGHT_GOAL && !(0x68..=0x6a).contains(&goal) {
            let predator_id: u32 = get_from_memory(predator + unit_offsets::ENTITY_ID);
            call_slot_0(self.0, goal_slots::CLEAR_GOAL);
            call_slot_2(self.0, goal_slots::GOAL_SET, CAUGHT_GOAL, 3);
            unsafe { BFCOGGOAL_SET_ID.hooked()((self.0 + unit_offsets::COG_GOAL) as *const u32, predator_id, (base() + GOAL_SET_DATA_RVA) as *const i32) };
            self.write(unit_offsets::PREDATOR, predator_id);
        }
        unsafe { BFENTITY_SET_DIRECTION.hooked()(self.0 as *const u32, get_from_memory(predator + FACING)) };
        let predator_type: u32 = get_from_memory(predator + unit_offsets::ENTITY_TYPE);
        let text_at = |offset: u32| object_bytes(predator_type + offset);
        let mut name = CAUGHT_BY_PREFIX.to_vec();
        name.extend(text_at(ANIMAL_TYPE_FIRST_NAME));
        name.extend(text_at(ANIMAL_TYPE_SECOND_NAME));
        let queue = |text: &[u8]| -> bool {
            let name = unsafe { new_owned_name(text) };
            let (call_flag, force, clear_mode) = CAUGHT_BS_FLAGS;
            low_byte_bool(unsafe { SET_BS.hooked()(self.0 as *const u32, name.begin as *const u32, name.end as *const u32, name.capacity as *const u32, call_flag, force, clear_mode) } as u32)
        };
        let mut queued = queue(&name);
        if !queued {
            let second_name_begin: u32 = get_from_memory(predator_type + ANIMAL_TYPE_SECOND_NAME);
            let female = second_name_begin != 0 && get_from_memory::<u8>(second_name_begin) == b'f';
            queued = queue(if female { CAUGHT_BY_LION_FEMALE } else { CAUGHT_BY_LION_MALE });
        }
        if queued {
            self.write(unit_offsets::BUSY, 1u8);
            self.write(unit_offsets::COG_GOAL_FLAG, 0u8);
        }
        queued
    }

    /// `ZTUnit::findPredator` (`0x004369b9`): the id of the nearest unit within sight that `checkPredator(candidate, flag)`
    /// accepts, or `0`. The recorded predator wins while it is within sight and still accepted (its id, `+0x22c`, is
    /// returned). In a "world" habitat (`+0x2c` set) the candidates are the escaped animals; elsewhere the units standing on
    /// the tiles of the unit's own habitat in a square around it (a unit type is needed), the first nearest one winning.
    /// Vanilla dereferences a null habitat and a predator without a tile; the port answers `0` / skips the shortcut.
    pub fn find_predator(self, flag: bool) -> i32 {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(answer) = super::live_support::intercept::find_predator(self.0, flag) {
            return answer;
        }
        let unit_type = self.unit_type();
        let tile = self.tile();
        if tile == 0 {
            return 0;
        }
        let (tile_x, tile_y): (i32, i32) = (get_from_memory(tile + tile_offsets::X), get_from_memory(tile + tile_offsets::Y));
        let radius: i32 = get_from_memory(base() + SIGHT_RADIUS_RVA);
        let predator = self.get_predator_unit();
        if predator != 0 {
            let predator_tile = unsafe { BFENTITY_GET_TILE.hooked()(predator as *const u32) } as u32;
            if predator_tile != 0 {
                let (x, y): (i32, i32) = (get_from_memory(predator_tile + tile_offsets::X), get_from_memory(predator_tile + tile_offsets::Y));
                if x.wrapping_sub(tile_x).wrapping_abs() <= radius
                    && y.wrapping_sub(tile_y).wrapping_abs() <= radius
                    && low_byte_bool(call_slot_2_returning(self.0, CHECK_PREDATOR_SLOT, predator, u32::from(flag)))
                {
                    return self.read(unit_offsets::PREDATOR);
                }
            }
        }
        let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
        if habitat == 0 {
            return 0;
        }
        let distance = |other_tile: u32| -> i32 {
            if other_tile == 0 {
                return i32::MAX;
            }
            let dx = get_from_memory::<i32>(other_tile + tile_offsets::X).wrapping_sub(tile_x);
            let dy = get_from_memory::<i32>(other_tile + tile_offsets::Y).wrapping_sub(tile_y);
            dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
        };
        let mut best = radius.wrapping_mul(radius);
        let mut found = 0i32;
        if get_from_memory::<u8>(habitat + HABITAT_BYTE_2C) != 0 {
            let sentinel: u32 = get_from_memory(base() + ESCAPE_LIST_RVA);
            let mut node: u32 = get_from_memory(sentinel);
            while node != sentinel {
                let candidate: u32 = get_from_memory(node + 8);
                if low_byte_bool(call_slot_2_returning(self.0, CHECK_PREDATOR_SLOT, candidate, u32::from(flag))) {
                    let nearness = distance(unsafe { BFENTITY_GET_TILE.hooked()(candidate as *const u32) } as u32);
                    if nearness < best {
                        found = get_from_memory(candidate + unit_offsets::ENTITY_ID);
                        best = nearness;
                    }
                }
                node = get_from_memory(node);
            }
            return found;
        }
        if unit_type == 0 {
            return 0;
        }
        for x in tile_x.wrapping_sub(radius)..=tile_x.wrapping_add(radius) {
            for y in tile_y.wrapping_sub(radius)..=tile_y.wrapping_add(radius) {
                let candidate_tile = map_tile(x, y);
                if candidate_tile == 0 || call_slot_1(self.0, unit_slots::GET_HABITAT_FROM_TILE, candidate_tile) != habitat {
                    continue;
                }
                let sentinel: u32 = get_from_memory(candidate_tile + tile_offsets::ENTITY_LIST);
                let mut node: u32 = get_from_memory(sentinel);
                while node != sentinel {
                    let entity: u32 = get_from_memory(node + 8);
                    let unit = if entity != 0 && unsafe { entity_type_matches(entity, UNIT_TYPE_TAG_RVA) } { entity } else { 0 };
                    if low_byte_bool(call_slot_2_returning(self.0, CHECK_PREDATOR_SLOT, unit, u32::from(flag))) {
                        let nearness = distance(candidate_tile);
                        if nearness < best {
                            if unit != 0 {
                                found = get_from_memory(unit + unit_offsets::ENTITY_ID);
                            }
                            best = nearness;
                        }
                        break;
                    }
                    node = get_from_memory(node);
                }
            }
        }
        found
    }

    /// `ZTUnit::findNearbyNeedSatisfier` (`0x00613375`): the base unit never finds one. Writes "no building" and "no need"
    /// and returns the building out-pointer with its low byte cleared, as the vanilla `EAX` is.
    pub fn find_nearby_need_satisfier(need: *mut u8, building: *mut u32) -> u32 {
        save_to_memory(building as u32, 0u32);
        save_to_memory(need as u32, 0xffu8);
        building as u32 & 0xffff_ff00
    }
}

#[detour_mod]
mod satisfier_detours {
    use super::*;

    #[detour(GET_TARGET_UNIT)]
    unsafe extern "thiscall" fn get_target_unit(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_target_unit() as i32
    }

    #[detour(GET_PREDATOR_UNIT)]
    unsafe extern "thiscall" fn get_predator_unit(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_predator_unit() as i32
    }

    #[detour(RELEASE_PREDATOR)]
    unsafe extern "thiscall" fn release_predator(this: *const u32) {
        ZTUnit(this as u32).release_predator();
    }

    #[detour(IS_FUN_SATISFIER)]
    unsafe extern "stdcall" fn is_fun_satisfier(quadrants: i32, position: *const u32) -> u32 {
        u32::from(ZTUnit::is_fun_satisfier(quadrants as u32, position as *const i32))
    }

    #[detour(IS_NEED_OR_FUN_SATISFIER)]
    unsafe extern "thiscall" fn is_need_or_fun_satisfier(this: *const u32, quadrants: i32, position: *const u32) -> bool {
        ZTUnit(this as u32).is_need_or_fun_satisfier(quadrants as u32, position as *const i32)
    }

    #[detour(IN_FRIENDLY_CHASE)]
    unsafe extern "thiscall" fn in_friendly_chase(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).in_friendly_chase())
    }

    #[detour(AVOID_DONE)]
    unsafe extern "thiscall" fn avoid_done(this: *const u32, _other: *const u32) -> u32 {
        ZTUnit(this as u32).avoid_done()
    }

    #[detour(CHASE_DONE)]
    unsafe extern "thiscall" fn chase_done(this: *const u32, other: *const u32) -> bool {
        ZTUnit(this as u32).chase_done(other as u32)
    }

    #[detour(CAUGHT_DONE)]
    unsafe extern "thiscall" fn caught_done(this: *const u32) -> u8 {
        ZTUnit(this as u32).caught_done()
    }

    #[detour(CHECK_PREDATOR)]
    unsafe extern "thiscall" fn check_predator(this: *const u32, candidate: *const u32, chasing: bool) -> i8 {
        i8::from(ZTUnit(this as u32).check_predator(candidate as u32, chasing))
    }

    #[detour(FIND_NEAREST_SATISFIER)]
    unsafe extern "thiscall" fn find_nearest_satisfier(this: *const u32, tile: i32, name: *const i32, out: *const u32, range: i32, filter: i32) -> bool {
        ZTUnit(this as u32).find_nearest_satisfier(tile as u32, name as u32, out as u32, range, filter)
    }

    #[detour(FIND_ANY_SATISFIER)]
    unsafe extern "thiscall" fn find_any_satisfier(this: *const u32, tile: i32, name: *const i32, out: *const i32, range: i32, filter: i32) -> bool {
        ZTUnit(this as u32).find_any_satisfier(tile as u32, name as u32, out as u32, range, filter)
    }

    #[detour(F_WAS_CAUGHT)]
    unsafe extern "thiscall" fn f_was_caught(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).f_was_caught())
    }

    #[detour(FIND_PREDATOR)]
    unsafe extern "thiscall" fn find_predator(this: *const u32, flag: bool) -> i32 {
        ZTUnit(this as u32).find_predator(flag)
    }

    #[detour(FIND_NEARBY_NEED_SATISFIER)]
    unsafe extern "thiscall" fn find_nearby_need_satisfier(_this: *const u32, _tile: *const u32, need: *const u8, building: *const u32, _flag: i32) -> u32 {
        ZTUnit::find_nearby_need_satisfier(need as *mut u8, building as *mut u32)
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_target_unit(this: u32) -> u32 {
            unsafe { super::GET_TARGET_UNIT_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_predator_unit(this: u32) -> u32 {
            unsafe { super::GET_PREDATOR_UNIT_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn check_predator(this: u32, candidate: u32, chasing: bool) -> bool {
            super::low_byte_bool(unsafe { super::CHECK_PREDATOR_DETOUR.call(this as *const u32, candidate as *const u32, chasing) } as u8 as u32)
        }

        pub(crate) fn release_predator(this: u32) {
            unsafe { super::RELEASE_PREDATOR_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_fun_satisfier(quadrants: u32, position: *const i32) -> bool {
            super::low_byte_bool(unsafe { super::IS_FUN_SATISFIER_DETOUR.call(quadrants as i32, position as *const u32) })
        }

        pub(crate) fn is_need_or_fun_satisfier(this: u32, quadrants: u32, position: *const i32) -> bool {
            unsafe { super::IS_NEED_OR_FUN_SATISFIER_DETOUR.call(this as *const u32, quadrants as i32, position as *const u32) }
        }

        pub(crate) fn in_friendly_chase(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::IN_FRIENDLY_CHASE_DETOUR.call(this as *const u32) })
        }

        pub(crate) fn avoid_done(this: u32, other: u32) -> u32 {
            unsafe { super::AVOID_DONE_DETOUR.call(this as *const u32, other as *const u32) }
        }

        pub(crate) fn chase_done(this: u32, other: u32) -> bool {
            unsafe { super::CHASE_DONE_DETOUR.call(this as *const u32, other as *const u32) }
        }

        pub(crate) fn caught_done(this: u32) -> u8 {
            unsafe { super::CAUGHT_DONE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn find_nearest_satisfier(this: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> bool {
            unsafe { super::FIND_NEAREST_SATISFIER_DETOUR.call(this as *const u32, tile as i32, name as *const i32, out as *const u32, range, filter) }
        }

        pub(crate) fn find_any_satisfier(this: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> bool {
            unsafe { super::FIND_ANY_SATISFIER_DETOUR.call(this as *const u32, tile as i32, name as *const i32, out as *const i32, range, filter) }
        }

        pub(crate) fn f_was_caught(this: u32) -> bool {
            super::low_byte_bool(unsafe { super::F_WAS_CAUGHT_DETOUR.call(this as *const u32) })
        }

        pub(crate) fn find_predator(this: u32, flag: bool) -> i32 {
            unsafe { super::FIND_PREDATOR_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn find_nearby_need_satisfier(this: u32, need: *mut u8, building: *mut u32) -> u32 {
            unsafe { super::FIND_NEARBY_NEED_SATISFIER_DETOUR.call(this as *const u32, std::ptr::null(), need as *const u8, building as *const u32, 0) }
        }
    }
}

/// Installs the satisfier detours.
pub(super) fn init() {
    if let Err(e) = unsafe { satisfier_detours::init_detours() } {
        error!("Failed to initialise ztunit satisfier detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    satisfier_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use satisfier_detours::test_real;
