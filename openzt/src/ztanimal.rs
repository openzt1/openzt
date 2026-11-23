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
    bfentity::{CLEAR_BS as BFENTITY_CLEAR_BS, CLEAR_QUEUE as BFENTITY_CLEAR_QUEUE, GET_TILE, UPDATE as BFENTITY_UPDATE},
    bfunit::{FACE_TOWARD_TILE as BFUNIT_FACE_TOWARD_TILE, STOP as BFUNIT_STOP, UPDATE_POSITION as BFUNIT_UPDATE_POSITION},
    ztscenery::SET_AMOUNT_FOOD_UNITS_DELTA,
    zoostatus::ANIMAL_ESCAPED as ZOOSTATUS_ANIMAL_ESCAPED,
    bfmap::{GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR, GET_SELECTION as BFMAP_GET_SELECTION, PAINT_CELL as BFMAP_PAINT_CELL, SET_SELECTION as BFMAP_SET_SELECTION},
    bfunit::CHECK_DONE as BFUNIT_CHECK_DONE,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
    standalone::OPERATOR_DELETE,
    zthabitat::GET_GATE_TILE_IN,
    ztworldmgr::SATISFIES as ZTWORLDMGR_SATISFIES,
    ztanimal::{
        SET_FOOD, SET_KEEPER_ARRIVES, FACE_TOWARD_FOOD, REMOVE_FOOD_TARGET_FOR_ALL, F_REDUCE_FOOD, F_REMOVE_ANIMAL, DROP_TEST_FENCE_CHANCE, ADD_CHANGES,
        DO_FOOD_TRICK, CHECK_DO_TRICK, BOX, UNBOX, ADD_FACE, CHECK_PREY, STOP_EATING, STOP_CHASE, CHANGE_TO_DIRT,
        CALC_HABITAT, CAN_CAPTURE, CAN_SERVICE, CAN_SLEEP_BE_SICK_EAT, CHECK_DONE, CLIFFS_OK, F_ANIMAL_SPACE_RATIO, F_FOOD_UNITS_ACTUALLY_EATEN,
        F_MATE_AVAILABLE, F_MATE_GONE, F_OTHER_ANIMAL_ANGRY, F_OTHER_ANIMAL_SICK, GET_HABITAT, GET_HOME_HABITAT, GET_ICON_ANIM, GET_PREY_UNIT,
        GET_SELL_PRICE, GET_SWIMS, GET_UNDERWATER, GET_UNDESIRABLE_SCENERY_IN_HABITAT, IS_NEED_SATISFIER, IS_WALKABLE_BY, RECALC_HOME_HABITAT,
        SET_HOME_HABITAT, F_GET_MATE, IS_DOING_SOMETHING,
        F_COMPAT_ANIMALS_RATING, F_NUM_ANIMALS, F_NUM_SAME_ADULT_ANIMALS, F_NUM_SAME_ANIMALS, F_TILES_PER_ADULT_ANIMAL, GET_ICON_ZOOM, GET_IDLE_ANIM,
        SET_MAX_HEALTH, DO_BREATH_CHECK, DO_EGG_CHECK, UPDATE, UPDATE_POSITION, DO_CONDITIONALS, SHOW_ESCAPED_ANIMAL_ALERT, DO_BOXED_CHECK, DO_BABY_TO_ADULT_CHECK, DO_REPRODUCE_CHECK, DO_ESCAPED_CHECK, DO_HEALTH_CHECK, DO_ZAP_CHECK, DO_BUILDING_USE_CHECK, DO_BORED_CHECK, DO_SLEEP_CHECK, DO_WATER_CHECK, DO_SOCIAL_CHECK, DO_HABITAT_CHECK, DO_HUNGER_CHECK, SET_TERRAIN_MODE, UPDATE_STATUS_VARIABLES, F_EAT, PUT_DOWN_DIRT, DO_MURKY_WATER_CHECK, DO_KEEPER_CHECK, DO_OTHER_CHECK, HATCH_EGG,
        F_IS_THERE_KEEPER_FOOD, GET_AMBIENT_KEY, GET_HABITAT_RATING, GET_HAPPY_PCT, GET_HEALTH_PCT, GET_HUNGER_PCT, GET_KEEP_MOVING, CLEAR_PREY,
        DIAGONAL_OK, IS_EGG, ABORT_GOAL, SAVE, CLEAR_ESCAPE_INFO, CLEAR_GOAL, CLEAR_STATE_VARIABLES, FIND_PLACE_TO_REST, SET_DEATH_CHECK, SET_EGG, SET_ESCAPED, TARGET_BUILDING, IS_HUNGRY, IS_HUNGRY_AND_FOODLESS, IS_IN_PREY_LIST, IS_PREATTACK, IS_SELECTABLE, IS_SICKLY, IS_UNHAPPY_FOR_REPRODUCTION,
        IS_UNHAPPY_WITH_HABITAT, IS_VERY_UNHAPPY_WITH_HABITAT, IS_WALKABLE, NEEDS_BREATH, NO_SHADOW_PART, PREYS_ON_MAN, WITHIN_PREATTACK_RANGE,
    },
    ztui_animalinfo::FORCE_UPDATE as ANIMALINFO_FORCE_UPDATE,
    ztunit::{CLEAR_GOAL as ZTUNIT_CLEAR_GOAL, CLEAR_STATE_VARIABLES as ZTUNIT_CLEAR_STATE_VARIABLES, SAVE as ZTUNIT_SAVE, HAS_BUILDING_USE_GOAL as ZTUNIT_HAS_BUILDING_USE_GOAL},
    ztworldmgr::GET_NEED_INDEX,
    ztgamemgr::GET_DATE as GAMEMGR_GET_DATE,
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    standalone::IS_ANIMAL_REST_SATISFIER,
    ztunit::{FIND_NEAREST_SATISFIER, IS_NEED_OR_FUN_SATISFIER, IN_FRIENDLY_CHASE, F_ZOO_MESSAGE_NAME as ZTUNIT_F_ZOO_MESSAGE_NAME, SET_IS_REMOVED as ZTUNIT_SET_IS_REMOVED, IS_DOING_SOMETHING as ZTUNIT_IS_DOING_SOMETHING, CLIFFS_OK as ZTUNIT_CLIFFS_OK, GET_HABITAT as ZTUNIT_GET_HABITAT, GET_IDLE_ANIM as ZTUNIT_GET_IDLE_ANIM},
};
use glam::i32::IVec3;
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::{ftol_low32, get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, ZTHabitat},
    ztguide::ZTGuide,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3, unit_offsets, write_field},
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
    pub const HEALTH_PENDING: u32 = 0x2bc;
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

/// Fields and addresses the lifecycle methods touch.
mod lifecycle {
    /// Byte set while the animal is outside its habitat; the animal is in `DAT_00638fb0`'s list while it is set.
    pub const ESCAPED: u32 = 0x3a3;
    /// Bytes `clearGoal` zeroes.
    pub const CLEAR_BYTES: [u32; 6] = [0x3a2, 0x39f, 0x3a0, 0x3a1, 0x3a4, 0x3a5];
    pub const ESCAPE_STAGE_A: u32 = 0x3a1;
    pub const ESCAPE_STAGE_B: u32 = 0x3a2;
    pub const PENDING_TARGET_FLAG: u32 = 0x3a4;
    pub const UNIT_FLAG_24C: u32 = 0x24c;
    pub const UNIT_WORD_22C: u32 = 0x22c;
    /// Byte compared with `ZTWorldMgr::getNeedIndex`.
    pub const UNIT_NEED_BYTE: u32 = 0x24e;
    pub const UNIT_REST_TILE: u32 = 0x244;
    pub const UNIT_REST_POS: u32 = 0x214;
    /// Death countdown `setDeathCheck` edits.
    pub const DEATH_CHECK: u32 = 0x34c;
    /// Hatch-countdown word `setEgg` seeds (`5` or the type's `+0x408`).
    pub const EGG_TIMER: u32 = 0x398;
    /// Begin and end pointers of the vector at `+0x260`.
    pub const VEC_BEGIN: u32 = 0x260;
    pub const VEC_END: u32 = 0x264;
    /// The block at `+0x8c` that `setEgg` resets (`0x41bf8f`).
    pub const BLOCK_8C: u32 = 0x8c;
    pub const SLOT_CLEAR_GOAL: u32 = 0x174;
    pub const SLOT_STOP_EVERYTHING: u32 = 0x198;
    pub const SLOT_UPDATE_ANIMATION: u32 = 0x40;
    pub const SLOT_IS_DOING_SHOW: u32 = 0x22c;
    pub const SLOT_ABORT_TRICK: u32 = 0x2c0;
    pub const SLOT_CLEAR_PREY: u32 = 0x258;
    pub const RESET_BLOCK_8C: u32 = 0x0041_bf8f;
    pub const REST_SATISFIER_2: u32 = 0x0061_349c;
    pub const SELECTION_NOTIFY: u32 = 0x0044_0a27;
    pub const EGG_TYPE_HEALTH_BASE: u32 = 0x420;
    pub const EGG_TYPE_TIMER: u32 = 0x408;
    /// `DAT_0063cfa1`.
    pub const EGG_TIMER_OVERRIDE: u32 = 0x0023_cfa1;
    /// `DAT_0063afc8` and `DAT_006463a0`: the need names `abortGoal` resolves.
    pub const NEED_A: u32 = 0x0023_afc8;
    pub const NEED_B: u32 = 0x0024_63a0;
    /// `DAT_00638fb0`: the global holding the sentinel of the escaped-animal list.
    pub const ESCAPE_LIST: u32 = 0x0023_8fb0;
}

/// The `(offset, size)` writes of `ZTAnimal::save`, in stream order, split around the strings and the escape record.
mod save_layout {
    pub const BEFORE_STRINGS: &[(u32, u32)] = &[
        (0x2a8, 4),
        (0x2b0, 4),
        (0x2b8, 4),
        (0x2c0, 4),
        (0x3a7, 1),
        (0x3a8, 1),
        (0x3a9, 1),
        (0x3aa, 1),
        (0x3ab, 1),
        (0x378, 4),
        (0x390, 1),
        (0x2d0, 4),
        (0x2d4, 4),
        (0x2d8, 4),
        (0x2dc, 4),
        (0x2e0, 4),
    ];
    /// Between the strings and the has-escape-info dword.
    pub const AFTER_STRINGS_A: &[(u32, u32)] = &[
        (0x2f4, 4),
        (0x2f8, 4),
        (0x300, 4),
        (0x308, 4),
        (0x30c, 4),
        (0x310, 4),
        (0x314, 4),
        (0x318, 4),
        (0x324, 4),
        (0x328, 4),
        (0x334, 4),
        (0x338, 4),
        (0x33c, 4),
        (0x2c8, 4),
        (0x39c, 1),
        (0x39e, 1),
        (0x3a3, 1),
        (0x304, 4),
        (0x3a4, 1),
        (0x3a5, 1),
        (0x2e4, 4),
        (0x32c, 4),
        (0x330, 4),
        (0x2e8, 4),
        (0x288, 8),
        (0x290, 8),
        (0x39f, 1),
        (0x3a0, 1),
        (0x2ec, 4),
        (0x2f0, 4),
        (0x348, 4),
        (0x34c, 4),
        (0x350, 4),
        (0x3a6, 1),
        (0x39d, 1),
        (0x320, 4),
    ];
    /// After the escape record.
    pub const AFTER_STRINGS_B: &[(u32, u32)] = &[
        (0x354, 4),
        (0x374, 4),
        (0x393, 1),
        (0x394, 1),
        (0x3a1, 1),
        (0x3a2, 1),
        (0x3b0, 1),
        (0x358, 4),
        (0x35c, 4),
        (0x3b1, 1),
        (0x395, 1),
        (0x398, 4),
        (0x360, 4),
        (0x364, 4),
    ];
    pub const DEFAULT_GATE_X: u32 = 0x0023_9370;
    pub const DEFAULT_GATE_Y: u32 = 0x0023_9374;
}

/// Fields, periods and slots the per-tick checks use.
mod pipeline {
    pub const BREATH_COUNTDOWN: u32 = 0x364;
    pub const KEEPER_COUNTDOWN: u32 = 0x30c;
    pub const OTHER_COUNTDOWN: u32 = 0x308;
    /// Keeper-count word `doKeeperCheck` compares with the type's required keepers.
    pub const KEEPER_COUNT: u32 = 0x378;
    pub const FLAG_WORDS_ANGER_BIT: u32 = 8;
    /// Process-global reload values (`DAT_00639450`, `DAT_00639410`, `DAT_00639424`) as RVAs.
    pub const BREATH_PERIOD: u32 = 0x0023_9450;
    pub const KEEPER_PERIOD: u32 = 0x0023_9410;
    pub const OTHER_PERIOD: u32 = 0x0023_9424;
    /// Terrain-mode value meaning underwater.
    pub const MODE_UNDERWATER: u32 = 4;
    pub const SLOT_IS_DOING_SHOW: u32 = 0x22c;
    pub const SLOT_HABITAT: u32 = 0x24c;
    /// Type fields: breath step/max/min, required keepers and the happiness change when short of them, the
    /// angry-neighbour happiness change, and the egg health base.
    pub const TYPE_BREATH_STEP: u32 = 0x200;
    pub const TYPE_BREATH_MAX: u32 = 0x204;
    pub const TYPE_BREATH_MIN: u32 = 0x208;
    pub const TYPE_KEEPERS_NEEDED: u32 = 0x28c;
    pub const TYPE_NO_KEEPER_HAPPINESS: u32 = 0x290;
    pub const TYPE_OTHER_ANGRY_HAPPINESS: u32 = 0x2f4;
    pub const TYPE_EGG_HEALTH_BASE: u32 = 0x420;
    /// Byte of a habitat that suppresses egg hatching.
    pub const HABITAT_NO_HATCH: u32 = 0x2c;
    pub const HABITAT_COUNTDOWN: u32 = 0x314;
    pub const MURKY_COUNTDOWN: u32 = 0x2fc;
    /// `DAT_00639400` and `DAT_006393e4`.
    pub const HABITAT_PERIOD: u32 = 0x0023_9400;
    pub const MURKY_PERIOD: u32 = 0x0023_93e4;
    pub const BIT_HABITAT: u32 = 0x400;
    /// Habitat vtable slots: `isTank`-style predicate and the type-taking predicate; fields `+0x4`, `+0x188`, `+0x1a8`.
    pub const HABITAT_SLOT_IS_TANK: u32 = 0x20;
    pub const HABITAT_SLOT_TYPE_OK: u32 = 0x28;
    pub const HABITAT_FIELD_4: u32 = 0x4;
    pub const HABITAT_FIELD_188: u32 = 0x188;
    pub const HABITAT_FIELD_1A8: u32 = 0x1a8;
    /// Type fields for the habitat rating bands (`doHabitatCheck`).
    pub const TYPE_RATING_MIN: u32 = 0x2bc;
    pub const TYPE_RATING_DISLIKE_FACTOR: u32 = 0x310;
    pub const TYPE_HAPPY_LOW: u32 = 0x304;
    pub const TYPE_HAPPY_MID: u32 = 0x308;
    pub const TYPE_HAPPY_BAD: u32 = 0x30c;
    pub const MESSAGE_ID: u32 = 0x272f;
    /// Murky-water bands: thresholds `+0x42c`, `+0x428`, `+0x424`; happiness `+0x438`, `+0x434`, `+0x430`;
    /// health `+0x444`, `+0x440`, `+0x43c`.
    pub const TYPE_MURKY_THRESHOLDS: [u32; 3] = [0x42c, 0x428, 0x424];
    pub const TYPE_MURKY_HAPPINESS: [u32; 3] = [0x438, 0x434, 0x430];
    pub const TYPE_MURKY_HEALTH: [u32; 3] = [0x444, 0x440, 0x43c];
    pub const TYPE_RANGE_LOW: u32 = 0x35c;
    pub const TYPE_RANGE_HIGH: u32 = 0x360;
    pub const TYPE_RANGE_HAPPINESS: u32 = 0x364;
    pub const TYPE_NOT_OK_HAPPINESS: u32 = 0x368;
    pub const TYPE_NOT_OK_HEALTH: u32 = 0x36c;
}

/// Fields the hunger check touches.
mod hunger {
    pub const COUNTDOWN: u32 = 0x2f4;
    /// `DAT_0063940c`.
    pub const PERIOD: u32 = 0x0023_940c;
    /// Per-tick hunger gain multiplier on the unit's `+0x2d4`, added to the pending hunger (`+0x2b4`).
    pub const HUNGER_RATE: u32 = 0x2d4;
    pub const HUNGER_PENDING: u32 = 0x2b4;
    /// Flag bytes: eating/seeking-food state.
    pub const BYTE_EATING: u32 = 0x3a8;
    pub const BYTE_FOOD_WANTED: u32 = 0x3a9;
    pub const BYTE_BLOCKED: u32 = 0x3b0;
    pub const BYTE_KEEPER_FOOD: u32 = 0x39f;
    pub const BYTE_KEEPER_FOOD_2: u32 = 0x3a0;
    pub const BYTE_KEEPER_FOOD_3: u32 = 0x3a1;
    pub const BYTE_KEEPER_FOOD_4: u32 = 0x3a2;
    pub const BYTE_NO_FOOD_SEEN: u32 = 0x39e;
    pub const UNIT_BYTES_MESSAGE_GATE: [u32; 2] = [0x24f, 0x250];
    pub const GOAL: u32 = 0x170;
    pub const GOALS_NO_EAT: [i32; 2] = [0x72, 0x73];
    pub const GOALS_SHOW: [i32; 2] = [0x85, 0x86];
    pub const MESSAGE_HUNGRY: u32 = 0x272e;
    pub const DIRT_COUNTER: u32 = 0x2c8;
    pub const DIRT_PENDING: u32 = 0x2cc;
    pub const TYPE_HUNGER_PENDING_STEP: u32 = 0x214;
    pub const TYPE_HUNGRY_AT: u32 = 0x20c;
    pub const TYPE_STARVING_AT: u32 = 0x240;
    pub const TYPE_STARVING_HAPPINESS: u32 = 0x224;
    pub const TYPE_STARVING_HEALTH: u32 = 0x210;
    pub const TYPE_DIRT_STEP: u32 = 0x328;
    pub const TYPE_DIRT_AT: u32 = 0x32c;
}

/// Fields `updateStatusVariables` folds. Each need is `(value, pending, type max, type min)`.
mod status {
    /// Happiness, hunger, the two unnamed needs (`+0x2c0`, `+0x2c8`) and health: value, pending delta, type max, type min.
    pub const HAPPINESS: (u32, u32, u32, u32) = (0x2a8, 0x2ac, 0x230, 0x22c);
    pub const HUNGER: (u32, u32, u32, u32) = (0x2b0, 0x2b4, 0x240, 0x244);
    pub const NEED_C0: (u32, u32, u32, u32) = (0x2c0, 0x2c4, 0x248, 0x24c);
    pub const NEED_C8: (u32, u32, u32, u32) = (0x2c8, 0x2cc, 0x250, 0x254);
    pub const HEALTH: (u32, u32, u32, u32) = (0x2b8, 0x2bc, 0x234, 0x238);
    /// Byte set while happiness is at or below `-0x50` (`ANGRY`).
    pub const ANGRY_HAPPINESS: i32 = -0x50;
}

/// Fields and slots `doBoredCheck` touches.
mod bored {
    pub const COUNTDOWN: u32 = 0x318;
    /// `DAT_00639414`.
    pub const PERIOD: u32 = 0x0023_9414;
    pub const BIT_BORED: u32 = 0x40;
    pub const NEED: u32 = 0x2c0;
    pub const TYPE_SLEEP_AT: u32 = 0x324;
    pub const BYTE_FLEEING: u32 = 0x24c;
    pub const BYTE_PLAYMATE: u32 = 0x3a4;
    pub const BYTE_ESCAPED: u32 = 0x3a3;
    pub const BYTE_WANTS_WATER: u32 = 0x3a1;
    pub const BYTE_BLOCKED: u32 = 0x3b0;
    pub const ACTIVITY_MODE: u32 = 0x380;
    pub const GOAL_CHASE_PLAYMATE: i32 = 0x74;
    pub const GOAL_PLAYMATE: i32 = 0x75;
    pub const SLOT_CLEAR_GOAL: u32 = 0x174;
    pub const SLOT_HAS_PRESSING_NEED: u32 = 0x288;
    pub const SLOT_FIND_PLAYMATE: u32 = 0x2f8;
    pub const SLOT_FIND_PREDATOR: u32 = 0x2fc;
    pub const SLOT_CAN_DRINK: u32 = 0x2c8;
    /// Type fields: the playmate roll's base count and the extra added when other animals share the habitat.
    pub const TYPE_PLAY_BASE: u32 = 0x3a8;
    pub const TYPE_PLAY_EXTRA: u32 = 0x3ac;
    /// Tile byte whose low two bits block the water wish.
    pub const TILE_FLAGS: u32 = 0x83;
}

/// Fields `doSocialCheck` touches. Status flags are 64-bit values split across two dwords: the low set is
/// `+0x298` (low dword) and `+0x29c` (high), the critical set `+0x2a0` and `+0x2a4`.
mod social {
    pub const COUNTDOWN: u32 = 0x324;
    /// `DAT_00639420`.
    pub const PERIOD: u32 = 0x0023_9420;
    pub const BIT_LONELY: u32 = 0x10;
    pub const BIT_CROWDED_SPECIES: u32 = 0x20;
    pub const BIT_CROWDED_TILES: u32 = 0x1_0000;
    /// Bit in the high dword (`+0x29c`/`+0x2a4`): crowded by guests.
    pub const BIT_CROWDED_GUESTS_HIGH: u32 = 0x100_0000;
    /// Guest crowding the animal currently feels (`+0x368`), and the two direction components (`+0x36c`, `+0x370`).
    pub const GUEST_CROWDING: u32 = 0x368;
    pub const DIRECTION_A: u32 = 0x36c;
    pub const DIRECTION_B: u32 = 0x370;
    pub const DIRECTION_CODE: u32 = 0x37c;
    pub const TYPE_MIN_SAME_SPECIES: u32 = 0x29c;
    pub const TYPE_SPACE_FACTOR: u32 = 0x2a0;
    pub const TYPE_LONELY_HAPPINESS: u32 = 0x2ac;
    pub const TYPE_CROWDED_SPECIES_HAPPINESS: u32 = 0x2b0;
    pub const TYPE_TILES_MIN: u32 = 0x314;
    pub const TYPE_CROWDED_TILES_HAPPINESS: u32 = 0x318;
    pub const TYPE_GUEST_TOLERANCE: u32 = 0x268;
    pub const TYPE_CROWDED_GUESTS_HAPPINESS: u32 = 0x26c;
    /// Happiness changes below this also raise the critical flag.
    pub const CRITICAL_BELOW: i32 = -5;
}

/// Fields and slots `doWaterCheck` touches.
mod water {
    pub const COUNTDOWN: u32 = 0x320;
    /// `DAT_00639418`.
    pub const PERIOD: u32 = 0x0023_9418;
    /// Slots: wants land, wants water, mode-1 blocker, mode-2 blocker.
    pub const SLOT_WANTS_LAND: u32 = 0x16c;
    pub const SLOT_WANTS_WATER: u32 = 0xbc;
    pub const SLOT_MODE1_BLOCK: u32 = 0xc0;
    pub const SLOT_MODE2_BLOCK: u32 = 0x170;
    /// Type percent chances (`i32`, compared with `rand() % 100`) and tile-count thresholds.
    pub const TYPE_WATER_CHANCE: u32 = 0x39c;
    pub const TYPE_LAND_CHANCE: u32 = 0x3a0;
    pub const TYPE_UNDERWATER_CHANCE: u32 = 0x3a4;
    pub const TYPE_WATER_TILES: u32 = 0x390;
    pub const TYPE_LAND_TILES: u32 = 0x394;
    pub const TYPE_UNDERWATER_TILES: u32 = 0x398;
    /// `setTerrainMode` modes.
    pub const MODE_UNDERWATER: i32 = 0;
    pub const MODE_WATER: i32 = 1;
    pub const MODE_LAND: i32 = 2;
}

/// Fields `update` and `updatePosition` touch.
mod tick {
    /// Begin and end of the vector at `+0x260`: 8-byte `(id, age)` entries.
    pub const ENTRIES_BEGIN: u32 = 0x260;
    pub const ENTRIES_END: u32 = 0x264;
    /// Byte: the entry vector is non-empty.
    pub const ENTRIES_NONEMPTY: u32 = 0x392;
    pub const ENTRY_SIZE: u32 = 8;
    /// An entry whose age reaches this is dropped.
    pub const ENTRY_MAX_AGE: i32 = 0x4e2;
    /// Tile byte `updatePosition` marks for a man-eating animal, and the bit it sets.
    pub const TILE_PREDATOR_BYTE: u32 = 0x85;
    pub const TILE_PREDATOR_BIT: u8 = 0x8;
}

/// Fields, globals and slots `doConditionals` touches.
mod conditionals {
    /// Countdowns: the per-tick drain (`+0x310`), the unhappiness message (`+0x33c`) and the AI-manager gate (`+0x340`).
    pub const COUNTDOWN_PRESSURE: u32 = 0x310;
    pub const COUNTDOWN_MESSAGE: u32 = 0x33c;
    pub const COUNTDOWN_AIMGR: u32 = 0x340;
    /// Globals as RVAs: `DAT_006393f8`, `DAT_006393f4`, `DAT_006394ac`, `DAT_006393e8`, `DAT_00641564`.
    pub const PRESSURE_PERIOD: u32 = 0x0023_93f8;
    pub const ANGRY_BASE: u32 = 0x0023_93f4;
    pub const DIRT_CHANCE: u32 = 0x0023_94ac;
    pub const MESSAGE_PERIOD: u32 = 0x0023_93e8;
    pub const DEATH_UNHAPPINESS: u32 = 0x0024_1564;
    /// `GLOBAL_ZTAIMgr` field tested before the AI slot call, `GLOBAL_ZTHabitatMgr`'s per-column cell table (`+0x28`).
    pub const AIMGR_GATE: u32 = 0x138;
    pub const HABITAT_MGR_TABLE: u32 = 0x28;
    pub const TILE_X: u32 = 0x34;
    pub const TILE_Y: u32 = 0x38;
    pub const COLUMN_STRIDE: u32 = 12;
    pub const CELL_STRIDE: u32 = 40;
    /// Habitat byte `+0x2c` and the hi-dword marker set in every status word while the animal is boxed or its tile's
    /// habitat flag is set.
    pub const HABITAT_FLAG: u32 = 0x2c;
    pub const MARKER_HIGH: u32 = 0x4000_0000;
    /// Animal fields: keeper-visit (`+0x39c`) and death-arrived (`+0x39d`) bytes, the hunger-stop flag byte (`+0x3a9`),
    /// the date stamp (`+0x288`), the drain multiplier (`+0x2e0`), and the not-selectable byte (`+0x3a6`).
    pub const BYTE_KEEPER_VISIT: u32 = 0x39c;
    pub const BYTE_DEATH_ARRIVED: u32 = 0x39d;
    pub const DATE_STAMP: u32 = 0x288;
    pub const DRAIN_FACTOR: u32 = 0x2e0;
    /// Type fields: the drain multiplier (`+0x274`), the dirt-chance cap (`+0x38c`), keeper-visit happiness (`+0x2fc`,
    /// `+0x300` for a type whose vtable `+0xcc` answers true) and the hunger limit `doConditionals` compares (`+0x244`).
    pub const TYPE_DRAIN: u32 = 0x274;
    pub const TYPE_DIRT_CAP: u32 = 0x38c;
    pub const TYPE_KEEPER_HAPPINESS: u32 = 0x2fc;
    pub const TYPE_KEEPER_HAPPINESS_ALT: u32 = 0x300;
    pub const TYPE_HUNGER_LIMIT: u32 = 0x244;
    pub const TYPE_SLOT_ALT: u32 = 0xcc;
    pub const SLOT_AIMGR_NOTIFY: u32 = 0x10c;
    pub const SLOT_UNKNOWN_0XAC: u32 = 0xac;
    pub const SLOT_TICK_NOTIFY: u32 = 0x48;
    /// `-0x3d`: happiness at or below it posts the "Not happy" message.
    pub const MESSAGE_HAPPINESS: i32 = -0x3d;
    pub const MESSAGE_ID: u32 = 0x272c;
    /// `ZTAnimal::isOnGrass`: the predicate gating the dirt patch.
    pub const DIRT_PREDICATE: u32 = 0x0061_3bde;
    pub const TILE_DIRT_WORDS: [u32; 4] = [4, 8, 12, 16];
}

/// Fields `doBuildingUseCheck` touches.
mod building_use {
    pub const COUNTDOWN: u32 = 0x344;
    /// `DAT_00639408`, and `DAT_0063afc8` (the need-name global whose address the satisfier calls take).
    pub const PERIOD: u32 = 0x0023_9408;
    pub const NEED_NAME: u32 = 0x0023_afc8;
    /// Type field: percent chance per firing that the animal looks for a building.
    pub const TYPE_CHANCE: u32 = 0x37c;
    /// The unit's destination position (`+0x214`, three `i32`s) and the need word `isNeedOrFunSatisfier` takes (`+0x244`).
    pub const UNIT_POSITION: u32 = 0x214;
    pub const UNIT_NEED_WORD: u32 = 0x244;
    /// Byte the found building's need index is stored in.
    pub const UNIT_NEED_BYTE: u32 = 0x24e;
    pub const NEED_NONE: u8 = 0xff;
    pub const SEARCH_RADIUS: i32 = 0xb504;
    pub const SEARCH_FLAG: i32 = -1;
    pub const SLOT_SET_TARGET: u32 = 0x2e4;
    /// Building position words: x (`+0x114`), y (`+0x118`), z (`+0x11c`).
    pub const BUILDING_X: u32 = 0x114;
    pub const BUILDING_Y: u32 = 0x118;
    pub const BUILDING_Z: u32 = 0x11c;
}

/// Fields `doEscapedCheck` touches.
mod escaped {
    pub const COUNTDOWN: u32 = 0x304;
    /// `DAT_006393e0`.
    pub const PERIOD: u32 = 0x0023_93e0;
    /// Rampage counter (`+0x35c`), the rampage flag (`+0x3b0`) and its companion byte (`+0x3b1`).
    pub const RAMPAGE_COUNTER: u32 = 0x35c;
    pub const BYTE_RAMPAGING: u32 = 0x3b0;
    pub const BYTE_RAMPAGE_AUX: u32 = 0x3b1;
    pub const ESCAPE_TIMER: u32 = 0x374;
    pub const UNIT_WORD_22C: u32 = 0x22c;
    pub const BYTE_FLEEING: u32 = 0x24c;
    pub const BYTE_PLAYMATE: u32 = 0x3a4;
    pub const BYTE_CHASE_AUX: u32 = 0x3a5;
    pub const BYTE_CAPTURE_STAGE: u32 = 0x393;
    pub const GOAL_CHASE_PLAYMATE: i32 = 0x74;
    pub const GOAL_PLAYMATE: i32 = 0x75;
    /// Type fields: rampage happiness ceiling (`+0x3f4`), start chance (`+0x3f8`), minimum (`+0x3fc`) and maximum
    /// (`+0x400`) duration.
    pub const TYPE_RAMPAGE_HAPPINESS: u32 = 0x3f4;
    pub const TYPE_RAMPAGE_CHANCE: u32 = 0x3f8;
    pub const TYPE_RAMPAGE_MIN: u32 = 0x3fc;
    pub const TYPE_RAMPAGE_MAX: u32 = 0x400;
    pub const SLOT_FIND_PREDATOR: u32 = 0x2fc;
    pub const SLOT_FIND_PLAYMATE: u32 = 0x2f8;
    pub const SLOT_HAS_PRESSING_NEED: u32 = 0x288;
    pub const SLOT_NOTIFY_PREDATOR: u32 = 0x2c4;
    pub const SLOT_CLEAR_GOAL: u32 = 0x174;
    pub const RAMPAGE_BEHAVIOURS: [&[u8]; 2] = [b"bEscapedRampage", b"bHabitatRampage"];
    /// `ZooStatus` lives at this offset of `GLOBAL_ZTGameMgr`.
    pub const ZOO_STATUS_OFFSET: u32 = 0x10;
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

    /// `ZTAnimal::fMateAvailable`: [`Self::f_get_mate`] found a mate.
    pub fn f_mate_available(self, flag: *const bool) -> bool {
        self.f_get_mate(flag as u32) != 0
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

    /// `ZTAnimal::clearEscapeInfo` (`0x0043e763`): clears the has-escape-info byte, frees the escape record through
    /// vanilla's `operator delete` and nulls the pointer.
    pub fn clear_escape_info(self) {
        if self.flag(animal_offsets::HAS_ESCAPE_INFO) {
            save_to_memory(self.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
        }
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record != 0 {
            unsafe { OPERATOR_DELETE.original()(record) };
            save_to_memory(self.0 + animal_offsets::ESCAPE_RECORD, 0u32);
        }
    }

    /// `ZTAnimal::clearGoal` (`0x0043e784`): `ZTUnit::clearGoal`, then the goal bytes, `+0x24c`, the prey id and
    /// `+0x22c` are zeroed and the escape info cleared.
    pub fn clear_goal(self) {
        unsafe { ZTUNIT_CLEAR_GOAL.original()(self.0 as *const u32) };
        for offset in lifecycle::CLEAR_BYTES {
            save_to_memory(self.0 + offset, 0u8);
        }
        save_to_memory(self.0 + lifecycle::UNIT_FLAG_24C, 0u8);
        save_to_memory(self.0 + animal_offsets::PREY_ID, 0u32);
        save_to_memory(self.0 + lifecycle::UNIT_WORD_22C, 0u32);
        self.clear_escape_info();
    }

    /// `ZTAnimal::clearStateVariables` (`0x0046adf2`): the prey id is zeroed, then `ZTUnit::clearStateVariables`.
    pub fn clear_state_variables(self) {
        save_to_memory(self.0 + animal_offsets::PREY_ID, 0u32);
        unsafe { ZTUNIT_CLEAR_STATE_VARIABLES.original()(self.0 as *const u32) };
    }

    /// `ZTAnimal::setDeathCheck` (`0x0059b225`): `change_type` `0` adds `value` to the death countdown, `1` sets it, `2`
    /// (while it is positive) adds `value` percent of it (truncated toward zero); anything else does nothing.
    pub fn set_death_check(self, value: i32, change_type: i32) {
        let current: i32 = self.read(lifecycle::DEATH_CHECK);
        match change_type {
            0 => save_to_memory(self.0 + lifecycle::DEATH_CHECK, current.wrapping_add(value)),
            1 => save_to_memory(self.0 + lifecycle::DEATH_CHECK, value),
            2 if current > 0 => save_to_memory(self.0 + lifecycle::DEATH_CHECK, current.wrapping_mul(value).wrapping_div(100).wrapping_add(current)),
            _ => {}
        }
    }

    /// `ZTAnimal::setEgg` (`0x005a5283`): marks the animal an egg (`+0x395`), resets the `+0x8c` block, stops
    /// everything (`1, 1, 1`), empties the vector at `+0x260`, runs `updateAnimation(0)`; when `reset_timer` is set the
    /// hatch timer `+0x398` is the type's `+0x408` (or `5` while `DAT_0063cfa1` is set). Health becomes
    /// `base + timer - 1` (base: the type's `+0x420`, or 60% of its `+0x234 - +0x238`), capped at the type's `+0x234`
    /// and at `(100 - sick percent)%` of it.
    pub fn set_egg(self, reset_timer: bool) {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        save_to_memory(self.0 + animal_offsets::EGG, 1u8);
        unsafe { transmute::<u32, extern "thiscall" fn(u32)>(lifecycle::RESET_BLOCK_8C)(self.0 + lifecycle::BLOCK_8C) };
        call_slot_3(self.0, lifecycle::SLOT_STOP_EVERYTHING, 1, 1, 1);
        save_to_memory(self.0 + lifecycle::VEC_END, self.read::<u32>(lifecycle::VEC_BEGIN));
        call_slot_1(self.0, lifecycle::SLOT_UPDATE_ANIMATION, 0);
        if reset_timer {
            let timer = if get_from_memory::<u8>(base() + lifecycle::EGG_TIMER_OVERRIDE) == 0 {
                Self::type_field::<i32>(entity_type, lifecycle::EGG_TYPE_TIMER)
            } else {
                5
            };
            save_to_memory(self.0 + lifecycle::EGG_TIMER, timer);
        }
        let max_hits = Self::type_field::<i32>(entity_type, type_offsets::MAX_HITS);
        let mut base_health = Self::type_field::<i32>(entity_type, lifecycle::EGG_TYPE_HEALTH_BASE);
        if base_health == 0 {
            let range = max_hits.wrapping_sub(Self::type_field::<i32>(entity_type, type_offsets::HEALTH_MIN));
            base_health = range.wrapping_mul(60).wrapping_div(100);
        }
        let timer: i32 = self.read(lifecycle::EGG_TIMER);
        let mut health = base_health.wrapping_add(timer).wrapping_sub(1);
        if max_hits < health {
            health = max_hits;
        }
        let sick_percent = get_from_memory::<i32>(base() + globals_rva::SICK_PERCENT).clamp(0, 100);
        let limit = (100 - sick_percent).wrapping_mul(max_hits).wrapping_div(100);
        if limit < health {
            health = limit;
        }
        save_to_memory(self.0 + animal_offsets::HEALTH, health);
    }

    /// `ZTAnimal::setEscaped` (`0x004409f8`): on a change of the escaped byte (`+0x3a3`) a cleared flag unlinks every
    /// node holding this animal from the escaped-animal list (`DAT_00638fb0`) and a set flag appends it and clears
    /// `+0x3a1`/`+0x3a2`; the selection notifier (`0x00440a27`) always runs.
    pub fn set_escaped(self, flag: u8) {
        if self.read::<u8>(lifecycle::ESCAPED) != flag {
            save_to_memory(self.0 + lifecycle::ESCAPED, flag);
            let sentinel: u32 = get_from_memory(base() + lifecycle::ESCAPE_LIST);
            if flag == 0 {
                let mut node: u32 = get_from_memory(sentinel);
                while node != sentinel {
                    let next: u32 = get_from_memory(node);
                    if get_from_memory::<u32>(node + 8) == self.0 {
                        let previous: u32 = get_from_memory(node + 4);
                        save_to_memory(previous, next);
                        save_to_memory(next + 4, previous);
                        ZTGuide::free_node(node);
                    }
                    node = next;
                }
            } else {
                let mut result: u32 = 0;
                let value = self.0;
                unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &value as *const u32 as *const i32) };
                save_to_memory(self.0 + lifecycle::ESCAPE_STAGE_A, 0u8);
                save_to_memory(self.0 + lifecycle::ESCAPE_STAGE_B, 0u8);
            }
        }
        unsafe { transmute::<u32, extern "cdecl" fn(u32, u32)>(lifecycle::SELECTION_NOTIFY)(self.0, flag as u32) };
    }

    /// Whether the unit's need byte (`+0x24e`) names the need at `need_rva` and the unit has no building-use goal.
    fn need_matches_without_building_goal(self, need_rva: u32) -> bool {
        let index = unsafe { GET_NEED_INDEX.hooked()((base() + need_rva) as *const i8) } as u8;
        self.read::<u8>(lifecycle::UNIT_NEED_BYTE) == index && !unsafe { ZTUNIT_HAS_BUILDING_USE_GOAL.hooked()(self.0 as *const u32) }
    }

    /// `ZTAnimal::abortGoal` (`0x005adb0a`): with `flag`, a unit whose need byte names the first need
    /// (`DAT_0063afc8`), has no building-use goal and whose tile and position satisfy `isAnimalRestSatisfier` gets
    /// `clearGoal` then `findPlaceToRest(false)`; failing that, the second need (`DAT_006463a0`) and
    /// `0x0061349c` give `clearGoal` then `targetBuilding(false)`. When one of those finds a destination the abort
    /// is done; otherwise (and for a clear `flag`) `abortTrick` runs if the unit is doing a show, then `clearGoal`.
    /// Every path ends by zeroing `+0x24c`/`+0x3a4`/`+0x22c` around a `clearPrey` call.
    pub fn abort_goal(self, flag: bool) {
        if flag {
            let position: [u32; 3] = [self.read(lifecycle::UNIT_REST_POS), self.read(lifecycle::UNIT_REST_POS + 4), self.read(lifecycle::UNIT_REST_POS + 8)];
            if self.need_matches_without_building_goal(lifecycle::NEED_A) {
                let tile: u32 = self.read(lifecycle::UNIT_REST_TILE);
                if low_byte_bool(unsafe { IS_ANIMAL_REST_SATISFIER.hooked()(tile as *const u32, position.as_ptr()) }) {
                    call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
                    let found = unsafe { FIND_PLACE_TO_REST.hooked()(self.0 as *const u32, false) };
                    return self.finish_abort(low_byte_bool(found));
                }
            }
            if self.need_matches_without_building_goal(lifecycle::NEED_B) {
                let tile: u32 = self.read(lifecycle::UNIT_REST_TILE);
                let satisfied = unsafe { transmute::<u32, extern "cdecl" fn(u32, *const u32) -> u32>(lifecycle::REST_SATISFIER_2)(tile, position.as_ptr()) };
                if low_byte_bool(satisfied) {
                    call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
                    let found = unsafe { TARGET_BUILDING.hooked()(self.0 as *const u32, false) };
                    return self.finish_abort(found);
                }
            }
        }
        self.finish_abort(false);
    }

    /// `ZTAnimal::save` (`0x00478cff`): every field in the vanilla order, all writes run even after a failure; then
    /// `ZTUnit::save`. The two strings (`+0x26c`, `+0x278`) are a length (`< 0x1000`, else that string fails and
    /// writes nothing) and the bytes; the escape record is written only while `+0x391` is set; the home gate tile's
    /// two coordinates (the type's defaults, `DAT_00639370`/`DAT_00639374`, for none) close the animal's own data.
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = true;
        let at = |offset: u32| self.0 + offset;
        let write = |address: u32, size: u32| write_field(address, size, file);
        for &(offset, size) in save_layout::BEFORE_STRINGS {
            ok &= write(at(offset), size);
        }
        for (begin_offset, end_offset) in [(0x26cu32, 0x270u32), (0x278, 0x27c)] {
            let begin: u32 = self.read(begin_offset);
            let length = self.read::<u32>(end_offset).wrapping_sub(begin);
            if length < 0x1000 {
                ok &= write(&length as *const u32 as u32, 4);
                if length != 0 {
                    ok &= write(begin, length);
                }
            } else {
                ok = false;
            }
        }
        for &(offset, size) in save_layout::AFTER_STRINGS_A {
            ok &= write(at(offset), size);
        }
        let has_record = self.flag(animal_offsets::HAS_ESCAPE_INFO);
        ok &= write(&(has_record as u32) as *const u32 as u32, 4);
        if has_record {
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            let tiles = (record != 0).then(|| (get_from_memory::<u32>(record + 4), get_from_memory::<u32>(record + 8)));
            match tiles {
                Some((first, second)) if first != 0 && second != 0 => {
                    for address in [first + 0x34, first + 0x38, second + 0x34, second + 0x38, record + 0xc] {
                        ok &= write(address, 4);
                    }
                    for slot in 1..4u32 {
                        let set = (get_from_memory::<u8>(record + slot) != 0) as u32;
                        ok &= write(&set as *const u32 as u32, 4);
                    }
                }
                // Vanilla faults on a missing record or tile.
                _ => ok = false,
            }
        }
        for &(offset, size) in save_layout::AFTER_STRINGS_B {
            ok &= write(at(offset), size);
        }
        let gate: u32 = self.read(animal_offsets::HOME_GATE_TILE);
        let (x, y): (u32, u32) = if gate != 0 {
            (get_from_memory(gate + 0x34), get_from_memory(gate + 0x38))
        } else {
            (get_from_memory(base() + save_layout::DEFAULT_GATE_X), get_from_memory(base() + save_layout::DEFAULT_GATE_Y))
        };
        ok &= write(&x as *const u32 as u32, 4);
        ok &= write(&y as *const u32 as u32, 4);
        let unit_ok = unsafe { ZTUNIT_SAVE.original()(self.0 as *const u32, file) };
        unit_ok & ok
    }

    fn finish_abort(self, found: bool) {
        if !found {
            if low_byte_bool(call_slot_0(self.0, lifecycle::SLOT_IS_DOING_SHOW)) {
                call_slot_0(self.0, lifecycle::SLOT_ABORT_TRICK);
            }
            call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
        }
        save_to_memory(self.0 + lifecycle::UNIT_FLAG_24C, 0u8);
        save_to_memory(self.0 + lifecycle::PENDING_TARGET_FLAG, 0u8);
        call_slot_0(self.0, lifecycle::SLOT_CLEAR_PREY);
        save_to_memory(self.0 + lifecycle::UNIT_WORD_22C, 0u32);
    }
}

impl ZTAnimal {
    fn update_flag_words(self, f: impl Fn(u32) -> u32, offsets: &[u32]) {
        for &offset in offsets {
            let value: u32 = self.read(offset);
            save_to_memory(self.0 + offset, f(value));
        }
    }

    /// `ZTAnimal::doBreathCheck` (`0x004389f7`): the breath countdown (`+0x364`) ticks down; when it was `0` and the
    /// animal is underwater (terrain mode `4`), it reloads from `DAT_00639450 - 1` and the breath value (`+0x360`)
    /// moves by the type's step, clamped to the type's max then min. The countdown is only reloaded in that branch.
    pub fn do_breath_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(pipeline::BREATH_COUNTDOWN);
        save_to_memory(self.0 + pipeline::BREATH_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 || self.read::<u32>(unit_fields::TERRAIN_MODE) != pipeline::MODE_UNDERWATER {
            return;
        }
        let period: i32 = get_from_memory(base() + pipeline::BREATH_PERIOD);
        save_to_memory(self.0 + pipeline::BREATH_COUNTDOWN, period.wrapping_sub(1));
        let mut breath: i32 = self.read::<i32>(animal_offsets::BREATH).wrapping_add(Self::type_field::<i32>(ty, pipeline::TYPE_BREATH_STEP));
        save_to_memory(self.0 + animal_offsets::BREATH, breath);
        let max = Self::type_field::<i32>(ty, pipeline::TYPE_BREATH_MAX);
        if breath > max {
            breath = max;
            save_to_memory(self.0 + animal_offsets::BREATH, breath);
        }
        let min = Self::type_field::<i32>(ty, pipeline::TYPE_BREATH_MIN);
        if breath < min {
            save_to_memory(self.0 + animal_offsets::BREATH, min);
        }
    }

    /// `ZTAnimal::doKeeperCheck` (`0x00438d31`): when its countdown (`+0x30c`) was `0` it reloads from
    /// `DAT_00639410 - 1`; if the keeper count (`+0x378`) is below the type's requirement, the animal is not an egg and
    /// vtable `+0x22c` answers true, the type's happiness change is added to the pending happiness. The vanilla
    /// "Not enough keepers" log string goes to a null sink and is not reproduced.
    pub fn do_keeper_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(pipeline::KEEPER_COUNTDOWN);
        save_to_memory(self.0 + pipeline::KEEPER_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + pipeline::KEEPER_PERIOD);
        save_to_memory(self.0 + pipeline::KEEPER_COUNTDOWN, period.wrapping_sub(1));
        if self.read::<i32>(pipeline::KEEPER_COUNT) >= Self::type_field::<i32>(ty, pipeline::TYPE_KEEPERS_NEEDED) {
            return;
        }
        if !self.flag(animal_offsets::EGG) && low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            let pending: i32 = self.read(animal_offsets::HAPPINESS_PENDING);
            save_to_memory(
                self.0 + animal_offsets::HAPPINESS_PENDING,
                pending.wrapping_add(Self::type_field::<i32>(ty, pipeline::TYPE_NO_KEEPER_HAPPINESS)),
            );
        }
    }

    /// `ZTAnimal::doOtherCheck` (`0x00438e70`): when its countdown (`+0x308`) was `0`, bit 3 of the low and critical
    /// status words is cleared and the countdown reloads from `DAT_00639424 - 1`. If another animal in the habitat is
    /// angry and the type's angry-neighbour change (`+0x2f4`) is negative, that change is added to the pending
    /// happiness and bit 3 of the low words is set (and of the critical words when the change is `-10` or lower).
    pub fn do_other_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(pipeline::OTHER_COUNTDOWN);
        save_to_memory(self.0 + pipeline::OTHER_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let bit = pipeline::FLAG_WORDS_ANGER_BIT;
        self.update_flag_words(|v| v & !bit, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
        let period: i32 = get_from_memory(base() + pipeline::OTHER_PERIOD);
        save_to_memory(self.0 + pipeline::OTHER_COUNTDOWN, period.wrapping_sub(1));
        let change = Self::type_field::<i32>(ty, pipeline::TYPE_OTHER_ANGRY_HAPPINESS);
        if !self.f_other_animal_angry() || change >= 0 {
            return;
        }
        let pending: i32 = self.read(animal_offsets::HAPPINESS_PENDING);
        save_to_memory(self.0 + animal_offsets::HAPPINESS_PENDING, pending.wrapping_add(change));
        self.update_flag_words(|v| v | bit, &[animal_offsets::STATUS_FLAGS]);
        if change <= -10 {
            self.update_flag_words(|v| v | bit, &[animal_offsets::STATUS_FLAGS_CRITICAL]);
        }
    }

    /// `ZTAnimal::doEggCheck` (`0x00437860`): an animal that is not marked not-selectable (`+0x3a6`) and whose health
    /// is below the type's egg health base (`+0x420`, else 60% of `max_hits - min`) does nothing. Otherwise, for an egg
    /// (`+0x395`), the hatch timer (`+0x398`) counts down; once it was `<= 0` and the habitat (vtable `+0x24c`)
    /// exists and its `+0x2c` byte is clear, a not-selectable egg is removed and any other hatches (`hatchEgg`).
    pub fn do_egg_check(self) {
        let Some(ty) = self.animal_type() else { return };
        if !self.flag(animal_offsets::NOT_SELECTABLE) {
            let mut threshold = Self::type_field::<i32>(ty, pipeline::TYPE_EGG_HEALTH_BASE);
            if threshold == 0 {
                let range = Self::type_field::<i32>(ty, type_offsets::MAX_HITS).wrapping_sub(Self::type_field::<i32>(ty, type_offsets::HEALTH_MIN));
                threshold = range.wrapping_mul(60).wrapping_div(100);
            }
            if self.health() < threshold {
                return;
            }
        }
        if !self.flag(animal_offsets::EGG) {
            return;
        }
        let timer: i32 = self.read(lifecycle::EGG_TIMER);
        save_to_memory(self.0 + lifecycle::EGG_TIMER, timer.wrapping_sub(1));
        if timer > 0 {
            return;
        }
        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
        if habitat == 0 || get_from_memory::<u8>(habitat + pipeline::HABITAT_NO_HATCH) != 0 {
            return;
        }
        if self.flag(animal_offsets::NOT_SELECTABLE) {
            unsafe { ZTUNIT_SET_IS_REMOVED.original()(self.0 as *const u32, 1) };
        } else {
            unsafe { HATCH_EGG.original()(self.0 as *const u32) };
        }
    }
}

impl ZTAnimal {
    fn add_pending(self, offset: u32, delta: i32) {
        let current: i32 = self.read(offset);
        save_to_memory(self.0 + offset, current.wrapping_add(delta));
    }

    /// `ZTAnimal::doHabitatCheck` (`0x00438c47`): the compatible-animals rating, clamped to `-500..=150` and divided
    /// by ten (toward zero), sets bit `0x400` of the low status words when negative and of the critical words at
    /// `-10` or lower (both cleared first). Its countdown (`+0x314`) then ticks; on firing it reloads from
    /// `DAT_00639400 - 1` and, for a non-egg that is not doing a show (vtable `+0x22c`), adds one of the type's happiness
    /// changes by where `getHabitatRating` falls against the type minimum (`+0x2bc`) and the dislike product (`+0x310`),
    /// posting zoo message `0x272f` for the worst band when the habitat's `+0x2c` byte is clear. The clamped
    /// compatibility change is then added to the pending happiness under the same egg and show test; that final
    /// addition only runs on the firing tick (the non-firing path returns right after the countdown).
    pub fn do_habitat_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let compat = self.f_compat_animals_rating().clamp(-500, 150);
        let change = compat.wrapping_div(10);
        let bit = pipeline::BIT_HABITAT;
        self.update_flag_words(|v| v & !bit, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
        if change < 0 {
            self.update_flag_words(|v| v | bit, &[animal_offsets::STATUS_FLAGS]);
            if change <= -10 {
                self.update_flag_words(|v| v | bit, &[animal_offsets::STATUS_FLAGS_CRITICAL]);
            }
        }
        let countdown: i32 = self.read(pipeline::HABITAT_COUNTDOWN);
        save_to_memory(self.0 + pipeline::HABITAT_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown == 0 {
            let period: i32 = get_from_memory(base() + pipeline::HABITAT_PERIOD);
            save_to_memory(self.0 + pipeline::HABITAT_COUNTDOWN, period.wrapping_sub(1));
            let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
            let rating = self.habitat_rating_in(habitat);
            if self.flag(animal_offsets::EGG) {
                return;
            }
            if !low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
                let minimum = Self::type_field::<i32>(ty, pipeline::TYPE_RATING_MIN);
                if rating < minimum {
                    let dislike = Self::type_field::<i32>(ty, pipeline::TYPE_RATING_DISLIKE_FACTOR);
                    if rating.wrapping_mul(100) < dislike.wrapping_mul(minimum) {
                        self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_HAPPY_BAD));
                        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
                        if habitat != 0 && get_from_memory::<u8>(habitat + pipeline::HABITAT_NO_HATCH) == 0 {
                            unsafe { ZTUNIT_F_ZOO_MESSAGE_NAME.original()(self.0 as *const u32, pipeline::MESSAGE_ID, 2, 0) };
                        }
                    } else {
                        self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_HAPPY_MID));
                    }
                } else {
                    self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_HAPPY_LOW));
                }
            }
            if !self.flag(animal_offsets::EGG) && !low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
                self.add_pending(animal_offsets::HAPPINESS_PENDING, change);
            }
        }
    }

    /// `ZTAnimal::doMurkyWaterCheck` (`0x00438a37`): when its countdown (`+0x2fc`) was `0` it reloads from
    /// `DAT_006393e4 - 1`. For a tank habitat (vtable `+0x20`), the habitat's `+0x1a8` value picks the first of three
    /// bands (below the type's `+0x42c`, else `+0x428`, else `+0x424`) and adds that band's happiness and health
    /// changes to the pending words. For a habitat that is neither a tank with `+0x4` set nor under a show, a habitat
    /// `+0x188` value outside the type's `[+0x35c, +0x360]` range (when that range is non-empty) adds `+0x364`
    /// happiness. A habitat whose type predicate (vtable `+0x28`, taking the animal type) is false adds `+0x368`
    /// happiness and `+0x36c` health.
    pub fn do_murky_water_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(pipeline::MURKY_COUNTDOWN);
        save_to_memory(self.0 + pipeline::MURKY_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + pipeline::MURKY_PERIOD);
        save_to_memory(self.0 + pipeline::MURKY_COUNTDOWN, period.wrapping_sub(1));
        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
        if habitat == 0 || !low_byte_bool(call_slot_0(habitat, pipeline::HABITAT_SLOT_IS_TANK)) {
            return;
        }
        let murk: i32 = get_from_memory(habitat + pipeline::HABITAT_FIELD_1A8);
        for band in 0..3 {
            if murk < Self::type_field::<i32>(ty, pipeline::TYPE_MURKY_THRESHOLDS[band]) {
                self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_MURKY_HAPPINESS[band]));
                self.add_pending(animal_offsets::HEALTH_PENDING, Self::type_field(ty, pipeline::TYPE_MURKY_HEALTH[band]));
                break;
            }
        }
        let is_tank_with_field = low_byte_bool(call_slot_0(habitat, pipeline::HABITAT_SLOT_IS_TANK)) && get_from_memory::<u32>(habitat + pipeline::HABITAT_FIELD_4) != 0;
        if !is_tank_with_field && !low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            let low = Self::type_field::<i32>(ty, pipeline::TYPE_RANGE_LOW);
            let high = Self::type_field::<i32>(ty, pipeline::TYPE_RANGE_HIGH);
            if low != high {
                let value: i32 = get_from_memory(habitat + pipeline::HABITAT_FIELD_188);
                if value < low || value > high {
                    self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_RANGE_HAPPINESS));
                }
            }
        }
        if !low_byte_bool(call_slot_1(habitat, pipeline::HABITAT_SLOT_TYPE_OK, ty)) {
            self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, pipeline::TYPE_NOT_OK_HAPPINESS));
            self.add_pending(animal_offsets::HEALTH_PENDING, Self::type_field(ty, pipeline::TYPE_NOT_OK_HEALTH));
        }
    }
}

impl ZTAnimal {
    /// Whether the animal may post status messages and flags: not doing a show (vtable `+0x22c`), not on a show goal
    /// (`+0x170` of `0x85`/`0x86`) and neither gate byte (`+0x24f`, `+0x250`) set.
    fn hunger_gate_open(self, goal: i32) -> bool {
        !low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW))
            && !hunger::GOALS_SHOW.contains(&goal)
            && hunger::UNIT_BYTES_MESSAGE_GATE.iter().all(|&offset| !self.flag(offset))
    }

    /// `ZTAnimal::doHungerCheck` (`0x00438a79`): nothing for an egg.
    /// Otherwise the countdown (`+0x2f4`) ticks; on firing it reloads from `DAT_0063940c - 1`, adds
    /// `+0x2d4 * type[+0x214]` to the pending hunger and, when the animal is hungry (`hunger >= type[+0x20c]`) and
    /// neither eating (`+0x3a8`) nor already wanting food (`+0x3a9`), searches for food: skipped on goals `0x72`/`0x73`,
    /// or while `+0x39f` or `+0x3b0` is set; with keeper food available (`fIsThereKeeperFood`) it marks `+0x39f` (and
    /// clears `+0x3a0`..`+0x3a2`) when the show gate is open; with none it sets `+0x39e` the first time, and afterwards
    /// posts message `0x272e` (gate open), raises bit 0 of the low and critical status words, and, from the
    /// starving threshold (`+0x240`), adds the type's happiness and subtracts its health, then clears `+0x39f`/`+0x3a0`.
    /// A wanting animal (`+0x3a9`) calls `fEat`. Every firing tick then adds `type[+0x328]` to the dirt pending
    /// (`+0x2cc`) and, once the dirt counter (`+0x2c8`) reaches `type[+0x32c]`, `putDownDirt(0)` resets it on success.
    /// The vanilla "Can't find any food"/"Found keeper food" log strings are dropped.
    pub fn do_hunger_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let goal: i32 = self.read(hunger::GOAL);
        if self.flag(animal_offsets::EGG) {
            return;
        }
        let countdown: i32 = self.read(hunger::COUNTDOWN);
        save_to_memory(self.0 + hunger::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + hunger::PERIOD);
        save_to_memory(self.0 + hunger::COUNTDOWN, period.wrapping_sub(1));
        let step = self.read::<i32>(hunger::HUNGER_RATE).wrapping_mul(Self::type_field::<i32>(ty, hunger::TYPE_HUNGER_PENDING_STEP));
        self.add_pending(hunger::HUNGER_PENDING, step);
        let hungry = !self.flag(hunger::BYTE_EATING) && self.hunger() >= Self::type_field::<i32>(ty, hunger::TYPE_HUNGRY_AT);
        if hungry && !self.flag(hunger::BYTE_FOOD_WANTED) {
            if !self.flag(hunger::BYTE_KEEPER_FOOD) && !hunger::GOALS_NO_EAT.contains(&goal) && !self.flag(hunger::BYTE_BLOCKED) {
                if self.f_is_there_keeper_food() {
                    if self.hunger_gate_open(goal) {
                        save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD, 1u8);
                        save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD_2, 0u8);
                        save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD_3, 0u8);
                        save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD_4, 0u8);
                    }
                } else {
                    if !self.flag(hunger::BYTE_NO_FOOD_SEEN) {
                        save_to_memory(self.0 + hunger::BYTE_NO_FOOD_SEEN, 1u8);
                        save_to_memory(self.0 + hunger::BYTE_FOOD_WANTED, 0u8);
                    } else {
                        if self.hunger_gate_open(goal) {
                            unsafe { ZTUNIT_F_ZOO_MESSAGE_NAME.original()(self.0 as *const u32, hunger::MESSAGE_HUNGRY, 2, 0) };
                        }
                        self.update_flag_words(|v| v | 1, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
                        if self.hunger() >= Self::type_field::<i32>(ty, hunger::TYPE_STARVING_AT) {
                            self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, hunger::TYPE_STARVING_HAPPINESS));
                            self.add_pending(animal_offsets::HEALTH_PENDING, Self::type_field::<i32>(ty, hunger::TYPE_STARVING_HEALTH).wrapping_neg());
                        }
                    }
                    save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD, 0u8);
                    save_to_memory(self.0 + hunger::BYTE_KEEPER_FOOD_2, 0u8);
                    // The log branch only evaluates the show gate (a vtable call) before building a discarded string.
                    let _ = self.hunger_gate_open(goal);
                }
            }
        } else if self.flag(hunger::BYTE_FOOD_WANTED) {
            unsafe { F_EAT.original()(self.0 as *const u32) };
        }
        self.add_pending(hunger::DIRT_PENDING, Self::type_field(ty, hunger::TYPE_DIRT_STEP));
        if self.read::<i32>(hunger::DIRT_COUNTER) >= Self::type_field::<i32>(ty, hunger::TYPE_DIRT_AT)
            && unsafe { PUT_DOWN_DIRT.original()(self.0 as *const u32, std::ptr::null()) }
        {
            save_to_memory(self.0 + hunger::DIRT_COUNTER, 0u32);
        }
    }
}

impl ZTAnimal {
    /// `value += pending`, clamped to the type's max then min. Returns nothing; the pending word is zeroed by the caller
    /// in vanilla's order (see `update_status_variables`).
    fn fold_need(self, ty: u32, need: (u32, u32, u32, u32)) {
        let (value, pending, max, min) = need;
        let mut current: i32 = self.read::<i32>(value).wrapping_add(self.read::<i32>(pending));
        save_to_memory(self.0 + value, current);
        let max = Self::type_field::<i32>(ty, max);
        if current > max {
            current = max;
            save_to_memory(self.0 + value, current);
        }
        let min = Self::type_field::<i32>(ty, min);
        if current < min {
            save_to_memory(self.0 + value, min);
        }
    }

    /// `ZTAnimal::setTerrainMode` is still vanilla; `updateStatusVariables` calls it with `(0, 0)` at minimum health.
    fn call_set_terrain_mode(self, mode: i32, flag: i8) {
        unsafe { SET_TERRAIN_MODE.original()(self.0 as *const u32, mode, flag) };
    }

    /// `ZTAnimal::updateStatusVariables` (`0x004378df`): folds each pending delta into its need and clamps it to the
    /// type's min/max, zeroing the pending word right after the previous need is stored (happiness pending first, then
    /// hunger's, the `+0x2c4` and `+0x2cc` words in turn). An egg (`+0x395`) skips the four needs and loses one health
    /// point instead (`+0x2bc -= 1`). Health is then folded the same way, its pending word zeroed, `+0x3a7` set when it
    /// equals the type's minimum, and in that case health is reset to the minimum and `setTerrainMode(0, 0)` runs.
    /// `+0x3aa` (angry) is rewritten as `happiness <= -0x50` after the needs, but not for an egg.
    pub fn update_status_variables(self) {
        let Some(ty) = self.animal_type() else { return };
        if self.flag(animal_offsets::EGG) {
            let pending: i32 = self.read(status::HEALTH.1);
            save_to_memory(self.0 + status::HEALTH.1, pending.wrapping_sub(1));
        } else {
            self.fold_need(ty, status::HAPPINESS);
            save_to_memory(self.0 + status::HAPPINESS.1, 0u32);
            self.fold_need(ty, status::HUNGER);
            save_to_memory(self.0 + status::HUNGER.1, 0u32);
            self.fold_need(ty, status::NEED_C0);
            save_to_memory(self.0 + status::NEED_C0.1, 0u32);
            self.fold_need(ty, status::NEED_C8);
            save_to_memory(self.0 + status::NEED_C8.1, 0u32);
            save_to_memory(self.0 + animal_offsets::ANGRY, (self.happiness() <= status::ANGRY_HAPPINESS) as u8);
        }
        self.fold_need(ty, status::HEALTH);
        save_to_memory(self.0 + status::HEALTH.1, 0u32);
        let at_minimum = self.health() == Self::type_field::<i32>(ty, status::HEALTH.3);
        save_to_memory(self.0 + animal_offsets::SICK, at_minimum as u8);
        if at_minimum {
            save_to_memory(self.0 + animal_offsets::HEALTH, Self::type_field::<i32>(ty, status::HEALTH.3));
            self.call_set_terrain_mode(0, 0);
        }
    }
}

impl ZTAnimal {
    /// `ZTAnimal::doBoredCheck` (`0x00438ad7`): nothing for an egg or while doing a show (vtable `+0x22c`). Otherwise its
    /// countdown (`+0x318`) ticks; on firing, bit `0x40` of the low and critical status words is cleared and the
    /// countdown reloads from `DAT_00639414 - 1`. A need (`+0x2c0`) at the type's `+0x324` hands over to `doSleepCheck`
    /// (still vanilla). A sick animal stops. Otherwise: a predator (`+0x2fc(0)`) sets the fleeing byte (`+0x24c`); a
    /// nearer one (`+0x2fc(1)`, with no pressing need, playmate, escape or flee flag) does too unless blocked (`+0x3b0`).
    /// If still not fleeing, pressing-need-free, unblocked and not hungry, a roll `r = rand() % limit + 1` over the type's
    /// `+0x3a8 + +0x3ac` (the extra drops to `0` when the habitat holds no other animal of the species, which also aborts a
    /// chase-playmate goal `0x74`) decides: `r <= +0x3a8` wishes for water (`+0x3a1`) on a walkable tile outside modes `1`
    /// and `2` that `+0x2c8` accepts, a larger `r` seeks a playmate (`+0x2f8(1)` sets `+0x3a4`). The log strings are dropped.
    pub fn do_bored_check(self) {
        let Some(ty) = self.animal_type() else { return };
        if self.flag(animal_offsets::EGG) || low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let countdown: i32 = self.read(bored::COUNTDOWN);
        save_to_memory(self.0 + bored::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let bit = bored::BIT_BORED;
        self.update_flag_words(|v| v & !bit, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
        let period: i32 = get_from_memory(base() + bored::PERIOD);
        save_to_memory(self.0 + bored::COUNTDOWN, period.wrapping_sub(1));
        if self.read::<i32>(bored::NEED) >= Self::type_field::<i32>(ty, bored::TYPE_SLEEP_AT) {
            unsafe { DO_SLEEP_CHECK.original()(self.0 as *const u32) };
            return;
        }
        if self.flag(animal_offsets::SICK) {
            return;
        }
        let set = |offset: u32, value: u8| save_to_memory(self.0 + offset, value);
        if !self.flag(bored::BYTE_FLEEING) {
            if call_slot_1(self.0, bored::SLOT_FIND_PREDATOR, 0) != 0 {
                set(bored::BYTE_FLEEING, 1);
                return;
            }
            if !self.flag(bored::BYTE_FLEEING)
                && !self.flag(bored::BYTE_PLAYMATE)
                && !self.flag(bored::BYTE_ESCAPED)
                && !low_byte_bool(call_slot_0(self.0, bored::SLOT_HAS_PRESSING_NEED))
                && call_slot_1(self.0, bored::SLOT_FIND_PREDATOR, 1) != 0
                && !self.flag(bored::BYTE_BLOCKED)
            {
                set(bored::BYTE_FLEEING, 1);
                return;
            }
        }
        if low_byte_bool(call_slot_0(self.0, bored::SLOT_HAS_PRESSING_NEED)) || self.flag(bored::BYTE_BLOCKED) || self.is_hungry() {
            return;
        }
        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
        let base_roll = Self::type_field::<i32>(ty, bored::TYPE_PLAY_BASE);
        let mut extra = Self::type_field::<i32>(ty, bored::TYPE_PLAY_EXTRA);
        let others = if habitat == 0 {
            0
        } else {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }
                .get_num_animals_by_species(Self::type_field(ty, type_offsets::SPECIES), false)
                .wrapping_sub(1)
        };
        let goal: i32 = self.read(hunger::GOAL);
        if others <= 0 {
            extra = 0;
            if goal == bored::GOAL_CHASE_PLAYMATE && low_byte_bool(unsafe { IN_FRIENDLY_CHASE.hooked()(self.0 as *const u32) }) {
                call_slot_0(self.0, bored::SLOT_CLEAR_GOAL);
                set(bored::BYTE_PLAYMATE, 0);
            }
        }
        let limit = base_roll.wrapping_add(extra);
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        if limit <= 0
            || low_byte_bool(call_slot_0(self.0, bored::SLOT_HAS_PRESSING_NEED))
            || goal == bored::GOAL_CHASE_PLAYMATE
            || goal == bored::GOAL_PLAYMATE
        {
            return;
        }
        let roll = (next_game_random() as i32).wrapping_rem(limit).wrapping_add(1);
        let busy = |this: Self| this.flag(bored::BYTE_PLAYMATE) || this.flag(bored::BYTE_FLEEING) || this.flag(bored::BYTE_ESCAPED);
        if roll <= base_roll {
            if busy(self) || get_from_memory::<u8>(tile + bored::TILE_FLAGS) & 3 != 0 || self.read::<i32>(animal_offsets::ACTIVITY_MODE) == 1 {
                return;
            }
            let tile_now = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
            if get_from_memory::<u8>(tile_now + bored::TILE_FLAGS) & 3 != 0
                || self.read::<i32>(animal_offsets::ACTIVITY_MODE) == 2
                || call_slot_0(self.0, bored::SLOT_CAN_DRINK) != 0
            {
                return;
            }
            set(bored::BYTE_WANTS_WATER, 1);
        } else {
            if roll > limit || busy(self) || call_slot_1(self.0, bored::SLOT_FIND_PLAYMATE, 1) == 0 {
                return;
            }
            set(bored::BYTE_PLAYMATE, 1);
        }
    }
}

impl ZTAnimal {
    /// `ratio * adults < other` in x87 extended precision, as vanilla's `FIMUL`/`FCOMP` evaluates it (unordered counts
    /// as less, `C0` set).
    fn x87_product_less(ratio: f32, factor: i32, other: f32) -> bool {
        let status: u32;
        unsafe {
            std::arch::asm!(
                "fld dword ptr [{ratio}]",
                "fimul dword ptr [{factor}]",
                "fcomp dword ptr [{other}]",
                "fnstsw ax",
                "and eax, 0x100",
                ratio = in(reg) &ratio,
                factor = in(reg) &factor,
                other = in(reg) &other,
                out("eax") status,
                options(nostack),
            );
        }
        status != 0
    }

    /// Sets `bit` in the low dword at `low_offset`; when the type's change is below `-5`, also in the critical dword.
    fn raise_flag(self, low_offset: u32, bit: u32, change: i32) {
        self.update_flag_words(|v| v | bit, &[low_offset]);
        if change < social::CRITICAL_BELOW {
            self.update_flag_words(|v| v | bit, &[low_offset + 8]);
        }
    }

    /// `ZTAnimal::doSocialCheck` (`0x004386a9`). Every call first clears the lonely (`0x10`), crowded-by-species (`0x20`)
    /// and crowded-tiles (`0x10000`) bits of both status flag sets and the crowded-by-guests bit (`0x1000000`, high
    /// dwords), then re-derives them: fewer same-species animals than the type's `+0x29c` sets lonely, otherwise a space
    /// ratio times the type's `+0x2a0` below the adult count sets crowded-by-species; fewer tiles per adult than `+0x314`
    /// sets crowded-tiles; and, for a non-egg not doing a show, guest crowding (`+0x368`) above the type's `+0x268` sets
    /// crowded-by-guests. Each also sets the critical bit when the type's matching happiness change is below `-5`. The
    /// countdown (`+0x324`) ticks (also for eggs); on firing, for a non-egg not doing a show, it reloads from
    /// `DAT_00639420 - 1`, adds the type's happiness changes for each raised bit to the pending happiness (lonely or else
    /// crowded-by-species, then crowded-tiles, then crowded-by-guests) and sets the direction code (`+0x37c`) from the
    /// signs of `+0x36c` and `+0x370` (`0..=7`; both zero leaves it). The log strings are dropped.
    pub fn do_social_check(self) {
        let Some(ty) = self.animal_type() else { return };
        for (bit, high) in [(social::BIT_LONELY, false), (social::BIT_CROWDED_SPECIES, false), (social::BIT_CROWDED_TILES, false), (social::BIT_CROWDED_GUESTS_HIGH, true)] {
            let offsets: [u32; 2] = if high { [0x29c, 0x2a4] } else { [animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL] };
            self.update_flag_words(|v| v & !bit, &offsets);
        }
        let field = |offset: u32| Self::type_field::<i32>(ty, offset);
        if self.f_num_same_animals() < field(social::TYPE_MIN_SAME_SPECIES) {
            self.raise_flag(animal_offsets::STATUS_FLAGS, social::BIT_LONELY, field(social::TYPE_LONELY_HAPPINESS));
        } else {
            let adults = self.f_num_same_adult_animals() as f32;
            let ratio = self.f_animal_space_ratio();
            if Self::x87_product_less(ratio, field(social::TYPE_SPACE_FACTOR), adults) {
                self.raise_flag(animal_offsets::STATUS_FLAGS, social::BIT_CROWDED_SPECIES, field(social::TYPE_CROWDED_SPECIES_HAPPINESS));
            }
        }
        if self.f_tiles_per_adult_animal() < field(social::TYPE_TILES_MIN) {
            self.raise_flag(animal_offsets::STATUS_FLAGS, social::BIT_CROWDED_TILES, field(social::TYPE_CROWDED_TILES_HAPPINESS));
        }
        if !self.flag(animal_offsets::EGG)
            && !low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW))
            && self.read::<i32>(social::GUEST_CROWDING) > field(social::TYPE_GUEST_TOLERANCE)
        {
            self.update_flag_words(|v| v | social::BIT_CROWDED_GUESTS_HIGH, &[0x29c]);
            if field(social::TYPE_CROWDED_GUESTS_HAPPINESS) < social::CRITICAL_BELOW {
                self.update_flag_words(|v| v | social::BIT_CROWDED_GUESTS_HIGH, &[0x2a4]);
            }
        }
        let countdown: i32 = self.read(social::COUNTDOWN);
        save_to_memory(self.0 + social::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 || self.flag(animal_offsets::EGG) || low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let period: i32 = get_from_memory(base() + social::PERIOD);
        save_to_memory(self.0 + social::COUNTDOWN, period.wrapping_sub(1));
        let flags: u32 = self.read(animal_offsets::STATUS_FLAGS);
        if flags & social::BIT_LONELY != 0 {
            self.add_pending(animal_offsets::HAPPINESS_PENDING, field(social::TYPE_LONELY_HAPPINESS));
        } else if flags & social::BIT_CROWDED_SPECIES != 0 {
            self.add_pending(animal_offsets::HAPPINESS_PENDING, field(social::TYPE_CROWDED_SPECIES_HAPPINESS));
        }
        if self.read::<u32>(animal_offsets::STATUS_FLAGS) & social::BIT_CROWDED_TILES != 0 {
            self.add_pending(animal_offsets::HAPPINESS_PENDING, field(social::TYPE_CROWDED_TILES_HAPPINESS));
        }
        if self.read::<u32>(0x29c) & social::BIT_CROWDED_GUESTS_HIGH == 0 {
            return;
        }
        self.add_pending(animal_offsets::HAPPINESS_PENDING, field(social::TYPE_CROWDED_GUESTS_HAPPINESS));
        let a: i32 = self.read(social::DIRECTION_A);
        let b: i32 = self.read(social::DIRECTION_B);
        let code = match (a.signum(), b.signum()) {
            (0, 1) => 4,
            (0, -1) => 0,
            (1, 0) => 6,
            (-1, 0) => 2,
            (1, 1) => 5,
            (1, -1) => 7,
            (-1, 1) => 3,
            (-1, -1) => 1,
            _ => return,
        };
        save_to_memory(self.0 + social::DIRECTION_CODE, code as u32);
    }
}

/// Which tile count a habitat check consults.
#[derive(Clone, Copy)]
enum Terrain {
    Underwater,
    Water,
    Land,
}

impl ZTAnimal {
    /// One percent roll against a type chance: draws `rand() % 100` (always, once reached) and passes when the chance is
    /// above it.
    fn percent_roll(chance: i32) -> bool {
        chance > (next_game_random() % 100) as i32
    }

    /// The tail shared by every water-check branch: the habitat (vtable `+0x24c`) must exist and hold more tiles of
    /// `terrain` than the type's threshold, then `setTerrainMode(mode, 1)` runs. Returns whether it ran.
    fn switch_terrain(self, ty: u32, terrain: Terrain) -> bool {
        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
        if habitat == 0 {
            return false;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat) };
        let (count, threshold, mode) = match terrain {
            Terrain::Underwater => (habitat.get_num_underwater_tiles(), water::TYPE_UNDERWATER_TILES, water::MODE_UNDERWATER),
            Terrain::Water => (habitat.get_num_water_tiles(), water::TYPE_WATER_TILES, water::MODE_WATER),
            Terrain::Land => (habitat.get_num_land_tiles(), water::TYPE_LAND_TILES, water::MODE_LAND),
        };
        if count <= Self::type_field::<i32>(ty, threshold) {
            return false;
        }
        self.call_set_terrain_mode(mode, 1);
        true
    }

    /// Not sick, `wantsWater` (vtable `+0xbc`), not escaped and the water roll passes, then a switch to water.
    fn water_branch(self, ty: u32) {
        if self.flag(animal_offsets::SICK)
            || !low_byte_bool(call_slot_0(self.0, water::SLOT_WANTS_WATER))
            || self.flag(lifecycle::ESCAPED)
            || !Self::percent_roll(Self::type_field(ty, water::TYPE_WATER_CHANCE))
        {
            return;
        }
        self.switch_terrain(ty, Terrain::Water);
    }

    /// `ZTAnimal::doWaterCheck` (`0x00438f6c`): nothing for an egg or while doing a show; otherwise its countdown
    /// (`+0x320`) ticks and, on firing, reloads from `DAT_00639418 - 1` and picks a terrain switch by the activity mode
    /// (`+0x380`): mode `0` (land) may switch to land (vtable `+0x16c`, land chance `+0x3a0`) then water (`+0xbc`,
    /// `+0x39c`); mode `1` (water), unless `+0xc0` blocks it, may switch to underwater (chance `+0x3a4`, skipped for the
    /// roll when escaped or sick) then land; mode `2` (underwater), unless `+0x170` blocks it, underwater then water. Each
    /// switch needs the habitat to hold more tiles of that terrain than the type's threshold and runs
    /// `setTerrainMode(mode, 1)`. A sick animal stops where vanilla tests `+0x3a7`. The shared RNG is drawn once per roll.
    pub fn do_water_check(self) {
        let Some(ty) = self.animal_type() else { return };
        if self.flag(animal_offsets::EGG) || low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let countdown: i32 = self.read(water::COUNTDOWN);
        save_to_memory(self.0 + water::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + water::PERIOD);
        save_to_memory(self.0 + water::COUNTDOWN, period.wrapping_sub(1));
        let mode: i32 = self.read(animal_offsets::ACTIVITY_MODE);
        let escaped_or_sick = |this: Self| this.flag(lifecycle::ESCAPED) || this.flag(animal_offsets::SICK);
        let land_tail = |this: Self| {
            if !this.flag(animal_offsets::SICK)
                && low_byte_bool(call_slot_0(this.0, water::SLOT_WANTS_LAND))
                && !this.flag(lifecycle::ESCAPED)
                && Self::percent_roll(Self::type_field(ty, water::TYPE_LAND_CHANCE))
            {
                this.switch_terrain(ty, Terrain::Land);
            }
        };
        match mode {
            0 => {
                if self.flag(animal_offsets::SICK) {
                    return;
                }
                if low_byte_bool(call_slot_0(self.0, water::SLOT_WANTS_LAND))
                    && !self.flag(lifecycle::ESCAPED)
                    && Self::percent_roll(Self::type_field(ty, water::TYPE_LAND_CHANCE))
                    && self.switch_terrain(ty, Terrain::Land)
                {
                    return;
                }
                self.water_branch(ty);
            }
            1 => {
                if low_byte_bool(call_slot_0(self.0, water::SLOT_MODE1_BLOCK)) {
                    return;
                }
                if (escaped_or_sick(self) || Self::percent_roll(Self::type_field(ty, water::TYPE_UNDERWATER_CHANCE)))
                    && self.switch_terrain(ty, Terrain::Underwater)
                {
                    return;
                }
                land_tail(self);
            }
            2 => {
                if low_byte_bool(call_slot_0(self.0, water::SLOT_MODE2_BLOCK)) {
                    return;
                }
                if (escaped_or_sick(self) || Self::percent_roll(Self::type_field(ty, water::TYPE_UNDERWATER_CHANCE)))
                    && self.switch_terrain(ty, Terrain::Underwater)
                {
                    return;
                }
                self.water_branch(ty);
            }
            _ => {}
        }
    }
}

impl ZTAnimal {
    /// `ZTAnimal::update` (`0x00436f11`): `BFEntity::update(dt)`, then, while the non-empty byte (`+0x392`) is set, every
    /// `(id, age)` entry of the vector at `+0x260` ages by `dt` and is erased (later entries shift down, the end pointer
    /// moves back, memory is not freed) once its age reaches `0x4e2`; the byte becomes whether any entry remains.
    pub fn update(self, dt: u32) {
        unsafe { BFENTITY_UPDATE.original()(self.0 as *const u32, dt) };
        if !self.flag(tick::ENTRIES_NONEMPTY) {
            return;
        }
        let mut entry: u32 = self.read(tick::ENTRIES_BEGIN);
        while entry != self.read::<u32>(tick::ENTRIES_END) {
            let age = get_from_memory::<i32>(entry + 4).wrapping_add(dt as i32);
            save_to_memory(entry + 4, age);
            if age >= tick::ENTRY_MAX_AGE {
                let end: u32 = self.read(tick::ENTRIES_END);
                let following = entry + tick::ENTRY_SIZE;
                if following != end {
                    let count = (end.wrapping_sub(following) as i32) >> 3;
                    for index in 0..count.max(0) as u32 {
                        let from = entry + (index + 1) * tick::ENTRY_SIZE;
                        let to = entry + index * tick::ENTRY_SIZE;
                        save_to_memory(to, get_from_memory::<u32>(from));
                        save_to_memory(to + 4, get_from_memory::<u32>(from + 4));
                    }
                }
                save_to_memory(self.0 + tick::ENTRIES_END, end.wrapping_sub(tick::ENTRY_SIZE));
            } else {
                entry += tick::ENTRY_SIZE;
            }
        }
        let remaining = (self.read::<u32>(tick::ENTRIES_END).wrapping_sub(self.read::<u32>(tick::ENTRIES_BEGIN)) as i32) >> 3;
        save_to_memory(self.0 + tick::ENTRIES_NONEMPTY, (remaining != 0) as u8);
    }

    /// `ZTAnimal::updatePosition` (`0x00436f32`): `BFUnit::updatePosition(arg)`, then a man-eating animal
    /// (`preysOnMan`) marks its tile (`+0x85 |= 8`).
    pub fn update_position(self, arg: u32) {
        unsafe { BFUNIT_UPDATE_POSITION.original()(self.0 as *const u32, arg) };
        if self.preys_on_man() {
            let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
            let flags: u8 = get_from_memory(tile + tick::TILE_PREDATOR_BYTE);
            save_to_memory(tile + tick::TILE_PREDATOR_BYTE, flags | tick::TILE_PREDATOR_BIT);
        }
    }
}

impl ZTAnimal {
    fn set_status_words(self, low: u32, high: u32) {
        save_to_memory(self.0 + animal_offsets::STATUS_FLAGS, low);
        save_to_memory(self.0 + animal_offsets::STATUS_FLAGS_CRITICAL, low);
        save_to_memory(self.0 + 0x29c, high);
        save_to_memory(self.0 + 0x2a4, high);
    }

    /// The habitat record in the habitat manager's cell table at the animal's tile: `table[x * 12][y * 40]`, null for
    /// a missing tile or column (vanilla dereferences the null; the port treats it as no flag).
    fn tile_habitat_cell(self) -> u32 {
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        if tile == 0 {
            return 0;
        }
        let manager = globals().zthabitatmgr_ptr() as u32;
        let table: u32 = get_from_memory(manager + conditionals::HABITAT_MGR_TABLE);
        let x: i32 = get_from_memory(tile + conditionals::TILE_X);
        let y: i32 = get_from_memory(tile + conditionals::TILE_Y);
        let column: u32 = get_from_memory(table.wrapping_add((x as u32).wrapping_mul(conditionals::COLUMN_STRIDE)));
        get_from_memory(column.wrapping_add((y as u32).wrapping_mul(conditionals::CELL_STRIDE)))
    }

    /// `ZTAnimal::doConditionals` (`0x00437620`): the animal's per-tick routine. `calcHabitat`, then `doBoxedCheck` and
    /// `doEggCheck`. A boxed animal sets every status word to the marker (`0x40000000` in the high dwords) and stops.
    /// An egg runs `doHealthCheck`, clears the status words, and sets the marker only when its habitat's `+0x2c` byte is
    /// set. Otherwise: the AI-manager gate (`+0x138`) counts down `+0x340` and calls vtable `+0x10c` when it reaches `0`;
    /// then `doBabyToAdultCheck`, `doReproduceCheck`, the not-selectable path (removed unless vtable `+0xac` answers
    /// true or the current behaviour is `bDie`), the keeper-visit and death-arrived happiness changes (clearing those
    /// bytes), the `+0x3a9` stop-eating check (`stopEating(0)` and a date stamp at `+0x288` while hunger is not above
    /// the type's `+0x244`), `doEscapedCheck`/`fEat`, and the need checks (hunger, breath, murky water, health, sleep).
    /// The pressure countdown (`+0x310`) drains happiness by `+0x2e0 * type[+0x274]` and may start a dirt patch; the
    /// social, keeper, habitat, bored, water, building-use, other and zap checks follow; the message countdown (`+0x33c`)
    /// posts "Not happy" (`0x272c`) at low happiness; finally vtable `+0x48(0)` runs and the habitat flag at the
    /// animal's tile decides between the marker and clearing the high bit of the high status dwords. Callees go through
    /// `.hooked()`, so ported and vanilla checks both run as installed.
    pub fn do_conditionals(self) {
        let this = self.0 as *const u32;
        unsafe { CALC_HABITAT.hooked()(this) };
        let Some(ty) = self.animal_type() else { return };
        let aimgr = globals().ztaimgr_ptr() as u32;
        unsafe {
            DO_BOXED_CHECK.hooked()(this);
            DO_EGG_CHECK.hooked()(this);
        }
        if self.flag(animal_offsets::BOXED) {
            self.set_status_words(0, conditionals::MARKER_HIGH);
            return;
        }
        if self.flag(animal_offsets::EGG) {
            unsafe { DO_HEALTH_CHECK.hooked()(this) };
            self.set_status_words(0, 0);
            let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
            if habitat != 0 && get_from_memory::<u8>(habitat + conditionals::HABITAT_FLAG) != 0 {
                self.set_status_words(0, conditionals::MARKER_HIGH);
            }
            return;
        }
        if aimgr != 0 && get_from_memory::<u32>(aimgr + conditionals::AIMGR_GATE) != 0 {
            let remaining = self.read::<i32>(conditionals::COUNTDOWN_AIMGR).wrapping_sub(1);
            save_to_memory(self.0 + conditionals::COUNTDOWN_AIMGR, remaining);
            if remaining <= 0 {
                call_slot_0(self.0, conditionals::SLOT_AIMGR_NOTIFY);
            }
        }
        unsafe {
            DO_BABY_TO_ADULT_CHECK.hooked()(this);
            DO_REPRODUCE_CHECK.hooked()(this);
        }
        if self.flag(animal_offsets::NOT_SELECTABLE) {
            if !low_byte_bool(call_slot_0(self.0, conditionals::SLOT_UNKNOWN_0XAC)) {
                let begin: u32 = self.read(unit_fields::BEHAVIOUR_BEGIN);
                let end: u32 = self.read(unit_fields::BEHAVIOUR_END);
                let is_die = end.wrapping_sub(begin) == 4 && unsafe { std::slice::from_raw_parts(begin as *const u8, 4) } == b"bDie";
                if !is_die {
                    unsafe { ZTUNIT_SET_IS_REMOVED.original()(this, 1) };
                }
            }
            return;
        }
        if self.flag(conditionals::BYTE_KEEPER_VISIT) {
            let alternate = low_byte_bool(call_slot_0(ty, conditionals::TYPE_SLOT_ALT));
            let field = if alternate { conditionals::TYPE_KEEPER_HAPPINESS_ALT } else { conditionals::TYPE_KEEPER_HAPPINESS };
            self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, field));
            save_to_memory(self.0 + conditionals::BYTE_KEEPER_VISIT, 0u8);
        }
        if self.flag(conditionals::BYTE_DEATH_ARRIVED) {
            self.add_pending(animal_offsets::HAPPINESS_PENDING, get_from_memory::<i32>(base() + conditionals::DEATH_UNHAPPINESS));
            save_to_memory(self.0 + conditionals::BYTE_DEATH_ARRIVED, 0u8);
        }
        if self.flag(hunger::BYTE_FOOD_WANTED) && self.hunger() <= Self::type_field::<i32>(ty, conditionals::TYPE_HUNGER_LIMIT) {
            ZTAnimal(this as u32).stop_eating(0);
            let mut date = [0u32; 2];
            let stamp = unsafe { GAMEMGR_GET_DATE.hooked()(globals().ztgamemgr_ptr() as *const u32, date.as_mut_ptr() as *const _) } as *const u32;
            if !stamp.is_null() {
                save_to_memory(self.0 + conditionals::DATE_STAMP, get_from_memory::<u32>(stamp as u32));
                save_to_memory(self.0 + conditionals::DATE_STAMP + 4, get_from_memory::<u32>(stamp as u32 + 4));
            }
        }
        unsafe { DO_ESCAPED_CHECK.hooked()(this) };
        if self.flag(hunger::BYTE_FOOD_WANTED) {
            unsafe { F_EAT.hooked()(this) };
        }
        unsafe {
            DO_HUNGER_CHECK.hooked()(this);
            DO_BREATH_CHECK.hooked()(this);
            DO_MURKY_WATER_CHECK.hooked()(this);
            DO_HEALTH_CHECK.hooked()(this);
            DO_SLEEP_CHECK.hooked()(this);
        }
        let countdown: i32 = self.read(conditionals::COUNTDOWN_PRESSURE);
        save_to_memory(self.0 + conditionals::COUNTDOWN_PRESSURE, countdown.wrapping_sub(1));
        if countdown == 0 {
            self.pressure_tick(ty);
        }
        unsafe {
            DO_SOCIAL_CHECK.hooked()(this);
            DO_KEEPER_CHECK.hooked()(this);
            DO_HABITAT_CHECK.hooked()(this);
            DO_BORED_CHECK.hooked()(this);
            DO_WATER_CHECK.hooked()(this);
            DO_BUILDING_USE_CHECK.hooked()(this);
            DO_OTHER_CHECK.hooked()(this);
            DO_ZAP_CHECK.hooked()(this);
        }
        let countdown: i32 = self.read(conditionals::COUNTDOWN_MESSAGE);
        save_to_memory(self.0 + conditionals::COUNTDOWN_MESSAGE, countdown.wrapping_sub(1));
        if countdown == 0 {
            let period: i32 = get_from_memory(base() + conditionals::MESSAGE_PERIOD);
            save_to_memory(self.0 + conditionals::COUNTDOWN_MESSAGE, period.wrapping_sub(1));
            if self.happiness() <= conditionals::MESSAGE_HAPPINESS {
                unsafe { ZTUNIT_F_ZOO_MESSAGE_NAME.original()(this, conditionals::MESSAGE_ID, 2, 0) };
            }
        }
        call_slot_1(self.0, conditionals::SLOT_TICK_NOTIFY, 0);
        if self.tile_habitat_cell_flag() {
            self.set_status_words(0, conditionals::MARKER_HIGH);
        } else {
            self.update_flag_words(|v| v & !conditionals::MARKER_HIGH, &[0x29c, 0x2a4]);
        }
    }

    fn tile_habitat_cell_flag(self) -> bool {
        let cell = self.tile_habitat_cell();
        cell != 0 && get_from_memory::<u8>(cell + conditionals::HABITAT_FLAG) != 0
    }

    /// The pressure countdown's firing branch: reload from `DAT_006393f8 - 1`, drain happiness by `+0x2e0 * type[+0x274]`;
    /// for a type with a positive dirt cap (`+0x38c`) and an animal at or below the angry threshold
    /// (`happiness <= 2 * DAT_006393f4 - 0xc9`), two rolls (`< DAT_006394ac`, then `< cap`) and `ZTAnimal::isOnGrass` start a
    /// dirt patch (`changeToDirt`) when the four tile words at `+4..+0x14` are all zero.
    fn pressure_tick(self, ty: u32) {
        let this = self.0 as *const u32;
        let period: i32 = get_from_memory(base() + conditionals::PRESSURE_PERIOD);
        save_to_memory(self.0 + conditionals::COUNTDOWN_PRESSURE, period.wrapping_sub(1));
        let drain = self.read::<i32>(conditionals::DRAIN_FACTOR).wrapping_mul(Self::type_field::<i32>(ty, conditionals::TYPE_DRAIN));
        let pending: i32 = self.read(animal_offsets::HAPPINESS_PENDING);
        save_to_memory(self.0 + animal_offsets::HAPPINESS_PENDING, pending.wrapping_sub(drain));
        let cap = Self::type_field::<i32>(ty, conditionals::TYPE_DIRT_CAP);
        if cap <= 0 {
            return;
        }
        let angry_base: i32 = get_from_memory(base() + conditionals::ANGRY_BASE);
        if self.happiness() > angry_base.wrapping_mul(2).wrapping_sub(0xc9) {
            return;
        }
        let chance: i32 = get_from_memory(base() + conditionals::DIRT_CHANCE);
        if (next_game_random() % 100) as i32 >= chance {
            return;
        }
        if ((next_game_random() % 100) as i32) >= cap {
            return;
        }
        let predicate = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(conditionals::DIRT_PREDICATE) };
        if !low_byte_bool(predicate(self.0)) {
            return;
        }
        let mut clear = true;
        for &offset in &conditionals::TILE_DIRT_WORDS {
            let tile = unsafe { GET_TILE.original()(this) } as u32;
            clear &= get_from_memory::<u32>(tile + offset) == 0;
        }
        if clear {
            self.change_to_dirt();
        }
    }
}

impl ZTAnimal {
    /// `ZTAnimal::doBuildingUseCheck` (`0x00438f20`): when its countdown (`+0x344`) was `0` it reloads from
    /// `DAT_00639408 - 1`; a type chance (`+0x37c`, percent) above `rand() % 100` then lets the animal look for a building
    /// unless `isNeedOrFunSatisfier` already accepts its current destination. `findNearestSatisfier` (radius `0xb504`)
    /// fills in a building; when one is found its need index (`getNeedIndex`) is stored in `+0x24e` unless it is `0xff`,
    /// vtable `+0x2e4` receives the building's tile, and the destination (`+0x214`) becomes the building's position
    /// reduced to the tile: `x % 64`, `y % 64` (C remainders) and `z % 16` shifted up by `16` when `z` is negative.
    /// The "Looking for Building"/"Found a building" log strings are dropped.
    pub fn do_building_use_check(self) {
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(building_use::COUNTDOWN);
        save_to_memory(self.0 + building_use::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + building_use::PERIOD);
        save_to_memory(self.0 + building_use::COUNTDOWN, period.wrapping_sub(1));
        let chance = Self::type_field::<i32>(ty, building_use::TYPE_CHANCE);
        if chance <= 0 || chance <= (next_game_random() % 100) as i32 {
            return;
        }
        let need_name = (base() + building_use::NEED_NAME) as *const i8;
        unsafe { GET_NEED_INDEX.hooked()(need_name) };
        let position: [i32; 3] = [
            self.read(building_use::UNIT_POSITION),
            self.read(building_use::UNIT_POSITION + 4),
            self.read(building_use::UNIT_POSITION + 8),
        ];
        let this = self.0 as *const u32;
        if unsafe { IS_NEED_OR_FUN_SATISFIER.original()(this, self.read::<i32>(building_use::UNIT_NEED_WORD), position.as_ptr() as *const u32) } {
            return;
        }
        let mut building: u32 = 0;
        let own_tile = unsafe { GET_TILE.original()(this) };
        let found = unsafe {
            FIND_NEAREST_SATISFIER.original()(
                this,
                own_tile,
                need_name as *const i32,
                &mut building as *mut u32 as *const u32,
                building_use::SEARCH_RADIUS,
                building_use::SEARCH_FLAG,
            )
        };
        if !found || building == 0 {
            return;
        }
        let index = unsafe { GET_NEED_INDEX.hooked()(need_name) } as u8;
        if index != building_use::NEED_NONE {
            save_to_memory(self.0 + building_use::UNIT_NEED_BYTE, index);
        }
        let building_tile = unsafe { GET_TILE.original()(building as *const u32) } as u32;
        call_slot_1(self.0, building_use::SLOT_SET_TARGET, building_tile);
        let x: i32 = get_from_memory(building + building_use::BUILDING_X);
        let y: i32 = get_from_memory(building + building_use::BUILDING_Y);
        let z: i32 = get_from_memory(building + building_use::BUILDING_Z);
        let z_in_tile = if z < 0 { z.wrapping_rem(16).wrapping_add(16) } else { z.wrapping_rem(16) };
        save_to_memory(self.0 + building_use::UNIT_POSITION, x.wrapping_rem(64));
        save_to_memory(self.0 + building_use::UNIT_POSITION + 4, y.wrapping_rem(64));
        save_to_memory(self.0 + building_use::UNIT_POSITION + 8, z_in_tile);
    }
}

impl ZTAnimal {
    /// The unit's current behaviour name (`+0x54..+0x58`) is one of the two rampage scripts.
    fn behaviour_is_rampage(self) -> bool {
        let begin: u32 = self.read(unit_fields::BEHAVIOUR_BEGIN);
        let end: u32 = self.read(unit_fields::BEHAVIOUR_END);
        let name = unsafe { std::slice::from_raw_parts(begin as *const u8, end.wrapping_sub(begin) as usize) };
        escaped::RAMPAGE_BEHAVIOURS.contains(&name)
    }

    /// `ZTAnimal::doEscapedCheck` (`0x00438514`): nothing for an egg or while doing a show. A rampage (`+0x3b0`) ages
    /// (`+0x35c += 1`) and stops (flags cleared; when the behaviour is `bEscapedRampage`/`bHabitatRampage` the queue and
    /// script are cleared, the unit stops and vtable `+0x174` clears the goal; the counter resets) once happiness is above
    /// the type's `+0x3f4` or the counter passes `+0x400`. Otherwise a type with a rampage chance (`+0x3f8 > 0`, minimum
    /// `+0x3fc > 0`), happiness at or below `+0x3f4` and a roll below the chance starts one: `+0x3b0 = 1`,
    /// `+0x35c = (max - min) - rand() % (max - min + 1)`; anything else resets the counter and flags. The countdown
    /// (`+0x304`) then ticks; on firing it reloads from `DAT_006393e0 - 1` and an animal outside its habitat (no habitat,
    /// or `+0x2c` clear) is un-escaped (`setEscaped(0)`, `+0x374 = 0`) while one inside raises the alarm (`setEscaped(1)`,
    /// `setTerrainMode(0, 1)`, the alert on the first escape and `ZooStatus::animalEscaped`). Predator, playmate and
    /// avoidance handling follows: a predator (`+0x2fc(0)`) sets the fleeing byte (`+0x24c`) once, a nearer one
    /// (`+0x2fc(1)`) when idle; otherwise `+0x22c` is cleared, fleeing is dropped, a goal `0x75` is cleared and an
    /// unoccupied animal that finds a playmate (`+0x2f8(0)`) starts chasing (`+0x3a4`). Log strings are dropped.
    pub fn do_escaped_check(self) {
        let Some(ty) = self.animal_type() else { return };
        if self.flag(animal_offsets::EGG) || low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let goal: i32 = self.read(hunger::GOAL);
        let field = |offset: u32| Self::type_field::<i32>(ty, offset);
        let set = |offset: u32, value: u8| save_to_memory(self.0 + offset, value);
        let reset_rampage = || save_to_memory(self.0 + escaped::RAMPAGE_COUNTER, 0u32);
        if self.flag(escaped::BYTE_RAMPAGING) {
            let counter = self.read::<i32>(escaped::RAMPAGE_COUNTER).wrapping_add(1);
            save_to_memory(self.0 + escaped::RAMPAGE_COUNTER, counter);
            if self.happiness() > field(escaped::TYPE_RAMPAGE_HAPPINESS) || counter > field(escaped::TYPE_RAMPAGE_MAX) {
                set(escaped::BYTE_RAMPAGING, 0);
                set(escaped::BYTE_RAMPAGE_AUX, 0);
                if self.behaviour_is_rampage() {
                    unsafe {
                        BFENTITY_CLEAR_QUEUE.hooked()(self.0 as *const u32);
                        BFENTITY_CLEAR_BS.hooked()(self.0 as *const u32);
                        BFUNIT_STOP.hooked()(self.0 as *const u32, 1, 0);
                    }
                    call_slot_0(self.0, escaped::SLOT_CLEAR_GOAL);
                }
                reset_rampage();
            }
        } else if field(escaped::TYPE_RAMPAGE_CHANCE) > 0
            && field(escaped::TYPE_RAMPAGE_MIN) > 0
            && self.happiness() <= field(escaped::TYPE_RAMPAGE_HAPPINESS)
            && ((next_game_random() % 100) as i32) < field(escaped::TYPE_RAMPAGE_CHANCE)
        {
            let span = field(escaped::TYPE_RAMPAGE_MAX).wrapping_sub(field(escaped::TYPE_RAMPAGE_MIN));
            if span < 0 {
                set(escaped::BYTE_RAMPAGING, 0);
                set(escaped::BYTE_RAMPAGE_AUX, 0);
                reset_rampage();
            } else {
                set(escaped::BYTE_RAMPAGING, 1);
                let draw = (next_game_random() as i32).wrapping_rem(span.wrapping_add(1));
                save_to_memory(self.0 + escaped::RAMPAGE_COUNTER, span.wrapping_sub(draw));
            }
        } else {
            reset_rampage();
        }
        let countdown: i32 = self.read(escaped::COUNTDOWN);
        save_to_memory(self.0 + escaped::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + escaped::PERIOD);
        save_to_memory(self.0 + escaped::COUNTDOWN, period.wrapping_sub(1));
        let habitat = call_slot_0(self.0, pipeline::SLOT_HABITAT);
        let inside = habitat != 0 && get_from_memory::<u8>(habitat + pipeline::HABITAT_NO_HATCH) != 0;
        if inside {
            let first_escape = !self.flag(lifecycle::ESCAPED);
            self.set_escaped(1);
            self.call_set_terrain_mode(0, 1);
            if first_escape {
                unsafe { SHOW_ESCAPED_ANIMAL_ALERT.hooked()(self.0 as *const u32) };
            }
            let status = globals().ztgamemgr_ptr() as u32 + escaped::ZOO_STATUS_OFFSET;
            unsafe { ZOOSTATUS_ANIMAL_ESCAPED.hooked()(status as i32) };
        } else {
            self.set_escaped(0);
            save_to_memory(self.0 + escaped::ESCAPE_TIMER, 0u32);
        }
        if call_slot_1(self.0, escaped::SLOT_FIND_PREDATOR, 0) != 0 {
            if self.flag(escaped::BYTE_FLEEING) {
                return;
            }
            call_slot_0(self.0, escaped::SLOT_NOTIFY_PREDATOR);
            set(escaped::BYTE_FLEEING, 1);
            return;
        }
        if call_slot_1(self.0, escaped::SLOT_FIND_PREDATOR, 1) != 0 {
            if self.flag(escaped::BYTE_FLEEING)
                || self.flag(escaped::BYTE_PLAYMATE)
                || self.flag(lifecycle::ESCAPED)
                || low_byte_bool(call_slot_0(self.0, escaped::SLOT_HAS_PRESSING_NEED))
                || self.flag(escaped::BYTE_RAMPAGING)
            {
                return;
            }
            set(escaped::BYTE_FLEEING, 1);
            return;
        }
        save_to_memory(self.0 + escaped::UNIT_WORD_22C, 0u32);
        if self.flag(escaped::BYTE_FLEEING) {
            set(escaped::BYTE_FLEEING, 0);
        }
        if goal == escaped::GOAL_PLAYMATE {
            call_slot_0(self.0, escaped::SLOT_CLEAR_GOAL);
        }
        if self.flag(escaped::BYTE_CHASE_AUX)
            || self.flag(escaped::BYTE_PLAYMATE)
            || self.flag(escaped::BYTE_CAPTURE_STAGE)
            || goal == escaped::GOAL_CHASE_PLAYMATE
            || self.flag(escaped::BYTE_FLEEING)
            || goal == escaped::GOAL_PLAYMATE
        {
            return;
        }
        if call_slot_1(self.0, escaped::SLOT_FIND_PLAYMATE, 0) == 0 {
            return;
        }
        call_slot_0(self.0, escaped::SLOT_NOTIFY_PREDATOR);
        set(escaped::BYTE_PLAYMATE, 1);
    }
}

/// Fields the feeding and trick helpers touch.
mod feeding {
    /// Type field: the entity name's first byte picks which happiness change `addChanges` applies.
    pub const TYPE_NAME_PTR: u32 = 0xa4;
    /// Type fields `dropTestFenceChance` subtracts and clamps against.
    pub const TYPE_FENCE_STEP: u32 = 0x3e4;
    pub const TYPE_FENCE_FLOOR: u32 = 0x3e8;
    /// Counter `dropTestFenceChance` lowers.
    pub const FENCE_CHANCE: u32 = 0x3ac;
    /// Byte `setKeeperArrives` sets once a keeper can service the animal.
    pub const KEEPER_ARRIVES: u32 = 0x39c;
    /// Pending hunger change and the fifth need's pending change (`addChanges`).
    pub const HUNGER_PENDING: u32 = 0x2b4;
    pub const NEED5_PENDING: u32 = 0x2c4;
    /// Food entity pointer inside the eat context.
    pub const CONTEXT_FOOD: u32 = 0x10;
    /// Remaining food units of a food entity.
    pub const FOOD_UNITS: u32 = 0x154;
    /// Habitat byte `fRemoveAnimal` tests, and the animal count it compares with `1`.
    pub const HABITAT_DIRTY_BYTE: u32 = 0x2d;
    pub const HABITAT_ANIMAL_COUNT: u32 = 0xf0;
    /// Words `doFoodTrick` writes.
    pub const TRICK_WORD_A: u32 = 0x252;
    pub const TRICK_WORD_B: u32 = 0x254;
    /// Slot `doFoodTrick` calls with `(0x7d, 0)`.
    pub const SLOT_SET_GOAL: u32 = 0x1d8;
    pub const GOAL_FOOD_TRICK: u32 = 0x7d;
    /// Slots `checkDoTrick` reads the happiness limit and the success percent from.
    pub const SLOT_TRICK_HAPPINESS_LIMIT: u32 = 0x238;
    pub const SLOT_TRICK_PERCENT: u32 = 0x23c;
}

impl ZTAnimal {
    /// Whether the entity type at `entity_type` passes `isCastClass(tag)`.
    fn type_is_cast(entity_type: u32, tag_rva: u32) -> bool {
        let function = unsafe {
            transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slots::TYPE_IS_CAST_CLASS))
        };
        function(entity_type, base() + tag_rva)
    }

    /// The food entity the eat context points at, or `0` when there is no context, entity or the entity is not a food
    /// entity (`isCastClass(DAT_006386c0)`).
    fn food_entity(self) -> u32 {
        let context: u32 = self.read(animal_offsets::EAT_CONTEXT);
        if context == 0 {
            return 0;
        }
        let food: u32 = get_from_memory(context + feeding::CONTEXT_FOOD);
        if food == 0 {
            return 0;
        }
        let entity_type: u32 = get_from_memory(food + unit_offsets::ENTITY_TYPE);
        if entity_type == 0 || !Self::type_is_cast(entity_type, tags::FOOD_ENTITY) {
            return 0;
        }
        food
    }

    /// `ZTAnimal::setFood` (`0x0049fb37`): the eat context becomes the food entity's tile (unchanged for a null food).
    pub fn set_food(self, food: u32) {
        if food != 0 {
            let tile = unsafe { GET_TILE.original()(food as *const u32) };
            save_to_memory(self.0 + animal_offsets::EAT_CONTEXT, tile);
        }
    }

    /// `ZTAnimal::setKeeperArrives` (`0x0049f55d`): flags the arrival when the keeper can service this animal.
    pub fn set_keeper_arrives(self, staff: u32) {
        if self.can_service(staff) {
            save_to_memory(self.0 + feeding::KEEPER_ARRIVES, 1u8);
        }
    }

    /// `ZTAnimal::faceTowardFood` (`0x0042661b`): `BFUnit::faceTowardTile` over the eat context, when there is one.
    pub fn face_toward_food(self) {
        let context: i32 = self.read(animal_offsets::EAT_CONTEXT);
        if context != 0 {
            unsafe { BFUNIT_FACE_TOWARD_TILE.original()(self.0 as *const u32, context) };
        }
    }

    /// `ZTAnimal::removeFoodTargetForAll` (`0x0059a955`): the animal's habitat drops every food target on `food`; `0` when
    /// the animal has no habitat.
    pub fn remove_food_target_for_all(self, food: u32) -> u32 {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat == 0 {
            return 0;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.remove_food_target_for_all(food) as u32
    }

    /// `ZTAnimal::fReduceFood` (`0x004fdbc9`): takes `units` off the food entity, marks the habitat dirty and, once the
    /// food is gone, removes it as a target for every animal. Returns the units left (`0` without habitat or food).
    pub fn f_reduce_food(self, units: i32) -> i32 {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        let food = self.food_entity();
        if habitat == 0 || food == 0 {
            return 0;
        }
        unsafe { SET_AMOUNT_FOOD_UNITS_DELTA.original()(food as *const u32, units.wrapping_neg()) };
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.set_dirty_characteristics();
        let left: u32 = get_from_memory(food + feeding::FOOD_UNITS);
        if left == 0 {
            let again = self.food_entity();
            self.remove_food_target_for_all(again);
        }
        left as i32
    }

    /// `ZTAnimal::fRemoveAnimal` (`0x004808ac`): recalculates the habitat's characteristics when they are flagged, and
    /// removes the animal from the world once the habitat holds no animals.
    pub fn f_remove_animal(self) {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat == 0 {
            return;
        }
        if get_from_memory::<u8>(habitat + feeding::HABITAT_DIRTY_BYTE) != 0 {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }.recalculate_characteristics();
        }
        if get_from_memory::<i32>(habitat + feeding::HABITAT_ANIMAL_COUNT) < 1 {
            unsafe { ZTUNIT_SET_IS_REMOVED.original()(self.0 as *const u32, 1) };
        }
    }

    /// `ZTAnimal::dropTestFenceChance` (`0x0059c4ba`): lowers the fence-test counter by the type's step, floored to `0`
    /// when it falls under the type's minimum.
    pub fn drop_test_fence_chance(self) {
        let Some(ty) = self.animal_type() else {
            return;
        };
        let lowered = self.read::<i32>(feeding::FENCE_CHANCE).wrapping_sub(Self::type_field::<i32>(ty, feeding::TYPE_FENCE_STEP));
        save_to_memory(self.0 + feeding::FENCE_CHANCE, lowered);
        if lowered < Self::type_field::<i32>(ty, feeding::TYPE_FENCE_FLOOR) {
            save_to_memory(self.0 + feeding::FENCE_CHANCE, 0i32);
        }
    }

    /// `ZTAnimal::addChanges` (`0x0042648c`): folds a changes record into the pending needs. The first word applies to
    /// happiness for adults (`m`/`f` types), the second for young (`y`); other types skip happiness. Always `true`.
    pub fn add_changes(self, changes: u32) -> bool {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let name: u32 = Self::type_field(entity_type, feeding::TYPE_NAME_PTR);
        let word = |index: u32| get_from_memory::<i32>(changes + index * 4);
        let delta = match get_from_memory::<u8>(name) {
            b'm' | b'f' => Some(word(0)),
            b'y' => Some(word(1)),
            _ => None,
        };
        if let Some(delta) = delta {
            let pending: i32 = self.read(animal_offsets::HAPPINESS_PENDING);
            save_to_memory(self.0 + animal_offsets::HAPPINESS_PENDING, pending.wrapping_add(delta));
        }
        let hunger: i32 = self.read(feeding::HUNGER_PENDING);
        save_to_memory(self.0 + feeding::HUNGER_PENDING, hunger.wrapping_add(word(2)));
        let need5: i32 = self.read(feeding::NEED5_PENDING);
        save_to_memory(self.0 + feeding::NEED5_PENDING, need5.wrapping_add(word(5)));
        true
    }

    /// `ZTAnimal::doFoodTrick` (`0x005a745e`): sets goal `0x7d` through the vtable and records the two trick words.
    pub fn do_food_trick(self, word_a: u16, word_b: u16) {
        call_slot_2(self.0, feeding::SLOT_SET_GOAL, feeding::GOAL_FOOD_TRICK, 0);
        save_to_memory(self.0 + feeding::TRICK_WORD_A, word_a);
        save_to_memory(self.0 + feeding::TRICK_WORD_B, word_b);
    }

    /// `ZTAnimal::checkDoTrick` (`0x005a70c1`): `true` while the animal is happier than the type's limit; otherwise one
    /// percent roll against the type's trick chance (`true` when the roll is not under it).
    pub fn check_do_trick(self) -> bool {
        let limit = call_slot_0(self.0, feeding::SLOT_TRICK_HAPPINESS_LIMIT) as i32;
        let percent = call_slot_0(self.0, feeding::SLOT_TRICK_PERCENT) as i32;
        if self.happiness() > limit {
            return true;
        }
        (next_game_random() as i32).wrapping_rem(100) >= percent
    }
}

/// Fields the eating, chasing, boxing and dirt helpers touch.
mod handling {
    /// Byte `stopEating` stores its argument in, and the byte that marks the animal as eating.
    pub const STOP_EATING_FLAG: u32 = 0x39e;
    pub const EATING: u32 = 0x3a9;
    /// Begin and end of the behaviour name; `stopEating` empties it.
    pub const NAME_BEGIN: u32 = 0xdc;
    pub const NAME_END: u32 = 0xe0;
    /// Byte `stopEating` raises once the name is cleared.
    pub const NAME_DIRTY: u32 = 0xe8;
    pub const SLOT_CLEAR_GOAL: u32 = 0x174;
    /// Slots `stopChase` calls on the animal.
    pub const SLOT_CLEAR_PREY: u32 = 0x258;
    pub const SLOT_GET_PREY: u32 = 0x25c;
    pub const SLOT_SET_VISIBLE: u32 = 0xcc;
    pub const SLOT_STOP_EVERYTHING: u32 = 0x198;
    pub const PREY_GOAL_WORD: u32 = 0x22c;
    pub const CHASE_BYTES: [u32; 2] = [0x3a5, 0x3a4];
    pub const VISIBLE_BYTE: u32 = 0x20d;
    /// Slots and bytes `box`/`unbox` use.
    pub const SLOT_SET_MAP_ENTRY: u32 = 0x78;
    pub const SLOT_REFRESH_A: u32 = 0xe0;
    pub const SLOT_REFRESH_B: u32 = 0x74;
    pub const SLOT_AFTER_BOX: u32 = 0x254;
    pub const UNBOX_BYTE: u32 = 0x211;
    pub const CAPTURE_STAGE: u32 = 0x393;
    pub const ESCAPE_WORD: u32 = 0x374;
    /// Begin and end of the 8-byte `(id, age)` vector `box` empties.
    pub const VEC_BEGIN: u32 = 0x260;
    pub const VEC_END: u32 = 0x264;
    /// Tile fields `changeToDirt` reads, the terrain info array in the terrain manager and the dirt entry's index.
    pub const TILE_X: u32 = 0x34;
    pub const TILE_Y: u32 = 0x38;
    pub const TERRAIN_INFO_BEGIN: u32 = 0x10;
    pub const TERRAIN_INFO_END: u32 = 0x14;
    pub const TERRAIN_INFO_SIZE: u32 = 0x30;
    pub const DIRT_INFO_INDEX: u32 = 0x10;
}

impl ZTAnimal {
    /// `ZTAnimal::stopEating` (`0x004fd86e`): records `flag`, clears the eating byte, clears the goal through the vtable
    /// and empties the behaviour name. (The "Stopped Eating" log string vanilla builds and frees is dropped.)
    pub fn stop_eating(self, flag: u8) {
        save_to_memory(self.0 + handling::STOP_EATING_FLAG, flag);
        save_to_memory(self.0 + handling::EATING, 0u8);
        call_slot_0(self.0, handling::SLOT_CLEAR_GOAL);
        let begin: u32 = self.read(handling::NAME_BEGIN);
        if begin != self.read::<u32>(handling::NAME_END) {
            save_to_memory(begin, 0u8);
            save_to_memory(self.0 + handling::NAME_END, begin);
        }
        save_to_memory(self.0 + handling::NAME_DIRTY, 1u8);
    }

    /// `ZTAnimal::stopChase` (`0x004a5e00`): when the animal has a prey and either `force` is set or the prey is of the
    /// animal's own type id, the prey's `+0x22c` is zeroed and the animal's prey cleared. Then the chase bytes are zeroed,
    /// `stopEverything(1, 0, 1)` runs and the animal becomes visible again. (The log string is dropped.)
    pub fn stop_chase(self, force: bool) {
        let prey = call_slot_0(self.0, handling::SLOT_GET_PREY);
        if prey != 0 {
            let prey_type: u32 = get_from_memory(prey + unit_offsets::ENTITY_TYPE);
            let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
            let prey_id = call_slot_0(prey_type, slots::TYPE_ID);
            let own_id = call_slot_0(own_type, slots::TYPE_ID);
            if prey_id == own_id || force {
                save_to_memory(prey + handling::PREY_GOAL_WORD, 0u32);
                call_slot_0(self.0, handling::SLOT_CLEAR_PREY);
            }
        }
        for offset in handling::CHASE_BYTES {
            save_to_memory(self.0 + offset, 0u8);
        }
        call_slot_3(self.0, handling::SLOT_STOP_EVERYTHING, 1, 0, 1);
        save_to_memory(self.0 + handling::VISIBLE_BYTE, 0u8);
        call_slot_1(self.0, handling::SLOT_SET_VISIBLE, 1);
    }

    /// The refresh slots, mixer reset and `stopEverything(1, 1, 1)` that `box` and `unbox` share.
    fn refresh_after_boxing_change(self) {
        call_slot_0(self.0, handling::SLOT_REFRESH_A);
        call_slot_0(self.0, handling::SLOT_REFRESH_B);
        unsafe { transmute::<u32, extern "thiscall" fn(u32)>(lifecycle::RESET_BLOCK_8C)(self.0 + lifecycle::BLOCK_8C) };
        call_slot_3(self.0, handling::SLOT_STOP_EVERYTHING, 1, 1, 1);
    }

    /// `ZTAnimal::box` (`0x00505941`): takes the animal off the map, marks it boxed, resets its mixer, stops everything,
    /// clears escape and chase state and empties the `(id, age)` vector. Always `true`.
    pub fn box_animal(self) -> bool {
        call_slot_1(self.0, handling::SLOT_SET_MAP_ENTRY, 0);
        save_to_memory(self.0 + animal_offsets::BOXED, 1u8);
        self.refresh_after_boxing_change();
        self.set_escaped(0);
        save_to_memory(self.0 + handling::CAPTURE_STAGE, 0u8);
        save_to_memory(self.0 + handling::ESCAPE_WORD, 0u32);
        self.stop_chase(true);
        call_slot_0(self.0, handling::SLOT_AFTER_BOX);
        save_to_memory(self.0 + handling::VEC_END, self.read::<u32>(handling::VEC_BEGIN));
        unsafe { ANIMALINFO_FORCE_UPDATE.original()(std::ptr::null()) };
        true
    }

    /// `ZTAnimal::unbox` (`0x00505a6b`): the reverse of [`Self::box_animal`] for the map entry and boxed bytes. Always `true`.
    pub fn unbox(self) -> bool {
        call_slot_1(self.0, handling::SLOT_SET_MAP_ENTRY, 0);
        save_to_memory(self.0 + animal_offsets::BOXED, 0u8);
        save_to_memory(self.0 + handling::UNBOX_BYTE, 0u8);
        self.refresh_after_boxing_change();
        unsafe { ANIMALINFO_FORCE_UPDATE.original()(std::ptr::null()) };
        true
    }

    /// `ZTAnimal::changeToDirt` (`0x00613c0c`): paints the animal's tile with the dirt terrain (entry `0x10` of the
    /// terrain manager's `0x30`-byte info array, none when the array is shorter), restoring the map selection afterwards.
    pub fn change_to_dirt(self) {
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        let world = ZTAnimal::world();
        let terrain_mgr = globals().ztadvterrainmgr_ptr() as u32;
        if tile == 0 || world == 0 || terrain_mgr == 0 {
            return;
        }
        let map = (world + 8) as *const u32;
        let (mut x, mut y, mut flag) = (0i32, 0i32, 0u32);
        unsafe {
            BFMAP_GET_SELECTION.original()(map, &mut x, &mut y, &mut flag);
            BFMAP_SET_SELECTION.original()(map, get_from_memory::<i32>(tile + handling::TILE_X), get_from_memory::<i32>(tile + handling::TILE_Y), 1);
        }
        let begin: u32 = get_from_memory(terrain_mgr + handling::TERRAIN_INFO_BEGIN);
        let end: u32 = get_from_memory(terrain_mgr + handling::TERRAIN_INFO_END);
        let count = (end.wrapping_sub(begin) as i32) / handling::TERRAIN_INFO_SIZE as i32;
        let cell = if count as u32 > handling::DIRT_INFO_INDEX { begin.wrapping_add(handling::DIRT_INFO_INDEX * handling::TERRAIN_INFO_SIZE) } else { 0 };
        unsafe {
            BFMAP_PAINT_CELL.original()(map, cell as i32, true);
            BFMAP_SET_SELECTION.original()(map, x, y, flag);
        }
    }
}

/// Fields `addFace` and `checkPrey` touch.
mod faces_prey {
    /// The `(face byte, age word)` vector: begin, end and capacity pointers.
    pub const VEC_BEGIN: u32 = 0x260;
    pub const VEC_END: u32 = 0x264;
    pub const VEC_CAP: u32 = 0x268;
    /// Byte raised whenever the vector gains an entry.
    pub const FACES_PENDING: u32 = 0x392;
    pub const ENTRY_SIZE: u32 = 8;
    /// `CAST_ZTGuest` and `CAST_ZTStaff` as RVAs.
    pub const GUEST_TAG: u32 = 0x0023_8700;
    pub const STAFF_TAG: u32 = 0x0023_8710;
    /// Slot answering "not selectable as prey" on the target.
    pub const SLOT_UNPREYABLE: u32 = 0xac;
    /// Type fields: the egg-protection byte and species word, the prey-list gate byte and the prey list.
    pub const TYPE_EGG_PROTECT: u32 = 0x405;
    pub const TYPE_SPECIES: u32 = 0x1ec;
    pub const TYPE_PREY_GATE: u32 = 0x3d4;
    /// Guest byte that makes the guest unpreyable, and the habitat byte that makes a keeper preyable.
    pub const GUEST_PROTECTED: u32 = 0x33b;
    pub const HABITAT_FLAG: u32 = 0x2c;
    /// Entity id word.
    pub const ENTITY_ID: u32 = 0x124;
}

impl ZTAnimal {
    /// `ZTAnimal::addFace` (`0x004d9a2f`): appends a `(face, 0)` entry to the `(id, age)` vector at `+0x260`, growing it
    /// through the vanilla pool (doubling, at least one entry), and flags the faces as pending.
    pub fn add_face(self, smile: bool) {
        #[cfg(all(feature = "reimplementation-tests", target_os = "windows"))]
        if crate::reimplementation_tests::generate_faces_recorder::intercept_animal_face(self.0, smile) {
            return;
        }
        let begin: u32 = self.read(faces_prey::VEC_BEGIN);
        let end: u32 = self.read(faces_prey::VEC_END);
        let capacity: u32 = self.read(faces_prey::VEC_CAP);
        if end != capacity {
            if end != 0 {
                save_to_memory(end, smile as u8);
                save_to_memory(end + 4, 0u32);
            }
            save_to_memory(self.0 + faces_prey::VEC_END, end.wrapping_add(faces_prey::ENTRY_SIZE));
            save_to_memory(self.0 + faces_prey::FACES_PENDING, 1u8);
            return;
        }
        let size = (end.wrapping_sub(begin) as i32) >> 3;
        let new_len = size.wrapping_add(size.max(1)) as u32;
        let storage = if new_len == 0 { 0 } else { (unsafe { POOLALLOC_ALLOCATE.original()(new_len * faces_prey::ENTRY_SIZE) }) as u32 };
        let mut destination = storage;
        let mut source = begin;
        while source != end {
            if destination != 0 {
                save_to_memory(destination, get_from_memory::<u8>(source));
                save_to_memory(destination + 4, get_from_memory::<u32>(source + 4));
            }
            destination = destination.wrapping_add(faces_prey::ENTRY_SIZE);
            source += faces_prey::ENTRY_SIZE;
        }
        if destination != 0 {
            save_to_memory(destination, smile as u8);
            save_to_memory(destination + 4, 0u32);
        }
        if begin != 0 {
            let bytes = (((capacity.wrapping_sub(begin)) as i32 >> 3) as u32) << 3;
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
        }
        save_to_memory(self.0 + faces_prey::VEC_END, destination.wrapping_add(faces_prey::ENTRY_SIZE));
        save_to_memory(self.0 + faces_prey::VEC_BEGIN, storage);
        save_to_memory(self.0 + faces_prey::VEC_CAP, storage.wrapping_add(new_len.wrapping_mul(faces_prey::ENTRY_SIZE)));
        save_to_memory(self.0 + faces_prey::FACES_PENDING, 1u8);
    }

    /// `ZTAnimal::checkPrey` (`0x0043971c`): whether `target` is a valid prey for this animal. `force` skips the prey list
    /// and requires only the same type id on a different entity. A null target or a null type answers `false`.
    pub fn check_prey(self, target: u32, force: bool) -> bool {
        if target == 0 {
            return false;
        }
        let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let target_type: u32 = get_from_memory(target + unit_offsets::ENTITY_TYPE);
        if own_type == 0 || target_type == 0 {
            return false;
        }
        let own_animal = if Self::type_is_cast(own_type, ANIMAL_TAG) { own_type } else { 0 };
        let target_is_animal = Self::type_is_cast(target_type, ANIMAL_TAG);
        if call_slot_0(target, faces_prey::SLOT_UNPREYABLE) & 0xff != 0 {
            return false;
        }
        if target_is_animal {
            if get_from_memory::<u8>(target + animal_offsets::BOXED) != 0 {
                return false;
            }
            if get_from_memory::<u8>(target + animal_offsets::EGG) != 0 {
                if own_animal == 0 {
                    return false;
                }
                return Self::type_field::<u8>(own_animal, faces_prey::TYPE_EGG_PROTECT) != 0
                    && Self::type_field::<i32>(own_animal, faces_prey::TYPE_SPECIES) != Self::type_field::<i32>(target_type, faces_prey::TYPE_SPECIES);
            }
        }
        let first = get_from_memory::<u8>(Self::type_field::<u32>(own_type, feeding::TYPE_NAME_PTR));
        if own_animal == 0 || (first != b'm' && first != b'f' && Self::type_field::<u8>(own_animal, faces_prey::TYPE_PREY_GATE) == 0) {
            return false;
        }
        let target_id = call_slot_0(target_type, slots::TYPE_ID);
        if force {
            let own_id = call_slot_0(own_type, slots::TYPE_ID);
            return target_id == own_id && self.read::<u32>(faces_prey::ENTITY_ID) != get_from_memory::<u32>(target + faces_prey::ENTITY_ID);
        }
        let mut entry = Self::type_field::<u32>(own_animal, type_offsets::PREY_LIST_BEGIN);
        let list_end = Self::type_field::<u32>(own_animal, type_offsets::PREY_LIST_END);
        while entry != list_end {
            if get_from_memory::<u32>(entry) == target_id {
                return Self::prey_target_allowed(target, target_type);
            }
            entry += 4;
        }
        false
    }

    /// The tail of `checkPrey` once the target's type id is on the prey list: an unprotected guest or a keeper in a
    /// habitat flagged as open is fair game, as is any target that is neither guest nor keeper.
    fn prey_target_allowed(target: u32, target_type: u32) -> bool {
        let guest = if Self::type_is_cast(target_type, faces_prey::GUEST_TAG) { target } else { 0 };
        let staff = if Self::type_is_cast(target_type, faces_prey::STAFF_TAG) { target } else { 0 };
        if guest != 0 && get_from_memory::<u8>(guest + faces_prey::GUEST_PROTECTED) == 0 {
            return true;
        }
        if staff != 0 {
            let habitat = call_slot_0(staff, slots::HABITAT);
            if habitat != 0 && get_from_memory::<u8>(habitat + faces_prey::HABITAT_FLAG) != 0 {
                return true;
            }
        }
        guest == 0 && staff == 0
    }
}

/// Fields `fGetMate` touches.
mod mating {
    /// Entity ids an animal must not be paired with (its previous partners).
    pub const AVOID_ID_A: u32 = 0x2ec;
    pub const AVOID_ID_B: u32 = 0x2f0;
    /// Habitat byte that stops mate searching.
    pub const HABITAT_NO_MATING: u32 = 0x2c;
    /// Bits cleared in the low and critical status words of both animals once paired.
    pub const CLEAR_MASK: u32 = !0x8080;
    /// Gender text the habitat query matches, and its opposite.
    pub const FEMALE: &[u8] = b"Female";
    pub const MALE: &[u8] = b"Male";
}

impl ZTAnimal {
    /// Whether the animal's own text (`+0x26c`..`+0x270`) is exactly `"Female"`.
    fn gender_text_is_female(self) -> bool {
        let begin: u32 = self.read(0x26c);
        let end: u32 = self.read(0x270);
        end.wrapping_sub(begin) as usize == mating::FEMALE.len() && mating::FEMALE.iter().enumerate().all(|(i, &byte)| get_from_memory::<u8>(begin + i as u32) == byte)
    }

    /// `ZTAnimal::fGetMate` (`0x00430a19`): the current mate while it is a living animal (not gone); otherwise, in a
    /// habitat that allows mating, clears the mate id and picks a random unpaired, adult, opposite-gender animal of the same
    /// species (not one of either animal's previous partners), pairs them and sets `*found`. Returns the mate, the new
    /// partner, or `0`.
    pub fn f_get_mate(self, found: u32) -> u32 {
        let habitat = call_slot_0(self.0, slots::HABITAT);
        let target_gender: &[u8] = if self.gender_text_is_female() { mating::MALE } else { mating::FEMALE };
        if !self.f_mate_gone() {
            let world = Self::world();
            if world != 0 {
                let mate = Self::world_entity(world, self.read(animal_offsets::MATE_ID));
                if mate != 0 {
                    let mate_type: u32 = get_from_memory(mate + unit_offsets::ENTITY_TYPE);
                    if mate_type != 0 && Self::type_is_cast(mate_type, ANIMAL_TAG) {
                        return mate;
                    }
                }
            }
        }
        if habitat == 0 || get_from_memory::<u8>(habitat + mating::HABITAT_NO_MATING) != 0 {
            return 0;
        }
        save_to_memory(self.0 + animal_offsets::MATE_ID, 0u32);
        let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let species = if own_type != 0 && Self::type_is_cast(own_type, ANIMAL_TAG) { Self::type_field::<i32>(own_type, type_offsets::SPECIES) } else { 0 };
        let gender_begin = target_gender.as_ptr() as u32;
        let gender_header = [gender_begin, gender_begin + target_gender.len() as u32, gender_begin + target_gender.len() as u32 + 1];
        let mut adults = [0u32; 3];
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_adult_gender_species_animals(gender_header.as_ptr() as u32, species, adults.as_mut_ptr() as u32);
        let own_id: u32 = self.read(faces_prey::ENTITY_ID);
        let own_avoid = [self.read::<u32>(mating::AVOID_ID_B), self.read::<u32>(mating::AVOID_ID_A)];
        let mut candidates: Vec<u32> = Vec::new();
        let mut cursor = adults[0];
        while cursor != adults[1] {
            let animal: u32 = get_from_memory(cursor);
            cursor += 4;
            if get_from_memory::<u32>(animal + animal_offsets::MATE_ID) != 0 {
                continue;
            }
            let animal_id: u32 = get_from_memory(animal + faces_prey::ENTITY_ID);
            if get_from_memory::<u32>(animal + mating::AVOID_ID_A) == own_id
                || get_from_memory::<u32>(animal + mating::AVOID_ID_B) == own_id
                || own_avoid.contains(&animal_id)
            {
                continue;
            }
            candidates.push(animal);
        }
        let mut result = 0u32;
        if !candidates.is_empty() {
            let pick = candidates[(next_game_random() as i32).wrapping_rem(candidates.len() as i32) as usize];
            result = pick;
            if pick != 0 {
                save_to_memory(self.0 + animal_offsets::MATE_ID, get_from_memory::<u32>(pick + faces_prey::ENTITY_ID));
                save_to_memory(pick + animal_offsets::MATE_ID, own_id);
                save_to_memory(found, 1u8);
                for animal in [self.0, pick] {
                    for offset in [animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL] {
                        let word: u32 = get_from_memory(animal + offset);
                        save_to_memory(animal + offset, word & mating::CLEAR_MASK);
                    }
                }
            }
        }
        unsafe { POOLALLOC_DEALLOCATE_N_4.original()(adults[0] as *const u32, (adults[2].wrapping_sub(adults[0]) as i32) >> 2) };
        result
    }
}

#[detour_mod]
mod animal_detours {
    use super::*;

    #[detour(F_GET_MATE)]
    unsafe extern "thiscall" fn f_get_mate(this: *const u32, found: *const bool) -> i32 {
        ZTAnimal(this as u32).f_get_mate(found as u32) as i32
    }

    #[detour(ADD_FACE)]
    unsafe extern "thiscall" fn add_face(this: *const u32, smile: bool) {
        ZTAnimal(this as u32).add_face(smile);
    }

    #[detour(CHECK_PREY)]
    unsafe extern "thiscall" fn check_prey(this: *const u32, target: *const u32, force: bool) -> u8 {
        ZTAnimal(this as u32).check_prey(target as u32, force) as u8
    }

    #[detour(STOP_EATING)]
    unsafe extern "thiscall" fn stop_eating(this: *const u32, flag: u8) {
        ZTAnimal(this as u32).stop_eating(flag);
    }

    #[detour(STOP_CHASE)]
    unsafe extern "thiscall" fn stop_chase(this: *const u32, force: bool) {
        ZTAnimal(this as u32).stop_chase(force);
    }

    #[detour(BOX)]
    unsafe extern "thiscall" fn box_animal(this: *const u32) -> u32 {
        ZTAnimal(this as u32).box_animal() as u32
    }

    #[detour(UNBOX)]
    unsafe extern "fastcall" fn unbox(this: *const i32) -> u32 {
        ZTAnimal(this as u32).unbox() as u32
    }

    #[detour(CHANGE_TO_DIRT)]
    unsafe extern "fastcall" fn change_to_dirt(this: i32) {
        ZTAnimal(this as u32).change_to_dirt();
    }

    #[detour(SET_FOOD)]
    unsafe extern "thiscall" fn set_food(this: *const u32, food: *const u32) {
        ZTAnimal(this as u32).set_food(food as u32);
    }

    #[detour(SET_KEEPER_ARRIVES)]
    unsafe extern "thiscall" fn set_keeper_arrives(this: *const u32, staff: *const u32) {
        ZTAnimal(this as u32).set_keeper_arrives(staff as u32);
    }

    #[detour(FACE_TOWARD_FOOD)]
    unsafe extern "thiscall" fn face_toward_food(this: *const u32) {
        ZTAnimal(this as u32).face_toward_food();
    }

    #[detour(REMOVE_FOOD_TARGET_FOR_ALL)]
    unsafe extern "thiscall" fn remove_food_target_for_all(this: *const u32, food: *const u32) -> u32 {
        ZTAnimal(this as u32).remove_food_target_for_all(food as u32)
    }

    #[detour(F_REDUCE_FOOD)]
    unsafe extern "thiscall" fn f_reduce_food(this: *const u32, units: i32) -> i32 {
        ZTAnimal(this as u32).f_reduce_food(units)
    }

    #[detour(F_REMOVE_ANIMAL)]
    unsafe extern "thiscall" fn f_remove_animal(this: *const c_void) {
        ZTAnimal(this as u32).f_remove_animal();
    }

    #[detour(DROP_TEST_FENCE_CHANCE)]
    unsafe extern "thiscall" fn drop_test_fence_chance(this: *const u32) {
        ZTAnimal(this as u32).drop_test_fence_chance();
    }

    #[detour(ADD_CHANGES)]
    unsafe extern "thiscall" fn add_changes(this: *const u32, changes: *const u32) -> u32 {
        ZTAnimal(this as u32).add_changes(changes as u32) as u32
    }

    #[detour(DO_FOOD_TRICK)]
    unsafe extern "thiscall" fn do_food_trick(this: *const u32, word_a: u16, word_b: u16) -> u32 {
        ZTAnimal(this as u32).do_food_trick(word_a, word_b);
        0
    }

    #[detour(CHECK_DO_TRICK)]
    unsafe extern "thiscall" fn check_do_trick(this: *const u32) -> u32 {
        ZTAnimal(this as u32).check_do_trick() as u32
    }

    #[detour(DO_ESCAPED_CHECK)]
    unsafe extern "thiscall" fn do_escaped_check(this: *const u32) {
        ZTAnimal(this as u32).do_escaped_check();
    }

    /// `generated.rs` declares a `u32` return; the body is `void` and `doConditionals` ignores `EAX`.
    #[detour(DO_BUILDING_USE_CHECK)]
    unsafe extern "thiscall" fn do_building_use_check(this: *const u32) -> u32 {
        ZTAnimal(this as u32).do_building_use_check();
        0
    }

    #[detour(DO_CONDITIONALS)]
    unsafe extern "thiscall" fn do_conditionals(this: *const u32) {
        ZTAnimal(this as u32).do_conditionals();
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, dt: u32) {
        ZTAnimal(this as u32).update(dt);
    }

    #[detour(UPDATE_POSITION)]
    unsafe extern "thiscall" fn update_position(this: *const u32, arg: u32) {
        ZTAnimal(this as u32).update_position(arg);
    }

    /// `generated.rs` declares a pointer return; the body is `void` and its only caller ignores `EAX`.
    #[detour(DO_WATER_CHECK)]
    unsafe extern "thiscall" fn do_water_check(this: *const u32) -> *const c_void {
        ZTAnimal(this as u32).do_water_check();
        std::ptr::null()
    }

    #[detour(DO_SOCIAL_CHECK)]
    unsafe extern "thiscall" fn do_social_check(this: *const u32) {
        ZTAnimal(this as u32).do_social_check();
    }

    #[detour(DO_BORED_CHECK)]
    unsafe extern "thiscall" fn do_bored_check(this: *const u32) {
        ZTAnimal(this as u32).do_bored_check();
    }

    #[detour(UPDATE_STATUS_VARIABLES)]
    unsafe extern "thiscall" fn update_status_variables(this: *const u32) {
        ZTAnimal(this as u32).update_status_variables();
    }

    #[detour(DO_HUNGER_CHECK)]
    unsafe extern "thiscall" fn do_hunger_check(this: *const u32) {
        ZTAnimal(this as u32).do_hunger_check();
    }

    #[detour(DO_HABITAT_CHECK)]
    unsafe extern "thiscall" fn do_habitat_check(this: *const u32) {
        ZTAnimal(this as u32).do_habitat_check();
    }

    #[detour(DO_MURKY_WATER_CHECK)]
    unsafe extern "thiscall" fn do_murky_water_check(this: *const u32) {
        ZTAnimal(this as u32).do_murky_water_check();
    }

    #[detour(DO_BREATH_CHECK)]
    unsafe extern "thiscall" fn do_breath_check(this: *const u32) {
        ZTAnimal(this as u32).do_breath_check();
    }

    #[detour(DO_KEEPER_CHECK)]
    unsafe extern "thiscall" fn do_keeper_check(this: *const u32) {
        ZTAnimal(this as u32).do_keeper_check();
    }

    #[detour(DO_OTHER_CHECK)]
    unsafe extern "thiscall" fn do_other_check(this: *const u32) {
        ZTAnimal(this as u32).do_other_check();
    }

    #[detour(DO_EGG_CHECK)]
    unsafe extern "thiscall" fn do_egg_check(this: *const u32) {
        ZTAnimal(this as u32).do_egg_check();
    }

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

    #[detour(CLEAR_ESCAPE_INFO)]
    unsafe extern "thiscall" fn clear_escape_info(this: *const c_void) {
        ZTAnimal(this as u32).clear_escape_info();
    }

    #[detour(CLEAR_GOAL)]
    unsafe extern "thiscall" fn clear_goal(this: *const c_void) {
        ZTAnimal(this as u32).clear_goal();
    }

    #[detour(CLEAR_STATE_VARIABLES)]
    unsafe extern "thiscall" fn clear_state_variables(this: *const u32) {
        ZTAnimal(this as u32).clear_state_variables();
    }

    #[detour(SET_DEATH_CHECK)]
    unsafe extern "thiscall" fn set_death_check(this: *const u32, value: i32, change_type: i32) {
        ZTAnimal(this as u32).set_death_check(value, change_type);
    }

    #[detour(SET_EGG)]
    unsafe extern "thiscall" fn set_egg(this: *const u32, reset_timer: bool) {
        ZTAnimal(this as u32).set_egg(reset_timer);
    }

    #[detour(SET_ESCAPED)]
    unsafe extern "thiscall" fn set_escaped(this: *const u32, flag: i8) {
        ZTAnimal(this as u32).set_escaped(flag as u8);
    }

    #[detour(ABORT_GOAL)]
    unsafe extern "thiscall" fn abort_goal(this: *const u32, flag: bool) {
        ZTAnimal(this as u32).abort_goal(flag);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTAnimal(this as u32).save(file)
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

        pub(crate) fn f_get_mate(this: u32, found: u32) -> i32 {
            unsafe { super::F_GET_MATE_DETOUR.call(this as *const u32, found as *const bool) }
        }

        pub(crate) fn add_face(this: u32, smile: bool) {
            unsafe { super::ADD_FACE_DETOUR.call(this as *const u32, smile) }
        }

        pub(crate) fn check_prey(this: u32, target: u32, force: bool) -> u8 {
            unsafe { super::CHECK_PREY_DETOUR.call(this as *const u32, target as *const u32, force) }
        }

        pub(crate) fn stop_eating(this: u32, flag: u8) {
            unsafe { super::STOP_EATING_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn stop_chase(this: u32, force: bool) {
            unsafe { super::STOP_CHASE_DETOUR.call(this as *const u32, force) }
        }

        pub(crate) fn box_animal(this: u32) -> u32 {
            unsafe { super::BOX_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn unbox(this: u32) -> u32 {
            unsafe { super::UNBOX_DETOUR.call(this as *const i32) }
        }

        pub(crate) fn change_to_dirt(this: u32) {
            unsafe { super::CHANGE_TO_DIRT_DETOUR.call(this as i32) }
        }

        pub(crate) fn set_food(this: u32, food: u32) {
            unsafe { super::SET_FOOD_DETOUR.call(this as *const u32, food as *const u32) }
        }

        pub(crate) fn set_keeper_arrives(this: u32, staff: u32) {
            unsafe { super::SET_KEEPER_ARRIVES_DETOUR.call(this as *const u32, staff as *const u32) }
        }

        pub(crate) fn face_toward_food(this: u32) {
            unsafe { super::FACE_TOWARD_FOOD_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn remove_food_target_for_all(this: u32, food: u32) -> u32 {
            unsafe { super::REMOVE_FOOD_TARGET_FOR_ALL_DETOUR.call(this as *const u32, food as *const u32) }
        }

        pub(crate) fn f_reduce_food(this: u32, units: i32) -> i32 {
            unsafe { super::F_REDUCE_FOOD_DETOUR.call(this as *const u32, units) }
        }

        pub(crate) fn f_remove_animal(this: u32) {
            unsafe { super::F_REMOVE_ANIMAL_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn drop_test_fence_chance(this: u32) {
            unsafe { super::DROP_TEST_FENCE_CHANCE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn add_changes(this: u32, changes: u32) -> u32 {
            unsafe { super::ADD_CHANGES_DETOUR.call(this as *const u32, changes as *const u32) }
        }

        pub(crate) fn do_food_trick(this: u32, word_a: u16, word_b: u16) -> u32 {
            unsafe { super::DO_FOOD_TRICK_DETOUR.call(this as *const u32, word_a, word_b) }
        }

        pub(crate) fn check_do_trick(this: u32) -> u32 {
            unsafe { super::CHECK_DO_TRICK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update_status_variables(this: u32) {
            unsafe { super::UPDATE_STATUS_VARIABLES_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_social_check(this: u32) {
            unsafe { super::DO_SOCIAL_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_escaped_check(this: u32) {
            unsafe { super::DO_ESCAPED_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_building_use_check(this: u32) {
            unsafe { super::DO_BUILDING_USE_CHECK_DETOUR.call(this as *const u32) };
        }

        pub(crate) fn do_conditionals(this: u32) {
            unsafe { super::DO_CONDITIONALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update(this: u32, dt: u32) {
            unsafe { super::UPDATE_DETOUR.call(this as *const u32, dt) }
        }

        pub(crate) fn update_position(this: u32, arg: u32) {
            unsafe { super::UPDATE_POSITION_DETOUR.call(this as *const u32, arg) }
        }

        pub(crate) fn do_water_check(this: u32) {
            unsafe { super::DO_WATER_CHECK_DETOUR.call(this as *const u32) };
        }

        pub(crate) fn do_bored_check(this: u32) {
            unsafe { super::DO_BORED_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_hunger_check(this: u32) {
            unsafe { super::DO_HUNGER_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_habitat_check(this: u32) {
            unsafe { super::DO_HABITAT_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_murky_water_check(this: u32) {
            unsafe { super::DO_MURKY_WATER_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_breath_check(this: u32) {
            unsafe { super::DO_BREATH_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_keeper_check(this: u32) {
            unsafe { super::DO_KEEPER_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_other_check(this: u32) {
            unsafe { super::DO_OTHER_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_egg_check(this: u32) {
            unsafe { super::DO_EGG_CHECK_DETOUR.call(this as *const u32) }
        }

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

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn clear_escape_info(this: u32) {
            unsafe { super::CLEAR_ESCAPE_INFO_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn clear_goal(this: u32) {
            unsafe { super::CLEAR_GOAL_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn clear_state_variables(this: u32) {
            unsafe { super::CLEAR_STATE_VARIABLES_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_death_check(this: u32, value: i32, change_type: i32) {
            unsafe { super::SET_DEATH_CHECK_DETOUR.call(this as *const u32, value, change_type) }
        }

        pub(crate) fn set_egg(this: u32, reset_timer: bool) {
            unsafe { super::SET_EGG_DETOUR.call(this as *const u32, reset_timer) }
        }

        pub(crate) fn set_escaped(this: u32, flag: u8) {
            unsafe { super::SET_ESCAPED_DETOUR.call(this as *const u32, flag as i8) }
        }

        pub(crate) fn abort_goal(this: u32, flag: bool) {
            unsafe { super::ABORT_GOAL_DETOUR.call(this as *const u32, flag) }
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
