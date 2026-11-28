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
    bfentity::{FIND_ANIM as BFENTITY_FIND_ANIM, CLEAR_BS as BFENTITY_CLEAR_BS, CLEAR_QUEUE as BFENTITY_CLEAR_QUEUE, GET_TILE, UPDATE as BFENTITY_UPDATE},
    bfunit::{FACE_TOWARD_TILE as BFUNIT_FACE_TOWARD_TILE, STOP as BFUNIT_STOP, UPDATE_POSITION as BFUNIT_UPDATE_POSITION},
    ztscenery::SET_AMOUNT_FOOD_UNITS_DELTA,
    zoostatus::ANIMAL_ESCAPED as ZOOSTATUS_ANIMAL_ESCAPED,
    bfmap::{GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR, GET_SELECTION as BFMAP_GET_SELECTION, PAINT_CELL as BFMAP_PAINT_CELL, SET_SELECTION as BFMAP_SET_SELECTION},
    bfunit::{CHECK_DONE as BFUNIT_CHECK_DONE, GET_MOVE_ANIM_1 as BFUNIT_GET_MOVE_ANIM},
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ai_cls_0x404fd6::{
        TREE_ERASE_RANGE_0X4FE4AF as TREE_ERASE_RANGE, TREE_INSERT_UNIQUE_0X453BCC as TREE_INSERT_UNIQUE, TREE_LOWER_BOUND_0X4FE4F5 as TREE_LOWER_BOUND,
        TREE_UPPER_BOUND_0X4FE532 as TREE_UPPER_BOUND,
    },
    ztui_multianimal::{ADD_ENTITY as MULTIANIMAL_ADD_ENTITY, FORCE_UPDATE as MULTIANIMAL_FORCE_UPDATE, REMOVE_ENTITY as MULTIANIMAL_REMOVE_ENTITY},
    bf3dsorter::SORT_POSITIONS as BF3DSORTER_SORT_POSITIONS,
    bfmap::GET_NEIGHBOR_2 as BFMAP_GET_NEIGHBOR_STEP,
    bftile::IS_CLEAR_FOR_WALK_BY as BFTILE_IS_CLEAR_FOR_WALK_BY,
    bfunit::INVALIDATE_TARGET as BFUNIT_INVALIDATE_TARGET,
    ztui_animalinfo::SET_ANIMAL_DYING as ANIMALINFO_SET_ANIMAL_DYING,
    ztscenery::SET_NAME as ZTSCENERY_SET_NAME,
    msvc_std_basic_string::ASSIGN_RANGE,
    zthabitat::GET_GATE_TILE_IN,
    ztworldmgr::SATISFIES as ZTWORLDMGR_SATISFIES,
    bfcoggoal::SET_TILE as BFCOGGOAL_SET_TILE,
    bfentity::{CHECK_AVOID_EDGES, SET_BS, SET_DIRECTION as BFENTITY_SET_DIRECTION},
    zthabitat::TRIGGER_DEATH_ARRIVED,
    bfmap::GET_DIRECTION_0 as BFMAP_GET_DIRECTION,
    bftile::IS_IN_ZOO as BFTILE_IS_IN_ZOO,
    bfunit::{GET_DEST_TILE, GET_PATH_1 as BFUNIT_GET_PATH, GO_TO_DEST_0 as BFUNIT_GO_TO_DEST, IS_MAX_COST_FOR_ALL, IS_SAME_TILE},
    ztfence::{IS_ELECTRIFIED as ZTFENCE_IS_ELECTRIFIED, SET_BROKEN as ZTFENCE_SET_BROKEN},
    ztscenerytype::IS_ZOO_GATE as ZTSCENERYTYPE_IS_ZOO_GATE,
    msvc_std_tree36::OPERATOR_INDEX as TREE36_OPERATOR_INDEX,
    bfmap::{GET_DIRECTION_1 as BFMAP_GET_DIRECTION_FROM_OFFSET, VALIDATE as BFMAP_VALIDATE},
    bfentity::GET_GRID_POS as BFENTITY_GET_GRID_POS,
    bfunit::GET_PATH_0 as BFUNIT_GET_PATH_TO_POSITION,
    msvc_std_list_ivec3::INSERT_IVEC3 as LIST_IVEC3_INSERT,
    ztunittype::{GET_DIRT_TYPE, GET_DINO_DIRT_TYPE},
    gxmixer::SET_BASE_0 as GXMIXER_SET_BASE,
    msvc_std::RAND as MSVC_RAND,
    bfentity::SET_WORLD_POS as BFENTITY_SET_WORLD_POS,
    bfmap::TILE_TO_WORLD as BFMAP_TILE_TO_WORLD,
    gxmixerlink::{CONSTRUCTOR as GXMIXERLINK_CONSTRUCTOR, DESTRUCTOR as GXMIXERLINK_DESTRUCTOR},
    ztunit::STOP_EVERYTHING as ZTUNIT_STOP_EVERYTHING,
    ztanimal::{
        COMPARE_LT, GET_IDLE_ANIM_SYMBOL, GET_MOVE_ANIM, GET_ONLY_SWIMS,
        BREAK_ESCAPE_FENCE, FIND_PREY, GET_TERRAIN_COST, INVALIDATE_TARGET, SET_REACHED_ESCAPE_DESTINATION_TILE, SET_REACHED_ESCAPE_TILE,
        SET_REACHED_ESCAPE_TRANSITION_TILE, STOP_EVERYTHING, TILE_FILTER, FENCES_OK, EVALUATE_ESCAPE, PICK_RANDOM_DEST, GET_FEEDING_PATH,
        SET_FOOD, SET_KEEPER_ARRIVES, FACE_TOWARD_FOOD, REMOVE_FOOD_TARGET_FOR_ALL, F_REDUCE_FOOD, F_REMOVE_ANIMAL, DROP_TEST_FENCE_CHANCE, ADD_CHANGES,
        DO_FOOD_TRICK, CHECK_DO_TRICK, BOX, UNBOX, ADD_FACE, CHECK_PREY, STOP_EATING, STOP_CHASE, CHASE_DONE, CHANGE_TO_DIRT, FINISH_HATCH, CONSUME,
        CALC_HABITAT, CAN_CAPTURE, CAN_SERVICE, CAN_SLEEP_BE_SICK_EAT, CHECK_DONE, CLIFFS_OK, F_ANIMAL_SPACE_RATIO, F_FOOD_UNITS_ACTUALLY_EATEN,
        F_CHECK_REPRODUCTION, F_MATE_AVAILABLE, F_MATE_GONE, F_OTHER_ANIMAL_ANGRY, F_OTHER_ANIMAL_SICK, GET_HABITAT, GET_HOME_HABITAT, GET_ICON_ANIM, GET_PREY_UNIT,
        GET_SELL_PRICE, GET_SWIMS, GET_UNDERWATER, GET_UNDESIRABLE_SCENERY_IN_HABITAT, IS_NEED_SATISFIER, IS_WALKABLE_BY, RECALC_HOME_HABITAT,
        SET_HOME_HABITAT, F_GET_MATE, IS_DOING_SOMETHING, DECIDE, INTO_NEW_TILE, UPDATE_GOALS,
        F_COMPAT_ANIMALS_RATING, F_NUM_ANIMALS, F_NUM_SAME_ADULT_ANIMALS, F_NUM_SAME_ANIMALS, F_TILES_PER_ADULT_ANIMAL, GET_ICON_ZOOM, GET_IDLE_ANIM,
        SET_MAX_HEALTH, DO_AMBIENT_ANIMS, DO_BREATH_CHECK, DO_EGG_CHECK, UPDATE, UPDATE_POSITION, DO_CONDITIONALS, SHOW_ESCAPED_ANIMAL_ALERT, DO_BOXED_CHECK, DO_BABY_TO_ADULT_CHECK, DO_REPRODUCE_CHECK, DO_ESCAPED_CHECK, DO_HEALTH_CHECK, DO_ZAP_CHECK, DO_BUILDING_USE_CHECK, DO_BORED_CHECK, DO_SLEEP_CHECK, DO_WATER_CHECK, DO_SOCIAL_CHECK, DO_HABITAT_CHECK, DO_HUNGER_CHECK, SET_TERRAIN_MODE, UPDATE_STATUS_VARIABLES, F_EAT, PUT_DOWN_DIRT, DO_MURKY_WATER_CHECK, DO_KEEPER_CHECK, DO_OTHER_CHECK, HATCH_EGG,
        F_IS_THERE_KEEPER_FOOD, GET_AMBIENT_KEY, GET_HABITAT_RATING, GET_HAPPY_PCT, GET_HEALTH_PCT, GET_HUNGER_PCT, GET_KEEP_MOVING, CLEAR_PREY,
        DIAGONAL_OK, IS_EGG, ABORT_GOAL, SAVE, LOAD, INIT_STATUS_VARS, ADD_TO_MAP, REMOVE_FROM_MAP, SET_DYING, SET_NAME, CLEAR_ESCAPE_INFO, CLEAR_GOAL, CLEAR_STATE_VARIABLES, FIND_PLACE_TO_REST, SET_DEATH_CHECK, SET_EGG, SET_ESCAPED, TARGET_BUILDING, IS_HUNGRY, IS_HUNGRY_AND_FOODLESS, IS_IN_PREY_LIST, IS_PREATTACK, IS_SELECTABLE, IS_SICKLY, IS_UNHAPPY_FOR_REPRODUCTION,
        IS_UNHAPPY_WITH_HABITAT, IS_VERY_UNHAPPY_WITH_HABITAT, IS_WALKABLE, NEEDS_BREATH, NO_SHADOW_PART, PREYS_ON_MAN, WITHIN_PREATTACK_RANGE,
        DROP, SET_VISIBLE,
    },
    ztui_animalinfo::SET_ANIMAL_VISIBLE as ANIMALINFO_SET_ANIMAL_VISIBLE,
    ztui_animalinfo::FORCE_UPDATE as ANIMALINFO_FORCE_UPDATE,
    ztworldmgr::GET_NEED_INDEX,
    ztgamemgr::GET_DATE as GAMEMGR_GET_DATE,
    msvc_std_listuint::INSERT as LIST_UINT_INSERT,
    standalone::{IS_ANIMAL_BUILDING_SATISFIER, IS_ANIMAL_REST_SATISFIER},
    ztanimal::IS_ON_GRASS as ZTANIMAL_IS_ON_GRASS,
    bfmap::{CAN_ADD_ENTITY as BFMAP_CAN_ADD_ENTITY, GET_NEIGHBORS_1 as BFMAP_GET_NEIGHBORS_AROUND},
    ztworldmgr::{CREATE_SCENERY as ZTWORLDMGR_CREATE_SCENERY, FIND_ADJACENT_ELECTRIC_FENCE_TO_ANIMAL as ZTWORLDMGR_FIND_FENCE},
    msvc_std_dequegxmixerchain::CLEAR_0X41BF8F as MIXER_CHAIN_DEQUE_CLEAR,
    gxmixerlink::COPY_0X40DDD5 as GXMIXERLINK_COPY,
    ztui_animalinfo::SET_ANIMAL_ESCAPED as ANIMALINFO_SET_ANIMAL_ESCAPED,
    bfaimgr::F_RANDOM_WALK as BFAIMGR_F_RANDOM_WALK,
    bfcoggoal::{CLEAR as BFCOGGOAL_CLEAR, SET as BFCOGGOAL_SET, SET_ID as BFCOGGOAL_SET_ID, SET_TARGET_POS as BFCOGGOAL_SET_TARGET_POS},
    bfuimgr::{CONFIRM_DIALOG_0 as BFUIMGR_CONFIRM_DIALOG, DISPLAY_MESSAGE_1 as BFUIMGR_DISPLAY_MESSAGE_1, GET_ELEMENT_0 as BFUIMGR_GET_ELEMENT},
    uicontrol::SET_ANIMATION as UICONTROL_SET_ANIMATION,
    bfunit::DECIDE as BFUNIT_DECIDE,
    ztapp::GET_APP,
    zthabitat::SET_DIRTY_CHARACTERISTICS as ZTHABITAT_SET_DIRTY_CHARACTERISTICS,
};
use glam::i32::IVec3;
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfentity::{new_owned_name, set_bs_overridable as set_bs, set_bs_new, set_pending_bs, set_queued_call_new},
    globals::{get_module_base, globals},
    util::{ftol_low32, get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, ZTHabitat, ZTTankExhibit},
    pool_vec::PoolVec,
    vanilla_string::VanillaString,
    ztunit::ZTUnit,
    ztkeeper::{read_field, call_slot_0, call_slot_1, call_slot_2, call_slot_3, unit_offsets, write_field},
    ztmegatilemgr::entity_type_matches,
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

/// The vanilla memory layout of a `ZTAnimal` (`ZTUnit` prefix, then the animal's own fields up to [`ANIMAL_SIZE`]). Ports
/// read and write animals through the [`ZTAnimal`] address handle with volatile accesses, so this struct is never
/// instantiated or dereferenced: it exists so the compiler checks the layout. Each offset in [`animal_offsets`] that
/// names a whole field is asserted against `offset_of!` below, together with the total size; fields not yet given a
/// role stay in the arrays (`countdowns`, `flag_bytes`) the save order treats as opaque. `_live` marks the struct
/// `!Freeze` like every other vanilla-layout struct.
#[repr(C)]
#[allow(dead_code)]
pub struct ZTAnimalLayout {
    /// `ZTUnit` and its bases (`0x000`-`0x25f`).
    unit: [u8; 0x260],
    /// `(face byte, age)` entries of `addFace`/`update`: `{begin, end, capacity}` of a vanilla pool vector.
    faces: [u32; 3],
    /// The two strings (`{begin, end, capacity}` of vanilla pool `vector<byte>`s).
    first_string: [u32; 3],
    second_string: [u32; 3],
    word_284: u32,
    stamp_288: [u32; 2],
    stamp_290: [u32; 2],
    /// Low flag words, critical flag words (`+0x298`, `+0x29c`, `+0x2a0`, `+0x2a4`).
    status_flags: [u32; 4],
    happiness: i32,
    happiness_pending: i32,
    hunger: i32,
    hunger_pending: i32,
    health: i32,
    health_pending: i32,
    need_c0: i32,
    need_c0_pending: i32,
    need_c8: i32,
    need_c8_pending: i32,
    words_2d0: [i32; 5],
    prey_id: u32,
    mate_id: u32,
    /// Per-check countdowns and counters (`+0x2ec`-`+0x373`); `+0x34c` is the death countdown, `+0x360` the breath value.
    countdowns: [i32; 34],
    word_374: i32,
    word_378: i32,
    word_37c: i32,
    activity_mode: i32,
    escape_record: u32,
    cached_habitat: u32,
    eat_context: u32,
    byte_390: u8,
    has_escape_info: u8,
    byte_392: u8,
    capture_stage: u8,
    boxed: u8,
    egg: u8,
    pad_396: [u8; 2],
    egg_timer: i32,
    /// `+0x39c`-`+0x3b3`: the state bytes (`escaped` at `+0x3a3`, `sick` `+0x3a7`, `angry` `+0x3aa`, ...).
    flag_bytes: [u8; 0x18],
    home_gate_tile: u32,
    pub _live: crate::util::LiveMemory,
}

const _: () = {
    use std::mem::{offset_of, size_of};
    assert!(size_of::<ZTAnimalLayout>() == ANIMAL_SIZE as usize);
    assert!(offset_of!(ZTAnimalLayout, faces) == lifecycle::VEC_BEGIN as usize);
    assert!(offset_of!(ZTAnimalLayout, status_flags) == animal_offsets::STATUS_FLAGS as usize);
    assert!(offset_of!(ZTAnimalLayout, status_flags) + 8 == animal_offsets::STATUS_FLAGS_CRITICAL as usize);
    assert!(offset_of!(ZTAnimalLayout, status_flags) + 12 == animal_offsets::STATUS_FLAGS_UI as usize);
    assert!(offset_of!(ZTAnimalLayout, happiness) == animal_offsets::HAPPINESS as usize);
    assert!(offset_of!(ZTAnimalLayout, happiness_pending) == animal_offsets::HAPPINESS_PENDING as usize);
    assert!(offset_of!(ZTAnimalLayout, hunger) == animal_offsets::HUNGER as usize);
    assert!(offset_of!(ZTAnimalLayout, health) == animal_offsets::HEALTH as usize);
    assert!(offset_of!(ZTAnimalLayout, health_pending) == animal_offsets::HEALTH_PENDING as usize);
    assert!(offset_of!(ZTAnimalLayout, prey_id) == animal_offsets::PREY_ID as usize);
    assert!(offset_of!(ZTAnimalLayout, mate_id) == animal_offsets::MATE_ID as usize);
    assert!(offset_of!(ZTAnimalLayout, countdowns) == 0x2ec);
    assert!(offset_of!(ZTAnimalLayout, countdowns) + 4 * 24 == lifecycle::DEATH_CHECK as usize);
    assert!(offset_of!(ZTAnimalLayout, countdowns) + 4 * 29 == animal_offsets::BREATH as usize);
    assert!(offset_of!(ZTAnimalLayout, activity_mode) == animal_offsets::ACTIVITY_MODE as usize);
    assert!(offset_of!(ZTAnimalLayout, escape_record) == animal_offsets::ESCAPE_RECORD as usize);
    assert!(offset_of!(ZTAnimalLayout, cached_habitat) == animal_offsets::CACHED_HABITAT as usize);
    assert!(offset_of!(ZTAnimalLayout, eat_context) == animal_offsets::EAT_CONTEXT as usize);
    assert!(offset_of!(ZTAnimalLayout, has_escape_info) == animal_offsets::HAS_ESCAPE_INFO as usize);
    assert!(offset_of!(ZTAnimalLayout, capture_stage) == animal_offsets::CAPTURE_STAGE as usize);
    assert!(offset_of!(ZTAnimalLayout, boxed) == animal_offsets::BOXED as usize);
    assert!(offset_of!(ZTAnimalLayout, egg) == animal_offsets::EGG as usize);
    assert!(offset_of!(ZTAnimalLayout, egg_timer) == lifecycle::EGG_TIMER as usize);
    assert!(offset_of!(ZTAnimalLayout, flag_bytes) == 0x39c);
    assert!(offset_of!(ZTAnimalLayout, flag_bytes) + (animal_offsets::CAPTURE_FLAG - 0x39c) as usize == lifecycle::ESCAPED as usize);
    assert!(offset_of!(ZTAnimalLayout, flag_bytes) + (animal_offsets::SICK - 0x39c) as usize == animal_offsets::FLAG_0X3A7 as usize);
    assert!(offset_of!(ZTAnimalLayout, home_gate_tile) == animal_offsets::HOME_GATE_TILE as usize);
};

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
    /// The block at `+0x8c` that `setEgg` resets (`0x41bf8f`, `deque<..>::clear_0x41bf8f`).
    pub const BLOCK_8C: u32 = 0x8c;
    pub const SLOT_CLEAR_GOAL: u32 = 0x174;
    pub const SLOT_STOP_EVERYTHING: u32 = 0x198;
    pub const SLOT_UPDATE_ANIMATION: u32 = 0x40;
    pub const SLOT_IS_DOING_SHOW: u32 = 0x22c;
    pub const SLOT_ABORT_TRICK: u32 = 0x2c0;
    pub const SLOT_CLEAR_PREY: u32 = 0x258;
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

/// Addresses and type fields `ZTAnimal::load` clamps against.
mod load_layout {
    /// `DAT_00639454`: one past the largest value `+0x348` may hold after a load.
    pub const MAX_WORD_RVA: u32 = 0x0023_9454;
    /// Entity-type field one past which the countdown `+0x34c` is clamped.
    pub const TYPE_COUNTDOWN_LIMIT: u32 = 0x384;
}

/// Globals and type fields `ZTAnimal::initStatusVars` reads.
mod init_layout {
    /// `s_Female_006393d0`, `s_Male_006393d8` and `DAT_00639458` (the second string's text), as RVAs.
    pub const FEMALE_RVA: u32 = 0x0023_93d0;
    pub const MALE_RVA: u32 = 0x0023_93d8;
    pub const SECOND_STRING_RVA: u32 = 0x0023_9458;
    /// `DAT_00639448`: one past the largest value `+0x354` may hold.
    pub const LIMIT_354_RVA: u32 = 0x0023_9448;
    /// Entity-type block the unit's `+0x68` points at, and the word that gates it.
    pub const BLOCK: u32 = 0x80;
    pub const BLOCK_GATE: u32 = 0x84;
    /// `(period global RVA, animal field)`: the draws, in vanilla's order. A zero period stores `-1`.
    pub const PERIODS: &[(u32, u32)] = &[
        (0x0023_940c, 0x2f4),
        (0x0023_9450, 0x364),
        (0x0023_941c, 0x2f8),
        (0x0023_93fc, 0x300),
        (0x0023_93e0, 0x304),
        (0x0023_9424, 0x308),
        (0x0023_9410, 0x30c),
        (0x0023_93f8, 0x310),
        (0x0023_9400, 0x314),
        (0x0023_9414, 0x318),
        (0x0023_9420, 0x324),
        (0x0023_9404, 0x328),
        (0x0023_93e8, 0x33c),
        (0x0023_9408, 0x344),
        (0x0023_9418, 0x320),
        (0x0023_93e4, 0x2fc),
    ];
}

/// The multi-animal panel's set and the unit slots `addToMap` branches on.
mod multianimal {
    /// `DAT_00639460` as an RVA.
    pub const SET_RVA: u32 = 0x0023_9460;
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_ON_LAND: u32 = 0xbc;
    pub const SLOT_IN_WATER: u32 = 0x16c;
    /// Tile bytes `addToMap` reads: `+0x83` (low two bits: land) and `+0x85` (bit 3: preys on man, bit 5: water).
    pub const TILE_FLAGS_A: u32 = 0x83;
    pub const TILE_FLAGS_B: u32 = 0x85;
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

/// Offsets used by the presentation getters and `compareLT`.
mod presentation {
    pub const TYPE_ONLY_SWIMS: u32 = 0x48;
    pub const TYPE_EGG_IDLE_SYMBOL: u32 = 0xd8;
    pub const TYPE_BOX_IDLE_SYMBOL: u32 = 0xd4;
    pub const TYPE_NAME: u32 = 0x14;
    pub const MOVE_ANIM_GATE: u32 = 0xbc;
    pub const NAME_BEGIN: u32 = 0x108;
    pub const NAME_END: u32 = 0x10c;
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

/// An animal in vanilla memory, addressed by its raw pointer. All reads and writes are volatile through the address;
/// [`ZTAnimalLayout`] holds the compile-time-checked layout those offsets refer to.
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
        ZTUnit(self.0).get_idle_anim(force_unit)
    }

    /// `ZTAnimal::getIdleAnimSymbol`: an egg answers its type's slot `+0xd8`, a boxed animal the type's slot `+0xd4`,
    /// everything else is `ZTUnit::getIdleAnimSymbol`. A null or non-animal type faults in vanilla; the port answers `0`.
    pub fn get_idle_anim_symbol(self) -> u32 {
        let slot = if self.is_egg() != 0 {
            presentation::TYPE_EGG_IDLE_SYMBOL
        } else if self.is_boxed() {
            presentation::TYPE_BOX_IDLE_SYMBOL
        } else {
            return ZTUnit(self.0).get_idle_anim_symbol();
        };
        let Some(entity_type) = self.animal_type() else { return 0 };
        let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + slot)) };
        function(entity_type)
    }

    /// `ZTAnimal::getOnlySwims`: the type's own slot `+0x48`. A null or non-animal type faults in vanilla; the port answers `false`.
    pub fn get_only_swims(self) -> bool {
        let Some(entity_type) = self.animal_type() else { return false };
        let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + presentation::TYPE_ONLY_SWIMS)) };
        function(entity_type) & 0xff != 0
    }

    /// `ZTAnimal::getMoveAnim(name)`: on a tile whose `+0x83` low two bits are set and while vtable `+0xbc` answers true
    /// the move animation is `BFUnit::getMoveAnim` (no argument); otherwise `BFEntity::findAnim(name)`.
    pub fn get_move_anim(self, name: *const i8) -> *const i8 {
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        if get_from_memory::<u8>(tile + 0x83) & 3 != 0 {
            let vtable: u32 = get_from_memory(self.0);
            let gate = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(vtable + presentation::MOVE_ANIM_GATE)) };
            if gate(self.0) & 0xff != 0 {
                return unsafe { BFUNIT_GET_MOVE_ANIM.hooked()(self.0 as *const u32) } as usize as *const i8;
            }
        }
        unsafe { BFENTITY_FIND_ANIM.original()(self.0 as *const u32, name as *const c_void) }
    }

    /// `ZTAnimal::compareLT` (`operator<`): the ordering of the multi-animal selection set. Entity type first (by the
    /// type's name, through type vtable `+0x14`), then the entity name (`+0x108`/`+0x10c`), then the entity id.
    pub fn compare_lt(self, other: u32) -> bool {
        let (own_type, other_type): (u32, u32) = (self.read(unit_offsets::ENTITY_TYPE), get_from_memory(other + unit_offsets::ENTITY_TYPE));
        if own_type != other_type {
            let own_name = Self::type_name_bytes(own_type);
            let other_name = Self::type_name_bytes(other_type);
            if own_name < other_name {
                return true;
            }
            if other_name < own_name {
                return false;
            }
        }
        let own_name = Self::name_bytes(self.0);
        let other_name = Self::name_bytes(other);
        if own_name < other_name {
            return true;
        }
        if other_name < own_name {
            return false;
        }
        self.read::<i32>(unit_offsets::ENTITY_ID) < get_from_memory::<i32>(other + unit_offsets::ENTITY_ID)
    }

    /// The bytes of a `(begin, end)` pointer pair; an empty or inverted pair reads as empty.
    fn range_bytes(begin: u32, end: u32) -> Vec<u8> {
        if begin == 0 || end <= begin {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }.to_vec()
    }

    fn name_bytes(entity: u32) -> Vec<u8> {
        Self::range_bytes(get_from_memory(entity + presentation::NAME_BEGIN), get_from_memory(entity + presentation::NAME_END))
    }

    /// The type's name through type vtable `+0x14`, which returns a pointer to a `(begin, end)` pair.
    fn type_name_bytes(entity_type: u32) -> Vec<u8> {
        if entity_type == 0 {
            return Vec::new();
        }
        let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + presentation::TYPE_NAME)) };
        let pair = function(entity_type);
        if pair == 0 {
            return Vec::new();
        }
        Self::range_bytes(get_from_memory(pair), get_from_memory(pair + 4))
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

    /// `ZTAnimal::getSwims`: an egg never swims, everything else is `ZTUnit::getSwims` (`0x00410764`).
    pub fn get_swims(self) -> u32 {
        if self.is_egg() != 0 {
            return 0;
        }
        u32::from(ZTUnit(self.0).get_swims())
    }

    /// `ZTAnimal::getUnderwater`: an egg is never underwater, everything else is `ZTUnit::getUnderwater` (`0x00410794`).
    pub fn get_underwater(self) -> u32 {
        if self.is_egg() != 0 {
            return 0;
        }
        u32::from(ZTUnit(self.0).get_underwater())
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
        ZTUnit(self.0).cliffs_ok(tile, direction)
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
        if ZTUnit(self.0).is_doing_something() {
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
        let habitat = ZTUnit(self.0).get_habitat();
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
        let mut local = PoolVec::sized(4);
        let habitat = call_slot_0(self.0, slots::HABITAT);
        if habitat != 0 && get_from_memory::<u8>(habitat + 0x2c) == 0 {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_undesirable_scenery(self.0, local.out_pointer() as u32, flag != 0);
        }
        let (begin, end) = (local.begin(), local.end());
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
        // `local`'s buffer goes back to the pool here.
        drop(local);
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
        ZTUnit(self.0).clear_goal();
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
        ZTUnit(self.0).clear_state_variables();
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
        unsafe { MIXER_CHAIN_DEQUE_CLEAR.original()((self.0 + lifecycle::BLOCK_8C) as *const u32) };
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
    /// `+0x3a1`/`+0x3a2`; the selection notifier (`ZTUI::animalinfo::setAnimalEscaped`, `0x00440a27`) always runs.
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
                        crate::vanilla_list::free_node(node);
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
        unsafe { ANIMALINFO_SET_ANIMAL_ESCAPED.original()(self.0 as *const u32, flag != 0) };
    }

    /// Whether the unit's need byte (`+0x24e`) names the need at `need_rva` and the unit has no building-use goal.
    fn need_matches_without_building_goal(self, need_rva: u32) -> bool {
        let index = unsafe { GET_NEED_INDEX.hooked()((base() + need_rva) as *const i8) } as u8;
        self.read::<u8>(lifecycle::UNIT_NEED_BYTE) == index && !ZTUnit(self.0).has_building_use_goal()
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
                if unsafe { IS_ANIMAL_REST_SATISFIER.hooked()(tile as *const u32, position.as_ptr()) } {
                    call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
                    let found = unsafe { FIND_PLACE_TO_REST.hooked()(self.0 as *const u32, false) };
                    return self.finish_abort(low_byte_bool(found));
                }
            }
            if self.need_matches_without_building_goal(lifecycle::NEED_B) {
                let tile: u32 = self.read(lifecycle::UNIT_REST_TILE);
                if unsafe { IS_ANIMAL_BUILDING_SATISFIER.hooked()(tile as *const u32, position.as_ptr()) } {
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
        let unit_ok = ZTUnit(self.0).save(file as u32);
        unit_ok & ok
    }

    /// `BFWorldMgr` tile at `(x, y)` of the zoo map, or `0` outside it.
    fn world_tile(x: i32, y: i32) -> u32 {
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let width: i32 = get_from_memory(world + 8 + movement::MAP_X_SIZE);
        let height: i32 = get_from_memory(world + 8 + movement::MAP_Y_SIZE);
        if x < 0 || y < 0 || x >= width || y >= height {
            return 0;
        }
        let tiles: u32 = get_from_memory(world + 8 + movement::MAP_TILES);
        tiles.wrapping_add((width.wrapping_mul(y).wrapping_add(x) as u32).wrapping_mul(movement::TILE_SIZE))
    }

    /// One length-prefixed string of `load`: a length of `0x1000` or more, or a failed length read, fails without
    /// touching the string at `offset`; otherwise the bytes (zero-filled when their read fails) are assigned to it.
    fn load_string(self, offset: u32, file: *const u32, version: u32) -> bool {
        let mut length: u32 = 0;
        if !read_field(&mut length as *mut u32 as u32, 4, file) || length >= 0x1000 {
            return false;
        }
        let mut bytes = vec![0u8; length as usize];
        let ok = length == 0 || read_field(bytes.as_mut_ptr() as u32, length, file);
        let begin = bytes.as_ptr();
        unsafe { ASSIGN_RANGE.original()((self.0 + offset) as *const c_void, begin as *const u32, begin.add(bytes.len()), version as *const u8) };
        ok
    }

    /// `ZTAnimal::load` (`0x00455533`) for every stream version; the fields, in stream order, are in the body. Every
    /// read runs even after a failed one, except at the two points where vanilla returns early without calling
    /// `ZTUnit::load` (after the escape record's words and after the home gate tile's coordinates), so the bytes consumed
    /// match. Fields the stream does not carry keep their value, except the habitat cache (`+0x388`) and the eat
    /// context (`+0x38c`), which are zeroed; `+0x391` and the escape record follow the stream's has-escape-info word.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let mut ok = true;
        // Vanilla keeps the discarded dword of version `0x2b` and the escape record's marked dword in one stack slot, so
        // a failed marked read sees the discarded value.
        let mut shared_slot: u32 = 0;
        macro_rules! read {
            ($offset:expr, $size:expr) => {
                ok &= read_field(self.0 + $offset, $size, file)
            };
        }
        for offset in [0x2a8u32, 0x2b0, 0x2b8, 0x2c0] {
            read!(offset, 4);
        }
        for offset in [0x3a7u32, 0x3a8, 0x3a9, 0x3aa] {
            read!(offset, 1);
        }
        if version < 0x10 {
            read!(0x13c, 1);
        }
        read!(0x3ab, 1);
        read!(0x378, 4);
        read!(0x390, 1);
        for offset in [0x2d0u32, 0x2d4, 0x2d8, 0x2dc, 0x2e0] {
            read!(offset, 4);
        }
        ok &= self.load_string(0x26c, file, version);
        ok &= self.load_string(0x278, file, version);
        if version >= 5 {
            for offset in [0x2f4u32, 0x2f8, 0x300, 0x308, 0x30c, 0x310, 0x314, 0x318, 0x324, 0x328, 0x334] {
                read!(offset, 4);
            }
            if version < 0x36 {
                let mut discarded: u32 = 0;
                ok &= read_field(&mut discarded as *mut u32 as u32, 4, file);
            }
            read!(0x338, 4);
            read!(0x33c, 4);
        }
        if version >= 0xe {
            read!(0x2c8, 4);
        }
        if version >= 0x10 {
            read!(0x39c, 1);
            read!(0x39e, 1);
        }
        if version >= 0x12 {
            read!(lifecycle::ESCAPED, 1);
            if self.flag(lifecycle::ESCAPED) {
                let sentinel: u32 = get_from_memory(base() + lifecycle::ESCAPE_LIST);
                let mut result: u32 = 0;
                let value = self.0;
                unsafe { LIST_UINT_INSERT.original()(&mut result as *mut u32 as *const i32, sentinel as i32, &value as *const u32 as *const i32) };
            }
        }
        if version >= 0x13 {
            read!(0x304, 4);
        }
        if version >= 0x16 {
            read!(0x3a4, 1);
            read!(0x3a5, 1);
            read!(0x2e4, 4);
        }
        if version >= 0x1f {
            read!(0x32c, 4);
            read!(0x330, 4);
            read!(0x2e8, 4);
        }
        if version >= 0x21 {
            read!(0x288, 8);
            read!(0x290, 8);
        }
        if version >= 0x23 {
            read!(0x39f, 1);
            read!(0x3a0, 1);
        }
        if version >= 0x29 {
            read!(0x2ec, 4);
            read!(0x2f0, 4);
        }
        if version >= 0x2b {
            let entity_type = self.animal_type();
            read!(0x348, 4);
            let word_limit: i32 = get_from_memory::<i32>(base() + load_layout::MAX_WORD_RVA).wrapping_sub(1);
            if self.read::<i32>(0x348) > word_limit {
                save_to_memory(self.0 + 0x348, word_limit);
            }
            read!(0x34c, 4);
            // Vanilla reads the type unchecked here and faults on an animal without one.
            if let Some(entity_type) = entity_type {
                let limit = Self::type_field::<i32>(entity_type, load_layout::TYPE_COUNTDOWN_LIMIT).wrapping_sub(1);
                if self.read::<i32>(0x34c) > limit {
                    save_to_memory(self.0 + 0x34c, limit);
                }
            }
            ok &= read_field(&mut shared_slot as *mut u32 as u32, 4, file);
            read!(0x3a6, 1);
        }
        if version >= 0x2c {
            read!(0x39d, 1);
        }
        if version >= 0x2e {
            read!(0x320, 4);
        }
        save_to_memory(self.0 + animal_offsets::CACHED_HABITAT, 0u32);
        save_to_memory(self.0 + animal_offsets::EAT_CONTEXT, 0u32);
        if version >= 0x2f {
            let mut has_record: u32 = 0;
            ok &= read_field(&mut has_record as *mut u32 as u32, 4, file);
            save_to_memory(self.0 + animal_offsets::HAS_ESCAPE_INFO, (has_record == 1) as u8);
            if has_record == 1 {
                // First tile (x, y), second tile (x, y), then the flags and the `+0xc` word, in the order the stream holds
                // them (the save writes them as x, y, x, y, word, three flags).
                let mut words = [0u32; 7];
                for word in &mut words {
                    ok &= read_field(word as *mut u32 as u32, 4, file);
                }
                if !ok {
                    return false;
                }
                let [first_x, first_y, second_x, second_y, word, set_a, set_b] = words;
                let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
                for offset in (0..0x10).step_by(4) {
                    save_to_memory(record + offset, 0u32);
                }
                save_to_memory(self.0 + animal_offsets::ESCAPE_RECORD, record);
                save_to_memory(record, 1u8);
                save_to_memory(record + 0xc, word);
                if set_a == 1 {
                    save_to_memory(record + 1, 1u8);
                }
                if set_b == 1 {
                    save_to_memory(record + 2, 1u8);
                }
                save_to_memory(record + 4, Self::world_tile(first_x as i32, first_y as i32));
                save_to_memory(record + 8, Self::world_tile(second_x as i32, second_y as i32));
                if version >= 0x51 {
                    ok &= read_field(&mut shared_slot as *mut u32 as u32, 4, file);
                    if shared_slot != 0 {
                        save_to_memory(record + 3, 1u8);
                    }
                }
            }
        }
        if version >= 0x34 {
            read!(0x354, 4);
            read!(0x374, 4);
            read!(0x393, 1);
            read!(0x394, 1);
        }
        if version >= 0x36 {
            read!(0x3a1, 1);
        }
        if version == 0x39 || version >= 0x3d {
            read!(0x3a2, 1);
        }
        if version >= 0x4c {
            read!(0x3b0, 1);
        }
        if version >= 0x4d {
            read!(0x358, 4);
            read!(0x35c, 4);
        }
        if version >= 0x4e {
            read!(0x3b1, 1);
        }
        if version >= 0x4f {
            read!(0x395, 1);
            read!(0x398, 4);
        }
        if version >= 0x57 {
            read!(0x360, 4);
            read!(0x364, 4);
        }
        if version >= 0x65 {
            let default_x: u32 = get_from_memory(base() + save_layout::DEFAULT_GATE_X);
            let default_y: u32 = get_from_memory(base() + save_layout::DEFAULT_GATE_Y);
            let (mut x, mut y) = (default_x, default_y);
            ok &= read_field(&mut x as *mut u32 as u32, 4, file);
            ok &= read_field(&mut y as *mut u32 as u32, 4, file);
            if !ok {
                return false;
            }
            if x != default_x || y != default_y {
                save_to_memory(self.0 + animal_offsets::HOME_GATE_TILE, Self::world_tile(x as i32, y as i32));
            }
        }
        if !ok {
            return false;
        }
        ZTUnit(self.0).load(file as u32, version) & ok
    }

    /// Replaces the pool string at `offset` with the NUL-terminated text at `text`.
    fn assign_text(self, offset: u32, text: u32) {
        let mut length = 0;
        while get_from_memory::<u8>(text + length) != 0 {
            length += 1;
        }
        unsafe { ASSIGN_RANGE.original()((self.0 + offset) as *const c_void, text as *const u32, (text + length) as *const u8, std::ptr::null()) };
    }

    /// One draw modulo the period at `period_rva`; `-1` without a draw when the period is zero.
    fn draw_period(period_rva: u32) -> i32 {
        let period: i32 = get_from_memory(base() + period_rva);
        if period == 0 {
            -1
        } else {
            (next_game_random() as i32).wrapping_rem(period)
        }
    }

    /// `ZTAnimal::initStatusVars` (`0x00453d63`): `ZTUnit::initStatusVars`, then for an animal type resets the animal's
    /// state: the needs from the type's start values, the flag bytes, the gender string (`"Female"` for types whose
    /// name starts with `f` or `p`, `"Male"` for `m`, otherwise one shared-RNG bit picks: clear is female) and the
    /// second string, the check slots (one shared-RNG draw modulo each global period, `-1` for a zero period, in the
    /// order of [`init_layout::PERIODS`]), the date stamps, the clamps from the type and the AI manager's countdown,
    /// the unit's self and type-block pointers, and finally `setHomeHabitat(0)`.
    pub fn init_status_vars(self) {
        ZTUnit(self.0).init_status_vars();
        let Some(entity_type) = self.animal_type() else {
            return;
        };
        let type_word = |offset: u32| get_from_memory::<u32>(entity_type + offset);
        let global_word = |rva: u32| get_from_memory::<u32>(base() + rva);
        let put = |offset: u32, value: u32| save_to_memory(self.0 + offset, value);
        let put_byte = |offset: u32, value: u8| save_to_memory(self.0 + offset, value);

        for offset in [0x298u32, 0x29c, 0x2a0, 0x2a4] {
            put(offset, 0);
        }
        put(animal_offsets::HAPPINESS, type_word(0x228));
        put(animal_offsets::HAPPINESS_PENDING, 0);
        put(animal_offsets::HUNGER, type_word(0x244));
        put(0x2b4, 0);
        put(animal_offsets::HEALTH, type_word(0x234));
        put(animal_offsets::HEALTH_PENDING, 0);
        put(0x2c0, type_word(0x24c));
        for offset in [0x2c4u32, 0x2c8, 0x2cc] {
            put(offset, 0);
        }
        put(animal_offsets::BREATH, type_word(0x208));
        for offset in [0x368u32, 0x36c, 0x370] {
            put(offset, 0);
        }
        put(0x37c, u32::MAX);
        for offset in [0x3a7u32, 0x3a8, 0x3a9, 0x3aa, 0x3ab, 0x3b0, 0x3b1] {
            put_byte(offset, 0);
        }
        put(0x378, 1);
        put_byte(0x390, 0);

        let name: u32 = type_word(0xa4);
        let initial = get_from_memory::<u8>(name);
        let female = match initial {
            b'm' => false,
            b'f' | b'p' => true,
            _ => next_game_random() & 1 == 0,
        };
        self.assign_text(0x26c, base() + if female { init_layout::FEMALE_RVA } else { init_layout::MALE_RVA });
        self.assign_text(0x278, base() + init_layout::SECOND_STRING_RVA);

        for offset in [0x2d0u32, 0x2d4, 0x2d8, 0x2dc, 0x2e0] {
            put(offset, 1);
        }
        for offset in [0x39cu32, 0x39d, 0x39e, 0x39f, 0x3a0, 0x3a1, 0x3a2, 0x3a3, 0x3a4, 0x3a5] {
            put_byte(offset, 0);
        }
        for offset in [0x2e4u32, 0x2e8, 0x2ec, 0x2f0] {
            put(offset, 0);
        }
        put_byte(0x3a6, 0);
        put(animal_offsets::ACTIVITY_MODE, 0);

        let mut date = [0u32; 2];
        let stamp = unsafe { GAMEMGR_GET_DATE.hooked()(globals().ztgamemgr_ptr() as *const u32, date.as_mut_ptr() as *const _) } as *const u32;
        if !stamp.is_null() {
            let (low, high): (u32, u32) = (get_from_memory(stamp as u32), get_from_memory(stamp as u32 + 4));
            put(0x288, low);
            put(0x28c, high);
            put(0x290, low);
            put(0x294, high);
        }

        for &(period_rva, offset) in init_layout::PERIODS {
            put(offset, Self::draw_period(period_rva) as u32);
        }
        put(0x32c, type_word(0x344).wrapping_sub(1));
        put(0x330, type_word(0x27c));
        put(0x348, global_word(load_layout::MAX_WORD_RVA).wrapping_sub(1));
        put(0x34c, type_word(load_layout::TYPE_COUNTDOWN_LIMIT).wrapping_sub(1));
        put(0x354, global_word(init_layout::LIMIT_354_RVA).wrapping_sub(1));
        put(0x358, 0);
        put(0x35c, 0);
        let aimgr = globals().ztaimgr_ptr() as u32;
        if aimgr != 0 {
            put(0x340, get_from_memory::<u32>(aimgr + conditionals::AIMGR_GATE));
        }
        put(0x64, self.0);
        put(0x68, if type_word(init_layout::BLOCK_GATE) == 0 { 0 } else { entity_type + init_layout::BLOCK });
        put_byte(animal_offsets::HAS_ESCAPE_INFO, 0);
        put(0x374, 0);
        put_byte(0x393, 0);
        put_byte(0x394, 0);
        put_byte(0x395, 0);
        put(0x398, 0);
        put_byte(0x392, 0);
        put(0x3ac, type_word(0x3e0));
        self.set_home_habitat(0);
    }

    /// The multi-animal panel's `std::set<ZTAnimal*>` object (`DAT_00639460`, `{head, size}`).
    fn multianimal_set() -> u32 {
        base() + multianimal::SET_RVA
    }

    /// Erases every node of the panel's set that holds this animal: `upper_bound`, `lower_bound`, then
    /// `erase(lower, upper)`, all through vanilla's own tree helpers (pool nodes stay with the vanilla allocator).
    fn erase_from_multianimal_set(self) {
        let set = Self::multianimal_set();
        let (mut upper, mut lower): (u32, u32) = (0, 0);
        let key = self.0;
        unsafe {
            TREE_UPPER_BOUND.original()(set as *const u32, &mut upper as *mut u32 as *const i32, &key as *const u32 as *const c_void);
            TREE_LOWER_BOUND.original()(set as *const u32, &mut lower as *mut u32 as *const i32, &key as *const u32);
            TREE_ERASE_RANGE.original()(set as *const u32, lower as i32, upper as i32);
        }
    }

    /// Inserts this animal into the panel's set (vanilla's `set::insert`).
    fn insert_into_multianimal_set(self) {
        let mut result = [0u32; 2];
        let key = self.0;
        unsafe { TREE_INSERT_UNIQUE.original()(Self::multianimal_set() as *const u32, result.as_mut_ptr() as *const i8, &key as *const u32 as *const c_void) };
    }

    /// `ZTUI::multianimal::removeEntity`: drops the animal's row from the panel's list box.
    fn remove_multianimal_row(self) {
        unsafe { MULTIANIMAL_REMOVE_ENTITY.original()(self.0 as i32) };
    }

    fn mark_habitat_dirty(self) {
        let habitat = call_slot_0(self.0, multianimal::SLOT_HABITAT);
        if habitat != 0 {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }.set_dirty_characteristics();
        }
    }

    /// `ZTAnimal::addToMap` (`0x00449c94`): an egg that is also flagged `+0x3a6` is refused. Otherwise `ZTUnit::addToMap`;
    /// on success the habitat is marked dirty, an animal that preys on man sets bit 3 of the tile's `+0x85`, an animal
    /// not excluded by `+0x142`/`+0x143`/`+0x3a6` joins the panel's set and list box, and the terrain mode is set from
    /// the tile (`1` when slot `+0xbc` holds and the tile's `+0x83 & 3` is set, `2` when slot `+0x16c` holds and its
    /// `+0x85` has bit 5, otherwise `0`; modes `1` and `2` only apply while `+0x3a7` is clear). Returns `ZTUnit`'s result.
    pub fn add_to_map(self) -> bool {
        if self.flag(0x395) && self.flag(animal_offsets::NOT_SELECTABLE) {
            return false;
        }
        let added = ZTUnit(self.0).add_to_map();
        if !added {
            return false;
        }
        self.mark_habitat_dirty();
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        if self.preys_on_man() {
            save_to_memory(tile + multianimal::TILE_FLAGS_B, get_from_memory::<u8>(tile + multianimal::TILE_FLAGS_B) | 8);
        }
        if get_from_memory::<u8>(self.0 + 0x143) == 0 && get_from_memory::<u8>(self.0 + 0x142) == 0 && !self.flag(animal_offsets::NOT_SELECTABLE) {
            self.insert_into_multianimal_set();
            unsafe { MULTIANIMAL_ADD_ENTITY.original()(self.0 as *const i32) };
        }
        if low_byte_bool(call_slot_0(self.0, multianimal::SLOT_ON_LAND)) && get_from_memory::<u8>(tile + multianimal::TILE_FLAGS_A) & 3 != 0 {
            if !self.flag(animal_offsets::FLAG_0X3A7) {
                self.set_terrain_mode(1, 0);
            }
        } else if low_byte_bool(call_slot_0(self.0, multianimal::SLOT_IN_WATER)) && get_from_memory::<u8>(tile + multianimal::TILE_FLAGS_B) & 0x20 != 0 {
            if !self.flag(animal_offsets::FLAG_0X3A7) {
                self.set_terrain_mode(2, 0);
            }
        } else {
            self.set_terrain_mode(0, 0);
        }
        true
    }

    /// `ZTAnimal::removeFromMap` (`0x004fe296`): an animal not excluded by `+0x142`/`+0x143` leaves the panel (its list
    /// box row, then its set nodes), and every node holding it is unlinked from the escaped-animal list and pushed on
    /// the 12-byte freelist. Without `flag` the habitat is marked dirty. Ends with `ZTUnit::removeFromMap`.
    pub fn remove_from_map(self, flag: bool) {
        if get_from_memory::<u8>(self.0 + 0x143) == 0 && get_from_memory::<u8>(self.0 + 0x142) == 0 {
            self.remove_multianimal_row();
            self.erase_from_multianimal_set();
            let sentinel: u32 = get_from_memory(base() + lifecycle::ESCAPE_LIST);
            let mut node: u32 = get_from_memory(sentinel);
            while node != sentinel {
                let next: u32 = get_from_memory(node);
                if get_from_memory::<u32>(node + 8) == self.0 {
                    let previous: u32 = get_from_memory(node + 4);
                    save_to_memory(previous, next);
                    save_to_memory(next + 4, previous);
                    crate::vanilla_list::free_node(node);
                }
                node = next;
            }
        }
        if !flag {
            self.mark_habitat_dirty();
        }
        ZTUnit(self.0).remove_from_map(flag);
    }

    /// `ZTAnimal::setDying` (`0x005a5606`): stores `flag` in `+0x3a6`, tells the animal-info panel, and a dying animal
    /// leaves the panel (row and set nodes). An egg that is dying gets a hatch countdown (`+0x398`) of `15`.
    pub fn set_dying(self, flag: bool) {
        save_to_memory(self.0 + animal_offsets::NOT_SELECTABLE, flag as u8);
        unsafe { ANIMALINFO_SET_ANIMAL_DYING.original()(self.0 as *const u32, flag as i8) };
        if self.flag(animal_offsets::NOT_SELECTABLE) {
            self.remove_multianimal_row();
            self.erase_from_multianimal_set();
            if self.flag(animal_offsets::NOT_SELECTABLE) && self.flag(0x395) {
                save_to_memory(self.0 + 0x398, 0xfu32);
            }
        }
    }

    /// Whether the panel's set holds this animal under its current ordering: the first node not less than it exists and
    /// is not greater.
    fn in_multianimal_set(self) -> bool {
        let set = Self::multianimal_set();
        let head: u32 = get_from_memory(set);
        let mut best = head;
        let mut node: u32 = get_from_memory(head + 4);
        while node != 0 {
            if ZTAnimal(get_from_memory::<u32>(node + 0x10)).compare_lt(self.0) {
                node = get_from_memory(node + 0xc);
            } else {
                best = node;
                node = get_from_memory(node + 8);
            }
        }
        best != head && !self.compare_lt(get_from_memory::<u32>(best + 0x10))
    }

    /// `ZTAnimal::setName` (`0x004e1d6b`): the set is ordered by name, so an animal in the panel's set is erased before
    /// `ZTScenery::setName` renames it and inserted again after, followed by `forceUpdate`.
    pub fn set_name(self, name: u32) {
        let listed = self.in_multianimal_set();
        if listed {
            self.erase_from_multianimal_set();
        }
        unsafe { ZTSCENERY_SET_NAME.original()(self.0 as *const c_void, name as *const u8) };
        if listed {
            self.insert_into_multianimal_set();
            unsafe { MULTIANIMAL_FORCE_UPDATE.original()() };
        }
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
            ZTUnit(self.0).set_is_removed(1);
        } else {
            self.hatch_egg();
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
                            ZTUnit(self.0).f_zoo_message_name(pipeline::MESSAGE_ID, 2, 0);
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
                            ZTUnit(self.0).f_zoo_message_name(hunger::MESSAGE_HUNGRY, 2, 0);
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
            self.f_eat();
        }
        self.add_pending(hunger::DIRT_PENDING, Self::type_field(ty, hunger::TYPE_DIRT_STEP));
        if self.read::<i32>(hunger::DIRT_COUNTER) >= Self::type_field::<i32>(ty, hunger::TYPE_DIRT_AT)
            && self.put_down_dirt(0)
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

    /// `ZTAnimal::setTerrainMode`; `updateStatusVariables` calls it with `(0, 0)` at minimum health.
    fn call_set_terrain_mode(self, mode: i32, flag: i8) {
        self.set_terrain_mode(mode, flag);
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
            self.do_sleep_check();
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
            if goal == bored::GOAL_CHASE_PLAYMATE && ZTUnit(self.0).in_friendly_chase() {
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

/// Offsets and behaviour-set names of the `BFBSCall`-driven checks (`doSleepCheck`, `doZapCheck`).
mod behaviour_checks {
    pub const SLEEP_COUNTDOWN: u32 = 0x300;
    /// `DAT_006393fc`: the sleep check's period.
    pub const SLEEP_PERIOD_RVA: u32 = 0x0023_93fc;
    pub const BYTE_SLEEPING: u32 = 0x3a8;
    pub const BYTE_ESCAPED: u32 = 0x3a3;
    pub const BYTE_ESCAPE_ARRIVED: u32 = 0x393;
    pub const BYTE_PLAYMATE: u32 = 0x3a4;
    pub const NEED: u32 = 0x2c0;
    pub const TYPE_SLEEP_AT: u32 = 0x324;
    pub const TYPE_WAKE_NEED: u32 = 0x24c;
    pub const REST_PENDING: u32 = 0x2c4;
    pub const REST_RATE: u32 = 0x2d8;
    pub const TYPE_REST_STEP: u32 = 0x320;
    pub const WOKE_DATE: u32 = 0x290;
    /// The embedded set's name string and the queued call's string, as `{begin, end}` word pairs.
    pub const SET_NAME_BEGIN: u32 = 0x54;
    pub const QUEUED_CALL_BEGIN: u32 = 0xdc;
    pub const SLEEP_SET: &[u8] = b"bSleep";

    pub const ZAP_COUNTDOWN: u32 = 0x31c;
    /// `DAT_00639520`: the zap check's period.
    pub const ZAP_PERIOD_RVA: u32 = 0x0023_9520;
    pub const SLOT_HAS_PRESSING_NEED: u32 = 0x288;
    pub const SLOT_STOP_EVERYTHING: u32 = 0x198;
    pub const FENCE_TEST_ARMED: u32 = 0x3ac;
    pub const TYPE_ZAP_CHANCE: u32 = 0x3e0;
    pub const TYPE_ZAP_HAPPINESS: u32 = 0x270;
    pub const TEST_FENCE_SET: &[u8] = b"bTestFence";
    pub const POSITION: u32 = 0x114;
    pub const NO_DIRECTION: u32 = 0xffff_ffff;
}

impl ZTAnimal {
    /// `ZTAnimal::doSleepCheck` (`0x004384ba`). While awake its countdown (`+0x300`) ticks; it reloads from
    /// `DAT_006393fc - 1` on firing (a sleeper always runs the body). The animal may sleep when `canSleepBeSickEat`
    /// holds and it is neither escaped without having arrived (`+0x3a3`/`+0x393`) nor after a playmate (`+0x3a4`). Awake
    /// and able, a need (`+0x2c0`) at the type's `+0x324` queues `bSleep` through `setQueuedCallNew`; when that queued it
    /// the animal is asleep (`+0x3a8`). Awake and unable or not needy, `+0x2c4` grows by the type's `+0x320` times
    /// `+0x2d8`. Asleep and unable, or asleep and already able: once neither the set's name nor the queued call is
    /// `bSleep` the need is restored from the type (`+0x24c`), `+0x393` and `+0x3a8` clear and the date is stamped at
    /// `+0x290`. The "Sleepy"/"Refreshed after sleeping" log strings are dropped.
    pub fn do_sleep_check(self) {
        use behaviour_checks as sleep;
        let Some(ty) = self.animal_type() else { return };
        let asleep = self.flag(sleep::BYTE_SLEEPING);
        if !asleep {
            let countdown: i32 = self.read(sleep::SLEEP_COUNTDOWN);
            save_to_memory(self.0 + sleep::SLEEP_COUNTDOWN, countdown.wrapping_sub(1));
            if countdown != 0 {
                return;
            }
        }
        let period: i32 = get_from_memory(base() + sleep::SLEEP_PERIOD_RVA);
        save_to_memory(self.0 + sleep::SLEEP_COUNTDOWN, period.wrapping_sub(1));
        let mut able = self.can_sleep_be_sick_eat();
        if able && ((self.flag(sleep::BYTE_ESCAPED) && !self.flag(sleep::BYTE_ESCAPE_ARRIVED)) || self.flag(sleep::BYTE_PLAYMATE)) {
            able = false;
        }
        if !asleep {
            if able && self.read::<i32>(sleep::NEED) >= Self::type_field::<i32>(ty, sleep::TYPE_SLEEP_AT) {
                let name = unsafe { new_owned_name(sleep::SLEEP_SET) };
                if unsafe { set_queued_call_new(self.0, name, 0, 0) } {
                    save_to_memory(self.0 + sleep::BYTE_SLEEPING, 1u8);
                }
                return;
            }
            let rest = Self::type_field::<i32>(ty, sleep::TYPE_REST_STEP).wrapping_mul(self.read::<i32>(sleep::REST_RATE));
            self.add_pending(sleep::REST_PENDING, rest);
            return;
        }
        let words_bytes = |offset: u32| -> Vec<u8> {
            let (begin, end): (u32, u32) = (self.read(offset), self.read(offset + 4));
            (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect()
        };
        if words_bytes(sleep::SET_NAME_BEGIN) == sleep::SLEEP_SET || words_bytes(sleep::QUEUED_CALL_BEGIN) == sleep::SLEEP_SET {
            return;
        }
        save_to_memory(self.0 + sleep::NEED, Self::type_field::<i32>(ty, sleep::TYPE_WAKE_NEED));
        save_to_memory(self.0 + sleep::BYTE_ESCAPE_ARRIVED, 0u8);
        save_to_memory(self.0 + sleep::BYTE_SLEEPING, 0u8);
        let mut date = [0u32; 2];
        let stamp = unsafe { GAMEMGR_GET_DATE.hooked()(globals().ztgamemgr_ptr() as *const u32, date.as_mut_ptr() as *const _) } as *const u32;
        if !stamp.is_null() {
            save_to_memory(self.0 + sleep::WOKE_DATE, get_from_memory::<u32>(stamp as u32));
            save_to_memory(self.0 + sleep::WOKE_DATE + 4, get_from_memory::<u32>(stamp as u32 + 4));
        }
    }

    /// `ZTAnimal::doZapCheck` (`0x00438ebc`): nothing for an egg, while doing a show (vtable `+0x22c`) or until its
    /// countdown (`+0x31c`) is spent; it reloads from `DAT_00639520 - 1`. Then a pressing need (vtable `+0x288`) or an
    /// unarmed fence test (`+0x3ac` zero) stops it, and a roll `rand() % 100` at or above the type's `+0x3e0` does too.
    /// `findAdjacentElectricFenceToAnimal` finds a direction toward a testable fence (`-1` stops); the animal stops everything
    /// (vtable `+0x198(1, 0, 1)`), turns that way and forces `bTestFence` through `setBSNew` (flag 0, force 1, clear mode
    /// 2). When that took, `dropTestFenceChance` runs and the type's `+0x270` is added to the pending happiness.
    pub fn do_zap_check(self) {
        use behaviour_checks as zap;
        let Some(ty) = self.animal_type() else { return };
        if self.flag(animal_offsets::EGG) || low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let countdown: i32 = self.read(zap::ZAP_COUNTDOWN);
        save_to_memory(self.0 + zap::ZAP_COUNTDOWN, countdown.wrapping_sub(1));
        if countdown > 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + zap::ZAP_PERIOD_RVA);
        save_to_memory(self.0 + zap::ZAP_COUNTDOWN, period.wrapping_sub(1));
        if low_byte_bool(call_slot_0(self.0, zap::SLOT_HAS_PRESSING_NEED)) || self.read::<u32>(zap::FENCE_TEST_ARMED) == 0 {
            return;
        }
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let chance = Self::type_field::<i32>(ty, zap::TYPE_ZAP_CHANCE);
        let roll = (next_game_random() % 100) as i32;
        if chance <= roll {
            return;
        }
        let direction = unsafe { ZTWORLDMGR_FIND_FENCE.hooked()(world as *const u32, self.0 as *const u32, (self.0 + zap::POSITION) as *const i32) };
        if direction == zap::NO_DIRECTION {
            return;
        }
        call_slot_3(self.0, zap::SLOT_STOP_EVERYTHING, 1, 0, 1);
        unsafe { BFENTITY_SET_DIRECTION.original()(self.0 as *const u32, direction) };
        let name = unsafe { new_owned_name(zap::TEST_FENCE_SET) };
        if unsafe { set_bs_new(self.0, name, 0, 1, 2) } {
            self.drop_test_fence_chance();
            self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, zap::TYPE_ZAP_HAPPINESS));
        }
    }
}

/// Offsets of `doBoxedCheck`.
mod boxed_check {
    pub const BYTE_BOXED: u32 = 0x394;
    pub const COUNTDOWN: u32 = 0x354;
    /// `DAT_00639448`: the period; `DAT_0063944c`: the rattle chance in percent.
    pub const PERIOD_RVA: u32 = 0x0023_9448;
    pub const RATTLE_CHANCE_RVA: u32 = 0x0023_944c;
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_TANK: u32 = 0x250;
    pub const SLOT_NOT_IN_WATER: u32 = 0x170;
    pub const SLOT_REMOVE_FROM_MAP: u32 = 0x78;
    pub const SLOT_ADD_TO_MAP: u32 = 0x74;
    /// The habitat byte that blocks releasing the animal; the tank's byte and water level.
    pub const HABITAT_BLOCK: u32 = 0x2c;
    pub const TANK_FILLED_BYTE: u32 = 0x198;
    pub const TANK_WATER_LEVEL: u32 = 0x188;
    pub const BYTE_ON_MAP: u32 = 0x211;
    pub const POSITION: u32 = 0x114;
    /// The map inside the world manager.
    pub const WORLD_MAP: u32 = 8;
    pub const RATTLE_SET: &[u8] = b"bRattle";
}

impl ZTAnimal {
    /// `ZTAnimal::doBoxedCheck` (`0x004377fc`): only for a boxed animal (`+0x394`), every `DAT_00639448` ticks
    /// (countdown `+0x354`). A habitat (vtable `+0x24c`) whose `+0x2c` byte is clear releases the animal (`unbox`) when it
    /// has no tank (vtable `+0x250(0)`) and vtable `+0x170` is false, or when the tank is filled (`+0x198`) with a
    /// positive water level (`+0x188`). Otherwise an animal not yet on the map (`+0x211`) that `BFMap::canAddEntity` accepts
    /// at its position is re-added through vtable `+0x78(0)`/`+0x74`. Then a roll under `DAT_0063944c` queues `bRattle`
    /// through `setQueuedCallNew` (flag 0, force 1). The leading entity-type cast has no effect and is dropped.
    pub fn do_boxed_check(self) {
        use boxed_check as boxed;
        if !self.flag(boxed::BYTE_BOXED) {
            return;
        }
        let countdown: i32 = self.read(boxed::COUNTDOWN);
        save_to_memory(self.0 + boxed::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + boxed::PERIOD_RVA);
        save_to_memory(self.0 + boxed::COUNTDOWN, period.wrapping_sub(1));
        let habitat = call_slot_0(self.0, boxed::SLOT_HABITAT);
        let tank = call_slot_1(self.0, boxed::SLOT_TANK, 0);
        if habitat != 0 && get_from_memory::<u8>(habitat + boxed::HABITAT_BLOCK) == 0 {
            let release = if tank == 0 {
                !low_byte_bool(call_slot_0(self.0, boxed::SLOT_NOT_IN_WATER))
            } else {
                get_from_memory::<u8>(tank + boxed::TANK_FILLED_BYTE) != 0 && get_from_memory::<i32>(tank + boxed::TANK_WATER_LEVEL) > 0
            };
            if release {
                self.unbox();
                return;
            }
        }
        if !self.flag(boxed::BYTE_ON_MAP) {
            let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
            if unsafe { BFMAP_CAN_ADD_ENTITY.original()((world + boxed::WORLD_MAP) as *const u32, self.0 as *const u32, (self.0 + boxed::POSITION) as *const i32, false, self.0 as *const u32) } {
                call_slot_1(self.0, boxed::SLOT_REMOVE_FROM_MAP, 0);
                save_to_memory(self.0 + boxed::BYTE_ON_MAP, 1u8);
                call_slot_0(self.0, boxed::SLOT_ADD_TO_MAP);
            }
        }
        let chance: i32 = get_from_memory(base() + boxed::RATTLE_CHANCE_RVA);
        if ((next_game_random() % 100) as i32) < chance {
            let name = unsafe { new_owned_name(boxed::RATTLE_SET) };
            unsafe { set_queued_call_new(self.0, name, 0, 1) };
        }
    }
}

/// Offsets and messages of `doHealthCheck`.
mod health_check {
    pub const COUNTDOWN: u32 = 0x2f8;
    /// `DAT_0063941c`: the period.
    pub const PERIOD_RVA: u32 = 0x0023_941c;
    /// Consecutive-ticks-at-zero-health counter and the `+0x3a7` "sick" byte.
    pub const ZERO_HEALTH_TICKS: u32 = 0x358;
    pub const BYTE_SICK: u32 = 0x3a7;
    pub const BYTE_ESCAPED: u32 = 0x3a3;
    pub const BYTE_PLAYMATE: u32 = 0x3a4;
    pub const HUNGER: u32 = 0x2b0;
    pub const BITS_SICK: u32 = 0x2;
    pub const BITS_OTHER_SICK: u32 = 0x4;
    pub const TYPE_SICK_HEALTH: u32 = 0x238;
    pub const TYPE_HUNGER_AT_ZERO: u32 = 0x240;
    pub const TYPE_TICKS_LIMIT: u32 = 0x330;
    pub const TYPE_PERCENT_FACTOR: u32 = 0x234;
    pub const TYPE_PERCENT_BASE: u32 = 0x23c;
    pub const TYPE_UNWELL_HAPPINESS: u32 = 0x258;
    pub const TYPE_CONTAGION_HAPPINESS: u32 = 0x25c;
    pub const TYPE_CONTAGION_CHANCE: u32 = 0x260;
    pub const TYPE_SICK_CHANCE: u32 = 0x264;
    pub const SLOT_REMOVED_MESSAGE: u32 = 0xac;
    pub const MESSAGE_VERY_SICK: u32 = 0x272d;
    pub const MESSAGE_GOT_SICK: u32 = 0x3a98;
    pub const MESSAGE_CONTAGION: u32 = 0x3a99;
    pub const MESSAGE_REMOVED: u32 = 0x271a;
    pub const SICK_SET: &[u8] = b"bSick";
}

impl ZTAnimal {
    /// `ZTAnimal::doHealthCheck` (`0x004389aa`), once per `DAT_0063941c` ticks (countdown `+0x2f8`). Clears bits `2` and `4` of
    /// the low and critical status words. Health `0` marks the animal sick (`+0x3a7`), restores the type's health (`+0x238`),
    /// sets terrain mode `(0, 0)` and counts consecutive such ticks (`+0x358`, reset otherwise); a count at the type's `+0x330`
    /// removes the animal (`fRemoveAnimal`), a sick animal below it posts "very sick" (`0x272d`). Not escaped, not after a
    /// playmate and at a health percentage under the type's limit, the animal queues `bSick` when it may be sick
    /// (`canSleepBeSickEat`), and when that queued, or it could not be asked, loses happiness (`+0x258`) and sets bit `2`.
    /// A non-egg then checks `fOtherAnimalSick`: if so it loses happiness (`+0x25c`) and sets bit `4`, and when not yet sick
    /// rolls `rand() % 100 < +0x260` then `rand() % 100 < 4` to get sick (message `0x3a99`); if not, it rolls
    /// `rand() % 100 < +0x264` then `== 0` to get sick (message `0x3a98`). Vtable `+0xac` true posts the "being removed"
    /// tile message (`0x271a`). The log strings are dropped.
    pub fn do_health_check(self) {
        use health_check as health;
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(health::COUNTDOWN);
        save_to_memory(self.0 + health::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let words = [animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL];
        self.update_flag_words(|v| v & !(health::BITS_SICK | health::BITS_OTHER_SICK), &words);
        let period: i32 = get_from_memory(base() + health::PERIOD_RVA);
        save_to_memory(self.0 + health::COUNTDOWN, period.wrapping_sub(1));
        let name_message = |id: u32| ZTUnit(self.0).f_zoo_message_name(id, 2, 0);
        let get_sick = |this: Self| {
            save_to_memory(this.0 + health::BYTE_SICK, 1u8);
            save_to_memory(this.0 + animal_offsets::HEALTH, Self::type_field::<i32>(ty, health::TYPE_SICK_HEALTH));
            this.set_terrain_mode(0, 0);
        };
        if self.health() == 0 {
            get_sick(self);
            if self.read::<i32>(health::HUNGER) == Self::type_field::<i32>(ty, health::TYPE_HUNGER_AT_ZERO) {
                let ticks: i32 = self.read(health::ZERO_HEALTH_TICKS);
                save_to_memory(self.0 + health::ZERO_HEALTH_TICKS, ticks.wrapping_add(1));
            } else {
                save_to_memory(self.0 + health::ZERO_HEALTH_TICKS, 0i32);
            }
        } else {
            save_to_memory(self.0 + health::ZERO_HEALTH_TICKS, 0i32);
        }
        if self.read::<i32>(health::ZERO_HEALTH_TICKS) >= Self::type_field::<i32>(ty, health::TYPE_TICKS_LIMIT) {
            self.f_remove_animal();
        } else if self.flag(health::BYTE_SICK) {
            name_message(health::MESSAGE_VERY_SICK);
        }
        if !self.flag(health::BYTE_ESCAPED)
            && !self.flag(health::BYTE_PLAYMATE)
            && self.health().wrapping_mul(100) <= Self::type_field::<i32>(ty, health::TYPE_PERCENT_BASE).wrapping_mul(Self::type_field::<i32>(ty, health::TYPE_PERCENT_FACTOR))
        {
            let queued = !self.can_sleep_be_sick_eat() || {
                let name = unsafe { new_owned_name(health::SICK_SET) };
                unsafe { set_queued_call_new(self.0, name, 0, 0) }
            };
            if queued {
                self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, health::TYPE_UNWELL_HAPPINESS));
                self.update_flag_words(|v| v | health::BITS_SICK, &words);
            }
        }
        if !self.flag(animal_offsets::EGG) {
            if self.f_other_animal_sick() {
                self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, health::TYPE_CONTAGION_HAPPINESS));
                self.update_flag_words(|v| v | health::BITS_OTHER_SICK, &words);
                if !self.flag(health::BYTE_SICK)
                    && Self::type_field::<i32>(ty, health::TYPE_CONTAGION_CHANCE) > (next_game_random() % 100) as i32
                    && ((next_game_random() % 100) as i32) < 4
                {
                    get_sick(self);
                    save_to_memory(self.0 + animal_offsets::HEALTH, Self::type_field::<i32>(ty, health::TYPE_SICK_HEALTH));
                    name_message(health::MESSAGE_CONTAGION);
                }
            } else if !self.flag(animal_offsets::EGG)
                && !self.flag(health::BYTE_SICK)
                && Self::type_field::<i32>(ty, health::TYPE_SICK_CHANCE) > (next_game_random() % 100) as i32
                && (next_game_random() % 100) < 1
            {
                get_sick(self);
                save_to_memory(self.0 + animal_offsets::HEALTH, Self::type_field::<i32>(ty, health::TYPE_SICK_HEALTH));
                name_message(health::MESSAGE_GOT_SICK);
            }
        }
        if low_byte_bool(call_slot_0(self.0, health::SLOT_REMOVED_MESSAGE)) {
            let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
            ZTUnit(self.0).f_zoo_message_tile(health::MESSAGE_REMOVED, 2, tile, 0, 1);
        }
    }
}

/// Offsets of `doAmbientAnims`.
mod ambient {
    pub const BYTE_DYING: u32 = 0x3a6;
    pub const BYTE_BOXED: u32 = 0x394;
    pub const BYTE_PLAYMATE: u32 = 0x3a4;
    pub const BYTE_FLEEING: u32 = 0x24c;
    pub const BYTE_ESCAPE_ARRIVED: u32 = 0x393;
    pub const BYTE_EGG: u32 = 0x395;
    pub const BYTE_ESCAPED: u32 = 0x3a3;
    pub const BYTE_RAMPAGE: u32 = 0x3b0;
    pub const BYTE_RAMPAGE_2: u32 = 0x3b1;
    pub const BYTE_SICK: u32 = 0x3a7;
    pub const BYTE_NO_ESCAPE_SET: u32 = 0x20c;
    pub const BYTE_HAS_PENDING_SET: u32 = 0x6d;
    pub const GOAL: u32 = 0x170;
    pub const GOALS_CHASE: [i32; 2] = [0x74, 0x75];
    pub const NEED: u32 = 0x2c0;
    pub const TYPE_SLEEP_AT: u32 = 0x324;
    pub const SLOT_SHOW_ACTIVE: u32 = 0x230;
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_ON_LAND: u32 = 0xbc;
    pub const SLOT_AT_EDGE: u32 = 0x16c;
    pub const TYPE_SLOT_FAILED: u32 = 0x14;
    pub const HABITAT_BLOCK: u32 = 0x2c;
    pub const TILE_FLAGS: u32 = 0x83;
    pub const TILE_FLAGS_2: u32 = 0x85;
    pub const TILE_WATERISH: u8 = 0x3;
    pub const TILE_EDGE: u8 = 0x20;
    pub const SET_NAME_BEGIN: u32 = 0x54;
    pub const QUEUED_CALL_BEGIN: u32 = 0xdc;
    pub const RESET_NAMES: [&[u8]; 3] = [b"bSleep", b"bSick", b"bHatch"];
    pub const HABITAT_RAMPAGE: &[u8] = b"bHabitatRampage";
    pub const ESCAPED_RAMPAGE: &[u8] = b"bEscapedRampage";
    pub const ESCAPED: &[u8] = b"bEscaped";
}

impl ZTAnimal {
    /// A string stored as `{begin, end}` words at `offset`, as bytes.
    fn string_bytes_at(self, offset: u32) -> Vec<u8> {
        let (begin, end): (u32, u32) = (self.read(offset), self.read(offset + 4));
        (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect()
    }

    /// `setBSNew(name, 0, 1, 2)` for a rampage set, calling the type's vtable `+0x14` when it fails.
    fn force_set_or_note_failure(self, ty: u32, name: &[u8]) {
        let words = unsafe { new_owned_name(name) };
        if !unsafe { set_bs_new(self.0, words, 0, 1, 2) } && ty != 0 {
            call_slot_0(ty, ambient::TYPE_SLOT_FAILED);
        }
    }

    /// `ZTAnimal::doAmbientAnims` (`0x00437102`): picks the animal's idle behaviour set when nothing else is going on.
    /// Returns at once for a dying, boxed, playmate-seeking, fleeing, escape-arrived or egg animal, for a goal of `0x74`/`0x75`
    /// outside a friendly chase, and while a show is on unless vtable `+0x230` holds. When the set's name or the queued call is
    /// `bSleep` or `bSick` (or the name `bHatch`), the rampage bytes (`+0x3b0`/`+0x3b1`) are cleared and nothing else happens.
    /// Otherwise, on an escaped animal at a dry non-edge tile, `bEscaped` (or, in a rampage, `bEscapedRampage`) is set or queued; on any
    /// other tile a rampage in a non-blocking habitat sets or queues `bHabitatRampage`. With no set pending (`+0x6d`) and no queued
    /// call, the animal sleeps when it may and needs to (`doSleepCheck`), else a rampage re-sets `bHabitatRampage`, else
    /// `ZTUnit::doAmbientAnims(0)` runs and the terrain mode becomes 1 (vtable `+0xbc` and a wet tile), 2 (vtable `+0x16c` and an edge
    /// tile) or 0, skipped when the animal is sick. The log strings are dropped; the failure slot's result is unused.
    pub fn do_ambient_anims(self) {
        use ambient as amb;
        if [amb::BYTE_DYING, amb::BYTE_BOXED, amb::BYTE_PLAYMATE, amb::BYTE_FLEEING, amb::BYTE_ESCAPE_ARRIVED, amb::BYTE_EGG].into_iter().any(|offset| self.flag(offset)) {
            return;
        }
        if low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) && !low_byte_bool(call_slot_0(self.0, amb::SLOT_SHOW_ACTIVE)) {
            return;
        }
        let goal: i32 = self.read(amb::GOAL);
        if amb::GOALS_CHASE.contains(&goal) && !ZTUnit(self.0).in_friendly_chase() {
            return;
        }
        let name = self.string_bytes_at(amb::SET_NAME_BEGIN);
        let queued = self.string_bytes_at(amb::QUEUED_CALL_BEGIN);
        if amb::RESET_NAMES[..2].iter().any(|reset| name == *reset || queued == *reset) || name == amb::RESET_NAMES[2] {
            save_to_memory(self.0 + amb::BYTE_RAMPAGE, 0u8);
            save_to_memory(self.0 + amb::BYTE_RAMPAGE_2, 0u8);
            return;
        }
        let ty = self.animal_type();
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        let tile_flag = |offset: u32, mask: u8| tile != 0 && get_from_memory::<u8>(tile + offset) & mask != 0;
        let habitat_blocks = |this: Self| {
            let habitat = call_slot_0(this.0, amb::SLOT_HABITAT);
            habitat != 0 && get_from_memory::<u8>(habitat + amb::HABITAT_BLOCK) != 0
        };
        let escaped_on_open_ground = self.flag(amb::BYTE_ESCAPED) && !tile_flag(amb::TILE_FLAGS, amb::TILE_WATERISH) && !tile_flag(amb::TILE_FLAGS_2, amb::TILE_EDGE);
        let ty_or_zero = ty.unwrap_or(0);
        if !escaped_on_open_ground {
            if self.flag(amb::BYTE_RAMPAGE) && !habitat_blocks(self) {
                if name == amb::HABITAT_RAMPAGE {
                    if queued != amb::HABITAT_RAMPAGE {
                        unsafe { set_pending_bs(self.0, new_owned_name(amb::HABITAT_RAMPAGE), 0, 1) };
                    }
                } else {
                    self.force_set_or_note_failure(ty_or_zero, amb::HABITAT_RAMPAGE);
                }
            }
        } else if !self.flag(amb::BYTE_RAMPAGE) {
            if !self.flag(amb::BYTE_NO_ESCAPE_SET) {
                let words = unsafe { new_owned_name(amb::ESCAPED) };
                unsafe { set_bs_new(self.0, words, 1, 1, 2) };
            }
        } else if name == amb::ESCAPED_RAMPAGE {
            if queued != amb::ESCAPED_RAMPAGE {
                unsafe { set_pending_bs(self.0, new_owned_name(amb::ESCAPED_RAMPAGE), 0, 1) };
            }
        } else if !self.flag(amb::BYTE_NO_ESCAPE_SET) {
            self.force_set_or_note_failure(ty_or_zero, amb::ESCAPED_RAMPAGE);
        }
        if self.flag(amb::BYTE_HAS_PENDING_SET) || !self.string_bytes_at(amb::QUEUED_CALL_BEGIN).is_empty() {
            return;
        }
        if self.can_sleep_be_sick_eat() && ty.is_some_and(|ty| self.read::<i32>(amb::NEED) >= Self::type_field::<i32>(ty, amb::TYPE_SLEEP_AT)) {
            self.do_sleep_check();
            return;
        }
        if self.flag(amb::BYTE_RAMPAGE) && !habitat_blocks(self) {
            self.force_set_or_note_failure(ty_or_zero, amb::HABITAT_RAMPAGE);
            return;
        }
        let no_ambient: *const u32 = std::ptr::null();
        let run_ambient = || ZTUnit(self.0).do_ambient_anims(no_ambient as u32);
        if low_byte_bool(call_slot_0(self.0, amb::SLOT_ON_LAND)) && tile_flag(amb::TILE_FLAGS, amb::TILE_WATERISH) {
            run_ambient();
            if !self.flag(amb::BYTE_SICK) {
                self.set_terrain_mode(1, 0);
            }
        } else if low_byte_bool(call_slot_0(self.0, amb::SLOT_AT_EDGE)) && tile_flag(amb::TILE_FLAGS_2, amb::TILE_EDGE) {
            run_ambient();
            if !self.flag(amb::BYTE_SICK) {
                self.set_terrain_mode(2, 0);
            }
        } else {
            run_ambient();
            self.set_terrain_mode(0, 0);
        }
    }
}

/// Offsets of `doBabyToAdultCheck`.
mod baby_to_adult {
    pub const COUNTDOWN: u32 = 0x32c;
    pub const TYPE_PERIOD: u32 = 0x344;
    pub const TYPE_NAME: u32 = 0xa4;
    /// The type's own name string (`{begin, end}` at `+0x98`).
    pub const TYPE_TEXT: u32 = 0x98;
    /// The animal's gender text (`{begin, ..}` at `+0x26c`); anything not starting with `M` selects the female set.
    pub const GENDER_TEXT: u32 = 0x26c;
    pub const ENTITY_TYPE: u32 = 0x128;
    pub const FIELD_3AC: u32 = 0x3ac;
    pub const TYPE_FIELD_3E0: u32 = 0x3e0;
    /// The global string `{begin, end}` pair (`0x638690`/`0x638694`) the first search string copies.
    pub const GLOBAL_STRING_RVA: u32 = 0x0023_8690;
    /// Static C strings: `DAT_00639020` (male search text) and `DAT_00639014` (female).
    pub const MALE_RVA: u32 = 0x0023_9020;
    pub const FEMALE_RVA: u32 = 0x0023_9014;
    pub const WORLD_VTABLE_FIND_TYPES: u32 = 0x14;
    pub const TYPE_SLOT_ADOPT: u32 = 0x10;
    pub const SLOT_UNKNOWN_110: u32 = 0x110;
    pub const SLOT_STOP_EVERYTHING: u32 = 0x198;
}

impl ZTAnimal {
    /// The C string at `address`, as bytes.
    fn c_string_bytes(address: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        while get_from_memory::<u8>(address + bytes.len() as u32) != 0 {
            bytes.push(get_from_memory::<u8>(address + bytes.len() as u32));
        }
        bytes
    }

    /// `ZTAnimal::doBabyToAdultCheck` (`0x00438663`): every `type[+0x344]` ticks (countdown `+0x32c`), an animal of a type whose
    /// name starts with `y` asks the world manager (`findTypes`, vtable `+0x14`) for the types matching the global string
    /// (`0x638690`/`0x638694`), the type's own text (`type+0x98`) and `DAT_00639020` (`DAT_00639014` unless the gender text starts
    /// with `M`). The first result becomes the animal's type (`+0x128`); its vtable `+0x10` runs, then the animal's `+0x110(0, 0)`
    /// and `+0x198(1, 1, 1)`, and when the new type is an animal type its `+0x3e0` is copied to `+0x3ac`. Vanilla copies the result
    /// vector into a local one before reading the first element; the port reads the first element of the returned vector and
    /// frees it. An empty result leaves the animal as it is.
    pub fn do_baby_to_adult_check(self) {
        use baby_to_adult as baby;
        let Some(ty) = self.animal_type() else { return };
        let countdown: i32 = self.read(baby::COUNTDOWN);
        save_to_memory(self.0 + baby::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        save_to_memory(self.0 + baby::COUNTDOWN, Self::type_field::<i32>(ty, baby::TYPE_PERIOD).wrapping_sub(1));
        if get_from_memory::<u8>(Self::type_field::<u32>(ty, baby::TYPE_NAME)) != b'y' {
            return;
        }
        let range_bytes = |begin: u32, end: u32| (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect::<Vec<u8>>();
        let global = base() + baby::GLOBAL_STRING_RVA;
        let search_class = VanillaString::from_bytes(&range_bytes(get_from_memory(global), get_from_memory(global + 4)));
        let search_name = VanillaString::from_bytes(&range_bytes(get_from_memory(ty + baby::TYPE_TEXT), get_from_memory(ty + baby::TYPE_TEXT + 4)));
        let male = get_from_memory::<u8>(self.read::<u32>(baby::GENDER_TEXT)) == b'M';
        let search_gender = VanillaString::from_bytes(&Self::c_string_bytes(base() + if male { baby::MALE_RVA } else { baby::FEMALE_RVA }));
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let find_types = unsafe {
            transmute::<u32, unsafe extern "thiscall" fn(u32, *mut [u32; 3], *const u32, *const u32, *const u32) -> *const [u32; 3]>(get_from_memory::<u32>(
                get_from_memory::<u32>(world) + baby::WORLD_VTABLE_FIND_TYPES,
            ))
        };
        let mut found = PoolVec::four_byte_elements();
        unsafe { find_types(world, found.out_pointer(), search_class.as_ptr(), search_name.as_ptr(), search_gender.as_ptr()) };
        if !found.is_empty() {
            let adult: u32 = get_from_memory(found.begin());
            save_to_memory(self.0 + baby::ENTITY_TYPE, adult);
            call_slot_0(adult, baby::TYPE_SLOT_ADOPT);
            call_slot_2(self.0, baby::SLOT_UNKNOWN_110, 0, 0);
            call_slot_3(self.0, baby::SLOT_STOP_EVERYTHING, 1, 1, 1);
            if adult != 0 && type_is_animal(adult) {
                save_to_memory(self.0 + baby::FIELD_3AC, Self::type_field::<u32>(adult, baby::TYPE_FIELD_3E0));
            }
        }
        drop(found);
    }
}

/// Offsets of `doReproduceCheck`.
mod reproduce_check {
    pub const COUNTDOWN: u32 = 0x328;
    /// `DAT_00639404`: the period; the death timer (`+0x34c`) drops by it each firing.
    pub const PERIOD_RVA: u32 = 0x0023_9404;
    pub const DEATH_TIMER: u32 = 0x34c;
    pub const BYTE_DYING: u32 = 0x3a6;
    /// Ticks since the last birth, against the type's cooldown.
    pub const TICKS: u32 = 0x330;
    pub const TYPE_TICKS_LIMIT: u32 = 0x27c;
    pub const TYPE_DEATH_CHANCE: u32 = 0x388;
    pub const TYPE_NAME: u32 = 0xa4;
    pub const MATE_ID: u32 = 0x2e8;
    pub const TYPE_MATE_MODE: u32 = 0x280;
    pub const MATE_WAIT: u32 = 0x348;
    /// `DAT_00639454`: the mate-search period.
    pub const MATE_PERIOD_RVA: u32 = 0x0023_9454;
    pub const TYPE_MATE_HAPPINESS: u32 = 0x380;
    pub const TYPE_HAPPINESS_LIMIT: u32 = 0x378;
    /// `DAT_0063c2e3`: gate for the unhappy-but-reproducing path.
    pub const UNHAPPY_GATE_RVA: u32 = 0x0023_c2e3;
    pub const BITS_LONELY: u32 = 0x8000;
    pub const BITS_MATING: u32 = 0x80;
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_HABITAT_BLOCKS: u32 = 0x20;
    pub const HABITAT_BLOCK_WORD: u32 = 0x4;
    pub const SLOT_TYPE_ID: u32 = 0x20;
    pub const SLOT_LOAD_BS_FUNCTIONS: u32 = 0x38;
    pub const TYPE_IDS_ALLOWING_UNHAPPY: [u32; 2] = [0x1398, 0x1399];
    pub const TILE_FLAGS: u32 = 0x83;
    pub const DIE_SET: &[u8] = b"bDie";
    pub const MESSAGE_BIRTH: u32 = 0x2730;
}

impl ZTAnimal {
    /// `ZTAnimal::doReproduceCheck` (`0x004385e5`). The cooldown tick counter (`+0x330`) climbs to the type's `+0x27c`
    /// every call. Every `DAT_00639404` ticks (countdown `+0x328`) the death timer (`+0x34c`) drops by the period; at or
    /// under zero for a not-yet-dying animal, a roll under the type's `+0x388` marks it dying (`+0x3a6`): if the type
    /// has a `bDie` set (`loadBSFunctions`) and the tile is not water-flagged it plays `bDie` (`setBS`, flag 0, force 1,
    /// clear mode 2), otherwise it is removed (`setIsRemoved(1)` and `triggerDeathArrived(habitat, 0)`).
    /// Then, for `m`/`f` types that are not dying, whose habitat does not block (vtable `+0x20` with `habitat+4`) and that are
    /// not doing a show: no mate (`+0x2e8` zero) looks for one (`fMateAvailable`) and with mate mode `1` sets bit `0x8000`;
    /// a gone mate runs the mate countdown (`+0x348`, `DAT_00639454`), adding the type's `+0x380` to happiness and bit
    /// `0x80` on its first tick and searching again when it reaches zero. A happy animal, or an unhappy one of a type whose
    /// id is `0x1398`/`0x1399` with `DAT_0063c2e3` set, that is male (`m`), has waited long enough, finds a mate and passes
    /// `fCheckReproduction` resets the tick counter, adds the habitat's baby-born bonus and posts message `0x2730` on its mate.
    /// The log strings are dropped.
    pub fn do_reproduce_check(self) {
        use reproduce_check as repro;
        let Some(ty) = self.animal_type() else { return };
        let ticks: i32 = self.read(repro::TICKS);
        let waited = ticks >= Self::type_field::<i32>(ty, repro::TYPE_TICKS_LIMIT);
        if !waited {
            save_to_memory(self.0 + repro::TICKS, ticks.wrapping_add(1));
        }
        let countdown: i32 = self.read(repro::COUNTDOWN);
        save_to_memory(self.0 + repro::COUNTDOWN, countdown.wrapping_sub(1));
        if countdown != 0 {
            return;
        }
        let period: i32 = get_from_memory(base() + repro::PERIOD_RVA);
        save_to_memory(self.0 + repro::COUNTDOWN, period.wrapping_sub(1));
        let timer = self.read::<i32>(repro::DEATH_TIMER).wrapping_sub(period);
        save_to_memory(self.0 + repro::DEATH_TIMER, timer);
        let habitat = call_slot_0(self.0, repro::SLOT_HABITAT);
        if timer <= 0 && !self.flag(repro::BYTE_DYING) {
            let roll = (next_game_random() % 100) as i32;
            if Self::type_field::<i32>(ty, repro::TYPE_DEATH_CHANCE) > roll {
                self.start_dying(ty, habitat);
            }
        }
        let first_letter = |ty: u32| get_from_memory::<u8>(Self::type_field::<u32>(ty, repro::TYPE_NAME));
        let letter = first_letter(ty);
        if (letter != b'm' && letter != b'f') || self.flag(repro::BYTE_DYING) {
            return;
        }
        if habitat != 0 && low_byte_bool(call_slot_0(habitat, repro::SLOT_HABITAT_BLOCKS)) && get_from_memory::<u32>(habitat + repro::HABITAT_BLOCK_WORD) != 0 {
            return;
        }
        if low_byte_bool(call_slot_0(self.0, pipeline::SLOT_IS_DOING_SHOW)) {
            return;
        }
        let mut found = false;
        let found_address = &mut found as *mut bool as *const bool;
        let mate_gone = self.f_mate_gone();
        let wait_period = || get_from_memory::<i32>(base() + repro::MATE_PERIOD_RVA);
        let mate_mode = Self::type_field::<i32>(ty, repro::TYPE_MATE_MODE);
        let mut skip_to_mating = false;
        if self.read::<u32>(repro::MATE_ID) == 0 {
            if mate_gone {
                skip_to_mating = true;
            } else if !self.f_mate_available(found_address) && mate_mode == 1 {
                self.update_flag_words(|v| v | repro::BITS_LONELY, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
            }
        } else if mate_gone {
            skip_to_mating = true;
        }
        if skip_to_mating {
            if mate_mode != 1 {
                self.f_mate_available(found_address);
                save_to_memory(self.0 + repro::MATE_WAIT, wait_period().wrapping_sub(1));
                return;
            }
            let wait: i32 = self.read(repro::MATE_WAIT);
            if wait == wait_period().wrapping_sub(1) {
                self.add_pending(animal_offsets::HAPPINESS_PENDING, Self::type_field(ty, repro::TYPE_MATE_HAPPINESS));
                self.update_flag_words(|v| v | repro::BITS_MATING, &[animal_offsets::STATUS_FLAGS, animal_offsets::STATUS_FLAGS_CRITICAL]);
            }
            save_to_memory(self.0 + repro::MATE_WAIT, wait.wrapping_sub(1));
            if wait == 0 {
                self.f_mate_available(found_address);
                save_to_memory(self.0 + repro::MATE_WAIT, wait_period().wrapping_sub(1));
            }
            return;
        }
        if self.happiness() < Self::type_field::<i32>(ty, repro::TYPE_HAPPINESS_LIMIT) {
            if get_from_memory::<u8>(base() + repro::UNHAPPY_GATE_RVA) == 0 {
                return;
            }
            let id = call_slot_0(ty, repro::SLOT_TYPE_ID);
            if !repro::TYPE_IDS_ALLOWING_UNHAPPY.contains(&id) {
                return;
            }
        }
        if first_letter(ty) == b'm' && waited && self.f_mate_available(found_address) && self.f_check_reproduction() {
            save_to_memory(self.0 + repro::TICKS, 0u32);
            let habitat = call_slot_0(self.0, repro::SLOT_HABITAT);
            if habitat != 0 {
                unsafe { ref_from_memory::<ZTHabitat>(habitat) }.add_baby_born_bonus(ty);
            }
            let mate = self.f_get_mate(found_address as u32);
            ZTUnit(mate).f_zoo_message_name(repro::MESSAGE_BIRTH, 4, 1);
        }
    }

    /// `doReproduceCheck`'s dying branch: marks the animal dying (`+0x3a6`) and plays `bDie` when the type has it and the
    /// tile is not water-flagged, otherwise removes the animal.
    fn start_dying(self, ty: u32, habitat: u32) {
        use reproduce_check as repro;
        save_to_memory(self.0 + repro::BYTE_DYING, 1u8);
        let name = VanillaString::from_bytes(repro::DIE_SET);
        let mut functions = PoolVec::four_byte_elements();
        let load = unsafe {
            transmute::<u32, unsafe extern "thiscall" fn(u32, *mut [u32; 3], *const u32) -> u8>(get_from_memory::<u32>(
                get_from_memory::<u32>(ty) + repro::SLOT_LOAD_BS_FUNCTIONS,
            ))
        };
        let loaded = unsafe { load(ty, functions.out_pointer(), name.as_ptr()) } != 0;
        let tile = unsafe { GET_TILE.original()(self.0 as *const u32) } as u32;
        let mut played = false;
        if loaded && tile != 0 && get_from_memory::<u8>(tile + repro::TILE_FLAGS) & 3 == 0 {
            let words = unsafe { new_owned_name(repro::DIE_SET) };
            played = unsafe {
                SET_BS.hooked()(self.0 as *const u32, words.begin as *const u32, words.end as *const u32, words.capacity as *const u32, 0, 1, 2)
            };
        }
        if !played {
            ZTUnit(self.0).set_is_removed(1);
            if habitat != 0 {
                unsafe { TRIGGER_DEATH_ARRIVED.hooked()(habitat as *const u32, 0) };
            }
        }
        drop(name);
        drop(functions);
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
                    ZTUnit(this as u32).set_is_removed(1);
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
                ZTUnit(this as u32).f_zoo_message_name(conditionals::MESSAGE_ID, 2, 0);
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
        if !unsafe { ZTANIMAL_IS_ON_GRASS.original()(self.0 as *const c_void) } {
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
        if ZTUnit(self.0).is_need_or_fun_satisfier(self.read::<u32>(building_use::UNIT_NEED_WORD), position.as_ptr()) {
            return;
        }
        let mut building: u32 = 0;
        let this = self.0 as *const u32;
        let own_tile = unsafe { GET_TILE.original()(this) };
        let found = ZTUnit(self.0).find_nearest_satisfier(own_tile as u32, need_name as u32, &mut building as *mut u32 as u32, building_use::SEARCH_RADIUS, building_use::SEARCH_FLAG);
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
        let [tile_x, tile_y, tile_z] = crate::util::position_in_tile(x, y, z);
        save_to_memory(self.0 + building_use::UNIT_POSITION, tile_x);
        save_to_memory(self.0 + building_use::UNIT_POSITION + 4, tile_y);
        save_to_memory(self.0 + building_use::UNIT_POSITION + 8, tile_z);
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
            ZTUnit(self.0).set_is_removed(1);
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

    /// `ZTAnimal::chaseDone` (`0x004a4d5f`): forwards to `ZTUnit::chaseDone` on the same object with the same argument.
    pub fn chase_done(self, unit: u32) -> bool {
        ZTUnit(self.0).chase_done(unit)
    }

    /// `ZTAnimal::consume` (`0x0050e648`): a guest that this type does not eat (`+0x3d5` clear) is left alone. Anything else
    /// handed in is consumed: a staff member first raises the "staff eaten" toast (message `0x2afa` over both units' name words,
    /// priority 2, tied to the animal), then the unit's slot `+0xcc(0)` runs and it is marked removed.
    pub fn consume(self, unit: u32) {
        if unit == 0 {
            return;
        }
        let unit_type = get_from_memory::<u32>(unit + unit_offsets::ENTITY_TYPE);
        let is_guest = unit_type != 0 && Self::type_is_cast(unit_type, faces_prey::GUEST_TAG);
        let own_type = self.read::<u32>(unit_offsets::ENTITY_TYPE);
        let own_type = if own_type != 0 && type_is_animal(own_type) { own_type } else { 0 };
        if is_guest && (own_type == 0 || Self::type_field::<u8>(own_type, alerts::TYPE_EATS_GUESTS) == 0) {
            return;
        }
        if unit_type != 0 && Self::type_is_cast(unit_type, faces_prey::STAFF_TAG) {
            let text = VanillaString::rvo_target();
            let build: unsafe extern "cdecl" fn(*const u32, *const u32, u32, u32, u32) -> *const u32 = unsafe { transmute(goal_flow::BUILD_STRING) };
            unsafe {
                build(
                    GET_APP.original()(),
                    text.as_ptr(),
                    alerts::STAFF_CONSUMED_MESSAGE,
                    get_from_memory::<u32>(unit + alerts::NAME_WORD),
                    self.read(alerts::NAME_WORD),
                )
            };
            let bfuimgr = base() + alerts::BFUIMGR_RVA;
            unsafe {
                BFUIMGR_DISPLAY_MESSAGE_1.hooked()(bfuimgr as *const u32, text.as_ptr() as *const i32, alerts::MESSAGE_PRIORITY, std::ptr::null(), self.0 as *const u32, true, false)
            };
        }
        call_slot_1(unit, alerts::SLOT_BEFORE_REMOVE, 0);
        ZTUnit(unit).set_is_removed(1);
    }

    /// `ZTAnimal::showEscapedAnimalAlert` (`0x0058cb88`): plays the shared exclaim animation on the dialog's icon element,
    /// and when the label element exists sets its caption to message `0x2717` formatted over this animal's name
    /// (`formatNameMessage`, then the element's slot `+0xc4`) and opens the confirm dialog for this animal.
    pub fn show_escaped_animal_alert(self) {
        let b = base();
        let half = |rva: u32| get_from_memory::<u16>(b + rva) as i32;
        let dialog = half(alerts::DIALOG_ID_RVA);
        let manager = (b + alerts::BFUIMGR_RVA) as *const u32;
        let icon = unsafe { BFUIMGR_GET_ELEMENT.hooked()(manager, half(alerts::ICON_OFFSET_RVA) + dialog) };
        let label = unsafe { BFUIMGR_GET_ELEMENT.hooked()(manager, half(alerts::LABEL_OFFSET_RVA) + dialog) };
        if !icon.is_null() {
            unsafe { UICONTROL_SET_ANIMATION.hooked()(icon, (b + alerts::EXCLAIM_ANIMATION_RVA) as *const i8, true) };
        }
        if label.is_null() {
            return;
        }
        let text = ZTUnit(self.0).format_name_message(alerts::ESCAPED_MESSAGE).release();
        let set_caption = unsafe {
            transmute::<u32, unsafe extern "thiscall" fn(u32, u32, u32, u32, u32)>(get_from_memory::<u32>(get_from_memory::<u32>(label as u32) + alerts::SLOT_SET_CAPTION))
        };
        unsafe {
            set_caption(label as u32, text[0], text[1], text[2], 0);
            BFUIMGR_CONFIRM_DIALOG.hooked()(manager, dialog, 0, 0, 1, self.0 as i32);
        }
    }

    /// `findTypes` (world vtable `+0x14`) over three search strings; the result vector is vanilla's (`[begin, end, cap]`).
    fn find_types_by(world: u32, class: *const u32, name: *const u32, subtype: *const u32) -> PoolVec {
        let find_types = unsafe {
            transmute::<u32, unsafe extern "thiscall" fn(u32, *mut [u32; 3], *const u32, *const u32, *const u32) -> *const [u32; 3]>(get_from_memory::<u32>(
                get_from_memory::<u32>(world) + baby_to_adult::WORLD_VTABLE_FIND_TYPES,
            ))
        };
        let mut found = PoolVec::four_byte_elements();
        unsafe { find_types(world, found.out_pointer(), class, name, subtype) };
        found
    }


    /// `ZTAnimal::resetAdultCheck` (the tail of the baby loop): the baby's growth countdown restarts from its type's period.
    fn reset_adult_check(self) {
        if let Some(ty) = self.animal_type() {
            save_to_memory(self.0 + baby_to_adult::COUNTDOWN, Self::type_field::<i32>(ty, baby_to_adult::TYPE_PERIOD).wrapping_sub(1));
        }
    }

    /// `ZTAnimal::fCheckReproduction` (`0x0043103b`, `fastcall`): rolls the type's birth percent (`+0x278`, the game RNG); the
    /// unhappy-reproduction gate (`DAT_0063c2e3`) clears and forces the birth for a type with id `0x1398`/`0x1399`. Then it
    /// looks the animal's type up through `findTypes` (class `"ZTAnimal"`, the type's own text, the empty text); with no result a
    /// coin flip (`rand() & 1`) retries with the female or male text. The habitat must exist and not have its `+0x2c` flag.
    /// Then `+0x284` babies are created from the first type found (an egg-laying type runs `findTypes` over its own class string
    /// and becomes an egg with `setEgg(1)`), each placed on a random clear tile of the habitat (`getRandomClearTile`, up to
    /// ten tries, `tileToWorld`, `setWorldPos`, `addEntity`, `validatePosition(1)`, `resetAdultCheck`) and recording its parent's
    /// id and mate. A failed creation answers false; ten failed placements delete that baby and end the loop. Answers whether
    /// the first baby was placed. The log strings are dropped.
    pub fn f_check_reproduction(self) -> bool {
        let Some(ty) = self.animal_type() else { return false };
        let world = ZTAnimal::world();
        if globals().ztaimgr_ptr() as u32 == 0 || world == 0 {
            return false;
        }
        let rolled = ((next_game_random() % 100) as i32) < Self::type_field::<i32>(ty, birth::TYPE_BIRTH_PERCENT);
        let mut proceed = rolled;
        let gate = base() + reproduce_check::UNHAPPY_GATE_RVA;
        if get_from_memory::<u8>(gate) != 0 {
            let id = call_slot_0(ty, reproduce_check::SLOT_TYPE_ID);
            if reproduce_check::TYPE_IDS_ALLOWING_UNHAPPY.contains(&id) {
                save_to_memory(gate, 0u8);
                proceed = true;
            }
        }
        if !proceed {
            return false;
        }
        let range_bytes = |begin: u32, end: u32| (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect::<Vec<u8>>();
        let global = base() + baby_to_adult::GLOBAL_STRING_RVA;
        let search_class = VanillaString::from_bytes(&range_bytes(get_from_memory(global), get_from_memory(global + 4)));
        let text = ty + baby_to_adult::TYPE_TEXT;
        let search_name = VanillaString::from_bytes(&range_bytes(get_from_memory(text), get_from_memory(text + 4)));
        let search_empty = VanillaString::from_bytes(&Self::c_string_bytes(base() + birth::EMPTY_SEARCH_RVA));
        let mut found = Self::find_types_by(world, search_class.as_ptr(), search_name.as_ptr(), search_empty.as_ptr());
        if found.is_empty() {
            let gendered = if unsafe { MSVC_RAND.hooked()() } & 1 == 0 { baby_to_adult::FEMALE_RVA } else { baby_to_adult::MALE_RVA };
            let search_gendered = VanillaString::from_bytes(&Self::c_string_bytes(base() + gendered));
            let second = Self::find_types_by(world, search_class.as_ptr(), search_name.as_ptr(), search_gendered.as_ptr());
            // Assigning drops (and frees) the first, empty result.
            found = second;
        }
        if found.is_empty() {
            return false;
        }
        let habitat = call_slot_0(self.0, reproduce_check::SLOT_HABITAT);
        if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            return false;
        }
        let first_type: u32 = get_from_memory(found.begin());
        let baby_type = if first_type != 0 && type_is_animal(first_type) { first_type } else { 0 };
        let litter = Self::type_field::<i32>(ty, birth::TYPE_LITTER_SIZE);
        if baby_type == 0 || litter <= 0 {
            return false;
        }
        let map = (world + 8) as *const u32;
        let mut first_placed = false;
        for index in 0..litter {
            let entity = if Self::type_field::<u8>(baby_type, birth::TYPE_LAYS_EGGS) != 0 {
                let class_string = call_slot_0(baby_type, birth::TYPE_SLOT_CLASS_STRING);
                let subtype = VanillaString::from_bytes(&Self::c_string_bytes(base() + birth::EMPTY_SEARCH_RVA));
                let egg_found = Self::find_types_by(world, class_string as *const u32, (baby_type + baby_to_adult::TYPE_TEXT) as *const u32, subtype.as_ptr());
                let egg_type = if !egg_found.is_empty() { get_from_memory::<u32>(egg_found.begin()) } else { baby_type };
                let created = call_slot_1(egg_type, birth::TYPE_SLOT_CREATE, 0);
                if created != 0 {
                    ZTAnimal(created).set_egg(true);
                }
                drop(egg_found);
                created
            } else {
                call_slot_1(baby_type, birth::TYPE_SLOT_CREATE, 0)
            };
            if entity == 0 {
                return false;
            }
            let entity_type: u32 = get_from_memory(entity + unit_offsets::ENTITY_TYPE);
            let baby = if entity_type != 0 && type_is_animal(entity_type) { ZTAnimal(entity) } else { ZTAnimal(0) };
            let mut placed = false;
            let mut attempts = 0;
            while attempts < birth::PLACEMENT_ATTEMPTS {
                attempts += 1;
                let tile = unsafe { ref_from_memory::<ZTHabitat>(habitat as *const u32) }.get_random_clear_tile_for_animal(baby.0, false, true);
                if tile == 0 {
                    continue;
                }
                let mut position = [0i32; 3];
                let extent = birth::EXTENT;
                unsafe {
                    BFMAP_TILE_TO_WORLD.hooked()(map, position.as_mut_ptr() as *const i32, (tile + birth::TILE_GRID_POS) as *const i32, extent.as_ptr());
                    BFENTITY_SET_WORLD_POS.original()(entity as *const u32, position.as_ptr() as *const IVec3);
                }
                let added = low_byte_bool(call_slot_1(world, birth::WORLD_SLOT_ADD_ENTITY, entity));
                call_slot_1(entity, birth::SLOT_VALIDATE_POSITION, 1);
                if baby.0 != 0 {
                    baby.reset_adult_check();
                }
                if added {
                    placed = true;
                    break;
                }
            }
            if !placed {
                call_slot_1(entity, birth::SLOT_DESTROY, 1);
                break;
            }
            if baby.0 != 0 {
                save_to_memory(baby.0 + birth::BABY_PARENT_ID, self.read::<u32>(birth::PARENT_ID));
                save_to_memory(baby.0 + birth::BABY_PARENT_MATE, self.read::<u32>(birth::PARENT_MATE));
            }
            if index == 0 {
                first_placed = true;
            }
        }
        first_placed
    }

    /// `ZTAnimal::hatchEgg` (`0x00613ca6`): forces behaviour set `bHatch` (`setBSNew`, call flag 0, force 1, clear mode 2).
    pub fn hatch_egg(self) {
        let name = unsafe { new_owned_name(eating::HATCH_SET) };
        unsafe { set_bs_new(self.0, name, 0, 1, 2) };
    }

    /// `ZTAnimal::finishHatch` (`0x00613d24`): the egg byte (`+0x395`) clears and the idle animation (slot `+0x8c`, argument 0)
    /// becomes the mixer's single link (`GXMixer::setBase` over the mixer at `+0x74`, clear flag 1). Then message `0x2852`
    /// goes out and the animal-info panel refreshes. The link and the one-entry vector the mixer copies from live in Rust
    /// memory; vanilla builds the vector in its pool and frees it again, which the mixer never sees.
    pub fn finish_hatch(self) {
        save_to_memory(self.0 + eating::BYTE_EGG, 0u8);
        self.rebase_mixer_on_idle_anim();
        unsafe {
            ZTUnit(self.0).f_zoo_message_name(eating::MESSAGE_HATCHED, 4, 1);
            ANIMALINFO_FORCE_UPDATE.original()(std::ptr::null());
        }
    }

    /// `finishHatch`'s mixer step: the mixer at `+0x74` ends up with exactly one link, on the animal's idle animation.
    pub fn rebase_mixer_on_idle_anim(self) {
        let idle = call_slot_1(self.0, eating::SLOT_IDLE_ANIM, 0);
        let mut link = [0u32; 3];
        let element = [0u32; 3];
        unsafe {
            GXMIXERLINK_CONSTRUCTOR.original()(link.as_mut_ptr() as *const c_void, idle as i32, 0, 0);
            GXMIXERLINK_COPY.original()(element.as_ptr() as *const c_void, link.as_ptr());
            let begin = element.as_ptr() as u32;
            let vector = [begin, begin + 12, begin + 12];
            GXMIXER_SET_BASE.original()((self.0 + eating::MIXER) as *const u32, vector.as_ptr() as *const i32, 1);
            GXMIXERLINK_DESTRUCTOR.original()(element.as_ptr() as *const c_void);
            GXMIXERLINK_DESTRUCTOR.original()(link.as_ptr() as *const c_void);
        }
    }

    /// `ZTAnimal::fEat` (`0x004fd951`): eats `fFoodUnitsActuallyEaten` units and answers that count. An escaped animal, none to
    /// eat or a tile with a blocking bit runs `stopEating(0)` instead. Otherwise it forces behaviour set `bEat` unless that is
    /// already the set (answering when that fails), clears the eat flag, starts eating (slot `+0x1d8(1, 0)` for a type with
    /// `+0x44c`, else the eating byte), clears bit 0 of four status words, reduces the food and stores the negated per-unit
    /// hunger cost in the hunger pending word. A type without `+0x44c` that still has food left queues `bEat` unless it is
    /// already the queued call or the animal cannot sleep, be sick and eat. The eating log strings are dropped.
    pub fn f_eat(self) -> u32 {
        let units = self.f_food_units_actually_eaten() as u32;
        let tile = (unsafe { GET_TILE.original()(self.0 as *const u32) }) as u32;
        if self.flag(eating::BYTE_ESCAPED) || units == 0 || tile_byte(tile, eating::TILE_BLOCK_BYTE) & eating::TILE_BLOCK_BITS != 0 {
            self.stop_eating(0);
            return units;
        }
        if self.string_bytes_at(eating::SET_NAME_BEGIN) != eating::EAT_SET {
            let name = unsafe { new_owned_name(eating::EAT_SET) };
            if !unsafe { set_bs(self.0, name, 0, 1, 2) } {
                return units;
            }
        }
        let ty = self.read::<u32>(unit_offsets::ENTITY_TYPE);
        if ty == 0 || !type_is_animal(ty) {
            return units;
        }
        save_to_memory(self.0 + eating::BYTE_EAT_FLAG, 0u8);
        let keep_moving = Self::type_field::<u8>(ty, type_offsets::KEEP_MOVING) != 0;
        if keep_moving {
            call_slot_2(self.0, eating::SLOT_START_EATING, 1, 0);
        } else {
            save_to_memory(self.0 + eating::BYTE_EATING, 1u8);
        }
        for offset in [eating::STATUS_LOW_A, eating::STATUS_LOW_B] {
            save_to_memory(self.0 + offset, self.read::<u32>(offset) & !1);
        }
        for offset in [eating::STATUS_HIGH_A, eating::STATUS_HIGH_B] {
            save_to_memory(self.0 + offset, self.read::<u32>(offset));
        }
        let left = self.f_reduce_food(units as i32);
        let cost = Self::type_field::<i32>(ty, eating::TYPE_HUNGER_COST).wrapping_mul(units as i32).wrapping_neg();
        save_to_memory(self.0 + eating::HUNGER_PENDING, cost);
        if left < 1 || keep_moving {
            return units;
        }
        if self.string_bytes_at(eating::QUEUED_CALL_BEGIN) == eating::EAT_SET || !self.can_sleep_be_sick_eat() {
            return units;
        }
        unsafe { set_pending_bs(self.0, new_owned_name(eating::EAT_SET), 0, 1) };
        units
    }

    /// `ZTAnimal::putDownDirt` (`0x004a8f89`): on a water tile whose habitat is a tank, lowers the tank's purity by the type's cost
    /// and answers `true`. On a tile free of the blocking bits it creates a dirt scenery (`getDirtType`, or `getDinoDirtType` for a
    /// dino-dirt type; mode `1`/`2` from the type's bytes) and, when the tile's habitat has its `+0x2c` flag, posts event `0x2730`
    /// through slot `0`. Answers whether the scenery was created. A null `tile` means the animal's own tile.
    pub fn put_down_dirt(self, tile: u32) -> bool {
        let world = ZTAnimal::world();
        if world == 0 {
            return false;
        }
        let tile = if tile == 0 { (unsafe { GET_TILE.original()(self.0 as *const u32) }) as u32 } else { tile };
        if tile == 0 {
            return false;
        }
        let ty = self.read::<u32>(unit_offsets::ENTITY_TYPE);
        if tile_byte(tile, dirt::TILE_WATER_BYTE) & dirt::TILE_WATER_BIT != 0 {
            let tank = call_slot_0(self.0, dirt::SLOT_HABITAT);
            if tank != 0 && low_byte_bool(call_slot_0(tank, dirt::SLOT_IS_TANK)) {
                let purity = get_from_memory::<i32>(tank + dirt::TANK_PURITY).wrapping_sub(Self::type_field::<i32>(ty, dirt::TYPE_PURITY_COST));
                unsafe { ref_from_memory::<ZTTankExhibit>(tank as *const u32) }.set_water_purity(purity);
                return true;
            }
        }
        if tile_byte(tile, dirt::TILE_BLOCK_BYTE) & dirt::TILE_BLOCK_BITS != 0 {
            return false;
        }
        let dirt_type = if Self::type_field::<u8>(ty, dirt::TYPE_DINO_DIRT) != 0 {
            unsafe { GET_DINO_DIRT_TYPE.original()() }
        } else {
            unsafe { GET_DIRT_TYPE.original()() }
        };
        let mut mode = (Self::type_field::<u8>(ty, dirt::TYPE_MODE_ONE) != 0) as i32;
        if Self::type_field::<u8>(ty, dirt::TYPE_MODE_TWO) != 0 {
            mode = 2;
        }
        let [ex, ey, ez] = dirt::EXTENT;
        let scenery = unsafe {
            ZTWORLDMGR_CREATE_SCENERY.original()(world as *const u32, dirt_type as *const u32, mode, tile as *const u32, IVec3::new(ex, ey, ez))
        } as u32;
        if scenery == 0 {
            return false;
        }
        let manager = globals().zthabitatmgr_ptr() as u32;
        let table: u32 = get_from_memory(manager + conditionals::HABITAT_MGR_TABLE);
        let x: u32 = get_from_memory(tile + conditionals::TILE_X);
        let y: u32 = get_from_memory(tile + conditionals::TILE_Y);
        let column: u32 = get_from_memory(table.wrapping_add(x.wrapping_mul(conditionals::COLUMN_STRIDE)));
        let cell: u32 = get_from_memory(column.wrapping_add(y.wrapping_mul(conditionals::CELL_STRIDE)));
        if cell != 0 && tile_byte(cell, conditionals::HABITAT_FLAG) != 0 {
            let send = unsafe {
                transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32)>(get_from_memory::<u32>(get_from_memory::<u32>(self.0)))
            };
            send(self.0, dirt::EVENT_ID, 0, dirt::EVENT_CATEGORY, tile, 0, 1);
        }
        true
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
        unsafe { MIXER_CHAIN_DEQUE_CLEAR.original()((self.0 + lifecycle::BLOCK_8C) as *const u32) };
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

/// Fields `fCheckReproduction` touches.
mod birth {
    /// Type fields: the birth percent (`+0x278`), the babies per birth (`+0x284`) and the egg-type byte (`+0x404`).
    pub const TYPE_BIRTH_PERCENT: u32 = 0x278;
    pub const TYPE_LITTER_SIZE: u32 = 0x284;
    pub const TYPE_LAYS_EGGS: u32 = 0x404;
    /// `DAT_0063901c` (the empty search text), as an RVA.
    pub const EMPTY_SEARCH_RVA: u32 = 0x0023_901c;
    /// Type slot `+0x14` answers the type's class string; slot `+0x24(0)` creates an entity of the type.
    pub const TYPE_SLOT_CLASS_STRING: u32 = 0x14;
    pub const TYPE_SLOT_CREATE: u32 = 0x24;
    /// Entity slots: the destructor (`+0x18`, delete flag 1) and `validatePosition` (`+0x84`, 1); the world's `addEntity` (`+0x18`).
    pub const SLOT_DESTROY: u32 = 0x18;
    pub const SLOT_VALIDATE_POSITION: u32 = 0x84;
    pub const WORLD_SLOT_ADD_ENTITY: u32 = 0x18;
    /// Tries to find a clear tile for each baby.
    pub const PLACEMENT_ATTEMPTS: u32 = 10;
    /// The tile's grid position (`+0x34`, `+0x38`) and the placement extent `{0x20, 0x20, 0}`.
    pub const TILE_GRID_POS: u32 = 0x34;
    pub const EXTENT: [i32; 3] = [0x20, 0x20, 0];
    /// The baby records its parent: the parent's id (`+0x124`) at `+0x2f0` and the parent's mate id (`+0x2e8`) at `+0x2ec`.
    pub const PARENT_ID: u32 = 0x124;
    pub const PARENT_MATE: u32 = 0x2e8;
    pub const BABY_PARENT_ID: u32 = 0x2f0;
    pub const BABY_PARENT_MATE: u32 = 0x2ec;
}

/// Fields `fEat`, `hatchEgg` and `finishHatch` touch.
mod eating {
    pub const EAT_SET: &[u8] = b"bEat";
    pub const HATCH_SET: &[u8] = b"bHatch";
    /// Begin of the embedded behaviour set's name (`+0x54`) and of the queued call string (`+0xdc`).
    pub const SET_NAME_BEGIN: u32 = 0x54;
    pub const QUEUED_CALL_BEGIN: u32 = 0xdc;
    /// Bytes: escaped-and-not-arrived (`+0x3a3`), the eat flag cleared on entry (`+0x39e`), the eating byte (`+0x3a9`) and
    /// the egg byte `finishHatch` clears (`+0x395`).
    pub const BYTE_ESCAPED: u32 = 0x3a3;
    pub const BYTE_EAT_FLAG: u32 = 0x39e;
    pub const BYTE_EATING: u32 = 0x3a9;
    pub const BYTE_EGG: u32 = 0x395;
    /// Tile blocking bits (`+0x83`), type fields: the keep-moving byte (`type_offsets::KEEP_MOVING`, `cKeepMoving`) and the per-unit hunger cost (`+0x218`).
    pub const TILE_BLOCK_BYTE: u32 = 0x83;
    pub const TILE_BLOCK_BITS: u8 = 0x3;
    pub const TYPE_HUNGER_COST: u32 = 0x218;
    /// Slot `+0x1d8(1, 0)` when the type has the byte set, status words cleared of bit 0, the hunger pending word.
    pub const SLOT_START_EATING: u32 = 0x1d8;
    pub const STATUS_LOW_A: u32 = 0x298;
    pub const STATUS_LOW_B: u32 = 0x2a0;
    pub const STATUS_HIGH_A: u32 = 0x29c;
    pub const STATUS_HIGH_B: u32 = 0x2a4;
    pub const HUNGER_PENDING: u32 = 0x2b4;
    /// The mixer `finishHatch` rebases (`+0x74`), the idle-animation slot (`+0x8c`) and the hatch message.
    pub const MIXER: u32 = 0x74;
    pub const SLOT_IDLE_ANIM: u32 = 0x8c;
    pub const MESSAGE_HATCHED: u32 = 0x2852;
}

/// Fields `consume` and `showEscapedAnimalAlert` touch.
mod alerts {
    /// `GLOBAL_BFUIMgr` (the object itself).
    pub const BFUIMGR_RVA: u32 = 0x0023_8de0;
    /// `BFApp::buildString` id of the staff-consumed message.
    pub const STAFF_CONSUMED_MESSAGE: u32 = 0x2afa;
    pub const MESSAGE_PRIORITY: i32 = 2;
    /// Type byte (`+0x3d5`) that lets an animal eat a guest; the unit's `+0x108` is its name word.
    pub const TYPE_EATS_GUESTS: u32 = 0x3d5;
    pub const NAME_WORD: u32 = 0x108;
    /// The unit's slot `+0xcc` (called with 0) before it is removed.
    pub const SLOT_BEFORE_REMOVE: u32 = 0xcc;
    /// `DAT_0063b94e` (two `u16`s), the second `u16` of `DAT_0063b942` and the animation path literal, as RVAs. The dialog
    /// id is the `u16` at `DAT_0063b94e + 2`; the icon element is `u16(0x63b94e) + that`, the label element
    /// `u16(0x63b944) + that`.
    pub const DIALOG_ID_RVA: u32 = 0x0023_b950;
    pub const ICON_OFFSET_RVA: u32 = 0x0023_b94e;
    pub const LABEL_OFFSET_RVA: u32 = 0x0023_b944;
    pub const EXCLAIM_ANIMATION_RVA: u32 = 0x0023_cc18;
    pub const ESCAPED_MESSAGE: u32 = 0x2717;
    /// The label element's slot `+0xc4(text by value, 0)` sets its caption and takes the string.
    pub const SLOT_SET_CAPTION: u32 = 0xc4;
}

/// Fields `putDownDirt` touches.
mod dirt {
    /// Tile flag bytes: `+0x85` bit `0x20` marks water, `+0x83` bits `0x3` block placing dirt.
    pub const TILE_WATER_BYTE: u32 = 0x85;
    pub const TILE_WATER_BIT: u8 = 0x20;
    pub const TILE_BLOCK_BYTE: u32 = 0x83;
    pub const TILE_BLOCK_BITS: u8 = 0x3;
    /// `ZTAnimal` vtable `+0x24c` answers the habitat (a `ZTTankExhibit` on a water tile); the habitat's `+0x20` slot
    /// answers whether it is a tank, and `+0x1a8` is its water purity.
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_IS_TANK: u32 = 0x20;
    pub const TANK_PURITY: u32 = 0x1a8;
    /// Animal type fields: purity cost (`+0x448`) and the three dirt-kind bytes.
    pub const TYPE_PURITY_COST: u32 = 0x448;
    pub const TYPE_DINO_DIRT: u32 = 0x3cf;
    pub const TYPE_MODE_ONE: u32 = 0x3ce;
    pub const TYPE_MODE_TWO: u32 = 0x3d0;
    /// The scenery's world-position extent by value: `{0x20, 0x20, 0}`.
    pub const EXTENT: [i32; 3] = [0x20, 0x20, 0];
    /// Event posted through vtable slot `0` when the new dirt lands in a flagged habitat: id, category, tile.
    pub const EVENT_ID: u32 = 0x2730;
    pub const EVENT_CATEGORY: u32 = 0x4d;
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
        // The old buffer goes back to the pool once its entries are copied.
        drop(PoolVec::adopt_sized([begin, end, capacity], faces_prey::ENTRY_SIZE));
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
        let mut adults = PoolVec::four_byte_elements();
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_adult_gender_species_animals(gender_header.as_ptr() as u32, species, adults.out_pointer() as u32);
        let own_id: u32 = self.read(faces_prey::ENTITY_ID);
        let own_avoid = [self.read::<u32>(mating::AVOID_ID_B), self.read::<u32>(mating::AVOID_ID_A)];
        let mut candidates: Vec<u32> = Vec::new();
        let mut cursor = adults.begin();
        while cursor != adults.end() {
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
        drop(adults);
        result
    }
}

/// Fields, slots and addresses the movement, pathing and escape methods use.
mod movement {
    /// Escape-info record (`+0x384`): byte `+1` is set once the animal reaches the escape tile, byte `+3` once the
    /// fence has been broken, `+4`/`+8` are the two tiles the escape crosses between and `+0xc` is the kind
    /// (`1` jump, `2` climb, `3` fence, `4` exit tank; `0` none).
    pub const REC_REACHED: u32 = 0x1;
    pub const REC_BROKEN: u32 = 0x3;
    pub const REC_FROM_TILE: u32 = 0x4;
    pub const REC_TO_TILE: u32 = 0x8;
    pub const REC_KIND: u32 = 0xc;
    /// The four edge-fence pointers of a tile, indexed by `direction / 2`.
    pub const TILE_EDGES: u32 = 0x14;
    /// Tile bytes: the cell kind, the terrain flags (water in the low two bits, `0x8` raised) and the second flag byte
    /// (`0x20` marks land cut off from water, `0x8` the predator flag).
    pub const TILE_KIND: u32 = 0x81;
    pub const TILE_TERRAIN: u32 = 0x83;
    pub const TILE_FLAGS: u32 = 0x85;
    /// Tile kinds the cost and filter code treat as one group (`'A'`, `'P'`, `0x14`, `5`).
    pub const TILE_KIND_GROUP: [u8; 4] = [0x41, 0x50, 0x14, 0x05];
    /// `CAST_ZTFence` as an RVA.
    pub const FENCE_TAG_RVA: u32 = 0x0023_8660;
    /// Type field: the minimum strength (`> 0`) an animal needs to break a fence on its way out.
    pub const TYPE_FENCE_STRENGTH: u32 = 0x3b4;
    /// `DAT_00635494`: the path-cost sentinel for an impassable tile.
    pub const MAX_COST_RVA: u32 = 0x0023_5494;
    /// Unit fields: the goal id (`BFCogGoal` at `+0x15c`, its id word at `+0x170`), the goal's target id, and the
    /// signed counter `getTerrainCost` compares with `4`.
    pub const UNIT_GOAL_ID: u32 = 0x170;
    pub const UNIT_GOAL_TARGET: u32 = 0x178;
    pub const UNIT_PATH_COUNTER: u32 = 0x150;
    pub const UNIT_GOAL_BLOCK: u32 = 0x15c;
    pub const UNIT_TARGET_FLAG: u32 = 0x3b0;
    /// The animal's "pending flag" byte `stopEverything` clears.
    pub const UNIT_STOP_FLAG: u32 = 0x3a8;
    /// Goal ids: a show goal (`0x85`/`0x86`), the chase-a-target goal and the goal `targetBuilding` sets.
    pub const GOAL_SHOW: [i32; 2] = [0x85, 0x86];
    pub const GOAL_FOLLOW: i32 = 0x74;
    pub const GOAL_BUILDING: u32 = 0x7b;
    /// Vtable slots.
    pub const SLOT_IS_WALKABLE: u32 = 0xb4;
    pub const SLOT_GET_SWIMS: u32 = 0xbc;
    pub const SLOT_FORCE_ANIM: u32 = 0x104;
    pub const SLOT_GET_UNDERWATER: u32 = 0x16c;
    pub const SLOT_GET_ONLY_UNDERWATER: u32 = 0x170;
    pub const SLOT_SET_DEST_TILE: u32 = 0x1a8;
    pub const SLOT_SET_GOAL: u32 = 0x1d8;
    pub const SLOT_HABITAT_FROM_TILE: u32 = 0x248;
    pub const SLOT_SET_BUILDING_TILE: u32 = 0x2e4;
    /// `DAT_006394f8`: the prey search radius in tiles.
    pub const PREY_RADIUS_RVA: u32 = 0x0023_94f8;
    /// `DAT_006386b0` is `tags::UNIT`; goals `0x68`-`0x6a` are the building-use goals `findPrey` skips for a unit without the target flag.
    pub const GOAL_BUILDING_USE: [i32; 3] = [0x68, 0x69, 0x6a];
    pub const SLOT_GET_PREY_UNIT: u32 = 0x25c;
    pub const SLOT_CHECK_PREY: u32 = 0x300;
    /// World map (`BFMap` at `GLOBAL_ZTWorldMgr + 8`): tile width and height, and the tile array, whose tiles are `0x8c` bytes.
    pub const MAP_X_SIZE: u32 = 0x2c;
    pub const MAP_Y_SIZE: u32 = 0x30;
    pub const MAP_TILES: u32 = 0x38;
    pub const TILE_SIZE: u32 = 0x8c;
    /// Tile position words (x, y).
    pub const TILE_X: u32 = 0x34;
    pub const TILE_Y: u32 = 0x38;
    /// `BFTile::hasUnit(RECT)` (`0x00427360`, `bool thiscall(tile, RECT)`, `RET 0x10`: four dwords by value, answers `AL`): whether an
    /// entity on the tile has a rectangle intersecting the argument (an empty rect asks whether the tile holds any entity). Called by
    /// address: the `bftile::HAS_UNIT` entry declares the RECT as a single `u32` (`RET 4`).
    pub const TILE_HAS_UNIT: u32 = 0x0042_7360;
    /// `fencesOK` fence fields. Fence entity: strength (`+0x15c`), the open-state word (`+0x160`, zero means "not
    /// built up") and virtual `+0x144` (a show-goal gate predicate). Fence type: `+0x190` blocks everything, `+0x191`
    /// passes everything, `+0x193` marks a type that may be a tank wall, `+0x195`/`+0x196` are the jump/climb
    /// permissions the animal type's `+0x3cd`/`+0x3cc` flags match, `+0x64` the height compared with the animal type's
    /// `+0x3dc`; `+0xa4`/`+0xa8` is the type-name string.
    pub const FENCE_STRENGTH: u32 = 0x15c;
    pub const FENCE_OPEN_WORD: u32 = 0x160;
    pub const SLOT_FENCE_SHOW_GATE: u32 = 0x144;
    pub const FTYPE_BLOCKS: u32 = 0x190;
    pub const FTYPE_PASSES: u32 = 0x191;
    pub const FTYPE_MAY_BE_TANK_WALL: u32 = 0x193;
    pub const FTYPE_JUMPABLE: u32 = 0x195;
    pub const FTYPE_CLIMBABLE: u32 = 0x196;
    pub const FTYPE_HEIGHT: u32 = 0x64;
    pub const FTYPE_NAME_BEGIN: u32 = 0xa4;
    pub const FTYPE_NAME_END: u32 = 0xa8;
    /// The cast tag (`DAT_00638720`) that marks a fence type as a tank wall, and the tank-wall entity bytes `fencesOK`
    /// reads (`+0x466` blocks, `+0x464` lets an animal that passes the type checks through).
    pub const TANK_WALL_TAG_RVA: u32 = 0x0023_8720;
    pub const TANK_WALL_BLOCKS: u32 = 0x466;
    pub const TANK_WALL_OPEN: u32 = 0x464;
    /// Animal type: `+0x3cc`/`+0x3cd` climb/jump flags, `+0x3dc` height limit; virtual `+0xcc` is the tank-wall
    /// exemption predicate.
    pub const ATYPE_CLIMBS: u32 = 0x3cc;
    pub const ATYPE_JUMPS: u32 = 0x3cd;
    pub const ATYPE_HEIGHT_LIMIT: u32 = 0x3dc;
    pub const SLOT_ANIMAL_TYPE_TANK_EXEMPT: u32 = 0xcc;
    /// `pickRandomDest`: the habitat's per-species escapability tree (`+0x16c`, 16-byte entries keyed by entity type),
    /// the unit's "escape in progress" byte (`+0x395`), pending direction (`+0x37c`), the unit virtual that answers true
    /// while a show runs (`+0x22c`), the habitat virtual `+0x20` (land-roaming predicate) and the unit virtual `+0x1f8`
    /// every destination goes through.
    pub const HABITAT_ESCAPE_TREE: u32 = 0x16c;
    pub const UNIT_ESCAPE_PENDING: u32 = 0x395;
    pub const UNIT_PENDING_DIRECTION: u32 = 0x37c;
    pub const SLOT_IS_SHOWING: u32 = 0x22c;
    pub const SLOT_HABITAT_LAND_ROAMER: u32 = 0x20;
    pub const SLOT_SET_DESTINATION: u32 = 0x1f8;
    pub const ESCAPE_ENTRY_SIZE: u32 = 0x10;
    /// `getFeedingPath`: the unit's walk-mode word (`+0x1bc`, `3` is passed to `BFMap::validate`), its path list (`+0x154`) and the
    /// virtual (`+0x180`) that asks for a trailing step.
    pub const UNIT_WALK_MODE: u32 = 0x1bc;
    pub const WALK_MODE_FLAG: i32 = 3;
    pub const UNIT_PATH_LIST: u32 = 0x154;
    pub const SLOT_FEEDING_TAIL: u32 = 0x180;
    pub const SHORTCUT_PERCENT: i32 = 0x32;
    pub const POSITION_BYTES: u32 = 12;
    /// Behaviour names `forceAnim` takes for each escape kind.
    pub const ANIM_JUMP: &str = "jump_high";
    pub const ANIM_CLIMB: &str = "climb_up";
    pub const ANIM_EXIT_TANK: &str = "exit_tank";
    pub const ANIM_RUN: &str = "run";
    pub const ANIM_WALK: &str = "bWalk";
}

/// `evaluateEscape`: the AI manager's path-search object (`ZTAIMgr + 0x10`: map pointer `+8`, map height `+0xc`, tile
/// count `+0x10`, search mode `+0x4c`, result path list `+0x70`), the unit's "searching" byte and the virtuals it calls.
/// The unit virtual `+0x1ac` (`diagonalOK`) answers the search mode (`AL`); `+0x1a0` (`setGridDest`) takes the final
/// destination position. The habitat virtual `+0x20` is also the land-roaming predicate `pickRandomDest` uses; here a true answer from the animal's own
/// habitat makes the escape kind `4`, and the call on a flagged destination habitat is followed by a read of its
/// `+0x188` word, which is added to the destination's height.
mod escape_eval {
    pub const PATHER: u32 = 0x10;
    pub const PATHER_MAP: u32 = 0x8;
    pub const PATHER_MAP_HEIGHT: u32 = 0xc;
    pub const PATHER_MAP_AREA: u32 = 0x10;
    pub const PATHER_MODE: u32 = 0x4c;
    pub const PATHER_RESULT: u32 = 0x70;
    pub const UNIT_SEARCHING: u32 = 0x391;
    pub const SLOT_DIAGONAL_OK: u32 = 0x1ac;
    pub const SLOT_SET_GRID_DEST: u32 = 0x1a0;
    pub const SLOT_HABITAT_ROAMER: u32 = 0x20;
    pub const HABITAT_WATER_LEVEL: u32 = 0x188;
    pub const TILE_TANK_FLAG: u8 = 0x20;
    pub const REC_STEP_IS_DESTINATION: u32 = 0x2;
}

/// A virtual call on `this` taking two arguments and returning `EAX`.
fn call_slot_2_returning(this: u32, offset: u32, first: u32, second: u32) -> u32 {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(this) + offset)) };
    function(this, first, second)
}

/// The fence on `tile`'s edge `direction` (`0..8`), `0` when the slot is empty or holds a non-fence.
fn fence_on_edge(tile: u32, direction: i32) -> u32 {
    let fence: u32 = get_from_memory(tile.wrapping_add(movement::TILE_EDGES).wrapping_add(((direction / 2) as u32).wrapping_mul(4)));
    if fence != 0 && unsafe { entity_type_matches(fence, movement::FENCE_TAG_RVA) } {
        fence
    } else {
        0
    }
}

/// A byte of a tile, or `0` for a missing tile (vanilla dereferences it).
fn tile_byte(tile: u32, offset: u32) -> u8 {
    if tile == 0 {
        0
    } else {
        get_from_memory(tile + offset)
    }
}

fn tile_kind_in_group(kind: u8) -> bool {
    movement::TILE_KIND_GROUP.contains(&kind)
}

/// Whether the tile's entity list holds an entity other than `this` whose virtual `isWalkable` answers false.
fn tile_has_blocker(tile: u32, this: u32) -> bool {
    let sentinel: u32 = get_from_memory(tile);
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let entity: u32 = get_from_memory(node + 8);
        if entity != this && !low_byte_bool(call_slot_0(entity, movement::SLOT_IS_WALKABLE)) {
            return true;
        }
        node = get_from_memory(node);
    }
    false
}

/// Whether the tile's entity list holds the entity with id `id`.
fn tile_has_entity_id(tile: u32, id: u32) -> bool {
    let sentinel: u32 = get_from_memory(tile);
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let entity: u32 = get_from_memory(node + 8);
        if get_from_memory::<u32>(entity + unit_offsets::ENTITY_ID) == id {
            return true;
        }
        node = get_from_memory(node);
    }
    false
}

/// The position vanilla stores for a building: its world position reduced to the tile.
fn building_tile_position(building: u32) -> [i32; 3] {
    crate::util::position_in_tile(
        get_from_memory(building + building_use::BUILDING_X),
        get_from_memory(building + building_use::BUILDING_Y),
        get_from_memory(building + building_use::BUILDING_Z),
    )
}

impl ZTAnimal {
    fn own_tile(self) -> u32 {
        unsafe { GET_TILE.hooked()(self.0 as *const u32) as u32 }
    }

    fn has_show_goal(self) -> bool {
        movement::GOAL_SHOW.contains(&self.read::<i32>(movement::UNIT_GOAL_ID))
    }

    /// `ZTAnimal::stopEverything` (`0x00427664`): the pending-flag byte (`+0x3a8`) is cleared, then `ZTUnit::stopEverything`.
    pub fn stop_everything(self, a: bool, b: bool, c: bool) {
        save_to_memory(self.0 + movement::UNIT_STOP_FLAG, 0u8);
        unsafe { ZTUNIT_STOP_EVERYTHING.hooked()(self.0 as *const u32, a, b, c) };
    }

    /// `ZTAnimal::invalidateTarget` (`0x005b24b3`): `clearEscapeInfo`, then `BFUnit::invalidateTarget`.
    pub fn invalidate_target(self) {
        self.clear_escape_info();
        unsafe { BFUNIT_INVALIDATE_TARGET.original()(self.0 as *const u32) };
    }

    /// The edge fence of `tile` on edge `direction` when it is a `ZTFence`, broken.
    fn break_edge_fence(tile: u32, direction: i32) {
        if tile == 0 {
            return;
        }
        let fence: u32 = get_from_memory(tile + movement::TILE_EDGES + (direction / 2) as u32 * 4);
        if fence != 0 && unsafe { entity_type_matches(fence, movement::FENCE_TAG_RVA) } {
            unsafe { ZTFENCE_SET_BROKEN.hooked()(fence as *const u32, 1) };
        }
    }

    /// `ZTAnimal::breakEscapeFence` (`0x006137fe`): breaks the fence on the edge between the escape record's two tiles
    /// (on the animal's own tile, and again from the neighbouring tile across it), then marks the record broken.
    pub fn break_escape_fence(self) {
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record == 0 {
            return;
        }
        let tile = self.own_tile();
        let direction = unsafe {
            BFMAP_GET_DIRECTION.original()(get_from_memory::<u32>(record + movement::REC_FROM_TILE) as i32, get_from_memory::<u32>(record + movement::REC_TO_TILE) as i32)
        };
        let near_edge = if direction == -1 {
            -1
        } else {
            let edge = direction.wrapping_sub(4) & 7;
            Self::break_edge_fence(tile, edge);
            edge
        };
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, near_edge as u32) };
        if direction != -1 {
            Self::break_edge_fence(neighbour, direction);
        }
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record != 0 {
            save_to_memory(record + movement::REC_BROKEN, 1u8);
        }
    }

    /// Vtable `+0x104` (`forceAnim`) with a behaviour name built (and freed) through the vanilla string.
    fn force_anim(self, name: &str) {
        let text = VanillaString::new(name);
        call_slot_1(self.0, movement::SLOT_FORCE_ANIM, text.as_ptr() as u32);
    }

    /// `ZTAnimal::setReachedEscapeTile` (`0x006138c5`): marks the record reached, then plays the animation of the escape
    /// kind (jump high, climb up, exit tank).
    pub fn set_reached_escape_tile(self) {
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record == 0 {
            return;
        }
        save_to_memory(record + movement::REC_REACHED, 1u8);
        match get_from_memory::<i32>(record + movement::REC_KIND) {
            1 => self.force_anim(movement::ANIM_JUMP),
            2 => self.force_anim(movement::ANIM_CLIMB),
            4 => self.force_anim(movement::ANIM_EXIT_TANK),
            _ => {}
        }
    }

    fn fence_strength(self) -> i32 {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            0
        } else {
            Self::type_field(entity_type, movement::TYPE_FENCE_STRENGTH)
        }
    }

    /// `ZTAnimal::setReachedEscapeTransitionTile` (`0x00613a1c`): a jump breaks the fence when the record is not yet broken
    /// and the type's strength is positive; a fence escape breaks it when the record is not yet broken.
    pub fn set_reached_escape_transition_tile(self) {
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record == 0 {
            return;
        }
        let broken = get_from_memory::<u8>(record + movement::REC_BROKEN) != 0;
        match get_from_memory::<i32>(record + movement::REC_KIND) {
            1 if !broken && self.fence_strength() > 0 => self.break_escape_fence(),
            3 if !broken => self.break_escape_fence(),
            _ => {}
        }
    }

    /// `ZTAnimal::setReachedEscapeDestinationTile` (`0x00613a60`): a jump runs, and breaks the fence when the type is strong
    /// enough and neither the reached nor the broken byte is set; a fence escape breaks it under the same byte
    /// conditions; a climb runs.
    pub fn set_reached_escape_destination_tile(self) {
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if record == 0 {
            return;
        }
        let kind = get_from_memory::<i32>(record + movement::REC_KIND);
        let fresh = |record: u32| get_from_memory::<u8>(record + movement::REC_REACHED) == 0 && get_from_memory::<u8>(record + movement::REC_BROKEN) == 0;
        match kind {
            1 => {
                self.force_anim(movement::ANIM_RUN);
                let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
                if self.fence_strength() > 0 && fresh(record) {
                    self.break_escape_fence();
                }
            }
            3 if fresh(record) => self.break_escape_fence(),
            2 => self.force_anim(movement::ANIM_RUN),
            _ => {}
        }
    }

    /// `ZTAnimal::tileFilter` (`0x004273ec`): whether the unit may step onto `tile`. The tile must lie in the same
    /// habitat as the unit's own (vtable `+0x248`), in the zoo, and fit the unit's terrain mode: a tile flagged `0x8` is
    /// refused unless `flag` is set; a swimmer (`+0xbc`) may use water tiles in mode `1`; a land animal needs dry,
    /// unflagged ground in mode `0`; an underwater unit (`+0x16c`) may use flagged tiles in mode `2`. The tile must
    /// also be clear for the unit to walk by and pass the unnamed `0x427360` test.
    pub fn tile_filter(self, tile: u32, flag: bool) -> bool {
        let this = self.0;
        let own = self.own_tile();
        let own_habitat = call_slot_1(this, movement::SLOT_HABITAT_FROM_TILE, own);
        let target_habitat = call_slot_1(this, movement::SLOT_HABITAT_FROM_TILE, tile);
        if tile == 0 || own_habitat != target_habitat {
            return false;
        }
        if !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.hooked()(tile as *const u32, 0) }) {
            return false;
        }
        let terrain = tile_byte(tile, movement::TILE_TERRAIN);
        let flags = tile_byte(tile, movement::TILE_FLAGS);
        if !flag && terrain & 8 != 0 {
            return false;
        }
        let mode: i32 = self.read(animal_offsets::ACTIVITY_MODE);
        let swimmer_in_water = low_byte_bool(call_slot_0(this, movement::SLOT_GET_SWIMS)) && terrain & 3 != 0 && mode == 1;
        let mode_allows = if swimmer_in_water || (mode == 0 && terrain & 3 == 0 && flags & 0x20 == 0) {
            true
        } else {
            low_byte_bool(call_slot_0(this, movement::SLOT_GET_UNDERWATER)) && flags & 0x20 != 0 && mode == 2
        };
        if !mode_allows {
            return false;
        }
        if !unsafe { BFTILE_IS_CLEAR_FOR_WALK_BY.original()(tile as *const u32, this as *const u32) } {
            return false;
        }
        let has_unit = unsafe { transmute::<u32, unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(movement::TILE_HAS_UNIT)(tile, 0, 0, 0, 0) };
        !low_byte_bool(has_unit)
    }

    /// Walks the unit to `destination` through a freshly stopped unit: `ZTUnit::stopEverything(1, 1, 1)`, vtable `+0x1a8`
    /// (the destination tile) and `BFUnit::goToDest("bWalk", 1, 1)`.
    fn walk_to_tile(self, destination: u32) {
        unsafe { ZTUNIT_STOP_EVERYTHING.hooked()(self.0 as *const u32, true, true, true) };
        call_slot_1(self.0, movement::SLOT_SET_DEST_TILE, destination);
        let name = VanillaString::new(movement::ANIM_WALK);
        unsafe { BFUNIT_GO_TO_DEST.hooked()(self.0 as *const u32, name.as_ptr(), 1, 1) };
    }

    /// `ZTAnimal::setTerrainMode` (`0x004139c9`): switches the activity mode (`+0x380`: `0` underwater, `1` water, `2`
    /// land). A unit whose vtable `+0x170` answers true is always mode `2`. When the mode changes and `flag` is set, a
    /// unit that stands on the wrong kind of tile (and has a habitat) is first sent to a random tile of the right kind.
    /// Modes above `2` change nothing.
    pub fn set_terrain_mode(self, mode: i32, flag: i8) {
        let mode = if low_byte_bool(call_slot_0(self.0, movement::SLOT_GET_ONLY_UNDERWATER)) { 2 } else { mode };
        if self.read::<i32>(animal_offsets::ACTIVITY_MODE) == mode {
            return;
        }
        let tile = self.own_tile();
        let in_water = tile_byte(tile, movement::TILE_TERRAIN) & 3 != 0;
        let cut_off = tile_byte(tile, movement::TILE_FLAGS) >> 5 & 1 != 0;
        let habitat = call_slot_0(self.0, slots::HABITAT);
        let destination = |wanted: bool, pick: &dyn Fn(&ZTHabitat) -> u32| {
            if wanted && flag != 0 && habitat != 0 {
                pick(unsafe { ref_from_memory::<ZTHabitat>(habitat) })
            } else {
                0
            }
        };
        let target = match mode {
            0 => destination(in_water || cut_off, &|h| h.get_random_underwater_tile()),
            1 => destination(!in_water, &|h| h.get_random_water_tile()),
            2 => destination(!cut_off, &|h| h.get_random_land_tile()),
            _ => return,
        };
        if target != 0 {
            self.walk_to_tile(target);
        }
        save_to_memory(self.0 + animal_offsets::ACTIVITY_MODE, mode);
    }

    /// The rest of `findPlaceToRest`/`targetBuilding` once a building is found: its position reduced to the tile is stored as the unit's
    /// destination position (`+0x214`).
    fn store_building_position(self, building: u32) {
        let position = building_tile_position(building);
        for (i, value) in position.iter().enumerate() {
            save_to_memory(self.0 + building_use::UNIT_POSITION + i as u32 * 4, *value);
        }
    }

    /// `ZTAnimal::findPlaceToRest` (`0x00613512`): looks for the nearest (`nearest`) or any building that satisfies the
    /// first need (`DAT_0063afc8`). Not found: `clearGoal` (vtable `+0x174`) and false. Found: the building's tile becomes the
    /// building tile (vtable `+0x2e4`) and its position the destination, `updateBuildingTiles(nearest)` must accept
    /// it (else `clearGoal` and false) and the unit's need byte takes the need's index.
    pub fn find_place_to_rest(self, nearest: bool) -> bool {
        let this = self.0 as *const u32;
        let need = (base() + building_use::NEED_NAME) as *const i32;
        let mut building: u32 = 0;
        let own_tile = self.own_tile() as i32;
        let (out, unit) = (&mut building as *mut u32 as u32, ZTUnit(self.0));
        let found = if nearest {
            unit.find_nearest_satisfier(own_tile as u32, need as u32, out, building_use::SEARCH_RADIUS, building_use::SEARCH_FLAG)
        } else {
            unit.find_any_satisfier(own_tile as u32, need as u32, out, building_use::SEARCH_RADIUS, building_use::SEARCH_FLAG)
        };
        if !found {
            call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
            return false;
        }
        if building == 0 {
            return true;
        }
        let building_tile = unsafe { GET_TILE.original()(building as *const u32) } as u32;
        call_slot_1(self.0, movement::SLOT_SET_BUILDING_TILE, building_tile);
        self.store_building_position(building);
        if !ZTUnit(this as u32).update_building_tiles(nearest) {
            call_slot_0(self.0, lifecycle::SLOT_CLEAR_GOAL);
            return false;
        }
        let index = unsafe { GET_NEED_INDEX.hooked()(need as *const i8) } as u8;
        if index != building_use::NEED_NONE {
            save_to_memory(self.0 + building_use::UNIT_NEED_BYTE, index);
        }
        true
    }

    /// `ZTAnimal::targetBuilding` (`0x005a5a90`): for an animal with the target flag (`+0x3b0`), finds the nearest (`nearest`)
    /// or any building satisfying the second need (`DAT_006463a0`) and paths to its tile (`BFUnit::getPath`); on success
    /// the destination tile and building tile are set, the building's position becomes the destination position, goal
    /// `0x7b` is set (vtable `+0x1d8(0x7b, 1)`), the goal block's tile is set, the need byte is updated and the building's
    /// grandstands are emptied.
    pub fn target_building(self, nearest: bool) -> bool {
        if !self.flag(movement::UNIT_TARGET_FLAG) {
            return false;
        }
        let this = self.0 as *const u32;
        let need = (base() + lifecycle::NEED_B) as *const i32;
        let mut building: u32 = 0;
        let own_tile = self.own_tile() as i32;
        let (out, unit) = (&mut building as *mut u32 as u32, ZTUnit(self.0));
        let found = if nearest {
            unit.find_nearest_satisfier(own_tile as u32, need as u32, out, building_use::SEARCH_RADIUS, building_use::SEARCH_FLAG)
        } else {
            unit.find_any_satisfier(own_tile as u32, need as u32, out, building_use::SEARCH_RADIUS, building_use::SEARCH_FLAG)
        };
        if !found || building == 0 {
            return found;
        }
        let building_tile = unsafe { GET_TILE.original()(building as *const u32) } as u32;
        if !unsafe { BFUNIT_GET_PATH.hooked()(this, building_tile as *const u32, false) } {
            return false;
        }
        call_slot_1(self.0, movement::SLOT_SET_DEST_TILE, building_tile);
        let building_tile_again = unsafe { GET_TILE.original()(building as *const u32) } as u32;
        call_slot_1(self.0, movement::SLOT_SET_BUILDING_TILE, building_tile_again);
        self.store_building_position(building);
        call_slot_2(self.0, movement::SLOT_SET_GOAL, movement::GOAL_BUILDING, 1);
        unsafe { BFCOGGOAL_SET_TILE.hooked()((self.0 + movement::UNIT_GOAL_BLOCK) as *const u32, building_tile as *const u32) };
        let index = unsafe { GET_NEED_INDEX.hooked()(need as *const i8) } as u8;
        if index != building_use::NEED_NONE {
            save_to_memory(self.0 + building_use::UNIT_NEED_BYTE, index);
        }
        crate::ztbuilding::ZTBuilding(building).empty_grandstands();
        true
    }

    /// `ZTAnimal::findPrey` (`0x0043954c`, vtable `+0x2f8`): the entity id of the best prey in sight, or `0`. A prey unit the
    /// animal already holds (`getPreyUnit`) within the search radius is answered directly: its id (`+0x2e4`) when the
    /// virtual `checkPrey` (`+0x300`) accepts it, else `0`. Otherwise every tile within the radius (`DAT_006394f8`) in the
    /// animal's own habitat is scanned: the first unit on a tile that `checkPrey` accepts, is not the held prey and is not
    /// (for an animal without the target flag `+0x3b0`) on a building-use goal competes with the best so far by squared
    /// tile distance from the animal.
    pub fn find_prey(self, force: bool) -> i32 {
        let this = self.0;
        let animal_type = self.animal_type();
        let tile = self.own_tile();
        if tile == 0 {
            return 0;
        }
        let tile_position = |tile: u32| (get_from_memory::<i32>(tile + movement::TILE_X), get_from_memory::<i32>(tile + movement::TILE_Y));
        let (x, y) = tile_position(tile);
        let radius: i32 = get_from_memory(base() + movement::PREY_RADIUS_RVA);
        let check_prey = |unit: u32| low_byte_bool(call_slot_2_returning(this, movement::SLOT_CHECK_PREY, unit, force as u32));
        let prey = call_slot_0(this, movement::SLOT_GET_PREY_UNIT);
        if prey != 0 {
            let prey_tile = unsafe { GET_TILE.original()(prey as *const u32) } as u32;
            if prey_tile != 0 {
                let (prey_x, prey_y) = tile_position(prey_tile);
                if prey_x.wrapping_sub(x).wrapping_abs() <= radius && prey_y.wrapping_sub(y).wrapping_abs() <= radius {
                    return if check_prey(prey) { self.read::<i32>(animal_offsets::PREY_ID) } else { 0 };
                }
            }
        }
        let habitat = call_slot_0(this, slots::HABITAT);
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let map = world.wrapping_add(8);
        let mut best_id: i32 = 0;
        let mut best_distance: i32 = 0;
        if map == 0 || animal_type.is_none() {
            return 0;
        }
        let (map_x, map_y): (i32, i32) = (get_from_memory(map + movement::MAP_X_SIZE), get_from_memory(map + movement::MAP_Y_SIZE));
        let tiles: u32 = get_from_memory(map + movement::MAP_TILES);
        let has_target_flag = self.flag(movement::UNIT_TARGET_FLAG);
        let held_prey: i32 = self.read(animal_offsets::PREY_ID);
        let mut scan_x = x.wrapping_sub(radius);
        while scan_x <= x.wrapping_add(radius) {
            let mut scan_y = y.wrapping_sub(radius);
            while scan_y <= y.wrapping_add(radius) {
                let scanned = if scan_x < 0 || scan_y < 0 || scan_x >= map_x || scan_y >= map_y {
                    0
                } else {
                    tiles.wrapping_add((scan_y.wrapping_mul(map_x).wrapping_add(scan_x) as u32).wrapping_mul(movement::TILE_SIZE))
                };
                let same_habitat = habitat != 0 && call_slot_1(this, movement::SLOT_HABITAT_FROM_TILE, scanned) == habitat;
                if scanned != 0 && same_habitat {
                    let sentinel: u32 = get_from_memory(scanned);
                    let mut node: u32 = get_from_memory(sentinel);
                    while node != sentinel {
                        let entity: u32 = get_from_memory(node + 8);
                        let unit = if entity != 0 && unsafe { entity_type_matches(entity, tags::UNIT) } { entity } else { 0 };
                        if check_prey(unit) && unit != 0 {
                            let id: i32 = get_from_memory(unit + unit_offsets::ENTITY_ID);
                            let building_use_goal = movement::GOAL_BUILDING_USE.contains(&get_from_memory::<i32>(unit + movement::UNIT_GOAL_ID));
                            if id != held_prey && (has_target_flag || !building_use_goal) {
                                let (scanned_x, scanned_y) = tile_position(scanned);
                                let (dx, dy) = (scanned_x.wrapping_sub(x), scanned_y.wrapping_sub(y));
                                let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
                                if best_id == 0 || distance < best_distance {
                                    best_distance = distance;
                                    best_id = id;
                                }
                                break;
                            }
                        }
                        node = get_from_memory(node);
                    }
                }
                scan_y = scan_y.wrapping_add(1);
            }
            scan_x = scan_x.wrapping_add(1);
        }
        best_id
    }

    /// `ZTAnimal::getTerrainCost` (`0x00414b13`): the pathfinder's cost of stepping onto `tile`. The unit's own tile costs
    /// `1`; an impassable tile (`isMaxCostForAll`), a land tile for a swimmer-only unit (`+0x170`) cut off from water,
    /// and an edge-avoiding step cost the sentinel; so does any tile holding a solid unit when the unit's own tile holds
    /// none (unless it is following a target that stands on the tile); a tile kind outside the walkable group costs the
    /// sentinel when the unit's path counter is at least 4 and the unit's own tile is bare or of that group. A show goal
    /// costs `1`; a tile of the wrong medium for the activity mode costs the sentinel minus `3`, a flagged or
    /// non-walkable tile (for a type that is not mode-`1`) the sentinel minus `4`.
    pub fn get_terrain_cost(self, tile: u32) -> i32 {
        let this = self.0 as *const u32;
        let max_cost: i32 = get_from_memory(base() + movement::MAX_COST_RVA);
        if unsafe { IS_SAME_TILE.hooked()(this, tile as *const u32) } != 0 {
            return 1;
        }
        if low_byte_bool(unsafe { IS_MAX_COST_FOR_ALL.hooked()(this, tile as *const u32) }) {
            return max_cost;
        }
        if low_byte_bool(call_slot_0(self.0, movement::SLOT_GET_ONLY_UNDERWATER)) && tile_byte(tile, movement::TILE_FLAGS) & 0x20 == 0 {
            return max_cost;
        }
        let own = self.own_tile();
        if low_byte_bool(unsafe { CHECK_AVOID_EDGES.hooked()(this, tile as *const u32) })
            && !self.flag(animal_offsets::HAS_ESCAPE_INFO)
            && !self.has_show_goal()
            && !low_byte_bool(unsafe { CHECK_AVOID_EDGES.hooked()(this, own as *const u32) })
        {
            let destination = unsafe { GET_DEST_TILE.hooked()(this) } as u32;
            if destination == 0 || (tile_byte(destination, movement::TILE_FLAGS) ^ tile_byte(own, movement::TILE_FLAGS)) & 0x20 == 0 {
                return max_cost;
            }
        }
        if tile_has_blocker(tile, self.0) && own != 0 && !tile_has_blocker(own, self.0) {
            if self.read::<i32>(movement::UNIT_GOAL_ID) != movement::GOAL_FOLLOW {
                return max_cost;
            }
            if !tile_has_entity_id(tile, self.read(movement::UNIT_GOAL_TARGET)) {
                return max_cost;
            }
        }
        let kind = tile_byte(tile, movement::TILE_KIND);
        if kind != 0 && !tile_kind_in_group(kind) && self.read::<i32>(movement::UNIT_PATH_COUNTER) >= 4 {
            let own_kind = tile_byte(own, movement::TILE_KIND);
            if own_kind == 0 || tile_kind_in_group(own_kind) {
                return max_cost;
            }
        }
        if self.has_show_goal() {
            return 1;
        }
        let mode: i32 = self.read(animal_offsets::ACTIVITY_MODE);
        let terrain = tile_byte(tile, movement::TILE_TERRAIN);
        let in_water = terrain & 3 != 0;
        if (mode == 1 && !in_water) || (mode == 0 && in_water) {
            return max_cost.wrapping_sub(3);
        }
        let cliff_mode = self.animal_type().map_or(0, |ty| Self::type_field::<i32>(ty, type_offsets::CLIFF_MODE));
        if cliff_mode != 1 && kind != 0 && !tile_kind_in_group(kind) {
            return max_cost.wrapping_sub(4);
        }
        if terrain & 8 != 0 {
            return max_cost.wrapping_sub(4);
        }
        1
    }

    /// `ZTAnimal::getFeedingPath` (`0x0049f6e8`): paths to the nearest reachable tile next to `destination`. One shared-RNG
    /// draw is always taken; with `shortcut` set and the draw's remainder modulo 100 below `0x32` the unit just paths to
    /// `destination` (`BFUnit::getPath`). Otherwise the four cardinal neighbours of `destination`, sorted by distance from
    /// the unit's grid position, are tried in order: the first that `BFMap::validate` accepts and `getPath` reaches ends the
    /// search. When the unit's virtual `+0x180` asks for a tail, `destination` and then (when valid) the step
    /// `destination + offset(direction(destination - reached))` are appended to the unit's path list (`+0x154`).
    pub fn get_feeding_path(self, destination: u32, shortcut: bool) -> bool {
        let this = self.0 as *const u32;
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let map = world.wrapping_add(8);
        if map == 0 {
            return false;
        }
        let roll = (next_game_random() as i32).wrapping_rem(100);
        if shortcut && roll < movement::SHORTCUT_PERCENT {
            return low_byte_bool(unsafe { BFUNIT_GET_PATH_TO_POSITION.hooked()(this, destination as *const i32, 0) });
        }
        let mut candidates = PoolVec::sized(movement::POSITION_BYTES);
        unsafe { BFMAP_GET_NEIGHBORS_AROUND.original()(map as *const u32, candidates.out_pointer() as *const i32, destination as *const i32, 0) };
        let mut scratch = [0i32; 3];
        let grid_position = unsafe { BFENTITY_GET_GRID_POS.hooked()(this, scratch.as_mut_ptr()) } as u32;
        let own_position: [i32; 3] = [get_from_memory(grid_position), get_from_memory(grid_position + 4), get_from_memory(grid_position + 8)];
        unsafe { BF3DSORTER_SORT_POSITIONS.original()(candidates.out_pointer() as *const i32, own_position.as_ptr() as *const u32) };
        let walk_flag = (self.read::<i32>(movement::UNIT_WALK_MODE) == movement::WALK_MODE_FLAG) as i8;
        let mut reached = [0i32; 3];
        let mut found = false;
        let mut cursor = candidates.begin();
        while cursor != candidates.end() {
            reached = [get_from_memory(cursor), get_from_memory(cursor + 4), get_from_memory(cursor + 8)];
            if unsafe { BFMAP_VALIDATE.hooked()(map as *const u32, destination as *const i32, reached.as_ptr(), this as *const i32, walk_flag) } != 0 {
                found = low_byte_bool(unsafe { BFUNIT_GET_PATH_TO_POSITION.hooked()(this, reached.as_ptr(), 0) });
            }
            cursor += movement::POSITION_BYTES;
            if found {
                break;
            }
        }
        if found && low_byte_bool(call_slot_0(self.0, movement::SLOT_FEEDING_TAIL)) {
            let list = self.0 + movement::UNIT_PATH_LIST;
            let mut iterator = 0u32;
            unsafe { LIST_IVEC3_INSERT.hooked()(&mut iterator as *mut u32 as *const i32, get_from_memory::<u32>(list) as i32, destination as *const i32) };
            let destination_position: [i32; 3] = [get_from_memory(destination), get_from_memory(destination + 4), get_from_memory(destination + 8)];
            let away = glam::i32::IVec3::new(
                destination_position[0].wrapping_sub(reached[0]),
                destination_position[1].wrapping_sub(reached[1]),
                destination_position[2].wrapping_sub(reached[2]),
            );
            let direction = unsafe { BFMAP_GET_DIRECTION_FROM_OFFSET.hooked()(&away as *const glam::i32::IVec3) };
            let mut step = [0i32; 3];
            unsafe { BFMAP_GET_NEIGHBOR_STEP.original()(step.as_mut_ptr(), destination as *const i32, direction as u32, 0) };
            if unsafe { BFMAP_VALIDATE.hooked()(map as *const u32, destination as *const i32, step.as_ptr(), this as *const i32, walk_flag) } != 0 {
                unsafe { LIST_IVEC3_INSERT.hooked()(&mut iterator as *mut u32 as *const i32, get_from_memory::<u32>(list) as i32, step.as_ptr()) };
            }
        }
        drop(candidates);
        found
    }

    /// Points the AI manager's path-search object at the world map and sets the unit's "searching" byte (`+0x391`) to
    /// `flag`, with the search mode the unit's `diagonalOK` (`+0x1ac`) answers. Answers the byte's previous value.
    fn prime_escape_search(self, aimgr: u32, map: u32, flag: u8) -> u8 {
        let pather = aimgr + escape_eval::PATHER;
        let height: u32 = get_from_memory(map + movement::MAP_Y_SIZE);
        let width: u32 = get_from_memory(map + movement::MAP_X_SIZE);
        save_to_memory(pather + escape_eval::PATHER_MAP, map);
        save_to_memory(pather + escape_eval::PATHER_MAP_HEIGHT, height);
        save_to_memory(pather + escape_eval::PATHER_MAP_AREA, (width as i32).wrapping_mul(height as i32));
        let saved: u8 = self.read(escape_eval::UNIT_SEARCHING);
        save_to_memory(self.0 + escape_eval::UNIT_SEARCHING, flag);
        let mode = call_slot_0(self.0, escape_eval::SLOT_DIAGONAL_OK) & 0xff;
        save_to_memory(pather + escape_eval::PATHER_MODE, mode);
        saved
    }

    /// Runs the path-search object's first virtual (`thiscall(from, to, unit, 0)`, `RET 0x10`, answers `AL`) and restores the
    /// unit's "searching" byte.
    fn run_escape_search(self, aimgr: u32, saved: u8, from: &[i32; 3], to: &[i32; 3]) -> bool {
        let pather = aimgr + escape_eval::PATHER;
        let search = unsafe {
            transmute::<u32, extern "thiscall" fn(u32, *const i32, *const i32, u32, u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(pather)))
        };
        let found = low_byte_bool(search(pather, from.as_ptr(), to.as_ptr(), self.0, 0));
        save_to_memory(self.0 + escape_eval::UNIT_SEARCHING, saved);
        found
    }

    /// The `(x, y, z)` words of every node of the AI manager's result path (the list at `ZTAIMgr + 0x80`: a head pointer,
    /// nodes `{next, prev, x, y, z}`).
    fn escape_search_result(aimgr: u32) -> Vec<[i32; 3]> {
        let head: u32 = get_from_memory(aimgr + escape_eval::PATHER + escape_eval::PATHER_RESULT);
        let mut path = Vec::new();
        let mut node: u32 = get_from_memory(head);
        while node != head {
            path.push([get_from_memory(node + 8), get_from_memory(node + 0xc), get_from_memory(node + 0x10)]);
            node = get_from_memory(node);
        }
        path
    }

    /// Appends `position` to the unit's path list (`+0x154`).
    fn push_path_position(self, position: &[i32; 3]) {
        let list = self.0 + movement::UNIT_PATH_LIST;
        let mut iterator = 0u32;
        unsafe { LIST_IVEC3_INSERT.hooked()(&mut iterator as *mut u32 as *const i32, get_from_memory::<u32>(list) as i32, position.as_ptr()) };
    }

    /// The escape kind a fence pair gives an animal of type `animal_type`: `None` leaves the record's kind alone.
    /// Per fence (nearest first, a fence counts only while its open word is non-zero): the type's jump flag with the
    /// fence type's jumpable byte is kind `1`, the climb flag with the climbable byte `2`, a fence weaker than the
    /// type's strength `3`, and a height limit at or above the fence type's height `3`. Neither fence counting is kind `0`.
    fn escape_kind_for_fences(animal_type: u32, near: u32, far: u32) -> Option<i32> {
        let counts = |fence: u32| fence != 0 && get_from_memory::<u32>(fence + movement::FENCE_OPEN_WORD) != 0;
        let judge = |fence: u32| -> Option<i32> {
            let fence_type: u32 = get_from_memory(fence + 0x128);
            if get_from_memory::<u8>(animal_type + movement::ATYPE_JUMPS) != 0 && get_from_memory::<u8>(fence_type + movement::FTYPE_JUMPABLE) != 0 {
                return Some(1);
            }
            if get_from_memory::<u8>(animal_type + movement::ATYPE_CLIMBS) != 0 && get_from_memory::<u8>(fence_type + movement::FTYPE_CLIMBABLE) != 0 {
                return Some(2);
            }
            if get_from_memory::<i32>(animal_type + movement::TYPE_FENCE_STRENGTH) > get_from_memory::<i32>(fence + movement::FENCE_STRENGTH) {
                return Some(3);
            }
            let limit: i32 = get_from_memory(animal_type + movement::ATYPE_HEIGHT_LIMIT);
            if limit != 0 && limit >= get_from_memory::<i32>(fence_type + movement::FTYPE_HEIGHT) {
                return Some(3);
            }
            None
        };
        match (counts(near), counts(far)) {
            (false, false) => Some(0),
            (true, false) => judge(near),
            (false, true) => judge(far),
            (true, true) => judge(near).or_else(|| judge(far)),
        }
    }

    /// `ZTAnimal::evaluateEscape` (`0x0042fffa`): decides whether the animal can leave its habitat through the escape
    /// record's destination tile (`+0x384`, `+8`) and rewrites its path list to do it. The AI manager's path search runs from
    /// the animal's grid position to the destination tile's position (raised by the tank's water level when the tile is
    /// flagged `0x20`) with the unit's searching byte set. A failed search, or a result of fewer than two positions, clears
    /// the record's valid byte. Otherwise the unit's path list is replaced by the result, appended position by position;
    /// the first step whose two tiles have a fence on the shared edge ends the walk: the record's source tile becomes the
    /// step's earlier tile, its kind is chosen from the fences (`4`, exit tank, when the animal's habitat answers its
    /// virtual `+0x20`), and for a non-zero kind a second search (searching byte clear) from the step to the tile across
    /// the fence, when it is exactly two positions long, appends that tile and replaces the step as the destination. The
    /// record's byte `+2` is set when the destination is the step itself, its destination tile becomes the tile of the
    /// final destination, which is also handed to the unit's `setGridDest` (`+0x1a0`). An animal standing on the record's source
    /// tile finishes with `setReachedEscapeTile`.
    pub fn evaluate_escape(self) {
        let this = self.0;
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let map = world.wrapping_add(8);
        if map == 0 {
            return;
        }
        let aimgr = globals().ztaimgr_ptr() as u32;
        let saved = self.prime_escape_search(aimgr, map, 1);
        let mut scratch = [0i32; 3];
        let grid_position = unsafe { BFENTITY_GET_GRID_POS.hooked()(this as *const u32, scratch.as_mut_ptr()) } as u32;
        let start: [i32; 3] = [get_from_memory(grid_position), get_from_memory(grid_position + 4), get_from_memory(grid_position + 8)];
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        let destination_tile: u32 = get_from_memory(record + movement::REC_TO_TILE);
        let mut destination: [i32; 3] = [
            get_from_memory(destination_tile + movement::TILE_X),
            get_from_memory(destination_tile + movement::TILE_Y),
            get_from_memory(destination_tile + movement::TILE_Y + 4),
        ];
        let own_habitat = call_slot_0(this, slots::HABITAT);
        let destination_habitat = call_slot_1(this, movement::SLOT_HABITAT_FROM_TILE, destination_tile);
        if get_from_memory::<u8>(destination_tile + movement::TILE_FLAGS) & escape_eval::TILE_TANK_FLAG != 0 {
            call_slot_0(destination_habitat, escape_eval::SLOT_HABITAT_ROAMER);
            destination[2] = destination[2].wrapping_add(get_from_memory::<i32>(destination_habitat + escape_eval::HABITAT_WATER_LEVEL));
        }
        let record_byte = |offset: u32, value: u8| {
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            save_to_memory(record + offset, value);
        };
        if !self.run_escape_search(aimgr, saved, &start, &destination) {
            record_byte(0, 0);
            return;
        }
        let path = Self::escape_search_result(aimgr);
        if path.len() < 2 {
            record_byte(0, 0);
            return;
        }
        let width: i32 = get_from_memory(map + movement::MAP_X_SIZE);
        let height: i32 = get_from_memory(map + movement::MAP_Y_SIZE);
        let tiles: u32 = get_from_memory(map + movement::MAP_TILES);
        let tile_at = |x: i32, y: i32| -> u32 {
            if x < 0 || y < 0 || x >= width || y >= height {
                0
            } else {
                tiles.wrapping_add((y.wrapping_mul(width).wrapping_add(x) as u32).wrapping_mul(movement::TILE_SIZE))
            }
        };
        let edge_fence = |tile: u32, direction: i32| if tile == 0 { 0 } else { fence_on_edge(tile, direction) };
        self.free_goal_list();
        self.push_path_position(&path[0]);
        let mut previous = path[0];
        for current in &path[1..] {
            self.push_path_position(current);
            let previous_tile = tile_at(previous[0], previous[1]);
            let current_tile = tile_at(current[0], current[1]);
            let direction = unsafe { BFMAP_GET_DIRECTION.original()(previous_tile as i32, current_tile as i32) };
            let (near, far) = if direction == -1 {
                (0, 0)
            } else {
                (edge_fence(previous_tile, direction), edge_fence(current_tile, direction.wrapping_sub(4) & 7))
            };
            if near == 0 && far == 0 {
                previous = *current;
                continue;
            }
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            save_to_memory(record + movement::REC_FROM_TILE, previous_tile);
            let animal_type = self.animal_type().unwrap_or(0);
            let kind = if low_byte_bool(call_slot_0(own_habitat, escape_eval::SLOT_HABITAT_ROAMER)) {
                Some(4)
            } else {
                Self::escape_kind_for_fences(animal_type, near, far)
            };
            if let Some(kind) = kind {
                let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
                save_to_memory(record + movement::REC_KIND, kind);
            }
            let mut across = [0i32; 3];
            unsafe { BFMAP_GET_NEIGHBOR_STEP.original()(across.as_mut_ptr(), current.as_ptr(), direction as u32, 0) };
            let mut final_position = *current;
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            if get_from_memory::<i32>(record + movement::REC_KIND) != 0 {
                let aimgr = globals().ztaimgr_ptr() as u32;
                let saved = self.prime_escape_search(aimgr, map, 0);
                if self.run_escape_search(aimgr, saved, current, &across) && Self::escape_search_result(aimgr).len() == 2 {
                    final_position = across;
                    self.push_path_position(&across);
                }
            }
            if *current == final_position {
                record_byte(escape_eval::REC_STEP_IS_DESTINATION, 1);
            }
            let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
            save_to_memory(record + movement::REC_TO_TILE, tile_at(final_position[0], final_position[1]));
            call_slot_1(this, escape_eval::SLOT_SET_GRID_DEST, final_position.as_ptr() as u32);
            break;
        }
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if self.own_tile() == get_from_memory::<u32>(record + movement::REC_FROM_TILE) {
            self.set_reached_escape_tile();
        }
    }

    /// Copies the tile's position (three words) into `position`, hands it to the unit's virtual `+0x1f8` and answers
    /// `true`; no tile answers `false`.
    fn commit_destination(self, tile: u32, position: u32) -> bool {
        if tile == 0 {
            return false;
        }
        for word in 0..3u32 {
            save_to_memory(position + word * 4, get_from_memory::<u32>(tile + movement::TILE_X + word * 4));
        }
        call_slot_1(self.0, movement::SLOT_SET_DESTINATION, position);
        true
    }

    /// `ZTAnimal::pickRandomDest` (`0x0043e363`): writes a destination position for an animal in a habitat that is not an
    /// exhibit (`+0x2c` clear) and answers whether it did. An animal not already escaping and not in a show, whose species
    /// can escape this habitat (the tree entry's first byte), takes a copy of that 16-byte entry as its escape record
    /// (through vanilla's allocator) and runs `evaluateEscape`; a positive evaluation marks the animal escaping, plays the
    /// escape animation (jump `1`, climb `2`, exit tank `4`) when the record's source tile is the animal's own, and heads for
    /// the record's destination tile. Otherwise a pending direction picks a tile in that direction; else the activity mode
    /// picks one: `1` water, `2` clear ground ahead or land (by the habitat's `+0x20`), anything else underwater. With no
    /// tile found the mode changes towards one that fits the animal's tile (`setTerrainMode`) and the answer is `false`.
    pub fn pick_random_dest(self, position: u32) -> bool {
        let this = self.0;
        let habitat = call_slot_0(this, slots::HABITAT);
        if habitat == 0 || get_from_memory::<u8>(habitat + 0x2c) != 0 {
            return false;
        }
        let habitat_ref = unsafe { ref_from_memory::<ZTHabitat>(habitat) };
        let mut target = 0u32;
        let mut found = false;
        if !self.flag(movement::UNIT_ESCAPE_PENDING) && !low_byte_bool(call_slot_0(this, movement::SLOT_IS_SHOWING)) {
            let key: u32 = self.read(unit_offsets::ENTITY_TYPE);
            let entry = unsafe { TREE36_OPERATOR_INDEX.original()((habitat + movement::HABITAT_ESCAPE_TREE) as *const u32, &key as *const u32) } as u32;
            if get_from_memory::<u8>(entry) != 0 {
                self.clear_escape_info();
                let record = unsafe { OPERATOR_NEW.original()(movement::ESCAPE_ENTRY_SIZE) } as u32;
                if record != 0 {
                    let entry = unsafe { TREE36_OPERATOR_INDEX.original()((habitat + movement::HABITAT_ESCAPE_TREE) as *const u32, &key as *const u32) } as u32;
                    for byte in 0..4u32 {
                        save_to_memory(record + byte, get_from_memory::<u8>(entry + byte));
                    }
                    for word in 1..4u32 {
                        save_to_memory(record + word * 4, get_from_memory::<u32>(entry + word * 4));
                    }
                }
                save_to_memory(this + animal_offsets::ESCAPE_RECORD, record);
                self.evaluate_escape();
                let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
                if get_from_memory::<u8>(record) != 0 {
                    target = get_from_memory(record + movement::REC_TO_TILE);
                    save_to_memory(this + animal_offsets::HAS_ESCAPE_INFO, 1u8);
                    if get_from_memory::<u32>(record + movement::REC_FROM_TILE) == self.own_tile() {
                        match get_from_memory::<i32>(record + movement::REC_KIND) {
                            1 => self.force_anim(movement::ANIM_JUMP),
                            2 => self.force_anim(movement::ANIM_CLIMB),
                            4 => self.force_anim(movement::ANIM_EXIT_TANK),
                            _ => {}
                        }
                    }
                    found = true;
                }
            }
        }
        if !found {
            let direction: i32 = self.read(movement::UNIT_PENDING_DIRECTION);
            if direction != -1 {
                target = habitat_ref.get_random_tile_in_direction(self.own_tile(), direction as u32);
                save_to_memory(this + movement::UNIT_PENDING_DIRECTION, -1i32);
            } else {
                match self.read::<i32>(animal_offsets::ACTIVITY_MODE) {
                    1 => target = habitat_ref.get_random_water_tile(),
                    2 => {
                        target = if low_byte_bool(call_slot_0(habitat, movement::SLOT_HABITAT_LAND_ROAMER)) {
                            habitat_ref.get_random_clear_tile_ahead(this)
                        } else {
                            habitat_ref.get_random_land_tile()
                        };
                    }
                    _ => {
                        target = habitat_ref.get_random_underwater_tile();
                        if target == 0 {
                            let tile = self.own_tile();
                            if low_byte_bool(call_slot_0(this, movement::SLOT_GET_SWIMS)) && tile_byte(tile, movement::TILE_TERRAIN) & 3 != 0 {
                                self.set_terrain_mode(1, 1);
                            } else if low_byte_bool(call_slot_0(this, movement::SLOT_GET_UNDERWATER)) && tile_byte(tile, movement::TILE_FLAGS) & 0x20 != 0 {
                                self.set_terrain_mode(2, 1);
                            }
                            return false;
                        }
                    }
                }
                if target == 0 && matches!(self.read::<i32>(animal_offsets::ACTIVITY_MODE), 1 | 2) {
                    self.set_terrain_mode(0, 1);
                    return false;
                }
            }
        }
        self.commit_destination(target, position)
    }

    /// The habitat record under `tile`'s coordinates in the habitat manager's cell table (`0` when the column is
    /// missing).
    fn habitat_cell_of_tile(tile: u32) -> u32 {
        let manager = globals().zthabitatmgr_ptr() as u32;
        let table: u32 = get_from_memory(manager + conditionals::HABITAT_MGR_TABLE);
        let x: i32 = get_from_memory(tile + conditionals::TILE_X);
        let y: i32 = get_from_memory(tile + conditionals::TILE_Y);
        let column: u32 = get_from_memory(table.wrapping_add((x as u32).wrapping_mul(conditionals::COLUMN_STRIDE)));
        if column == 0 {
            return 0;
        }
        get_from_memory(column.wrapping_add((y as u32).wrapping_mul(conditionals::CELL_STRIDE)))
    }

    /// Whether this animal may cross `fence` (an edge fence of `tile`); `tank_wall` is the fence when its type is a tank
    /// wall type, else `0`. A type that passes everything lets it through; a blocking, gate-named or zoo-gate type
    /// never does. Otherwise an unbuilt fence, the tank-wall exemption and a show goal on a show gate let it through;
    /// an electrified fence or a non-escaping animal never does; an escaping animal in a non-exhibit habitat is judged
    /// by the animal type's climb, jump, strength and height limits.
    fn fence_passable(self, tile: u32, fence: u32, tank_wall: u32) -> bool {
        let fence_type: u32 = get_from_memory(fence + unit_offsets::ENTITY_TYPE);
        if tile_byte(fence_type, movement::FTYPE_PASSES) != 0 {
            return true;
        }
        if tile_byte(fence_type, movement::FTYPE_BLOCKS) != 0 || low_byte_bool(unsafe { ZTSCENERYTYPE_IS_ZOO_GATE.original()(fence_type as *const u32) }) {
            return false;
        }
        let name_begin: u32 = get_from_memory(fence_type + movement::FTYPE_NAME_BEGIN);
        let name_end: u32 = get_from_memory(fence_type + movement::FTYPE_NAME_END);
        if name_end != name_begin && get_from_memory::<u8>(name_begin) == b'g' {
            return false;
        }
        if get_from_memory::<i32>(fence + movement::FENCE_OPEN_WORD) == 0 {
            return true;
        }
        let escaping = self.flag(animal_offsets::HAS_ESCAPE_INFO);
        if tank_wall != 0
            && self.animal_type().is_some_and(|ty| low_byte_bool(call_slot_0(ty, movement::SLOT_ANIMAL_TYPE_TANK_EXEMPT)))
            && !escaping
            && tile_byte(tank_wall, movement::TANK_WALL_BLOCKS) != 0
        {
            return true;
        }
        if low_byte_bool(call_slot_0(fence, movement::SLOT_FENCE_SHOW_GATE)) && movement::GOAL_SHOW.contains(&self.read::<i32>(movement::UNIT_GOAL_ID)) {
            return true;
        }
        if low_byte_bool(unsafe { ZTFENCE_IS_ELECTRIFIED.original()(fence as *const c_void) }) || !escaping {
            return false;
        }
        let habitat = Self::habitat_cell_of_tile(tile);
        if habitat != 0 && tile_byte(habitat, 0x2c) != 0 {
            return false;
        }
        if tank_wall != 0 {
            if tile_byte(tank_wall, movement::TANK_WALL_BLOCKS) != 0 {
                return false;
            }
            if tile_byte(tank_wall, movement::TANK_WALL_OPEN) == 0 {
                return true;
            }
        }
        let own_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if own_type == 0 {
            return false;
        }
        let height_limit = Self::type_field::<i32>(own_type, movement::ATYPE_HEIGHT_LIMIT);
        (tile_byte(own_type, movement::ATYPE_CLIMBS) != 0 && tile_byte(fence_type, movement::FTYPE_CLIMBABLE) != 0)
            || (tile_byte(own_type, movement::ATYPE_JUMPS) != 0 && tile_byte(fence_type, movement::FTYPE_JUMPABLE) != 0)
            || Self::type_field::<i32>(own_type, movement::TYPE_FENCE_STRENGTH) > get_from_memory::<i32>(fence + movement::FENCE_STRENGTH)
            || (height_limit != 0 && height_limit >= get_from_memory::<i32>(fence_type + movement::FTYPE_HEIGHT))
    }

    /// `ZTAnimal::fencesOK` (`0x00414a73`): whether the step from `tile` in `direction` crosses no fence this animal
    /// cannot pass. Clears the bytes at `out_a` and `out_b` first. A missing tile or neighbour is `false`; direction
    /// `-1` is `true`. The fence on the tile's edge and the fence on the neighbour's far edge (`(direction - 4) & 7`)
    /// are judged by [`Self::fence_passable`] (the far one only when the near one passes). A non-escaping animal sets
    /// `out_b` when the near fence, else the far one, is present, not a tank wall and not always-passable.
    pub fn fences_ok(self, tile: u32, direction: i32, out_a: u32, out_b: u32) -> bool {
        save_to_memory(out_a, 0u8);
        save_to_memory(out_b, 0u8);
        if tile == 0 {
            return false;
        }
        if direction == -1 {
            return true;
        }
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction as u32) };
        if neighbour == 0 {
            return false;
        }
        let near = fence_on_edge(tile, direction);
        let far = fence_on_edge(neighbour, direction.wrapping_sub(4) & 7);
        if near == 0 && far == 0 {
            return true;
        }
        let tank_wall = |fence: u32| {
            if fence == 0 {
                return 0;
            }
            let fence_type: u32 = get_from_memory(fence + unit_offsets::ENTITY_TYPE);
            if tile_byte(fence_type, movement::FTYPE_MAY_BE_TANK_WALL) != 0 && Self::type_is_cast(fence_type, movement::TANK_WALL_TAG_RVA) {
                fence
            } else {
                0
            }
        };
        let (near_wall, far_wall) = (tank_wall(near), tank_wall(far));
        let near_ok = near == 0 || self.fence_passable(tile, near, near_wall);
        let far_ok = near_ok && (far == 0 || self.fence_passable(tile, far, far_wall));
        if !self.flag(animal_offsets::HAS_ESCAPE_INFO) {
            let plain = |fence: u32, wall: u32| fence != 0 && wall == 0 && tile_byte(get_from_memory::<u32>(fence + unit_offsets::ENTITY_TYPE), movement::FTYPE_PASSES) == 0;
            if plain(near, near_wall) || plain(far, far_wall) {
                save_to_memory(out_b, 1u8);
            }
        }
        near_ok && far_ok
    }
}

/// Layout the goal pipeline (`updateGoals`, `decide`, `intoNewTile`) reads.
mod goal_flow {
    /// `GLOBAL_ZTAIMgr` and the word it hands the unit at `+0x138` after a threat goal starts.
    pub const AIMGR_RVA: u32 = 0x0023_8098;
    pub const AIMGR_GATE_WORD: u32 = 0xec;
    /// `DAT_00639360`: the second argument of every `BFCogGoal::set`/`setID` call here.
    pub const GOAL_ARG_RVA: u32 = 0x0023_9360;
    /// `DAT_00639370`: three words, the "no position" value `decide` compares the walk target against.
    pub const NO_POSITION_RVA: u32 = 0x0023_9370;
    /// Unit fields: the pool list of goals (`+0x154`, a head pointer whose nodes start `{next, prev, ..}`), the walk target
    /// (three words), the predator/target id (`+0x22c`) and the prey id (`+0x2e4`).
    pub const GOAL_LIST: u32 = 0x154;
    pub const WALK_TARGET: u32 = 0x1dc;
    pub const THREAT_ID: u32 = 0x22c;
    pub const PREY_ID: u32 = 0x2e4;
    pub const GATE_WORD: u32 = 0x138;
    /// A goal-list node's size class: vanilla pushes freed nodes onto `DAT_00638008`, bucket `(size - 1) >> 3 == 2`, so
    /// the nodes are 17-24 bytes; `24` frees into that same bucket.
    pub const GOAL_NODE_SIZE: u32 = 24;
    /// Unit and animal bytes `updateGoals` tests.
    pub const BYTE_FLEEING: u32 = 0x24c;
    pub const BYTE_24F: u32 = 0x24f;
    pub const BYTE_250: u32 = 0x250;
    pub const BYTE_25C: u32 = 0x25c;
    pub const BYTE_CAPTURED: u32 = 0x3a3;
    pub const BYTE_CHASING: u32 = 0x3a4;
    pub const NOT_SELECTABLE: u32 = 0x3a6;
    pub const BOXED: u32 = 0x394;
    pub const EGG: u32 = 0x395;
    pub const CAPTURE_STAGE: u32 = 0x393;
    pub const BYTE_39F: u32 = 0x39f;
    pub const BYTE_3A0: u32 = 0x3a0;
    pub const BYTE_3A1: u32 = 0x3a1;
    pub const BYTE_3A2: u32 = 0x3a2;
    /// Word at `+0x1a8` of a unit using a building (`isUsingBuilding`).
    pub const BUILDING_WORD: u32 = 0x1a8;
    /// The cog goal's byte at `+0x4c`.
    pub const GOAL_BYTE_4C: u32 = 0x4c;
    /// Goal ids.
    pub const GOAL_CHASE: i32 = 0x74;
    pub const GOAL_AVOID: i32 = 0x75;
    pub const GOAL_USE_BUILDING: [i32; 2] = [0x66, 0x67];
    pub const GOAL_WANDER_OUT: i32 = 0x72;
    pub const GOAL_SLEEP: i32 = 0x73;
    pub const GOAL_REST: i32 = 0x7a;
    pub const GOAL_BREATH: i32 = 0x7c;
    pub const GOAL_SHOW_A: i32 = 0x85;
    pub const GOAL_SHOW_B: i32 = 0x86;
    /// Vtable slot of the cog goal's "has a tile" test, and the habitat fields `updateGoals` tests.
    pub const SLOT_GOAL_HAS_TILE: u32 = 8;
    pub const HABITAT_FIELD_4: u32 = 4;
    pub const HABITAT_FIELD_18: u32 = 0x18;
    /// Entity layout `intoNewTile` reads: a tile's four entity pointers (a 2x2 block from `+4`), the type byte that
    /// marks scenery to trample, and the entity's vtable slot that tramples it.
    pub const TILE_ENTITY_SLOTS: u32 = 4;
    pub const TILE_ENTITY_COUNT: u32 = 4;
    pub const SCENERY_TAG_RVA: u32 = 0x0023_8670;
    pub const TYPE_TRAMPLE_FLAG: u32 = 0x59;
    pub const SLOT_TRAMPLE: u32 = 0x124;
    /// Escape record byte `+2`: with `+1` set it keeps firing the transition handler on the destination tile.
    pub const REC_TRANSITION_AGAIN: u32 = 0x2;
    /// The animal's name word, the habitat's name word, the "left habitat" message id and its priority.
    pub const ANIMAL_NAME_WORD: u32 = 0x108;
    pub const HABITAT_NAME_WORD: u32 = 0x154;
    pub const LEFT_HABITAT_MESSAGE: u32 = 0x2846;
    pub const MESSAGE_PRIORITY: i32 = 2;
    /// `GLOBAL_BFUIMgr` (the object itself) and `BFApp::buildString` (`cdecl`, varargs).
    pub const BFUIMGR_RVA: u32 = 0x0023_8de0;
    pub const BUILD_STRING: u32 = 0x0041_ca7f;
    pub const RUN: &str = "bRun";
    pub const RUN_FROM_PREDATOR: &str = "bRunFromPredator";
}

/// How a stage of `updateGoals` ends: continue to the next stage, finish through `ZTUnit::updateGoals`, or return
/// without it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GoalFlow {
    Next,
    Tail,
    Done,
}

impl ZTAnimal {
    fn current_goal(self) -> i32 {
        self.read(movement::UNIT_GOAL_ID)
    }

    fn goal_block(self) -> u32 {
        self.0 + movement::UNIT_GOAL_BLOCK
    }

    /// The current goal is one of the building-use goals `0x68`-`0x6a`.
    fn current_goal_is_building_use(self) -> bool {
        movement::GOAL_BUILDING_USE.contains(&self.current_goal())
    }

    fn set_goal(self, goal: i32, flag: u32) {
        call_slot_2(self.0, movement::SLOT_SET_GOAL, goal as u32, flag);
    }

    fn goal_arg() -> *const i32 {
        (base() + goal_flow::GOAL_ARG_RVA) as *const i32
    }

    fn set_cog_goal(self, goal: i32, target: u32) {
        unsafe { BFCOGGOAL_SET.hooked()(self.goal_block() as *const u32, goal, 3, target, Self::goal_arg(), 0) };
    }

    fn set_cog_goal_id(self, id: u32) {
        unsafe { BFCOGGOAL_SET_ID.hooked()(self.goal_block() as *const u32, id, Self::goal_arg()) };
    }

    /// Whether the unit's goal list (`+0x154`) holds any node.
    fn has_pending_goals(self) -> bool {
        let head: u32 = self.read(goal_flow::GOAL_LIST);
        get_from_memory::<u32>(head) != head
    }

    /// `ZTBuilding::meth_0x437392` (`0x00437392`): the unit's goal is a building-use goal (`0x66`-`0x6a`) and `+0x1a8`
    /// is set, or the goal is `0x69`.
    fn is_using_building(self) -> bool {
        let goal = self.current_goal();
        let in_use = goal_flow::GOAL_USE_BUILDING.contains(&goal) || movement::GOAL_BUILDING_USE.contains(&goal);
        (in_use && self.read::<u32>(goal_flow::BUILDING_WORD) as u8 != 0) || goal == 0x69
    }

    /// The goal-gate word the AI manager hands the unit once a threat goal starts (`+0x138 = ZTAIMgr[+0xec]`).
    fn refresh_gate_word(self) {
        let aimgr: u32 = get_from_memory(base() + goal_flow::AIMGR_RVA);
        save_to_memory(self.0 + goal_flow::GATE_WORD, get_from_memory::<u32>(aimgr + goal_flow::AIMGR_GATE_WORD));
    }

    /// Starts the avoid goal against `threat`: goal `0x75`, the cog goal's id, the threat id, the prey cleared and a
    /// chase in progress stopped. The log strings vanilla builds around the stops feed a no-op and are dropped.
    fn start_avoiding(self, threat: u32) {
        self.set_goal(goal_flow::GOAL_AVOID, 3);
        self.set_cog_goal_id(threat);
        save_to_memory(self.0 + goal_flow::THREAT_ID, threat);
        let chasing = self.flag(goal_flow::BYTE_CHASING);
        save_to_memory(self.0 + goal_flow::PREY_ID, 0u32);
        if chasing {
            save_to_memory(self.0 + goal_flow::BYTE_CHASING, 0u8);
        }
        self.refresh_gate_word();
    }

    /// The threat stage of `updateGoals`: from the fleeing byte through the chase and playmate goals.
    fn update_goals_threat_stage(self, saved: i32) -> GoalFlow {
        let this = self.0;
        if self.flag(goal_flow::BYTE_FLEEING) && saved != goal_flow::GOAL_AVOID && !self.current_goal_is_building_use() {
            let threat = call_slot_1(this, escaped::SLOT_FIND_PREDATOR, 0);
            if threat != 0 {
                call_slot_0(this, escaped::SLOT_NOTIFY_PREDATOR);
                self.start_avoiding(threat);
                return GoalFlow::Done;
            }
            if !self.flag(goal_flow::BYTE_CAPTURED) && !self.flag(goal_flow::BYTE_CHASING) && !low_byte_bool(call_slot_0(this, escaped::SLOT_HAS_PRESSING_NEED)) {
                let second = call_slot_1(this, escaped::SLOT_FIND_PREDATOR, 1);
                if second != 0 {
                    self.start_avoiding(second);
                    return GoalFlow::Done;
                }
                save_to_memory(this + goal_flow::THREAT_ID, 0u32);
                if self.flag(goal_flow::BYTE_FLEEING) {
                    save_to_memory(this + goal_flow::BYTE_FLEEING, 0u8);
                }
            }
        }
        if self.flag(goal_flow::BYTE_25C) && saved != goal_flow::GOAL_AVOID {
            if self.read::<u32>(goal_flow::THREAT_ID) == 0 {
                let threat = call_slot_1(this, escaped::SLOT_FIND_PREDATOR, 0);
                if threat != 0 {
                    save_to_memory(this + goal_flow::THREAT_ID, threat);
                    save_to_memory(this + goal_flow::PREY_ID, 0u32);
                }
            }
            self.set_goal(goal_flow::GOAL_AVOID, 3);
            if self.flag(goal_flow::BYTE_CHASING) {
                save_to_memory(this + goal_flow::BYTE_CHASING, 0u8);
            }
            self.set_cog_goal_id(self.read(goal_flow::THREAT_ID));
            self.refresh_gate_word();
            return GoalFlow::Done;
        }
        if self.flag(goal_flow::BYTE_CAPTURED) || self.flag(goal_flow::BYTE_CHASING) {
            if saved == goal_flow::GOAL_CHASE {
                return GoalFlow::Tail;
            }
            if !self.current_goal_is_building_use() {
                let mate = call_slot_1(this, escaped::SLOT_FIND_PLAYMATE, 0);
                if mate != 0 && self.read::<u32>(goal_flow::PREY_ID) != mate && call_slot_1(this, escaped::SLOT_FIND_PREDATOR, 0) == 0 {
                    call_slot_0(this, escaped::SLOT_NOTIFY_PREDATOR);
                    self.set_cog_goal(goal_flow::GOAL_CHASE, mate);
                    return GoalFlow::Done;
                }
                if !self.flag(goal_flow::BYTE_CAPTURED) && !self.flag(goal_flow::BYTE_FLEEING) && !low_byte_bool(call_slot_0(this, escaped::SLOT_HAS_PRESSING_NEED)) {
                    let other = call_slot_1(this, escaped::SLOT_FIND_PLAYMATE, 1);
                    if other != 0 && self.read::<u32>(goal_flow::PREY_ID) == 0 {
                        self.set_cog_goal(goal_flow::GOAL_CHASE, other);
                        return GoalFlow::Done;
                    }
                }
            }
        }
        GoalFlow::Next
    }

    /// The needs stage of `updateGoals`: show goals, habitat duties, keeper food, sleeping, resting and breath.
    fn update_goals_needs_stage(self, saved: i32) -> GoalFlow {
        let this = self.0;
        if saved == goal_flow::GOAL_CHASE
            || saved == goal_flow::GOAL_AVOID
            || self.flag(goal_flow::BYTE_FLEEING)
            || self.flag(goal_flow::BYTE_CHASING)
            || movement::GOAL_SHOW.contains(&self.current_goal())
        {
            return GoalFlow::Tail;
        }
        let habitat = call_slot_0(this, pipeline::SLOT_HABITAT);
        if self.flag(goal_flow::BYTE_24F) && saved != goal_flow::GOAL_SHOW_A && !self.current_goal_is_building_use() {
            self.set_goal(goal_flow::GOAL_SHOW_A, 1);
            return GoalFlow::Tail;
        }
        let second_show = if self.flag(goal_flow::BYTE_250) {
            true
        } else {
            let tank_with_field = low_byte_bool(call_slot_0(habitat, pipeline::HABITAT_SLOT_IS_TANK)) && get_from_memory::<u32>(habitat + goal_flow::HABITAT_FIELD_4) != 0;
            tank_with_field && !low_byte_bool(call_slot_0(this, pipeline::SLOT_IS_DOING_SHOW)) && get_from_memory::<u32>(habitat + goal_flow::HABITAT_FIELD_18) != 0
        };
        if second_show && saved != goal_flow::GOAL_SHOW_B && !self.current_goal_is_building_use() {
            self.set_goal(goal_flow::GOAL_SHOW_B, 1);
            return GoalFlow::Tail;
        }
        if self.flag(goal_flow::BYTE_39F) && saved != goal_flow::GOAL_WANDER_OUT && !self.current_goal_is_building_use() && !self.flag(goal_flow::BYTE_3A2) {
            self.set_goal(goal_flow::GOAL_WANDER_OUT, 1);
            return GoalFlow::Tail;
        }
        if self.flag(goal_flow::BYTE_3A0)
            && saved != goal_flow::GOAL_SLEEP
            && !ZTUnit(this as u32).has_building_use_goal()
            && !self.flag(goal_flow::BYTE_3A2)
        {
            self.set_goal(goal_flow::GOAL_SLEEP, 1);
            return GoalFlow::Tail;
        }
        if self.is_using_building() {
            return GoalFlow::Tail;
        }
        let position: [u32; 3] = [self.read(building_use::UNIT_POSITION), self.read(building_use::UNIT_POSITION + 4), self.read(building_use::UNIT_POSITION + 8)];
        let need: i32 = self.read(building_use::UNIT_NEED_WORD);
        let satisfier = ZTUnit(self.0).is_need_or_fun_satisfier(need as u32, position.as_ptr() as *const i32);
        if satisfier && get_from_memory::<u8>(self.goal_block() + goal_flow::GOAL_BYTE_4C) == 0 {
            let goal = self.current_goal();
            let in_use = goal_flow::GOAL_USE_BUILDING.contains(&goal) || self.current_goal_is_building_use();
            if !in_use || (saved == goal_flow::GOAL_USE_BUILDING[0] && !self.has_pending_goals()) {
                ZTUnit(this as u32).update_building_tiles(false);
                return GoalFlow::Tail;
            }
        }
        if self.flag(goal_flow::BYTE_3A1) && saved != goal_flow::GOAL_REST && !self.flag(goal_flow::BYTE_3A2) && !self.current_goal_is_building_use() {
            self.set_goal(goal_flow::GOAL_REST, 1);
            return GoalFlow::Tail;
        }
        if self.needs_breath() && !low_byte_bool(call_slot_0(this, escaped::SLOT_HAS_PRESSING_NEED)) {
            self.set_goal(goal_flow::GOAL_BREATH, 0);
        }
        GoalFlow::Tail
    }

    /// `ZTAnimal::updateGoals` (`0x00436f5c`): an unselectable, captured, boxed or egg animal drops its cog goal.
    /// Otherwise a threat stage (avoid a predator, chase a playmate) and a needs stage (show, habitat duties, keeper
    /// food, sleep, rest, breath) each either start a goal or fall through; unless a stage returns early every path
    /// ends in `ZTUnit::updateGoals`. The log strings vanilla builds around goal changes feed a no-op and are dropped.
    pub fn update_goals(self) {
        let saved = self.current_goal();
        if self.flag(goal_flow::NOT_SELECTABLE) || self.flag(goal_flow::CAPTURE_STAGE) || self.flag(goal_flow::BOXED) || self.flag(goal_flow::EGG) {
            unsafe { BFCOGGOAL_CLEAR.hooked()(self.goal_block() as *const c_void) };
            return;
        }
        let flow = match self.update_goals_threat_stage(saved) {
            GoalFlow::Next => self.update_goals_needs_stage(saved),
            other => other,
        };
        if flow == GoalFlow::Tail {
            ZTUnit(self.0).update_goals();
        }
    }

    /// Frees the nodes of the unit's goal list (`+0x154`) into the pool and resets the list to empty.
    fn free_goal_list(self) {
        let head: u32 = self.read(goal_flow::GOAL_LIST);
        let mut node: u32 = get_from_memory(head);
        while node != head {
            let next: u32 = get_from_memory(node);
            if node != 0 {
                unsafe { POOLALLOC_DEALLOCATE.original()(node as *const u32, goal_flow::GOAL_NODE_SIZE) };
            }
            node = next;
        }
        save_to_memory(head, head);
        save_to_memory(head + 4, head);
    }

    fn run_to_destination(self, behaviour: &str) -> bool {
        let name = VanillaString::new(behaviour);
        low_byte_bool(unsafe { BFUNIT_GO_TO_DEST.hooked()(self.0 as *const u32, name.as_ptr(), 0, 1) })
    }

    /// `ZTAnimal::decide` (`0x0043e440`), answering `AL`: without a `ZTAIMgr` nothing. A unit whose goal is not `1`
    /// takes `BFUnit::decide`. Otherwise, unless the cog goal has a tile while the goal list holds nodes, the list is
    /// freed and `BFAIMgr::fRandomWalk` picks a target; a new target is set on the cog goal and, for an animal with an
    /// escape record, run to (`bRun`, then `bRunFromPredator`). When neither can start, the escape record is freed
    /// through `operator delete` and cleared.
    pub fn decide(self) -> u32 {
        let aimgr: u32 = get_from_memory(base() + goal_flow::AIMGR_RVA);
        if aimgr == 0 {
            return 0;
        }
        if self.current_goal() != 1 {
            return unsafe { BFUNIT_DECIDE.hooked()(self.0 as *const u32) } & 0xff;
        }
        if low_byte_bool(call_slot_0(self.goal_block(), goal_flow::SLOT_GOAL_HAS_TILE)) && self.has_pending_goals() {
            return 0;
        }
        self.free_goal_list();
        let walked = unsafe { BFAIMGR_F_RANDOM_WALK.hooked()(aimgr as *const u32, self.0 as *const i32) } & 0xff;
        if walked == 0 {
            return 0;
        }
        let target: [u32; 3] = [self.read(goal_flow::WALK_TARGET), self.read(goal_flow::WALK_TARGET + 4), self.read(goal_flow::WALK_TARGET + 8)];
        let none_at = base() + goal_flow::NO_POSITION_RVA;
        let none: [u32; 3] = [get_from_memory(none_at), get_from_memory(none_at + 4), get_from_memory(none_at + 8)];
        if target == none {
            return walked;
        }
        unsafe { BFCOGGOAL_SET_TARGET_POS.hooked()(self.goal_block() as *const u32, target.as_ptr() as *const i32) };
        if !self.flag(animal_offsets::HAS_ESCAPE_INFO) {
            return walked;
        }
        if self.run_to_destination(goal_flow::RUN) || self.run_to_destination(goal_flow::RUN_FROM_PREDATOR) {
            return walked;
        }
        save_to_memory(self.0 + animal_offsets::HAS_ESCAPE_INFO, 0u8);
        unsafe { OPERATOR_DELETE.original()(self.read(animal_offsets::ESCAPE_RECORD)) };
        save_to_memory(self.0 + animal_offsets::ESCAPE_RECORD, 0u32);
        walked
    }

    /// Trampling for a rampaging animal: every distinct scenery entity on the four entity slots of `tile` whose type
    /// flag (`+0x59`) is set gets vtable `+0x124` once.
    fn trample_scenery(self, tile: u32) {
        let mut touched: Vec<u32> = Vec::new();
        for slot in 0..goal_flow::TILE_ENTITY_COUNT {
            let entity: u32 = get_from_memory(tile + goal_flow::TILE_ENTITY_SLOTS + slot * 4);
            if entity == 0 || !unsafe { entity_type_matches(entity, goal_flow::SCENERY_TAG_RVA) } {
                continue;
            }
            let entity_type: u32 = get_from_memory(entity + 0x128);
            if get_from_memory::<u8>(entity_type + goal_flow::TYPE_TRAMPLE_FLAG) == 0 || touched.contains(&entity) {
                continue;
            }
            call_slot_0(entity, goal_flow::SLOT_TRAMPLE);
            touched.push(entity);
        }
    }

    /// An escaping animal entering `tile`: the record's reached, transition and destination handlers, then the dirty
    /// marks and the "left habitat" message when `tile` and `neighbour` lie in different habitats.
    fn escape_tile_events(self, tile: u32, neighbour: u32) {
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if tile == get_from_memory::<u32>(record + movement::REC_FROM_TILE) {
            self.set_reached_escape_tile();
        } else if get_from_memory::<u8>(record + movement::REC_REACHED) != 0
            && (tile != get_from_memory::<u32>(record + movement::REC_TO_TILE) || get_from_memory::<u8>(record + goal_flow::REC_TRANSITION_AGAIN) != 0)
        {
            self.set_reached_escape_transition_tile();
        }
        let record: u32 = self.read(animal_offsets::ESCAPE_RECORD);
        if tile == get_from_memory::<u32>(record + movement::REC_TO_TILE) {
            self.set_reached_escape_destination_tile();
        }
        let neighbour_habitat = call_slot_1(self.0, movement::SLOT_HABITAT_FROM_TILE, neighbour);
        let tile_habitat = call_slot_1(self.0, movement::SLOT_HABITAT_FROM_TILE, tile);
        if neighbour_habitat == tile_habitat {
            return;
        }
        unsafe { ZTHABITAT_SET_DIRTY_CHARACTERISTICS.hooked()(neighbour_habitat as *const u32) };
        if get_from_memory::<u8>(tile_habitat + pipeline::HABITAT_NO_HATCH) != 0 {
            return;
        }
        unsafe { ZTHABITAT_SET_DIRTY_CHARACTERISTICS.hooked()(tile_habitat as *const u32) };
        let text = VanillaString::rvo_target();
        let build: unsafe extern "cdecl" fn(*const u32, *const u32, u32, u32, u32) -> *const u32 = unsafe { transmute(goal_flow::BUILD_STRING) };
        unsafe {
            build(
                GET_APP.original()(),
                text.as_ptr(),
                goal_flow::LEFT_HABITAT_MESSAGE,
                self.read(goal_flow::ANIMAL_NAME_WORD),
                get_from_memory::<u32>(tile_habitat + goal_flow::HABITAT_NAME_WORD),
            )
        };
        let bfuimgr = base() + goal_flow::BFUIMGR_RVA;
        unsafe {
            BFUIMGR_DISPLAY_MESSAGE_1.hooked()(bfuimgr as *const u32, text.as_ptr() as *const i32, goal_flow::MESSAGE_PRIORITY, std::ptr::null(), self.0 as *const u32, true, false)
        };
    }

    /// `ZTAnimal::intoNewTile` (`0x00437572`): the tile the animal enters and the neighbour in `direction` are looked
    /// up; an animal with escape info runs the record's tile handlers, a rampaging one tramples the scenery on the
    /// tile, and every path ends in `ZTUnit::intoNewTile`.
    pub fn into_new_tile(self, direction: u32) {
        let tile = self.own_tile();
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.hooked()((world + 8) as *const u32, tile as *const u32, direction) };
        if self.flag(animal_offsets::HAS_ESCAPE_INFO) {
            self.escape_tile_events(tile, neighbour);
        } else if self.flag(escaped::BYTE_RAMPAGING) {
            self.trample_scenery(tile);
        }
        ZTUnit(self.0).into_new_tile(direction);
    }
}

/// Slots and fields `drop` and `setVisible` use.
mod drop_visible {
    /// Animal vtable: the habitat (`+0x24c`), predator search (`+0x2fc`), prey search (`+0x2f8`), each taking `0`.
    pub const SLOT_HABITAT: u32 = 0x24c;
    pub const SLOT_FIND_PREDATOR: u32 = 0x2fc;
    pub const SLOT_FIND_PREY: u32 = 0x2f8;
    /// Habitat vtable `+0x20` answers (in `AL`) whether the habitat is a tank-style exhibit.
    pub const HABITAT_SLOT_QUERY: u32 = 0x20;
    /// `BFWorldMgr::getUnit(id)`.
    pub const WORLD_GET_UNIT: u32 = 0x2c;
    /// The unit's own slot invoked once the unit is found (`+0x2c4`, no arguments).
    pub const UNIT_SLOT_RELEASE: u32 = 0x2c4;
    /// The entity's visible byte.
    pub const VISIBLE: u32 = 0x13f;
}

impl ZTAnimal {
    /// `ZTAnimal::drop` (vtable `+0xc8`). Needs a habitat answering the habitat-slot query. The first non-zero id out of
    /// the predator search then the prey search names a unit; when that unit's type is a `ZTUnit` type the unit's
    /// slot `+0x2c4` runs. Both id tests are full-register (`TEST EAX, EAX`).
    pub fn drop_target(self) {
        let habitat = call_slot_0(self.0, drop_visible::SLOT_HABITAT);
        if habitat == 0 || !low_byte_bool(call_slot_0(habitat, drop_visible::HABITAT_SLOT_QUERY)) {
            return;
        }
        let mut id = call_slot_1(self.0, drop_visible::SLOT_FIND_PREDATOR, 0);
        if id == 0 {
            id = call_slot_1(self.0, drop_visible::SLOT_FIND_PREY, 0);
            if id == 0 {
                return;
            }
        }
        let world = Self::world();
        let unit = call_slot_1(world, drop_visible::WORLD_GET_UNIT, id);
        if unit == 0 || !Self::entity_is(unit, tags::UNIT) {
            return;
        }
        call_slot_0(unit, drop_visible::UNIT_SLOT_RELEASE);
    }

    /// `ZTAnimal::setVisible`: stores the byte and refreshes the animal panel.
    pub fn set_visible(self, visible: bool) {
        self.write_visible(visible);
        unsafe { ANIMALINFO_SET_ANIMAL_VISIBLE.original()(self.0 as *const u32, visible as i8) };
    }

    fn write_visible(self, visible: bool) {
        save_to_memory(self.0 + drop_visible::VISIBLE, u8::from(visible));
    }
}

#[detour_mod]
mod animal_detours {
    use super::*;

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        ZTAnimal(this as u32).update_goals();
    }

    #[detour(DECIDE)]
    unsafe extern "thiscall" fn decide(this: *const u32) -> u32 {
        ZTAnimal(this as u32).decide()
    }

    #[detour(INTO_NEW_TILE)]
    unsafe extern "thiscall" fn into_new_tile(this: *const u32, direction: u32) {
        ZTAnimal(this as u32).into_new_tile(direction);
    }

    #[detour(STOP_EVERYTHING)]
    unsafe extern "thiscall" fn stop_everything(this: *const u32, a: bool, b: bool, c: bool) {
        ZTAnimal(this as u32).stop_everything(a, b, c);
    }

    #[detour(INVALIDATE_TARGET)]
    unsafe extern "thiscall" fn invalidate_target(this: *const c_void) {
        ZTAnimal(this as u32).invalidate_target();
    }

    #[detour(BREAK_ESCAPE_FENCE)]
    unsafe extern "thiscall" fn break_escape_fence(this: *const u32) {
        ZTAnimal(this as u32).break_escape_fence();
    }

    #[detour(SET_REACHED_ESCAPE_TILE)]
    unsafe extern "thiscall" fn set_reached_escape_tile(this: *const c_void) {
        ZTAnimal(this as u32).set_reached_escape_tile();
    }

    #[detour(SET_REACHED_ESCAPE_TRANSITION_TILE)]
    unsafe extern "fastcall" fn set_reached_escape_transition_tile(this: i32) {
        ZTAnimal(this as u32).set_reached_escape_transition_tile();
    }

    #[detour(SET_REACHED_ESCAPE_DESTINATION_TILE)]
    unsafe extern "thiscall" fn set_reached_escape_destination_tile(this: *const c_void) {
        ZTAnimal(this as u32).set_reached_escape_destination_tile();
    }

    #[detour(TILE_FILTER)]
    unsafe extern "thiscall" fn tile_filter(this: *const c_void, tile: *const c_void, flag: bool) -> bool {
        ZTAnimal(this as u32).tile_filter(tile as u32, flag)
    }

    #[detour(FENCES_OK)]
    unsafe extern "thiscall" fn fences_ok(this: *const u32, tile: *const u32, direction: u32, out_a: *const bool, out_b: *const bool) -> u32 {
        ZTAnimal(this as u32).fences_ok(tile as u32, direction as i32, out_a as u32, out_b as u32) as u32
    }

    #[detour(EVALUATE_ESCAPE)]
    unsafe extern "thiscall" fn evaluate_escape(this: *const c_void) {
        ZTAnimal(this as u32).evaluate_escape()
    }

    #[detour(PICK_RANDOM_DEST)]
    unsafe extern "thiscall" fn pick_random_dest(this: *const u32, position: *const IVec3) -> u32 {
        ZTAnimal(this as u32).pick_random_dest(position as u32) as u32
    }

    #[detour(GET_FEEDING_PATH)]
    unsafe extern "thiscall" fn get_feeding_path(this: *const c_void, destination: *const i32, shortcut: u8) -> u32 {
        ZTAnimal(this as u32).get_feeding_path(destination as u32, shortcut != 0) as u32
    }

    #[detour(GET_TERRAIN_COST)]
    unsafe extern "thiscall" fn get_terrain_cost(this: *const u32, tile: *const u32) -> u32 {
        ZTAnimal(this as u32).get_terrain_cost(tile as u32) as u32
    }

    #[detour(SET_TERRAIN_MODE)]
    unsafe extern "thiscall" fn set_terrain_mode(this: *const u32, mode: i32, flag: i8) {
        ZTAnimal(this as u32).set_terrain_mode(mode, flag);
    }

    #[detour(FIND_PREY)]
    unsafe extern "thiscall" fn find_prey(this: *const u32, force: bool) -> i32 {
        ZTAnimal(this as u32).find_prey(force)
    }

    #[detour(FIND_PLACE_TO_REST)]
    unsafe extern "thiscall" fn find_place_to_rest(this: *const u32, nearest: bool) -> u32 {
        ZTAnimal(this as u32).find_place_to_rest(nearest) as u32
    }

    #[detour(TARGET_BUILDING)]
    unsafe extern "thiscall" fn target_building(this: *const u32, nearest: bool) -> bool {
        ZTAnimal(this as u32).target_building(nearest)
    }

    #[detour(F_GET_MATE)]
    unsafe extern "thiscall" fn f_get_mate(this: *const u32, found: *const bool) -> i32 {
        ZTAnimal(this as u32).f_get_mate(found as u32) as i32
    }

    #[detour(ADD_FACE)]
    unsafe extern "thiscall" fn add_face(this: *const u32, smile: bool) {
        ZTAnimal(this as u32).add_face(smile);
    }

    #[detour(CHECK_PREY)]
    unsafe extern "thiscall" fn check_prey(this: *const u32, target: *const u32, force: bool) -> bool {
        ZTAnimal(this as u32).check_prey(target as u32, force)
    }

    #[detour(STOP_EATING)]
    unsafe extern "thiscall" fn stop_eating(this: *const u32, flag: bool) {
        ZTAnimal(this as u32).stop_eating(flag as u8);
    }

    #[detour(HATCH_EGG)]
    unsafe extern "thiscall" fn hatch_egg(this: *const u32) {
        ZTAnimal(this as u32).hatch_egg();
    }

    #[detour(FINISH_HATCH)]
    unsafe extern "thiscall" fn finish_hatch(this: *const u32) {
        ZTAnimal(this as u32).finish_hatch();
    }

    #[detour(F_EAT)]
    unsafe extern "thiscall" fn f_eat(this: *const u32) -> *const u32 {
        ZTAnimal(this as u32).f_eat() as *const u32
    }

    #[detour(F_CHECK_REPRODUCTION)]
    unsafe extern "fastcall" fn f_check_reproduction(this: *const i32) -> bool {
        ZTAnimal(this as u32).f_check_reproduction()
    }

    #[detour(CONSUME)]
    unsafe extern "thiscall" fn consume(this: *const u32, unit: *const i32) {
        ZTAnimal(this as u32).consume(unit as u32);
    }

    #[detour(SHOW_ESCAPED_ANIMAL_ALERT)]
    unsafe extern "thiscall" fn show_escaped_animal_alert(this: *const u32) {
        ZTAnimal(this as u32).show_escaped_animal_alert();
    }

    #[detour(CHASE_DONE)]
    unsafe extern "thiscall" fn chase_done(this: *const u32, unit: *const u32) -> bool {
        ZTAnimal(this as u32).chase_done(unit as u32)
    }

    #[detour(PUT_DOWN_DIRT)]
    unsafe extern "thiscall" fn put_down_dirt(this: *const u32, tile: *const u32) -> bool {
        ZTAnimal(this as u32).put_down_dirt(tile as u32)
    }

    #[detour(STOP_CHASE)]
    unsafe extern "thiscall" fn stop_chase(this: *const u32, force: bool) {
        ZTAnimal(this as u32).stop_chase(force);
    }

    #[detour(BOX)]
    unsafe extern "thiscall" fn box_animal(this: *const u32) -> bool {
        ZTAnimal(this as u32).box_animal()
    }

    #[detour(UNBOX)]
    unsafe extern "fastcall" fn unbox(this: *const u32) -> bool {
        ZTAnimal(this as u32).unbox()
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
    unsafe extern "thiscall" fn remove_food_target_for_all(this: *const u32, food: *const u32) -> bool {
        ZTAnimal(this as u32).remove_food_target_for_all(food as u32) != 0
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
    unsafe extern "thiscall" fn add_changes(this: *const u32, changes: *const u32) -> bool {
        ZTAnimal(this as u32).add_changes(changes as u32)
    }

    #[detour(DO_FOOD_TRICK)]
    unsafe extern "thiscall" fn do_food_trick(this: *const u32, word_a: u16, word_b: u16) -> u32 {
        ZTAnimal(this as u32).do_food_trick(word_a, word_b);
        0
    }

    #[detour(CHECK_DO_TRICK)]
    unsafe extern "thiscall" fn check_do_trick(this: *const u32) -> bool {
        ZTAnimal(this as u32).check_do_trick()
    }

    #[detour(DO_ESCAPED_CHECK)]
    unsafe extern "thiscall" fn do_escaped_check(this: *const u32) {
        ZTAnimal(this as u32).do_escaped_check();
    }

    /// `generated.rs` declares a `u32` return; the body is `void` and `doConditionals` ignores `EAX`.
    #[detour(DO_BUILDING_USE_CHECK)]
    unsafe extern "thiscall" fn do_building_use_check(this: *const u32) {
        ZTAnimal(this as u32).do_building_use_check();
    }

    /// `generated.rs` declares a pointer return; the body leaves `EAX` unspecified and callers ignore it.
    #[detour(DO_AMBIENT_ANIMS)]
    unsafe extern "thiscall" fn do_ambient_anims(this: *const u32, _unused: *const u32) {
        ZTAnimal(this as u32).do_ambient_anims();
    }

    #[detour(DO_BABY_TO_ADULT_CHECK)]
    unsafe extern "thiscall" fn do_baby_to_adult_check(this: *const u32) {
        ZTAnimal(this as u32).do_baby_to_adult_check();
    }

    #[detour(DO_REPRODUCE_CHECK)]
    unsafe extern "thiscall" fn do_reproduce_check(this: *const u32) {
        ZTAnimal(this as u32).do_reproduce_check();
    }

    #[detour(DO_HEALTH_CHECK)]
    unsafe extern "thiscall" fn do_health_check(this: *const u32) {
        ZTAnimal(this as u32).do_health_check();
    }

    #[detour(DO_BOXED_CHECK)]
    unsafe extern "thiscall" fn do_boxed_check(this: *const u32) -> *const u32 {
        ZTAnimal(this as u32).do_boxed_check();
        std::ptr::null()
    }

    #[detour(DO_SLEEP_CHECK)]
    unsafe extern "thiscall" fn do_sleep_check(this: *const u32) {
        ZTAnimal(this as u32).do_sleep_check();
    }

    /// `generated.rs` declares a `u32` return; the body leaves `EAX` unspecified and `doConditionals` ignores it.
    #[detour(DO_ZAP_CHECK)]
    unsafe extern "thiscall" fn do_zap_check(this: *const u32) -> u32 {
        ZTAnimal(this as u32).do_zap_check();
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
    unsafe extern "thiscall" fn do_water_check(this: *const u32) {
        ZTAnimal(this as u32).do_water_check();
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
    unsafe extern "thiscall" fn is_hungry(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_hungry()
    }

    #[detour(IS_HUNGRY_AND_FOODLESS)]
    unsafe extern "thiscall" fn is_hungry_and_foodless(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_hungry_and_foodless()
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
    unsafe extern "thiscall" fn is_unhappy_with_habitat(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_unhappy_with_habitat()
    }

    #[detour(IS_VERY_UNHAPPY_WITH_HABITAT)]
    unsafe extern "thiscall" fn is_very_unhappy_with_habitat(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_very_unhappy_with_habitat()
    }

    #[detour(IS_SICKLY)]
    unsafe extern "thiscall" fn is_sickly(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_sickly()
    }

    #[detour(IS_EGG)]
    unsafe extern "thiscall" fn is_egg(this: *const c_void) -> bool {
        ZTAnimal(this as u32).is_egg() != 0
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
    unsafe extern "thiscall" fn is_in_prey_list(this: *const u32, id: i32) -> bool {
        ZTAnimal(this as u32).is_in_prey_list(id)
    }

    #[detour(IS_PREATTACK)]
    unsafe extern "thiscall" fn is_preattack(this: *const u32, other: i32) -> bool {
        ZTAnimal(this as u32).is_preattack(other as u32)
    }

    #[detour(WITHIN_PREATTACK_RANGE)]
    unsafe extern "thiscall" fn within_preattack_range(this: *const u32, other: i32) -> bool {
        ZTAnimal(this as u32).within_preattack_range(other as u32)
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
    unsafe extern "thiscall" fn is_walkable(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_walkable()
    }

    #[detour(CLEAR_PREY)]
    unsafe extern "thiscall" fn clear_prey(this: *const u32) {
        ZTAnimal(this as u32).clear_prey();
    }

    #[detour(NO_SHADOW_PART)]
    unsafe extern "thiscall" fn no_shadow_part(this: *const u32) -> bool {
        ZTAnimal(this as u32).no_shadow_part()
    }

    #[detour(NEEDS_BREATH)]
    unsafe extern "thiscall" fn needs_breath(this: *const c_void) -> bool {
        ZTAnimal(this as u32).needs_breath()
    }

    #[detour(GET_KEEP_MOVING)]
    unsafe extern "thiscall" fn get_keep_moving(this: *const c_void) -> bool {
        ZTAnimal(this as u32).get_keep_moving()
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

    #[detour(ADD_TO_MAP)]
    unsafe extern "thiscall" fn add_to_map(this: *const u32) -> bool {
        ZTAnimal(this as u32).add_to_map()
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, flag: bool) {
        ZTAnimal(this as u32).remove_from_map(flag);
    }

    #[detour(SET_DYING)]
    unsafe extern "thiscall" fn set_dying(this: *const u32, flag: bool) {
        ZTAnimal(this as u32).set_dying(flag);
    }

    #[detour(SET_NAME)]
    unsafe extern "thiscall" fn set_name(this: *const u32, name: *const u32) {
        ZTAnimal(this as u32).set_name(name as u32);
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) {
        ZTAnimal(this as u32).init_status_vars();
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTAnimal(this as u32).load(file, version)
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

    #[detour(GET_IDLE_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_idle_anim_symbol(this: *const u32) -> *const u32 {
        ZTAnimal(this as u32).get_idle_anim_symbol() as *const u32
    }

    #[detour(GET_ONLY_SWIMS)]
    unsafe extern "thiscall" fn get_only_swims(this: *const u32) -> bool {
        ZTAnimal(this as u32).get_only_swims()
    }

    #[detour(GET_MOVE_ANIM)]
    unsafe extern "thiscall" fn get_move_anim(this: *const u32, name: *const i8) -> *const i8 {
        ZTAnimal(this as u32).get_move_anim(name)
    }

    #[detour(COMPARE_LT)]
    unsafe extern "thiscall" fn compare_lt(this: *const c_void, other: *const u8) -> bool {
        ZTAnimal(this as u32).compare_lt(other as u32)
    }

    #[detour(SET_MAX_HEALTH)]
    unsafe extern "thiscall" fn set_max_health(this: *const c_void) {
        ZTAnimal(this as u32).set_max_health();
    }

    #[detour(IS_DOING_SOMETHING)]
    unsafe extern "thiscall" fn is_doing_something(this: *const u32) -> bool {
        ZTAnimal(this as u32).is_doing_something()
    }

    #[detour(GET_SWIMS)]
    unsafe extern "thiscall" fn get_swims(this: *const u32) -> bool {
        ZTAnimal(this as u32).get_swims() != 0
    }

    #[detour(GET_UNDERWATER)]
    unsafe extern "thiscall" fn get_underwater(this: *const c_void) -> bool {
        ZTAnimal(this as u32).get_underwater() != 0
    }

    #[detour(IS_WALKABLE_BY)]
    unsafe extern "thiscall" fn is_walkable_by(this: *const u32, other: *const u32) -> bool {
        ZTAnimal(this as u32).is_walkable_by(other as u32)
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
    unsafe extern "thiscall" fn f_mate_gone(this: *const u32) -> bool {
        ZTAnimal(this as u32).f_mate_gone()
    }

    #[detour(F_MATE_AVAILABLE)]
    unsafe extern "thiscall" fn f_mate_available(this: *const u32, flag: *const bool) -> bool {
        ZTAnimal(this as u32).f_mate_available(flag)
    }

    #[detour(F_OTHER_ANIMAL_ANGRY)]
    unsafe extern "thiscall" fn f_other_animal_angry(this: *const u32) -> bool {
        ZTAnimal(this as u32).f_other_animal_angry()
    }

    #[detour(F_OTHER_ANIMAL_SICK)]
    unsafe extern "thiscall" fn f_other_animal_sick(this: *const u32) -> bool {
        ZTAnimal(this as u32).f_other_animal_sick()
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
    unsafe extern "thiscall" fn f_is_there_keeper_food(this: *const u32) -> bool {
        ZTAnimal(this as u32).f_is_there_keeper_food()
    }

    #[detour(CHECK_DONE)]
    unsafe extern "thiscall" fn check_done(this: *const u32) -> u32 {
        ZTAnimal(this as u32).check_done()
    }

    #[detour(DROP)]
    unsafe extern "thiscall" fn drop_target(this: *const u32) {
        ZTAnimal(this as u32).drop_target();
    }

    #[detour(SET_VISIBLE)]
    unsafe extern "thiscall" fn set_visible(this: *const u32, visible: bool) {
        ZTAnimal(this as u32).set_visible(visible);
    }

    #[detour(CAN_CAPTURE)]
    unsafe extern "thiscall" fn can_capture(this: *const c_void, other: i32) -> bool {
        ZTAnimal(this as u32).can_capture(other as u32)
    }

    #[detour(IS_NEED_SATISFIER)]
    unsafe extern "thiscall" fn is_need_satisfier(_this: *const u32, tile: *const u32, pos: *const IVec3) -> bool {
        ZTAnimal::is_need_satisfier(tile as u32, pos as u32)
    }

    #[detour(CAN_SERVICE)]
    unsafe extern "thiscall" fn can_service(this: *const u32, staff: *const u32) -> bool {
        ZTAnimal(this as u32).can_service(staff as u32)
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

        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn decide(this: u32) -> u32 {
            unsafe { super::DECIDE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn into_new_tile(this: u32, direction: u32) {
            unsafe { super::INTO_NEW_TILE_DETOUR.call(this as *const u32, direction) }
        }

        pub(crate) fn f_get_mate(this: u32, found: u32) -> i32 {
            unsafe { super::F_GET_MATE_DETOUR.call(this as *const u32, found as *const bool) }
        }

        pub(crate) fn add_face(this: u32, smile: bool) {
            unsafe { super::ADD_FACE_DETOUR.call(this as *const u32, smile) }
        }

        pub(crate) fn check_prey(this: u32, target: u32, force: bool) -> u8 {
            (unsafe { super::CHECK_PREY_DETOUR.call(this as *const u32, target as *const u32, force) }) as u8
        }

        pub(crate) fn stop_eating(this: u32, flag: u8) {
            unsafe { super::STOP_EATING_DETOUR.call(this as *const u32, flag != 0) }
        }

        pub(crate) fn chase_done(this: u32, unit: u32) -> bool {
            unsafe { super::CHASE_DONE_DETOUR.call(this as *const u32, unit as *const u32) }
        }

        pub(crate) fn f_check_reproduction(this: u32) -> bool {
            unsafe { super::F_CHECK_REPRODUCTION_DETOUR.call(this as *const i32) }
        }

        pub(crate) fn consume(this: u32, unit: u32) {
            unsafe { super::CONSUME_DETOUR.call(this as *const u32, unit as *const i32) }
        }

        pub(crate) fn show_escaped_animal_alert(this: u32) {
            unsafe { super::SHOW_ESCAPED_ANIMAL_ALERT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn hatch_egg(this: u32) {
            unsafe { super::HATCH_EGG_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_eat(this: u32) -> u32 {
            (unsafe { super::F_EAT_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn put_down_dirt(this: u32, tile: u32) -> bool {
            unsafe { super::PUT_DOWN_DIRT_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn stop_chase(this: u32, force: bool) {
            unsafe { super::STOP_CHASE_DETOUR.call(this as *const u32, force) }
        }

        pub(crate) fn box_animal(this: u32) -> u32 {
            (unsafe { super::BOX_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn unbox(this: u32) -> u32 {
            (unsafe { super::UNBOX_DETOUR.call(this as *const u32) }) as u32
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
            (unsafe { super::REMOVE_FOOD_TARGET_FOR_ALL_DETOUR.call(this as *const u32, food as *const u32) }) as u32
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
            (unsafe { super::ADD_CHANGES_DETOUR.call(this as *const u32, changes as *const u32) }) as u32
        }

        pub(crate) fn do_food_trick(this: u32, word_a: u16, word_b: u16) -> u32 {
            unsafe { super::DO_FOOD_TRICK_DETOUR.call(this as *const u32, word_a, word_b) }
        }

        pub(crate) fn check_do_trick(this: u32) -> u32 {
            (unsafe { super::CHECK_DO_TRICK_DETOUR.call(this as *const u32) }) as u32
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

        pub(crate) fn do_ambient_anims(this: u32) {
            unsafe { super::DO_AMBIENT_ANIMS_DETOUR.call(this as *const u32, std::ptr::null()) }
        }

        pub(crate) fn do_baby_to_adult_check(this: u32) {
            unsafe { super::DO_BABY_TO_ADULT_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_reproduce_check(this: u32) {
            unsafe { super::DO_REPRODUCE_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_health_check(this: u32) {
            unsafe { super::DO_HEALTH_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_boxed_check(this: u32) {
            unsafe { super::DO_BOXED_CHECK_DETOUR.call(this as *const u32) };
        }

        pub(crate) fn do_sleep_check(this: u32) {
            unsafe { super::DO_SLEEP_CHECK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_zap_check(this: u32) {
            unsafe { super::DO_ZAP_CHECK_DETOUR.call(this as *const u32) };
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
            (unsafe { super::IS_HUNGRY_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn is_hungry_and_foodless(this: u32) -> u32 {
            (unsafe { super::IS_HUNGRY_AND_FOODLESS_DETOUR.call(this as *const u32) }) as u32
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
            (unsafe { super::IS_UNHAPPY_WITH_HABITAT_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn is_very_unhappy_with_habitat(this: u32) -> u32 {
            (unsafe { super::IS_VERY_UNHAPPY_WITH_HABITAT_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn is_sickly(this: u32) -> u32 {
            (unsafe { super::IS_SICKLY_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn is_egg(this: u32) -> u8 {
            (unsafe { super::IS_EGG_DETOUR.call(this as *const c_void) }) as u8
        }

        pub(crate) fn is_selectable(this: u32) -> bool {
            unsafe { super::IS_SELECTABLE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn preys_on_man(this: u32) -> bool {
            unsafe { super::PREYS_ON_MAN_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_in_prey_list(this: u32, id: i32) -> u32 {
            (unsafe { super::IS_IN_PREY_LIST_DETOUR.call(this as *const u32, id) }) as u32
        }

        pub(crate) fn is_preattack(this: u32, other: u32) -> u32 {
            (unsafe { super::IS_PREATTACK_DETOUR.call(this as *const u32, other as i32) }) as u32
        }

        pub(crate) fn within_preattack_range(this: u32, other: u32) -> u32 {
            (unsafe { super::WITHIN_PREATTACK_RANGE_DETOUR.call(this as *const u32, other as i32) }) as u32
        }

        pub(crate) fn get_ambient_key(this: u32) -> u32 {
            unsafe { super::GET_AMBIENT_KEY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn diagonal_ok(this: u32) -> bool {
            unsafe { super::DIAGONAL_OK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_walkable(this: u32) -> u32 {
            (unsafe { super::IS_WALKABLE_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn add_to_map(this: u32) -> bool {
            unsafe { super::ADD_TO_MAP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn remove_from_map(this: u32, flag: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn set_dying(this: u32, flag: bool) {
            unsafe { super::SET_DYING_DETOUR.call(this as *const u32, flag) }
        }

        pub(crate) fn set_name(this: u32, name: u32) {
            unsafe { super::SET_NAME_DETOUR.call(this as *const u32, name as *const u32) }
        }

        pub(crate) fn init_status_vars(this: u32) {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
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

        pub(crate) fn stop_everything(this: u32, a: bool, b: bool, c: bool) {
            unsafe { super::STOP_EVERYTHING_DETOUR.call(this as *const u32, a, b, c) }
        }

        pub(crate) fn invalidate_target(this: u32) {
            unsafe { super::INVALIDATE_TARGET_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn break_escape_fence(this: u32) {
            unsafe { super::BREAK_ESCAPE_FENCE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_reached_escape_tile(this: u32) {
            unsafe { super::SET_REACHED_ESCAPE_TILE_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn set_reached_escape_transition_tile(this: u32) {
            unsafe { super::SET_REACHED_ESCAPE_TRANSITION_TILE_DETOUR.call(this as i32) }
        }

        pub(crate) fn set_reached_escape_destination_tile(this: u32) {
            unsafe { super::SET_REACHED_ESCAPE_DESTINATION_TILE_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn tile_filter(this: u32, tile: u32, flag: bool) -> bool {
            unsafe { super::TILE_FILTER_DETOUR.call(this as *const c_void, tile as *const c_void, flag) }
        }

        pub(crate) fn fences_ok(this: u32, tile: u32, direction: i32, out_a: u32, out_b: u32) -> bool {
            unsafe { super::FENCES_OK_DETOUR.call(this as *const u32, tile as *const u32, direction as u32, out_a as *const bool, out_b as *const bool) as u8 != 0 }
        }

        pub(crate) fn evaluate_escape(this: u32) {
            unsafe { super::EVALUATE_ESCAPE_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn pick_random_dest(this: u32, position: u32) -> bool {
            unsafe { super::PICK_RANDOM_DEST_DETOUR.call(this as *const u32, position as *const glam::i32::IVec3) as u8 != 0 }
        }

        pub(crate) fn get_feeding_path(this: u32, destination: u32, shortcut: bool) -> bool {
            unsafe { super::GET_FEEDING_PATH_DETOUR.call(this as *const c_void, destination as *const i32, shortcut as u8) as u8 != 0 }
        }

        pub(crate) fn get_terrain_cost(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_TERRAIN_COST_DETOUR.call(this as *const u32, tile as *const u32) as i32 }
        }

        pub(crate) fn set_terrain_mode(this: u32, mode: i32, flag: i8) {
            unsafe { super::SET_TERRAIN_MODE_DETOUR.call(this as *const u32, mode, flag) }
        }

        pub(crate) fn find_prey(this: u32, force: bool) -> i32 {
            unsafe { super::FIND_PREY_DETOUR.call(this as *const u32, force) }
        }

        pub(crate) fn find_place_to_rest(this: u32, nearest: bool) -> bool {
            unsafe { super::FIND_PLACE_TO_REST_DETOUR.call(this as *const u32, nearest) as u8 != 0 }
        }

        pub(crate) fn target_building(this: u32, nearest: bool) -> bool {
            unsafe { super::TARGET_BUILDING_DETOUR.call(this as *const u32, nearest) }
        }

        pub(crate) fn clear_prey(this: u32) {
            unsafe { super::CLEAR_PREY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn no_shadow_part(this: u32) -> u32 {
            (unsafe { super::NO_SHADOW_PART_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn needs_breath(this: u32) -> u32 {
            (unsafe { super::NEEDS_BREATH_DETOUR.call(this as *const c_void) }) as u32
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

        pub(crate) fn get_idle_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_IDLE_ANIM_SYMBOL_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn get_only_swims(this: u32) -> bool {
            unsafe { super::GET_ONLY_SWIMS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_move_anim(this: u32, name: u32) -> u32 {
            unsafe { super::GET_MOVE_ANIM_DETOUR.call(this as *const u32, name as *const i8) as u32 }
        }

        pub(crate) fn compare_lt(this: u32, other: u32) -> bool {
            unsafe { super::COMPARE_LT_DETOUR.call(this as *const c_void, other as *const u8) }
        }

        pub(crate) fn set_max_health(this: u32) {
            unsafe { super::SET_MAX_HEALTH_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn is_doing_something(this: u32) -> u32 {
            (unsafe { super::IS_DOING_SOMETHING_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn get_swims(this: u32) -> u32 {
            (unsafe { super::GET_SWIMS_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn get_underwater(this: u32) -> u32 {
            (unsafe { super::GET_UNDERWATER_DETOUR.call(this as *const c_void) }) as u32
        }

        pub(crate) fn is_walkable_by(this: u32, other: u32) -> u32 {
            (unsafe { super::IS_WALKABLE_BY_DETOUR.call(this as *const u32, other as *const u32) }) as u32
        }

        pub(crate) fn cliffs_ok(this: u32, tile: u32, direction: u32) -> bool {
            unsafe { super::CLIFFS_OK_DETOUR.call(this as *const u32, tile as *const u32, direction) }
        }

        pub(crate) fn get_prey_unit(this: u32) -> i32 {
            unsafe { super::GET_PREY_UNIT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn f_mate_gone(this: u32) -> u32 {
            (unsafe { super::F_MATE_GONE_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn f_other_animal_angry(this: u32) -> u32 {
            (unsafe { super::F_OTHER_ANIMAL_ANGRY_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn f_other_animal_sick(this: u32) -> u32 {
            (unsafe { super::F_OTHER_ANIMAL_SICK_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn f_animal_space_ratio(this: u32) -> f32 {
            unsafe { super::F_ANIMAL_SPACE_RATIO_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_food_units_actually_eaten(this: u32) -> i32 {
            unsafe { super::F_FOOD_UNITS_ACTUALLY_EATEN_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn f_is_there_keeper_food(this: u32) -> u32 {
            (unsafe { super::F_IS_THERE_KEEPER_FOOD_DETOUR.call(this as *const u32) }) as u32
        }

        pub(crate) fn check_done(this: u32) -> u32 {
            unsafe { super::CHECK_DONE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn drop_target(this: u32) {
            unsafe { super::DROP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn set_visible(this: u32, visible: bool) {
            unsafe { super::SET_VISIBLE_DETOUR.call(this as *const u32, visible) }
        }

        pub(crate) fn can_capture(this: u32, other: u32) -> u32 {
            (unsafe { super::CAN_CAPTURE_DETOUR.call(this as *const c_void, other as i32) }) as u32
        }

        pub(crate) fn is_need_satisfier(this: u32, tile: u32, pos: u32) -> u32 {
            (unsafe { super::IS_NEED_SATISFIER_DETOUR.call(this as *const u32, tile as *const u32, pos as *const glam::i32::IVec3) }) as u32
        }

        pub(crate) fn can_service(this: u32, staff: u32) -> u32 {
            (unsafe { super::CAN_SERVICE_DETOUR.call(this as *const u32, staff as *const u32) }) as u32
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
            (unsafe { super::GET_KEEP_MOVING_DETOUR.call(this as *const c_void) }) as u32
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
