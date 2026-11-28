//! `ZTGuest`: the visitor entity, a `ZTUnit` subclass (`ZTUnit` -> `BFUnit` -> `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): guests are heap entities (`0x348` bytes, [`GUEST_SIZE`]) that `ZTGuide`, the
//! goal classes, `ZTAIMgr`, `ZTWorldMgr` and `ZooStatus` read by raw offset, so the object stays vanilla memory and
//! this module reads and writes it in place through [`ZTGuest`]. [`guest_offsets`] is the one layout table. Four
//! fields hold vanilla pool containers (`+0x260`, `+0x26c`, `+0x278` vectors and the `+0x288` string buffer); code that
//! allocates or frees them goes through the vanilla pool and never through Rust's allocator.
//!
//! The leaf accessors and predicates live here, together with the three megatile-reading methods -
//! `fCrowdDensityMegatile`/`fEstheticBonusMegatile`/`fStinkyMegatile`, detoured onto the reimplementations
//! below. These detours remove the last direct pointer-chasing reads of `ZTMegatileMgr`'s grid from
//! vanilla code (see `ztmegatilemgr.rs`'s module doc comment); `ZTMegatileMgr`'s struct layout must still
//! stay byte-exact, since the accessors this file calls
//! (`megatile()`/`guest_count()`/`stink()`/`category_value()`) still read vanilla's own live-owned memory,
//! not a migrated Rust store.
//!
//! No Windows decompile exists for any of the three, nor for their sole caller,
//! `ZTGuest::doEnvironmentEffectCheck` - all four addresses used here were confirmed by direct
//! disassembly of `zoo.dll`. See `private/docs/vtables/ZTGuest.md`'s "Non-virtual confirmed methods"
//! section for the full evidence chain (call-site uniqueness, the
//! `ZTGuestType::environment_effect_check`/`crowded_viewing_threshold`/`object_esthetic_threshold`/
//! `stink_threshold` field-offset matches, and the `GLOBAL_ZTMegatileMgr`/`MegatileRow`/`ZTMegatile`
//! stride matches).

use std::{ffi::c_void, mem};

use openzt_detour::generated::{
    bfentity::{GET_COLOR_REP_INFO as BFENTITY_GET_COLOR_REP_INFO, GET_TILE as BFENTITY_GET_TILE},
    ztguest::{
        CHECK_VIEWING_AREA, DIAGONAL_OK, DO_ANGRY_PRICE_CHANGE, DO_HAPPY_PRICE_CHANGE, DO_TOURGUIDE_BONUS, F_ANIMAL_HAPPY, F_CROWD_DENSITY_MEGATILE,
        F_ESTHETIC_BONUS_MEGATILE, F_STINKY_MEGATILE, F_TOUR_GUIDE_BONUS, GET_BATHROOM_PCT, GET_COLOR_REP_INFO, GET_ENERGY_PCT, GET_HABITAT_FROM_VA,
        GET_HAPPY_PCT, GET_HUNGER_PCT, GET_RANDOM_OK, GET_RETURN_BUILDING_NAME_ID, GET_RETURN_BUILDING_STRING, GET_THIRST_PCT, GET_VIEWING_AREA,
        HAS_PRESSING_NEED, IS_DOING_SOMETHING, IS_RETURNING, IS_WATER_RECOVERING, IS_WATER_TRAVELING, RANDOM_DONE, SET_RETURN_BUILDING_NAME_ID, SET_SPEED,
        SET_WATER_RECOVERING, ABORT_GOAL, ADD_TO_MAP, CLEAR_GOAL, FIND_PLACE_TO_SIT, FIND_TRASH_CAN, INIT, INIT_STATUS_VARS, LOAD, REMOVE_FROM_MAP, RESET_AI, SAVE,
    },
    poolalloc::{DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
    msvc_std::RAND,
    msvc_std_basic_string::{ASSIGN_RANGE, ERASE_RANGE},
    ztgamemgr::{GET_DATE as GAMEMGR_GET_DATE, IS_GAME_DATE as GAMEMGR_IS_GAME_DATE, IS_REAL_WORLD_DATE as GAMEMGR_IS_REAL_WORLD_DATE},
    ztui_multiguest::{ADD_ENTITY as MULTIGUEST_ADD_ENTITY, REMOVE_ENTITY as MULTIGUEST_REMOVE_ENTITY},
    ztunit::{
        INIT as ZTUNIT_INIT, },
    ztworldmgr::GET_NEED_INDEX as ZTWORLDMGR_GET_NEED_INDEX,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

mod checks;
mod goals;
mod inventory;
mod movement;
mod sites;
mod viewing;

use crate::ztunit::ZTUnit;
use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, viewing_area::recalculate_characteristics as recalculate_viewing_area, ZTHabitat},
    ztguide::ZTGuide,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, read_field, unit_offsets, write_field},
    ztmapview::BFTile,
    ztmegatilemgr::{entity_type_matches, ZTMegatile},
    ztworldmgr::BFEntity,
    vanilla_list,
    vanilla_vector::{push_word, push_words},
};

/// Overrides the live tests install on the viewing-area check's collaborators.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod check_overrides {
    use std::sync::Mutex;

    pub type FindViewingArea = fn(this: u32) -> Option<bool>;
    pub type AnimalHappy = fn(this: u32) -> Option<i32>;
    /// One of the viewing-area statistics queries (`fNumAnimals`, `fSickAnimals`, `fNumSickAnimals`,
    /// `fDifferentSpecies`).
    pub type AreaQuery = fn(this: u32) -> Option<u32>;
    pub type FindAnimal = fn(this: u32, low: i32, high: i32) -> Option<u32>;
    pub type CrowdDensity = fn(this: u32) -> Option<i64>;

    pub static FIND_VIEWING_AREA: Mutex<Option<FindViewingArea>> = Mutex::new(None);
    pub static ANIMAL_HAPPY: Mutex<Option<AnimalHappy>> = Mutex::new(None);
    pub static NUM_ANIMALS: Mutex<Option<AreaQuery>> = Mutex::new(None);
    pub static SICK_ANIMALS: Mutex<Option<AreaQuery>> = Mutex::new(None);
    pub static NUM_SICK_ANIMALS: Mutex<Option<AreaQuery>> = Mutex::new(None);
    pub static DIFFERENT_SPECIES: Mutex<Option<AreaQuery>> = Mutex::new(None);
    pub static FIND_ANIMAL: Mutex<Option<FindAnimal>> = Mutex::new(None);
    pub static CROWD_DENSITY: Mutex<Option<CrowdDensity>> = Mutex::new(None);
}

/// `ZTGuestType::createEntity` allocates this many bytes.
pub const GUEST_SIZE: u32 = 0x348;

/// `CAST_ZTGuest` (`0x00638700`) as an RVA: the class tag `isCastClass` takes to test for a guest type.
pub const GUEST_TYPE_TAG_RVA: u32 = 0x0023_8700;

/// `ZTGuest` fields past the `ZTUnit` prefix (`0x000`-`0x25f`); inherited offsets are in `ztkeeper::unit_offsets`.
pub mod guest_offsets {
    /// `vector<12-byte record>` (begin, end, cap) of visited sites. Pool-allocated.
    pub const VISITED_SITES: u32 = 0x260;
    /// `vector<u32>` (begin, end, cap) of carried-item slots, indexed by item kind. Pool-allocated.
    pub const ITEMS: u32 = 0x26c;
    /// `vector<u32>` (begin, end, cap) saved and loaded but read by no other method. Pool-allocated.
    pub const SAVED_IDS: u32 = 0x278;
    /// Packed word: the low 19 bits are the return building's name id, bit 19 (`0x80000`) is the water-recovering
    /// flag. Saved.
    pub const RETURN_BUILDING_BITS: u32 = 0x284;
    /// The return-building string buffer (begin, end, cap). Pool-allocated.
    pub const RETURN_BUILDING_STRING: u32 = 0x288;
    /// Eight bytes that are only saved and loaded.
    pub const FIELD_0X298: u32 = 0x298;
    /// The in-game date (a `FILETIME`). `ZTWorldMgr::resetGuestsWithoutRemoving` writes it by raw offset.
    pub const DATE: u32 = 0x2a0;
    pub const HAPPINESS: u32 = 0x2a8;
    pub const HUNGER: u32 = 0x2b0;
    pub const THIRST: u32 = 0x2b8;
    pub const ENERGY: u32 = 0x2c0;
    pub const BATHROOM: u32 = 0x2c8;
    /// The entity id of the guide the guest follows (`0` when none).
    pub const GUIDE_ID: u32 = 0x2e4;
    /// Check-period countdown `doConditionals` reloads.
    pub const CHECK_COUNTDOWN: u32 = 0x314;
    /// Dword (`0` or `1`) `ZTGuide::triggerNearbyGuests` compares with the guide type's name `"f"`/`"m"`.
    pub const STATE: u32 = 0x320;
    /// The two current viewing-area ids.
    pub const VIEWING_AREA_ID: u32 = 0x328;
    pub const VIEWING_AREA: u32 = 0x32c;
    /// Byte set while a gawk target or flag is active; `isDoingSomething` and `getColorRepInfo` test it.
    pub const FLAG_0X336: u32 = 0x336;
    /// Byte `ZTGuide::releaseGuests` clears when it lets a guest go.
    pub const FOLLOWING_FLAG: u32 = 0x337;
    /// Byte `isDoingSomething` tests.
    pub const FLAG_0X339: u32 = 0x339;
    /// Byte `randomDone` sets and `getRandomOK` tests.
    pub const RANDOM_DONE: u32 = 0x33e;
}

/// `ZTGuestType` fields.
mod type_offsets {
    pub const HAPPY_HIGH: u32 = 0x1e4;
    pub const HAPPY_LOW: u32 = 0x1ec;
    pub const HUNGER_HIGH: u32 = 0x1f0;
    pub const HUNGER_LOW: u32 = 0x1f4;
    pub const THIRST_HIGH: u32 = 0x1f8;
    pub const THIRST_LOW: u32 = 0x1fc;
    pub const ENERGY_HIGH: u32 = 0x200;
    pub const ENERGY_LOW: u32 = 0x204;
    pub const BATHROOM_HIGH: u32 = 0x208;
    pub const BATHROOM_LOW: u32 = 0x20c;
    /// Thresholds `hasPressingNeed` compares each need level with.
    pub const ENERGY_PRESSING: u32 = 0x214;
    pub const HUNGER_PRESSING: u32 = 0x220;
    pub const THIRST_PRESSING: u32 = 0x230;
    pub const BATHROOM_PRESSING: u32 = 0x23c;
    /// Mood values `doHappyPriceChange`/`doAngryPriceChange` pass to the guest's slot `+0x1fc`.
    pub const HAPPY_PRICE: u32 = 0x244;
    pub const ANGRY_PRICE: u32 = 0x248;
    /// Colour-rep value `getColorRepInfo` copies into the entity's colour-rep block.
    pub const COLOR_REP: u32 = 0x2cc;
}

/// `ZTViewingArea` fields.
mod viewing_area_offsets {
    /// The habitat the viewing area belongs to.
    pub const HABITAT: u32 = 0;
    pub const CHARACTERISTICS_DIRTY: u32 = 0x4c;
    pub const GUIDE_VALUE: u32 = 0x58;
}

/// `BFTile` byte `+0x85`: bit `0x20` allows diagonal steps.
const TILE_DIAGONAL_OK_OFFSET: u32 = 0x85;
const TILE_DIAGONAL_OK_BIT: u8 = 0x20;

/// `ZTUnit` fields `isDoingSomething`/`setSpeed` read.
mod unit_fields {
    pub const FLAG_0X25C: u32 = 0x25c;
    pub const FLAG_0X24C: u32 = 0x24c;
    /// Byte added to the requested speed in the variable-speed branch of `setSpeed`.
    pub const SPEED_BASE: u32 = 0x204;
}

/// Vtable slots dispatched on the guest.
mod slots {
    /// The two predicates `isWaterTraveling` ORs and `isDoingSomething` tests the first of.
    pub const WATER_TRAVEL_A: u32 = 0x264;
    pub const WATER_TRAVEL_B: u32 = 0x270;
    /// Takes a pointer to a six-dword record; the price-change and tour-guide-bonus methods fill it.
    pub const MOOD_CHANGE: u32 = 0x1fc;
}

/// Globals, as RVAs from the `zoo.exe` base.
mod globals_rva {
    /// `DAT_00630d68`: the multiplier `setSpeed` applies when the guest is tired.
    pub const SPEED_TIRED_FACTOR: u32 = 0x0023_0d68;
    /// `DAT_00635828`: the multiplier `setSpeed` applies when the guest is energetic.
    pub const SPEED_ENERGETIC_FACTOR: u32 = 0x0023_5828;
}

/// `ZTAIMgr` field: variable unit speeds on.
const AIMGR_VARIABLE_SPEEDS_OFFSET: u32 = 0x11c;
/// Energy levels `setSpeed` splits on.
const SPEED_ENERGETIC_FROM: i32 = 0x4b;
const SPEED_TIRED_UP_TO: i32 = 0x13;

/// Vanilla's `(int)(longlong)x` on a double: truncate to 64 bits, keep the low dword. A value outside the 64-bit
/// range (or NaN) gives the x87 "integer indefinite", whose low dword is `0`.
fn ftol_low32_f64(value: f64) -> i32 {
    let truncated = value.trunc();
    const TWO_POW_63: f64 = 9_223_372_036_854_775_808.0;
    if !(-TWO_POW_63..TWO_POW_63).contains(&truncated) {
        return 0;
    }
    (truncated as i64) as i32
}

/// A guest in vanilla memory. All reads and writes are volatile through the raw address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTGuest(pub u32);

impl ZTGuest {
    pub(super) fn read<T: Copy>(self, offset: u32) -> T {
        get_from_memory(self.0 + offset)
    }

    /// The guest's `ZTGuestType` pointer, or `0` when the entity type is missing or not a guest type.
    fn guest_type(self) -> u32 {
        if unsafe { entity_type_matches(self.0, GUEST_TYPE_TAG_RVA) } {
            self.read(unit_offsets::ENTITY_TYPE)
        } else {
            0
        }
    }

    fn return_building_bits(self) -> u32 {
        self.read(guest_offsets::RETURN_BUILDING_BITS)
    }

    /// `ZTGuest::isReturning` (`0x0043a837`): whether a return building name id is set.
    pub fn is_returning(self) -> bool {
        self.return_building_bits() & RETURN_BUILDING_NAME_MASK != 0
    }

    /// `ZTGuest::isWaterRecovering` (`0x0043aa0a`): bit 19 of `+0x284`.
    pub fn is_water_recovering(self) -> bool {
        self.return_building_bits() & WATER_RECOVERING_BIT != 0
    }

    /// `ZTGuest::isWaterTraveling` (`0x0043a9e1`): either of two virtual predicates (`+0x264`, `+0x270`) holds.
    pub fn is_water_traveling(self) -> bool {
        low_byte_bool(call_slot_0(self.0, slots::WATER_TRAVEL_A)) || low_byte_bool(call_slot_0(self.0, slots::WATER_TRAVEL_B))
    }

    /// `ZTGuest::setWaterRecovering` (`0x0042a7ed`): sets bit 19 of `+0x284`; clearing it also clears every bit above
    /// the name id.
    pub fn set_water_recovering(self, recovering: bool) {
        let bits = self.return_building_bits();
        let bits = if recovering { bits | WATER_RECOVERING_BIT } else { bits & RETURN_BUILDING_NAME_MASK };
        save_to_memory(self.0 + guest_offsets::RETURN_BUILDING_BITS, bits);
    }

    /// `ZTGuest::setReturnBuildingNameID` (`0x00499266`): stores the whole word, so the water flag is cleared too.
    pub fn set_return_building_name_id(self, id: i32) {
        save_to_memory(self.0 + guest_offsets::RETURN_BUILDING_BITS, id);
    }

    /// `ZTGuest::getReturnBuildingNameID` (`0x004992c3`).
    pub fn return_building_name_id(self) -> u32 {
        self.return_building_bits() & RETURN_BUILDING_NAME_MASK
    }

    /// `ZTGuest::getReturnBuildingString` (`0x0059fe75`): the address of the string buffer.
    pub fn return_building_string(self) -> u32 {
        self.0 + guest_offsets::RETURN_BUILDING_STRING
    }

    /// `ZTGuest::diagonalOK` (`0x0043a849`): bit `0x20` of the guest's tile's byte `+0x85`; `false` off the map.
    pub fn diagonal_ok(self) -> bool {
        let tile = unsafe { BFENTITY_GET_TILE.original()(self.0 as *const u32) } as u32;
        tile != 0 && get_from_memory::<u8>(tile + TILE_DIAGONAL_OK_OFFSET) & TILE_DIAGONAL_OK_BIT != 0
    }

    /// `ZTGuest::randomDone` (`0x004225b5`).
    pub fn random_done(self) {
        save_to_memory(self.0 + guest_offsets::RANDOM_DONE, 1u8);
    }

    /// `ZTGuest::getRandomOK` (`0x00422594`): `ZTUnit::getRandomOK` holds and `randomDone` has not run.
    pub fn random_ok(self) -> bool {
        ZTUnit(self.0).get_random_ok() && self.read::<u8>(guest_offsets::RANDOM_DONE) == 0
    }

    /// `ZTGuest::getViewingArea` (`0x004232f4`): runs `checkViewingArea` on the current id (its result is unused,
    /// the call refreshes `+0x32c`), then returns the viewing area pointer.
    pub fn viewing_area(self) -> u32 {
        // `.hooked()`: what vanilla's direct call runs, so a later port of `checkViewingArea` is picked up.
        unsafe { CHECK_VIEWING_AREA.hooked()(self.0 as *const u32, self.read(guest_offsets::VIEWING_AREA_ID)) };
        self.read(guest_offsets::VIEWING_AREA)
    }

    /// `ZTGuest::getHabitatFromVA` (`0x0043ed2c`): the habitat of the viewing area, `0` without one.
    pub fn habitat_from_va(self) -> u32 {
        let viewing_area = self.viewing_area();
        if viewing_area == 0 {
            0
        } else {
            get_from_memory(viewing_area + viewing_area_offsets::HABITAT)
        }
    }

    /// `ZTGuest::getColorRepInfo` (`0x0043408c`): `BFEntity::getColorRepInfo`; with `+0x336` set, the colour-rep
    /// block's `+0xc` is overwritten from the guest type's `+0x2cc`. Vanilla faults on a missing guest type there.
    pub fn color_rep_info(self) -> i32 {
        let info = unsafe { BFENTITY_GET_COLOR_REP_INFO.original()(self.0 as *const u32) };
        if self.read::<u8>(guest_offsets::FLAG_0X336) != 0 {
            let guest_type = self.guest_type();
            let block: u32 = get_from_memory(info as u32 + 0x10);
            if guest_type != 0 && block != 0 {
                save_to_memory(block + 0xc, get_from_memory::<i32>(guest_type + type_offsets::COLOR_REP));
            }
        }
        info
    }

    /// `(level - low) * 100 / (high - low)` against the guest type; `0` without a guest type, and for an empty range,
    /// where vanilla would trap on the division.
    fn percent(self, level_offset: u32, high_offset: u32, low_offset: u32) -> i32 {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return 0;
        }
        let low: i32 = get_from_memory(guest_type + low_offset);
        let high: i32 = get_from_memory(guest_type + high_offset);
        let level: i32 = self.read(level_offset);
        let range = high.wrapping_sub(low);
        if range == 0 {
            return 0;
        }
        level.wrapping_sub(low).wrapping_mul(100).wrapping_div(range)
    }

    /// `ZTGuest::getHappyPct` (`0x0046a606`).
    pub fn happy_pct(self) -> i32 {
        self.percent(guest_offsets::HAPPINESS, type_offsets::HAPPY_HIGH, type_offsets::HAPPY_LOW)
    }

    /// `ZTGuest::getHungerPct` (`0x0046a6a4`).
    pub fn hunger_pct(self) -> i32 {
        self.percent(guest_offsets::HUNGER, type_offsets::HUNGER_HIGH, type_offsets::HUNGER_LOW)
    }

    /// `ZTGuest::getThirstPct` (`0x0046a655`).
    pub fn thirst_pct(self) -> i32 {
        self.percent(guest_offsets::THIRST, type_offsets::THIRST_HIGH, type_offsets::THIRST_LOW)
    }

    /// `ZTGuest::getEnergyPct` (`0x0046a6f3`).
    pub fn energy_pct(self) -> i32 {
        self.percent(guest_offsets::ENERGY, type_offsets::ENERGY_HIGH, type_offsets::ENERGY_LOW)
    }

    /// `ZTGuest::getBathroomPct` (`0x0046a742`).
    pub fn bathroom_pct(self) -> i32 {
        self.percent(guest_offsets::BATHROOM, type_offsets::BATHROOM_HIGH, type_offsets::BATHROOM_LOW)
    }

    /// `ZTGuest::hasPressingNeed` (`0x0058d5eb`): any of hunger, thirst, energy or bathroom has reached the guest
    /// type's pressing threshold. `false` without a guest type, where vanilla faults.
    pub fn has_pressing_need(self) -> bool {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return false;
        }
        [
            (guest_offsets::HUNGER, type_offsets::HUNGER_PRESSING),
            (guest_offsets::THIRST, type_offsets::THIRST_PRESSING),
            (guest_offsets::ENERGY, type_offsets::ENERGY_PRESSING),
            (guest_offsets::BATHROOM, type_offsets::BATHROOM_PRESSING),
        ]
        .into_iter()
        .any(|(level, threshold)| self.read::<i32>(level) >= get_from_memory::<i32>(guest_type + threshold))
    }

    /// `ZTGuest::isDoingSomething` (`0x0058d9c0`): busy flags (`+0x25c`, `+0x336`, `+0x24c`, `+0x339`), the busy goal
    /// ids (`0x67`-`0x6d`, `0x71`, `0x75`, `0x79`) and the virtual predicate at `+0x264`, tested in vanilla's order.
    pub fn is_doing_something(self) -> bool {
        let goal: u32 = self.read(unit_offsets::GOAL_ID);
        let flag = |offset: u32| self.read::<u8>(offset) != 0;
        flag(unit_fields::FLAG_0X25C)
            || goal == 0x67
            || (0x68..=0x6a).contains(&goal)
            || flag(guest_offsets::FLAG_0X336)
            || goal == 0x6b
            || goal == 0x75
            || flag(unit_fields::FLAG_0X24C)
            || goal == 0x71
            || flag(guest_offsets::FLAG_0X339)
            || goal == 0x79
            || goal == 0x6c
            || low_byte_bool(call_slot_0(self.0, slots::WATER_TRAVEL_A))
            || goal == 0x6d
    }

    /// `ZTGuest::fAnimalHappy` (`0x00423354`): the average animal happiness of the viewing area's habitat, `0`
    /// without a viewing area.
    pub fn animal_happy(self) -> i32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = check_overrides::ANIMAL_HAPPY.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        let habitat = self.habitat_from_va();
        if habitat == 0 {
            return 0;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_avg_animal_happiness()
    }

    /// `ZTGuest::fTourGuideBonus` (`0x00426236`): the viewing area's guide value (recalculated when dirty), `0`
    /// without a viewing area.
    pub fn tour_guide_bonus(self) -> u32 {
        let viewing_area = self.viewing_area();
        if viewing_area == 0 {
            return 0;
        }
        if get_from_memory::<u8>(viewing_area + viewing_area_offsets::CHARACTERISTICS_DIRTY) != 0 {
            recalculate_viewing_area(viewing_area);
        }
        get_from_memory(viewing_area + viewing_area_offsets::GUIDE_VALUE)
    }

    /// Calls the guest's virtual mood-change slot (`+0x1fc`) with a zeroed six-dword record whose first two dwords
    /// are `first` and `second`.
    fn mood_change(self, first: u32, second: u32) {
        let record = [first, second, 0, 0, 0, 0];
        call_slot_1(self.0, slots::MOOD_CHANGE, record.as_ptr() as u32);
    }

    /// `ZTGuest::doAngryPriceChange` (`0x00533480`): the guest type's `+0x248` in both leading dwords.
    pub fn do_angry_price_change(self) {
        let value = self.type_value(type_offsets::ANGRY_PRICE);
        self.mood_change(value, value);
    }

    /// `ZTGuest::doHappyPriceChange` (`0x00619245`): the guest type's `+0x244` in both leading dwords.
    pub fn do_happy_price_change(self) {
        let value = self.type_value(type_offsets::HAPPY_PRICE);
        self.mood_change(value, value);
    }

    /// `ZTGuest::doTourguideBonus` (`0x0058a8dc`): `fTourGuideBonus` in both leading dwords.
    pub fn do_tourguide_bonus(self) {
        let first = self.tour_guide_bonus();
        let second = self.tour_guide_bonus();
        self.mood_change(first, second);
    }

    /// A dword of the guest type, `0` without a guest type (vanilla faults).
    fn type_value(self, offset: u32) -> u32 {
        let guest_type = self.guest_type();
        if guest_type == 0 {
            0
        } else {
            get_from_memory(guest_type + offset)
        }
    }

    /// `ZTGuest::setSpeed` (`0x0042250a`): with variable unit speeds off the speed is stored as given. With them on
    /// it is `speed + byte(+0x204) - 5`, scaled by a double (read live from the binary) when the energy level is
    /// at or above `0x4b` or at or below `0x13`; the product goes through the 64-bit truncation vanilla's `FISTP`
    /// does, taking the unsigned dword as the multiplicand.
    pub fn set_speed(self, speed: u32) {
        let aimgr = globals().ztaimgr_ptr() as u32;
        let variable = aimgr != 0 && get_from_memory::<u32>(aimgr + AIMGR_VARIABLE_SPEEDS_OFFSET) != 0;
        let value = if variable {
            let base = speed.wrapping_add(self.read::<u8>(unit_fields::SPEED_BASE) as u32).wrapping_sub(5);
            let energy: i32 = self.read(guest_offsets::ENERGY);
            let factor_rva = if energy >= SPEED_ENERGETIC_FROM {
                Some(globals_rva::SPEED_ENERGETIC_FACTOR)
            } else if energy <= SPEED_TIRED_UP_TO {
                Some(globals_rva::SPEED_TIRED_FACTOR)
            } else {
                None
            };
            match factor_rva {
                Some(rva) => {
                    let factor: f64 = get_from_memory(get_module_base("zoo.exe") as u32 + rva);
                    ftol_low32_f64(base as f64 * factor) as u32
                }
                None => base,
            }
        } else {
            speed
        };
        save_to_memory(self.0 + unit_offsets::SPEED, value);
    }
}

/// `ZTGuest` fields past the leaf accessors' set: lifecycle state.
mod lifecycle_offsets {
    /// `ZTUnit` entity id.
    pub const ENTITY_ID: u32 = 0x124;
    /// `vector<char>` (begin, end) cleared by `clearGoal`.
    pub const SCRATCH_BEGIN: u32 = 0x1c8;
    pub const SCRATCH_END: u32 = 0x1cc;
    /// Byte `clearGoal` sets.
    pub const FLAG_0X1D4: u32 = 0x1d4;
    /// Two bytes `clearGoal` fills with `0xff`; `+0x24e` also holds the need index `abortGoal` tests.
    pub const NEED_INDEX_A: u32 = 0x24d;
    pub const NEED_INDEX: u32 = 0x24e;
    /// Dword `clearGoal` and `init` set from `DAT_006393ec`.
    pub const DEFAULT_0X230: u32 = 0x230;
    /// Bytes `addToMap`/`removeFromMap` test: set for a guest that is not tracked by the multi-guest panel.
    pub const UNTRACKED_A: u32 = 0x142;
    pub const UNTRACKED_B: u32 = 0x143;
    /// Byte set by `init`.
    pub const SPEED_JITTER: u32 = 0x204;
    /// Building grid pointer and the three-dword position record `abortGoal` hands the satisfier checks.
    pub const BUILDING_GRID: u32 = 0x244;
    pub const POSITION_RECORD: u32 = 0x214;
    /// Flags `clearGoal` clears.
    pub const FOLLOWING_FLAG_B: u32 = 0x338;
    pub const FLAG_0X33F: u32 = 0x33f;
}

/// Globals, as RVAs from the `zoo.exe` base.
mod lifecycle_globals {
    /// `GLOBAL_ZTWorldMgr`.
    pub const WORLD_MGR: u32 = 0x0023_8040;
    /// `DAT_006393ec`, a dword copied into `+0x230`.
    pub const DEFAULT_0X230: u32 = 0x0023_93ec;
    /// `DAT_00639118`: the `list<uint>` of guests the multi-guest panel tracks. The global itself holds the list's sentinel
    /// node pointer (vanilla reads it once, `MOV ECX, [DAT_00639118]`).
    pub const MULTIGUEST_LIST: u32 = 0x0023_9118;
    /// `DAT_0063c518` and `DAT_0063c508`: the need names `abortGoal` resolves with `getNeedIndex`.
    pub const NEED_NAME_A: u32 = 0x0023_c518;
    pub const NEED_NAME_B: u32 = 0x0023_c508;
}

/// `FUN_00618e3d` and `FUN_00618eb3` (`cdecl(grid, *const [i32; 3]) -> AL`): whether the building on the grid cell the
/// position record names satisfies need `NEED_NAME_A` / `NEED_NAME_B`. `generated.rs` has no entry for either.
const SATISFIER_CHECK_A: u32 = 0x0061_8e3d;
const SATISFIER_CHECK_B: u32 = 0x0061_8eb3;

/// Vtable slots dispatched on the guest or the world.
mod lifecycle_slots {
    pub const CLEAR_GOAL: u32 = 0x174;
    pub const GOAL_SET: u32 = 0x1d8;
    pub const ABORT_PREDICATE: u32 = 0x22c;
    pub const ABORT_ACTION: u32 = 0x2c0;
    pub const RESET_AI_TAIL: u32 = 0x294;
    /// `BFWorldMgr::getEntity(id, flag)`.
    pub const WORLD_GET_ENTITY: u32 = 0x24;
}

/// Goal `abortGoal` hands over to after finding a seat or a trash can.
const GOAL_SIT_OR_TRASH: u32 = 0x66;

impl ZTGuest {
    fn write<T: Copy>(self, offset: u32, value: T) {
        save_to_memory(self.0 + offset, value);
    }

    /// The `ZTGuide` entity this guest follows (`+0x2e4`), or `0` when it follows none, the id resolves to nothing or
    /// the entity is not a guide.
    fn guide(self) -> u32 {
        let id: u32 = self.read(guest_offsets::GUIDE_ID);
        if id == 0 {
            return 0;
        }
        let world: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + lifecycle_globals::WORLD_MGR);
        let slot = get_from_memory::<u32>(get_from_memory::<u32>(world) + lifecycle_slots::WORLD_GET_ENTITY);
        let get_entity = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(slot) };
        let entity = get_entity(world, id, 1);
        if entity != 0 && unsafe { entity_type_matches(entity, crate::ztguide::GUIDE_TYPE_TAG_RVA) } {
            entity
        } else {
            0
        }
    }

    /// `ZTGuest::clearGoal` (`0x00422763`, vtable `+0x174`): `ZTUnit::clearGoal`; `+0x24d`/`+0x24e` become `0xff`,
    /// `+0x230` the default dword, the scratch string at `+0x1c8` is emptied, `+0x1d4` is set and `+0x337`, `+0x338`,
    /// `+0x33f` cleared. A guest following a guide is removed from the guide's follower list and forgets it (`+0x2e4`
    /// is left alone when the guide cannot be found).
    pub fn clear_goal(self) {
        ZTUnit(self.0).clear_goal();
        self.write(lifecycle_offsets::NEED_INDEX_A, 0xffu8);
        self.write(lifecycle_offsets::NEED_INDEX, 0xffu8);
        self.write(lifecycle_offsets::DEFAULT_0X230, get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + lifecycle_globals::DEFAULT_0X230));
        let begin: u32 = self.read(lifecycle_offsets::SCRATCH_BEGIN);
        if begin != self.read::<u32>(lifecycle_offsets::SCRATCH_END) {
            save_to_memory(begin, 0u8);
            self.write(lifecycle_offsets::SCRATCH_END, begin);
        }
        self.write(lifecycle_offsets::FLAG_0X1D4, 1u8);
        self.write(lifecycle_offsets::FOLLOWING_FLAG_B, 0u8);
        self.write(guest_offsets::FOLLOWING_FLAG, 0u8);
        self.write(lifecycle_offsets::FLAG_0X33F, 0u8);
        let guide = self.guide();
        if guide != 0 {
            ZTGuide(guide).remove_guest(self.read(lifecycle_offsets::ENTITY_ID));
            self.write(guest_offsets::GUIDE_ID, 0u32);
        }
    }

    fn multiguest_list(self) -> u32 {
        get_from_memory(get_module_base("zoo.exe") as u32 + lifecycle_globals::MULTIGUEST_LIST)
    }

    fn tracked_by_multiguest(self) -> bool {
        self.read::<u8>(lifecycle_offsets::UNTRACKED_B) == 0 && self.read::<u8>(lifecycle_offsets::UNTRACKED_A) == 0
    }

    /// `ZTGuest::addToMap` (`0x00412ba7`): `ZTUnit::addToMap`; on success a guest not excluded by `+0x142`/`+0x143` is
    /// appended to the multi-guest panel's list and handed to `ZTUI::multiguest::addEntity`. Returns `ZTUnit`'s result.
    pub fn add_to_map(self) -> bool {
        let added = ZTUnit(self.0).add_to_map();
        if added && self.tracked_by_multiguest() {
            vanilla_list::push_back(self.multiguest_list(), self.0);
            unsafe { MULTIGUEST_ADD_ENTITY.original()(self.0 as *const u32) };
        }
        added
    }

    /// `ZTGuest::removeFromMap` (`0x0042789a`): a guest not excluded by `+0x142`/`+0x143` leaves the multi-guest panel
    /// (`removeEntity(this, false)`), every node holding it is unlinked from the panel's list and pushed on the 12-byte
    /// freelist, and its thoughts are dropped. A guest that follows a guide is removed from that guide's list (`+0x2e4`
    /// stays). Ends with `ZTUnit::removeFromMap`.
    pub fn remove_from_map(self, param: bool) {
        if self.tracked_by_multiguest() {
            unsafe { MULTIGUEST_REMOVE_ENTITY.original()(self.0 as *const u32, false) };
            vanilla_list::remove_matching(self.multiguest_list(), self.0);
            globals().ztthoughtmgr().remove_thoughts_by_thinker(self.0);
        }
        let guide = self.guide();
        if guide != 0 {
            ZTGuide(guide).remove_guest(self.read(lifecycle_offsets::ENTITY_ID));
        }
        ZTUnit(self.0).remove_from_map(param);
    }

    /// `ZTGuest::resetAI` (`0x005ae78a`): `ZTUnit::resetAI`, then the virtual `+0x294` with `0`.
    pub fn reset_ai(self) {
        ZTUnit(self.0).reset_ai();
        call_slot_1(self.0, lifecycle_slots::RESET_AI_TAIL, 0);
    }

    /// `ZTGuest::init` (`0x004f6082`): `ZTUnit::init`, then one shared-RNG draw modulo 11 into the byte at `+0x204`,
    /// viewing area cleared (`+0x32c` zero, `+0x328` `-1`) and `+0x230` set to the default dword. Returns what vanilla
    /// leaves in `EAX`: the default dword for a typed guest, the RNG quotient otherwise.
    pub fn init(self, param: bool) -> u32 {
        unsafe { ZTUNIT_INIT.original()(self.0 as *const u32, param) };
        let draw = next_game_random();
        self.write(guest_offsets::VIEWING_AREA, 0u32);
        self.write(guest_offsets::VIEWING_AREA_ID, -1i32);
        self.write(lifecycle_offsets::SPEED_JITTER, (draw % 11) as u8);
        let default: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + lifecycle_globals::DEFAULT_0X230);
        self.write(lifecycle_offsets::DEFAULT_0X230, default);
        if self.read::<u32>(unit_offsets::ENTITY_TYPE) != 0 {
            default
        } else {
            draw / 11
        }
    }

    /// Whether the building the grid cell of the position record names satisfies the need (`check`'s address).
    fn satisfier_check(self, check: u32) -> bool {
        let record: [i32; 3] = [self.read(lifecycle_offsets::POSITION_RECORD), self.read(lifecycle_offsets::POSITION_RECORD + 4), self.read(lifecycle_offsets::POSITION_RECORD + 8)];
        let function = unsafe { mem::transmute::<u32, extern "cdecl" fn(u32, *const i32) -> u32>(check) };
        low_byte_bool(function(self.read(lifecycle_offsets::BUILDING_GRID), record.as_ptr()))
    }

    fn need_index(name_rva: u32) -> u8 {
        unsafe { ZTWORLDMGR_GET_NEED_INDEX.hooked()((get_module_base("zoo.exe") as u32 + name_rva) as *const i8) as u8 }
    }

    fn has_building_use_goal(self) -> bool {
        ZTUnit(self.0).has_building_use_goal()
    }

    /// `ZTGuest::abortGoal` (`0x005ad820`). With `param` set and the guest's need index (`+0x24e`) matching the first
    /// need (`NEED_NAME_A`), no building-use goal and a satisfier on its grid cell, the goal is cleared and, when
    /// `findPlaceToSit(0)` finds a seat, goal `0x66` takes over and the method returns. Otherwise the second need
    /// (`NEED_NAME_B`) is tried the same way with `findTrashCan(0)`. The common tail runs the virtual predicate
    /// `+0x22c` (acting through `+0x2c0` when it holds), clears the goal virtually and sets the `randomDone` byte.
    pub fn abort_goal(self, param: bool) {
        if param {
            let matches = |name_rva: u32| self.read::<u8>(lifecycle_offsets::NEED_INDEX) == Self::need_index(name_rva);
            let mut try_second = true;
            if matches(lifecycle_globals::NEED_NAME_A) && !self.has_building_use_goal() && self.satisfier_check(SATISFIER_CHECK_A) {
                call_slot_0(self.0, lifecycle_slots::CLEAR_GOAL);
                if unsafe { FIND_PLACE_TO_SIT.hooked()(self.0 as *const u32, std::ptr::null()) } {
                    call_slot_2(self.0, lifecycle_slots::GOAL_SET, GOAL_SIT_OR_TRASH, 4);
                    return;
                }
                try_second = false;
            }
            if try_second && matches(lifecycle_globals::NEED_NAME_B) && !self.has_building_use_goal() && self.satisfier_check(SATISFIER_CHECK_B) {
                call_slot_0(self.0, lifecycle_slots::CLEAR_GOAL);
                if unsafe { FIND_TRASH_CAN.hooked()(self.0 as *const u32, 0) } {
                    call_slot_2(self.0, lifecycle_slots::GOAL_SET, GOAL_SIT_OR_TRASH, 4);
                    return;
                }
            }
        }
        if low_byte_bool(call_slot_0(self.0, lifecycle_slots::ABORT_PREDICATE)) {
            call_slot_0(self.0, lifecycle_slots::ABORT_ACTION);
        }
        call_slot_0(self.0, lifecycle_slots::CLEAR_GOAL);
        self.write(guest_offsets::RANDOM_DONE, 1u8);
    }

    /// `ZTGuest::save` (`0x0047873e`): the guest's fields in vanilla's order (needs, `+0x336`, the per-need values,
    /// the `+0x320` and `+0x324` words, the check slots, the carried-item and saved-id vectors as count plus dwords,
    /// `+0x284`, the return-building string (length then bytes; a length of `0x1000` or more fails the save and
    /// writes neither), the visited-site records as count plus two dwords each, then the remaining words and flags),
    /// then `ZTUnit::save`. Every write runs after a failure; the result is the conjunction.
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = true;
        macro_rules! put {
            ($address:expr, $size:expr) => {
                ok &= write_field($address, $size, file)
            };
        }
        let this = self.0;
        put!(this + guest_offsets::HAPPINESS, 4);
        for offset in [guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM] {
            put!(this + offset, 4);
        }
        put!(this + guest_offsets::FLAG_0X336, 1);
        for offset in [0x2d0u32, 0x2d4, 0x2d8, 0x2dc, 0x2e0, 0x320, 0x324, 0x2e8, 0x2ec, 0x2f0, 0x2f4, 0x2f8, 0x2fc, 0x304, 0x308, 0x30c, 0x310] {
            put!(this + offset, 4);
        }
        for vector in [guest_offsets::ITEMS, guest_offsets::SAVED_IDS] {
            let begin: u32 = self.read(vector);
            let count = (self.read::<u32>(vector + 4).wrapping_sub(begin) as i32) >> 2;
            put!(&count as *const i32 as u32, 4);
            for index in 0..count as u32 {
                put!(self.read::<u32>(vector) + index * 4, 4);
            }
        }
        put!(this + guest_offsets::RETURN_BUILDING_BITS, 4);
        let string_begin: u32 = self.read(guest_offsets::RETURN_BUILDING_STRING);
        let string_length = self.read::<u32>(guest_offsets::RETURN_BUILDING_STRING + 4).wrapping_sub(string_begin);
        if string_length >= 0x1000 {
            ok = false;
        } else {
            put!(&string_length as *const u32 as u32, 4);
            if string_length > 0 {
                put!(string_begin, string_length);
            }
        }
        let sites_begin: u32 = self.read(guest_offsets::VISITED_SITES);
        let site_count = (self.read::<u32>(guest_offsets::VISITED_SITES + 4).wrapping_sub(sites_begin) as i32) / 12;
        put!(&site_count as *const i32 as u32, 4);
        for index in 0..site_count as u32 {
            let site = self.read::<u32>(guest_offsets::VISITED_SITES) + index * 12;
            put!(site, 4);
            put!(site + 4, 4);
        }
        for (offset, size) in [
            (0x300u32, 4u32),
            (0x339, 1),
            (0x33a, 1),
            (0x314, 4),
            (0x33b, 1),
            (0x33c, 1),
            (guest_offsets::DATE, 8),
            (0x31c, 4),
            (0x33e, 1),
            (0x33f, 1),
            (guest_offsets::FIELD_0X298, 8),
            (0x334, 2),
            (0x337, 1),
            (0x338, 1),
            (guest_offsets::GUIDE_ID, 4),
        ] {
            put!(this + offset, size);
        }
        let unit_ok = ZTUnit(self.0).save(file as u32);
        unit_ok && ok
    }
}


/// First save version whose guest data carries `+0x284` and the return-building string directly; older streams carry
/// two length-prefixed strings that are read and discarded instead.
const MODERN_LOAD_VERSION: u32 = 0x5e;
/// Streams older than this carry the four per-slot bytes of the `BFEntity` byte vector at `+0xfc`.
const SLOT_MARK_VERSION: u32 = 0x43;
/// Streams older than this carry three more discarded values (a byte, a dword, a byte) after the `+0x324` word.
const LEGACY_TRIPLE_VERSION: u32 = 0x41;
/// `BFEntity` byte vector (begin, end, cap) `load` resizes to four entries for streams older than [`SLOT_MARK_VERSION`].
const SLOT_MARK_VECTOR: u32 = 0xfc;
/// Number of entries the slot-mark vector holds after such a load.
const SLOT_MARK_COUNT: u32 = 4;
/// `ZTGuestType` field holding the array of four `vector<u32>` (12 bytes each) whose lengths bound the slot marks.
const TYPE_SLOT_LISTS: u32 = 0x28;
/// `vector<byte>::insert(this, position, count, &value)` (`0x004128c5`, `thiscall`, `RET 0xc`), unnamed in `generated.rs`.
const BYTE_VECTOR_INSERT: u32 = 0x0041_28c5;
/// `FUN_00449f50` (`cdecl(id) -> entry or 0`): finds a carried item's record by id in the item-type map
/// (`DAT_006390c8`); `generated.rs` has no entry for it.
const ITEM_RECORD_LOOKUP: u32 = 0x0044_9f50;
/// `FUN_004f60f5` (`cdecl() -> animal type id`): picks a random animal type with one shared-RNG draw.
const RANDOM_ANIMAL_TYPE: u32 = 0x004f_60f5;
/// The item and saved-id vectors keep at most this many entries from a stream; further entries are read and dropped.
const LOAD_MAX_SMALL_VECTOR: u32 = 5;
/// The visited-site vector keeps at most this many records from a stream.
const LOAD_MAX_SITES: u32 = 10;
/// `BFEntity` dword the viewing area id (`+0x328`) is seeded from on load.
const VIEWING_AREA_SEED_OFFSET: u32 = 0x12c;

impl ZTGuest {
    /// Appends `value` to the pool `vector<u32>` header at `vector`, growing as vanilla does.
    fn push_pool_word(vector: u32, value: u32) {
        push_word(vector, value, |begin, bytes| unsafe { POOLALLOC_DEALLOCATE_N_4.original()(begin as *const u32, (bytes >> 2) as i32) });
    }

    /// Appends the record `{first, second, 0}` to the pool `vector<12-byte>` header at `vector`: when full a buffer of
    /// double the element count (one for an empty vector) is taken from `PoolAlloc`, the old records are copied, and the
    /// old buffer (if any) is handed back.
    fn push_site(vector: u32, first: u32, second: u32, third: u32) {
        push_words(vector, [first, second, third], |begin, byte_capacity| {
            if begin != 0 {
                unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, (byte_capacity / 12) * 12) };
            }
        });
    }

    fn vector_len(vector: u32, element: u32) -> u32 {
        get_from_memory::<u32>(vector + 4).wrapping_sub(get_from_memory::<u32>(vector)) / element
    }

    /// Reads and discards one length-prefixed legacy string; a length of `0x1000` or more fails.
    fn skip_legacy_string(file: *const u32) -> bool {
        let mut length: u32 = 0;
        if !read_field(&mut length as *mut u32 as u32, 4, file) || length >= 0x1000 {
            return false;
        }
        if length == 0 {
            return true;
        }
        let mut bytes = vec![0u8; length as usize];
        read_field(bytes.as_mut_ptr() as u32, length, file)
    }

    /// Streams older than [`SLOT_MARK_VERSION`]: resizes the `+0xfc` byte vector to four entries (zero-filled when it
    /// grows) and reads one value per entry, `4` bytes wide up to version `7` and `1` byte wide after. Each entry keeps
    /// the value's low byte, or `0` when it reaches the length of the guest type's matching list. `ok` is whether every
    /// read so far succeeded; returns it with this method's reads folded in, and stops after the first read once it is
    /// `false`, as vanilla does.
    fn load_slot_marks(self, file: *const u32, version: u32, mut ok: bool) -> bool {
        let vector = self.0 + SLOT_MARK_VECTOR;
        let size = get_from_memory::<u32>(vector + 4).wrapping_sub(get_from_memory::<u32>(vector));
        if (size as i32) < SLOT_MARK_COUNT as i32 + 1 {
            let zero = 0u8;
            let insert = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, *const u8)>(BYTE_VECTOR_INSERT) };
            insert(vector, get_from_memory(vector + 4), SLOT_MARK_COUNT - size, &zero);
        } else {
            save_to_memory(vector + 4, get_from_memory::<u32>(vector) + SLOT_MARK_COUNT);
        }
        let width = if version > 7 { 1 } else { 4 };
        let lists: u32 = match self.guest_type() {
            0 => 0,
            guest_type => get_from_memory(guest_type + TYPE_SLOT_LISTS),
        };
        for slot in 0..SLOT_MARK_COUNT {
            let mut value: u32 = 0;
            let read_ok = read_field(&mut value as *mut u32 as u32, width, file);
            // A read wider than one byte lands on the following entries too; only the entries inside the vector are
            // kept, and the next read overwrites all but the first of them.
            let begin: u32 = get_from_memory(vector);
            if read_ok {
                for spill in 0..width.min(SLOT_MARK_COUNT - slot) {
                    save_to_memory(begin + slot + spill, (value >> (8 * spill)) as u8);
                }
            }
            ok &= read_ok;
            if !ok {
                return false;
            }
            let mut mark = value as u8;
            if lists != 0 {
                let list = lists + slot * 12;
                let length = (get_from_memory::<u32>(list + 4).wrapping_sub(get_from_memory::<u32>(list)) as i32 >> 2) as u32;
                if length <= u32::from(mark) {
                    mark = 0;
                }
            }
            save_to_memory(begin + slot, mark);
        }
        true
    }

    /// `ZTGuest::load` (`0x00455fa2`) for every stream version. Reads the needs, `+0x336` and the per-need words, sets the viewing area ids (`+0x32c` zero, `+0x328`
    /// from `+0x12c`), then the `+0x320`/`+0x324` words, the check slots, the carried-item vector (each id is looked up
    /// and zeroed when unknown; at most 5 kept) and the saved-id vector (at most 5 kept), `+0x284`, the
    /// return-building string (a length of `0x1000` or more fails the load), the visited sites (at most 10 kept), the
    /// remaining words and flags, then `ZTUnit::load`. Reads continue after a failed one up to the next point where
    /// vanilla returns early, so the bytes consumed match.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let this = self.0;
        let mut ok = true;
        macro_rules! read {
            ($offset:expr, $size:expr) => {
                ok &= read_field(this + $offset, $size, file)
            };
        }
        for offset in [guest_offsets::HAPPINESS, guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM] {
            read!(offset, 4);
        }
        read!(guest_offsets::FLAG_0X336, 1);
        for offset in [0x2d0u32, 0x2d4, 0x2d8, 0x2dc, 0x2e0] {
            read!(offset, 4);
        }
        self.write(guest_offsets::VIEWING_AREA, 0u32);
        self.write(guest_offsets::VIEWING_AREA_ID, self.read::<u32>(VIEWING_AREA_SEED_OFFSET));
        if version > 2 {
            if version < SLOT_MARK_VERSION && !self.load_slot_marks(file, version, ok) {
                return false;
            }
            read!(0x320, 4);
            read!(0x324, 4);
            if version < LEGACY_TRIPLE_VERSION {
                let mut discarded: u64 = 0;
                ok &= read_field(&mut discarded as *mut u64 as u32, 1, file);
                ok &= read_field(&mut discarded as *mut u64 as u32, 4, file);
                ok &= read_field(&mut discarded as *mut u64 as u32, 1, file);
            }
        }
        if version > 4 {
            for offset in [0x2e8u32, 0x2ec, 0x2f0, 0x2f4, 0x2f8, 0x2fc, 0x304, 0x308, 0x30c, 0x310] {
                read!(offset, 4);
            }
            let mut count: u32 = 0;
            ok &= read_field(&mut count as *mut u32 as u32, 4, file);
            if !ok {
                return false;
            }
            let items = this + guest_offsets::ITEMS;
            save_to_memory(items + 4, get_from_memory::<u32>(items));
            for _ in 0..count {
                let mut id: u32 = 0;
                ok &= read_field(&mut id as *mut u32 as u32, 4, file);
                if !ok {
                    return false;
                }
                if id != 0 && unsafe { mem::transmute::<u32, extern "cdecl" fn(u32) -> u32>(ITEM_RECORD_LOOKUP)(id) } == 0 {
                    id = 0;
                }
                if Self::vector_len(items, 4) < LOAD_MAX_SMALL_VECTOR {
                    Self::push_pool_word(items, id);
                }
            }

            if version == 5 {
                ok &= read_field(&mut count as *mut u32 as u32, 4, file);
                if !ok {
                    return false;
                }
                for _ in 0..count {
                    let mut discarded: u64 = 0;
                    ok &= read_field(&mut discarded as *mut u64 as u32, 8, file);
                }
            }
            ok &= read_field(&mut count as *mut u32 as u32, 4, file);
            if !ok {
                return false;
            }
            let ids = this + guest_offsets::SAVED_IDS;
            save_to_memory(ids + 4, get_from_memory::<u32>(ids));
            for _ in 0..count {
                let mut id: u32 = 0;
                ok &= read_field(&mut id as *mut u32 as u32, 4, file);
                if !ok {
                    return false;
                }
                if Self::vector_len(ids, 4) < LOAD_MAX_SMALL_VECTOR {
                    Self::push_pool_word(ids, id);
                }
            }
            if version < MODERN_LOAD_VERSION {
                if !Self::skip_legacy_string(file) {
                    return false;
                }
                ok &= Self::skip_legacy_string(file);
            } else {
                read!(guest_offsets::RETURN_BUILDING_BITS, 4);
                if !ok {
                    return false;
                }
                let mut length: u32 = 0;
                let mut string_ok = read_field(&mut length as *mut u32 as u32, 4, file);
                if string_ok {
                    if length < 0x1000 {
                        let mut bytes = vec![0u8; length as usize];
                        if length > 0 {
                            string_ok &= read_field(bytes.as_mut_ptr() as u32, length, file);
                        }
                        let range = bytes.as_ptr() as u32;
                        unsafe {
                            ASSIGN_RANGE.original()(
                                (this + guest_offsets::RETURN_BUILDING_STRING) as *const c_void,
                                range as *const u32,
                                (range + length) as *const u8,
                                std::ptr::null(),
                            )
                        };
                    } else {
                        string_ok = false;
                    }
                }
                ok &= string_ok;
            }
            if !ok {
                return false;
            }
        }

        let sites = this + guest_offsets::VISITED_SITES;
        if version > 8 {
            let mut count: u32 = 0;
            ok &= read_field(&mut count as *mut u32 as u32, 4, file);
            if !ok {
                return false;
            }
            save_to_memory(sites + 4, get_from_memory::<u32>(sites));
            for _ in 0..count {
                let (mut first, mut second): (u32, u32) = (0, 0);
                ok &= read_field(&mut first as *mut u32 as u32, 4, file);
                ok &= read_field(&mut second as *mut u32 as u32, 4, file);
                if !ok {
                    return false;
                }
                if Self::vector_len(sites, 12) < LOAD_MAX_SITES {
                    Self::push_site(sites, first, second, 0);
                }
            }
        }
        if version > 0x12 {
            read!(0x300, 4);
        }
        if version > 0x15 {
            read!(0x339, 1);
            read!(0x33a, 1);
            read!(0x314, 4);
        }
        if version > 0x1e {
            read!(0x33b, 1);
            read!(0x33c, 1);
            if !ok {
                return false;
            }
            let items: u32 = get_from_memory(this + guest_offsets::ITEMS);
            if items != 0 && get_from_memory::<u32>(items + 0xc) != 0 {
                self.write(0x33c, 0u8);
            }
        }
        if version > 0x20 {
            read!(guest_offsets::DATE, 8);
            read!(0x31c, 4);
            if !ok {
                return false;
            }
            if unsafe { openzt_detour::generated::ztanimaltype::GET_TYPE_FROM_ID.original()(self.read::<u32>(0x31c) as i32) } == 0 {
                self.write(0x31c, unsafe { mem::transmute::<u32, extern "cdecl" fn() -> u32>(RANDOM_ANIMAL_TYPE)() });
            }
        }
        if version > 0x22 {
            read!(0x33e, 1);
            read!(0x33f, 1);
        }
        if version > 0x24 {
            read!(guest_offsets::FIELD_0X298, 8);
        }
        if version > 0x29 {
            read!(0x334, 2);
            if !ok {
                return false;
            }
            if self.read::<u16>(0x334) == 0 {
                self.write(0x334, Self::vector_len(sites, 12) as u16);
            }
        }
        if version > 0x35 {
            read!(0x337, 1);
        }
        if version > 0x37 {
            read!(0x338, 1);
        }
        if version > 0x40 {
            read!(guest_offsets::GUIDE_ID, 4);
        }
        if !ok {
            return false;
        }
        ZTUnit(self.0).load(file as u32, version)
    }
}

/// `ZTGuestType` fields `initStatusVars` reads.
mod status_type_offsets {
    pub const NAME_BEGIN: u32 = 0x98;
    pub const NAME_END: u32 = 0x9c;
    /// `initStatusVars` links the unit's `+0x68` to this block when the word at `+0x84` is non-zero.
    pub const BLOCK: u32 = 0x80;
    pub const BLOCK_GATE: u32 = 0x84;
    pub const START_HAPPINESS: u32 = 0x1e8;
    pub const HUNGER_START: u32 = 0x1f4;
    pub const HUNGER_END: u32 = 0x1f0;
    pub const THIRST_START: u32 = 0x1fc;
    pub const THIRST_END: u32 = 0x1f8;
    pub const ENERGY_START: u32 = 0x204;
    pub const ENERGY_END: u32 = 0x200;
    pub const BATHROOM_START: u32 = 0x20c;
    pub const BATHROOM_END: u32 = 0x208;
    /// The eleven periods the check slots `+0x2e8..+0x310` are drawn from (`+0x300` is the seventh).
    pub const SLOT_PERIODS: [u32; 11] = [0x1b4, 0x1b8, 0x1bc, 0x1c0, 0x1c4, 0x1c8, 0x1cc, 0x1d0, 0x1d4, 0x1d8, 0x1dc];
    pub const CHECK_PERIOD: u32 = 0x1e0;
}

/// The check slots `initStatusVars` fills, in the order of [`status_type_offsets::SLOT_PERIODS`].
const CHECK_SLOTS: [u32; 11] = [0x2e8, 0x2ec, 0x2f0, 0x2f4, 0x2f8, 0x2fc, 0x300, 0x304, 0x308, 0x30c, 0x310];

/// Globals, as RVAs from the `zoo.exe` base.
mod status_globals {
    /// `DAT_00638060`: the shared RNG state.
    pub const RNG_STATE: u32 = 0x0023_8060;
    /// `DAT_00639114`: the one-in-N chance a new guest's need start values are varied (`0` or less disables it).
    pub const VARIATION_ONE_IN: u32 = 0x0023_9114;
    /// `DAT_0063e296`: set once the range caches below have been evaluated against the guest type.
    pub const RANGES_READY: u32 = 0x0023_e296;
    /// The variation ranges (`low` < `high`, ints) and the flag byte set when the guest type's own range covers them.
    pub const HUNGER_RANGE: (u32, u32, u32) = (0x0023_e2ac, 0x0023_e2b0, 0x0023_e2b4);
    pub const THIRST_RANGE: (u32, u32, u32) = (0x0023_e2b8, 0x0023_e2bc, 0x0023_e2c0);
    pub const ENERGY_RANGE: (u32, u32, u32) = (0x0023_e2c4, 0x0023_e2c8, 0x0023_e2a8);
    pub const BATHROOM_RANGE: (u32, u32, u32) = (0x0023_e2cc, 0x0023_e2d0, 0x0023_e2a9);
}

/// `ZTAIMgr` field copied into the status-reset countdown (`+0x318`).
const AIMGR_RESET_INTERVAL_OFFSET: u32 = 0x13c;
/// `GLOBAL_ZTGameMgr` is reached through [`globals`]; the in-game and real-world date that dress guests as costumes.
const HALLOWEEN: (u32, u32) = (0x1f, 10);
/// `BFEntity` pointer to the byte vector `initStatusVars` writes the costume bytes through.
const COSTUME_BYTES_OFFSET: u32 = 0xfc;

impl ZTGuest {
    fn status_base() -> u32 {
        get_module_base("zoo.exe") as u32
    }

    /// One draw from the shared RNG: advances `DAT_00638060` and returns its bits `16..31` (`0..0x7fff`).
    fn rng_draw() -> u32 {
        next_game_random()
    }

    /// `ZTGuest::initStatusVars` (`0x004f6372`): `ZTUnit::initStatusVars`; for a guest type, resets the guest's state.
    ///
    /// Needs: happiness from the type's start value, the four need levels from the type's start values. When
    /// `DAT_00639114` is positive, one shared-RNG draw modulo it that comes out zero varies one need: a second draw's low
    /// two bits pick hunger (0, shared-RNG draw within `[DAT_0063e2ac, DAT_0063e2b0)`), thirst (1, same within
    /// `[DAT_0063e2b8, DAT_0063e2bc)`), energy (2, CRT `rand()` within `[DAT_0063e2c4, DAT_0063e2c8)`) or bathroom (3,
    /// CRT `rand()` within `[DAT_0063e2cc, DAT_0063e2d0)`), provided that need's flag byte is set. The flags are decided
    /// once per process from the first guest type seen (`DAT_0063e296`).
    ///
    /// Then the item vector holds five zero slots, the saved-id and visited-site vectors are emptied, the return-building
    /// string is cleared, the two `+0x320`/`+0x324` words are drawn one bit each (overridden by the type name `"woman"`,
    /// `"girl"`, `"man"` or `"boy"`), the check slots are drawn modulo the type's periods (`-1` for a zero period), and
    /// the date, the random animal type, the Halloween costume bytes and the unit's self and type-block pointers are
    /// set.
    pub fn init_status_vars(self) {
        ZTUnit(self.0).init_status_vars();
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return;
        }
        let base = Self::status_base();
        let global = |rva: u32| base + rva;
        let type_int = |offset: u32| get_from_memory::<i32>(guest_type + offset);

        self.write(guest_offsets::HAPPINESS, type_int(status_type_offsets::START_HAPPINESS));
        self.write(0x2ac, 0u32);

        if get_from_memory::<u8>(global(status_globals::RANGES_READY)) == 0 {
            for (range, type_start, type_end) in [
                (status_globals::HUNGER_RANGE, status_type_offsets::HUNGER_START, status_type_offsets::HUNGER_END),
                (status_globals::THIRST_RANGE, status_type_offsets::THIRST_START, status_type_offsets::THIRST_END),
                (status_globals::ENERGY_RANGE, status_type_offsets::ENERGY_START, status_type_offsets::ENERGY_END),
                (status_globals::BATHROOM_RANGE, status_type_offsets::BATHROOM_START, status_type_offsets::BATHROOM_END),
            ] {
                let (low, high): (i32, i32) = (get_from_memory(global(range.0)), get_from_memory(global(range.1)));
                if low < high && type_int(type_start) <= low && high <= type_int(type_end) {
                    save_to_memory(global(range.2), 1u8);
                }
            }
            save_to_memory(global(status_globals::RANGES_READY), 1u8);
        }

        let flag = |range: (u32, u32, u32)| get_from_memory::<u8>(global(range.2)) != 0;
        let ranged = |range: (u32, u32, u32), draw: i64| -> i32 {
            let (low, high): (i32, i32) = (get_from_memory(global(range.0)), get_from_memory(global(range.1)));
            (draw % i64::from(high.wrapping_sub(low))) as i32 + low
        };
        let one_in: i32 = get_from_memory(global(status_globals::VARIATION_ONE_IN));
        let varied = one_in > 0 && (i64::from(Self::rng_draw()) % i64::from(one_in)) == 0;
        let (mut hunger, mut thirst, mut energy, mut bathroom) = (
            type_int(status_type_offsets::HUNGER_START),
            type_int(status_type_offsets::THIRST_START),
            type_int(status_type_offsets::ENERGY_START),
            type_int(status_type_offsets::BATHROOM_START),
        );
        if varied {
            let pick = Self::rng_draw() & 3;
            match pick {
                0 if flag(status_globals::HUNGER_RANGE) => hunger = ranged(status_globals::HUNGER_RANGE, i64::from(Self::rng_draw())),
                1 if flag(status_globals::THIRST_RANGE) => thirst = ranged(status_globals::THIRST_RANGE, i64::from(Self::rng_draw())),
                2 if flag(status_globals::ENERGY_RANGE) => {
                    let (low, high): (i32, i32) = (get_from_memory(global(status_globals::ENERGY_RANGE.0)), get_from_memory(global(status_globals::ENERGY_RANGE.1)));
                    energy = (unsafe { RAND.hooked()() } as i32).wrapping_rem(high.wrapping_sub(low)).wrapping_add(low);
                }
                3 if flag(status_globals::BATHROOM_RANGE) => {
                    let (low, high): (i32, i32) = (get_from_memory(global(status_globals::BATHROOM_RANGE.0)), get_from_memory(global(status_globals::BATHROOM_RANGE.1)));
                    bathroom = (unsafe { RAND.hooked()() } as i32).wrapping_rem(high.wrapping_sub(low)).wrapping_add(low);
                }
                _ => {}
            }
        }
        self.write(guest_offsets::HUNGER, hunger);
        self.write(guest_offsets::THIRST, thirst);
        self.write(guest_offsets::ENERGY, energy);
        self.write(guest_offsets::BATHROOM, bathroom);
        for offset in [0x2b4u32, 0x2bc, 0x2c4, 0x2cc] {
            self.write(offset, 0u32);
        }
        self.write(guest_offsets::FLAG_0X336, 0u8);

        let items = self.0 + guest_offsets::ITEMS;
        save_to_memory(items + 4, get_from_memory::<u32>(items));
        for _ in 0..5 {
            Self::push_pool_word(items, 0);
        }
        let ids = self.0 + guest_offsets::SAVED_IDS;
        save_to_memory(ids + 4, get_from_memory::<u32>(ids));
        let sites = self.0 + guest_offsets::VISITED_SITES;
        save_to_memory(sites + 4, get_from_memory::<u32>(sites));
        self.write(0x334, 0u16);
        self.write(guest_offsets::RETURN_BUILDING_BITS, 0u32);
        let string = self.0 + guest_offsets::RETURN_BUILDING_STRING;
        unsafe { ERASE_RANGE.original()(string as *const c_void, get_from_memory::<u32>(string) as *const u32, get_from_memory::<u32>(string + 4) as *const u32) };
        self.write(0x337, 0u8);
        self.write(guest_offsets::GUIDE_ID, 0u32);
        self.write(0x338, 0u8);

        self.write(0x320, Self::rng_draw() & 1);
        self.write(0x324, Self::rng_draw() & 1);
        let name_begin: u32 = get_from_memory(guest_type + status_type_offsets::NAME_BEGIN);
        let name_end: u32 = get_from_memory(guest_type + status_type_offsets::NAME_END);
        let name: Vec<u8> = if name_begin == 0 || name_end < name_begin { Vec::new() } else { (name_begin..name_end).map(get_from_memory::<u8>).collect() };
        match name.as_slice() {
            b"woman" => {
                self.write(0x320, 1u32);
                self.write(0x324, 0u32);
            }
            b"girl" => {
                self.write(0x320, 1u32);
                self.write(0x324, 1u32);
            }
            b"man" => {
                self.write(0x320, 0u32);
                self.write(0x324, 0u32);
            }
            b"boy" => {
                self.write(0x320, 0u32);
                self.write(0x324, 1u32);
            }
            _ => {}
        }

        for offset in [0x339u32, 0x33b, 0x33a, 0x33c, 0x33d] {
            self.write(offset, 0u8);
        }
        for offset in [0x2d0u32, 0x2d4, 0x2d8, 0x2dc, 0x2e0] {
            self.write(offset, 1u32);
        }
        for (slot, period_offset) in CHECK_SLOTS.into_iter().zip(status_type_offsets::SLOT_PERIODS) {
            let period = type_int(period_offset);
            let value = if period == 0 { -1 } else { (Self::rng_draw() as i32).wrapping_rem(period) };
            self.write(slot, value);
        }
        self.write(guest_offsets::CHECK_COUNTDOWN, type_int(status_type_offsets::CHECK_PERIOD));
        self.write(guest_offsets::RANDOM_DONE, 0u8);
        self.write(0x33f, 0u8);
        let aimgr = globals().ztaimgr_ptr() as u32;
        self.write(0x318, if aimgr == 0 { 0 } else { get_from_memory::<u32>(aimgr + AIMGR_RESET_INTERVAL_OFFSET) });

        let gamemgr = globals().ztgamemgr_ptr() as *const u32;
        let mut date = [0u32; 2];
        let date_ptr = unsafe { GAMEMGR_GET_DATE.hooked()(gamemgr, date.as_mut_ptr() as *const _) } as *const u32;
        if !date_ptr.is_null() {
            self.write(guest_offsets::DATE, get_from_memory::<u32>(date_ptr as u32));
            self.write(guest_offsets::DATE + 4, get_from_memory::<u32>(date_ptr as u32 + 4));
        }
        self.write(0x31c, unsafe { mem::transmute::<u32, extern "cdecl" fn() -> u32>(RANDOM_ANIMAL_TYPE)() });
        if unsafe { GAMEMGR_IS_GAME_DATE.hooked()(gamemgr, 1, 1) } {
            self.write(guest_offsets::FLAG_0X336, 1u8);
        }
        if unsafe { GAMEMGR_IS_GAME_DATE.hooked()(gamemgr, HALLOWEEN.0, HALLOWEEN.1) }
            || low_byte_bool(unsafe { GAMEMGR_IS_REAL_WORLD_DATE.hooked()(HALLOWEEN.0 as i32, HALLOWEEN.1) })
        {
            let costume: u32 = self.read(COSTUME_BYTES_OFFSET);
            if costume != 0 {
                save_to_memory(costume, 10u8);
                save_to_memory(costume + 1, if self.read::<u32>(0x320) != 0 { 11u8 } else { 2u8 });
            }
        }

        self.write(unit_offsets::SELF_POINTER, self.0);
        let block = if get_from_memory::<u32>(guest_type + status_type_offsets::BLOCK_GATE) == 0 { 0 } else { guest_type + status_type_offsets::BLOCK };
        self.write(unit_offsets::TYPE_BLOCK_POINTER, block);
    }
}
/// The low 19 bits of `+0x284`.
const RETURN_BUILDING_NAME_MASK: u32 = 0x7ffff;
/// Bit 19 of `+0x284`.
const WATER_RECOVERING_BIT: u32 = 0x80000;

/// Shared tile-to-megatile lookup for all three methods below: `ZTMegatile`s are indexed by `x/5`, `y/5`
/// (see `ztmegatilemgr.rs`'s own `recalculate_characteristics` doc comment for the same indexing), and a
/// negative tile coordinate (never real in practice, but not representable as the `usize` the accessor
/// takes) is treated the same as an out-of-range one: `None`.
fn tile_megatile(tile: &BFTile) -> Option<&'static ZTMegatile> {
    if tile.pos.x < 0 || tile.pos.y < 0 {
        return None;
    }
    globals().ztmegatilemgr().megatile((tile.pos.x / 5) as usize, (tile.pos.y / 5) as usize)
}

/// `ZTGuest::fCrowdDensityMegatile` (`0x0043b7e3`). Confirmed by the MSVC magic-constant (`0x51eb851f`)
/// division idiom at the tail of the real function, applied to the megatile's `guest_count` field
/// (offset `0x0`) - matching the mac decompile's own `guest_count()*10/25` formula exactly. `0` for a
/// tile with no allocated megatile, matching vanilla's own implicit "empty tile" behavior.
pub fn crowd_density_megatile(tile: &BFTile) -> i32 {
    tile_megatile(tile).map_or(0, |m| m.guest_count() * 10 / 25)
}

/// `ZTGuest::fStinkyMegatile` (`0x0043b84a`). Reads the megatile's `stink` scalar, confirmed by the real
/// function loading `ZTMegatile+0x10` (`fld dword ptr [eax+edx*4+0x10]`) and comparing the result against
/// `ZTGuestType::stink_threshold` (`bfentitytype.rs`, offset `0x2B0`). `0.0` for a tile with no allocated
/// megatile.
pub fn stinky_megatile(tile: &BFTile) -> f32 {
    tile_megatile(tile).map_or(0.0, |m| m.stink())
}

/// `ZTGuest::fEstheticBonusMegatile` (`0x0043b6c0`). Looks up `this`'s own entity type's category id (via
/// [`entity_category_id`]) in the megatile's `category_map` (`ZTMegatile::category_value`) - confirmed by
/// the real function's virtual call through the entity type's vtable slot `+0x20`, then a red-black-tree
/// search matching `category_map`'s existing shape, then a comparison of the found value against
/// `ZTGuestType::object_esthetic_threshold` (`bfentitytype.rs`, offset `0x2A4`). `0.0` for a tile with no
/// allocated megatile, or a category id with no entry in that megatile's map - vanilla's own
/// find-or-fail-then-`fld` idiom would return whatever the FPU stack happened to hold on a failed lookup,
/// a case never actually hit in practice since every real entity type has a populated category before a
/// guest ever calls this; `0.0` is a well-defined, safer stand-in for that unreachable branch.
pub fn esthetic_bonus_megatile(this: &BFEntity, tile: &BFTile) -> f32 {
    let Some(megatile) = tile_megatile(tile) else {
        return 0.0;
    };
    megatile.category_value(entity_category_id(this)).unwrap_or(0.0)
}

/// Calls `entity`'s own entity type's vtable slot `+0x20` - confirmed `ZTGuestType::0x00401fb7`
/// (`private/docs/vtables/ZTGuestType.md`), a trivial `mov eax,[ecx+0x130]; ret` getter - to resolve the
/// category id [`esthetic_bonus_megatile`] looks up. Same vtable-slot call-through shape as
/// `ztmegatilemgr::entity_type_matches`.
///
/// Offset `0x130` on a `ZTGuestType`/`ZTUnitType` instance is also exactly `BFUnitType::name_id`
/// (`bfentitytype.rs:765`, a string resource id, not obviously a `BFCategory`-style category id in the
/// `0x251f..0x2523` range `recalculate_characteristics` populates the map with) - this may be a genuine
/// "category id" field that happens to share `name_id`'s slot by coincidence of this project's own field
/// naming, or evidence the vtable-slot identification needs another look. Not asserted as settled; the
/// live `ZTGUEST_MEGATILE_METHODS_LIVE` comparison in `reimplementation_tests` is the actual verification
/// for this call, not this doc comment.
fn entity_category_id(entity: &BFEntity) -> i32 {
    let entity_type_ptr = *entity.inner_class_ptr();
    let vtable = get_from_memory::<u32>(entity_type_ptr);
    let get_category_id_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> i32>(get_from_memory::<u32>(vtable + 0x20)) };
    get_category_id_fn(entity_type_ptr)
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(F_CROWD_DENSITY_MEGATILE)]
    unsafe extern "thiscall" fn f_crowd_density_megatile(this: *const u32) -> i32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0;
        };
        crowd_density_megatile(&tile)
    }

    #[detour(F_STINKY_MEGATILE)]
    unsafe extern "thiscall" fn f_stinky_megatile(this: *const u32) -> f32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0.0;
        };
        stinky_megatile(&tile)
    }

    #[detour(F_ESTHETIC_BONUS_MEGATILE)]
    unsafe extern "thiscall" fn f_esthetic_bonus_megatile(this: *const u32) -> f32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0.0;
        };
        esthetic_bonus_megatile(entity, &tile)
    }

    #[detour(IS_RETURNING)]
    unsafe extern "thiscall" fn is_returning(this: *const u32) -> bool {
        ZTGuest(this as u32).is_returning()
    }

    #[detour(IS_WATER_RECOVERING)]
    unsafe extern "thiscall" fn is_water_recovering(this: *const u32) -> bool {
        ZTGuest(this as u32).is_water_recovering()
    }

    #[detour(IS_WATER_TRAVELING)]
    unsafe extern "thiscall" fn is_water_traveling(this: *const u32) -> bool {
        ZTGuest(this as u32).is_water_traveling()
    }

    #[detour(SET_WATER_RECOVERING)]
    unsafe extern "thiscall" fn set_water_recovering(this: *const u32, recovering: bool) {
        ZTGuest(this as u32).set_water_recovering(recovering);
    }

    #[detour(DIAGONAL_OK)]
    unsafe extern "thiscall" fn diagonal_ok(this: *const u32) -> bool {
        ZTGuest(this as u32).diagonal_ok()
    }

    #[detour(RANDOM_DONE)]
    unsafe extern "thiscall" fn random_done(this: *const u32) {
        ZTGuest(this as u32).random_done();
    }

    #[detour(GET_RANDOM_OK)]
    unsafe extern "thiscall" fn get_random_ok(this: *const u32) -> bool {
        ZTGuest(this as u32).random_ok()
    }

    #[detour(SET_RETURN_BUILDING_NAME_ID)]
    unsafe extern "thiscall" fn set_return_building_name_id(this: *const u32, id: i32) {
        ZTGuest(this as u32).set_return_building_name_id(id);
    }

    #[detour(GET_RETURN_BUILDING_NAME_ID)]
    unsafe extern "thiscall" fn get_return_building_name_id(this: *const u32) -> u32 {
        ZTGuest(this as u32).return_building_name_id()
    }

    #[detour(GET_RETURN_BUILDING_STRING)]
    unsafe extern "thiscall" fn get_return_building_string(this: *const u32) -> i32 {
        ZTGuest(this as u32).return_building_string() as i32
    }

    #[detour(GET_VIEWING_AREA)]
    unsafe extern "thiscall" fn get_viewing_area(this: *const u32) -> u32 {
        ZTGuest(this as u32).viewing_area()
    }

    #[detour(GET_HABITAT_FROM_VA)]
    unsafe extern "thiscall" fn get_habitat_from_va(this: *const u32) -> u32 {
        ZTGuest(this as u32).habitat_from_va()
    }

    #[detour(GET_COLOR_REP_INFO)]
    unsafe extern "thiscall" fn get_color_rep_info(this: *const u32) -> i32 {
        ZTGuest(this as u32).color_rep_info()
    }

    #[detour(GET_HAPPY_PCT)]
    unsafe extern "fastcall" fn get_happy_pct(this: i32) -> i32 {
        ZTGuest(this as u32).happy_pct()
    }

    #[detour(GET_THIRST_PCT)]
    unsafe extern "fastcall" fn get_thirst_pct(this: i32) -> i32 {
        ZTGuest(this as u32).thirst_pct()
    }

    #[detour(GET_HUNGER_PCT)]
    unsafe extern "fastcall" fn get_hunger_pct(this: i32) -> i32 {
        ZTGuest(this as u32).hunger_pct()
    }

    #[detour(GET_ENERGY_PCT)]
    unsafe extern "fastcall" fn get_energy_pct(this: i32) -> i32 {
        ZTGuest(this as u32).energy_pct()
    }

    #[detour(GET_BATHROOM_PCT)]
    unsafe extern "fastcall" fn get_bathroom_pct(this: i32) -> i32 {
        ZTGuest(this as u32).bathroom_pct()
    }

    #[detour(HAS_PRESSING_NEED)]
    unsafe extern "thiscall" fn has_pressing_need(this: *const u32) -> bool {
        ZTGuest(this as u32).has_pressing_need()
    }

    #[detour(IS_DOING_SOMETHING)]
    unsafe extern "thiscall" fn is_doing_something(this: *const u32) -> bool {
        ZTGuest(this as u32).is_doing_something()
    }

    #[detour(F_ANIMAL_HAPPY)]
    unsafe extern "fastcall" fn f_animal_happy(this: *const c_void) -> i32 {
        ZTGuest(this as u32).animal_happy()
    }

    #[detour(F_TOUR_GUIDE_BONUS)]
    unsafe extern "fastcall" fn f_tour_guide_bonus(this: *const c_void) -> u32 {
        ZTGuest(this as u32).tour_guide_bonus()
    }

    #[detour(DO_ANGRY_PRICE_CHANGE)]
    unsafe extern "thiscall" fn do_angry_price_change(this: *const u32) {
        ZTGuest(this as u32).do_angry_price_change();
    }

    #[detour(DO_HAPPY_PRICE_CHANGE)]
    unsafe extern "thiscall" fn do_happy_price_change(this: *const u32) {
        ZTGuest(this as u32).do_happy_price_change();
    }

    #[detour(DO_TOURGUIDE_BONUS)]
    unsafe extern "thiscall" fn do_tourguide_bonus(this: *const u32) {
        ZTGuest(this as u32).do_tourguide_bonus();
    }

    #[detour(SET_SPEED)]
    unsafe extern "thiscall" fn set_speed(this: *const u32, speed: u32) {
        ZTGuest(this as u32).set_speed(speed);
    }

    #[detour(CLEAR_GOAL)]
    unsafe extern "thiscall" fn clear_goal(this: *const u32) {
        ZTGuest(this as u32).clear_goal();
    }

    #[detour(ABORT_GOAL)]
    unsafe extern "thiscall" fn abort_goal(this: *const u32, param: u32) {
        ZTGuest(this as u32).abort_goal(low_byte_bool(param));
    }

    #[detour(ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> bool {
        ZTGuest(this as u32).add_to_map()
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, param: bool) {
        ZTGuest(this as u32).remove_from_map(param);
    }

    #[detour(RESET_AI)]
    unsafe extern "thiscall" fn reset_ai(this: *const u32) {
        ZTGuest(this as u32).reset_ai();
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(this: *const u32, param: bool) -> u32 {
        ZTGuest(this as u32).init(param)
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTGuest(this as u32).save(file)
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) {
        ZTGuest(this as u32).init_status_vars();
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTGuest(this as u32).load(file, version)
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn is_returning(this: u32) -> bool {
            unsafe { super::IS_RETURNING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn is_water_recovering(this: u32) -> bool {
            unsafe { super::IS_WATER_RECOVERING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn is_water_traveling(this: u32) -> bool {
            unsafe { super::IS_WATER_TRAVELING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn set_water_recovering(this: u32, recovering: bool) {
            unsafe { super::SET_WATER_RECOVERING_DETOUR.call(this as *const u32, recovering) }
        }
        pub(crate) fn diagonal_ok(this: u32) -> bool {
            unsafe { super::DIAGONAL_OK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn random_done(this: u32) {
            unsafe { super::RANDOM_DONE_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_random_ok(this: u32) -> bool {
            unsafe { super::GET_RANDOM_OK_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn set_return_building_name_id(this: u32, id: i32) {
            unsafe { super::SET_RETURN_BUILDING_NAME_ID_DETOUR.call(this as *const u32, id) }
        }
        pub(crate) fn get_return_building_name_id(this: u32) -> u32 {
            unsafe { super::GET_RETURN_BUILDING_NAME_ID_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_return_building_string(this: u32) -> i32 {
            unsafe { super::GET_RETURN_BUILDING_STRING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_viewing_area(this: u32) -> u32 {
            unsafe { super::GET_VIEWING_AREA_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_habitat_from_va(this: u32) -> u32 {
            unsafe { super::GET_HABITAT_FROM_VA_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_color_rep_info(this: u32) -> i32 {
            unsafe { super::GET_COLOR_REP_INFO_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_happy_pct(this: u32) -> i32 {
            unsafe { super::GET_HAPPY_PCT_DETOUR.call(this as i32) }
        }
        pub(crate) fn get_thirst_pct(this: u32) -> i32 {
            unsafe { super::GET_THIRST_PCT_DETOUR.call(this as i32) }
        }
        pub(crate) fn get_hunger_pct(this: u32) -> i32 {
            unsafe { super::GET_HUNGER_PCT_DETOUR.call(this as i32) }
        }
        pub(crate) fn get_energy_pct(this: u32) -> i32 {
            unsafe { super::GET_ENERGY_PCT_DETOUR.call(this as i32) }
        }
        pub(crate) fn get_bathroom_pct(this: u32) -> i32 {
            unsafe { super::GET_BATHROOM_PCT_DETOUR.call(this as i32) }
        }
        pub(crate) fn has_pressing_need(this: u32) -> bool {
            unsafe { super::HAS_PRESSING_NEED_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn is_doing_something(this: u32) -> bool {
            unsafe { super::IS_DOING_SOMETHING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn f_animal_happy(this: u32) -> i32 {
            unsafe { super::F_ANIMAL_HAPPY_DETOUR.call(this as *const c_void) }
        }
        pub(crate) fn f_tour_guide_bonus(this: u32) -> u32 {
            unsafe { super::F_TOUR_GUIDE_BONUS_DETOUR.call(this as *const c_void) }
        }
        pub(crate) fn do_angry_price_change(this: u32) {
            unsafe { super::DO_ANGRY_PRICE_CHANGE_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_happy_price_change(this: u32) {
            unsafe { super::DO_HAPPY_PRICE_CHANGE_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_tourguide_bonus(this: u32) {
            unsafe { super::DO_TOURGUIDE_BONUS_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn set_speed(this: u32, speed: u32) {
            unsafe { super::SET_SPEED_DETOUR.call(this as *const u32, speed) }
        }
        pub(crate) fn clear_goal(this: u32) {
            unsafe { super::CLEAR_GOAL_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn abort_goal(this: u32, param: u32) {
            unsafe { super::ABORT_GOAL_DETOUR.call(this as *const u32, param) }
        }
        pub(crate) fn add_to_map(this: u32) -> bool {
            unsafe { super::ADD_TO_MAP_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn remove_from_map(this: u32, param: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, param) }
        }
        pub(crate) fn reset_ai(this: u32) {
            unsafe { super::RESET_AI_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn init(this: u32, param: bool) -> u32 {
            unsafe { super::INIT_DETOUR.call(this as *const u32, param) }
        }
        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }
        pub(crate) fn init_status_vars(this: u32) {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }
    }
}

/// Registers this module's live detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest detours: {e:?}");
    }
    checks::init();
    goals::init();
    inventory::init();
    movement::init();
    sites::init();
    viewing::init();
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use crate::bfentitytype::ZTEntityTypeClass;

    /// Finds every live `ZTGuest` in the world's entity array (via `BFEntity::entity_type_class()`, the
    /// same coarse type check `list_entities`/`get_zt_world_mgr_entities` already use elsewhere),
    /// returning each one's raw pointer and current tile - so a live comparison can sample the whole
    /// live population (different tiles, different megatiles, different entity-type category ids)
    /// instead of just whichever guest happens to be first in the array. Empty if no guest is currently
    /// on a resolvable tile (e.g. an empty/guest-less save, or a save loaded before any guest has
    /// spawned).
    pub(crate) fn find_live_guests() -> Vec<(u32, BFTile)> {
        let world = globals().ztworldmgr();
        let mut addr = world.entity_array_start();
        let end = world.entity_array_end();
        let mut guests = Vec::new();
        while addr < end {
            let entity_ptr = get_from_memory::<u32>(addr);
            if entity_ptr != 0 {
                let entity = unsafe { ref_from_memory::<BFEntity>(entity_ptr) };
                if entity.entity_type_class() == ZTEntityTypeClass::Guest
                    && let Some(tile) = entity.get_tile()
                {
                    guests.push((entity_ptr, tile));
                }
            }

            addr += 4;
        }
        guests
    }

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        let mut status = detours::status();
        status.extend(super::checks::live_support::detour_status());
        status.extend(super::goals::live_support::detour_status());
        status.extend(super::inventory::live_support::detour_status());
        status.extend(super::movement::live_support::detour_status());
        status.extend(super::sites::live_support::detour_status());
        status.extend(super::viewing::live_support::detour_status());
        status
    }

    /// The first `ZTGuestType` the live world holds, if any.
    pub fn find_guest_type() -> Option<u32> {
        let world = globals().ztworldmgr();
        let tag = get_module_base("zoo.exe") as u32 + GUEST_TYPE_TAG_RVA;
        let mut slot = world.entity_type_array_start;
        while slot != world.entity_type_array_end {
            let ty: u32 = get_from_memory(slot);
            slot += 4;
            if ty == 0 {
                continue;
            }
            let vtable: u32 = get_from_memory(ty);
            let is_cast_class = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
            if is_cast_class(ty, tag) {
                return Some(ty);
            }
        }
        None
    }

    /// Guests built by [`build_standalone_guest`] and neither destroyed nor forgotten.
    static LIVE_FIXTURES: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

    /// A zeroed guest built by the real vanilla constructor (not detoured) over `guest_type`. It is torn down through
    /// the real scalar-deleting destructor by [`destroy_standalone_guest`], or by [`teardown_standalone_guests`] when the
    /// test finishes. A test that points a field of the guest at memory it owns must put the original value back first,
    /// or call [`forget_standalone_guest`] to leak the fixture: the destructor frees whatever the fields hold.
    pub fn build_standalone_guest(guest_type: u32) -> ZTGuest {
        use openzt_detour::generated::{standalone::OPERATOR_NEW, ztguest::CONSTRUCTOR};
        let memory = unsafe { OPERATOR_NEW.original()(GUEST_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, GUEST_SIZE as usize) };
        unsafe { CONSTRUCTOR.original()(memory as *const u32, guest_type) };
        LIVE_FIXTURES.lock().unwrap().push(memory);
        ZTGuest(memory)
    }

    /// Stops tracking `guest`, leaving its memory allocated.
    pub fn forget_standalone_guest(guest: ZTGuest) {
        LIVE_FIXTURES.lock().unwrap().retain(|&fixture| fixture != guest.0);
    }

    /// Tears a guest down through the real scalar-deleting destructor (not detoured).
    pub fn destroy_standalone_guest(guest: ZTGuest) {
        LIVE_FIXTURES.lock().unwrap().retain(|&fixture| fixture != guest.0);
        unsafe { openzt_detour::generated::ztguest::DESTRUCTOR_1.original()(guest.0 as *const u32, 1) };
    }

    /// Destroys every guest still tracked from [`build_standalone_guest`].
    pub fn teardown_standalone_guests() {
        let fixtures = std::mem::take(&mut *LIVE_FIXTURES.lock().unwrap());
        for fixture in fixtures {
            unsafe { openzt_detour::generated::ztguest::DESTRUCTOR_1.original()(fixture as *const u32, 1) };
        }
    }

    pub(crate) use super::detours::test_real as real;
    pub(crate) use super::checks::live_support::real as checks_real;
    pub(crate) use super::goals::live_support::real as goals_real;
    pub(crate) use super::goals::overrides as goals_overrides;
    pub(crate) use super::check_overrides;
    pub(crate) use super::inventory::live_support::real as inventory_real;
    pub(crate) use super::movement::live_support::real as movement_real;
    pub(crate) use super::sites::live_support::real as sites_real;
    pub(crate) use super::sites::overrides as sites_overrides;
    pub(crate) use super::movement::overrides as movement_overrides;
    pub(crate) use super::viewing::live_support::real as viewing_real;
    pub(crate) use super::viewing::overrides as viewing_overrides;
    pub(crate) use super::movement::live_support::real_fences_ok_would_fault as movement_fences_would_fault;
}
