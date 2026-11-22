//! `ZTAnimal`: the zoo animal entity, a `ZTUnit` subclass (`ZTUnit` -> `BFUnit` -> `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): animals are heap entities that `ZTHabitat`, `ZTAIMgr`, the keeper/capture/
//! chase goals and the UI read by raw offset, and two string buffers inside the struct are vanilla-pool containers,
//! so the object stays vanilla memory and this module reads and writes it in place. An animal is
//! [`ANIMAL_SIZE`] (`0x3b8`) bytes: the `ZTUnit` prefix (`0x000`-`0x25f`) followed by the fields in
//! [`animal_offsets`]. Nothing here owns an allocation.
//!
//! The constructor and destructors stay vanilla (pool-allocated strings). Ported so far: the leaf predicates and
//! accessors that read only animal fields, the animal's type, and one other animal (`isHungry`, the percentage
//! getters, `isSickly`, `isInPreyList`, `isPreattack`, ...). `getHabitatRating` stays vanilla (a float-to-int
//! rounding chain through `0x415f2e`) and is called through `.original()` by the two habitat-unhappiness checks that
//! need it; `fIsThereKeeperFood` likewise.
//!
//! A null or non-animal entity type makes the vanilla bodies dereference null. The port answers `false`/`0` there
//! instead of faulting; live animals always carry an animal type.

use std::{ffi::c_void, mem::transmute};

use openzt_detour::generated::{
    bfentity::GET_TILE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bfunit::CHECK_DONE as BFUNIT_CHECK_DONE,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    standalone::OPERATOR_DELETE,
    zthabitat::GET_GATE_TILE_IN,
    ztworldmgr::SATISFIES as ZTWORLDMGR_SATISFIES,
    ztanimal::{
        CALC_HABITAT, CAN_CAPTURE, CAN_SERVICE, CAN_SLEEP_BE_SICK_EAT, CHECK_DONE, CLIFFS_OK, F_ANIMAL_SPACE_RATIO, F_FOOD_UNITS_ACTUALLY_EATEN,
        F_MATE_AVAILABLE, F_MATE_GONE, F_OTHER_ANIMAL_ANGRY, F_OTHER_ANIMAL_SICK, GET_HABITAT, GET_HOME_HABITAT, GET_ICON_ANIM, GET_PREY_UNIT,
        GET_SELL_PRICE, GET_SWIMS, GET_UNDERWATER, GET_UNDESIRABLE_SCENERY_IN_HABITAT, IS_NEED_SATISFIER, IS_WALKABLE_BY, RECALC_HOME_HABITAT,
        SET_HOME_HABITAT, F_GET_MATE, IS_DOING_SOMETHING,
        F_COMPAT_ANIMALS_RATING, F_NUM_ANIMALS, F_NUM_SAME_ADULT_ANIMALS, F_NUM_SAME_ANIMALS, F_TILES_PER_ADULT_ANIMAL, GET_ICON_ZOOM, GET_IDLE_ANIM,
        SET_MAX_HEALTH,
        F_IS_THERE_KEEPER_FOOD, GET_AMBIENT_KEY, GET_HABITAT_RATING, GET_HAPPY_PCT, GET_HEALTH_PCT, GET_HUNGER_PCT, GET_KEEP_MOVING, CLEAR_PREY,
        DIAGONAL_OK, IS_EGG, IS_HUNGRY, IS_HUNGRY_AND_FOODLESS, IS_IN_PREY_LIST, IS_PREATTACK, IS_SELECTABLE, IS_SICKLY, IS_UNHAPPY_FOR_REPRODUCTION,
        IS_UNHAPPY_WITH_HABITAT, IS_VERY_UNHAPPY_WITH_HABITAT, IS_WALKABLE, NEEDS_BREATH, NO_SHADOW_PART, PREYS_ON_MAN, WITHIN_PREATTACK_RANGE,
    },
    ztui_animalinfo::FORCE_UPDATE as ANIMALINFO_FORCE_UPDATE,
    ztunit::{IS_DOING_SOMETHING as ZTUNIT_IS_DOING_SOMETHING, CLIFFS_OK as ZTUNIT_CLIFFS_OK, GET_HABITAT as ZTUNIT_GET_HABITAT, GET_IDLE_ANIM as ZTUNIT_GET_IDLE_ANIM},
};
use glam::i32::IVec3;
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{ftol_low32, get_from_memory, ref_from_memory, save_to_memory},
    zthabitat::ZTHabitat,
    ztkeeper::{call_slot_0, unit_offsets},
};

/// `ZTAnimalType::createEntity` allocates this many bytes.
pub const ANIMAL_SIZE: u32 = 0x3b8;

/// Vtable address of `ZTAnimal` (`0x0062ff54`) as an RVA from the `zoo.exe` base.
pub const ANIMAL_VTABLE_RVA: u32 = 0x0022_ff54;

/// Class tag (RVA) the entity type's `isCastClass` takes to test for an animal type (`CAST_ZTAnimal`).
pub const ANIMAL_TYPE_TAG_RVA: u32 = 0x0023_8690;

/// Fields of an animal past the `ZTUnit` prefix. Roles marked "tentative" rest on a name or a neighbour only.
pub mod animal_offsets {
    /// Happiness (`i32`), clamped each tick to the type's min/max; pending change at `+0x2ac`.
    pub const HAPPINESS: u32 = 0x2a8;
    pub const HAPPINESS_PENDING: u32 = 0x2ac;
    /// Hunger (`i32`); pending change at `+0x2b4`.
    pub const HUNGER: u32 = 0x2b0;
    /// Health (`i32`); pending change at `+0x2bc`.
    pub const HEALTH: u32 = 0x2b8;
    /// The prey unit id (`clearPrey` zeroes it, `getPreyUnit` resolves it). Tentative.
    pub const PREY_ID: u32 = 0x2e4;
    /// Breath value `needsBreath` compares against the type's threshold.
    pub const BREATH: u32 = 0x360;
    /// Status flag words (low and critical bits for the happiness/hunger/health/... conditions); `setMaxHealth`
    /// clears bit 1 of the first and third and passes the fourth to `animalinfo::forceUpdate`.
    pub const STATUS_FLAGS: u32 = 0x298;
    pub const STATUS_FLAGS_CRITICAL: u32 = 0x2a0;
    pub const STATUS_FLAGS_UI: u32 = 0x2a4;
    /// Byte `setMaxHealth` clears.
    pub const FLAG_0X3A7: u32 = 0x3a7;
    /// Mate's entity id; cleared on both animals when the mate is gone.
    pub const MATE_ID: u32 = 0x2e8;
    /// Habitat the animal last recorded (`calcHabitat`).
    pub const CACHED_HABITAT: u32 = 0x388;
    /// The home habitat's gate tile (`setHomeHabitat`).
    pub const HOME_GATE_TILE: u32 = 0x3b4;
    /// Escape-info record pointer: `+0x8` is the tile the animal is heading for.
    pub const ESCAPE_RECORD: u32 = 0x384;
    /// Eat-action context: `+0x10` is the food entity.
    pub const EAT_CONTEXT: u32 = 0x38c;
    /// Byte set once the animal has been captured by a keeper (`canCapture`), and the flag byte next to it.
    pub const CAPTURE_FLAG: u32 = 0x3a3;
    pub const CAPTURE_STAGE: u32 = 0x393;
    /// Word `canSleepBeSickEat` compares with `1` and `2`.
    pub const ACTIVITY_MODE: u32 = 0x380;
    /// Bytes counted into the angry/sick neighbour totals.
    pub const ANGRY: u32 = 0x3aa;
    pub const SICK: u32 = 0x3a7;
    /// Set while the animal is in a box; `unbox` clears it.
    pub const BOXED: u32 = 0x394;
    /// Set while the animal is still an egg (`isEgg`).
    pub const EGG: u32 = 0x395;
    /// "Has escape info" flag; `diagonalOK` is its negation.
    pub const HAS_ESCAPE_INFO: u32 = 0x391;
    /// Cleared for animals the player can select (`isSelectable`).
    pub const NOT_SELECTABLE: u32 = 0x3a6;
}

/// `ZTUnit` fields the predicates read.
mod unit_fields {
    /// `BFEntity` byte `isUnhappyWithHabitat` and `getPreyUnit` treat as "removed".
    pub const REMOVED_A: u32 = 0x143;
    pub const REMOVED_B: u32 = 0x142;
    /// The unit's movement mode word `needsBreath` and `noShadowPart` compare (`4` underwater, `3` flying).
    pub const TERRAIN_MODE: u32 = 0x258;
    /// Begin and end pointers of the current behaviour name.
    pub const BEHAVIOUR_BEGIN: u32 = 0x54;
    pub const BEHAVIOUR_END: u32 = 0x58;
}

/// Behaviour names `isDoingSomething` matches, as RVAs of their C strings.
mod strings_rva {
    pub const SICK: u32 = 0x0023_c2cc;
    pub const SLEEP: u32 = 0x0023_c2d4;
    pub const EAT: u32 = 0x0023_c2e8;
    pub const TEST_FENCE: u32 = 0x0023_9514;
}

/// `ZTAnimalType` fields the predicates read.
mod type_offsets {
    /// Byte: the animal can be walked through by other units (`isWalkable`).
    pub const WALKABLE: u32 = 0x56;
    pub const BREATH_THRESHOLD: u32 = 0x1fc;
    pub const HUNGER_THRESHOLD: u32 = 0x20c;
    pub const HAPPINESS_MIN: u32 = 0x22c;
    pub const HAPPINESS_MAX: u32 = 0x230;
    pub const MAX_HITS: u32 = 0x234;
    pub const SPECIES: u32 = 0x1ec;
    /// Figure `fTilesPerAdultAnimal` returns when the habitat gives no ratio.
    pub const TILES_PER_ADULT: u32 = 0x314;
    /// Icon zoom and idle animations: the plain zoom, and the box and egg variants.
    pub const ICON_ZOOM: u32 = 0x50;
    pub const BOX_ICON_ZOOM: u32 = 0x3f0;
    pub const EGG_ICON_ZOOM: u32 = 0x40c;
    pub const BOX_IDLE_ANIM: u32 = 0x484;
    pub const EGG_IDLE_ANIM: u32 = 0x488;
    pub const HEALTH_MIN: u32 = 0x238;
    pub const FOOD_UNITS_EATEN: u32 = 0x21c;
    pub const SPACE_NEEDED: u32 = 0x298;
    pub const KEEPER_FOOD_CATEGORY: u32 = 0x3c8;
    pub const SERVICE_TYPE: u32 = 0x2f0;
    pub const CLIFF_MODE: u32 = 0x3b0;
    pub const CAPTURE_TWO_STAGE: u32 = 0x3d7;
    pub const HUNGER_MAX: u32 = 0x240;
    pub const HUNGER_MIN: u32 = 0x244;
    pub const HABITAT_PREFERENCE: u32 = 0x2bc;
    /// Begin and end of the type's prey-id list (`i32`s).
    pub const PREY_LIST_BEGIN: u32 = 0x2e4;
    pub const PREY_LIST_END: u32 = 0x2e8;
    /// Multiplied with the habitat preference to give the "very unhappy" rating floor.
    pub const VERY_UNHAPPY_FACTOR: u32 = 0x310;
    pub const HAPPY_REPRODUCE_THRESHOLD: u32 = 0x378;
    /// Byte: the type attacks before reaching its target (`isPreattack`).
    pub const PREATTACK: u32 = 0x3d6;
    /// Byte: the animal keeps moving while idling (`getKeepMoving`).
    pub const KEEP_MOVING: u32 = 0x44c;
}

/// Vtable slots (byte offsets) dispatched through so subclass overrides are honoured.
mod slots {
    /// `isCastClass` on an entity type.
    pub const TYPE_IS_CAST_CLASS: u32 = 0x1c;
    /// `ZTUnit::getHabitat`-style accessor: the habitat the unit stands in.
    pub const HABITAT: u32 = 0x24c;
    /// `ZTUnit::getHabitatFromVA`-style resolver: takes the home gate tile and returns the habitat.
    pub const HABITAT_FROM_TILE: u32 = 0x248;
    /// Entity type: the animal's own type id (`canCapture`, `canService` compare it against type ids).
    pub const TYPE_ID: u32 = 0x20;
    /// Entity type: the sell-price base (`getSellPrice`, returns a float in `ST0`).
    pub const TYPE_SELL_BASE: u32 = 0xbc;
    /// Entity type: whether the type has the keeper-food predicate (`fIsThereKeeperFood`).
    pub const TYPE_KEEPER_FOOD_FLAG: u32 = 0xcc;
    /// Entity type: `isWalkableBy(other)`.
    pub const TYPE_IS_WALKABLE_BY: u32 = 0x40;
    /// `BFWorldMgr::getEntity(id, flag)`.
    pub const WORLD_GET_ENTITY: u32 = 0x24;
    /// Animal vtable `getIdleAnim(bool)`.
    pub const IDLE_ANIM: u32 = 0x8c;
    /// Slots `canSleepBeSickEat` queries.
    pub const SLEEP_GATE_A: u32 = 0x170;
    pub const SLEEP_GATE_B: u32 = 0x2c8;
    pub const SLEEP_GATE_C: u32 = 0x22c;
}

/// Class tags as RVAs.
mod tags {
    pub const BUILDING: u32 = 0x0023_8680;
    pub const UNIT: u32 = 0x0023_86b0;
    pub const FOOD_ENTITY: u32 = 0x0023_86c0;
}

/// `getKeepMoving`'s goal ids: idling and wandering.
const GOAL_IDLE: i32 = 1;
/// `CAST_ZTAnimal` as an RVA.
const ANIMAL_TAG: u32 = ANIMAL_TYPE_TAG_RVA;
const GOAL_WANDER: i32 = 0x72;
/// The prey id `preysOnMan` looks for: the type id of a person.
const MAN_TYPE_ID: i32 = 0x251f;

/// Globals, as RVAs from the `zoo.exe` base.
mod globals_rva {
    /// `DAT_006386f8`: the percentage of max health below which an animal counts as sickly (clamped to `0..=100`).
    pub const SICK_PERCENT: u32 = 0x0023_86f8;
    /// `DAT_00641560`: the preattack radius in tiles.
    pub const PREATTACK_RADIUS: u32 = 0x0024_1560;
    /// `DAT_00630d5c`: the float `0.0` the tile and rating ratios are compared against.
    pub const FLOAT_ZERO: u32 = 0x0023_0d5c;
    /// `DAT_00635490`: the floor `fAnimalSpaceRatio` clamps to.
    pub const SPACE_RATIO_FLOOR: u32 = 0x0023_5490;
    /// `getSellPrice` constants: `DAT_00634eb8`, `DAT_00630d64`, `DAT_00635060`, `DAT_00630d54`.
    pub const SELL_SCALE: u32 = 0x0023_4eb8;
    pub const SELL_HAPPY_OFFSET: u32 = 0x0023_0d64;
    pub const SELL_HAPPY_WEIGHT: u32 = 0x0023_5060;
    pub const SELL_SICK_FACTOR: u32 = 0x0023_0d54;
    /// `GLOBAL_ZTWorldMgr` (the global holds the world manager pointer).
    pub const WORLD_MGR: u32 = 0x0023_8040;
    /// `DAT_0063afc8`: the need `isNeedSatisfier` asks buildings to satisfy.
    pub const NEED_ARG: u32 = 0x0023_afc8;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// Whether the entity type at `entity_type` passes `isCastClass(CAST_ZTAnimal)`.
fn type_is_animal(entity_type: u32) -> bool {
    let function = unsafe {
        transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slots::TYPE_IS_CAST_CLASS))
    };
    function(entity_type, base() + ANIMAL_TYPE_TAG_RVA)
}

/// `(value - low) * 100 / (high - low)`, `0` for a degenerate range (vanilla faults on the division).
fn percent(value: i32, low: i32, high: i32) -> i32 {
    let range = high.wrapping_sub(low);
    if range == 0 {
        return 0;
    }
    value.wrapping_sub(low).wrapping_mul(100).wrapping_div(range)
}

/// An animal in vanilla memory. All reads and writes are volatile through the raw address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTAnimal(pub u32);

#[allow(dead_code)]
impl ZTAnimal {
    fn read<T: Copy>(self, offset: u32) -> T {
        get_from_memory(self.0 + offset)
    }

    fn flag(self, offset: u32) -> bool {
        self.read::<u8>(offset) != 0
    }

    pub fn happiness(self) -> i32 {
        self.read(animal_offsets::HAPPINESS)
    }

    pub fn hunger(self) -> i32 {
        self.read(animal_offsets::HUNGER)
    }

    pub fn health(self) -> i32 {
        self.read(animal_offsets::HEALTH)
    }

    /// The animal's type when it is non-null and passes `isCastClass(CAST_ZTAnimal)`; every typed read in the
    /// vanilla bodies goes through this check.
    pub fn animal_type(self) -> Option<u32> {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        (entity_type != 0 && type_is_animal(entity_type)).then_some(entity_type)
    }

    fn type_field<T: Copy>(entity_type: u32, offset: u32) -> T {
        get_from_memory(entity_type + offset)
    }

    /// `ZTAnimal::isHungry` the hunger value has reached the type's threshold.
    pub fn is_hungry(self) -> bool {
        self.animal_type().is_some_and(|ty| self.hunger() >= Self::type_field::<i32>(ty, type_offsets::HUNGER_THRESHOLD))
    }

    /// `ZTAnimal::isHungryAndFoodless`: hungry and `fIsThereKeeperFood` reports no food.
    pub fn is_hungry_and_foodless(self) -> bool {
        self.is_hungry() && !self.f_is_there_keeper_food()
    }

    pub fn get_happy_pct(self) -> i32 {
        self.animal_type().map_or(0, |ty| {
            percent(self.happiness(), Self::type_field(ty, type_offsets::HAPPINESS_MIN), Self::type_field(ty, type_offsets::HAPPINESS_MAX))
        })
    }

    pub fn get_hunger_pct(self) -> i32 {
        self.animal_type()
            .map_or(0, |ty| percent(self.hunger(), Self::type_field(ty, type_offsets::HUNGER_MIN), Self::type_field(ty, type_offsets::HUNGER_MAX)))
    }

    pub fn get_health_pct(self) -> i32 {
        self.animal_type()
            .map_or(0, |ty| percent(self.health(), Self::type_field(ty, type_offsets::HEALTH_MIN), Self::type_field(ty, type_offsets::MAX_HITS)))
    }

    /// `ZTAnimal::isUnhappyForReproduction`: happiness at or below the type's reproduction threshold.
    pub fn is_unhappy_for_reproduction(self) -> bool {
        self.animal_type().is_some_and(|ty| self.happiness() <= Self::type_field::<i32>(ty, type_offsets::HAPPY_REPRODUCE_THRESHOLD))
    }

    /// The rating of the habitat the animal stands in (`getHabitatRating` through vanilla, on the habitat from the
    /// vtable's habitat slot).
    fn habitat_rating(self) -> i32 {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        self.habitat_rating_in(habitat)
    }

    /// `ZTAnimal::getHabitatRating`: `ZTHabitat::getHabitatRating` (floored at the zero constant, rounded up by the CRT helper at `0x415f2e`
    /// (control word `0x1b3f`), then truncated), or `0` for a non-animal type, a null habitat or a flagged one.
    pub fn habitat_rating_in(self, habitat: u32) -> i32 {
        if self.animal_type().is_none() || habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            return 0;
        }
        let mut rating = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.habitat_rating_unrounded(self.0, true);
        let floor = f64::from(get_from_memory::<f32>(base() + globals_rva::FLOAT_ZERO));
        if rating < floor {
            rating = floor;
        }
        let rounded = rating.ceil();
        if !(-(2f64.powi(63))..2f64.powi(63)).contains(&rounded) {
            return 0;
        }
        (rounded as i64) as i32
    }

    /// `ZTAnimal::isUnhappyWithHabitat`: false for a removed, boxed-out or egg animal, otherwise the habitat rating
    /// is below the type's habitat preference.
    pub fn is_unhappy_with_habitat(self) -> bool {
        if self.flag(unit_fields::REMOVED_A) || self.flag(unit_fields::REMOVED_B) || self.is_egg() != 0 {
            return false;
        }
        match self.animal_type() {
            Some(ty) => self.habitat_rating() < Self::type_field::<i32>(ty, type_offsets::HABITAT_PREFERENCE),
            None => false,
        }
    }

    /// `ZTAnimal::isVeryUnhappyWithHabitat`: `rating * 100` below the type's factor times its habitat preference.
    pub fn is_very_unhappy_with_habitat(self) -> bool {
        let Some(ty) = self.animal_type() else {
            return false;
        };
        let floor = Self::type_field::<i32>(ty, type_offsets::VERY_UNHAPPY_FACTOR).wrapping_mul(Self::type_field::<i32>(ty, type_offsets::HABITAT_PREFERENCE));
        self.habitat_rating().wrapping_mul(100) < floor
    }

    /// `ZTAnimal::isSickly`: `health * 100 <= maxHits * (100 - p)`, where `p` is the global sick percentage clamped
    /// to `0..=100`.
    pub fn is_sickly(self) -> bool {
        let percent_global: i32 = get_from_memory(base() + globals_rva::SICK_PERCENT);
        let sick_percent = percent_global.clamp(0, 100);
        let Some(ty) = self.animal_type() else {
            return false;
        };
        let max_hits = Self::type_field::<i32>(ty, type_offsets::MAX_HITS);
        self.health().wrapping_mul(100) <= max_hits.wrapping_mul(100 - sick_percent)
    }

    pub fn is_egg(self) -> u8 {
        self.read(animal_offsets::EGG)
    }

    pub fn is_selectable(self) -> bool {
        !self.flag(animal_offsets::NOT_SELECTABLE)
    }

    pub fn is_boxed(self) -> bool {
        self.flag(animal_offsets::BOXED)
    }

    /// `ZTAnimal::isInPreyList`: whether `id` is in the type's prey list.
    pub fn is_in_prey_list(self, id: i32) -> bool {
        let Some(ty) = self.animal_type() else {
            return false;
        };
        let begin: u32 = Self::type_field(ty, type_offsets::PREY_LIST_BEGIN);
        let end: u32 = Self::type_field(ty, type_offsets::PREY_LIST_END);
        (begin..end).step_by(4).any(|slot| get_from_memory::<i32>(slot) == id)
    }

    /// `ZTAnimal::preysOnMan`: not boxed, a person is on the type's prey list, and not an egg.
    pub fn preys_on_man(self) -> bool {
        !self.is_boxed() && self.is_in_prey_list(MAN_TYPE_ID) && self.is_egg() == 0
    }

    fn type_preattacks(entity_type: u32) -> bool {
        Self::type_field::<u8>(entity_type, type_offsets::PREATTACK) != 0
    }

    /// `ZTAnimal::isPreattack`: both animals' types preattack, and `other` is not an egg.
    pub fn is_preattack(self, other: u32) -> bool {
        if other == 0 {
            return false;
        }
        let Some(own_type) = self.animal_type() else {
            return false;
        };
        if !Self::type_preattacks(own_type) {
            return false;
        }
        let other = ZTAnimal(other);
        let Some(other_type) = other.animal_type() else {
            return false;
        };
        other.is_egg() == 0 && Self::type_preattacks(other_type)
    }

    /// `ZTAnimal::withinPreattackRange`: `isPreattack(other)` and the squared tile distance between the two animals
    /// is within the preattack radius squared. A missing tile counts as `i32::MAX`.
    pub fn within_preattack_range(self, other: u32) -> bool {
        if other == 0 || !self.is_preattack(other) {
            return false;
        }
        let other_tile = unsafe { GET_TILE.original()(other as *const u32) } as u32;
        let own_tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        let distance_squared = if other_tile == 0 || own_tile == 0 {
            i32::MAX
        } else {
            let dx = get_from_memory::<i32>(own_tile + 0x34).wrapping_sub(get_from_memory::<i32>(other_tile + 0x34));
            let dy = get_from_memory::<i32>(own_tile + 0x38).wrapping_sub(get_from_memory::<i32>(other_tile + 0x38));
            dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
        };
        let radius: i32 = get_from_memory(base() + globals_rva::PREATTACK_RADIUS);
        distance_squared <= radius.wrapping_mul(radius)
    }

    pub fn get_ambient_key(self) -> u32 {
        self.happiness() as u32
    }

    /// `ZTAnimal::diagonalOK`: animals with escape info may not cut corners.
    pub fn diagonal_ok(self) -> bool {
        !self.flag(animal_offsets::HAS_ESCAPE_INFO)
    }

    /// `ZTAnimal::isWalkable`: not boxed and the type allows walking through.
    pub fn is_walkable(self) -> bool {
        if self.is_boxed() {
            return false;
        }
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        entity_type != 0 && Self::type_field::<u8>(entity_type, type_offsets::WALKABLE) != 0
    }

    pub fn clear_prey(self) {
        save_to_memory(self.0 + animal_offsets::PREY_ID, 0u32);
    }

    /// `ZTAnimal::noShadowPart`: neither underwater (`4`) nor flying (`3`) and not boxed.
    pub fn no_shadow_part(self) -> bool {
        let mode: u32 = self.read(unit_fields::TERRAIN_MODE);
        mode != 4 && mode != 3 && !self.is_boxed()
    }

    /// `ZTAnimal::needsBreath`: underwater and the breath value has reached the type's threshold.
    pub fn needs_breath(self) -> bool {
        if self.read::<u32>(unit_fields::TERRAIN_MODE) != 4 {
            return false;
        }
        self.animal_type().is_some_and(|ty| self.read::<i32>(animal_offsets::BREATH) >= Self::type_field::<i32>(ty, type_offsets::BREATH_THRESHOLD))
    }

    /// The habitat the animal stands in (vtable `+0x24c`), if any.
    fn habitat(self) -> Option<&'static ZTHabitat> {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        (habitat != 0).then(|| unsafe { ref_from_memory::<ZTHabitat>(habitat) })
    }

    pub fn f_num_animals(self) -> i32 {
        self.habitat().map_or(0, |habitat| habitat.get_num_animals(true))
    }

    pub fn f_num_same_animals(self) -> i32 {
        match (self.habitat(), self.animal_type()) {
            (Some(habitat), Some(ty)) => habitat.get_num_animals_by_species(Self::type_field(ty, type_offsets::SPECIES), true),
            _ => 0,
        }
    }

    pub fn f_num_same_adult_animals(self) -> i32 {
        match (self.habitat(), self.animal_type()) {
            (Some(habitat), Some(ty)) => habitat.get_num_adult_animals_by_species(Self::type_field(ty, type_offsets::SPECIES), true),
            _ => 0,
        }
    }

    /// `ZTAnimal::fTilesPerAdultAnimal`: habitat size per adult animal, or the type's own figure when the habitat has
    /// no adults or gives a zero ratio.
    pub fn f_tiles_per_adult_animal(self) -> i32 {
        let zero: f32 = get_from_memory(base() + globals_rva::FLOAT_ZERO);
        if let Some(habitat) = self.habitat() {
            let adults = habitat.get_num_adult_animals(true);
            let size = habitat.get_size(true);
            if adults as f32 != zero {
                let ratio = size as f32 / adults as f32;
                if ratio != zero {
                    return ftol_low32(ratio);
                }
            }
        }
        let fallback = self.animal_type().map_or(0, |ty| Self::type_field::<i32>(ty, type_offsets::TILES_PER_ADULT));
        ftol_low32(fallback as f32)
    }

    /// `ZTAnimal::fCompatAnimalsRating`: the habitat's compatible-animal rating for the animal's species, truncated.
    pub fn f_compat_animals_rating(self) -> i32 {
        let Some(ty) = self.animal_type() else {
            return 0;
        };
        let species: i32 = Self::type_field(ty, type_offsets::SPECIES);
        self.habitat().map_or(0, |habitat| ftol_low32(habitat.get_compatible_animal_rating(species)))
    }

    /// `ZTAnimal::getIconZoom`: the box zoom while boxed, the egg zoom while an egg, else the type's plain zoom.
    pub fn get_icon_zoom(self) -> i32 {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if self.is_boxed() {
            return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::BOX_ICON_ZOOM));
        }
        if self.is_egg() != 0 {
            return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::EGG_ICON_ZOOM));
        }
        Self::type_field(entity_type, type_offsets::ICON_ZOOM)
    }

    /// `ZTAnimal::getIdleAnim`: unless `force_unit` is set, an egg or boxed animal uses the type's own animation;
    /// everything else is `ZTUnit::getIdleAnim`.
    pub fn get_idle_anim(self, force_unit: bool) -> i32 {
        if !force_unit {
            if self.is_egg() != 0 {
                return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::EGG_IDLE_ANIM));
            }
            if self.is_boxed() {
                return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::BOX_IDLE_ANIM));
            }
        }
        unsafe { ZTUNIT_GET_IDLE_ANIM.original()(self.0 as *const u32, force_unit) }
    }

    /// `ZTAnimal::setMaxHealth`: health to the type's maximum, clear the sick marker and the bit-1 status flags, and
    /// refresh the animal info panel. Vanilla passes the value of `+0x2a4` as the panel's `this`.
    pub fn set_max_health(self) {
        let max_hits = self.animal_type().map_or(0, |ty| Self::type_field::<i32>(ty, type_offsets::MAX_HITS));
        save_to_memory(self.0 + animal_offsets::HEALTH, max_hits);
        save_to_memory(self.0 + animal_offsets::FLAG_0X3A7, 0u8);
        let flags: u32 = self.read(animal_offsets::STATUS_FLAGS);
        save_to_memory(self.0 + animal_offsets::STATUS_FLAGS, flags & !2);
        let critical: u32 = self.read(animal_offsets::STATUS_FLAGS_CRITICAL);
        save_to_memory(self.0 + animal_offsets::STATUS_FLAGS_CRITICAL, critical & !2);
        let panel: u32 = self.read(animal_offsets::STATUS_FLAGS_UI);
        unsafe { ANIMALINFO_FORCE_UPDATE.original()(panel as *const c_void) };
    }

    /// The world manager (`GLOBAL_ZTWorldMgr`'s value).
    fn world() -> u32 {
        get_from_memory(base() + globals_rva::WORLD_MGR)
    }

    /// `BFWorldMgr::getEntity(id, true)` through the world's vtable.
    fn world_entity(world: u32, id: u32) -> u32 {
        let function = unsafe {
            transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(world) + slots::WORLD_GET_ENTITY))
        };
        function(world, id, 1)
    }

    /// Whether the entity's type passes `isCastClass` for the class tag at `tag_rva`.
    fn entity_is(entity: u32, tag_rva: u32) -> bool {
        let entity_type: u32 = get_from_memory(entity + unit_offsets::ENTITY_TYPE);
        let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slots::TYPE_IS_CAST_CLASS)) };
        function(entity_type, base() + tag_rva)
    }

    /// Calls a vanilla `ZTUnit` method declared without a return in `generated.rs` but returning its result in `AL`/`EAX`.
    fn call_unit_u32(address: u32, this: u32) -> u32 {
        unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(address)(this) }
    }

    /// `ZTAnimal::getSwims`: an egg never swims, everything else is `ZTUnit::getSwims` (`0x00410764`).
    pub fn get_swims(self) -> u32 {
        if self.is_egg() != 0 {
            return 0;
        }
        Self::call_unit_u32(0x0041_0764, self.0)
    }

    /// `ZTAnimal::getUnderwater`: an egg is never underwater, everything else is `ZTUnit::getUnderwater` (`0x00410794`).
    pub fn get_underwater(self) -> u32 {
        if self.is_egg() != 0 {
            return 0;
        }
        Self::call_unit_u32(0x0041_0794, self.0)
    }

    /// `ZTAnimal::isWalkableBy`: not boxed and the type's walkable-by slot accepts `other`.
    pub fn is_walkable_by(self, other: u32) -> bool {
        if self.is_boxed() {
            return false;
        }
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let function = unsafe {
            transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slots::TYPE_IS_WALKABLE_BY))
        };
        function(entity_type, other) & 0xff != 0
    }

    /// `ZTAnimal::cliffsOK`: a type with cliff mode `1` may step between neighbouring tiles that are not flagged `0x20`
    /// and differ by at most one level; everything else is `ZTUnit::cliffsOK`.
    pub fn cliffs_ok(self, tile: u32, direction: u32) -> bool {
        if self.animal_type().is_some_and(|ty| Self::type_field::<i32>(ty, type_offsets::CLIFF_MODE) == 1) {
            let map = Self::world() + 8;
            let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()(map as *const u32, tile as *const u32, direction) };
            if get_from_memory::<u8>(tile + 0x85) & 0x20 == 0
                && get_from_memory::<u8>(neighbour + 0x85) & 0x20 == 0
                && get_from_memory::<i32>(tile + 0x3c).wrapping_sub(get_from_memory::<i32>(neighbour + 0x3c)).wrapping_abs() <= 1
            {
                return true;
            }
        }
        unsafe { ZTUNIT_CLIFFS_OK.original()(self.0 as *const u32, tile as *const u32, direction) }
    }

    /// `ZTAnimal::getPreyUnit`: the prey id resolved to a live, non-removed unit, else `0`.
    pub fn get_prey_unit(self) -> u32 {
        let id: u32 = self.read(animal_offsets::PREY_ID);
        if id == 0 {
            return 0;
        }
        let entity = Self::world_entity(Self::world(), id);
        if entity == 0 || !Self::entity_is(entity, tags::UNIT) {
            return 0;
        }
        if get_from_memory::<u8>(entity + unit_fields::REMOVED_A) != 0 || get_from_memory::<u8>(entity + unit_fields::REMOVED_B) != 0 {
            return 0;
        }
        entity
    }

    /// `ZTAnimal::fMateGone`: no mate recorded is not gone. A mate that is not an animal any more is gone. A mate in a
    /// different habitat (or none) is gone and both animals forget each other; one in the same habitat, or in a flagged
    /// habitat, is not gone.
    pub fn f_mate_gone(self) -> bool {
        let mate: u32 = self.read(animal_offsets::MATE_ID);
        if mate == 0 {
            return false;
        }
        let world = Self::world();
        if world == 0 {
            return false;
        }
        let entity = Self::world_entity(world, mate);
        if entity == 0 || !Self::entity_is(entity, ANIMAL_TAG) {
            return true;
        }
        let own_habitat = call_slot_0(self.0, slots::HABITAT);
        let mate_habitat = call_slot_0(entity, slots::HABITAT);
        if mate_habitat != 0 && (get_from_memory::<u8>(mate_habitat + 0x2c) != 0 || mate_habitat == own_habitat) {
            return false;
        }
        save_to_memory(self.0 + animal_offsets::MATE_ID, 0u32);
        save_to_memory(entity + animal_offsets::MATE_ID, 0u32);
        true
    }

    /// `ZTAnimal::fMateAvailable`: `fGetMate(flag)` (still vanilla) found a mate.
    pub fn f_mate_available(self, flag: *const bool) -> bool {
        unsafe { F_GET_MATE.original()(self.0 as *const u32, flag) != 0 }
    }

    /// The angry or sick count shared by `fOtherAnimalAngry`/`fOtherAnimalSick`: `count` capped at the habitat's animal
    /// total (falling back to `fNumAnimals`), less this animal when it is itself in that state.
    fn others(self, count: i32, own_flag: bool) -> bool {
        let mut count = count;
        let total = self.habitat().map_or(0, |habitat| habitat.get_num_animals(true));
        if count > total {
            count = self.f_num_animals();
        }
        if own_flag {
            count -= 1;
        }
        count > 0
    }

    pub fn f_other_animal_angry(self) -> bool {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            return false;
        }
        let angry = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_num_angry_animals(true);
        self.others(angry, self.flag(animal_offsets::ANGRY))
    }

    pub fn f_other_animal_sick(self) -> bool {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat == 0 {
            return false;
        }
        let sick = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_num_sick_animals(true);
        self.others(sick, self.flag(animal_offsets::SICK))
    }

    /// `ZTAnimal::fAnimalSpaceRatio`: habitat size over the type's space need, never below the floor constant; `1.0`
    /// without a habitat, type or size.
    pub fn f_animal_space_ratio(self) -> f32 {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        let (Some(ty), true) = (self.animal_type(), habitat != 0) else {
            return 1.0;
        };
        let size = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_size(true);
        let zero: f32 = get_from_memory(base() + globals_rva::FLOAT_ZERO);
        if size as f32 == zero {
            return 1.0;
        }
        let ratio = size as f32 / Self::type_field::<i32>(ty, type_offsets::SPACE_NEEDED) as f32;
        let floor: f32 = get_from_memory(base() + globals_rva::SPACE_RATIO_FLOOR);
        if ratio < floor {
            floor
        } else {
            ratio
        }
    }

    /// `ZTAnimal::fFoodUnitsActuallyEaten`: the smaller of the type's units-per-meal and the food entity's units.
    pub fn f_food_units_actually_eaten(self) -> i32 {
        let Some(ty) = self.animal_type() else {
            return 0;
        };
        let context: u32 = self.read(animal_offsets::EAT_CONTEXT);
        if context == 0 {
            return 0;
        }
        let food: u32 = get_from_memory(context + 0x10);
        if food == 0 || !Self::entity_is(food, tags::FOOD_ENTITY) {
            return 0;
        }
        let eaten = Self::type_field::<i32>(ty, type_offsets::FOOD_UNITS_EATEN);
        let available: i32 = get_from_memory(food + 0x154);
        if eaten >= available {
            available
        } else {
            eaten
        }
    }

    /// `ZTAnimal::fIsThereKeeperFood`: the habitat holds keeper food of the type's category.
    pub fn f_is_there_keeper_food(self) -> bool {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat == 0 {
            return false;
        }
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let ty = self.animal_type();
        let flag = call_slot_0(if ty.is_some() { entity_type } else { 0 }, slots::TYPE_KEEPER_FOOD_FLAG) & 0xff != 0;
        let category = ty.map_or(0, |ty| Self::type_field::<u32>(ty, type_offsets::KEEPER_FOOD_CATEGORY));
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_amount_keeper_food(category, flag) > 0
    }

    /// `ZTAnimal::checkDone`: an animal heading for its escape tile that has reached it drops the escape record.
    pub fn check_done(self) -> u32 {
        if self.read::<i32>(unit_offsets::GOAL_ID) == 1 && self.flag(animal_offsets::HAS_ESCAPE_INFO) {
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            let target: u32 = get_from_memory(record + 8);
            let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
            if tile == target {
                save_to_memory(self.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
                unsafe { OPERATOR_DELETE.original()(record) };
                save_to_memory(self.0 + animal_offsets::ESCAPE_RECORD, 0u32);
            }
            return 1;
        }
        unsafe { BFUNIT_CHECK_DONE.original()(self.0 as *const u32) }
    }

    /// `ZTAnimal::canCapture`: whether a keeper holding a `other`-typed capture tool can capture this animal on its
    /// current tile.
    pub fn can_capture(self, other: u32) -> bool {
        if other == 0 {
            return false;
        }
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        let ty = self.animal_type();
        let other_type: u32 = get_from_memory(other + unit_offsets::ENTITY_TYPE);
        let code = call_slot_0(other_type, slots::TYPE_ID);
        if !self.flag(animal_offsets::CAPTURE_FLAG) {
            return false;
        }
        let Some(ty) = ty else {
            return false;
        };
        let on_clear_tile = || tile != 0 && get_from_memory::<u8>(tile + 0x83) & 3 == 0;
        if Self::type_field::<u8>(ty, type_offsets::CAPTURE_TWO_STAGE) == 0 {
            return code == 0x251c && on_clear_tile();
        }
        let staged = self.flag(animal_offsets::CAPTURE_STAGE);
        match code {
            0x254e => !staged && on_clear_tile(),
            0x254f => staged && on_clear_tile(),
            _ => false,
        }
    }

    /// `ZTAnimal::isNeedSatisfier`: the building on the megatile cell of `pos` satisfies the animal's need.
    pub fn is_need_satisfier(tile: u32, pos: u32) -> bool {
        if tile == 0 {
            return false;
        }
        let x: i32 = get_from_memory(pos);
        let y: i32 = get_from_memory(pos + 4);
        if !(0..0x40).contains(&x) || !(0..0x40).contains(&y) {
            return false;
        }
        let index = (y >> 5) + (x >> 5) * 2;
        let entity: u32 = get_from_memory(tile + 4 + (index as u32) * 4);
        if entity == 0 || !Self::entity_is(entity, tags::BUILDING) {
            return false;
        }
        unsafe { ZTWORLDMGR_SATISFIES.original()(entity as i32, (base() + globals_rva::NEED_ARG) as *const i8, std::ptr::null()) & 0xff != 0 }
    }

    /// `ZTAnimal::canService`: the keeper's type id equals the animal type's service type. An egg that cannot be
    /// selected, or a missing keeper, cannot be serviced.
    pub fn can_service(self, staff: u32) -> bool {
        if self.is_egg() != 0 && !self.is_selectable() {
            return false;
        }
        if staff == 0 {
            return false;
        }
        let staff_type: u32 = get_from_memory(staff + unit_offsets::ENTITY_TYPE);
        let id = call_slot_0(staff_type, slots::TYPE_ID) as i32;
        self.animal_type().is_some_and(|ty| Self::type_field::<i32>(ty, type_offsets::SERVICE_TYPE) == id)
    }

    /// `ZTAnimal::canSleepBeSickEat`.
    pub fn can_sleep_be_sick_eat(self) -> bool {
        if call_slot_0(self.0, slots::SLEEP_GATE_A) & 0xff == 0 {
            let mode: i32 = self.read(animal_offsets::ACTIVITY_MODE);
            if mode == 1 {
                return false;
            }
            let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
            if tile == 0 || get_from_memory::<u8>(tile + 0x83) & 3 != 0 || mode == 2 {
                return false;
            }
            if call_slot_0(self.0, slots::SLEEP_GATE_B) != 0 {
                return false;
            }
        }
        self.is_egg() == 0 && call_slot_0(self.0, slots::SLEEP_GATE_C) & 0xff == 0
    }

    /// `ZTAnimal::getIconAnim`: the type's egg or box animation, else the idle animation.
    pub fn get_icon_anim(self) -> u32 {
        if self.is_egg() != 0 {
            return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::EGG_IDLE_ANIM));
        }
        if self.is_boxed() {
            return self.animal_type().map_or(0, |ty| Self::type_field(ty, type_offsets::BOX_IDLE_ANIM));
        }
        crate::ztkeeper::call_slot_1(self.0, slots::IDLE_ANIM, 1)
    }

    /// `ZTAnimal::getSellPrice`. The arithmetic runs on the x87 stack exactly as vanilla's does (the type's sell-base
    /// slot returns its float in `ST0`; the product and sums stay at the FPU's precision until the final store), so the
    /// result matches to the last bit.
    pub fn get_sell_price(self) -> f32 {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let base_price = unsafe {
            transmute::<u32, extern "thiscall" fn(u32) -> f32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slots::TYPE_SELL_BASE))(entity_type)
        };
        let constant = |rva: u32| get_from_memory::<f32>(base() + rva);
        // [base price, scale, health delta, max hits, happiness, happiness offset, happiness weight, factor, sick, result]
        let mut frame: [u32; 10] = [
            base_price.to_bits(),
            constant(globals_rva::SELL_SCALE).to_bits(),
            self.health().wrapping_sub(Self::type_field::<i32>(entity_type, type_offsets::HEALTH_MIN)) as u32,
            Self::type_field::<u32>(entity_type, type_offsets::MAX_HITS),
            self.happiness() as u32,
            constant(globals_rva::SELL_HAPPY_OFFSET).to_bits(),
            constant(globals_rva::SELL_HAPPY_WEIGHT).to_bits(),
            constant(globals_rva::SELL_SICK_FACTOR).to_bits(),
            self.flag(animal_offsets::SICK) as u32,
            0,
        ];
        unsafe {
            std::arch::asm!(
                "fld dword ptr [{f}]",
                "fmul dword ptr [{f} + 4]",
                "fild dword ptr [{f} + 8]",
                "fidiv dword ptr [{f} + 12]",
                "fild dword ptr [{f} + 16]",
                "fadd dword ptr [{f} + 20]",
                "fmul dword ptr [{f} + 24]",
                "faddp st(1), st",
                "fmulp st(1), st",
                "fmul dword ptr [{f} + 28]",
                "cmp dword ptr [{f} + 32], 0",
                "jz 2f",
                "fmul dword ptr [{f} + 28]",
                "2:",
                "fstp dword ptr [{f} + 36]",
                f = in(reg) frame.as_mut_ptr(),
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
                options(nostack),
            );
        }
        f32::from_bits(frame[9])
    }

    /// Whether the unit's current behaviour name (the string between `+0x54` and `+0x58`) equals the C string at
    /// `rva`.
    fn behaviour_is(self, rva: u32) -> bool {
        let expected = base() + rva;
        let length = (0..).take_while(|&i| get_from_memory::<u8>(expected + i) != 0).count() as u32;
        let begin: u32 = self.read(unit_fields::BEHAVIOUR_BEGIN);
        let end: u32 = self.read(unit_fields::BEHAVIOUR_END);
        end.wrapping_sub(begin) == length && (0..length).all(|i| get_from_memory::<u8>(begin + i) == get_from_memory::<u8>(expected + i))
    }

    /// `ZTAnimal::isDoingSomething`: `ZTUnit::isDoingSomething`, a sick/sleep/eat/test-fence behaviour, one of six busy
    /// flag bytes, or one of the show, escape and chase goals.
    pub fn is_doing_something(self) -> bool {
        let goal: i32 = self.read(unit_offsets::GOAL_ID);
        if unsafe { ZTUNIT_IS_DOING_SOMETHING.original()(self.0 as *const u32) } & 0xff != 0 {
            return true;
        }
        if [strings_rva::SICK, strings_rva::SLEEP, strings_rva::EAT, strings_rva::TEST_FENCE].into_iter().any(|rva| self.behaviour_is(rva)) {
            return true;
        }
        const BUSY_FLAGS: [u32; 7] = [0x39f, 0x3a0, 0x3a1, 0x3a5, 0x3a6, 0x3b1, 0x3a2];
        BUSY_FLAGS.into_iter().any(|offset| self.flag(offset)) || matches!(goal, 0x7a | 0x72 | 0x73 | 0x7c | 0x85 | 0x86)
    }

    /// `ZTAnimal::calcHabitat`: caches `ZTUnit::getHabitat` at `+0x388`.
    pub fn calc_habitat(self) {
        let habitat = unsafe { ZTUNIT_GET_HABITAT.original()(self.0 as *const u32) };
        save_to_memory(self.0 + animal_offsets::CACHED_HABITAT, habitat);
    }

    /// `ZTAnimal::getHabitat`: recomputes and returns the cached habitat.
    pub fn get_habitat(self) -> i32 {
        self.calc_habitat();
        self.read(animal_offsets::CACHED_HABITAT)
    }

    /// `ZTAnimal::setHomeHabitat`: the habitat's gate tile, or null.
    pub fn set_home_habitat(self, habitat: u32) {
        let gate = if habitat == 0 { 0 } else { (unsafe { GET_GATE_TILE_IN.hooked()(habitat as *const u32) }) as u32 };
        save_to_memory(self.0 + animal_offsets::HOME_GATE_TILE, gate);
    }

    /// `ZTAnimal::recalcHomeHabitat`: the most suitable habitat around the animal's own, unless that is a tank with
    /// `+0x4` set, in which case there is no home habitat.
    pub fn recalc_home_habitat(self) {
        let own = call_slot_0(self.0, slots::HABITAT);
        let mut home = if own == 0 { 0 } else { unsafe { ref_from_memory::<ZTHabitat>(own) }.get_most_suitable_habitat(self.0) };
        if home == 0 {
            home = own;
        }
        if home != 0 && call_slot_0(home, 0x20) & 0xff != 0 && get_from_memory::<u32>(home + 4) != 0 {
            home = 0;
        }
        self.set_home_habitat(home);
    }

    /// `ZTAnimal::getHomeHabitat`: recomputes the home habitat, then resolves its gate tile back to a habitat.
    pub fn get_home_habitat(self) -> u32 {
        self.recalc_home_habitat();
        crate::ztkeeper::call_slot_1(self.0, slots::HABITAT_FROM_TILE, self.read(animal_offsets::HOME_GATE_TILE))
    }

    /// `ZTAnimal::getUndesirableSceneryInHabitat`: writes into the `std::vector<void*>` at `out` a pool-allocated copy of
    /// the scenery the habitat reports as undesirable for this animal.
    pub fn get_undesirable_scenery_in_habitat(self, out: u32, flag: u32) -> u32 {
        let mut local: [u32; 3] = [0; 3];
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat != 0 && get_from_memory::<u8>(habitat + 0x2c) == 0 {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_undesirable_scenery(self.0, local.as_mut_ptr() as u32, flag != 0);
        }
        let (begin, end, capacity) = (local[0], local[1], local[2]);
        let count = (end.wrapping_sub(begin) as i32) >> 2;
        let storage = if count == 0 { 0 } else { (unsafe { POOLALLOC_ALLOCATE.original()((count as u32) * 4) }) as u32 };
        let mut cursor = storage;
        let mut source = begin;
        while source != end {
            if cursor != 0 {
                save_to_memory(cursor, get_from_memory::<u32>(source));
            }
            cursor = cursor.wrapping_add(4);
            source += 4;
        }
        save_to_memory(out, storage);
        save_to_memory(out + 4, cursor);
        save_to_memory(out + 8, storage.wrapping_add((count as u32).wrapping_mul(4)));
        if begin != 0 {
            let bytes = (((capacity.wrapping_sub(begin)) as i32 >> 2) as u32) << 2;
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
        }
        out
    }

    /// `ZTAnimal::getKeepMoving`: the type keeps moving while idle and the goal is idling or wandering.
    pub fn get_keep_moving(self) -> bool {
        let Some(ty) = self.animal_type() else {
            return false;
        };
        if Self::type_field::<u8>(ty, type_offsets::KEEP_MOVING) == 0 {
            return false;
        }
        let goal: i32 = self.read(unit_offsets::GOAL_ID);
        goal == GOAL_IDLE || goal == GOAL_WANDER
    }
}

#[detour_mod]
mod animal_detours {
    use super::*;

    #[detour(IS_HUNGRY)]
    unsafe extern "thiscall" fn is_hungry(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_hungry() as u32
    }

    #[detour(IS_HUNGRY_AND_FOODLESS)]
    unsafe extern "thiscall" fn is_hungry_and_foodless(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_hungry_and_foodless() as u32
    }

    #[detour(GET_HAPPY_PCT)]
    unsafe extern "thiscall" fn get_happy_pct(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_happy_pct()
    }

    #[detour(GET_HUNGER_PCT)]
    unsafe extern "thiscall" fn get_hunger_pct(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_hunger_pct()
    }

    #[detour(GET_HEALTH_PCT)]
    unsafe extern "thiscall" fn get_health_pct(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_health_pct()
    }

    #[detour(IS_UNHAPPY_FOR_REPRODUCTION)]
    unsafe extern "thiscall" fn is_unhappy_for_reproduction(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_unhappy_for_reproduction()
    }

    #[detour(IS_UNHAPPY_WITH_HABITAT)]
    unsafe extern "thiscall" fn is_unhappy_with_habitat(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_unhappy_with_habitat() as u32
    }

    #[detour(IS_VERY_UNHAPPY_WITH_HABITAT)]
    unsafe extern "thiscall" fn is_very_unhappy_with_habitat(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_very_unhappy_with_habitat() as u32
    }

    #[detour(IS_SICKLY)]
    unsafe extern "thiscall" fn is_sickly(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_sickly() as u32
    }

    #[detour(IS_EGG)]
    unsafe extern "thiscall" fn is_egg(this: *const c_void) -> u8 {
        ZTAnimal(this as u32).is_egg()
    }

    #[detour(IS_SELECTABLE)]
    unsafe extern "thiscall" fn is_selectable(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_selectable()
    }

    #[detour(PREYS_ON_MAN)]
    unsafe extern "thiscall" fn preys_on_man(this: *const u32) -> bool {
        ZTAnimal(this as u32).preys_on_man()
    }

    #[detour(IS_IN_PREY_LIST)]
    unsafe extern "thiscall" fn is_in_prey_list(this: *const u32, id: i32) -> u32 {
        ZTAnimal(this as u32).is_in_prey_list(id) as u32
    }

    #[detour(IS_PREATTACK)]
    unsafe extern "thiscall" fn is_preattack(this: *const u32, other: i32) -> u32 {
        ZTAnimal(this as u32).is_preattack(other as u32) as u32
    }

    #[detour(WITHIN_PREATTACK_RANGE)]
    unsafe extern "thiscall" fn within_preattack_range(this: *const u32, other: i32) -> u32 {
        ZTAnimal(this as u32).within_preattack_range(other as u32) as u32
    }

    #[detour(GET_AMBIENT_KEY)]
    unsafe extern "thiscall" fn get_ambient_key(this: *const u32) -> u32 {
        ZTAnimal(this as u32).get_ambient_key()
    }

    #[detour(DIAGONAL_OK)]
    unsafe extern "thiscall" fn diagonal_ok(this: *const u32) -> bool {
        ZTAnimal(this as u32).diagonal_ok()
    }

    #[detour(IS_WALKABLE)]
    unsafe extern "thiscall" fn is_walkable(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_walkable() as u32
    }

    #[detour(CLEAR_PREY)]
    unsafe extern "thiscall" fn clear_prey(this: *const u32) {
        ZTAnimal(this as u32).clear_prey();
    }

    #[detour(NO_SHADOW_PART)]
    unsafe extern "thiscall" fn no_shadow_part(this: *const u32) -> u32 {
        ZTAnimal(this as u32).no_shadow_part() as u32
    }

    #[detour(NEEDS_BREATH)]
    unsafe extern "thiscall" fn needs_breath(this: *const c_void) -> u32 {
        ZTAnimal(this as u32).needs_breath() as u32
    }

    #[detour(GET_KEEP_MOVING)]
    unsafe extern "thiscall" fn get_keep_moving(this: *const c_void) -> u32 {
        ZTAnimal(this as u32).get_keep_moving() as u32
    }

    #[detour(F_NUM_ANIMALS)]
    unsafe extern "thiscall" fn f_num_animals(this: *const u32) -> i32 {
        ZTAnimal(this as u32).f_num_animals()
    }

    #[detour(F_NUM_SAME_ANIMALS)]
    unsafe extern "thiscall" fn f_num_same_animals(this: *const u32) -> i32 {
        ZTAnimal(this as u32).f_num_same_animals()
    }

    #[detour(F_NUM_SAME_ADULT_ANIMALS)]
    unsafe extern "thiscall" fn f_num_same_adult_animals(this: *const c_void) -> i32 {
        ZTAnimal(this as u32).f_num_same_adult_animals()
    }

    #[detour(F_TILES_PER_ADULT_ANIMAL)]
    unsafe extern "thiscall" fn f_tiles_per_adult_animal(this: *const u32) -> i32 {
        ZTAnimal(this as u32).f_tiles_per_adult_animal()
    }

    #[detour(F_COMPAT_ANIMALS_RATING)]
    unsafe extern "thiscall" fn f_compat_animals_rating(this: *const u32) -> i32 {
        ZTAnimal(this as u32).f_compat_animals_rating()
    }

    #[detour(GET_ICON_ZOOM)]
    unsafe extern "thiscall" fn get_icon_zoom(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_icon_zoom()
    }

    #[detour(GET_IDLE_ANIM)]
    unsafe extern "thiscall" fn get_idle_anim(this: *const u32, force_unit: bool) -> i32 {
        ZTAnimal(this as u32).get_idle_anim(force_unit)
    }

    #[detour(SET_MAX_HEALTH)]
    unsafe extern "thiscall" fn set_max_health(this: *const c_void) {
        ZTAnimal(this as u32).set_max_health();
    }

    #[detour(IS_DOING_SOMETHING)]
    unsafe extern "thiscall" fn is_doing_something(this: *const u32) -> u32 {
        ZTAnimal(this as u32).is_doing_something() as u32
    }

    #[detour(GET_SWIMS)]
    unsafe extern "thiscall" fn get_swims(this: *const u32) -> u32 {
        ZTAnimal(this as u32).get_swims()
    }

    #[detour(GET_UNDERWATER)]
    unsafe extern "thiscall" fn get_underwater(this: *const c_void) -> u32 {
        ZTAnimal(this as u32).get_underwater()
    }

    #[detour(IS_WALKABLE_BY)]
    unsafe extern "thiscall" fn is_walkable_by(this: *const u32, other: *const u32) -> u32 {
        ZTAnimal(this as u32).is_walkable_by(other as u32) as u32
    }

    #[detour(CLIFFS_OK)]
    unsafe extern "thiscall" fn cliffs_ok(this: *const u32, tile: *const u32, direction: u32) -> bool {
        ZTAnimal(this as u32).cliffs_ok(tile as u32, direction)
    }

    #[detour(GET_PREY_UNIT)]
    unsafe extern "thiscall" fn get_prey_unit(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_prey_unit() as i32
    }

    #[detour(F_MATE_GONE)]
    unsafe extern "thiscall" fn f_mate_gone(this: *const u32) -> u32 {
        ZTAnimal(this as u32).f_mate_gone() as u32
    }

    #[detour(F_MATE_AVAILABLE)]
    unsafe extern "thiscall" fn f_mate_available(this: *const u32, flag: *const bool) -> u32 {
        ZTAnimal(this as u32).f_mate_available(flag) as u32
    }

    #[detour(F_OTHER_ANIMAL_ANGRY)]
    unsafe extern "thiscall" fn f_other_animal_angry(this: *const u32) -> u32 {
        ZTAnimal(this as u32).f_other_animal_angry() as u32
    }

    #[detour(F_OTHER_ANIMAL_SICK)]
    unsafe extern "thiscall" fn f_other_animal_sick(this: *const u32) -> u32 {
        ZTAnimal(this as u32).f_other_animal_sick() as u32
    }

    #[detour(F_ANIMAL_SPACE_RATIO)]
    unsafe extern "thiscall" fn f_animal_space_ratio(this: *const c_void) -> f32 {
        ZTAnimal(this as u32).f_animal_space_ratio()
    }

    #[detour(F_FOOD_UNITS_ACTUALLY_EATEN)]
    unsafe extern "thiscall" fn f_food_units_actually_eaten(this: *const c_void) -> i32 {
        ZTAnimal(this as u32).f_food_units_actually_eaten()
    }

    #[detour(F_IS_THERE_KEEPER_FOOD)]
    unsafe extern "thiscall" fn f_is_there_keeper_food(this: *const u32) -> u32 {
        ZTAnimal(this as u32).f_is_there_keeper_food() as u32
    }

    #[detour(CHECK_DONE)]
    unsafe extern "thiscall" fn check_done(this: *const u32) -> u32 {
        ZTAnimal(this as u32).check_done()
    }

    #[detour(CAN_CAPTURE)]
    unsafe extern "thiscall" fn can_capture(this: *const c_void, other: i32) -> u32 {
        ZTAnimal(this as u32).can_capture(other as u32) as u32
    }

    #[detour(IS_NEED_SATISFIER)]
    unsafe extern "thiscall" fn is_need_satisfier(_this: *const u32, tile: *const u32, pos: *const IVec3) -> u32 {
        ZTAnimal::is_need_satisfier(tile as u32, pos as u32) as u32
    }

    #[detour(CAN_SERVICE)]
    unsafe extern "thiscall" fn can_service(this: *const u32, staff: *const u32) -> u32 {
        ZTAnimal(this as u32).can_service(staff as u32) as u32
    }

    #[detour(CAN_SLEEP_BE_SICK_EAT)]
    unsafe extern "thiscall" fn can_sleep_be_sick_eat(this: *const u32) -> bool {
        ZTAnimal(this as u32).can_sleep_be_sick_eat()
    }

    #[detour(GET_ICON_ANIM)]
    unsafe extern "thiscall" fn get_icon_anim(this: *const u32) -> u32 {
        ZTAnimal(this as u32).get_icon_anim()
    }

    #[detour(GET_SELL_PRICE)]
    unsafe extern "thiscall" fn get_sell_price(this: *const u32) -> f32 {
        ZTAnimal(this as u32).get_sell_price()
    }

    #[detour(GET_HABITAT_RATING)]
    unsafe extern "thiscall" fn get_habitat_rating(this: *const u32, habitat: *const u32) -> i32 {
        ZTAnimal(this as u32).habitat_rating_in(habitat as u32)
    }

    #[detour(CALC_HABITAT)]
    unsafe extern "thiscall" fn calc_habitat(this: *const u32) {
        ZTAnimal(this as u32).calc_habitat();
    }

    #[detour(GET_HABITAT)]
    unsafe extern "thiscall" fn get_habitat(this: *const u32) -> i32 {
        ZTAnimal(this as u32).get_habitat()
    }

    #[detour(SET_HOME_HABITAT)]
    unsafe extern "thiscall" fn set_home_habitat(this: *const u32, habitat: *const u32) {
        ZTAnimal(this as u32).set_home_habitat(habitat as u32);
    }

    #[detour(RECALC_HOME_HABITAT)]
    unsafe extern "thiscall" fn recalc_home_habitat(this: *const u32) {
        ZTAnimal(this as u32).recalc_home_habitat();
    }

    #[detour(GET_HOME_HABITAT)]
    unsafe extern "thiscall" fn get_home_habitat(this: *const u32) -> *const u32 {
        ZTAnimal(this as u32).get_home_habitat() as *const u32
    }

    #[detour(GET_UNDESIRABLE_SCENERY_IN_HABITAT)]
    unsafe extern "thiscall" fn get_undesirable_scenery_in_habitat(this: *const u32, out: *const i32, flag: u32) -> *const i32 {
        ZTAnimal(this as u32).get_undesirable_scenery_in_habitat(out as u32, flag) as *const i32
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn is_hungry(this: u32) -> u32 {
            unsafe { super::IS_HUNGRY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_hungry_and_foodless(this: u32) -> u32 {
            unsafe { super::IS_HUNGRY_AND_FOODLESS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_happy_pct(this: u32) -> i32 {
            unsafe { super::GET_HAPPY_PCT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_hunger_pct(this: u32) -> i32 {
            unsafe { super::GET_HUNGER_PCT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_health_pct(this: u32) -> i32 {
            unsafe { super::GET_HEALTH_PCT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_unhappy_for_reproduction(this: u32) -> bool {
            unsafe { super::IS_UNHAPPY_FOR_REPRODUCTION_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_unhappy_with_habitat(this: u32) -> u32 {
            unsafe { super::IS_UNHAPPY_WITH_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_very_unhappy_with_habitat(this: u32) -> u32 {
            unsafe { super::IS_VERY_UNHAPPY_WITH_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_sickly(this: u32) -> u32 {
            unsafe { super::IS_SICKLY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_egg(this: u32) -> u8 {
            unsafe { super::IS_EGG_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn is_selectable(this: u32) -> bool {
            unsafe { super::IS_SELECTABLE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn preys_on_man(this: u32) -> bool {
            unsafe { super::PREYS_ON_MAN_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_in_prey_list(this: u32, id: i32) -> u32 {
            unsafe { super::IS_IN_PREY_LIST_DETOUR.call(this as *const u32, id) }
        }

        pub(crate) fn is_preattack(this: u32, other: u32) -> u32 {
            unsafe { super::IS_PREATTACK_DETOUR.call(this as *const u32, other as i32) }
        }

        pub(crate) fn within_preattack_range(this: u32, other: u32) -> u32 {
            unsafe { super::WITHIN_PREATTACK_RANGE_DETOUR.call(this as *const u32, other as i32) }
        }

        pub(crate) fn get_ambient_key(this: u32) -> u32 {
            unsafe { super::GET_AMBIENT_KEY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn diagonal_ok(this: u32) -> bool {
            unsafe { super::DIAGONAL_OK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_walkable(this: u32) -> u32 {
            unsafe { super::IS_WALKABLE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn clear_prey(this: u32) {
            unsafe { super::CLEAR_PREY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn no_shadow_part(this: u32) -> u32 {
            unsafe { super::NO_SHADOW_PART_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn needs_breath(this: u32) -> u32 {
            unsafe { super::NEEDS_BREATH_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_num_animals(this: u32) -> i32 {
            unsafe { super::F_NUM_ANIMALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_num_same_animals(this: u32) -> i32 {
            unsafe { super::F_NUM_SAME_ANIMALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_num_same_adult_animals(this: u32) -> i32 {
            unsafe { super::F_NUM_SAME_ADULT_ANIMALS_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_tiles_per_adult_animal(this: u32) -> i32 {
            unsafe { super::F_TILES_PER_ADULT_ANIMAL_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_compat_animals_rating(this: u32) -> i32 {
            unsafe { super::F_COMPAT_ANIMALS_RATING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_icon_zoom(this: u32) -> i32 {
            unsafe { super::GET_ICON_ZOOM_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_idle_anim(this: u32, force_unit: bool) -> i32 {
            unsafe { super::GET_IDLE_ANIM_DETOUR.call(this as *const u32, force_unit) }
        }

        pub(crate) fn set_max_health(this: u32) {
            unsafe { super::SET_MAX_HEALTH_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn is_doing_something(this: u32) -> u32 {
            unsafe { super::IS_DOING_SOMETHING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_swims(this: u32) -> u32 {
            unsafe { super::GET_SWIMS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_underwater(this: u32) -> u32 {
            unsafe { super::GET_UNDERWATER_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn is_walkable_by(this: u32, other: u32) -> u32 {
            unsafe { super::IS_WALKABLE_BY_DETOUR.call(this as *const u32, other as *const u32) }
        }

        pub(crate) fn cliffs_ok(this: u32, tile: u32, direction: u32) -> bool {
            unsafe { super::CLIFFS_OK_DETOUR.call(this as *const u32, tile as *const u32, direction) }
        }

        pub(crate) fn get_prey_unit(this: u32) -> i32 {
            unsafe { super::GET_PREY_UNIT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_mate_gone(this: u32) -> u32 {
            unsafe { super::F_MATE_GONE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_other_animal_angry(this: u32) -> u32 {
            unsafe { super::F_OTHER_ANIMAL_ANGRY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_other_animal_sick(this: u32) -> u32 {
            unsafe { super::F_OTHER_ANIMAL_SICK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_animal_space_ratio(this: u32) -> f32 {
            unsafe { super::F_ANIMAL_SPACE_RATIO_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_food_units_actually_eaten(this: u32) -> i32 {
            unsafe { super::F_FOOD_UNITS_ACTUALLY_EATEN_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_is_there_keeper_food(this: u32) -> u32 {
            unsafe { super::F_IS_THERE_KEEPER_FOOD_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn check_done(this: u32) -> u32 {
            unsafe { super::CHECK_DONE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn can_capture(this: u32, other: u32) -> u32 {
            unsafe { super::CAN_CAPTURE_DETOUR.call(this as *const c_void, other as i32) }
        }

        pub(crate) fn is_need_satisfier(this: u32, tile: u32, pos: u32) -> u32 {
            unsafe { super::IS_NEED_SATISFIER_DETOUR.call(this as *const u32, tile as *const u32, pos as *const glam::i32::IVec3) }
        }

        pub(crate) fn can_service(this: u32, staff: u32) -> u32 {
            unsafe { super::CAN_SERVICE_DETOUR.call(this as *const u32, staff as *const u32) }
        }

        pub(crate) fn can_sleep_be_sick_eat(this: u32) -> bool {
            unsafe { super::CAN_SLEEP_BE_SICK_EAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_icon_anim(this: u32) -> u32 {
            unsafe { super::GET_ICON_ANIM_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_sell_price(this: u32) -> f32 {
            unsafe { super::GET_SELL_PRICE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_habitat_rating(this: u32, habitat: u32) -> i32 {
            unsafe { super::GET_HABITAT_RATING_DETOUR.call(this as *const u32, habitat as *const u32) }
        }

        pub(crate) fn calc_habitat(this: u32) {
            unsafe { super::CALC_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_habitat(this: u32) -> i32 {
            unsafe { super::GET_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_home_habitat(this: u32, habitat: u32) {
            unsafe { super::SET_HOME_HABITAT_DETOUR.call(this as *const u32, habitat as *const u32) }
        }

        pub(crate) fn recalc_home_habitat(this: u32) {
            unsafe { super::RECALC_HOME_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_home_habitat(this: u32) -> u32 {
            unsafe { super::GET_HOME_HABITAT_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_undesirable_scenery_in_habitat(this: u32, out: u32, flag: u32) -> u32 {
            unsafe { super::GET_UNDESIRABLE_SCENERY_IN_HABITAT_DETOUR.call(this as *const u32, out as *const i32, flag) as u32 }
        }

        pub(crate) fn get_keep_moving(this: u32) -> u32 {
            unsafe { super::GET_KEEP_MOVING_DETOUR.call(this as *const c_void) }
        }
    }
}

/// Installs `ZTAnimal`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTANIMAL_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { animal_detours::init_detours() } {
        error!("Failed to initialise ztanimal detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::animal_detours::status()
    }

    pub(crate) use super::animal_detours::test_real as real;
}
