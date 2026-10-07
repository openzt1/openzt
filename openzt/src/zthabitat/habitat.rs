use nt_time::{time::UtcDateTime, FileTime};
use openzt_detour::generated::{
        bfaimgr::CHECK_PATH as BFAIMGR_CHECK_PATH,
        bfcategory::GET_VALUE as BFCATEGORY_GET_VALUE,
        bfworldmgr::VERIFY_ENTITY_0 as BFWORLDMGR_VERIFY_ENTITY_0,
        bfentity::{GET_GRID_POS as BFENTITY_GET_GRID_POS, GET_TILE as BFENTITY_GET_TILE},bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, IS_CLOSE_DIRECTION as BFMAP_IS_CLOSE_DIRECTION},
        bftile::{
            IS_IN_ZOO as BFTILE_IS_IN_ZOO, VALIDATE_POSITIONS as BFTILE_VALIDATE_POSITIONS,
        },
        msvc_std_basic_string::BASIC_STRING_0 as MSVC_BASIC_STRING_DTOR,
        msvc_std_listuint::{INSERT_RANGE as MSVC_LIST_UINT_INSERT_RANGE, LIST as MSVC_LIST_UINT_DTOR},
        msvc_std_vector_t_4::VECTOR_T_4 as MSVC_VECTOR_T_4_DTOR,
        msvc_std_tree36::{CLEAR as MSVC_TREE36_CLEAR, OPERATOR_INDEX as MSVC_TREE36_OPERATOR_INDEX},
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW, TILE_WITHIN_AVA},
        ztanimal::{
            CAN_SERVICE, IS_HUNGRY as ZTANIMAL_IS_HUNGRY, IS_HUNGRY_AND_FOODLESS, IS_SICKLY,
            IS_UNHAPPY_FOR_REPRODUCTION as ZTANIMAL_IS_UNHAPPY_FOR_REPRODUCTION, SET_FOOD, SET_KEEPER_ARRIVES, STOP_EATING,
        },
        ztfence::{MAKE_FENCE as ZTFENCE_MAKE_FENCE, MAKE_GATE as ZTFENCE_MAKE_GATE},
        zthabitat::{
            ADD_FOUND_SPECIES, CREATE_VIEWING_AREAS, GET_EVENTS,
            REVISE_SPECIES_LIST, SEND_EVENT,
            SPECIES_SUITABILITY_CACHE_CLEAR, SPECIES_SUITABILITY_CACHE_DTOR,
        },
        zthabitatmgr::REMOVE_HABITAT_0,
        ztkeeper::CLEANS_UP,
        ztapp::GET_WORLD_MGR as ZTAPP_GET_WORLD_MGR,
        ztspecies::IS_SPECIAL_DUMMY_SPECIES,
        ztstaff::IS_HABITAT_ASSIGNED as ZTSTAFF_IS_HABITAT_ASSIGNED,
        ztworldmgr::GET_STAFF_LIST as ZTWORLDMGR_GET_STAFF_LIST,
        ztshowinfo::{CONSTRUCTOR_1 as ZTSHOWINFO_CONSTRUCTOR, DESTRUCTOR_1 as ZTSHOWINFO_DESTRUCTOR},
        ztshowmgr::{REGISTER_SHOW, UNREGISTER_SHOW},
        ztui_showpanel::SET_EXHIBIT,
        ztunit::GET_HABITAT as ZTUNIT_GET_HABITAT,
        ztviewingarea::{
            ADD_TILE as ZTVIEWINGAREA_ADD_TILE, CONSTRUCTOR as ZTVIEWINGAREA_CONSTRUCTOR, DESTRUCTOR as ZTVIEWINGAREA_DESTRUCTOR,
            GET_EWEXTENT as ZTVIEWINGAREA_GET_EWEXTENT, GET_NSEXTENT as ZTVIEWINGAREA_GET_NSEXTENT,
            RECALCULATE_CHARACTERISTICS as ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS, REMOVE_TILE as ZTVIEWINGAREA_REMOVE_TILE,
            UPDATE_AMBIENTS as ZTVIEWINGAREA_UPDATE_AMBIENTS,
        },
    };
use getset::Getters;
use std::{collections::HashMap, fmt, mem::offset_of};

use crate::{
    ambients::Ambients,
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory, write_live_ptr, ZTBufferString, ZTString},
    vanilla_vector::{VanillaEventVector, VanillaVector},
    write_live,
    zoostatus::ZooStatus,
    ztmapview::BFTile,
    ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG},
    ztshow::{call_entity_vtable_noargs, call_entity_vtable_u32_noargs, type_check, RVA_ANIMAL_TYPE_CHECK},
    ztshowinfo,
    ztworldmgr::Direction,
};
use super::mgr::zthabitatmgr::ZTHabitatMgr;

/// Number of times [`ZTHabitat::destruct`] has run - lets the live battery prove the destructor port was
/// actually reached by vanilla teardown paths.
#[cfg(feature = "reimplementation-tests")]
pub(crate) static DESTRUCT_CALLS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
use super::support::{self, *};
use super::tank_exhibit::ZTTankExhibit;

/// Water-clean threshold setting (`DAT_006390a4`) `needsService` compares a tank's `+0x1a8` against. RVA = address - `0x400000`.
const RVA_WATER_CLEAN_THRESHOLD: u32 = 0x006390a4 - 0x400000;

/// `ZTAnimal::FUN_004d9a2f(smile)` (not in `generated.rs`): queues a smile/frown face on the animal.
/// Reached through `.hooked()` by [`ZTHabitat::generate_faces`] so the live battery's recorder can
/// observe the per-animal requests without spawning faces.
pub(crate) const ANIMAL_SHOW_FACE: openzt_detour::FunctionDef<unsafe extern "thiscall" fn(*const u32, bool)> = openzt_detour::FunctionDef::new(0x004d9a2f);

/// `f32` weights `findBestRating` applies to the five `get*Suitability` terms of a land habitat
/// (`DAT_00635408`/`0063540c`/`00635480`/`00635484`/`00635488`). RVA = address - `0x400000`.
const RVA_SUITABILITY_WEIGHT_TERRAIN: u32 = 0x0023_5408;
const RVA_SUITABILITY_WEIGHT_OBJECT: u32 = 0x0023_540c;
const RVA_SUITABILITY_WEIGHT_FOLIAGE: u32 = 0x0023_5480;
const RVA_SUITABILITY_WEIGHT_ROCK: u32 = 0x0023_5484;
const RVA_SUITABILITY_WEIGHT_ELEVATION: u32 = 0x0023_5488;
const RVA_SUITABILITY_WEIGHT_SHELTER: u32 = 0x0023_548c;
const RVA_SUITABILITY_WEIGHT_LAND_TOY: u32 = 0x0023_5410;

/// `getHabitatRating`'s `f32` constants: the rating of a null animal/species type
/// (`DAT_00630d5c`), the penalty for a tank rated for an amphibious animal (`DAT_00635544`), and the
/// seed of the max-over-neighbours search (`0xc47a0000`, `-1000.0`).
const RVA_RATING_NO_ANIMAL: u32 = 0x0023_0d5c;
const RVA_RATING_AMPHIBIOUS_TANK_PENALTY: u32 = 0x0023_5544;
const NEIGHBOR_RATING_SEED: f32 = -1000.0;

/// Weights `getHabitatRating` applies to the depth/cleanliness/salinity/object/toy terms of a tank
/// (the foliage/rock/elevation/shelter weights are shared with the land sum): show tank
/// (`DAT_00635018..28`) and ordinary tank (`DAT_006354d0..e0`).
struct TankRatingWeights {
    depth: u32,
    cleanliness: u32,
    salinity: u32,
    object: u32,
    toy: u32,
}
const SHOW_TANK_RATING_WEIGHTS: TankRatingWeights =
    TankRatingWeights { depth: 0x0023_5018, cleanliness: 0x0023_501c, salinity: 0x0023_5020, object: 0x0023_5024, toy: 0x0023_5028 };
const PLAIN_TANK_RATING_WEIGHTS: TankRatingWeights =
    TankRatingWeights { depth: 0x0023_54d0, cleanliness: 0x0023_54d4, salinity: 0x0023_54d8, object: 0x0023_54dc, toy: 0x0023_54e0 };

/// The accumulation `getHabitatRating` performs. The game runs with the x87 control word at 24-bit
/// precision (Direct3D's default), so every `FMUL`/`FADD`/`FSUB` result is already rounded to `f32`;
/// plain `f32` arithmetic reproduces it (a wider intermediate is off by one ULP).
fn x87_weighted_sum(terms: &[(f64, f64)]) -> f64 {
    let mut sum = 0.0f32;
    for (value, weight) in terms {
        let product = *value as f32 * *weight as f32;
        sum = product + sum;
    }
    sum as f64
}

#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTHabitat {
    pub vtable: u32,                 // 0x000
    pub zt_show_info_ptr: u32,       // 0x004
    pub amphibious_neighbors_head: u32, // 0x008 // MSVC `std::set<ZTHabitat*>` head/sentinel node pointer for the amphibious-neighbor set - confirmed red-black-tree node layout (`+0x0`=color/isnil, `+0x4`=parent, `+0x8`=left, `+0xc`=right, `+0x10`=value) via `ZTHabitat_hiliteAmphibiousNeighbors.c`'s own in-order walk (see `walk_neighbor_tree`) and `addAmphibiousNeighbor`'s own STL insert helper (both left un-ported - see `Self::hilite_amphibious_neighbors`'s own doc comment). `ZTHabitat_getSize.c` independently walks this exact same field with identical node arithmetic (see `Self::get_size`) - this corrects `zthabitatmgr-implementation-plan.md`'s own earlier step 6h note (which speculated this was an unrelated nested-sub-habitat tree).
    pub pad1a_a1: [u8; 0x8],          // ----------------------- padding: 8 bytes
    pub show_neighbors_head: u32,    // 0x014 // Same shape as `amphibious_neighbors_head`, for the show-neighbor set (`ZTHabitat_hiliteShowNeighbors.c`/`addShowNeighbor`/`clearShowNeighbors`). The C decompiles mislabel this field `zoo_entrance_y` (an OOAnalyzer type-propagation artifact bleeding in a `ZTHabitatMgr`-shaped name) - trust the `.asm`-confirmed `+0x14` offset, named here for what it actually is.
    pub pad1a_a2a: [u8; 0x8],         // ----------------------- padding: 8 bytes
    pub show_portal_map_head: u32,   // 0x020 // MSVC `std::map<ZTHabitat*, ZTFence*>` head/sentinel node pointer for the per-neighbor show-portal cache - same red-black-tree node layout as `amphibious_neighbors_head` (`+0x0`=color/isnil, `+0x4`=parent, `+0x8`=left, `+0xc`=right, `+0x10`=key, `+0x14`=value), confirmed via `ZTHabitat_getShowPortal.asm`/`.c` (`0x0059e0a9`, this file's `Self::get_show_portal`) reading `&this->field_0x20` as the tree object and passing it directly to `AI_cls_0x404fd6::find`/`msvc_std::tree24::insert`, and independently corroborated by the macOS `ZTHabitat_getShowPortal.c`/`_addShowPortal.c`/`_removeShowPortal.c` decompiles' own `map<P9ZTHabitat,P7ZTFence>`-typed tree at the equivalent field - the plan's original sketch called this a `ZTShowInfo*`-valued cache, but every real accessor stores/returns a `ZTFence*` (the tile-boundary fence entity acting as the show portal), not a `ZTShowInfo*`. Populated by `addShowPortal`/`removeShowPortal` (both left un-ported, real vanilla calls only) - see `Self::get_show_portal`'s own doc comment for why this is a pure read.
    pub pad1a_a2b: [u8; 0x1],         // ----------------------- padding: 1 byte
    pub neighbor_dirty: u8,          // 0x025 // Set to 1 by `ZTHabitatMgr::habitatTileChanged`/`sceneryEntityChange` on every cached neighbor-habitat pointer found in a changed tile's own grid-cell row (see `ZTHabitatMgr::habitat_tile_changed`), and by `ZTHabitat::recreateOAs`/`ZTHabitatMgr::pathRemoved` (`Self::recreate_oas`/`ZTHabitatMgr::path_removed`). No reader identified in this pass - real consumer not yet found in the decompile corpus.
    pub pad1a_b: [u8; 0x6],          // ----------------------- padding: 6 bytes
    pub unknown_flag_0x2c: u8,       // 0x02c // Gates ZTThought::ZTThought's acceptance of a passed-in habitat pointer (see ztthoughtmgr.rs); ZTHabitat::recalculateCharacteristics also early-returns when this is set. Meaning not otherwise confirmed.
    pub characteristics_dirty: u8,   // 0x02d // Gates the lazy `recalculateCharacteristics` call in getAttractiveness/hasKeeperAssigned (see ZTHabitat_getAttractiveness.c/ZTHabitat_hasKeeperAssigned.c) - distinct from unknown_flag_0x2c above.
    pub reentrancy_guard: u8,        // 0x02e // ZTHabitat::recalculateCharacteristics's own reentrancy guard: set to 1 on entry, cleared to 0 on every exit path. See `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 5 note on why this matters - the ten per-factor suitability getters phase 4 calls through to each independently re-check `characteristics_dirty` and could otherwise recurse back into this function via another call path.
    pub show_unit_scan_pending: u8,  // 0x02f // One-shot gate for recalculateCharacteristics's phase-2 per-tile `addShowUnit` scan - checked (and implicitly consumed) alongside a per-animal `initStatusVars` vtable check, cleared unconditionally at the end of the function.
    pub unknown_flag_0x30: u8,       // 0x030 // Set to `1` by `ZTHabitatMgr::fenceRemoved` (`Self::fence_removed`) on either side of a removed fence whenever that side's own `unknown_flag_0x2c` is clear (i.e. it's a real, non-"world" habitat) - a third distinct flag alongside `unknown_flag_0x2c`/`characteristics_dirty`, never cleared or read anywhere in this pass's own scope, so its real meaning/consumer is unconfirmed. `ZTHabitat::recalculateCharacteristics` is a second, independent writer of this same field - its own "characteristics changed" output flag, set (in [`Self::recalc_phase_3`]) when `species_found_count` changes across the function's species-propagation phase, and separately when `num_animals` changes across phases 1-3 (which also, if the new `num_animals` is `0`, triggers `sendMaintWorkerCleanupEvents` unless a save is loading, and zeroes `unknown_nt_time`).
    pub species_list_dirty: u8,      // 0x031 // Gates the lazy `reviseSpeciesList` call in update() once species_list_timer crosses its threshold - same dirty-flag/timer shape as characteristics_dirty/characteristics_timer, cleared by real vanilla reviseSpeciesList's own (still un-ported) body as a side effect.
    pub pad1b_b: [u8; 0x2],          // ----------------------- padding: 2 bytes
    pub viewing_areas_begin: u32,    // 0x034 // Begin pointer of the real vanilla std::vector<ZTViewingArea*> update() walks to tick each entry's own ambient state.
    pub viewing_areas_end: u32,      // 0x038
    pub viewing_areas_cap_end: u32,  // 0x03c // The vector's own capacity end - written by `ZTHabitat::addViewingArea`'s own growth path (see `Self::add_viewing_area`), previously undifferentiated padding.
    pub owned_tiles_ptr: u32,        // 0x040 // Pointer to the sentinel node of this habitat's owned-tile list (see TileListNode below), not a BFTile* itself - see getSize/removeHabitatTiles/validatePositions/resetUnitAI/createEdgePairs, all of which walk it identically.
    pub pad2a1: [u8; 0x4],            // ----------------------- padding: 4 bytes (0x044 - a real field per `ZTHabitat_createViewingAreas.c`'s own use of it as a second tile-list-shaped container, but that function is deferred - see `Self::create_viewing_areas`'s absence - so this stays unidentified padding rather than a guessed name/shape)
    pub boundary_tile_pairs_begin: u32, // 0x048 // Vanilla's `std::vector<std::pair<BFTile*,BFTile*>>` begin/end/cap_end triple, zeroed by the vanilla constructor and never written by the reimplementation: the boundary pairs live in a Rust-side store ([`Self::boundary_tile_pairs`]). Real vanilla `createEdgePairs` is the only writer (live tests only); `Self::destruct` frees any buffer it allocated.
    pub boundary_tile_pairs_end: u32,   // 0x04c
    pub boundary_tile_pairs_cap_end: u32, // 0x050
    pub ambients_begin: u32,         // 0x054 // Begin pointer of the real vanilla std::vector<(u32, Ambients*)> update() walks to play each entry's own ambient sound - see viewing_areas_begin's own doc comment for the same only-begin/end-modeled reasoning.
    pub ambients_end: u32,           // 0x058
    pub pad2b_a: [u8; 0x4],          // ----------------------- padding: 4 bytes (cap_end of the ambients vector above)
    pub species_list_begin: u32,     // 0x060 // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSpeciesList exposes (`&this->field_0x60`) - lazily recalculated via the same characteristics_dirty gate as get_attractiveness/has_keeper_assigned. See Self::species_list.
    pub species_list_end: u32,       // 0x064
    pub pad2b_b1: [u8; 0x4],         // ----------------------- padding: 4 bytes (cap_end of the species-list vector above)
    pub all_animals_begin: u32,      // 0x06c // Begin pointer of the real vanilla std::vector<ZTAnimal*> ZTHabitat::getAllAnimals exposes (`&this->field_0x6c`, `.asm`-confirmed `LEA EAX,[ESI+0x6c]`) - unlike species_list_begin/end this vector is never recalculated here, only optionally sorted in place by Self::get_all_animals.
    pub all_animals_end: u32,        // 0x070
    pub all_animals_cap_end: u32,    // 0x074 // The animals vector's own capacity end - never observed written by any function ported so far (`get_all_animals`'s own sort never grows it), carried here for completeness alongside the vector's other two fields.
    pub building_list_begin: u32,    // 0x078 // Begin pointer of a real vanilla std::vector<BFEntity*> - `ZTHabitat::hasBldg` (`Self::has_bldg`) reads it; `ZTHabitat::addToBuildingList` (`Self::add_to_building_list`) writes it, appending onto another habitat's own list of this field's namesake.
    pub building_list_end: u32,      // 0x07c
    pub building_list_cap_end: u32,  // 0x080 // The building-list vector's own capacity end - never written by any function ported so far.
    pub characteristics_timer: u32,  // 0x084 // Elapsed-time accumulator update() advances every tick; past 6999 sets characteristics_dirty and rerolls to a random 0..200 value via the shared game RNG.
    pub species_list_timer: u32,     // 0x088 // Same shape as characteristics_timer, gating species_list_dirty/reviseSpeciesList at threshold 7999.
    pub entrance_tile_ptr: u32,      // 0x08c
    pub entrance_rotation: u32,      // 0x090
    pub num_animals: i32,            // 0x094 // ZTHabitat::getNumAnimals's own cached direct-occupant count (`.asm`-confirmed `MOV EAX,[EDI+0x94]`) - zeroed and re-tallied by recalculateCharacteristics's own animal census (one increment per owned-tile occupant passing the ZTAnimal cast and getTile match, per ZTHabitat_recalculateCharacteristics.c), so a dirty-flags recalculate refreshes it along with the other cached tallies.
    pub num_angry_animals: i32,      // 0x098 // ZTHabitat::getNumAngryAnimals's cached angry-animal count (`.asm`-confirmed `MOV %EBX,[EDI+0x98]`) - zeroed and re-tallied by recalculateCharacteristics's own animal census (one increment per animal whose `+0x3aa` flag byte is set, per ZTHabitat_recalculateCharacteristics.c). See Self::get_num_angry_animals.
    pub unhappy_for_reproduction_count: i32, // 0x09c // recalculateCharacteristics's own owned-tile census tally: one increment per animal `ZTAnimal::isUnhappyForReproduction` reports true for. No reader found anywhere in the corpus yet.
    pub num_sick_animals: i32,       // 0x0a0 // ZTHabitat::getNumSickAnimals's cached sick-animal count - zeroed and re-tallied by the same recalculateCharacteristics census pass (one increment per animal whose `+0x3a7` flag byte is set). See Self::get_num_sick_animals.
    pub hungry_count: i32,           // 0x0a4 // recalculateCharacteristics's own owned-tile census tally: one increment per animal `ZTAnimal::isHungry` reports true for. No reader found anywhere in the corpus yet.
    pub keeper_food_category_amounts: [i32; 16], // 0x0a8 // Per-category cached "amount of keeper food left out" tally (`ZTHabitat_getAmountKeeperFood.c`'s own `&this->field_0xa8 + category * 4`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same `18-factor per-tile suitability` recalculation `zthabitatmgr-implementation-plan.md`'s step 6m documents as this function's own writer. See Self::get_amount_keeper_food.
    pub avg_animal_happiness: i32,   // 0x0e8 // ZTHabitat::getAvgAnimalHappiness's cached average animal happiness (`.asm`-confirmed `MOV %EAX,[ESI+0xe8]`) - zeroed (when num_animals is 0) or set to sum-of-happiness/num_animals by recalculateCharacteristics's own animal census (one `animal+0x2a8` happiness read per animal - `ZTHabitat_recalculateCharacteristics.asm`'s `MOV %EDX,[ECX+0x2a8]` - accumulated into per-species suitability records and totalled before the divide, per ZTHabitat_recalculateCharacteristics.c). See Self::get_avg_animal_happiness.
    pub time_last_serviced: u32,     // 0x0ec // ZTHabitat::setTimeLastServiced's own written field (`this->mbr_0xec`, `.asm`-confirmed) - a timestamp, presumably compared against ZTScenarioTimer to gate keeper-service scheduling, though no reader was found anywhere in this pass's own scope (see Self::set_time_last_serviced).
    pub num_keepers: i32,            // 0x0f0 // Cached count of assigned keepers (subtype `0x6e`) tallied by recalculateCharacteristics's own owned-tile census, per `ZTHabitat_getNumKeepers.c`'s `*(int*)&this->field_0xf0`. See Self::get_num_keepers.
    pub scheduled_service_counter: i32, // 0x0f4 // `ZTHabitat::setScheduledForService`'s own counter (`.asm`-confirmed `MOV %EAX,[ECX+0xf4]`; the macOS decompile reads its own `+0x108`): `true` → +1, `false` → -1 clamped at 0. `triggerKeeperArrived`'s inlined `setScheduledForService(this, false)` is its other writer - always the decrement arm, since real vanilla hardcodes the bool clear (see `Self::trigger_keeper_arrived`). See `ZTHabitat_setScheduledForService.c`.
    pub attractiveness: i32,         // 0x0f8 // ZTHabitat::getAttractiveness's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    pub current_donations: f32,     // 0xfc
    pub last_donations: f32,        // 0x100
    pub total_donations: f32,       // 0x104
    pub current_upkeep: f32,         // 0x108
    pub last_upkeep: f32,            // 0x10c
    pub total_upkeep: f32,           // 0x110
    pub unknown_u32_2: u32,          // 0x114
    pub unknown_u32_3: u32,          // 0x118
    pub unknown_u32_4: u32,          // 0x11c
    pub created_timestamp: FileTime, // 0x120
    pub unknown_nt_time: FileTime,   // 0x128 // Zeroed by `ZTHabitat::recalculateCharacteristics` ([`Self::recalc_phase_3`]) whenever `num_animals` changes to `0` across the census - real vanilla writes both raw dwords of this field directly (`this->mbr_0x128 = 0; this->mbr_0x12c = 0;`), not through any named setter. No reader found in this pass's own scope.
    pub unknown_flag_0x130: u8,      // 0x130 // Zeroed unconditionally by recalculateCharacteristics's own reset block, right before `species_found_count`. No reader found anywhere in the corpus; real meaning unconfirmed.
    pub has_keeper_assigned_raw: u8, // 0x131 // ZTHabitat::hasKeeperAssigned's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    pub is_being_serviced_raw: u8,   // 0x132 // ZTHabitat::isBeingServiced's cached result (`this->mbr_0x132`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same shape as has_keeper_assigned_raw. See Self::is_being_serviced.
    pub pad5b1b: [u8; 0x1],          // ----------------------- padding: 1 byte (0x133)
    pub deterioration: u32,          // 0x134 // Fence-deterioration level (`0` none, `1` minor, `2` major) - `ZTHabitat::setDeterioration` writes it and never lowers a `2` to a `1` (`ZTHabitat_setDeterioration.c`).
    pub species_found_count: u32,    // 0x138 // ZTHabitat::recalculateCharacteristics's own species-found counter: snapshotted before/after its recursive `addFoundSpecies` propagation over the show/amphibious neighbor sets, the before/after delta sets `unknown_flag_0x30` ("characteristics changed").
    pub surrounding_species_begin: u32, // 0x13c // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSurroundingSpecies exposes (`&this->field_0x13c`, `.asm`-confirmed `LEA EAX,[ESI+0x13c]`), same characteristics_dirty-gated lazy-recalculate shape as species_list_begin/end. Populated by ZTHabitat::constructSurroundingSpeciesList (Self::construct_surrounding_species_list): unions this habitat's own species_list with its amphibious neighbors' (gated on !is_tank() and each entry's own vtable+0xcc predicate) and show neighbors' (ungated) species lists, called internally by recalculateCharacteristics.
    pub surrounding_species_end: u32,   // 0x140
    pub pad5b2: [u8; 0x10],          // ----------------------- padding: 16 bytes (0x144-0x154: cap_end of the vector above at 0x144, then a msvc_std::map<int, ZTHabitatSuitabilityRecord> tree handle (node pointer + count) at 0x148/0x14c - ZTHabitat's own per-species suitability-scoring cache, keyed by ZTAnimalType::species and holding a 112-byte scoring record (elevation range, neighbor-habitat-tile count, per-scenery/building BFCategory scores, tank water-level min/max, ~15 sub-factors), whole-tree-replaced at the end of every recalculateCharacteristics pass (see ZTHabitat::speciesSuitabilityCache_alloc/_clear/_dtor and cls_0x40143b-disambiguation-handover.md). ZTHabitatSuitabilityRecord's own internal field layout is not yet reverse-engineered.)
    pub exhibit_name: ZTBufferString, // 0x154 // 3-pointer (start/end/buffer_end) buffer string - ZTHabitat::ZTHabitat zero-inits all of field_0x154/0x158/0x15c before allocating, and field_0x160 is a distinct, separately-referenced pointer (ZTHabitat::playShowStartSound etc.) right after it. Was previously mis-typed as the 2-pointer ZTBoundedString, which shifted every field below 4 bytes early.
    pub start_sound_ptr: u32,        // 0x160 // Real vanilla SNDSound* for the configured `[sounds] startSound`, built/acquired by set_is_show_exhibit and torn down by set_is_not_show_exhibit.
    pub end_sound_ptr: u32,          // 0x164 // Real vanilla SNDSound* for the configured `[sounds] endSound` - see start_sound_ptr.
    pub tank_walk_visited_marker: u8, // 0x168 // Scratch cycle-detection flag shared by `getOutermostTank`/`getNeedyNestedTank`'s own gate-chain/boundary-pair walks (`ZTHabitat_getOutermostTank.c`/`.asm`, `ZTHabitat_getNeedyNestedTank.c`/`.asm`): set to `1` on entry, checked before recursing into a candidate neighbour to stop a cycle. Real meaning outside these two calls unconfirmed; part of the ctor's own 0x168-0x178 zero-init range (see pad6's own note).
    pub pad6: [u8; 0xf],             // ----------------------- padding: 15 bytes (0x169-0x178: the real head is a msvc_std::tree36 sentinel at +0x16c the ctor zero-inits and ~ZTHabitat tears down via tree36::clear/_Erase - `ZTHabitat::checkEscapability`'s own per-species escapability cache, keyed by `entity_type_ptr` with a `{bool can_escape; ...; u32 close_outside_tile_ptr at +8}` value; [`Self::check_escapability`] addresses it directly (`self_addr + 0x16c`) via `MSVC_TREE36_CLEAR`/`MSVC_TREE36_OPERATOR_INDEX` rather than naming sub-fields here, since real vanilla's own tree functions own its internal layout)
    #[getset(skip)]
    pub _live: crate::util::LiveMemory,
}

// `ZTHabitatMgr::createHabitat` (`ZTHabitatMgr_createHabitat.c`) allocates a plain `ZTHabitat` with
// `operator_new(0x178)` and a `ZTTankExhibit` (single-inheritance subclass) with a *separate*
// `operator_new(0x1e8)` - i.e. `ZTHabitat` itself is only 0x178 bytes; `tank_height`/`water_level`/
// `is_filled` (used to live directly on this struct) only exist on the derived `ZTTankExhibit`, from
// 0x178 onward - see that struct below. Putting them here made every full-struct copy of a real,
// non-tank habitat (`get_from_memory::<ZTHabitat>`) over-read past its true 0x178-byte allocation.
const _: () = assert!(std::mem::size_of::<ZTHabitat>() == 0x178);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, deterioration) == 0x134);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, show_portal_map_head) == 0x020);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, reentrancy_guard) == 0x02e);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, show_unit_scan_pending) == 0x02f);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, unhappy_for_reproduction_count) == 0x09c);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, hungry_count) == 0x0a4);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, unknown_flag_0x130) == 0x130);
const _: () = assert!(std::mem::offset_of!(ZTHabitat, species_found_count) == 0x138);

/// One entry of `ZTHabitat::speciesSuitabilityCache` (`field_0x148`/`_0x14c`) - a per-species
/// suitability-scoring record `ZTHabitat::recalculateCharacteristics` builds in a function-local scratch
/// map (phase 4) and whole-map-replaces into the habitat's persistent cache (phase 6). Both live in the Rust
/// suitability store (`support.rs`), keyed by map handle; the vanilla header at `+0x148`/`+0x14c` stays an
/// empty, never-read sentinel. Layout
/// confirmed field-by-field against the live Ghidra project - see
/// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s "`ZTHabitatSuitabilityRecord`'s
/// 112-byte layout" table for the full evidence per field. The record keeps vanilla's `0x70`-byte layout
/// because the code reads fields by raw offset, not because any vanilla code reads it: every reader of the
/// cache is ported.
///
/// Field types/names/offsets confirmed directly against the live Ghidra project's own
/// `/auto_structs/ZTHabitatSuitabilityRecord` definition (`mcp__ghidra__types` `get`), not inferred from
/// the `.c` decompile's own rendering - several fields the decompile's arithmetic made look
/// float-shaped (`raw_percent_a`, `raw_building_count`) are real `int`s per Ghidra's own struct, fed by
/// `BFCategory::getValue`'s `i32` return. The record's own default constructor
/// (`ZTHabitatSuitabilityRecord::ZTHabitatSuitabilityRecord`, decompiled live) zero-inits every field
/// unconditionally, including `tank_depth_score`/`tank_or_visibility_score`/`tank_score_baseline` - the
/// per-species `100.0` resets of those fields are separate, later writes in phase 6, not part of
/// construction.
///
/// `unk_0x28`/`unk_0x2c`/`unk_0x40`/`unk_0x44` are Ghidra's own `undefined4` (raw, unresolved
/// dwords) - ported as `u32` per this codebase's established "unresolved field" convention rather than
/// guessing a real type.
///
/// **Stage 5 correction** (`setAnimalConditions`, `0x00447164`): this same struct is also OOAnalyzer's
/// own `ZTSpeciesAttribs` (identical `0x70`-byte size, identical field types/offsets everywhere checked,
/// just a second, independently-run identification pass over the same real vanilla type), confirmed live
/// via `mcp__ghidra__types get "ZTSpeciesAttribs"`. That pass recovered REAL field names for two spans
/// this pass had only guessed at:
/// - `0x48`/`0x4c` are not terrain-category overrides - they're `saltWaterTileAdjustment`/
///   `freshWaterTileAdjustment` (confirmed by `setAnimalConditions`'s own disassembly: category indices
///   9/10 of its per-category loop substitute these two fields in place of the normal tile-count array,
///   and the loop's percentage denominator is `occurrence_count + freshWaterTileAdjustment +
///   saltWaterTileAdjustment` - a population count, not a category-override sum).
/// - `flag_pack`'s 22 bytes are 22 individually-named `bool`s, not an opaque array - real field order
///   below (this pass's own guessed order, restated in the Stage 5 plan text, was wrong).
#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTHabitatSuitabilityRecord {
    pub occurrence_count: i32,          // 0x00 // Matching-species population count across owned tiles + neighbors.
    pub sum_unk_2a8: i32,               // 0x04 // Accumulated `animal+0x2a8`-sourced sum; real meaning gated on identifying `cls_0x62fa44` (unrelated, separate task).
    pub sum_unk_2b8: i32,               // 0x08 // Same shape as sum_unk_2a8, `animal+0x2b8`.
    pub terrain_type_score: f32,        // 0x0c // 18-category tile-tally score, `[0,100]` clamped.
    pub scenery_category_score: f32,    // 0x10 // Per-neighbor-entity accumulated, population-normalized, clamped.
    pub raw_percent_a: i32,             // 0x14
    pub score_percent_a: f32,           // 0x18 // vs. species-config threshold at `+0x34c`.
    pub raw_building_count: i32,        // 0x1c
    pub score_building_count: f32,      // 0x20 // vs. config `+0x350`.
    pub score_percent_c: f32,           // 0x24 // vs. config `+0x358`; raw numerator is scratch-only, not persisted.
    pub unk_0x28: u32,                  // 0x28 // Zero-init only - see struct doc comment.
    pub unk_0x2c: u32,                  // 0x2c // Zero-init only - see struct doc comment.
    pub score_d: f32,                   // 0x30 // Gated by its own "count >= 3" flag pair.
    pub score_e: f32,                   // 0x34 // Gated by its own "count >= 3" flag pair.
    pub sum_category_tally: f32,        // 0x38 // Summed `BFCategory::getValue` over a per-species state list.
    pub tank_depth_score: f32,          // 0x3c // Reset to 100.0 per species, then overwritten with the tank depth-fit score (`100.0 - min(30, deviation) * 20`) when the habitat is a tank whose species has a min != max water depth. Real vanilla's `FSTP [ESI]` through a `[ESP+0x34] = record+0x3c` pointer local; no reader identified.
    pub unk_0x40: u32,                  // 0x40 // Zero references anywhere in this function.
    pub unk_0x44: u32,                  // 0x44 // Zero references anywhere in this function.
    pub salt_water_tile_adjustment: i32,  // 0x48 // `setAnimalConditions`'s own category-9/10 substitute count and denominator term - real Ghidra name `saltWaterTileAdjustment` (`ZTSpeciesAttribs`).
    pub fresh_water_tile_adjustment: i32, // 0x4c // Same shape as `salt_water_tile_adjustment` - real Ghidra name `freshWaterTileAdjustment`.
    pub tank_or_visibility_score: f32,  // 0x50 // Tank: the tank's raw water purity (`+0x1a8`) as a float. Non-tank: `100.0 - purity_debt/tank_neighbor_size * 100.0` (guest-visibility percentage), else the `100.0` default. Also `setAnimalConditions`'s own "guest visibility" threshold (`< 50.0`/`< 25.0`) for its two tail condition bits.
    pub tank_score_baseline: f32,       // 0x54 // `100.0` if the habitat's vtable `+0x28` predicate (taking the species type) is true, else `-100.0`.
    // 0x58-0x6d: 22 individually-named `bool`s (real Ghidra field order/names, `ZTSpeciesAttribs`) -
    // `setAnimalConditions`'s own direct inputs, translated 1:1 into the animal's low/critical condition
    // flag word (`animal+0x260+0x38/0x3c` low, `+0x40/0x44` critical).
    pub need_rocks: bool,               // 0x58
    pub too_many_rocks: bool,           // 0x59
    pub need_foliage: bool,             // 0x5a
    pub too_much_foliage: bool,         // 0x5b
    pub need_elevation: bool,           // 0x5c
    pub too_much_elevation: bool,       // 0x5d
    pub need_space: bool,               // 0x5e
    pub need_more_shelter: bool,        // 0x5f
    pub too_crowded: bool,              // 0x60
    pub need_toys: bool,                // 0x61
    pub tank_too_shallow: bool,         // 0x62
    pub tank_too_deep: bool,            // 0x63
    pub bad_tank_salinity: bool,        // 0x64 // Not read by `setAnimalConditions` - no consumer identified in this pass's own scope.
    pub need_more_toys: bool,           // 0x65 // Not read by `setAnimalConditions` - no consumer identified in this pass's own scope.
    pub bad_toy_suitability: bool,      // 0x66 // Not read by `setAnimalConditions` - no consumer identified in this pass's own scope.
    pub critical_rocks: bool,           // 0x67
    pub critical_foliage: bool,         // 0x68
    pub critical_elevation: bool,       // 0x69
    pub critical_shelter: bool,         // 0x6a
    pub critical_toys: bool,            // 0x6b
    pub critical_tank_depth: bool,      // 0x6c
    pub critical_water: bool,           // 0x6d
    pub pad_0x6e: [u8; 0x2],            // 0x6e-0x6f // Trailing alignment padding to the record's real 0x70-byte size (Ghidra's own `pad_0x6e`).
    #[getset(skip)]
    pub _live: crate::util::LiveMemory,
}

const _: () = assert!(std::mem::size_of::<ZTHabitatSuitabilityRecord>() == 0x70);
const _: () = assert!(std::mem::offset_of!(ZTHabitatSuitabilityRecord, tank_depth_score) == 0x3c);
const _: () = assert!(std::mem::offset_of!(ZTHabitatSuitabilityRecord, tank_score_baseline) == 0x54);
const _: () = assert!(std::mem::offset_of!(ZTHabitatSuitabilityRecord, need_rocks) == 0x58);
const _: () = assert!(std::mem::offset_of!(ZTHabitatSuitabilityRecord, critical_water) == 0x6d);
const _: () = assert!(std::mem::offset_of!(ZTHabitatSuitabilityRecord, pad_0x6e) == 0x6e);

/// Matches `ZTHabitatSuitabilityRecord::ZTHabitatSuitabilityRecord`'s real body (confirmed live via
/// Ghidra decompile): every field zero-inited unconditionally, the 22 flag `bool`s and `pad_0x6e`
/// included.
impl Default for ZTHabitatSuitabilityRecord {
    fn default() -> Self {
        // SAFETY: every field is a plain integer/float/bool/byte-array type - all-zero bytes is a valid
        // value for each (a zeroed `bool` is `false`).
        unsafe { std::mem::zeroed() }
    }
}

/// [`ZTHabitat::recalc_phase_5`]'s output - the four locals real vanilla's own tank/water-level pass
/// survives past this phase (confirmed via the `FUN_004469ba` tail-call continuation - see that
/// method's own doc comment). A later stage porting phases 4/6 threads these straight through to that
/// continuation unchanged.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct RecalcPhase5Summary {
    pub tank_neighbor_total_size: i32,
    pub freshwater_filled_size: i32,
    pub saltwater_filled_size: i32,
    pub weighted_purity_debt: f32,
}

/// `ZTAnimal::getHomeHabitat` (`0x004161da`): the animal's home habitat (recomputed through
/// `recalcHomeHabitat`/`getMostSuitableHabitat` on every call), as a raw pointer.
unsafe fn animal_home_habitat(animal_ptr: u32) -> u32 {
    unsafe { openzt_detour::generated::ztanimal::GET_HOME_HABITAT.original()(animal_ptr as *const u32) as u32 }
}

/// The two `ph_AdditionalConditionValues` bytes `setAnimalConditions` reads
/// (`0x004451a6`-`0x004451f4`): `overlap_tally / owned_tile_total` compared against `0.05` and `0.1`
/// (the `f32` constants at `0x00635540`/`0x0063553c`), in x87 double precision.
fn overlap_condition_bits(overlap_tally: i32, owned_tile_total: i32) -> (bool, bool) {
    let ratio = overlap_tally as f64 / owned_tile_total as f64;
    (ratio > 0.05_f32 as f64, ratio > 0.1_f32 as f64)
}

/// Everything [`ZTHabitat::recalc_phase_6_species`] needs besides the record and species: the shared,
/// per-call inputs of `recalculateCharacteristics`'s phase-6 loop.
pub(crate) struct RecalcPhase6Inputs<'a> {
    pub owned_tile_total: i32,
    pub elevation_tally: i32,
    pub histogram: &'a [i32; 18],
    pub phase5: &'a RecalcPhase5Summary,
    pub float_cache_a: u32,
    pub float_cache_b: u32,
    pub field_0x16c_values: &'a [i32],
    pub field_0x170_clamped: &'a [i32],
    /// The found-species list as it stood after the animal census, before phase 3's neighbor
    /// propagation extended it (vanilla's `local_b6c` snapshot).
    pub companion_species: &'a [u32],
}

/// Vanilla's `FUN_0044699a` (`0x0044699a`/`0x0044837b`): the water tile adjustment for one water category.
/// `0` unless the species predicate holds and `category_value` is in `(0, 100)`; otherwise
/// `clamp((owned - tiles) * category_value / (100 - category_value) - tiles, 0, cap)` (32-bit `IMUL`/`IDIV`).
fn tile_count_adjustment(species_pred: bool, category_value: i32, tiles: i32, owned_tile_total: i32, cap: i32) -> i32 {
    if !species_pred || category_value <= 0 || category_value == 100 {
        return 0;
    }
    let scaled = owned_tile_total.wrapping_sub(tiles).wrapping_mul(category_value).wrapping_div(100 - category_value);
    scaled.wrapping_sub(tiles).min(cap).max(0)
}

/// One iteration of the 18-category loop in [`ZTHabitat::recalc_phase_6_terrain_scores`]: the new running
/// `terrain_type_score`. `tiles` is the raw histogram entry, `count` the same entry after the water
/// adjustment, `weight` the species' category value.
fn terrain_category_step(terrain: f32, weight: i32, tiles: i32, count: i32, denom: i32) -> f32 {
    let weight32 = weight as f32;
    if weight32 < 0.0 {
        return if tiles > 0 { (weight32 as f64 + terrain as f64) as f32 } else { terrain };
    }
    let pct = (count as f64 * 100.0 / denom as f64) as f32;
    let contribution = if weight32 < pct { weight32 } else { pct };
    (contribution as f64 + terrain as f64) as f32
}

/// The `scenery_category_score` normalisation: divide by `max(1.0, owned * 0.025) * 100.0`, cap at
/// `100.0`, multiply by `50.0` if still negative (`0x00445972`-`0x004459a5`: `1.0` is selected when the
/// product compares below it). Every step is `f32`: the game runs the x87 at 24-bit precision, so the
/// product, `factor * 100.0` and the quotient each round to single precision.
fn normalise_scenery_score(scenery: f32, owned_tile_total: i32) -> f32 {
    let product = owned_tile_total as f32 * 0.025_f32;
    let factor = if product < 1.0 { 1.0 } else { product };
    let normalised = scenery / (factor * 100.0_f32);
    let capped = if normalised < 100.0 { normalised } else { 100.0 };
    if capped < 0.0 { capped * 50.0 } else { capped }
}

/// One `raw * 100 / denom` metric of `recalculateCharacteristics`'s phase 6, scored against a species
/// threshold: see [`percent_band`].
#[derive(Debug, Clone, Copy, PartialEq)]
struct PercentBand {
    too_low: bool,
    too_high: bool,
    critical: bool,
    score: f32,
}

/// Vanilla's x87 threshold-band scoring for `score_percent_a`/`score_building_count`. Intermediates are
/// `f64` (MSVC's default 53-bit x87 precision) and rounded to `f32` only where vanilla stores to memory
/// (`pct`, the clamped deviation product, the final score).
///
/// `pct = raw * 100 / denom`; `too_low` when `pct < threshold - 4` or `pct == 0 && threshold > 0` (the
/// `0x0044703c`/`0x00447033` stubs), otherwise `too_high` when `threshold + 4 < pct`. `critical` when
/// `|pct - threshold| > 8`. `score = min(100, 100 - 2 * (clamp(|pct - threshold|, 0, 50) * factor))`.
/// A zero `denom` yields `inf`/`NaN` like vanilla's masked x87 division, but NaN's x87 compare flags are
/// not modelled; `owned_tile_total >= 1` for any habitat with tiles.
fn percent_band(raw: i32, denom: f64, threshold: i32, factor: f64) -> PercentBand {
    let pct = (raw as f64 * 100.0 / denom) as f32;
    let threshold = threshold as f64;
    let pct64 = pct as f64;

    let too_low = pct64 < threshold - 4.0 || (pct64 == 0.0 && threshold > 0.0);
    let too_high = !too_low && threshold + 4.0 < pct64;

    let deviation = (pct64 - threshold).abs();
    let critical = deviation > 8.0;

    let deviation32 = deviation as f32;
    let clamped = if deviation >= 50.0 { 50.0 } else { deviation32 };
    let clamped = if 0.0 < clamped { clamped } else { 0.0 };
    let scaled = (clamped as f64 * factor) as f32;
    let score = (100.0 - (scaled + scaled) as f64) as f32;
    let score = if score < 100.0 { score } else { 100.0 };

    PercentBand { too_low, too_high, critical, score }
}

/// `[ESP+0x68]` as `recalculateCharacteristics`'s phase-6 scoring loop reads it in the `score_d` `v > 6`
/// term: the loop never writes it, and its last write before the loop is `0` (`0x004455ce`).
const STALE_ESP_0X68: i32 = 0;

/// The count-based penalty shared by `score_d` (shelter, `critical_offset = 3`) and `score_e` (toys,
/// `critical_offset = 2`): `50 * count + 100` once `count > 2`, times `count - critical_offset` when the
/// species' critical flag is set. Rounded to `f32` at vanilla's two `FSTP`s.
fn count_penalty(count: i32, critical: bool, critical_offset: i32) -> f32 {
    let base = if count > 2 { (count.wrapping_mul(50) as f64 + 100.0) as f32 } else { 0.0 };
    if critical {
        (count.wrapping_sub(critical_offset) as f64 * base as f64) as f32
    } else {
        base
    }
}

/// `BFTile::getCornerHeight` (`0x0040f4f9`) for the four corners the elevation census samples: the
/// tile's base elevation (`tile+0x3c`) adjusted by the two-bit corner fields packed in `tile+0x81`.
fn tile_corner_height(tile_ptr: u32, corner: u32) -> i32 {
    corner_height(get_from_memory(tile_ptr + 0x3c), get_from_memory(tile_ptr + 0x81), corner)
}

/// The pure part of [`tile_corner_height`]. Corner `1` uses bits 6-7, `3` bits 4-5, `5` bits 2-3, each minus
/// the low two bits; any other corner (the census uses `7`) is the base elevation unchanged.
fn corner_height(base: i32, packed: u8, corner: u32) -> i32 {
    let low = (packed & 3) as i32;
    match corner {
        1 => base + (packed >> 6) as i32 - low,
        3 => base + ((packed >> 4) & 3) as i32 - low,
        5 => base + ((packed >> 2) & 3) as i32 - low,
        _ => base,
    }
}

/// `score_percent_c`'s band scoring, [`PercentBand`]-shaped: `pct = tally * 50 / (float)owned`, tolerance
/// `10` around the threshold, critical beyond `20`, score `min(100, 100 - clamp(|pct - threshold|, 0, 50))`.
fn elevation_band(elevation_tally: i32, owned_tile_total: i32, threshold: i32) -> PercentBand {
    let pct = (elevation_tally as f64 * 50.0 / owned_tile_total as f32 as f64) as f32;
    let pct64 = pct as f64;
    let threshold = threshold as f64;

    let too_low = threshold - 10.0 > pct64;
    let too_high = !too_low && threshold + 10.0 < pct64;

    let deviation = (pct64 - threshold).abs();
    let critical = deviation > 20.0;

    let deviation32 = deviation as f32;
    let clamped = if deviation >= 50.0 { 50.0 } else { deviation32 };
    let clamped = if 0.0 < clamped { clamped } else { 0.0 };
    let score = (100.0 - clamped as f64) as f32;
    let score = if score < 100.0 { score } else { 100.0 };

    PercentBand { too_low, too_high, critical, score }
}

/// `min(100.0, 100.0 - penalty)`, the tail of `score_d`/`score_e`.
fn clamped_score(penalty: f32) -> f32 {
    let score = (100.0 - penalty as f64) as f32;
    if score < 100.0 { score } else { 100.0 }
}

/// Sets a record flag byte to `1` when `condition` holds; vanilla's phase 6 only ever writes `1`, never
/// `0`, to these bytes (the reset-to-clean block is the sole clearer).
fn set_flag_if(addr: u32, condition: bool) {
    if condition {
        save_to_memory::<u8>(addr, 1);
    }
}

/// The shared per-tile gate every keeper-food walker applies ([`Self::get_num_keeper_food_tiles`],
/// [`Self::get_smallest_keeper_food`], [`Self::get_nearest_keeper_food`], [`Self::get_random_keeper_food`]):
/// `tile_ptr`'s occupant (`tile+0x10`) must be non-null, pass the `ZTFood` type-cast
/// ([`entity_type_matches`] with [`RVA_ZTFOOD_TYPE_CHECK_ARG`]), and its own type's `+0x168`
/// keeper-food-category word (the entity's type is `*(entity+0x128)`) must equal `category`.
///
/// The Windows bodies render two back-to-back `isCastClass(DAT_006386c0)` calls on the same type
/// pointer - the inlined `getKeeperFoodType`'s own type-cast re-checking the predicate the first call
/// already answered - with deliberate crash stubs on the false/null paths
/// (`XOR %EAX, %EAX; MOV %EAX, [%EAX+0x168]`, dereferencing address `0x168`). The port performs the
/// gate once; [`entity_type_matches`]'s internal null-type guard (returns false) covers the stub's
/// null-type arm, same established deviation as [`Self::send_maint_worker_cleanup_events`].
fn keeper_food_category_matches(tile_ptr: u32, category: u32) -> bool {
    let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
    entity_ptr != 0
        && unsafe { entity_type_matches(entity_ptr, RVA_ZTFOOD_TYPE_CHECK_ARG) }
        && get_from_memory::<u32>(get_from_memory::<u32>(entity_ptr + 0x128) + 0x168) == category
}

/// The animal's currently-targeted `ZTFood` entity, or `0` if it has none - the chain real vanilla's
/// `ZTAnimal::getFood` walks, fully inlined at every Windows call site (confirmed no standalone
/// `generated.rs` entry exists for it, and no Windows decompile file - only
/// `private/resources/macos-decompiles/ZTAnimal_getFood.c`). `animal_ptr + 0x38c` is the animal's
/// current eat-action context pointer (corroborated independently by `ZTAnimal_fReduceFood.c`'s
/// identical `this->_pad_0x2bc + 0xd0` read, `0x2bc + 0xd0 == 0x38c`); `+0x10` within it is the
/// candidate food entity, gated by the same [`entity_type_matches`]/[`RVA_ZTFOOD_TYPE_CHECK_ARG`]
/// `ZTFood` type-check [`keeper_food_category_matches`] already uses for the identical `&DAT_006386c0`
/// constant.
pub(crate) fn animal_food_target(animal_ptr: u32) -> u32 {
    let action_ctx: u32 = get_from_memory(animal_ptr + 0x38c);
    if action_ctx == 0 {
        return 0;
    }
    let candidate: u32 = get_from_memory(action_ctx + 0x10);
    if candidate != 0 && unsafe { entity_type_matches(candidate, RVA_ZTFOOD_TYPE_CHECK_ARG) } {
        candidate
    } else {
        0
    }
}

/// The squared straight-line distance [`Self::get_nearest_keeper_food`]'s two comparisons use: plain
/// `dx*dx + dy*dy` (the Windows inline's own `SUB`/`IMUL`/`ADD` sequence, wrapping arithmetic matching
/// `IMUL` overflow) between the reference tile's and the candidate's raw `+0x34`/`+0x38` coordinates.
/// Either pointer null -> `0x7fffffff`, which can never be beaten by the strict-`<` best tracking - so
/// a null reference tile makes every candidate tie at the sentinel and nothing wins.
fn keeper_food_distance_squared(reference_tile: u32, candidate_tile: u32) -> i32 {
    if reference_tile == 0 || candidate_tile == 0 {
        return 0x7fff_ffff;
    }
    let dx = get_from_memory::<i32>(reference_tile + 0x34).wrapping_sub(get_from_memory::<i32>(candidate_tile + 0x34));
    let dy = get_from_memory::<i32>(reference_tile + 0x38).wrapping_sub(get_from_memory::<i32>(candidate_tile + 0x38));
    dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
}

/// One `+0x13c` portal-fence vtable dispatch `ZTHabitat::updatePortals` performs per
/// show-neighbor pair, in walk order. Split out from [`ZTHabitat::update_portals`] so the live
/// comparison test can diff the full dispatch sequence against real vanilla without either
/// pole touching fence state.
pub(crate) struct PortalDispatch {
    pub fence_ptr: u32,
    pub is_open: bool,
    pub play_sound: bool,
}

impl ZTHabitat {
    pub(crate) const TANK_VTABLE_PTR: u32 = 0x006312bc;

    /// Sets [`Self::species_list_dirty`], the flag `update` checks before calling `reviseSpeciesList`.
    pub(crate) fn set_species_list_dirty(&self) {
        write_live!(self, species_list_dirty, 1u8);
    }

    /// Sets [`Self::tank_walk_visited_marker`] (`0` clears it before a fresh tank walk).
    pub(crate) fn set_tank_walk_visited_marker(&self, value: u8) {
        write_live!(self, tank_walk_visited_marker, value);
    }

    /// Ports `ZTHabitat::getGateTileIn` (`ZTHabitat_getGateTileIn.c`): the address of the gate tile on
    /// this habitat's side of its entrance. That's the entrance tile itself when this habitat owns it,
    /// otherwise the tile one step through the gate ([`ZTWorldMgr::get_neighbour_ptr_raw`] with
    /// `entrance_rotation`, so a rotation above 7 returns the entrance tile, as vanilla's does). `0`
    /// with no habitat manager or no entrance tile.
    pub fn gate_tile_in_ptr(&self) -> u32 {
        let entrance_ptr = self.entrance_tile_ptr;
        if globals().zthabitatmgr_ptr().is_null() || entrance_ptr == 0 {
            return 0;
        }
        if self.owns_tile(entrance_ptr) {
            return entrance_ptr;
        }
        self.step_through_gate(entrance_ptr)
    }

    /// Ports `ZTHabitat::getGateTileOut` (`ZTHabitat_getGateTileOut.c`), the mirror of
    /// [`Self::gate_tile_in_ptr`]: the tile one step through the gate when this habitat owns the
    /// entrance tile, otherwise the entrance tile itself.
    pub fn gate_tile_out_ptr(&self) -> u32 {
        let entrance_ptr = self.entrance_tile_ptr;
        if globals().zthabitatmgr_ptr().is_null() || entrance_ptr == 0 {
            return 0;
        }
        if self.owns_tile(entrance_ptr) {
            return self.step_through_gate(entrance_ptr);
        }
        entrance_ptr
    }

    /// Whether the habitat grid assigns `tile_ptr`'s position to this habitat - a pointer compare, as
    /// in vanilla's `getGateTileIn`/`getGateTileOut`.
    fn owns_tile(&self, tile_ptr: u32) -> bool {
        let x = get_from_memory::<i32>(tile_ptr + 0x34);
        let y = get_from_memory::<i32>(tile_ptr + 0x38);
        globals().zthabitatmgr().get_habitat_ptr(x, y) == self as *const Self as u32
    }

    fn step_through_gate(&self, tile_ptr: u32) -> u32 {
        let world = globals().ztworldmgr_ptr();
        if world.is_null() {
            return 0;
        }
        unsafe { &*world }.get_neighbour_ptr_raw(tile_ptr, self.entrance_rotation)
    }

    /// [`Self::gate_tile_in_ptr`] as a tile snapshot.
    pub fn get_gate_tile_in(&self) -> Option<BFTile> {
        match self.gate_tile_in_ptr() {
            0 => None,
            ptr => Some(get_from_memory::<BFTile>(ptr)),
        }
    }

    /// [`Self::gate_tile_out_ptr`] as a tile snapshot.
    pub fn get_gate_tile_out(&self) -> Option<BFTile> {
        match self.gate_tile_out_ptr() {
            0 => None,
            ptr => Some(get_from_memory::<BFTile>(ptr)),
        }
    }

    /// Ports `ZTHabitat::getGateTilePassIn` (`ZTHabitat_getGateTilePassIn.c`/`.asm`,
    /// `generated.rs`'s `GET_GATE_TILE_PASS_IN`) - the whole function is a two-call composition:
    /// resolve the "in" gate tile ([`Self::get_gate_tile_in`]) and hand unit + tile to
    /// [`Self::get_adjacent_clear_tile`], whose picked tile rides through as the return (the `.asm`
    /// tail - `CALL getAdjacentClearTile`; `POP ESI`; `RET 0x4` - never touches `EAX`, and every
    /// caller consumes it: `ZTGoalKeeperHabitat::decide`'s `MOV EDI, EAX` / `TEST EDI, EDI` /
    /// `BFUnit::getPath` chain, `ZTGoalTrickFood::decide`). Both legs use the call-the-port
    /// convention: real vanilla composes the two at their own (both detoured) addresses, so every
    /// caller's composition re-enters the Rust ports under the live battery. The `.asm`'s
    /// `MOV ECX, ESI` ahead of the second call is a dead write - the `stdcall` callee never reads
    /// `ECX` (the same unused-`this` shape [`Self::get_adjacent_clear_tile`]'s own note describes).
    ///
    /// A habitat without a usable gate tile resolves to a null tile pointer, which
    /// [`Self::get_adjacent_clear_tile`]'s own null guard passes back unchanged without touching the
    /// shared RNG state - exactly real vanilla's composition of `getGateTileIn`'s null return
    /// through `getAdjacentClearTile`'s `base_tile != 0` guard.
    pub fn get_gate_tile_pass_in(&self, unit_ptr: u32) -> u32 {
        Self::get_adjacent_clear_tile(unit_ptr, self.gate_tile_in_ptr())
    }

    /// Ports `ZTHabitat::getGateTilePassOut` (`ZTHabitat_getGateTilePassOut.c`/`.asm`,
    /// `generated.rs`'s `GET_GATE_TILE_PASS_OUT`) - [`Self::get_gate_tile_pass_in`]'s exact mirror
    /// with "out" in place of "in" ([`Self::get_gate_tile_out`]). Same ride-through `EAX` return,
    /// consumed by `ZTGuest`/`ZTGuide`/`ZTStaff::pickRandomDest` (`ZTGuest_pickRandomDest.asm`'s own
    /// `TEST EAX, EAX` / `ADD EAX, 0x34` reads the picked tile's position fields straight out of the
    /// return). Same dead `MOV ECX, ESI` in the `.asm`, same null-gate pass-through.
    pub fn get_gate_tile_pass_out(&self, unit_ptr: u32) -> u32 {
        Self::get_adjacent_clear_tile(unit_ptr, self.gate_tile_out_ptr())
    }

    /// Ports `ZTHabitat::getGate` (`ZTHabitat_getGate.c`/`.asm`, `generated.rs`'s `GET_GATE` at
    /// `0x00492ca3`). The C decompile's own `(**(vtable+0x1c))(&CAST_ZTFence)` reads like a novel
    /// "double-indirection" dispatch, but the raw `.asm` shows it's byte-for-byte [`entity_type_matches`] -
    /// confirmed by cross-referencing `&CAST_ZTFence` against `ZTHabitatMgr_replaceFenceWithGate.c`/`.asm`
    /// (already ported as [`ZTHabitatMgr::replace_fence_with_gate`], gated on the exact same
    /// [`RVA_FENCE_TYPE_CHECK_ARG`] global) - same symbol, not a new mechanism.
    ///
    /// Real body: resolves `entrance_rotation / 2` (signed, round-toward-zero - the `.asm`'s own
    /// `CDQ`/`SUB`/`SAR` idiom) as an index into [`Self::entrance_tile_ptr`]'s own `north_fence`/
    /// `east_fence`/`south_fence`/`west_fence` slots (`+0x14/+0x18/+0x1c/+0x20`, the same fields
    /// [`Self::tile_fence_in_direction`] already reads), returning that candidate only if it's a genuine
    /// fence-family member ([`entity_type_matches`]/[`RVA_FENCE_TYPE_CHECK_ARG`]). Returns `0` for a null
    /// `entrance_tile_ptr`, an `entrance_rotation` of `-1` (`0xffff_ffff`, real vanilla's own "no gate"
    /// sentinel), a null candidate, or one that fails the type check.
    ///
    /// Unlike [`Self::tile_fence_in_direction`] (keyed on a full [`Direction`], `0` for any non-cardinal
    /// value), this indexes directly by `entrance_rotation / 2` - the raw `.asm` computes a valid `0..=3`
    /// slot for *any* rotation, even an odd/diagonal one, where `tile_fence_in_direction` would already
    /// return `0`. Reproduced faithfully via [`Self::fence_slot_by_index`] rather than reusing
    /// `tile_fence_in_direction`, even though a real entrance gate should only ever sit on a cardinal tile
    /// edge in practice.
    pub fn get_gate(&self) -> u32 {
        if self.entrance_tile_ptr == 0 || self.entrance_rotation == 0xffff_ffff {
            return 0;
        }
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);
        let index = (self.entrance_rotation as i32) / 2;
        let candidate = Self::fence_slot_by_index(&tile, index);
        if candidate != 0 && unsafe { entity_type_matches(candidate, RVA_FENCE_TYPE_CHECK_ARG) } {
            candidate
        } else {
            0
        }
    }

    /// Ports `ZTHabitat::getOutermostTank` (`ZTHabitat_getOutermostTank.c`/`.asm`) - the C decompile's own
    /// `cls_0x49e12a` return type is just the decompiler failing to resolve a *self*-recursive call
    /// (confirmed: `generated.rs`'s own `GET_OUTERMOST_TANK` address, `0x0049e12a`, is this exact
    /// function), not a real unidentified helper as `zthabitatmgr-implementation-plan.md`'s own scoping
    /// previously worried - fully resolves the plan's "one remaining unidentified helper" note.
    ///
    /// Walks the chain of tanks connected through each tank's own gate-tile neighbour
    /// ([`Self::get_gate_tile_out`] -> [`ZTHabitatMgr::get_habitat_ptr`] on the gate tile's own position),
    /// marking each visited tank's own [`Self::tank_walk_visited_marker`] along the way, stopping and
    /// returning the last tank found once either the neighbour isn't itself a tank ([`Self::is_tank`]) or
    /// has already been visited this walk (a cycle).
    ///
    /// Real vanilla's own `.asm` unconditionally dereferences the neighbour's vtable pointer even when
    /// `getGateTileOut` finds no tile (`ESI` zeroed at `.15`/`.1466e`) - a genuine null-deref in real
    /// vanilla itself. Reproduced here as "stop and return the current tank" instead of crashing: a real
    /// habitat reachable through this call always owns an entrance gate by the time anything walks its
    /// tank chain, the same "dead in practice" reasoning already used elsewhere in this file (e.g.
    /// [`ZTHabitat::save`]'s own unguarded entrance-tile read) for a real-vanilla-crashing edge case with
    /// no live-observed trigger.
    pub fn get_outermost_tank(&self) -> u32 {
        let mgr = globals().zthabitatmgr();
        let mut current_ptr = self as *const Self as u32;
        loop {
            unsafe { ref_from_memory::<Self>(current_ptr) }.set_tank_walk_visited_marker(1);
            let gate_tile_ptr = unsafe { ref_from_memory::<Self>(current_ptr) }.gate_tile_out_ptr();
            let next_ptr = if gate_tile_ptr == 0 {
                0
            } else {
                let gate_tile = get_from_memory::<BFTile>(gate_tile_ptr);
                mgr.get_habitat_ptr(gate_tile.pos.x, gate_tile.pos.y)
            };
            if next_ptr == 0 {
                return current_ptr;
            }
            let next = unsafe { ref_from_memory::<Self>(next_ptr) };
            if !next.is_tank() {
                return current_ptr;
            }
            if next.tank_walk_visited_marker != 0 {
                return current_ptr;
            }
            current_ptr = next_ptr;
        }
    }

    /// Ports `ZTHabitat::getNeedyNestedTank` (`ZTHabitat_getNeedyNestedTank.c`/`.asm`) - depth-first
    /// search over the amphibious-connected tank chain (via [`Self::boundary_tile_pairs`],
    /// not the `amphibious_neighbors_head` tree despite this file's own earlier "recurses the
    /// amphibious-neighbor tree" description) for a tank whose own `needsService(keeper_ptr, true)` is
    /// true and whose keeper isn't already stationed there ([`ztunit::GET_HABITAT`] on `keeper_ptr` !=
    /// the candidate).
    ///
    /// `generated.rs`'s own `zthabitat::GET_NEEDY_NESTED_TANK` entry previously declared 2 stack args
    /// instead of the real `.asm`'s own `RET 0x4` (1 stack arg) - fixed upstream, unblocking this port.
    /// The real remaining complication this file's earlier scoping flagged - "an unidentified `ZTKeeper`
    /// vtable slot at a very large offset (`+0x24c`)" - dissolves once checked against the vtable docs
    /// already in `private/docs/vtables/`: `ZTKeeper.md`/`ZTUnit.md` both list `+0x24c` (index 147) as a
    /// real, ordinary base-`ZTUnit` slot (not a second/thunk-adjusted vtable - the C decompile's own
    /// `(param_2->cls_0x62d4b4).vftptr_0x0[1]` notation just *looks* like one, same false alarm as
    /// `checkEnterHabitat`'s own correction above), and its address (`0x00410642`) already has a named
    /// decompile - `ZTUnit::getHabitat` - with an existing `generated.rs` entry (`ztunit::GET_HABITAT`).
    ///
    /// Real vanilla calls `getGateTileOut` on the neighbour resolved from each boundary pair's own tile
    /// position unconditionally, with no null guard, for two independent reasons a pair can resolve to no
    /// habitat: the pair's own second tile pointer can itself be null, or [`ZTHabitatMgr::get_habitat_ptr`]
    /// can legitimately return `0` for an unowned tile. Both guarded here (skip that pair) rather than
    /// reproduced - same "dead in practice" reasoning as this file's other unguarded-real-vanilla-read
    /// deferrals, and consistent with [`Self::get_outermost_tank`]'s own null-deref workaround just above.
    ///
    /// A recursive match returns the *immediate* child candidate (matches real vanilla's own `.13c319`
    /// exit, which reloads `ESI` - the child just recursed into - rather than propagating the deeper
    /// call's own return value), not necessarily the actual deepest tank found - faithfully reproduced
    /// rather than "corrected", since callers only need *some* genuinely needy nested tank.
    pub fn get_needy_nested_tank(&self, keeper_ptr: u32) -> u32 {
        let self_ptr = self as *const Self as u32;
        if self.tank_walk_visited_marker != 0 {
            return 0;
        }
        self.set_tank_walk_visited_marker(1);

        if !self.is_tank() {
            return 0;
        }

        if self.needs_service(keeper_ptr, true) {
            let keepers_habitat = unsafe { ZTUNIT_GET_HABITAT.original()(keeper_ptr as *const u32) } as u32;
            if keepers_habitat != self_ptr {
                return self_ptr;
            }
        }

        let mgr = globals().zthabitatmgr();
        let pairs = self.boundary_tile_pairs();
        for (first_ptr, second_ptr) in pairs {
            if first_ptr == 0 || second_ptr == 0 {
                continue;
            }
            let second_tile = get_from_memory::<BFTile>(second_ptr);
            let candidate_ptr = mgr.get_habitat_ptr(second_tile.pos.x, second_tile.pos.y);
            if candidate_ptr == 0 {
                continue;
            }
            let candidate = unsafe { ref_from_memory::<Self>(candidate_ptr) };
            if candidate.gate_tile_out_ptr() != first_ptr {
                continue;
            }
            if candidate.get_needy_nested_tank(keeper_ptr) != 0 {
                return candidate_ptr;
            }
        }
        0
    }

    /// The fence occupying `tile`'s own slot in `direction` (`&tile->field_0x14 + (direction/2)*4` in
    /// the decompile - the same idiom `BFTile`'s own `north_fence`/`east_fence`/`south_fence`/
    /// `west_fence` fields already model, see [`fence_pair`]), or `0` for a non-cardinal direction (real
    /// callers - fence/gate placement - never pass one in practice, but this stays defensive rather than
    /// panicking on an unexpected value from a live, external caller).
    pub(crate) fn tile_fence_in_direction(tile: &BFTile, direction_raw: u32) -> u32 {
        match direction_raw {
            0 => tile.north_fence,
            2 => tile.east_fence,
            4 => tile.south_fence,
            6 => tile.west_fence,
            _ => 0,
        }
    }

    /// The fence occupying `tile`'s own raw array slot `index` (`&tile->field_0x14 + index*4` in the
    /// decompile) - [`Self::get_gate`]'s own index-based sibling to [`Self::tile_fence_in_direction`]'s
    /// `Direction`-keyed lookup over the same four fields. `0` for an out-of-range index, matching this
    /// file's own defensive convention for an unexpected value from a live, external caller.
    pub(crate) fn fence_slot_by_index(tile: &BFTile, index: i32) -> u32 {
        match index {
            0 => tile.north_fence,
            1 => tile.east_fence,
            2 => tile.south_fence,
            3 => tile.west_fence,
            _ => 0,
        }
    }

    /// Ports `ZTHabitat::moveGateTo(ZTFence*)` (`ZTHabitat_moveGateTo_0.c`/`.asm`, `generated.rs`'s
    /// `MOVE_GATE_TO_0` at `0x0046616c` - the C decompile's own struct-offset math is garbled, e.g. the
    /// final `setName` call's real target is confirmed via `.asm` as vtable `+0x1c`, not the decompile's
    /// own nested-base guess). Demotes this habitat's current gate ([`Self::get_gate`]) back into a plain
    /// fence and promotes `candidate_fence_ptr` into the new gate - updating
    /// [`Self::entrance_tile_ptr`]/[`Self::entrance_rotation`] and giving the new gate this habitat's own
    /// name - but only when `candidate_fence_ptr` genuinely separates two different habitats (its own
    /// tile and the neighbour stepped through by its own rotation belong to different
    /// [`ZTHabitatMgr::get_habitat_ptr`] owners). No-op (`false`) for a null candidate or one that doesn't
    /// satisfy that check.
    ///
    /// **Must only be called on a live `ZTHabitat` reference** - `self`'s address is written into
    /// directly (`entrance_tile_ptr`/`entrance_rotation`) and passed to real vanilla `setName`, same
    /// precondition [`Self::get_attractiveness`] documents.
    ///
    /// Deviation: also requires the candidate's own tile to have an owner. Vanilla's `ZTFence::makeGate`
    /// -> `setHealthy` -> `dirtyHabitatEscapability` chain dereferences `+0x2c` on that owner with no
    /// null guard (`ZTFence_dirtyHabitatEscapability.c`), which crashes when a habitat's current gate is
    /// passed in: that fence's own tile is the unowned outside tile, while the neighbour through its
    /// rotation is the inside tile.
    pub fn move_gate_to_fence(&self, candidate_fence_ptr: u32) -> bool {
        if candidate_fence_ptr == 0 {
            return false;
        }
        let zthm = globals().zthabitatmgr();
        let world = globals().ztworldmgr();

        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(candidate_fence_ptr as *const u32) } as u32;
        let rotation: u32 = get_from_memory(candidate_fence_ptr + 0x12c);
        let neighbour_ptr = if tile_ptr != 0 { world.get_neighbour_ptr_raw(tile_ptr, rotation) } else { 0 };

        let owner = if tile_ptr != 0 {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            zthm.get_habitat_ptr(tile.pos.x, tile.pos.y)
        } else {
            0
        };
        let neighbour_owner = if neighbour_ptr != 0 {
            let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
            zthm.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y)
        } else {
            0
        };
        if owner == 0 || owner == neighbour_owner {
            return false;
        }

        let old_gate_ptr = self.get_gate();
        if old_gate_ptr != 0 {
            unsafe {
                ZTFENCE_MAKE_FENCE.original()(old_gate_ptr as *const u32);
                call_vtable_slot_noargs(old_gate_ptr, 0x20); // createName
                call_vtable_slot_with_u8(old_gate_ptr, 0x84, 1); // validatePosition(true)
            }
        }

        unsafe { ZTFENCE_MAKE_GATE.original()(candidate_fence_ptr as *const u32) };
        let new_gate_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(candidate_fence_ptr as *const u32) } as u32;
        let new_rotation: u32 = get_from_memory(candidate_fence_ptr + 0x12c);
        let self_addr = self as *const Self as u32;
        save_to_memory(self_addr + 0x8c, new_gate_tile_ptr);
        save_to_memory(self_addr + 0x90, new_rotation);
        unsafe { call_vtable_slot_with_ptr(candidate_fence_ptr, 0x1c, self_addr + 0x154) }; // setName(&exhibit_name)
        true
    }

    #[inline]
    pub(crate) fn move_gate_to_inner(&self, candidate_fence_ptr: u32) -> bool {
        self.move_gate_to_fence(candidate_fence_ptr)
    }

    /// Ports `ZTHabitat::moveGateTo` (`ZTHabitat_moveGateTo_1.c`, `generated.rs`'s `MOVE_GATE_TO_1`):
    /// resolves whatever fence currently occupies `tile_ptr`'s own slot `direction_raw / 2` (only when
    /// it's genuinely a member of the fence/wall family, per [`entity_type_matches`]/
    /// [`RVA_FENCE_TYPE_CHECK_ARG`] - discarded (treated as no candidate) otherwise, matching real
    /// vanilla's own guard) and hands it to [`Self::move_gate_to_inner`]. `direction_raw == 0xffffffff`
    /// (real vanilla's own "no direction" `EDirection` sentinel) always skips straight to a null
    /// candidate.
    ///
    /// **Correctness bug fixed (2026-09-17):** previously used [`Self::tile_fence_in_direction`] (a
    /// `Direction`-enum match, `0` for any non-cardinal value) for the candidate lookup. Real vanilla's
    /// own `.c`/`.asm` instead does `*(&tile->field_0x14 + ((int)direction / 2) * 4)` - the exact same
    /// raw index-by-`rotation/2` array scheme [`Self::get_gate`] already uses for this habitat's own
    /// *current* gate - which still resolves a valid `0..=3` slot for a diagonal/non-cardinal
    /// `EDirection` (truncated onto the preceding cardinal's own slot) rather than treating it as "no
    /// candidate". Fixed by switching to [`Self::fence_slot_by_index`], matching `get_gate`'s own
    /// approach and the real `.asm` exactly.
    ///
    /// Always returns `true` once past the null-tile guard - real vanilla's own return value is
    /// `CONCAT31(garbage, 1)` (see `CLAUDE.md`'s note on this decompile shape), i.e. the low byte is
    /// hardcoded regardless of `move_gate_to_inner`'s own result. Same live-reference precondition as
    /// [`Self::move_gate_to_inner`].
    pub fn move_gate_to(&self, tile_ptr: u32, direction_raw: u32) -> bool {
        if tile_ptr == 0 {
            return false;
        }
        let candidate_ptr = if direction_raw != 0xffff_ffff {
            let tile = get_from_memory::<BFTile>(tile_ptr);
            let fence_ptr = Self::fence_slot_by_index(&tile, (direction_raw as i32) / 2);
            if fence_ptr != 0 && unsafe { entity_type_matches(fence_ptr, RVA_FENCE_TYPE_CHECK_ARG) } {
                fence_ptr
            } else {
                0
            }
        } else {
            0
        };
        self.move_gate_to_inner(candidate_ptr);
        true
    }

    /// Ports `ZTHabitat::getAttractiveness` (`ZTHabitat_getAttractiveness.c`/macOS
    /// `ZTHabitat_getAttractiveness.c`, same shape on both platforms): lazily recomputes via
    /// [`Self::recalculate_characteristics`] when `characteristics_dirty` is set, then returns the
    /// cached field.
    ///
    /// **Must only be called on a live `ZTHabitat` reference** (one obtained via `ref_from_memory`/a real
    /// detour's `this` pointer, e.g. `exhibit_array`'s own entries while iterated in place) - `self`'s
    /// address is passed straight into vanilla's `recalculateCharacteristics`, so calling this on a stack
    /// copy (e.g. one returned by `ZTArray::get`) would hand vanilla a bogus pointer. Same precondition
    /// [`Self::has_keeper_assigned`] carries, and the same one `zoostatus.rs`'s own `newguest_checks`
    /// documents for its `F_CREATE_GUEST` call-through.
    pub fn get_attractiveness(&self) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self.attractiveness
    }

    /// Ports `ZTHabitat::hasKeeperAssigned` (`ZTHabitat_hasKeeperAssigned.c`) - same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_attractiveness`]; see that
    /// method's doc comment for the live-reference precondition this one shares.
    pub fn has_keeper_assigned(&self) -> bool {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self.has_keeper_assigned_raw != 0
    }

    /// Ports `ZTHabitat::getSpeciesList` (`ZTHabitat_getSpeciesList.c`): same `characteristics_dirty`-gated
    /// lazy-recalculate shape as [`Self::get_attractiveness`]/[`Self::has_keeper_assigned`], then yields
    /// every raw catalog-entry pointer in the real vanilla `std::vector<T*>` at `species_list_begin`/
    /// `species_list_end` (`&this->field_0x60` in the decompile) - callers read whichever offset off each
    /// entry they need (e.g. [`ZTHabitatMgr::distinct_species_catalog_ids`]'s `0x1e4`/`0x1ec` family/
    /// species ids) rather than this method modeling the catalog-entry struct itself.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - the lazy recalculate call passes `self`'s own address to real
    /// vanilla, and the vector fields are re-read from live memory after it runs.
    pub fn species_list(&self) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let begin = self.species_list_begin;
        let end = self.species_list_end;
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getSurroundingSpecies` (`ZTHabitat_getSurroundingSpecies.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], yielding raw
    /// catalog-entry pointers from `surrounding_species_begin`/`_end` (`&this->field_0x13c`,
    /// `.asm`-confirmed `LEA EAX,[ESI+0x13c]`) instead. Populated by
    /// [`Self::construct_surrounding_species_list`] (`ZTHabitat::constructSurroundingSpeciesList` - unions
    /// this habitat's own [`Self::species_list`] with its amphibious- and show-neighbor sets' own species
    /// lists, see that method's own doc comment), reached transitively through the same `.original()`
    /// `recalculateCharacteristics` call-through below - no separate call-through is needed here for that
    /// reason.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::species_list`].
    pub fn surrounding_species(&self) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let begin = self.surrounding_species_begin;
        let end = self.surrounding_species_end;
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getNumAnimals` (`ZTHabitat_getNumAnimals.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], then returns the
    /// cached direct-occupant count ([`Self::num_animals`]) alone when `include_neighbors` is `false`. When
    /// `true`, additionally walks the amphibious-neighbor set ([`walk_neighbor_tree`] over
    /// `amphibious_neighbors_head`) - `.asm`-confirmed identical `+0x8` node arithmetic to
    /// [`Self::hilite_amphibious_neighbors`]'s own walk, per the correction
    /// `zthabitatmgr-implementation-plan.md`'s step 6d recorded for this same field - recursively summing
    /// each neighbor's own `get_num_animals(false)` (never recursing sub-neighbors of sub-neighbors,
    /// matching real vanilla's own `getNumAnimals(neighbor, false)` call exactly).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::species_list`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry, which reads
    /// real `ZTHabitat*` pointers directly out of the tree).
    pub fn get_num_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = self.num_animals;
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumAnimals(species_id, include_neighbors)` (`ZTHabitat_getNumAnimals_1.c`,
    /// `GET_NUM_ANIMALS_1`, `0x004388d7`): the per-species occurrence count, i.e. the first field
    /// (`occurrence_count`) of the species's `ZTHabitatSuitabilityRecord` in the `+0x148` cache, lazily
    /// recalculating first. A missing record is default-inserted, as vanilla does. With `include_neighbors`, adds the same lookup
    /// (with `false`) for every amphibious neighbour.
    ///
    /// Must only be called on a live `ZTHabitat` reference; every neighbour in the tree is live.
    pub fn get_num_animals_by_species(&self, species_id: i32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let record_ptr = map_int_habitatsuitability_find_or_insert(self as *const Self as u32 + 0x148, species_id);
        let mut total: i32 = get_from_memory(record_ptr);
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_animals_by_species(species_id, false);
            }
        }
        total
    }

    /// Shared body of the ten per-factor suitability getters (`getTerrainSuitability` ... `getTankSalinitySuitability`):
    /// lazily recalculates when `characteristics_dirty` is set, then returns the species's record in the `+0x148`
    /// cache, default-inserting it when absent (the insert is visible as a side effect, as in vanilla).
    /// Re-entry into `recalculate_characteristics` is stopped by its own
    /// `reentrancy_guard`, exactly as in vanilla.
    ///
    /// Must only be called on a live `ZTHabitat` reference.
    fn suitability_record(&self, species: i32) -> u32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        map_int_habitatsuitability_find_or_insert(self as *const Self as u32 + 0x148, species)
    }

    fn suitability_score(&self, species: i32, record_offset: usize) -> f32 {
        get_from_memory::<f32>(self.suitability_record(species) + record_offset as u32)
    }

    /// Ports `ZTHabitat::getTerrainSuitability` (`GET_TERRAIN_SUITABILITY`, `0x4160ec`).
    pub fn get_terrain_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, terrain_type_score))
    }

    /// Ports `ZTHabitat::getObjectSuitability` (`GET_OBJECT_SUITABILITY`, `0x415a41`).
    pub fn get_object_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, scenery_category_score))
    }

    /// Ports `ZTHabitat::getFoliageDensitySuitability` (`GET_FOLIAGE_DENSITY_SUITABILITY`, `0x415ada`).
    pub fn get_foliage_density_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, score_percent_a))
    }

    /// Ports `ZTHabitat::getRockDensitySuitability` (`GET_ROCK_DENSITY_SUITABILITY`, `0x415b73`).
    pub fn get_rock_density_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, score_building_count))
    }

    /// Ports `ZTHabitat::getElevationSuitability` (`GET_ELEVATION_SUITABILITY`, `0x415c0c`).
    pub fn get_elevation_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, score_percent_c))
    }

    /// Ports `ZTHabitat::getShelterSuitability` (`GET_SHELTER_SUITABILITY`, `0x415ca5`).
    pub fn get_shelter_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, score_d))
    }

    /// Ports `ZTHabitat::getToySuitability` (`GET_TOY_SUITABILITY`, `0x415d3e`).
    pub fn get_toy_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, score_e))
    }

    /// Ports `ZTHabitat::getTankDepthSuitability` (`GET_TANK_DEPTH_SUITABILITY`, `0x49380b`).
    pub fn get_tank_depth_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, tank_depth_score))
    }

    /// Ports `ZTHabitat::getTankCleanlinessSuitability` (`GET_TANK_CLEANLINESS_SUITABILITY`, `0x4938a4`).
    pub fn get_tank_cleanliness_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, tank_or_visibility_score))
    }

    /// Ports `ZTHabitat::getTankSalinitySuitability` (`GET_TANK_SALINITY_SUITABILITY`, `0x49393d`).
    pub fn get_tank_salinity_suitability(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, tank_score_baseline))
    }

    /// Ports `ZTHabitat::getCompatibleAnimalRating` (`GET_COMPATIBLE_ANIMAL_RATING`, `0x438d7b`): the species's
    /// `sum_category_tally`. Its sole vanilla caller is `ZTAnimal::fCompatAnimalsRating` (`0x438bd4`).
    pub fn get_compatible_animal_rating(&self, species: i32) -> f32 {
        self.suitability_score(species, std::mem::offset_of!(ZTHabitatSuitabilityRecord, sum_category_tally))
    }

    /// Ports `ZTHabitat::getSpeciesRating` (`GET_SPECIES_RATING`, `0x4d92db`): the unweighted sum of the
    /// per-factor suitability getters for `species`. `isTank` is a real vtable `+0x20` dispatch
    /// (`ZTTankExhibit` overrides it). Land habitats sum terrain, object, foliage, rock, elevation,
    /// shelter and toy; tanks sum depth, cleanliness, salinity, object, foliage, rock, shelter and toy.
    /// The terms are added in the same (reverse-of-call) order as vanilla, rounding to `f32` at each step
    /// (the game runs the x87 at 24-bit precision, so plain `f32` adds are bit-exact). The first getter
    /// performs the lazy recalculation and the missing-key default insert, the only visible side effects.
    ///
    /// Must only be called on a live `ZTHabitat` reference.
    pub fn get_species_rating(&self, species: i32) -> f32 {
        let self_addr = self as *const Self as u32;
        if unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } {
            let depth = self.get_tank_depth_suitability(species);
            let cleanliness = self.get_tank_cleanliness_suitability(species);
            let salinity = self.get_tank_salinity_suitability(species);
            let object = self.get_object_suitability(species);
            let foliage = self.get_foliage_density_suitability(species);
            let rock = self.get_rock_density_suitability(species);
            let shelter = self.get_shelter_suitability(species);
            let toy = self.get_toy_suitability(species);
            salinity + cleanliness + depth + toy + shelter + rock + foliage + object
        } else {
            let terrain = self.get_terrain_suitability(species);
            let object = self.get_object_suitability(species);
            let foliage = self.get_foliage_density_suitability(species);
            let rock = self.get_rock_density_suitability(species);
            let elevation = self.get_elevation_suitability(species);
            let shelter = self.get_shelter_suitability(species);
            let toy = self.get_toy_suitability(species);
            toy + shelter + elevation + rock + foliage + object + terrain
        }
    }

    /// Ports `ZTHabitat::getNumAdultAnimals` (`ZTHabitat_getNumAdultAnimals_0.c`, `generated.rs`'s
    /// `GET_NUM_ADULT_ANIMALS_0`): reaches the animals through the same `getAllAnimals(this, '\0')` call
    /// real vanilla makes - here [`Self::get_all_animals`]`(false)`, the same
    /// `characteristics_dirty`-gated lazy-recalculate + unsorted-yield shape - then counts direct-occupant
    /// animals whose entity-type gender string (`char*` stored at `entity_type+0xa4`, the same field
    /// [`crate::bfentitytype::read_zt_entity_type_from_memory`] reads as `zt_sub_type`, read through its
    /// first byte here - real vanilla's own `**(char**)(entity_type + 0xa4)`) starts with `'m'` or `'f'`.
    /// Real vanilla performs no null checks on either the animal or its entity-type pointer, and neither
    /// does this port. When `include_neighbors` is set, additionally sums every amphibious neighbor's own
    /// count (recursing with `false`, same single-level-only shape as [`Self::get_num_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_num_adult_animals(&self, include_neighbors: bool) -> i32 {
        let mut total = 0i32;
        for animal_ptr in self.get_all_animals(false) {
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let gender: u8 = get_from_memory(get_from_memory::<u32>(animal_type_ptr + 0xa4));
            if gender == b'm' || gender == b'f' {
                total += 1;
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_adult_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumAdultAnimals` (`ZTHabitat_getNumAdultAnimals_1.c`, `generated.rs`'s
    /// `GET_NUM_ADULT_ANIMALS_1`): the species-filtered overload. Builds a scratch
    /// `std::vector<ZTAnimal*>` via [`Self::get_species_animals`] (real vanilla's own stack-local
    /// out-param, zero-initialized here identically), then counts adult gender tags
    /// ([`Self::get_num_adult_animals`]'s own predicate) over the scratch contents. Frees the scratch
    /// buffer exactly as real vanilla's two distinct teardown paths
    /// do: `PoolAlloc::deallocate_n_4` by capacity on the early return (called unconditionally - real
    /// vanilla's own no-op-on-empty semantics, same as this file's own growth helpers), or
    /// [`free_event_vector_buffer`] by capacity when the subhabitat walk ran first (the same manual
    /// freelist-bucket/`operator_delete` split that decompile's own tail uses).
    ///
    /// When `include_neighbors` is set, additionally sums every amphibious neighbor's own count
    /// (recursing with `false`, same single-level-only shape as [`Self::get_num_animals`]; real vanilla
    /// walks this between the counting and the scratch-buffer teardown, order preserved here).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_num_adult_animals_by_species(&self, species_id: i32, include_neighbors: bool) -> i32 {
        let mut scratch_vector = [0u32; 3];
        self.get_species_animals(species_id, scratch_vector.as_mut_ptr() as u32);
        let mut total = 0i32;
        for addr in (scratch_vector[0]..scratch_vector[1]).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let gender: u8 = get_from_memory(get_from_memory::<u32>(animal_type_ptr + 0xa4));
            if gender == b'm' || gender == b'f' {
                total += 1;
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_adult_animals_by_species(species_id, false);
            }
            free_event_vector_buffer(scratch_vector[0], scratch_vector[2] - scratch_vector[0]);
        } else {
            unsafe {
                POOLALLOC_DEALLOCATE_N_4.original()(
                    scratch_vector[0] as *const u32,
                    ((scratch_vector[2] - scratch_vector[0]) >> 2) as i32,
                )
            };
        }
        total
    }

    /// Ports `ZTHabitat::getSpeciesAnimals` (`ZTHabitat_getSpeciesAnimals.c`/`.asm`, `generated.rs`'s
    /// `GET_SPECIES_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then appends every direct-occupant animal whose entity-type species
    /// id (`entity_type+0x1ec`, the same read real vanilla's own compare makes) matches `species_id`
    /// onto the real vanilla `std::vector<ZTAnimal*>` out-param at `out_vector_ptr`
    /// ([`vector_push_pool_alloc4`] - the same `PoolAlloc::allocate`-doubling-growth/manual-freelist
    /// -teardown shape this decompile's own tail uses). `out_vector_ptr` is never read as a pre-existing
    /// vector on entry beyond its current `begin`/`end`/`cap_end` state - matching real vanilla, which
    /// only ever appends.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_species_animals(&self, species_id: i32, out_vector_ptr: u32) {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            if get_from_memory::<i32>(animal_type_ptr + 0x1ec) == species_id {
                vector_push_pool_alloc4(out_vector_ptr, animal_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::getAdultGenderSpeciesAnimals` (`ZTHabitat_getAdultGenderSpeciesAnimals.c`/`.asm`,
    /// `generated.rs`'s `GET_ADULT_GENDER_SPECIES_ANIMALS`): `ZTAnimal::fGetMate`'s breeding-partner
    /// lookup. Same `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_species_animals`],
    /// then appends every direct-occupant animal passing all three of its filters onto the real vanilla
    /// `std::vector<ZTAnimal*>` out-param at `out_vector_ptr` ([`vector_push_pool_alloc4`], same growth
    /// shape as that sibling): the adult gender tag (`'m'`/`'f'` first byte of the `char*` at
    /// `entity_type+0xa4`, the same read [`Self::get_num_adult_animals`] makes), the species id
    /// (`entity_type+0x1ec`, the same read [`Self::get_species_animals`] makes), and a byte-equality
    /// match between `gender_str_ptr`'s string and the animal's own gender text at
    /// `animal+0x26c`/`+0x270` (both `{start_ptr, end_ptr}` span headers - length pre-check then
    /// byte-by-byte compare, the `.asm`'s `repz cmpsb`; a zero-length requested string therefore
    /// matches every adult of the species, the `.asm`'s own `ECX=0` no-op-compare fall-through).
    ///
    /// `gender_str_ptr` points at the string object real vanilla's own caller passes - a stack
    /// `basic_string` it assigns `"Female"`, then overwrites with `"Male"` when the asking animal's own
    /// gender text is `"Female"` (`ZTAnimal_fGetMate.c`); only its first two `{start_ptr, end_ptr}`
    /// dwords are read, matching the callee.
    ///
    /// Returns `this` (the `.asm`'s `EAX` on the loop path). The return is dead: the macOS decompile
    /// returns `void` and the only Windows caller ignores it (the `.asm`'s empty-`all_animals` early
    /// return leaves `EAX` = the `all_animals` end pointer instead - a register accident, not ported).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_adult_gender_species_animals(&self, gender_str_ptr: u32, species_id: i32, out_vector_ptr: u32) {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let gender_start: u32 = get_from_memory(gender_str_ptr);
        let gender_end: u32 = get_from_memory(gender_str_ptr + 4);
        let gender_len = gender_end.wrapping_sub(gender_start);
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let animal_gender: u8 = get_from_memory(get_from_memory::<u32>(animal_type_ptr + 0xa4));
            if animal_gender != b'm' && animal_gender != b'f' {
                continue;
            }
            if get_from_memory::<i32>(animal_type_ptr + 0x1ec) != species_id {
                continue;
            }
            let text_start: u32 = get_from_memory(animal_ptr + 0x26c);
            let text_end: u32 = get_from_memory(animal_ptr + 0x270);
            if text_end.wrapping_sub(text_start) != gender_len {
                continue;
            }
            let mut text_matches = true;
            for offset in 0..gender_len {
                if get_from_memory::<u8>(text_start + offset) != get_from_memory::<u8>(gender_start + offset) {
                    text_matches = false;
                    break;
                }
            }
            if text_matches {
                vector_push_pool_alloc4(out_vector_ptr, animal_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::getRandomAnimal` (`ZTHabitat_getRandomAnimal.c`/`.asm`, `generated.rs`'s
    /// `GET_RANDOM_ANIMAL`): reaches the animals through the same `getAllAnimals(this, '\0')` call real
    /// vanilla makes - here the `characteristics_dirty`-gated lazy-recalculate + `all_animals_begin`/
    /// `all_animals_end` reads [`Self::get_species_animals`] makes, since the decompile consumes the
    /// returned vector header itself (element count, then one indexed element) rather than iterating.
    /// Returns null on an empty habitat without touching the shared game RNG state; otherwise advances
    /// `DAT_00638060` with exactly one MSVC LCG step ([`lcg_next`], the `.asm`'s own `IMUL`/`ADD` dword
    /// pair) and returns the animal pointer at index `(seed >> 0x10 & 0x7fff) % count` - on a `u32` the
    /// shift/mask pair equals the `.asm`'s `SAR 0x10` + `AND 0x7fff`, and the modulo is the same
    /// unsigned `DIV` (both operands non-negative).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_animal(&self) -> u32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let begin = self.all_animals_begin;
        let count = (self.all_animals_end - begin) >> 2;
        if count == 0 {
            return 0;
        }
        let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
        let rng = lcg_next(get_from_memory::<u32>(rng_addr));
        save_to_memory(rng_addr, rng);
        let index = ((rng >> 0x10) & 0x7fff) % count;
        get_from_memory(begin + index * 4)
    }

    /// Ports `ZTHabitat::getSize` (`ZTHabitat_getSize.c`/`.asm`, `generated.rs`'s `GET_SIZE`): the
    /// habitat's owned-tile count, plus every amphibious neighbor's own count when `subhabs` is set.
    /// The false arm is exactly the owned-tile node count [`walk_tile_list`] walks over
    /// [`Self::owned_tiles_ptr`] (the loop both decompile arms run). When `subhabs` is set,
    /// additionally walks the amphibious-neighbor set ([`walk_neighbor_tree`] over
    /// [`Self::amphibious_neighbors_head`], `.asm`-confirmed identical `+0x8` node arithmetic to
    /// [`Self::get_num_animals`]'s own walk, `ZTHabitat*` payload read directly from node `+0x10`),
    /// summing each neighbor's own `get_size(false)` (never re-passing `true` down, matching real
    /// vanilla's own `getSize(neighbor, false)` call exactly). A pure read both ways: no
    /// `characteristics_dirty` lazy-recalculate, no RNG.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_num_animals`]; each neighbor visited must also be live (true for every
    /// [`walk_neighbor_tree`] entry, which reads real `ZTHabitat*` pointers directly out of the tree).
    pub fn get_size(&self, subhabs: bool) -> i32 {
        let mut total = walk_tile_list(self.owned_tiles_ptr).count() as i32;
        if subhabs {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_size(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getRandomTile` (`ZTHabitat_getRandomTile.c`/`.asm`, `generated.rs`'s
    /// `GET_RANDOM_TILE`): picks a random tile from the habitat's owned-tile list. Gets the count
    /// through the same `getSize(this, false)` call real vanilla makes ([`Self::get_size`], the
    /// same owned-tile node count [`walk_tile_list`] walks - and the same port
    /// [`ZTHabitatMgr::fence_removed`]'s neighbor-size reads use), bails null on an
    /// empty list without touching the shared game RNG state (the `.asm`'s `JLE` sitting before the
    /// LCG pair), then advances `DAT_00638060` with exactly one MSVC LCG step ([`lcg_next`]) and walks
    /// the owned-tile list ([`Self::owned_tiles_ptr`] sentinel, the same `+0x0`=next / `+0x8`=payload
    /// node shape every corroborating walker reads) to index `(seed >> 0x10 & 0x7fff) % count`,
    /// returning that node's `BFTile*` payload. On a `u32` the shift/mask pair equals the `.asm`'s
    /// `SAR 0x10` + `AND 0x7fff`, and the modulo is the same `IDIV` remainder (both operands
    /// non-negative). The `.asm` walk's own secondary bound (`i >= count` falling through to a null
    /// return) is unreachable - the index is `< count` by construction and both the count and the walk
    /// read the same synchronous, unmutated list - so the walk simply ends at the sentinel.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_tile(&self) -> u32 {
        let count = self.get_size(false);
        if count <= 0 {
            return 0;
        }
        let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
        let rng = lcg_next(get_from_memory::<u32>(rng_addr));
        save_to_memory(rng_addr, rng);
        let index = ((rng >> 0x10) & 0x7fff) % count as u32;
        for (i, node) in walk_tile_list(self.owned_tiles_ptr).enumerate() {
            if i == index as usize {
                return get_from_memory::<TileListNode>(node).payload;
            }
        }
        0
    }

    /// Shared tail of [`Self::get_random_tile_in_direction`]/[`Self::get_random_clear_tile_ahead`]:
    /// one MSVC LCG step over the shared game RNG state, then the candidate at index
    /// `(seed >> 0x10 & 0x7fff) % count`. An empty candidate list falls back to
    /// [`Self::get_random_tile`] - both callers' own empty-`.c`-list branch, which leaves the seed
    /// untouched on its own empty-habitat path.
    pub(crate) fn pick_random_candidate_tile(&self, candidates: Vec<u32>) -> u32 {
        if candidates.is_empty() {
            return self.get_random_tile();
        }
        let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
        let rng = lcg_next(get_from_memory::<u32>(rng_addr));
        save_to_memory(rng_addr, rng);
        let index = ((rng >> 0x10) & 0x7fff) % candidates.len() as u32;
        candidates[index as usize]
    }

    /// Ports `ZTHabitat::getRandomTileInDirection` (`ZTHabitat_getRandomTileInDirection.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_TILE_IN_DIRECTION`): picks a random owned tile whose direction from
    /// `from_tile` is close to `direction` - `BFMap::isCloseDirection`'s "within one step on the
    /// 8-direction compass" test. Builds the candidate set exactly as the `.asm`'s inlined
    /// `std::list<uint>` build does - every owned-tile payload ([`walk_tile_list`], node `+0x8`) whose
    /// `BFMap::getDirection(from_tile, tile)` result passes `isCloseDirection(direction, dir)` - then
    /// draws through [`Self::pick_random_candidate_tile`]. The temp list is a native `Vec`, dropped
    /// before returning - vanilla's `PoolAlloc::deallocate_n_12` node teardown reproduces nothing here
    /// (the plan's own allocator-safety rule for internal temporaries).
    ///
    /// The first parameter is the tile the direction is measured *from* (a real `BFTile*`; the plan's
    /// own `ZTUnit* unit` gloss is wrong - both decompiles and the macOS prototype comment agree on
    /// `BFTile *`), so `null` from-tiles are legal input like any other: `getDirection` answers `-1`
    /// for them, `isCloseDirection` rejects it, and the candidate set comes out empty.
    ///
    /// When no owned tile matches, real vanilla falls back to `getRandomTile(this)` - a direct call to
    /// the now-detoured address, so vanilla's own fallback re-enters this port under the live battery;
    /// the port calls its sibling directly (same call-the-port convention as
    /// [`Self::get_nearest_sick_animal`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_tile_in_direction(&self, from_tile: u32, direction: u32) -> u32 {
        let candidates: Vec<u32> = walk_tile_list(self.owned_tiles_ptr)
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&tile| {
                let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(from_tile as i32, tile as i32) };
                low_byte_bool(unsafe { BFMAP_IS_CLOSE_DIRECTION.original()(direction, dir) })
            })
            .collect();
        self.pick_random_candidate_tile(candidates)
    }

    /// Ports `ZTHabitat::getRandomClearTileAhead` (`ZTHabitat_getRandomClearTileAhead.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_CLEAR_TILE_AHEAD`): picks a random owned tile that is both cheap
    /// enough for `unit` to enter and roughly ahead of it. The `.c`'s `rotation` field is the unit's
    /// 8-direction facing at `+0x12c` (`0xffffffff` = never faced anything); the reference heading is
    /// its *opposite* (`(rotation - 4) & 7`), and a tile qualifies when its path cost - the unnamed
    /// `BFUnit` vtable `+0x164` slot ([`call_bfunit_tile_cost_vtable_slot`], the real per-subclass
    /// `getTerrainCost`-family dispatch, `BFUnit.md` slot 89 - the same comparison
    /// `BFAIMgr::fRandomWalk` makes against the same sentinel) is below the shared
    /// [`MAX_PATH_COST_RVA`] "unreachable" bound **and** `isCloseDirection(heading,
    /// getDirection(unit_tile, tile))` reports *not* close (i.e. within a step of the heading's
    /// opposite means behind; everything else is ahead-ish). An unset facing makes
    /// `isCloseDirection` false for every tile, so the candidate set degenerates to "all affordable
    /// tiles". Candidate build/draw/teardown as in [`Self::get_random_tile_in_direction`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_clear_tile_ahead(&self, unit_ptr: u32) -> u32 {
        let rotation: u32 = get_from_memory(unit_ptr + 0x12c);
        let heading = if rotation == 0xffff_ffff { rotation } else { rotation.wrapping_sub(4) & 7 };
        let unit_tile = unsafe { BFENTITY_GET_TILE.original()(unit_ptr as *const u32) };
        let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        let candidates: Vec<u32> = walk_tile_list(self.owned_tiles_ptr)
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&tile| {
                let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, tile) };
                if cost >= max_cost {
                    return false;
                }
                let dir = unsafe { BFMAP_GET_DIRECTION_0.original()(unit_tile, tile as i32) };
                !low_byte_bool(unsafe { BFMAP_IS_CLOSE_DIRECTION.original()(heading, dir) })
            })
            .collect();
        self.pick_random_candidate_tile(candidates)
    }

    /// Ports `ZTHabitat::addClearTiles` (`ZTHabitat_addClearTiles.c`/`.asm`, `generated.rs`'s
    /// `ADD_CLEAR_TILES`): appends every owned tile a unit could stand on onto the caller's real
    /// vanilla `std::vector<BFTile*>` out-param at `out_vector_ptr` ([`vector_push_pool_alloc4`]
    /// growth, same shape as [`Self::get_species_animals`]) - the candidate-gathering dependency
    /// `getRandomClearTile`'s overloads (Stage 11) build their pools from. Per owned tile
    /// ([`walk_tile_list`], payload `node+0x8`), in list order, the `.c`'s four-way filter:
    ///
    /// 1. the four direct-entity slots at `tile+0x4..+0x10` are all null - the first of `BFTile`'s
    ///    two entity-pointer groups (`BFTile_calculateShape.c`/`_validatePositions.c` walk
    ///    `+0x4..+0x10` and the `+0x14..+0x20` edge-entity group identically; `+0x10` is the tile's
    ///    own `entity_ptr` this file already reads elsewhere). Edge entities are *not* checked, so
    ///    fenced-in habitat tiles still qualify.
    /// 2. the entity list at `tile+0x0` is empty. Real vanilla constructs a throwaway
    ///    `std::list<uint>` copy of the tile's list and consumes it only as `size() < 1` - the port
    ///    reads the list's own emptiness (the sentinel's `next` pointing back at itself) and
    ///    reproduces none of the copy's `PoolAlloc::deallocate_n_12` teardown (the plan's own
    ///    internal-temporaries allocator rule).
    /// 3. the animal gate: a null `animal_ptr` passes; otherwise the tile's path cost - the unnamed
    ///    `BFUnit` vtable `+0x164` slot ([`call_bfunit_tile_cost_vtable_slot`], the same
    ///    `getTerrainCost`-family dispatch [`Self::get_random_clear_tile_ahead`] uses) - must not
    ///    equal the shared [`MAX_PATH_COST_RVA`] "unreachable" sentinel, by full-width equality (the
    ///    macOS `.asm`'s own `cmpw`/`beq` pair). The Windows `.c`'s `vftptr_0x0[1]`/`CONCAT31` render
    ///    of this call is Ghidra struct garbage - the real dispatch is the two-arg `(unit, tile)`
    ///    virtual, `+0x16c` in the macOS table / `+0x164` (slot 89) in the Windows one, and the
    ///    Windows `.asm` export itself stops mid-prologue, so the macOS listing carries the call
    ///    shape.
    /// 4. when `check_path` is set and an animal was given (real vanilla's own parameter is what the
    ///    plan glosses as `check_occupied`, but it gates exactly this), `BFAIMgr::checkPath`
    ///    ([`BFAIMGR_CHECK_PATH`] undetoured `.original()` call-through) must report the animal's
    ///    current tile reaches the candidate.
    ///
    /// The animal's current tile ([`BFENTITY_GET_TILE`], a pure field read) and the AI-mgr global are
    /// read once before the walk - nothing in the loop can move the animal or swap the global.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_clear_tiles(&self, out_vector_ptr: u32, animal_ptr: u32, check_path: bool) {
        let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        let animal_tile = if animal_ptr != 0 {
            (unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) }) as u32
        } else {
            0
        };
        let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            if get_from_memory::<u32>(tile + 0x4) != 0
                || get_from_memory::<u32>(tile + 0x8) != 0
                || get_from_memory::<u32>(tile + 0xc) != 0
                || get_from_memory::<u32>(tile + 0x10) != 0
            {
                continue;
            }
            let list_head: u32 = get_from_memory(tile);
            if get_from_memory::<u32>(list_head) != list_head {
                continue;
            }
            if animal_ptr != 0 && unsafe { call_bfunit_tile_cost_vtable_slot(animal_ptr, tile) } == max_cost {
                continue;
            }
            if check_path && animal_ptr != 0 {
                let reachable = low_byte_bool(unsafe {
                    BFAIMGR_CHECK_PATH.original()(
                        ai_mgr_ptr as *const u32,
                        animal_tile as *const u32,
                        tile as *const u32,
                        animal_ptr as *const u32,
                    )
                });
                if !reachable {
                    continue;
                }
            }
            vector_push_pool_alloc4(out_vector_ptr, tile);
        }
    }

    /// Ports `ZTHabitat::getRandomClearTile` overload 0 (`ZTHabitat_getRandomClearTile_0.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_CLEAR_TILE_0`): the two-line delegator - one
    /// [`Self::get_random_animal`] draw, forwarded straight into the animal-taking overload
    /// ([`Self::get_random_clear_tile_for_animal`], a direct sibling call matching real vanilla's own
    /// call into the now-detoured overload-1 address, which re-enters this port under the live battery).
    /// A null draw is forwarded like any other: real vanilla performs no early-out, and the overload-1
    /// body itself gates only on the candidate pool's own emptiness.
    ///
    /// The `.asm`'s tail (`CALL getRandomClearTile`; `POP ESI`; `RET 0x8`) never touches `EAX`, so the
    /// overload-1 result rides through as the return, and both real callers
    /// (`ZTGoalPutFood::decide`, [`Self::get_near_clear_tile`], each passing `false`/`false`) consume it.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_clear_tile_default(&self, check_path: bool, subhabs: bool) -> u32 {
        let animal = self.get_random_animal();
        self.get_random_clear_tile_for_animal(animal, check_path, subhabs)
    }

    /// Ports `ZTHabitat::getRandomClearTile` overload 1 (`ZTHabitat_getRandomClearTile_1.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_CLEAR_TILE_1`; the macOS decompile `ZTHabitat_getRandomClearTile.c`
    /// is this same overload and corroborates every step): gathers the clear-tile candidate pool into
    /// the same zero-initialized scratch `std::vector<BFTile*>` real vanilla's own stack-local is (the
    /// `.asm`'s three zero stores) - [`Self::add_clear_tiles`] for `this`, then one call per amphibious
    /// neighbor when `subhabs` is set, walked in [`walk_neighbor_tree`] order with the habitat payload
    /// read from node `+0x10` (the same tree the dirty-gated count getters recurse; each call is the
    /// call-the-port convention - real vanilla calls `addClearTiles` at its now-detoured address, which
    /// re-enters this file's port under the live battery). `animal_ptr` may be null - [`Self::add_clear_tiles`]
    /// treats it as its own short-circuit, so the pool degenerates to "every unoccupied owned tile".
    ///
    /// When the pool is non-empty, advances `DAT_00638060` with exactly one MSVC LCG step ([`lcg_next`])
    /// and returns the tile at index `(seed >> 0x10 & 0x7fff) % count` (on a `u32` the shift/mask pair
    /// equals the `.asm`'s `SAR 0x10` + `AND 0x7fff`, and the modulo is the same unsigned `DIV`); an
    /// empty pool returns null without touching the shared game RNG state (the `.asm`'s emptiness `JZ`
    /// sits before the `IMUL`/`ADD` pair, same shape as [`Self::get_random_animal`]). The scratch
    /// buffer is freed by **capacity** via [`free_event_vector_buffer`] - the decompile's own tail (the
    /// same manual freelist-bucket/`operator_delete` split, not a `PoolAlloc::deallocate` call).
    ///
    /// The real body's `GLOBAL_ZTWorldMgr == 0xfffffff8` "world not initialized" guard is dead in
    /// practice, like [`Self::validate_positions`]'s own identical guard - checked here as
    /// `GLOBAL_ZTWorldMgr != 0` instead (equivalent for every real value, and additionally guards the
    /// one case the real check cannot).
    ///
    /// The one caller passing a real animal is `ZTAnimal::fCheckReproduction` (the egg-laying siting
    /// pick, `(animal, false, true)`), which consumes the return; the other path in is overload 0's
    /// delegation above.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_random_clear_tile_for_animal(&self, animal_ptr: u32, check_path: bool, subhabs: bool) -> u32 {
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return 0;
        }
        let mut scratch_vector = [0u32; 3];
        self.add_clear_tiles(scratch_vector.as_mut_ptr() as u32, animal_ptr, check_path);
        if subhabs {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }
                    .add_clear_tiles(scratch_vector.as_mut_ptr() as u32, animal_ptr, check_path);
            }
        }
        let begin = scratch_vector[0];
        let picked = if scratch_vector[1] != begin {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            let count = (scratch_vector[1] - begin) >> 2;
            let index = ((rng >> 0x10) & 0x7fff) % count;
            get_from_memory::<u32>(begin + index * 4)
        } else {
            0
        };
        free_event_vector_buffer(begin, scratch_vector[2].wrapping_sub(begin));
        picked
    }

    /// Ports `ZTHabitat::getAdjacentClearTile` (`ZTHabitat_getAdjacentClearTile.c`/`.asm`, `generated.rs`'s
    /// `GET_ADJACENT_CLEAR_TILE`): despite the `ZTHabitat::` Ghidra namespace this is a genuine free
    /// function - `stdcall`, no `this` - that reaches everything through the `GLOBAL_ZTWorldMgr`/
    /// `GLOBAL_ZTHabitatMgr` globals instead. The macOS decompile confirms the same shape: its own
    /// 3-parameter signature carries an unused leading `param_1` (an OOAnalyzer namespace-inference
    /// artifact, never read in the body) alongside the two real arguments - `param_2` (the unit, used
    /// only for the cost-vtable dispatch below) and `param_3` (`base_tile`, used for everything else) -
    /// matching the Windows `.asm`'s own 2-argument `stdcall` exactly.
    ///
    /// Scans `base_tile`'s 8 neighboring map coordinates in real vanilla's own loop order (dx outer,
    /// -1/0/1; dy inner, -1/0/1; the `(0,0)` self tile skipped) - this order matters, since the final
    /// random pick indexes into the candidate list built in this order. A neighbor coordinate qualifies
    /// when it is within the live map's own bounds ([`crate::ztworldmgr::ZTWorldMgr::map_x_size`]/
    /// `map_y_size`), is occupied by the same habitat as `base_tile`
    /// ([`crate::zthabitatmgr::ZTHabitatMgr::get_habitat_ptr`] on both sides - reproducing real vanilla's
    /// own raw, unchecked grid-cell dereference exactly, including its `0 == 0` "both tiles ownerless"
    /// pass-through, since every neighbor here is already confirmed in-bounds), and is affordable to
    /// `unit` - the unnamed `BFUnit` vtable `+0x164` slot ([`call_bfunit_tile_cost_vtable_slot`], the
    /// same `getTerrainCost`-family dispatch [`Self::add_clear_tiles`]/[`Self::get_random_clear_tile_ahead`]
    /// use) must not exactly equal the shared [`MAX_PATH_COST_RVA`] "unreachable" sentinel (a full-width
    /// `i32` comparison, not a bool - the Windows `.c`'s own `CONCAT31`/`vftptr_0x0[1]` render of this
    /// call site is Ghidra struct garbage; the macOS listing's plain two-arg virtual dispatch at the
    /// platform's own `+0x16c` slot settles the real shape, same corroboration as [`Self::add_clear_tiles`]).
    ///
    /// A non-empty candidate list advances `DAT_00638060` with exactly one MSVC LCG step ([`lcg_next`])
    /// and returns the tile at index `(seed >> 0x10 & 0x7fff) % count`; an empty list returns `base_tile`
    /// itself, untouched, without advancing the shared RNG state (the `.asm`'s own count check gates the
    /// `IMUL`/`ADD` pair) - unlike [`Self::get_random_tile_in_direction`]/[`Self::get_random_clear_tile_ahead`]'s
    /// own empty-list fallback to a *different* random draw ([`Self::get_random_tile`]), this function's
    /// empty-list tail is a plain, RNG-free pass-through of its own input tile. The real world/habitat-
    /// manager/`base_tile` null guards (`&GLOBAL_ZTWorldMgr->field_0x8 != 0`, `GLOBAL_ZTHabitatMgr != 0`,
    /// `base_tile != 0`) are checked here as `GLOBAL_ZTWorldMgr != 0` (dead in practice for the first,
    /// like this file's other `0xfffffff8`-sentinel guards - see [`Self::get_random_clear_tile_for_animal`]'s
    /// own note) alongside the other two, all short-circuiting to `base_tile` unchanged.
    pub fn get_adjacent_clear_tile(unit_ptr: u32, base_tile_ptr: u32) -> u32 {
        let world_ptr = globals().ztworldmgr_ptr() as u32;
        let habitat_mgr_ptr = globals().zthabitatmgr_ptr() as u32;
        if world_ptr == 0 || habitat_mgr_ptr == 0 || base_tile_ptr == 0 {
            return base_tile_ptr;
        }
        let world = globals().ztworldmgr();
        let habitat_mgr = globals().zthabitatmgr();
        let base_tile = get_from_memory::<BFTile>(base_tile_ptr);
        let base_x = base_tile.pos.x;
        let base_y = base_tile.pos.y;
        let base_habitat = habitat_mgr.get_habitat_ptr(base_x, base_y);
        let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);

        let mut candidates: Vec<u32> = Vec::new();
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let cand_x = base_x + dx;
                let cand_y = base_y + dy;
                if cand_x < 0 || cand_y < 0 || cand_x as u32 >= world.map_x_size || cand_y as u32 >= world.map_y_size {
                    continue;
                }
                if habitat_mgr.get_habitat_ptr(cand_x, cand_y) != base_habitat {
                    continue;
                }
                let candidate_tile_ptr = world.get_tile_ptr(cand_x as u32, cand_y as u32);
                let cost = unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, candidate_tile_ptr) };
                if cost == max_cost {
                    continue;
                }
                candidates.push(candidate_tile_ptr);
            }
        }

        if candidates.is_empty() {
            return base_tile_ptr;
        }
        let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
        let rng = lcg_next(get_from_memory::<u32>(rng_addr));
        save_to_memory(rng_addr, rng);
        let index = ((rng >> 0x10) & 0x7fff) % candidates.len() as u32;
        candidates[index as usize]
    }

    /// Ports `ZTHabitat::getNearestClearTile` (`generated.rs`'s `GET_NEAREST_CLEAR_TILE`, IDA listing
    /// of `sub_48AC91`): scans every owned tile in [`walk_tile_list`] order for the one closest to
    /// `unit`'s current tile that `unit` could actually stand on, returning it as a raw tile pointer
    /// (`0` when nothing qualifies or the early guards fail). Deterministic - the walk has no own
    /// candidate list and no draw of its own; the only RNG use is [`Self::get_random_animal`]'s
    /// internal draw (one `DAT_00638060` advance when the habitat has animals, none when it
    /// doesn't), whose result gates only the reachability check below.
    ///
    /// Guards, in order, all returning `0` without touching the RNG: the world global
    /// (real vanilla's dead `GLOBAL_ZTWorldMgr == 0xfffffff8` sentinel, ported as the equivalent
    /// `!= 0` check like this file's other guards), a null `unit_ptr`, and a null
    /// `BFEntity::getTile(unit)` ([`BFENTITY_GET_TILE`], a pure field read).
    ///
    /// Per candidate tile, in the IDA listing's own order:
    ///
    /// 1. squared Cartesian distance from `unit`'s tile (`BFTile` pos at `+0x34`/`+0x38`, read as
    ///    `i32`; Windows inlines the `dx*dx + dy*dy`, macOS calls `BFMap::distanceCartesianSquared`;
    ///    either tile null -> `0x7fffffff`) must be **strictly below** the best so far - first tile in
    ///    walk order wins ties;
    /// 2. the four direct-entity slots at `tile+0x4..+0x10` all null (the same block
    ///    [`Self::add_clear_tiles`] checks);
    /// 3. the entity list at `tile+0x0` empty - real vanilla builds a throwaway `std::list<uint>`
    ///    range-copy of it and consumes that only as `count <= 0`, so the port reads the list's own
    ///    emptiness and reproduces none of the copy/`PoolAlloc` teardown (same internal-temporaries
    ///    rule as [`Self::add_clear_tiles`]);
    /// 4. the path cost (the unnamed `BFUnit` vtable `+0x164` slot, [`call_bfunit_tile_cost_vtable_slot`])
    ///    must be **strictly below** the shared [`MAX_PATH_COST_RVA`] "unreachable" sentinel (IDA
    ///    renders the sentinel as literal 20; read live here like everywhere else). The strict
    ///    comparison is this function's own shape: [`Self::add_clear_tiles`],
    ///    [`Self::get_adjacent_clear_tile`] and [`Self::get_near_clear_tile`] all use full-width
    ///    `!=` against the same global;
    /// 5. when the [`Self::get_random_animal`] draw returned a real animal,
    ///    `BFAIMgr::checkPath` ([`BFAIMGR_CHECK_PATH`] undetoured `.original()` call-through) must
    ///    report that animal's current tile reaches the candidate - the mover is the drawn animal
    ///    itself (`checkPath(mgr, getTile(animal), tile, animal)`), the same settled shape
    ///    [`Self::get_near_clear_tile`]'s `.asm` shows explicitly.
    ///
    /// An accepting tile updates the running best; the final best rides out as the return. The
    /// animal's tile and the AI-mgr global are read once before the walk - nothing in the loop can
    /// move the animal or swap the global (same hoisting [`Self::add_clear_tiles`] documents).
    ///
    /// The Windows `.asm` export of this function is truncated mid-body (stops after the prologue and
    /// early guards, same phenomenon as [`Self::add_clear_tiles`]'s own truncated export); the IDA
    /// listing is the ground truth for the loop, and the macOS decompile corroborates the check order
    /// and accept-shape (`best = tile; best_dist = dist`) step-for-step.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_nearest_clear_tile(&self, unit_ptr: u32) -> u32 {
        if globals().ztworldmgr_ptr() as u32 == 0 {
            return 0;
        }
        if unit_ptr == 0 {
            return 0;
        }
        let unit_tile = unsafe { BFENTITY_GET_TILE.original()(unit_ptr as *const u32) } as u32;
        if unit_tile == 0 {
            return 0;
        }
        let random_animal = self.get_random_animal();
        let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
        let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        let animal_tile = if random_animal != 0 {
            (unsafe { BFENTITY_GET_TILE.original()(random_animal as *const u32) }) as u32
        } else {
            0
        };
        let mut best_tile = 0u32;
        let mut best_dist = 0x7fff_ffffi32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            let dist: i32 = if tile == 0 {
                0x7fff_ffff
            } else {
                let dx: i32 = get_from_memory::<i32>(unit_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
                let dy: i32 = get_from_memory::<i32>(unit_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
            };
            if dist >= best_dist {
                continue;
            }
            if get_from_memory::<u32>(tile + 0x4) != 0
                || get_from_memory::<u32>(tile + 0x8) != 0
                || get_from_memory::<u32>(tile + 0xc) != 0
                || get_from_memory::<u32>(tile + 0x10) != 0
            {
                continue;
            }
            let list_head: u32 = get_from_memory(tile);
            if get_from_memory::<u32>(list_head) != list_head {
                continue;
            }
            if unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, tile) } >= max_cost {
                continue;
            }
            if random_animal != 0 {
                let reachable = low_byte_bool(unsafe {
                    BFAIMGR_CHECK_PATH.original()(
                        ai_mgr_ptr as *const u32,
                        animal_tile as *const u32,
                        tile as *const u32,
                        random_animal as *const u32,
                    )
                });
                if !reachable {
                    continue;
                }
            }
            best_tile = tile;
            best_dist = dist;
        }
        best_tile
    }

    /// Ports `ZTHabitat::getNearClearTile` (`ZTHabitat_getNearClearTile.c`/`.asm`,
    /// `generated.rs`'s `GET_NEAR_CLEAR_TILE`): gathers the owned tiles "near" `unit` that it could
    /// stand on into the same zero-initialized scratch `std::vector<BFTile*>` real vanilla's own
    /// stack-local is, picks one at random, and falls back through
    /// [`Self::get_nearest_clear_tile`] -> [`Self::get_random_clear_tile_default`]`(false, false)`
    /// when the pool comes out empty (or the picked element is null - the `.asm`'s null-pick test
    /// sits *after* the LCG advance, so even that impossible-on-real-tiles path consumes its draw).
    /// Real caller: `ZTGoalPutFood::decide`, passing the keeper as `unit` and its target animal as
    /// `animal`.
    ///
    /// The plan's own `getNearClearTile(ZTUnit* unit, BFTile* ref_tile)` gloss is wrong: the second
    /// parameter is a **`ZTAnimal*`** - the `checkPath` source/mover, nullable, never used as a
    /// reference tile (macOS prototype comment `(ZTKeeper *, ZTAnimal *)`, Windows `.asm` `RET 0x8`).
    ///
    /// Guards, in order, all returning `0` without touching the RNG: the world global (the same
    /// `0xfffffff8`-sentinel convention as [`Self::get_nearest_clear_tile`]), a null `unit_ptr`, and
    /// a null `BFEntity::getTile(unit)`.
    ///
    /// Per owned tile ([`walk_tile_list`] order - the order that decides which candidate the pick
    /// indexes), checks in the `.asm`'s own order:
    ///
    /// 1. **not** in the unit's reserved-tile vector at `unit+0x27c..+0x280` - the Windows inline of
    ///    `ZTStaff::isInvalidTile` (macOS calls it by name; same linear `begin..end` scan, whose
    ///    macOS field offsets are the platform's own `+0x244`/`+0x248`). The vector's meaning beyond
    ///    this function is unconfirmed, so it stays raw;
    /// 2. the four direct-entity slots at `tile+0x4..+0x10` all null (same block as
    ///    [`Self::add_clear_tiles`]);
    /// 3. the entity list at `tile+0x0` empty - same throwaway-list-copy/emptiness-read shape as
    ///    [`Self::add_clear_tiles`]/[`Self::get_nearest_clear_tile`];
    /// 4. the path cost ([`call_bfunit_tile_cost_vtable_slot`]) **must not equal** the shared
    ///    [`MAX_PATH_COST_RVA`] sentinel - full-width `!=` here, unlike
    ///    [`Self::get_nearest_clear_tile`]'s strict `<` against the same global;
    /// 5. the tile is not [`Self::get_gate_tile_in`]'s tile - resolved once before the walk (nothing
    ///    in the loop can move the gate) via the port (the detoured `GET_GATE_TILE_IN` address, so
    ///    real vanilla's own call re-enters it under the battery). A habitat without a usable gate
    ///    resolves to a null tile pointer no candidate can equal, so every tile passes - exactly the
    ///    real comparison's own behavior against a null return;
    /// 6. squared Cartesian distance from `unit`'s tile **below 10** (unsigned compare against the
    ///    literal, `.asm` `CMP %EAX, 0xa` / `JC`; either tile null -> `0x7fffffff`);
    /// 7. when `animal_ptr` is non-null, `BFAIMgr::checkPath`
    ///    ([`BFAIMGR_CHECK_PATH`] undetoured `.original()` call-through) must report the animal's
    ///    current tile reaches the candidate - the mover is the animal itself;
    /// 8. bit `0x4` of the flag byte at `tile+0x85` clear (the same unconfirmed-flag read
    ///    [`Self::get_num_sickly_animals`]/[`Self::get_nearest_sick_animal`] make; reproduced raw -
    ///    no corroborating name anywhere in the corpus).
    ///
    /// A tile passing everything is pushed ([`vector_push_pool_alloc4`]). The non-empty pool
    /// advances `DAT_00638060` with exactly one MSVC LCG step ([`lcg_next`]) and returns the tile at
    /// index `(seed >> 0x10 & 0x7fff) % count`; an empty pool falls back without touching the shared
    /// RNG state (the `.asm`'s emptiness `JZ` sits before the `IMUL`/`ADD` pair). The scratch buffer
    /// is freed by **capacity** via [`free_event_vector_buffer`] - the `.asm`'s own tail. Both
    /// fallback legs are detoured addresses real vanilla calls directly, so the port calls the
    /// sibling ports (call-the-port convention, same as [`Self::get_gate_tile_pass_in`]'s
    /// composition); the chain *does* advance the RNG when the habitat has animals, through
    /// [`Self::get_nearest_clear_tile`]'s internal [`Self::get_random_animal`] draw.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_near_clear_tile(&self, unit_ptr: u32, animal_ptr: u32) -> u32 {
        if globals().ztworldmgr_ptr() as u32 == 0 {
            return 0;
        }
        if unit_ptr == 0 {
            return 0;
        }
        let unit_tile = unsafe { BFENTITY_GET_TILE.original()(unit_ptr as *const u32) } as u32;
        if unit_tile == 0 {
            return 0;
        }
        let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
        let max_cost: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        let gate_tile_ptr = self.gate_tile_in_ptr();
        let animal_tile = if animal_ptr != 0 {
            (unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) }) as u32
        } else {
            0
        };
        let invalid_begin: u32 = get_from_memory(unit_ptr + 0x27c);
        let invalid_end: u32 = get_from_memory(unit_ptr + 0x280);
        let mut scratch_vector = [0u32; 3];
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            let mut reserved = false;
            let mut cursor = invalid_begin;
            while cursor != invalid_end {
                if get_from_memory::<u32>(cursor) == tile {
                    reserved = true;
                    break;
                }
                cursor += 4;
            }
            if reserved {
                continue;
            }
            if get_from_memory::<u32>(tile + 0x4) != 0
                || get_from_memory::<u32>(tile + 0x8) != 0
                || get_from_memory::<u32>(tile + 0xc) != 0
                || get_from_memory::<u32>(tile + 0x10) != 0
            {
                continue;
            }
            let list_head: u32 = get_from_memory(tile);
            if get_from_memory::<u32>(list_head) != list_head {
                continue;
            }
            if unsafe { call_bfunit_tile_cost_vtable_slot(unit_ptr, tile) } == max_cost {
                continue;
            }
            if tile == gate_tile_ptr {
                continue;
            }
            let dist: u32 = if tile == 0 {
                0x7fff_ffff
            } else {
                let dx: i32 = get_from_memory::<i32>(unit_tile + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
                let dy: i32 = get_from_memory::<i32>(unit_tile + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
                (dx.wrapping_mul(dx) as u32).wrapping_add(dy.wrapping_mul(dy) as u32)
            };
            if dist >= 10 {
                continue;
            }
            if animal_ptr != 0 {
                let reachable = low_byte_bool(unsafe {
                    BFAIMGR_CHECK_PATH.original()(
                        ai_mgr_ptr as *const u32,
                        animal_tile as *const u32,
                        tile as *const u32,
                        animal_ptr as *const u32,
                    )
                });
                if !reachable {
                    continue;
                }
            }
            if get_from_memory::<u8>(tile + 0x85) & 4 != 0 {
                continue;
            }
            vector_push_pool_alloc4(scratch_vector.as_mut_ptr() as u32, tile);
        }
        let begin = scratch_vector[0];
        let mut picked = 0u32;
        if scratch_vector[1] != begin {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            let count = (scratch_vector[1] - begin) >> 2;
            let index = ((rng >> 0x10) & 0x7fff) % count;
            picked = get_from_memory::<u32>(begin + index * 4);
        }
        free_event_vector_buffer(begin, scratch_vector[2].wrapping_sub(begin));
        if picked != 0 {
            return picked;
        }
        let nearest = self.get_nearest_clear_tile(unit_ptr);
        if nearest != 0 {
            return nearest;
        }
        self.get_random_clear_tile_default(false, false)
    }

    /// Ports `ZTHabitat::getNearestClearWaterTile` (`ZTHabitat_getNearestClearWaterTile.c`/`.asm`,
    /// `generated.rs`'s `GET_NEAREST_CLEAR_WATER_TILE`): scans the habitat's own owned tiles for the
    /// water tile nearest `ref_tile_ptr` and returns it (null when nothing qualifies). Real caller:
    /// `ZTGoalDrinkWater::decide`, passing `BFEntity::getTile(entity)` - the asking animal's own
    /// tile - as the reference point.
    ///
    /// The plan's own walkthrough is wrong on both counts: the decompile never calls
    /// `getWaterTiles` (it walks [`Self::owned_tiles_ptr`] directly), and the parameter is a
    /// **`BFTile*`** reference point (macOS prototype comment `(BFTile *)`), not a `ZTUnit*`.
    ///
    /// Fully deterministic - no RNG draw, no `characteristics_dirty` recalculate. The world global
    /// guard (`GLOBAL_ZTWorldMgr + 8 == 0`, the same `0xfffffff8` sentinel as
    /// [`Self::get_nearest_clear_tile`]) returns null; the port uses the shared `!= 0` convention.
    /// Per owned tile ([`walk_tile_list`] order - the order that decides which candidate wins
    /// ties), two filters:
    ///
    /// 1. the water-class flag `tile+0x83 & 3` set - the exact filter real vanilla's own
    ///    `addWaterTiles` applies (`ZTHabitat_addWaterTiles.c`; the inverse of the read
    ///    [`Self::get_num_sickly_animals`] makes against the same byte);
    /// 2. the byte at `tile+0x80` not `0xa` - an exclusion `addWaterTiles` lacks; no corroborating
    ///    name for that byte anywhere in the corpus, reproduced raw.
    ///
    /// The distance is the Windows inline of `BFMap::distanceCartesianSquared` - plain
    /// `dx*dx + dy*dy` between the reference tile's and the candidate's raw `+0x34`/`+0x38`
    /// coordinates; the map pointer the macOS decompile passes is never read by the inlined math.
    /// Either tile null -> `0x7fffffff` (the candidate-null check is dead - the flag filter above
    /// already dereferenced the tile - kept for decompile fidelity). The best-tracking keeps the
    /// first candidate unconditionally and replaces only on strictly smaller distance
    /// (`.asm` `TEST %EDI, %EDI` / `CMP %EAX, %EBP` / `JGE`), so a null reference tile - every
    /// candidate at `0x7fffffff` - still selects the first filter-passing tile in walk order, and
    /// distance ties go to the earlier tile.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_nearest_clear_water_tile(&self, ref_tile_ptr: u32) -> u32 {
        if globals().ztworldmgr_ptr() as u32 == 0 {
            return 0;
        }
        let mut best_tile = 0u32;
        let mut best_dist = 0x7fff_ffffi32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            if get_from_memory::<u8>(tile + 0x83) & 3 == 0 || get_from_memory::<u8>(tile + 0x80) == 0xa {
                continue;
            }
            let dist: i32 = if tile == 0 || ref_tile_ptr == 0 {
                0x7fff_ffff
            } else {
                let dx: i32 = get_from_memory::<i32>(ref_tile_ptr + 0x34).wrapping_sub(get_from_memory::<i32>(tile + 0x34));
                let dy: i32 = get_from_memory::<i32>(ref_tile_ptr + 0x38).wrapping_sub(get_from_memory::<i32>(tile + 0x38));
                dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))
            };
            if best_tile == 0 || dist < best_dist {
                best_tile = tile;
                best_dist = dist;
            }
        }
        best_tile
    }

    /// Shared owned-tile walk behind the three biome-classification filters below
    /// (`ZTHabitat_addLandTiles.c`/`_addWaterTiles.c`/`_addUnderwaterTiles.c` - three byte-for-byte
    /// identical `.asm` bodies apart from the predicate): walks [`Self::owned_tiles_ptr`]
    /// ([`walk_tile_list`]), reads each node's `BFTile*` payload at `+0x8`, and pushes the tiles whose
    /// predicate passes onto the real vanilla `std::vector<BFTile*>` out-param at `out_vector_ptr`
    /// ([`vector_push_pool_alloc4_pool_dealloc`] - the `PoolAlloc::allocate`-doubling-growth/
    /// `PoolAlloc::deallocate`-teardown shape all three decompiles' own inlined push-backs use).
    /// Deterministic on all three: no RNG draw, no `characteristics_dirty` recalculate.
    fn add_tiles_matching(&self, out_vector_ptr: u32, tile_qualifies: impl Fn(u32) -> bool) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            if tile_qualifies(tile) {
                vector_push_pool_alloc4_pool_dealloc(out_vector_ptr, tile);
            }
        }
    }

    /// Ports `ZTHabitat::addLandTiles` (`ZTHabitat_addLandTiles.c`/`.asm`, `generated.rs`'s
    /// `ADD_LAND_TILES`): appends every owned tile with the land-class bit set (`tile+0x85 & 0x20`)
    /// onto the real vanilla `std::vector<BFTile*>` out-param at `out_vector_ptr`. The macOS decompile
    /// applies the same tri-partite structure through its own bitfield packing (dedicated bit
    /// `byte+0x8d & 4` at byte address +8 from Windows', water bits `byte+0x8b >> 6`); the Windows
    /// `.asm` is authoritative for this port. The plan's "filters non-water terrain tiles" gloss is
    /// imprecise: land and water are **independent bits** - a tile can carry both, and this filter
    /// takes the land bit alone. What the triplet guarantees is union coverage (every owned tile
    /// lands in at least one of [`Self::add_land_tiles`]/[`Self::add_water_tiles`]/
    /// [`Self::add_underwater_tiles`]: land bit set -> land; water bits set -> water; both clear ->
    /// underwater); exclusivity is only empirical, asserted by no decompile. The `& 4` bit at the
    /// same `+0x85` byte is the unrelated sickly-animal tile flag [`Self::get_num_sickly_animals`]
    /// reads.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_land_tiles(&self, out_vector_ptr: u32) {
        self.add_tiles_matching(out_vector_ptr, |tile| get_from_memory::<u8>(tile + 0x85) & 0x20 != 0);
    }

    /// Ports `ZTHabitat::addWaterTiles` (`ZTHabitat_addWaterTiles.c`/`.asm`, `generated.rs`'s
    /// `ADD_WATER_TILES`): appends every owned tile with the water-class flags set
    /// (`tile+0x83 & 3 != 0` - the same two-bit read [`Self::get_nearest_clear_water_tile`] filters on;
    /// macOS byte+0x8b bits 6-7) onto the real vanilla `std::vector<BFTile*>` out-param at
    /// `out_vector_ptr`. Land (`tile+0x85 & 0x20`) is not consulted - see [`Self::add_land_tiles`]'s
    /// union-vs-exclusivity note. Unlike [`Self::get_nearest_clear_water_tile`] there is no
    /// `tile+0x80 != 0xa` exclusion: the water-class flags are the whole predicate.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_water_tiles(&self, out_vector_ptr: u32) {
        self.add_tiles_matching(out_vector_ptr, |tile| get_from_memory::<u8>(tile + 0x83) & 3 != 0);
    }

    /// Ports `ZTHabitat::addUnderwaterTiles` (`ZTHabitat_addUnderwaterTiles.c`/`.asm`,
    /// `generated.rs`'s `ADD_UNDERWATER_TILES`): appends every owned tile with **both** the
    /// water-class flags and the land-class bit clear (`tile+0x83 & 3 == 0 && tile+0x85 & 0x20 == 0`)
    /// onto the real vanilla `std::vector<BFTile*>` out-param at `out_vector_ptr` - the triplet's
    /// catch-all complement, not a dedicated bit (the plan's `water_level > 0` gloss matches no
    /// decompiled check; on macOS it's this function that owns the dedicated `byte+0x8d & 4` bit, with
    /// land as the catch-all - the roles swap between platforms, the tri-partite structure doesn't).
    /// The plan's "submerged tank tiles" reading is at best the empirical intent; the decompiled
    /// predicate is exactly this negation.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_underwater_tiles(&self, out_vector_ptr: u32) {
        self.add_tiles_matching(out_vector_ptr, |tile| {
            get_from_memory::<u8>(tile + 0x83) & 3 == 0 && get_from_memory::<u8>(tile + 0x85) & 0x20 == 0
        });
    }

    /// Shared recursive aggregation behind the three biome-tile getters below
    /// (`ZTHabitat_getLandTiles.c`/`_getWaterTiles.c`/`_getUnderwaterTiles.c` - three byte-for-byte
    /// identical `.asm` bodies apart from the callee): one call to the matching
    /// [`Self::add_tiles_matching`]-sibling for `this`, then an in-order [`walk_neighbor_tree`] walk
    /// over [`Self::amphibious_neighbors_head`] calling the same sibling per neighbor into the same
    /// out-vector, the habitat payload read from node `+0x10` (each sibling call is the call-the-port
    /// convention - real vanilla calls `add*Tiles` at their now-detoured addresses, which re-enter
    /// this file's ports under the live battery, the same shape as
    /// [`Self::get_random_clear_tile_for_animal`]'s subhabitat loop). Deterministic on all three: no
    /// RNG draw, no `characteristics_dirty` recalculate.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    fn get_tiles_aggregating(&self, out_vector_ptr: u32, add_tiles: impl Fn(&Self, u32)) {
        add_tiles(self, out_vector_ptr);
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            add_tiles(unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }, out_vector_ptr);
        }
    }

    /// Ports `ZTHabitat::getLandTiles` (`ZTHabitat_getLandTiles.c`/`.asm`, `generated.rs`'s
    /// `GET_LAND_TILES`): appends every land-class tile of `this` **and every amphibious neighbor**
    /// onto the real vanilla `std::vector<BFTile*>` out-param at `out_vector_ptr` - the recursive
    /// aggregator wrapping [`Self::add_land_tiles`] via [`Self::get_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_land_tiles(&self, out_vector_ptr: u32) {
        self.get_tiles_aggregating(out_vector_ptr, Self::add_land_tiles);
    }

    /// Ports `ZTHabitat::getWaterTiles` (`ZTHabitat_getWaterTiles.c`/`.asm`, `generated.rs`'s
    /// `GET_WATER_TILES`): the recursive aggregator wrapping [`Self::add_water_tiles`] - see
    /// [`Self::get_land_tiles`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_water_tiles(&self, out_vector_ptr: u32) {
        self.get_tiles_aggregating(out_vector_ptr, Self::add_water_tiles);
    }

    /// Ports `ZTHabitat::getUnderwaterTiles` (`ZTHabitat_getUnderwaterTiles.c`/`.asm`,
    /// `generated.rs`'s `GET_UNDERWATER_TILES`): the recursive aggregator wrapping
    /// [`Self::add_underwater_tiles`] - see [`Self::get_land_tiles`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_underwater_tiles(&self, out_vector_ptr: u32) {
        self.get_tiles_aggregating(out_vector_ptr, Self::add_underwater_tiles);
    }

    /// Shared count-only wrapper behind the three biome-tile count getters below
    /// (`ZTHabitat_getNumLandTiles.c`/`_getNumWaterTiles.c`/`_getNumUnderwaterTiles.c` - three
    /// otherwise byte-for-byte identical `.asm` bodies apart from the callee): builds the same
    /// zero-initialized 12-byte stack scratch `std::vector<BFTile*>` real vanilla's own frame is,
    /// calls the matching [`Self::get_tiles_aggregating`] getter into it, computes
    /// `(end - begin) >> 2`, and only then frees the scratch buffer via [`free_event_vector_buffer`]
    /// by **byte capacity** (the same manual freelist-bucket/`operator_delete` split, not a
    /// `PoolAlloc::deallocate` call). The count comes out **before** the teardown on purpose: the
    /// freelist push overwrites the buffer's first word (`begin` itself), so the `.asm` stows the
    /// count in `ESI` across it. The `.asm`'s `SAR 2`/`SHL 2` round-trip on the byte capacity is a
    /// no-op - a `BFTile*` vector's byte size is always a multiple of 4 - so the port passes the raw
    /// subtraction.
    ///
    /// Deterministic on all three: no RNG draw, no `characteristics_dirty` recalculate.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    fn get_num_tiles_aggregating(&self, get_tiles: impl Fn(&Self, u32)) -> i32 {
        let mut scratch_vector = [0u32; 3];
        get_tiles(self, scratch_vector.as_mut_ptr() as u32);
        let count = (scratch_vector[1] as i32).wrapping_sub(scratch_vector[0] as i32) >> 2;
        free_event_vector_buffer(scratch_vector[0], scratch_vector[2].wrapping_sub(scratch_vector[0]));
        count
    }

    /// Ports `ZTHabitat::getNumLandTiles` (`ZTHabitat_getNumLandTiles.c`/`.asm`, `generated.rs`'s
    /// `GET_NUM_LAND_TILES`): the count-only wrapper over [`Self::get_land_tiles`] - see
    /// [`Self::get_num_tiles_aggregating`]. Real vanilla's sole caller (`ZTAnimal::doWaterCheck`)
    /// consumes the full-width signed return against per-species thresholds without truncating it.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_land_tiles(&self) -> i32 {
        self.get_num_tiles_aggregating(Self::get_land_tiles)
    }

    /// Ports `ZTHabitat::getNumWaterTiles` (`ZTHabitat_getNumWaterTiles.c`/`.asm`, `generated.rs`'s
    /// `GET_NUM_WATER_TILES`): the count-only wrapper over [`Self::get_water_tiles`] - see
    /// [`Self::get_num_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_water_tiles(&self) -> i32 {
        self.get_num_tiles_aggregating(Self::get_water_tiles)
    }

    /// Ports `ZTHabitat::getNumUnderwaterTiles` (`ZTHabitat_getNumUnderwaterTiles.c`/`.asm`,
    /// `generated.rs`'s `GET_NUM_UNDERWATER_TILES`): the count-only wrapper over
    /// [`Self::get_underwater_tiles`] - see [`Self::get_num_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_underwater_tiles(&self) -> i32 {
        self.get_num_tiles_aggregating(Self::get_underwater_tiles)
    }

    /// Shared random-draw wrapper behind the three biome-tile random getters below
    /// (`ZTHabitat_getRandomLandTile.c`/`_getRandomWaterTile.c`/`_getRandomUnderwaterTile.c` - three
    /// otherwise byte-for-byte identical `.asm` bodies apart from the callee): builds the same
    /// zero-initialized 12-byte stack scratch `std::vector<BFTile*>` real vanilla's own frame is, calls
    /// the matching [`Self::get_tiles_aggregating`] getter into it, and computes `(end - begin) >> 2`.
    /// Only a strictly positive count reaches the draw - the `.asm`'s signed `TEST %ESI,%ESI`/`JLE`
    /// gate sits before any RNG touch, so an empty pool returns null and leaves `DAT_00638060` alone -
    /// and the picked tile (`(seed >> 0x10 & 0x7fff) % count` after exactly one MSVC LCG step,
    /// [`lcg_next`], new value stored and used) is read out of the buffer **before** the teardown,
    /// since the freelist push overwrites the buffer's first word (`begin` itself). The scratch buffer
    /// is freed via [`free_event_vector_buffer`] by **byte capacity** - the `.asm`'s own
    /// `TEST %EAX,%EAX`/`JZ`-guarded manual freelist-bucket/`operator_delete` split (the `SAR 2`/
    /// `SHL 2` round-trip on the capacity is a no-op - a `BFTile*` vector's byte size is always a
    /// multiple of 4).
    ///
    /// No fallback draw on an empty pool, unlike [`Self::pick_random_candidate_tile`]'s tail - the
    /// `.asm`'s `JLE` target goes straight to the teardown-and-return-null path. Deterministic apart
    /// from the one LCG advance: no `characteristics_dirty` recalculate anywhere in these bodies.
    /// `generated.rs`'s `-> i32` return for all three entries is an ABI-identical wart - EAX carries a
    /// tile pointer (or null) - so the port keeps `u32` and the detour casts.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    fn get_random_tiles_aggregating(&self, get_tiles: impl Fn(&Self, u32)) -> u32 {
        let mut scratch_vector = [0u32; 3];
        get_tiles(self, scratch_vector.as_mut_ptr() as u32);
        let count = (scratch_vector[1] as i32).wrapping_sub(scratch_vector[0] as i32) >> 2;
        let picked = if count > 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            let index = ((rng >> 0x10) & 0x7fff) % count as u32;
            get_from_memory(scratch_vector[0] + index * 4) // pick BEFORE teardown: the freelist push overwrites begin
        } else {
            0
        };
        free_event_vector_buffer(scratch_vector[0], scratch_vector[2].wrapping_sub(scratch_vector[0]));
        picked
    }

    /// Ports `ZTHabitat::getRandomLandTile` (`ZTHabitat_getRandomLandTile.c`/`.asm`, `generated.rs`'s
    /// `GET_RANDOM_LAND_TILE`): the random-draw wrapper over [`Self::get_land_tiles`] - see
    /// [`Self::get_random_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_land_tile(&self) -> u32 {
        self.get_random_tiles_aggregating(Self::get_land_tiles)
    }

    /// Ports `ZTHabitat::getRandomWaterTile` (`ZTHabitat_getRandomWaterTile.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_WATER_TILE`): the random-draw wrapper over
    /// [`Self::get_water_tiles`] - see [`Self::get_random_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_water_tile(&self) -> u32 {
        self.get_random_tiles_aggregating(Self::get_water_tiles)
    }

    /// Ports `ZTHabitat::getRandomUnderwaterTile` (`ZTHabitat_getRandomUnderwaterTile.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_UNDERWATER_TILE`): the random-draw wrapper over
    /// [`Self::get_underwater_tiles`] - see [`Self::get_random_tiles_aggregating`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_random_underwater_tile(&self) -> u32 {
        self.get_random_tiles_aggregating(Self::get_underwater_tiles)
    }

    /// Ports `ZTHabitat::addBabyBornBonus` (`ZTHabitat_addBabyBornBonus.c`/`.asm`, `generated.rs`'s
    /// `ADD_BABY_BORN_BONUS`): `ZTAnimal::doReproduceCheck`'s birth celebration. Builds the same
    /// zero-initialized scratch `std::vector<ZTAnimal*>` real vanilla's own stack-local is (the `.asm`'s
    /// three zero stores) and fills it via [`Self::get_species_animals`] with the species id read from
    /// `species_type_ptr+0x1ec` (the same read [`Self::get_species_animals`] filters on), then adds the
    /// species' birth bonus (`species_type_ptr+0x31c`, `bfentitytype.rs`'s `ZTAnimalType::baby_born_change`)
    /// into each matching animal's pending happiness-change accumulator at `animal+0x2ac` - the queue
    /// `ZTAnimal::updateStatusVariables` drains into the happiness value at `+0x2a8` each tick and zeroes
    /// (`getAmbientKey`'s `.asm` pins happiness itself at `+0x2a8`, directly before the accumulator; the
    /// same queue `ZTGoalEatingStanding::init` feeds when a guest consumes an item). The macOS decompile's
    /// `ZTAnimal::addHappinessChange` call is the same plain unclamped `+=`, inlined by MSVC here (its
    /// `return 1` survives as the `.asm` listing's dead `MOV AL, 0x1`). No null checks on either pointer,
    /// matching real vanilla.
    ///
    /// Frees the scratch vector's buffer via [`free_event_vector_buffer`] by **capacity** - the
    /// decompile's own tail (the same manual freelist-bucket/`operator_delete` split, not a
    /// `PoolAlloc::deallocate` call).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_baby_born_bonus(&self, species_type_ptr: u32) {
        let species_id: i32 = get_from_memory(species_type_ptr + 0x1ec);
        let bonus: i32 = get_from_memory(species_type_ptr + 0x31c);
        let mut scratch_vector = [0u32; 3];
        self.get_species_animals(species_id, scratch_vector.as_mut_ptr() as u32);
        for addr in (scratch_vector[0]..scratch_vector[1]).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let pending: i32 = get_from_memory(animal_ptr + 0x2ac);
            save_to_memory(animal_ptr + 0x2ac, pending.wrapping_add(bonus));
        }
        free_event_vector_buffer(scratch_vector[0], scratch_vector[2] - scratch_vector[0]);
    }

    /// Ports `ZTHabitat::getAllAnimals` (`ZTHabitat_getAllAnimals.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], then, when `sort`
    /// is set, sorts the real vanilla `std::vector<ZTAnimal*>` at `all_animals_begin`/`_end`
    /// (`.asm`-confirmed `LEA EAX,[ESI+0x6c]`) in place before yielding it - real vanilla always returns
    /// the same vector pointer regardless of `sort`, only conditionally reordering its contents first.
    ///
    /// Real vanilla's own sort is an inlined MSVC introsort over the comparator at `0x004690cd`
    /// (`bool __cdecl(BFEntity*, BFEntity*)`, unnamed in the Windows binary): `std::string operator<`
    /// on the two entities' `name` strings (`+0x108` begin / `+0x10c` end) - an unsigned byte-wise
    /// compare over the shorter length, then shorter-first. That is exactly Rust's `[u8]` ordering, so
    /// the port sorts by [`entity_name_bytes`]. This is a pure in-place reorder of an already-allocated
    /// real vanilla array - no allocation on either side - so there is no cross-allocator risk (per
    /// `CLAUDE.md`'s own `PoolAlloc` caveat) in using Rust's own sort algorithm instead of vanilla's. The
    /// two agree on the ordering of distinctly-named animals; animals sharing a name may end up in a
    /// different relative order (Rust's sort is stable, vanilla's introsort is not).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::species_list`].
    pub fn get_all_animals(&self, sort: bool) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let begin = self.all_animals_begin;
        let end = self.all_animals_end;
        if sort && end > begin {
            let mut animals: Vec<u32> = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
            animals.sort_by_cached_key(|&animal| entity_name_bytes(animal));
            for (i, &animal_ptr) in animals.iter().enumerate() {
                save_to_memory(begin + (i as u32) * 4, animal_ptr);
            }
        }
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getAnimals` (`ZTHabitat_getAnimals.c`, `generated.rs`'s `GET_ANIMALS`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_all_animals`], but returns the
    /// raw address of the vector header itself (`&this->field_0x6c`, i.e. `&self.all_animals_begin`)
    /// rather than yielding individual animal pointers - real vanilla's own callers treat the return value
    /// as a `std::vector<ZTAnimal*>&`. Distinct from [`Self::get_all_animals`], which is the more useful
    /// entry point for anything actually iterating the animals (and additionally supports sorting); this
    /// exists for parity with real vanilla's own signature/callers (e.g. `ZTHabitat::blockService`'s own
    /// `cls_0x498958::meth_0x498958` callee, per `zthabitatmgr-implementation-plan.md`'s step 6l notes).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_animals(&self) -> u32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self as *const Self as u32 + 0x6c
    }

    /// Ports `ZTHabitat::getAmountKeeperFood` (`ZTHabitat_getAmountKeeperFood.c`, `generated.rs`'s
    /// `GET_AMOUNT_KEEPER_FOOD`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then reads this habitat's own cached per-category tally
    /// ([`Self::keeper_food_category_amounts`]`[category]`). When `include_neighbors` is set, additionally
    /// sums every amphibious neighbor's own cached tally for the same category ([`walk_neighbor_tree`] over
    /// `amphibious_neighbors_head`) - real vanilla's own recursive call always passes `include_neighbors =
    /// false` for each neighbor, so despite being coded as recursion in the decompile this is only ever one
    /// level deep, matching [`Self::get_num_animals`]'s own established shape for the identical pattern.
    ///
    /// `category` is an opaque index into the 16-entry array, not otherwise validated here - matching real
    /// vanilla, which performs no bounds check either.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live (true for every `walk_neighbor_tree` entry).
    pub fn get_amount_keeper_food(&self, category: u32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let entry_addr = (self as *const Self as u32)
            .wrapping_add(offset_of!(Self, keeper_food_category_amounts) as u32)
            .wrapping_add(category.wrapping_mul(4));
        let mut total = get_from_memory::<i32>(entry_addr);
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total = total.wrapping_add(unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_amount_keeper_food(category, false));
            }
        }
        total
    }

    /// Ports `ZTHabitat::getFoodToLeave` (`ZTHabitat_getFoodToLeave.c`, `generated.rs`'s
    /// `GET_FOOD_TO_LEAVE`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`]. Sums [`crate::bfentitytype::ZTAnimalType::needed_food`] (`+0x220`, real
    /// vanilla's own `piVar6[0x88]`) over every direct occupant animal ([`entity_type_matches`]-gated,
    /// [`RVA_ANIMAL_TYPE_CHECK`]) whose own [`crate::bfentitytype::ZTAnimalType::keeper_food_type`]
    /// (`+0x3c8`, `piVar6[0xf2]`) equals `category`. When `include_neighbors` is set, additionally sums the
    /// same total over every amphibious neighbor (recursing with `include_neighbors = false`, same
    /// single-level-only shape as [`Self::get_amount_keeper_food`]) and subtracts
    /// [`Self::get_amount_keeper_food`]`(category, include_neighbors)` (i.e. food already left out, over
    /// the same self+neighbors scope) - matching real vanilla's own final `iVar5 - iVar3` exactly. Clamps
    /// the result to `0` (never negative), matching real vanilla's own trailing `-1 < iVar5` check.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_food_to_leave(&self, category: i32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 || !unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let keeper_food_type: i32 = get_from_memory(entity_type_ptr + 0x3c8);
            if keeper_food_type == category {
                total += get_from_memory::<i32>(entity_type_ptr + 0x220);
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_food_to_leave(category, false);
            }
            total -= self.get_amount_keeper_food(category as u32, include_neighbors);
        }
        total.max(0)
    }

    /// Ports `ZTHabitat::getNumKeeperFoodTiles` (`ZTHabitat_getNumKeeperFoodTiles.c`/`.asm`,
    /// `generated.rs`'s `GET_NUM_KEEPER_FOOD_TILES`): counts this habitat's own owned tiles whose occupant
    /// is a `ZTFood` entity of keeper-food category `category`. Walks the owned-tile list
    /// ([`walk_tile_list`] over [`Self::owned_tiles_ptr`]), reads each tile's occupant
    /// ([`crate::ztmapview::BFTile::entity_ptr`]), gates on [`entity_type_matches`] with
    /// [`RVA_ZTFOOD_TYPE_CHECK_ARG`], and compares the surviving entity's type's `+0x168`
    /// keeper-food-category word against `category` (the macOS build reads the equivalent through
    /// `ZTFood::getKeeperFoodType` at `type+0x14c` - platforms pack the type descriptor differently; the
    /// Windows `.asm` is authoritative). Unlike [`Self::get_amount_keeper_food`] this is deterministic and
    /// read-only: no `characteristics_dirty` recalc, no RNG, no amphibious-neighbor recursion.
    ///
    /// The Windows `.c` renders a second `(...+0x1c)(&DAT_006386c0, entity)` vtable call whose false path
    /// reads absolute address `0x168` (a crash). That second call is real but provably redundant - a
    /// back-to-back re-check of the same predicate on the same type pointer (the inlined
    /// `getKeeperFoodType`'s own type-cast), whose false/null paths are crash stubs, not reachable
    /// behavior; the stack arithmetic that made Ghidra read it as a second stack argument is the entity
    /// spill slot, and `ZTHabitat_getRandomKeeperFood.c` renders the identical pattern with one arg. The
    /// port performs the gate once ([`keeper_food_category_matches`]);
    /// [`entity_type_matches`]'s internal null-type guard (returns false) deviates from vanilla's
    /// unchecked crash-on-null deref, same established deviation as
    /// [`Self::send_maint_worker_cleanup_events`].
    ///
    /// Sole caller: `ZTHabitat::getRandomKeeperFood` (undetoured), which vanilla routes through the
    /// detour once installed.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_keeper_food_tiles(&self, category: u32) -> i32 {
        let mut count: i32 = 0;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            if keeper_food_category_matches(tile_ptr, category) {
                count += 1;
            }
        }
        count
    }

    /// Ports `ZTHabitat::removeFoodTarget` (`ZTHabitat_removeFoodTarget.c`/`.asm`, `generated.rs`'s
    /// `REMOVE_FOOD_TARGET` at `0x0059aa67`) - a free function despite the `ZTHabitat::` namespace: `this`
    /// is never read (the `.asm`'s own `RET 0x4`, one stack arg, no `ECX` use), the same unused-`this`
    /// shape as [`Self::get_adjacent_clear_tile`]. Clears `animal_ptr`'s current food target
    /// ([`animal_food_target`]) if it has one, delegating the mutation itself to real vanilla
    /// `ZTAnimal::setFood(animal, null)`/`ZTAnimal::stopEating(animal, false)` (`SET_FOOD`/`STOP_EATING`,
    /// both un-detoured - `ZTAnimal` itself isn't ported) via the call-the-port convention. A null
    /// `animal_ptr` is a legal real-vanilla no-op (its own leading guard). Real vanilla always returns
    /// `true`.
    pub fn remove_food_target(animal_ptr: u32) -> bool {
        if animal_ptr != 0 && animal_food_target(animal_ptr) != 0 {
            unsafe {
                SET_FOOD.original()(animal_ptr as *const u32, std::ptr::null());
                STOP_EATING.original()(animal_ptr as *const u32, 0);
            }
        }
        true
    }

    /// Ports `ZTHabitat::removeFoodTargetForAll` (`ZTHabitat_removeFoodTargetForAll.c`/`.asm`,
    /// `generated.rs`'s `REMOVE_FOOD_TARGET_FOR_ALL` at `0x0059a9d9`) - calls [`Self::remove_food_target`]
    /// on every animal in the habitat ([`Self::get_all_animals`]`(false)`) whose current food target
    /// ([`animal_food_target`]) is `food_entity_ptr`, then tears the entity down through its own vtable:
    /// `setVisible(false)` (slot `+0xcc`, [`call_vtable_slot_with_u8`]) then `setIsRemoved(true, false)`
    /// (slot `+0xa8`, [`call_vtable_slot_with_u8_u8`]) - both confirmed real `BFEntity` base slots. Real
    /// vanilla dereferences `food_entity_ptr`'s vtable unconditionally with no null guard for this final
    /// step (a real crash in vanilla on a null entity) - guarded here instead, the same "dead in practice"
    /// deviation this file already applies elsewhere (e.g. [`Self::get_outermost_tank`]'s own doc comment).
    /// Real vanilla ANDs each `removeFoodTarget` call's own low byte into a running result, but every path
    /// through [`Self::remove_food_target`] returns `true`, so the accumulated result is always `true` -
    /// reproduced here as a plain `true` return.
    pub fn remove_food_target_for_all(&self, food_entity_ptr: u32) -> bool {
        for animal_ptr in self.get_all_animals(false) {
            if animal_food_target(animal_ptr) == food_entity_ptr {
                Self::remove_food_target(animal_ptr);
            }
        }
        if food_entity_ptr != 0 {
            unsafe {
                call_vtable_slot_with_u8(food_entity_ptr, 0xcc, 0);
                call_vtable_slot_with_u8_u8(food_entity_ptr, 0xa8, 1, 0);
            }
        }
        true
    }

    /// Ports `ZTHabitat::hasPortalAnimal` (`ZTHabitat_hasPortalAnimal.c`/`.asm`, `generated.rs`'s
    /// `HAS_PORTAL_ANIMAL` at `0x0059e9a3`): whether any direct-occupant animal is in a show-portal
    /// transit state *and heading for* `target_habitat_ptr`. Reads the `all_animals` vector fields
    /// directly (`all_animals_begin`/`all_animals_end`) - no `characteristics_dirty`
    /// lazy-recalculate and no [`Self::get_all_animals`] sort, matching real vanilla's own straight
    /// field reads (`MOV ESI,[EBX+0x6c]` / `MOV EAX,[EBX+0x70]`).
    ///
    /// Per animal, the current goal/action id at `animal+0x170` must be exactly `0x85` or `0x86` (a
    /// full 32-bit compare in the `.asm`, not a byte test). The macOS decompile reaches the two
    /// checks through named calls - `ZTAnimal::hasShowGoal` for this state check and
    /// `ZTUnit::getTargetHabitat` for the target resolution - both inlined on Windows. The target
    /// resolution reads the animal's destination tile pointer at `animal+0x234` and resolves the
    /// habitat owning that tile: a null tile pointer yields candidate habitat `0`, otherwise the
    /// tile's own grid coordinates (`BFTile` `pos` at `+0x34`/`+0x38`, the same fields every other
    /// tile walker reads) index the habitat grid exactly as [`ZTHabitatMgr::get_habitat_ptr`] does
    /// (`*(*(mgr+0x28 + x*0xc) + y*0x28)` at `.asm` level). One deviation: real vanilla derefs both
    /// grid levels unconditionally (raw UB on an out-of-range coordinate - `cmp` against the target
    /// is the very next instruction after the read), while [`ZTHabitatMgr::get_habitat_ptr`] is the
    /// established bounds-checked grid read - identical for every in-range coordinate, the only
    /// state a real animal's destination tile produces. The first animal whose resolved habitat
    /// equals `target_habitat_ptr` wins; a null `target_habitat_ptr` therefore matches a portal
    /// animal with no destination tile (vanilla's own `0 == 0` arm), reproduced faithfully.
    ///
    /// Real vanilla's return is a low-byte-only bool (`MOV AL,1` / `XOR AL,AL`; the C render's
    /// `CONCAT31`/`& 0xffffff00` shapes pack garbage upper bytes around it) - the sole caller,
    /// `ZTHabitat::updatePortals` (Win call sites `0x0059e9f9`/`0x0059ea09`; macOS
    /// `ZTHabitat_updatePortals.c`), consumes it with `TEST AL,AL` while deciding which direction of
    /// each show-portal pair to animate. The port returns a clean `bool`.
    pub fn has_portal_animal(&self, target_habitat_ptr: u32) -> bool {
        for animal_addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(animal_addr);
            let state: u32 = get_from_memory(animal_ptr + 0x170);
            if state != 0x85 && state != 0x86 {
                continue;
            }
            let target_tile_ptr: u32 = get_from_memory(animal_ptr + 0x234);
            let resolved_habitat = if target_tile_ptr == 0 {
                0
            } else {
                let x = get_from_memory::<i32>(target_tile_ptr + 0x34);
                let y = get_from_memory::<i32>(target_tile_ptr + 0x38);
                globals().zthabitatmgr().get_habitat_ptr(x, y)
            };
            if resolved_habitat == target_habitat_ptr {
                return true;
            }
        }
        false
    }

    /// Ports `ZTHabitat::getSmallestKeeperFood` (`ZTHabitat_getSmallestKeeperFood.c`/`.asm`,
    /// `generated.rs`'s `GET_SMALLEST_KEEPER_FOOD`): returns the food entity of keeper-food category
    /// `category` carrying the smallest `entity+0x154` quantity (macOS reads the equivalent through its
    /// own `ZTFood+0x120` layout) among this habitat's own owned tiles, ties going to the first in
    /// owned-tile-list order (strict `<` against the `0x7fffffff` sentinel). The own hit is returned
    /// immediately - the amphibious neighbors are consulted only when the own pool is empty and
    /// `include_neighbors` is set, each neighbor ([`walk_neighbor_tree`] order) resolving its own
    /// internal minimum through the same function with `include_neighbors = false`, the parent then
    /// comparing the *returned entity's* amount against its running best (strict `<`), not re-walking
    /// the neighbor's tiles itself.
    ///
    /// The first parameter is a **`BFTile*`** reference tile (macOS prototype
    /// `(BFTile *, ZTFoodType::EKeeperFoodType, bool)`), which this function never reads - it only
    /// threads it down the subhab recursion.
    ///
    /// The Windows `.c` render is a degraded cold-split fragment: the own-walk body ends in
    /// `JMP FUN_005c6f30` (the optimizer-split subhab tail) whose argument list is spill-slot garbage;
    /// the real body and the `RET 0xc` epilogue are in the same `.asm` listing, and the subhab tail's
    /// semantics come from the clean macOS decompile. The `GLOBAL_ZTWorldMgr + 8 == 0` "world not
    /// initialized" guard (the same `0xfffffff8` sentinel as [`Self::get_nearest_clear_water_tile`]'s)
    /// returns null; the port uses the shared `== 0` convention.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    #[allow(clippy::only_used_in_recursion)] // the reference tile is threaded down the subhab recursion unread, vanilla's own shape
    pub fn get_smallest_keeper_food(&self, tile_ptr: u32, category: u32, include_neighbors: bool) -> u32 {
        if globals().ztworldmgr_ptr() as u32 == 0 {
            return 0;
        }
        let mut best_entity: u32 = 0;
        let mut best_amount = 0x7fff_ffffi32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let candidate_tile = get_from_memory::<TileListNode>(node).payload;
            if !keeper_food_category_matches(candidate_tile, category) {
                continue;
            }
            let entity_ptr: u32 = get_from_memory(candidate_tile + 0x10);
            let amount: i32 = get_from_memory(entity_ptr + 0x154);
            if amount < best_amount {
                best_amount = amount;
                best_entity = entity_ptr;
            }
        }
        if best_entity != 0 {
            return best_entity;
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let candidate =
                    unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_smallest_keeper_food(tile_ptr, category, false);
                if candidate != 0 {
                    let amount: i32 = get_from_memory(candidate + 0x154);
                    if amount < best_amount {
                        best_amount = amount;
                        best_entity = candidate;
                    }
                }
            }
        }
        best_entity
    }

    /// Ports `ZTHabitat::getNearestKeeperFood` (`ZTHabitat_getNearestKeeperFood.c`/`.asm`,
    /// `generated.rs`'s `GET_NEAREST_KEEPER_FOOD`): returns the own food entity of keeper-food category
    /// `category` whose host tile is spatially nearest to the **`BFTile*`** reference tile `tile_ptr`
    /// (macOS prototype `(BFTile *, ZTFoodType::EKeeperFoodType, bool)`; the distance is
    /// [`keeper_food_distance_squared`] over raw `+0x34`/`+0x38` reads), ties going to the first in
    /// owned-tile-list order (strict `<`). A null reference tile leaves every candidate at the
    /// `0x7fffffff` sentinel, so nothing wins and the result is null. Same early-return-own-hit /
    /// consult-neighbors-only-when-own-empty shape as [`Self::get_smallest_keeper_food`], except each
    /// neighbor's returned candidate is re-distanced through its *actual* tile - real vanilla calls
    /// `BFEntity::getTile` on the result (which may have moved off the tile its amount was read from)
    /// before the parent compare.
    ///
    /// The Windows `.c` is clean and authoritative. Guard/return-type notes as
    /// [`Self::get_smallest_keeper_food`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_nearest_keeper_food(&self, tile_ptr: u32, category: u32, include_neighbors: bool) -> u32 {
        if globals().ztworldmgr_ptr() as u32 == 0 {
            return 0;
        }
        let mut best_entity: u32 = 0;
        let mut best_dist = 0x7fff_ffffi32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let candidate_tile = get_from_memory::<TileListNode>(node).payload;
            if !keeper_food_category_matches(candidate_tile, category) {
                continue;
            }
            let entity_ptr: u32 = get_from_memory(candidate_tile + 0x10);
            let dist = keeper_food_distance_squared(tile_ptr, candidate_tile);
            if dist < best_dist {
                best_dist = dist;
                best_entity = entity_ptr;
            }
        }
        if best_entity != 0 {
            return best_entity;
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let candidate =
                    unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_nearest_keeper_food(tile_ptr, category, false);
                if candidate != 0 {
                    let candidate_tile = unsafe { BFENTITY_GET_TILE.original()(candidate as *const u32) } as u32;
                    let dist = keeper_food_distance_squared(tile_ptr, candidate_tile);
                    if dist < best_dist {
                        best_dist = dist;
                        best_entity = candidate;
                    }
                }
            }
        }
        best_entity
    }

    /// Ports `ZTHabitat::getRandomKeeperFood` (`ZTHabitat_getRandomKeeperFood.c`/`.asm`,
    /// `generated.rs`'s `GET_RANDOM_KEEPER_FOOD`): uniform random pick over the own keeper-food pool of
    /// `category` - the count comes from [`Self::get_num_keeper_food_tiles`] (call-the-port convention:
    /// real vanilla's own `CALL getNumKeeperFoodTiles` hits that entry's detour address too, re-entering
    /// the port under the live battery), and a non-empty pool advances the shared game RNG
    /// ([`lcg_next`] over `DAT_00638060`, same one-step shape as
    /// [`Self::get_random_clear_tile_for_animal`]) exactly once and returns the
    /// `((rng >> 0x10) & 0x7fff) % count`-th matching entity in owned-tile-list order. An empty pool
    /// returns the first non-null neighbor hit (each neighbor resolving through the same function with
    /// `include_neighbors = false`) when `include_neighbors` is set - the RNG is untouched on that
    /// path - and null otherwise. The count/pick gate shares [`keeper_food_category_matches`] with the
    /// count, so both walks agree by construction. Unlike the other two food getters there is no
    /// `GLOBAL_ZTWorldMgr` guard (the `.asm` goes straight to the count call), and the walk-exhausted
    /// `return 0` tail is dead (the pick target is always inside the walked count) - kept for shape.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    #[allow(clippy::only_used_in_recursion)] // the reference tile is threaded down the subhab recursion unread, vanilla's own shape
    pub fn get_random_keeper_food(&self, tile_ptr: u32, category: u32, include_neighbors: bool) -> u32 {
        let count = self.get_num_keeper_food_tiles(category);
        if count < 1 {
            if include_neighbors {
                for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                    let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                    let hit =
                        unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_random_keeper_food(tile_ptr, category, false);
                    if hit != 0 {
                        return hit;
                    }
                }
            }
            return 0;
        }
        let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
        let rng = lcg_next(get_from_memory::<u32>(rng_addr));
        save_to_memory(rng_addr, rng);
        let target = ((rng >> 0x10) & 0x7fff) % count as u32;
        let mut counter: u32 = 0;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let candidate_tile = get_from_memory::<TileListNode>(node).payload;
            if !keeper_food_category_matches(candidate_tile, category) {
                continue;
            }
            if counter == target {
                return get_from_memory(candidate_tile + 0x10);
            }
            counter += 1;
        }
        0
    }

    /// Ports `ZTHabitat::getNumKeepers` (`ZTHabitat_getNumKeepers.c`, `generated.rs`'s `GET_NUM_KEEPERS`):
    /// same `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_all_animals`], then returns
    /// the cached count ([`Self::num_keepers`]) recalculateCharacteristics's own owned-tile census tallies.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_num_keepers(&self) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self.num_keepers
    }

    /// Ports `ZTHabitat::isBeingServiced` (`ZTHabitat_isBeingServiced.c`, `generated.rs`'s
    /// `IS_BEING_SERVICED`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then returns the cached flag ([`Self::is_being_serviced_raw`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn is_being_serviced(&self) -> bool {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self.is_being_serviced_raw != 0
    }

    /// Ports `ZTHabitat::sendMaintWorkerCleanupEvents` (`ZTHabitat_sendMaintWorkerCleanupEvents.c`,
    /// `generated.rs`'s `SEND_MAINT_WORKER_CLEANUP_EVENTS`): a no-op on a tank habitat ([`Self::is_tank`]);
    /// otherwise walks every owned tile ([`walk_tile_list`] over [`Self::owned_tiles_ptr`]) and, for each
    /// tile with a real occupant ([`crate::ztmapview::BFTile::entity_ptr`]) whose own entity type
    /// ([`entity_type_matches`]-gated, [`RVA_SCENERY_TYPE_CHECK_ARG`]) has either of two unconfirmed
    /// scenery-type flag bytes set (`entity_type+0x11c != 0` or `entity_type+0x12c != 0` - real vanilla's
    /// own `piVar1[0x47]`/`(char)piVar1[0x4b]`, no corroborating name found elsewhere in the corpus this
    /// pass), calls this habitat's own `sendEvent` (vtable `+0x0`) - real vanilla always calls this address
    /// directly rather than through the vtable pointer, since [`SEND_EVENT`] is inherited unchanged by
    /// every known subclass including `ZTTankExhibit` (confirmed via `private/docs/vtables/ZTTankExhibit.md`),
    /// so a raw call-through is exactly equivalent to a real virtual dispatch here.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address is passed directly into
    /// [`SEND_EVENT`].
    ///
    /// **`SEND_EVENT`'s real args, per `ZTHabitat_sendMaintWorkerCleanupEvents.asm`'s own call site**: six
    /// stack args (`RET 0x18`), not the zero `generated.rs` previously carried - `(event_id: u16 = 0x2730,
    /// _unused: u32 = 0, category: u8 = 0x4d, tile_ptr: u32, _unused: u16 = 0, 1u16)`. `0x4d` is `'M'`
    /// (maintenance) - `ztshowinfo.rs`'s own analogous `sendEvent` forward uses `0x53` (`'S'`, show) in the
    /// same slot, confirming this is a real per-source event-category tag rather than a coincidence. Calling
    /// through with the old zero-arg signature read six garbage stack dwords as these args and then popped
    /// 0x18 bytes of the *caller's* stack on return that were never pushed - genuine stack corruption on
    /// every call, live-confirmed (via `cdb`, an actually-hung real `zoo.exe` process) to hang the whole game
    /// on exit once a loaded save's habitats reached this cleanup path, not just the stripped
    /// reimplementation-tests harness this was previously suspected to be confined to.
    pub fn send_maint_worker_cleanup_events(&self) {
        let self_addr = self as *const Self as u32;
        for tile_ptr in self.maint_worker_cleanup_tiles() {
            unsafe { SEND_EVENT.original()(self_addr as *const u32, 0x2730, 0, 0x4d, tile_ptr, 0, 1) };
        }
    }

    /// The tiles [`Self::send_maint_worker_cleanup_events`] sends one event for, in walk order: empty on a
    /// tank habitat, otherwise every owned tile whose occupant is scenery with either type flag set. Lazy
    /// so each send still happens between tile reads, as in real vanilla. Exposed so the live comparison
    /// test can diff the plan against the sends it records from real vanilla.
    pub fn maint_worker_cleanup_tiles(&self) -> impl Iterator<Item = u32> {
        let tiles = if self.is_tank() { 0 } else { self.owned_tiles_ptr };
        let walk: Box<dyn Iterator<Item = u32>> = if tiles == 0 { Box::new(std::iter::empty()) } else { Box::new(walk_tile_list(tiles)) };
        walk.filter_map(|node| {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
            if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                return None;
            }
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
            let flag_a: u32 = get_from_memory(entity_type_ptr + 0x11c);
            let flag_b: u8 = get_from_memory(entity_type_ptr + 0x12c);
            (flag_a != 0 || flag_b != 0).then_some(tile_ptr)
        })
    }

    /// Ports `ZTHabitat::getNumHungryFoodlessAnimals` (`ZTHabitat_getNumHungryFoodlessAnimals.c`,
    /// `generated.rs`'s `GET_NUM_HUNGRY_FOODLESS_ANIMALS`): same `characteristics_dirty`-gated
    /// lazy-recalculate shape as [`Self::get_all_animals`], then counts direct-occupant animals for which
    /// real vanilla `ZTAnimal::isHungryAndFoodless` (masked via [`low_byte_bool`] - its own decompile shows
    /// the standard `CONCAT31` undefined-upper-bytes-bool shape `CLAUDE.md` documents) returns true. When
    /// `include_neighbors` is set, additionally sums every amphibious neighbor's own count (recursing with
    /// `false`, same single-level-only shape as [`Self::get_num_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_num_hungry_foodless_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if low_byte_bool(unsafe { IS_HUNGRY_AND_FOODLESS.original()(animal_ptr as *const u32) }) {
                total += 1;
            }
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_hungry_foodless_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumSicklyAnimals` (`ZTHabitat_getNumSicklyAnimals.c`, `generated.rs`'s
    /// `GET_NUM_SICKLY_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then counts direct-occupant animals for which all of the following hold:
    /// real vanilla `ZTAnimal::canService(animal, keeper_ptr)` (masked via [`low_byte_bool`]);
    /// [`Self::keeper_assigned_to_animal`] (`keeper_ptr`'s own assigned-species id list contains the
    /// animal's id); the animal's own tile doesn't have an unconfirmed flag set (`tile+0x85 & 4`, real
    /// vanilla's own `(*(byte*)(tile+0x85) & 4) == 0` check - no corroborating name found elsewhere in the
    /// corpus this pass, plausibly "escaped"/"not visible" given its use alongside a sickness check); real
    /// vanilla `ZTAnimal::isSickly` (masked via [`low_byte_bool`]); and a second, distinct unconfirmed tile
    /// flag check (`tile+0x83 & 3`, real vanilla's own `(*(byte*)(tile+0x83) & 3) == 0`). When
    /// `include_neighbors` is set, additionally sums every amphibious neighbor's own count (recursing with
    /// `false`, same single-level-only shape as [`Self::get_num_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`];
    /// each neighbor visited must also be live.
    pub fn get_num_sickly_animals(&self, keeper_ptr: u32, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = 0i32;
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 {
                continue;
            }
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                continue;
            }
            if !Self::keeper_assigned_to_animal(keeper_ptr, animal_ptr) {
                continue;
            }
            let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) } as u32;
            if tile_ptr == 0 || get_from_memory::<u8>(tile_ptr + 0x85) & 4 != 0 {
                continue;
            }
            if !low_byte_bool(unsafe { IS_SICKLY.original()(animal_ptr as *const u32) }) {
                continue;
            }
            if get_from_memory::<u8>(tile_ptr + 0x83) & 3 != 0 {
                continue;
            }
            total += 1;
        }
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_sickly_animals(keeper_ptr, false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumAngryAnimals` (`ZTHabitat_getNumAngryAnimals.c`/`.asm`, `generated.rs`'s
    /// `GET_NUM_ANGRY_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_attractiveness`] - the lazy recalculate is also this counter's own writer (its census
    /// zeroes `num_angry_animals` and re-increments it per angry animal) - then returns the cached count
    /// alone when `include_neighbors` is `false`. When `true`, additionally walks the amphibious-neighbor
    /// set ([`walk_neighbor_tree`] over `amphibious_neighbors_head`) summing each neighbor's own
    /// `get_num_angry_animals(false)` (never recursing sub-neighbors of sub-neighbors, matching real
    /// vanilla's own `getNumAngryAnimals(neighbor, false)` recursion exactly). Real vanilla's own
    /// subhabitat recursion is a direct self-call at this function's own (detoured) entry, so
    /// `.original()`'s inner neighbor counts re-enter this port - same shape as [`Self::get_num_animals`]
    /// and semantically identical, since both sides compute the same sum.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`]; each neighbor visited must also be live (true for every
    /// `walk_neighbor_tree` entry).
    pub fn get_num_angry_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = self.num_angry_animals;
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_angry_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getNumSickAnimals` (`ZTHabitat_getNumSickAnimals.c`/`.asm`, `generated.rs`'s
    /// `GET_NUM_SICK_ANIMALS`): instruction-for-instruction twin of [`Self::get_num_angry_animals`] over
    /// `num_sick_animals` (recalculateCharacteristics's census re-increments it per animal whose `+0x3a7`
    /// flag byte is set) - see that method's doc comment for the lazy-recalculate shape, the
    /// `include_neighbors` single-level-only recursion, and the live-reference precondition.
    pub fn get_num_sick_animals(&self, include_neighbors: bool) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut total = self.num_sick_animals;
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_num_sick_animals(false);
            }
        }
        total
    }

    /// Ports `ZTHabitat::getAvgAnimalHappiness` (`ZTHabitat_getAvgAnimalHappiness.c`/`.asm`,
    /// `generated.rs`'s `GET_AVG_ANIMAL_HAPPINESS`): same `characteristics_dirty`-gated
    /// lazy-recalculate shape as [`Self::get_attractiveness`] - the lazy recalculate is also this
    /// value's own writer (its census zero-inits `avg_animal_happiness` when there are no animals,
    /// else divides the summed `animal+0x18` happiness by the direct-occupant count) - then returns
    /// the cached average alone. Unlike the sibling count getters this takes no `include_neighbors`
    /// flag: real vanilla reads the single field and returns, no neighbor walk. Must only be called
    /// on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_avg_animal_happiness(&self) -> i32 {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        self.avg_animal_happiness
    }

    /// Ports `ZTHabitat::getSicklyAnimals` (`ZTHabitat_getSicklyAnimals.c`, `generated.rs`'s
    /// `GET_SICKLY_ANIMALS`): same `characteristics_dirty`-gated lazy-recalculate shape as
    /// [`Self::get_all_animals`], then appends every direct-occupant animal for which real vanilla
    /// `ZTAnimal::isSickly` (masked via [`low_byte_bool`]) returns true onto the real vanilla
    /// `std::vector<ZTAnimal*>` out-param at `out_vector_ptr` ([`vector_push_pool_alloc4`] - the same
    /// `PoolAlloc::allocate`-doubling-growth/manual-freelist-teardown shape this decompile's own tail uses).
    /// `out_vector_ptr` is never read as a pre-existing vector on entry beyond its current
    /// `begin`/`end`/`cap_end` state - matching real vanilla, which only ever appends.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_sickly_animals(&self, out_vector_ptr: u32) {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if low_byte_bool(unsafe { IS_SICKLY.original()(animal_ptr as *const u32) }) {
                vector_push_pool_alloc4(out_vector_ptr, animal_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::getNearestSickAnimal` (`ZTHabitat_getNearestSickAnimal.c`/`.asm`, `generated.rs`'s
    /// `GET_NEAREST_SICK_ANIMAL`): returns `0` immediately if this habitat [`Self::is_tank`] and has a
    /// `ZTShowInfo` attached (real vanilla's own `isTank(this) && this->zt_show_info_ptr != 0` check -
    /// `.asm`-confirmed `MOV EAX, [ESI+0x4]`, the decompile's `this->mbr_0x4` correctly names the raw byte
    /// offset per this repo's own `mbr_0xADDR` convention, not a misread of some other field - a show tank
    /// is simply never a valid maintenance-request target). Also bails if `keeper_ptr` (real vanilla's own
    /// `param_1`, actually a `ZTStaff*`) is null, if `GLOBAL_ZTAIMgr` is null, or if `keeper_ptr`'s own
    /// tile can't be resolved. Real vanilla's own separate `GLOBAL_ZTWorldMgr == 0xfffffff8` ("world not
    /// loaded") sentinel check is not reproduced as its own branch here - [`BFENTITY_GET_TILE`]'s real
    /// body already returns null in that exact state (confirmed via `BFEntity_getTile.c`'s own identical
    /// sentinel check), so the tile-resolution null-check below already covers it with the same observable
    /// result.
    ///
    /// Otherwise builds a scratch `std::vector<ZTAnimal*>` via [`Self::get_sickly_animals`] (real vanilla's
    /// own stack-local out-param, zero-initialized here identically), then scans it for the closest animal
    /// for which all of the following hold: the animal is not itself a `ZTKeeper` (vtable `+0x110`-style
    /// slot the decompile mislabels `virt_meth_0x401115_272` - the `_N` suffix is the decompiler's own
    /// resolved byte offset per `private/docs/vtables/README.md`, not independently identified further this
    /// pass, kept as a raw vtable dispatch); its own tile doesn't have the unconfirmed `+0x85 & 4` flag set
    /// (same check [`Self::get_num_sickly_animals`] uses); [`Self::keeper_assigned_to_animal`]; and real
    /// vanilla `ZTAnimal::canService(animal, keeper_ptr)` (masked via [`low_byte_bool`]). When
    /// `check_can_see` is set, additionally requires the shared `GLOBAL_ZTAIMgr` vtable `+0x1c`
    /// visibility/path-reachability dispatch ([`call_vtable_slot_ptr_ptr_ptr_u32_ret_bool`], `this=ai_mgr`,
    /// `keeper_tile_ptr`, `animal_tile_ptr`, `keeper_ptr`, `0` - a 4-stack-arg thiscall, confirmed via a
    /// manual `.asm` trace and identical to the call [`Self::get_nearest_dirt_pile`] makes to the same
    /// slot) to pass. Distance is squared Euclidean over each candidate's own tile `x`/`y` (`+0x34`/`+0x38`,
    /// [`crate::ztmapview::BFTile::pos`]) against `keeper_ptr`'s own tile.
    ///
    /// Frees the scratch vector's buffer via [`vector_push_pool_alloc4`]'s own counterpart,
    /// [`free_event_vector_buffer`], by **capacity** - matching real vanilla's own tail exactly (the same
    /// manual freelist-bucket/`operator_delete` split, not a `PoolAlloc::deallocate` call).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_nearest_sick_animal(&self, keeper_ptr: u32, check_can_see: bool) -> u32 {
        if self.is_tank() && self.zt_show_info_ptr != 0 {
            return 0;
        }
        let ai_mgr = globals().ztaimgr_ptr() as u32;
        if keeper_ptr == 0 || ai_mgr == 0 {
            return 0;
        }
        let keeper_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(keeper_ptr as *const u32) } as u32;
        if keeper_tile_ptr == 0 {
            return 0;
        }

        let mut scratch_vector = [0u32; 3];
        let scratch_ptr = scratch_vector.as_mut_ptr() as u32;
        self.get_sickly_animals(scratch_ptr);

        let mut best_animal = 0u32;
        let mut best_dist = i32::MAX;
        for addr in (scratch_vector[0]..scratch_vector[1]).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let is_keeper = unsafe { call_entity_vtable_noargs(animal_ptr, 0x110) };
            if is_keeper {
                continue;
            }
            let animal_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(animal_ptr as *const u32) } as u32;
            if animal_tile_ptr == 0 || get_from_memory::<u8>(animal_tile_ptr + 0x85) & 4 != 0 {
                continue;
            }
            if !Self::keeper_assigned_to_animal(keeper_ptr, animal_ptr) {
                continue;
            }
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                continue;
            }
            if check_can_see
                && !unsafe { call_vtable_slot_ptr_ptr_ptr_u32_ret_bool(ai_mgr, 0x1c, keeper_tile_ptr, animal_tile_ptr, keeper_ptr, 0) }
            {
                continue;
            }
            let dx: i32 = get_from_memory::<i32>(keeper_tile_ptr + 0x34) - get_from_memory::<i32>(animal_tile_ptr + 0x34);
            let dy: i32 = get_from_memory::<i32>(keeper_tile_ptr + 0x38) - get_from_memory::<i32>(animal_tile_ptr + 0x38);
            let dist = dx * dx + dy * dy;
            if best_animal == 0 || dist < best_dist {
                best_animal = animal_ptr;
                best_dist = dist;
            }
        }

        free_event_vector_buffer(scratch_vector[0], scratch_vector[2] - scratch_vector[0]);
        best_animal
    }

    /// Ports `ZTHabitat::getNearestDirtPile` (`ZTHabitat_getNearestDirtPile.c`/`.asm`, `generated.rs`'s
    /// `GET_NEAREST_DIRT_PILE`): returns `0` if `keeper_ptr` (real vanilla's own `param_1`, actually a
    /// `ZTStaff*`) is null, if `GLOBAL_ZTAIMgr` is null, or if `keeper_ptr`'s own tile can't be resolved.
    /// Real vanilla's own separate `GLOBAL_ZTWorldMgr == 0xfffffff8` sentinel check is not reproduced as
    /// its own branch here, same established simplification as [`Self::get_nearest_sick_animal`]'s own
    /// doc comment explains - [`BFENTITY_GET_TILE`]'s real body already returns null in that exact state.
    ///
    /// Otherwise walks every owned tile ([`walk_tile_list`] over [`Self::owned_tiles_ptr`]), reads each
    /// tile's occupant entity (raw `tile+0x10` - the `.asm`'s own odd "push `0x20,0x20,0`; `SAR 0x5` per
    /// axis" grid-index sequence constant-folds to this flat read, the same compiler artifact already
    /// documented for [`Self::get_num_keeper_food_tiles`]), and scans for the closest occupant for which
    /// all of the following hold: the tile is **not** present in [`Self::keeper_has_invalid_tile`]'s
    /// per-keeper exclusion list; `keeper_ptr`'s own vtable `+0x324` slot
    /// ([`call_vtable_slot_with_ptr_ret_bool`], `this=keeper_ptr`, the candidate entity) returns true -
    /// the real per-keeper entity-target filter (the master plan's own gloss claiming this is an
    /// `RVA_SCENERY_TYPE_CHECK_ARG`/`entity_type_matches` call does not hold up against the real
    /// decompile/disassembly - no such call appears anywhere in this function on either platform); the
    /// tile does not have the same unconfirmed `+0x85 & 4` flag [`Self::get_num_sickly_animals`] uses; and,
    /// when `check_can_see` is set, the shared `GLOBAL_ZTAIMgr` vtable `+0x1c` visibility/path-reachability
    /// dispatch ([`call_vtable_slot_ptr_ptr_ptr_u32_ret_bool`], identical shape and slot to
    /// [`Self::get_nearest_sick_animal`]'s own call) passes. Distance is squared Euclidean via
    /// [`keeper_food_distance_squared`] between `keeper_ptr`'s own tile and each candidate's tile, ties
    /// going to the first in walk order (strict `<`).
    ///
    /// Naming correction: the master plan's signature gloss `getNearestDirtPile(ZTStaff* keeper, bool
    /// subhabs)` is wrong for the second parameter - tracing the disassembly shows this bool gates only
    /// the `GLOBAL_ZTAIMgr` visibility check above; there is no habitat-neighbor recursion anywhere in this
    /// function (unlike [`Self::get_nearest_sick_animal`]/the keeper-food triplet). The real parameter
    /// plays the exact same role as `get_nearest_sick_animal`'s own `check_can_see` parameter - named
    /// accordingly here, not `subhabs`.
    ///
    /// Real vanilla also builds a permanently-empty local scratch `std::vector`-shaped object at function
    /// entry and conditionally tears it down via a `PoolAlloc`-bucket deallocate at the very end - nothing
    /// in the function body ever writes to it, so the teardown's guard (`pointer != 0`) is always false at
    /// runtime. Dead weight with zero observable effect; no Rust equivalent is needed for it.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn get_nearest_dirt_pile(&self, keeper_ptr: u32, check_can_see: bool) -> u32 {
        if keeper_ptr == 0 || globals().ztaimgr_ptr() as u32 == 0 {
            return 0;
        }
        let keeper_tile_ptr = unsafe { BFENTITY_GET_TILE.original()(keeper_ptr as *const u32) } as u32;
        if keeper_tile_ptr == 0 {
            return 0;
        }

        let mut best_entity = 0u32;
        let mut best_dist = i32::MAX;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
            if entity_ptr == 0 {
                continue;
            }
            if Self::keeper_has_invalid_tile(keeper_ptr, tile_ptr) {
                continue;
            }
            if !unsafe { call_vtable_slot_with_ptr_ret_bool(keeper_ptr, 0x324, entity_ptr) } {
                continue;
            }
            if get_from_memory::<u8>(tile_ptr + 0x85) & 4 != 0 {
                continue;
            }
            if check_can_see {
                let ai_mgr = globals().ztaimgr_ptr() as u32;
                let visible = unsafe {
                    call_vtable_slot_ptr_ptr_ptr_u32_ret_bool(ai_mgr, 0x1c, keeper_tile_ptr, tile_ptr, keeper_ptr, 0)
                };
                if !visible {
                    continue;
                }
            }
            let dist = keeper_food_distance_squared(keeper_tile_ptr, tile_ptr);
            if best_entity == 0 || dist < best_dist {
                best_entity = entity_ptr;
                best_dist = dist;
            }
        }
        best_entity
    }

    /// Ports `ZTHabitat::needsShowKeeper` (Win `0x0041680c`, `generated.rs`'s `NEEDS_SHOW_KEEPER`; macOS
    /// `ZTHabitat_needsShowKeeper.c`). `false` if `keeper_ptr` is null. Otherwise looks up the keeper's own
    /// catalog/type ID via its `BFEntityType`'s (`keeper_ptr + 0x128`) vtable `+0x20` slot
    /// ([`call_entity_vtable_u32_noargs`] - the same "isUserTypeID" slot [`Self::block_service`] already
    /// calls on a keeper's entity type) - real vanilla makes this call unconditionally whenever
    /// `keeper_ptr` is non-null, before checking `zt_show_info_ptr`, so this preserves that order. `false`
    /// again with no attached `ZTShowInfo`; otherwise delegates to the already-ported
    /// `ztshowinfo::needs_keeper`.
    pub fn needs_show_keeper(&self, keeper_ptr: u32) -> bool {
        if keeper_ptr == 0 {
            return false;
        }
        let entity_type_ptr: u32 = get_from_memory(keeper_ptr + 0x128);
        let keeper_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
        if self.zt_show_info_ptr == 0 {
            return false;
        }
        ztshowinfo::needs_keeper(self.zt_show_info_ptr, keeper_type_id)
    }

    /// Ports `ZTHabitat::getViewingAreasWithGuests` (`ZTHabitat_getViewingAreasWithGuests.c`,
    /// `generated.rs`'s `GET_VIEWING_AREAS_WITH_GUESTS`): for every non-null entry in
    /// [`Self::viewing_areas_begin`]/`_end`, recalculates it (real, un-ported vanilla
    /// `ZTViewingArea::recalculateCharacteristics`, called through - gated on the viewing area's own dirty
    /// flag at `+0x4c`, not otherwise modeled in this codebase) then, if its own guest count (`+0x54`) is
    /// positive, appends it onto the real vanilla `std::vector<ZTViewingArea*>` out-param at
    /// `out_vector_ptr` ([`vector_push_pool_alloc4`] - same growth shape as [`Self::get_sickly_animals`]).
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds the vector fields
    /// read directly off it.
    pub fn get_viewing_areas_with_guests(&self, out_vector_ptr: u32) {
        for cursor in (self.viewing_areas_begin..self.viewing_areas_end).step_by(4) {
            let va_ptr: u32 = get_from_memory(cursor);
            if va_ptr == 0 {
                continue;
            }
            if get_from_memory::<u32>(va_ptr + 0x4c) != 0 {
                unsafe { ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS.original()(va_ptr as *const u32) };
            }
            if get_from_memory::<i32>(va_ptr + 0x54) > 0 {
                vector_push_pool_alloc4(out_vector_ptr, va_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::hasBldg` (`ZTHabitat_hasBldg.c`, `generated.rs`'s `HAS_BLDG`): membership test
    /// over [`Self::building_list_begin`]/`_end` (a real vanilla `std::vector<BFEntity*>` this pass newly
    /// names - previously undifferentiated padding, see that field's own doc comment) for `entity_ptr`.
    pub fn has_bldg(&self, entity_ptr: u32) -> bool {
        (self.building_list_begin..self.building_list_end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == entity_ptr)
    }

    /// Ports `ZTHabitat::addToBuildingList` (`ZTHabitat_addToBuildingList.c`/`.asm`, `generated.rs`'s
    /// `ADD_TO_BUILDING_LIST`): [`Self::has_bldg`]'s own writer. Walks every owned tile's four
    /// direct-entity slots (`tile+0x4..+0x10` - see [`Self::add_clear_tiles`]'s own doc comment for this
    /// shape, confirmed `.asm`-side here too: a 2x2 nested loop over the same four dwords), and for each
    /// non-null occupant passing the `ZTBuilding` type-cast gate ([`entity_type_matches`],
    /// [`RVA_BUILDING_TYPE_CHECK_ARG`]) whose own type has either its `+0x23e` flag byte set or its
    /// `+0x170` dword positive, appends it onto `other_ptr`'s own building list (`other_ptr+0x78`/`+0x7c`,
    /// the same [`Self::has_bldg`]-style dedup scan, just parameterized over `other_ptr` instead of
    /// `self`; [`vector_push_pool_alloc4`] push onto `out_vector_ptr`) unless already present. Called once
    /// per amphibious neighbor from [`Self::recalculate_characteristics`]
    /// (`neighbor.addToBuildingList(&target.building_list_begin, target)` - `self` here is the *neighbor*
    /// being merged from, not the merge target `other_ptr`).
    pub fn add_to_building_list(&self, out_vector_ptr: u32, other_ptr: u32) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile: u32 = get_from_memory(node + 0x8);
            for slot in (0x4u32..=0x10).step_by(4) {
                let entity_ptr: u32 = get_from_memory(tile + slot);
                if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_BUILDING_TYPE_CHECK_ARG) } {
                    continue;
                }
                let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
                let flag: u8 = get_from_memory(entity_type_ptr + 0x23e);
                let count: i32 = get_from_memory(entity_type_ptr + 0x170);
                if flag == 0 && count <= 0 {
                    continue;
                }
                let already_present = (get_from_memory::<u32>(other_ptr + 0x78)..get_from_memory::<u32>(other_ptr + 0x7c))
                    .step_by(4)
                    .any(|addr| get_from_memory::<u32>(addr) == entity_ptr);
                if !already_present {
                    vector_push_pool_alloc4(out_vector_ptr, entity_ptr);
                }
            }
        }
    }

    /// Ports `ZTHabitat::additionalScenerySuitabilityChange`
    /// (`ZTHabitat_additionalScenerySuitabilityChange.c`/`.asm`, `generated.rs`'s
    /// `ADDITIONAL_SCENERY_SUITABILITY_CHANGE`): for every species type in the caller-built
    /// `species_vector_ptr` vector passing its own vtable `+0xcc` gate ([`call_entity_vtable_noargs`],
    /// dispatched directly on the species type object - not through a `+0x128` indirection, unlike every
    /// occupant-entity gate elsewhere in this file), finds or inserts
    /// ([`map_int_habitatsuitability_find_or_insert`]) that species's own suitability record (keyed on
    /// `species_type+0x1ec`) in the tree at `map_ptr`, then walks every owned tile's four direct-entity
    /// slots (`tile+0x4..+0x10`, same shape as [`Self::add_to_building_list`]). For each non-null occupant
    /// passing the `ZTSceneryType` cast gate ([`entity_type_matches`], [`RVA_SCENERY_TYPE_CHECK_ARG`]):
    /// accumulates `BFCategory::getValue(category, type+0x10c) + BFCategory::getValue(category,
    /// vtable-slot-0x20(type))`, scaled `*100` then divided by the *occupant entity's own* `+0x150` dword
    /// (not the type's - confirmed directly against the `.c`'s own `iVar1`/`piVar7` variable split, easy
    /// to misread since both are "the entity" in prose) with plain truncating integer division exactly as
    /// real vanilla's own per-item arithmetic does (only cast to `f32` once, after the whole owned-tile
    /// walk completes), into a running total; ORs the type's own `+0x12a` flag into a per-tile flag
    /// (bumping the record's `matching_item_count` once per matching occupant whose own `+0x12b` byte is
    /// set), and bumps the record's `tiles_with_match_count` once per tile where that OR'd flag ended up
    /// set. Finally adds the accumulated running total onto the record's own `category_score_sum`.
    ///
    /// All integer arithmetic wraps, matching vanilla's 32-bit `ADD`/`IMUL`/`IDIV`. Deviation: an occupant
    /// whose `+0x150` divisor is `0` (vanilla would fault on its `IDIV`) contributes `0` and logs a warning.
    pub fn additional_scenery_suitability_change(&self, species_vector_ptr: u32, map_ptr: u32) {
        let begin: u32 = get_from_memory(species_vector_ptr);
        let end: u32 = get_from_memory(species_vector_ptr + 4);
        for addr in (begin..end).step_by(4) {
            let species_type_ptr: u32 = get_from_memory(addr);
            if !unsafe { call_entity_vtable_noargs(species_type_ptr, 0xcc) } {
                continue;
            }
            let key: i32 = get_from_memory(species_type_ptr + 0x1ec);
            let category_ptr = species_type_ptr + 0x2cc;
            let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, key);

            let mut running_score: i32 = 0;
            for node in walk_tile_list(self.owned_tiles_ptr) {
                let tile: u32 = get_from_memory(node + 0x8);
                let mut tile_flag = false;
                for slot in (0x4u32..=0x10).step_by(4) {
                    let entity_ptr: u32 = get_from_memory(tile + slot);
                    if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                        continue;
                    }
                    let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
                    let arg1: i32 = get_from_memory(entity_type_ptr + 0x10c);
                    let arg2 = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) } as i32;
                    let val1 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg1) };
                    let val2 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg2) };
                    let divisor: i32 = get_from_memory(entity_ptr + 0x150);
                    if divisor == 0 {
                        tracing::warn!("ZTHabitat::additionalScenerySuitabilityChange: entity {:#010x} has a zero +0x150 divisor, skipping its score", entity_ptr);
                    } else {
                        running_score = running_score.wrapping_add(val1.wrapping_add(val2).wrapping_mul(100).wrapping_div(divisor));
                    }

                    if get_from_memory::<u8>(entity_type_ptr + 0x12b) != 0 {
                        save_to_memory::<i32>(record_ptr + 0x1c, get_from_memory::<i32>(record_ptr + 0x1c).wrapping_add(1));
                    }
                    tile_flag |= get_from_memory::<u8>(entity_type_ptr + 0x12a) != 0;
                }
                if tile_flag {
                    save_to_memory::<i32>(record_ptr + 0x14, get_from_memory::<i32>(record_ptr + 0x14).wrapping_add(1));
                }
            }
            let current_score: f32 = get_from_memory(record_ptr + 0x10);
            save_to_memory::<f32>(record_ptr + 0x10, running_score as f32 + current_score);
        }
    }

    /// Ports the setup + owned-tile animal/keeper census portion (phases 1-2) of
    /// `ZTHabitat::recalculateCharacteristics` (`RECALCULATE_CHARACTERISTICS`, `0x00444827`) - see
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 2. Not yet called by
    /// anything: real vanilla's own reentrancy guard ([`Self::reentrancy_guard`]) gates the *entire*
    /// function body as one continuous block, not per-phase, so this method's caller (assembled in a
    /// later stage once every phase is ported) owns setting/clearing that guard and constructing
    /// `map_ptr` ([`init_suitability_scratch_tree`]) before calling this.
    ///
    /// `map_ptr` is the scratch suitability map handle this phase's own animal census both reads and
    /// extends (find-or-insert via [`map_int_habitatsuitability_find_or_insert`]); later phases keep
    /// extending the same map.
    ///
    /// `found_species_vec` is the real vanilla-layout `msvc_std::vector_pod<BFEntityType*>` scratch list
    /// (real vanilla's own `in_stack_fffff3b8`/`_bc`/`_c0` stack local, `msvc_std::vector_pod<>::init`'d by
    /// the caller before this call - a plain `VanillaVector::<u32>::rvo_target()` reproduces that
    /// zero-init identically) of distinct `BFEntityType*` species-type pointers found so far. **Must stay
    /// real vanilla-layout, not a Rust-owned `Vec`** - phase 3's own `addFoundSpecies` calls (still
    /// un-ported, real vanilla) grow/read this exact vector through real vanilla's own `PoolAlloc`-backed
    /// allocator (the same growth path [`vector_push_pool_alloc4`] already reimplements for this struct's
    /// `all_animals` field); handing vanilla a `Box`/`Vec`-owned buffer it might then free through its own
    /// allocator - or vice versa - is the cross-allocator hazard `AGENTS.md`'s Reimplementation Pattern
    /// section warns about. New entries are appended via [`vector_push_pool_alloc4`], never
    /// `Vec::push`. Phase 4 later consumes the fully-extended vector (after phase 3's neighbor-set
    /// propagation) as its own outer loop.
    ///
    /// Returns `false` on real vanilla's own early-return path (`unknown_flag_0x2c` set - matching real
    /// vanilla clearing the reentrancy guard and returning immediately without touching anything below
    /// that check, including never constructing `found_species_vec` at all - real vanilla's own
    /// `vector_pod<>::init` call sits after this check). Returns `true` otherwise, with
    /// `found_species_vec` populated from this census's own owned-tile animals.
    ///
    /// Real body, per `ZTHabitat_recalculateCharacteristics.c`/`.asm` (verified directly against the live
    /// Ghidra project - several statements in the `.c`'s own rendering turned out to be decompiler
    /// artifacts, not real operations, noted below):
    /// - Optional `reviseSpeciesList` call-through if `species_list_dirty` is set (real vanilla still
    ///   un-ported).
    /// - Clears `characteristics_dirty`; zeroes `num_animals`/`num_angry_animals`/
    ///   `unhappy_for_reproduction_count`/`num_sick_animals`/`hungry_count`/`species_found_count`/
    ///   `unknown_flag_0x130`; resets the `all_animals` vector to empty (`_end = _begin`, keeping its
    ///   buffer); zeroes all 16 `keeper_food_category_amounts` slots; zeroes `is_being_serviced_raw`/
    ///   `num_keepers`/`has_keeper_assigned_raw`/`attractiveness`.
    /// - Each census animal adds its entity type's `+0x3b8` into `attractiveness` (`0x00446e13`-`0x00446e1b`).
    /// - Early return if `unknown_flag_0x2c` is set: caller must clear [`Self::reentrancy_guard`] and stop
    ///   (this method just returns `None`).
    /// - Otherwise walks [`Self::owned_tiles_ptr`] ([`walk_tile_list`]); per owned tile, walks *that
    ///   tile's own* occupant list (`BFTile::unit_list_ptr`, `get_from_memory::<u32>(tile_ptr)` as the
    ///   sentinel - same [`walk_tile_list`]/[`TileListNode`] shape one level more nested, exactly
    ///   [`reset_unit_ai_for_tile_occupants`]'s own pattern, confirmed directly against the real `.asm`'s
    ///   register-for-register loop shape, not just the decompile). Per occupant:
    ///   - `ZTAnimal`-castable ([`entity_type_matches`]/[`RVA_ANIMAL_TYPE_CHECK`]) whose own current tile
    ///     ([`BFENTITY_GET_TILE`]) matches this owned tile: inctements `num_animals`; appends onto
    ///     `all_animals` ([`vector_push_pool_alloc4`] on `&self.all_animals_begin`, the same
    ///     established append idiom [`Self::add_to_building_list`] uses); increments
    ///     `num_angry_animals`/`unhappy_for_reproduction_count`/`num_sick_animals`/`hungry_count` per
    ///     their own established per-animal flag/call-through gates (`+0x3aa`/`ZTAnimal::
    ///     isUnhappyForReproduction`/`+0x3a7`/`ZTAnimal::isHungry`, the last masked with
    ///     [`low_byte_bool`] per its raw `u32` return); zeroes 3 scratch dwords at `animal+0x368..0x373`
    ///     (real vanilla's own per-animal reset, meaning not otherwise identified); reads the animal's own
    ///     species id (`entity_type+0x1ec`), stashes it into the shared global
    ///     [`RVA_CURRENT_SPECIES_ID_STASH`], and scans `found_species_types` for a match via
    ///     `ZTSpecies::isSpecialDummySpecies` (real vanilla repurposes this as a plain species-id equality
    ///     check against that stashed global here, not a genuine dummy-species filter - see
    ///     [`RVA_CURRENT_SPECIES_ID_STASH`]'s own doc comment) - appends the species-type pointer and
    ///     increments `species_found_count` only on a genuinely new species; finds-or-inserts
    ///     ([`map_int_habitatsuitability_find_or_insert`]) that species's own record in `map_ptr`,
    ///     unconditionally incrementing its `occurrence_count` (record `+0x00`) and accumulating
    ///     `animal+0x2a8`/`animal+0x2b8` into `sum_unk_2a8`/`sum_unk_2b8` (record `+0x04`/`+0x08`) - the
    ///     `.c`'s own `local_a40.occurrence_count = iVar26` (writing the species id, not a count) right
    ///     before this insert is a second decompiler artifact matching the `footprintY` one above (the
    ///     record's real default constructor, live-decompiled, zero-inits every field unconditionally -
    ///     see [`ZTHabitatSuitabilityRecord`]'s own `Default` impl - so a `+0x00 = species_id` write here
    ///     would leave `occurrence_count` polluted with a huge, nonsensical value with no correcting write
    ///     found anywhere in the corpus); finally, if `show_unit_scan_pending` is set and the animal's own
    ///     vtable slot `+0x4` (`vtable `+0x228`, [`call_entity_vtable_noargs`]) returns true, calls
    ///     `addShowUnit` (real vanilla, still un-ported).
    ///   - `ZTKeeper`-castable ([`RVA_KEEPER_TYPE_CHECK_ARG`]) whose own `+0x170` subtype dword is `0x6e`
    ///     (an already-established fact per [`Self::num_keepers`]'s own doc comment, re-confirmed here
    ///     directly against the real `.asm`'s `CMP [EDI+0x170],0x6e`): increments `num_keepers`, sets
    ///     `is_being_serviced_raw`.
    ///
    /// Then, once per owned tile (not per occupant): walks the tile's own 4 direct-entity slots
    /// (`tile+0x4/0x8/0xc/0x10` - the same shape [`Self::add_to_building_list`]/
    /// [`Self::additional_scenery_suitability_change`] already establish for this exact field group,
    /// confirmed live via the real `.asm`'s explicit `PUSH CAST_ZTSceneryType`/`PUSH DAT_006386c0`
    /// isCastClass tag arguments - the `.c`'s own zero-argument rendering of these two calls is a third
    /// decompiler artifact), gated on both the `ZTSceneryType` and `ZTFood`
    /// ([`RVA_SCENERY_TYPE_CHECK_ARG`]/[`RVA_ZTFOOD_TYPE_CHECK_ARG`]) casts passing (same shape as
    /// [`keeper_food_category_matches`], just against a 4-slot array instead of the single `+0x10`
    /// occupant): adds the item's own `+0x154` amount into `keeper_food_category_amounts[category]`
    /// (`category` from the item's own type `+0x168`, per [`keeper_food_category_matches`]'s
    /// established convention).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub(crate) fn recalc_phase_1_2(&self, map_ptr: u32, found_species_vec: &mut VanillaVector<u32>) -> bool {
        if self.species_list_dirty != 0 {
            unsafe { REVISE_SPECIES_LIST.original()(self as *const Self as *const u32) };
        }
        write_live!(self, characteristics_dirty, 0u8);
        write_live!(self, num_animals, 0i32);
        write_live!(self, num_angry_animals, 0i32);
        write_live!(self, unhappy_for_reproduction_count, 0i32);
        write_live!(self, num_sick_animals, 0i32);
        write_live!(self, hungry_count, 0i32);
        write_live!(self, species_found_count, 0u32);
        write_live!(self, unknown_flag_0x130, 0u8);
        write_live!(self, all_animals_end, self.all_animals_begin);
        for i in 0..16usize {
            unsafe { write_live_ptr(std::ptr::addr_of!(self.keeper_food_category_amounts[i]), 0i32) };
        }
        write_live!(self, is_being_serviced_raw, 0u8);
        write_live!(self, num_keepers, 0i32);
        write_live!(self, has_keeper_assigned_raw, 0u8);
        write_live!(self, attractiveness, 0i32);

        if self.unknown_flag_0x2c != 0 {
            write_live!(self, reentrancy_guard, 0u8);
            return false;
        }

        let base = get_module_base("zoo.exe") as u32;

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;

            let occupant_sentinel = get_from_memory::<u32>(tile_ptr);
            for occ_node in walk_tile_list(occupant_sentinel) {
                let occupant = get_from_memory::<TileListNode>(occ_node).payload;
                if occupant == 0 {
                    continue;
                }

                if unsafe { entity_type_matches(occupant, RVA_ANIMAL_TYPE_CHECK) } {
                    let occupant_tile = unsafe { BFENTITY_GET_TILE.original()(occupant as *const u32) } as u32;
                    if occupant_tile != tile_ptr {
                        continue;
                    }

                    write_live!(self, num_animals, self.num_animals + 1);
                    vector_push_pool_alloc4(std::ptr::addr_of!(self.all_animals_begin) as u32, occupant);

                    if get_from_memory::<u8>(occupant + 0x3aa) != 0 {
                        write_live!(self, num_angry_animals, self.num_angry_animals + 1);
                    }
                    if unsafe { ZTANIMAL_IS_UNHAPPY_FOR_REPRODUCTION.original()(occupant as *const u32) } {
                        write_live!(self, unhappy_for_reproduction_count, self.unhappy_for_reproduction_count + 1);
                    }
                    if get_from_memory::<u8>(occupant + 0x3a7) != 0 {
                        write_live!(self, num_sick_animals, self.num_sick_animals + 1);
                    }
                    if low_byte_bool(unsafe { ZTANIMAL_IS_HUNGRY.original()(occupant as *const u32) }) {
                        write_live!(self, hungry_count, self.hungry_count + 1);
                    }

                    save_to_memory::<u32>(occupant + 0x368, 0);
                    save_to_memory::<u32>(occupant + 0x36c, 0);
                    save_to_memory::<u32>(occupant + 0x370, 0);

                    let entity_type_ptr = get_from_memory::<u32>(occupant + 0x128);
                    write_live!(self, attractiveness, self.attractiveness + get_from_memory::<i32>(entity_type_ptr + 0x3b8));
                    let species_id = get_from_memory::<i32>(entity_type_ptr + 0x1ec);
                    save_to_memory::<i32>(base + RVA_CURRENT_SPECIES_ID_STASH, species_id);
                    let already_found = found_species_vec
                        .as_slice()
                        .iter()
                        .any(|&candidate| unsafe { IS_SPECIAL_DUMMY_SPECIES.original()(candidate as i32) });
                    if !already_found {
                        vector_push_pool_alloc4(found_species_vec.as_ptr() as u32, entity_type_ptr);
                        write_live!(self, species_found_count, self.species_found_count + 1);
                    }

                    let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, species_id);
                    let occurrence_count: i32 = get_from_memory(record_ptr);
                    save_to_memory(record_ptr, occurrence_count + 1);
                    let sum_2a8: i32 = get_from_memory(record_ptr + 0x04);
                    save_to_memory(record_ptr + 0x04, sum_2a8 + get_from_memory::<i32>(occupant + 0x2a8));
                    let sum_2b8: i32 = get_from_memory(record_ptr + 0x08);
                    save_to_memory(record_ptr + 0x08, sum_2b8 + get_from_memory::<i32>(occupant + 0x2b8));

                    if self.show_unit_scan_pending != 0 && unsafe { call_entity_vtable_noargs(occupant, 0x228) } {
                        self.add_show_unit(occupant);
                    }
                } else if unsafe { entity_type_matches(occupant, RVA_KEEPER_TYPE_CHECK_ARG) } {
                    let subtype: u32 = get_from_memory(occupant + 0x170);
                    if subtype == 0x6e {
                        write_live!(self, num_keepers, self.num_keepers + 1);
                        write_live!(self, is_being_serviced_raw, 1u8);
                    }
                }
            }

            for slot_offset in (0x4u32..=0x10).step_by(4) {
                let slot_value: u32 = get_from_memory(tile_ptr + slot_offset);
                if slot_value == 0
                    || !unsafe { entity_type_matches(slot_value, RVA_SCENERY_TYPE_CHECK_ARG) }
                    || !unsafe { entity_type_matches(slot_value, RVA_ZTFOOD_TYPE_CHECK_ARG) }
                {
                    continue;
                }
                let category = get_from_memory::<u32>(get_from_memory::<u32>(slot_value + 0x128) + 0x168) as usize;
                let amount: i32 = get_from_memory(slot_value + 0x154);
                let current: i32 = get_from_memory(std::ptr::addr_of!(self.keeper_food_category_amounts[category]) as u32);
                unsafe { write_live_ptr(std::ptr::addr_of!(self.keeper_food_category_amounts[category]), current + amount) };
            }
        }

        true
    }

    /// Ports the species-found propagation portion (phase 3) of `ZTHabitat::recalculateCharacteristics`
    /// (`RECALCULATE_CHARACTERISTICS`, `0x00444827`) - see
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 3.
    ///
    /// `found_species_vec` is the same real vanilla-layout scratch vector [`Self::recalc_phase_1_2`]
    /// populates and must have already been passed to that call this same `recalculateCharacteristics`
    /// pass. `old_species_found_count`/`old_num_animals` are [`Self::species_found_count`]/
    /// [`Self::num_animals`]'s values from **before** [`Self::recalc_phase_1_2`] zeroed and re-tallied
    /// them (real vanilla's own `iVar31`/`iVar57` locals, snapshotted at the very top of the whole
    /// function, before its own reset block) - the caller must capture both before calling
    /// `recalc_phase_1_2`.
    ///
    /// Real body, per `ZTHabitat_recalculateCharacteristics.c`/`.asm`:
    /// - Walks [`Self::amphibious_neighbors_head`] ([`walk_neighbor_tree`]); per amphibious neighbor,
    ///   calls the still-un-ported real vanilla [`ADD_FOUND_SPECIES`] on that neighbor directly (its own
    ///   owned-tile census, same shape [`Self::recalc_phase_1_2`]'s own per-animal loop performs), then
    ///   walks *that neighbor's own* `show_neighbors_head` (node `+0x14` off the neighbor pointer) and
    ///   calls `ADD_FOUND_SPECIES` on each of those too - two levels deep.
    /// - Then walks this habitat's own [`Self::show_neighbors_head`] and calls `ADD_FOUND_SPECIES` on each
    ///   direct show-neighbor.
    /// - Never calls `ADD_FOUND_SPECIES` on `self` - this habitat's own species were already found by
    ///   [`Self::recalc_phase_1_2`]'s own per-animal census.
    /// - Not ported: real vanilla's own `local_b6c` scratch copy of `found_species_vec` taken right before
    ///   this phase's own neighbor walk (`msvc_std::vector<int>::buy` + `copy4`) - confirmed via exhaustive
    ///   grep that it is allocated and immediately torn down (`_Tidy`, at the very end of the whole
    ///   function) without ever being read in between. A real, observable allocation/free with zero
    ///   observable effect - safe to skip entirely.
    /// - If `species_found_count` changed (`old_species_found_count` vs. the value after every
    ///   `ADD_FOUND_SPECIES` call above), sets `unknown_flag_0x30` ("characteristics changed").
    /// - Separately, if `num_animals` changed (`old_num_animals` vs. current), sets `unknown_flag_0x30`
    ///   too; if the *new* `num_animals` is `0`, additionally: calls the already-ported
    ///   [`Self::send_maint_worker_cleanup_events`] unless a save is currently loading (the same
    ///   `loadInProgress` raw flag [`Self::reset_unit_ai`] reads, [`RVA_APP_INIT_SUCCESS_BASE`]`+0x441` -
    ///   real vanilla calls `ZTUI::gameopts::loadInProgress()`, which is just an accessor for this same
    ///   byte), then zeroes `unknown_nt_time` (real vanilla's own `this->mbr_0x128`/`mbr_0x12c` - the two
    ///   dwords of this field's 8-byte `FileTime` span, confirmed via the struct's own offsets).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub(crate) fn recalc_phase_3(&self, found_species_vec: &mut VanillaVector<u32>, old_species_found_count: u32, old_num_animals: i32) {
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe {
                ADD_FOUND_SPECIES.original()(
                    neighbor_ptr as *const u32,
                    found_species_vec.as_ptr() as *const i32,
                    std::ptr::addr_of!(self.species_found_count) as *const i32,
                )
            };

            let neighbor_show_neighbors_head: u32 = get_from_memory(neighbor_ptr + 0x14);
            for inner_node in walk_neighbor_tree(neighbor_show_neighbors_head) {
                let inner_neighbor_ptr: u32 = get_from_memory(inner_node + 0x10);
                unsafe {
                    ADD_FOUND_SPECIES.original()(
                        inner_neighbor_ptr as *const u32,
                        found_species_vec.as_ptr() as *const i32,
                        std::ptr::addr_of!(self.species_found_count) as *const i32,
                    )
                };
            }
        }

        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe {
                ADD_FOUND_SPECIES.original()(
                    neighbor_ptr as *const u32,
                    found_species_vec.as_ptr() as *const i32,
                    std::ptr::addr_of!(self.species_found_count) as *const i32,
                )
            };
        }

        if old_species_found_count != self.species_found_count {
            write_live!(self, unknown_flag_0x30, 1u8);
        }

        if old_num_animals != self.num_animals {
            write_live!(self, unknown_flag_0x30, 1u8);
            if self.num_animals == 0 {
                let load_in_progress = get_from_memory::<u8>(get_module_base("zoo.exe") as u32 + RVA_APP_INIT_SUCCESS_BASE + 0x441) != 0;
                if !load_in_progress {
                    self.send_maint_worker_cleanup_events();
                }
                unsafe { write_live_ptr(std::ptr::addr_of!(self.unknown_nt_time) as *const u64, 0u64) };
            }
        }
    }

    /// Ports the nested/tank-habitat water-level pass (phase 5) of `ZTHabitat::
    /// recalculateCharacteristics` (`RECALCULATE_CHARACTERISTICS`, `0x00444827`) - see
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 4. Not yet called by
    /// anything - assembled into the full function alongside phases 1-3 (already ported) and 4/6 (not
    /// yet ported) by a later stage.
    ///
    /// `map_ptr`/`found_species_vec` are the same real vanilla-layout scratch structures
    /// [`Self::recalc_phase_1_2`] takes (real vanilla constructs both once at the top of the whole
    /// function and threads them through every phase); this phase passes them straight through to
    /// [`Self::additional_scenery_suitability_change`] unchanged, never touching either itself.
    ///
    /// Real body, per `ZTHabitat_recalculateCharacteristics.c`/`.asm`: walks every amphibious neighbor
    /// ([`walk_neighbor_tree`] over [`Self::amphibious_neighbors_head`]). For each neighbor that
    /// [`Self::is_tank`]:
    /// - Adds the neighbor's own `get_size(false)` into `tank_neighbor_total_size`.
    /// - When the neighbor `is_filled` (`+0x198`), adds that same size into `freshwater_filled_size` or
    ///   `saltwater_filled_size` depending on the neighbor's own `current_water_type` (`+0x18c`, `0` =
    ///   freshwater - the same discriminant [`ZTTankExhibit::is_right_salinity`]'s own doc comment
    ///   establishes); left untouched (neither bucket) when not filled.
    /// - Accumulates `(100.0 - water_purity) * size * 0.01` into a running `weighted_purity_debt`
    ///   (real vanilla's literal `_DAT_00630d64`/`_DAT_00635418` constants - confirmed `100.0`/`0.01`
    ///   against sibling decompiles sharing the same two globals, e.g.
    ///   `ZTFoodType_changeCharacteristic.c`'s own percent-to-fraction conversion).
    /// - If `self` itself is **not** a tank ([`Self::is_tank`]), calls the neighbor's own
    ///   [`Self::additional_scenery_suitability_change`] with `found_species_vec`/`map_ptr`.
    ///
    /// Then, **unconditionally for every amphibious neighbor** (not gated on `is_tank()` - confirmed
    /// directly against the real `.asm`'s branch structure, easy to misread from the `.c` rendering as
    /// tank-only since it sits inside the same source line range), calls the neighbor's own
    /// [`Self::add_to_building_list`] to merge its building list onto `self`'s own
    /// ([`Self::building_list_begin`]) - `self` here is the merge target, the neighbor is `add_to_building_
    /// list`'s own `&self`, matching that method's own doc comment.
    ///
    /// **Two of real vanilla's own locals computed inside the tank branch - a running min/max water
    /// level across every tank neighbor - are never read again anywhere in the rest of the function.**
    /// Confirmed by tracing both variables' real disassembly registers past this point: both are
    /// unconditionally clobbered by unrelated phase-4 code within a handful of instructions, with no
    /// intervening read on any path. Not ported - computing them would have no observable effect, the
    /// same "real, computed, never consumed" shape this plan's own `ZTHabitatSuitabilityRecord` tally
    /// already found for several of that struct's own dwords.
    ///
    /// `tank_neighbor_total_size`/`freshwater_filled_size`/`saltwater_filled_size`/`weighted_purity_debt`
    /// genuinely do survive past this phase, unlike the min/max above - confirmed via a `FUN_004469ba`
    /// call passing all four straight through, immediately after phase 4's own per-species scan. That
    /// call is followed by an unconditional `return` in the `.c` rendering, but it is **not a real
    /// return** - `FUN_004469ba` is a real, un-ported continuation of *this same function*, relocated
    /// elsewhere in the binary by MSVC's hot/cold code-splitting and mislabeled as an independent
    /// function by Ghidra/OOAnalyzer, the same artifact class this plan's own "Corrections made this
    /// pass" item 1 already found for `FUN_00447033`/`FUN_0044703c`. A later stage porting phases 4/6
    /// must thread this method's return value through to that continuation unchanged.
    ///
    /// Must only be called on a live `ZTHabitat` reference; each neighbor visited must also be live
    /// (true for every [`walk_neighbor_tree`] entry, which reads real `ZTHabitat*` pointers directly out
    /// of the tree). Constructing a [`ZTTankExhibit`] reference over a neighbor is only safe once
    /// [`Self::is_tank`] has confirmed it - matches that struct's own doc comment on why an unconfirmed
    /// plain `ZTHabitat` pointer would over-read.
    pub(crate) fn recalc_phase_5(&self, map_ptr: u32, found_species_vec: &mut VanillaVector<u32>) -> RecalcPhase5Summary {
        let mut summary = RecalcPhase5Summary::default();

        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };

            if neighbor.is_tank() {
                let tank = unsafe { ref_from_memory::<ZTTankExhibit>(neighbor_ptr) };
                let size = neighbor.get_size(false);
                summary.tank_neighbor_total_size += size;

                if *tank.is_filled() {
                    if *tank.current_water_type() == 0 {
                        summary.freshwater_filled_size += size;
                    } else {
                        summary.saltwater_filled_size += size;
                    }
                }

                summary.weighted_purity_debt += (100.0 - *tank.water_purity() as f32) * size as f32 * 0.01;

                if !self.is_tank() {
                    neighbor.additional_scenery_suitability_change(found_species_vec.as_ptr() as u32, map_ptr);
                }
            }

            neighbor.add_to_building_list(std::ptr::addr_of!(self.building_list_begin) as u32, self as *const Self as u32);
        }

        summary
    }

    /// Ports the part of `ZTHabitat::recalculateCharacteristics`'s per-species scoring loop (real
    /// vanilla's phase 6, the tail of `0x00444827`) that writes `score_percent_a` (`+0x18`),
    /// `score_building_count` (`+0x20`), their six threshold flags, `tank_depth_score` (`+0x3c`),
    /// `tank_or_visibility_score` (`+0x50`) and `tank_score_baseline` (`+0x54`). Every input was
    /// verified against raw `[ESP+N]` disassembly, not Ghidra's call-site argument names (the `param_N`
    /// list of the `FUN_004469ba` "call" drifts by a dword in places).
    ///
    /// `record_ptr` is the species' [`ZTHabitatSuitabilityRecord`] in the scratch tree, already carrying
    /// phase 4's `raw_percent_a`/`raw_building_count` tallies. `owned_tile_total` is the owned-tile count
    /// (`unaff_EBX`/`param_4` in the decompile). `phase5` is [`Self::recalc_phase_5`]'s summary.
    ///
    /// - The percentage denominator is `owned_tile_total`, plus `freshwater_filled_size +
    ///   saltwater_filled_size` when the species' own vtable `+0xcc` predicate is true; the building-count
    ///   metric divides by four times that.
    /// - Each metric is `raw * 100 / denom` against a species threshold (`+0x34c` / `+0x350`), see
    ///   [`percent_band`]. The `pct == 0 && threshold > 0` tail-jumps to `0x0044703c`/`0x00447033` are
    ///   not calls: they are two-instruction stubs that set the "too low" flag and rejoin the main flow.
    /// - `tank_score_baseline` is `100.0` when the habitat's vtable `+0x28` predicate (which takes the
    ///   species type as a stack argument) is true, else `-100.0`.
    /// - Tank habitat: `tank_or_visibility_score` is the raw water purity (`+0x1a8`); if the species'
    ///   `+0x35c`/`+0x360` water-depth bounds differ, the depth deviation drives `tank_depth_score` and the
    ///   `+0x62`/`+0x63`/`+0x6c` flags. Non-tank habitat: the guest-visibility fallback from the
    ///   tank-neighbor purity debt.
    ///
    /// `sum_category_tally` (`+0x38`) is written by [`Self::recalc_phase_6_companion_tally`].
    ///
    /// Not covered here (still un-ported phase 6): `terrain_type_score`/`scenery_category_score`
    /// normalisation, `score_percent_c` (depends on uninitialised stack), `score_d`/`score_e`, the
    /// shelter/toy flags and the reset-to-clean block that later clears the flags written here.
    ///
    /// Must only be called on a live `ZTHabitat` reference, with `record_ptr`/`species_type_ptr` live.
    pub(crate) fn recalc_phase_6_species_scores(&self, record_ptr: u32, species_type_ptr: u32, owned_tile_total: i32, phase5: &RecalcPhase5Summary) {
        let species_filled_addend = if unsafe { call_vtable_slot_noargs_ret_bool(species_type_ptr, 0xcc) } {
            phase5.freshwater_filled_size.wrapping_add(phase5.saltwater_filled_size)
        } else {
            0
        };
        let denom = owned_tile_total.wrapping_add(species_filled_addend);

        let percent_a = percent_band(
            get_from_memory::<i32>(record_ptr + 0x14),
            denom as f64,
            get_from_memory::<i32>(species_type_ptr + 0x34c),
            4.0,
        );
        save_to_memory::<f32>(record_ptr + 0x18, percent_a.score);
        set_flag_if(record_ptr + 0x5a, percent_a.too_low);
        set_flag_if(record_ptr + 0x5b, percent_a.too_high);
        set_flag_if(record_ptr + 0x68, percent_a.critical);

        let building_count = percent_band(
            get_from_memory::<i32>(record_ptr + 0x1c),
            denom as f64 * 4.0,
            get_from_memory::<i32>(species_type_ptr + 0x350),
            3.0,
        );
        save_to_memory::<f32>(record_ptr + 0x20, building_count.score);
        set_flag_if(record_ptr + 0x58, building_count.too_low);
        set_flag_if(record_ptr + 0x59, building_count.too_high);
        set_flag_if(record_ptr + 0x67, building_count.critical);

        self.recalc_phase_6_tank_visibility(record_ptr, species_type_ptr, phase5);
    }

    /// The `tank_depth_score`/`tank_or_visibility_score`/`tank_score_baseline` half of
    /// [`Self::recalc_phase_6_species_scores`]; see that method for the formulas.
    fn recalc_phase_6_tank_visibility(&self, record_ptr: u32, species_type_ptr: u32, phase5: &RecalcPhase5Summary) {
        let self_addr = self as *const Self as u32;

        save_to_memory::<f32>(record_ptr + 0x50, 100.0);
        save_to_memory::<f32>(record_ptr + 0x3c, 100.0);

        let baseline_positive = unsafe { call_vtable_slot_with_ptr_ret_bool(self_addr, 0x28, species_type_ptr) };
        save_to_memory::<f32>(record_ptr + 0x54, if baseline_positive { 100.0 } else { -100.0 });

        if !self.is_tank() {
            if unsafe { call_vtable_slot_noargs_ret_bool(species_type_ptr, 0xcc) } && phase5.tank_neighbor_total_size != 0 {
                let debt_share = phase5.weighted_purity_debt as f64 / phase5.tank_neighbor_total_size as f64;
                save_to_memory::<f32>(record_ptr + 0x50, (100.0 - debt_share * 100.0) as f32);
            }
            return;
        }

        let tank = unsafe { ref_from_memory::<ZTTankExhibit>(self_addr) };
        save_to_memory::<f32>(record_ptr + 0x50, *tank.water_purity() as f32);

        let min_depth: i32 = get_from_memory(species_type_ptr + 0x35c);
        let max_depth: i32 = get_from_memory(species_type_ptr + 0x360);
        if min_depth == max_depth {
            return;
        }

        let level = *tank.water_level() as i32;
        let deviation = if level < min_depth {
            set_flag_if(record_ptr + 0x62, true);
            (level as f32 - min_depth as f32).abs()
        } else if level > max_depth {
            set_flag_if(record_ptr + 0x63, true);
            (level as f32 - max_depth as f32).abs()
        } else {
            0.0
        };

        let scaled = deviation.min(30.0).max(0.0) * 20.0;
        set_flag_if(record_ptr + 0x6c, scaled >= 50.0);
        save_to_memory::<f32>(record_ptr + 0x3c, (100.0 - scaled).min(100.0));
    }

    /// Ports the terminal block of `ZTHabitat::recalculateCharacteristics` (`0x00445e62`-`0x00446039`),
    /// everything after the per-species scoring loop. Checked against raw disassembly.
    ///
    /// 1. `species_suitability_cache` (`+0x148`) `= *map_ptr` ([`assign_suitability_tree`], a copy into the
    ///    habitat's own store entry).
    /// 2. Keeper walk: the first staff-list entry that casts to `ZTKeeper` and reports this habitat as
    ///    assigned sets `has_keeper_assigned_raw` (`+0x131`, cleared at function entry) and stops the walk.
    /// 3. Per surrounding animal (`animals`, the vector [`Self::recalc_surrounding_animal_building_scan`]
    ///    returned) except those with `animal+0x395 != 0`, and, for a tank habitat, except animal-typed
    ///    entities whose home habitat is another habitat: look up the species' record, and for an
    ///    animal-typed entity in a tank habitat set `critical_water` (`+0x6d`) when *every* amphibious
    ///    neighbor is also a tank; then [`Self::set_animal_conditions`] with the per-call
    ///    `overlap_condition_bits`.
    /// 4. [`Self::recalculate_viewing_areas`], [`Self::check_escapability`], then clear
    ///    `show_unit_scan_pending` (`+0x2f`) and the reentrancy guard (`+0x2e`).
    ///
    /// The two extra condition bits are vanilla's `ph_AdditionalConditionValues` bytes `+4`/`+5`
    /// (`[ESP+0x33]`/`[ESP+0x3b]`), which the census leaves as per-tile scratch and `0x004451a6`-`0x004451f4`
    /// then overwrites with `overlap_tally / owned_tile_total > 0.05` and `> 0.1` - see
    /// [`overlap_condition_bits`]. The struct's other three dwords are not read by
    /// [`Self::set_animal_conditions`].
    ///
    /// Teardown of vanilla's scratch vectors and trees is the caller's job (Stage 8): every one of them is
    /// a Rust-owned or explicitly-freed local here. The early-exit guard clears (`0x0044607a`) are also the
    /// caller's.
    ///
    /// Must only be called on a live `ZTHabitat` reference; `animals` entries must be live entities.
    pub(crate) fn recalc_terminal_block(&self, map_ptr: u32, histogram: &[i32; 18], animals: &VanillaEventVector, overlap_tally: i32, owned_tile_total: i32) {
        let self_addr = self as *const Self as u32;
        assign_suitability_tree(self_addr + 0x148, map_ptr);

        let world_mgr = unsafe { ZTAPP_GET_WORLD_MGR.original()() };
        let staff_list = unsafe { ZTWORLDMGR_GET_STAFF_LIST.original()(world_mgr as *const u32) } as u32;
        let staff_begin: u32 = get_from_memory(staff_list);
        let staff_end: u32 = get_from_memory(staff_list + 4);
        for addr in (staff_begin..staff_end).step_by(4) {
            let staff_ptr: u32 = get_from_memory(addr);
            let is_keeper = staff_ptr != 0 && unsafe { entity_type_matches(staff_ptr, RVA_KEEPER_TYPE_CHECK_ARG) };
            if is_keeper && low_byte_bool(unsafe { ZTSTAFF_IS_HABITAT_ASSIGNED.original()(staff_ptr as *const u32, self_addr as *const u32) }) {
                write_live!(self, has_keeper_assigned_raw, 1u8);
                break;
            }
        }

        let (extra_low_bit, extra_critical_bit) = overlap_condition_bits(overlap_tally, owned_tile_total);
        let is_tank = self.is_tank();
        for addr in (animals.begin..animals.end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if get_from_memory::<u8>(animal_ptr + 0x395) != 0 {
                continue;
            }
            // `ZTBuilding::isAnimalType` (`0x00446c9a`) is not a type test: it returns the animal type's own
            // vtable `+0xcc` predicate, so an animal whose predicate is false is processed even in a tank it
            // does not call home.
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let is_animal_type = animal_type_ptr != 0
                && unsafe { type_check(animal_type_ptr, RVA_ANIMAL_TYPE_CHECK) }
                && unsafe { call_vtable_slot_noargs_ret_bool(animal_type_ptr, 0xcc) };
            if is_tank && is_animal_type && unsafe { animal_home_habitat(animal_ptr) } != self_addr {
                continue;
            }

            let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, get_from_memory(animal_type_ptr + 0x1ec));

            if is_animal_type && is_tank {
                let has_neighbors = walk_neighbor_tree(self.amphibious_neighbors_head).next().is_some();
                let all_neighbors_are_tanks = has_neighbors
                    && walk_neighbor_tree(self.amphibious_neighbors_head).all(|node| {
                        let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                        unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.is_tank()
                    });
                save_to_memory::<u8>(record_ptr + 0x6d, u8::from(all_neighbors_are_tanks));
            }

            self.set_animal_conditions(animal_ptr, record_ptr, histogram.as_ptr() as u32, owned_tile_total, extra_low_bit, extra_critical_bit);
        }

        self.recalculate_viewing_areas();
        self.check_escapability();
        write_live!(self, show_unit_scan_pending, 0u8);
        write_live!(self, reentrancy_guard, 0u8);
    }

    /// `ZTHabitat::recalculateCharacteristics` (`0x00444827`): runs the ported phases in vanilla's own
    /// order and owns the scratch state every phase shares.
    ///
    /// 1. Reentrancy guard; snapshot `species_found_count`/`num_animals`; [`Self::recalc_phase_1_2`]
    ///    (returns early, guard cleared, when `unknown_flag_0x2c` is set).
    /// 2. [`Self::recalc_phase_3`], [`Self::recalc_phase_4_terrain_census`] (also empties and refills the
    ///    building list), the owned-tile count, [`Self::recalc_phase_5`].
    /// 3. [`Self::build_building_type_scratch_arrays`], the two float caches,
    ///    [`Self::recalc_surrounding_animal_building_scan`], [`Self::construct_surrounding_species_list`],
    ///    [`Self::recalc_surrounding_species_flags`].
    /// 4. [`Self::recalc_phase_6_species`] once per found species, then [`Self::recalc_terminal_block`]
    ///    (persists the scratch tree, sets animal conditions, clears the guard).
    /// 5. Teardown of the surrounding-animal vector, float caches, scratch tree and found-species vector.
    ///
    /// The snapshots are taken before `reviseSpeciesList` (inside phase 1-2) rather than after it as in
    /// vanilla; that call rebuilds the species list and does not touch either counter.
    pub(crate) fn recalculate_characteristics(&self) {
        if self.reentrancy_guard != 0 {
            return;
        }
        write_live!(self, reentrancy_guard, 1u8);
        let old_species_found_count = self.species_found_count;
        let old_num_animals = self.num_animals;

        let mut map_header = [0u32; 4];
        let map_ptr = map_header.as_mut_ptr() as u32;
        init_suitability_scratch_tree(map_ptr);
        let mut found_species = VanillaVector::<u32>::rvo_target();

        if !self.recalc_phase_1_2(map_ptr, &mut found_species) {
            destroy_suitability_scratch_tree(map_ptr);
            return;
        }
        let companion_species = found_species.as_slice().to_vec();
        self.recalc_phase_3(&mut found_species, old_species_found_count, old_num_animals);

        let (histogram, overlap_tally, elevation_tally) = self.recalc_phase_4_terrain_census(found_species.as_ptr() as u32, map_ptr);
        let owned_tile_total = walk_tile_list(self.owned_tiles_ptr).count() as i32;
        let phase5 = self.recalc_phase_5(map_ptr, &mut found_species);

        let (mut field_0x16c_values, mut field_0x170_clamped) = self.build_building_type_scratch_arrays();
        let mut float_header_a = [0u32; 4];
        let mut float_header_b = [0u32; 4];
        let float_cache_a = float_header_a.as_mut_ptr() as u32;
        let float_cache_b = float_header_b.as_mut_ptr() as u32;
        init_float_scratch_tree(float_cache_a);
        init_float_scratch_tree(float_cache_b);

        let animals = self.recalc_surrounding_animal_building_scan(map_ptr, &mut field_0x16c_values, &mut field_0x170_clamped, float_cache_a, float_cache_b);
        self.construct_surrounding_species_list();
        self.recalc_surrounding_species_flags(map_ptr, float_cache_a, float_cache_b);

        let inputs = RecalcPhase6Inputs {
            owned_tile_total,
            elevation_tally,
            histogram: &histogram,
            phase5: &phase5,
            float_cache_a,
            float_cache_b,
            field_0x16c_values: &field_0x16c_values,
            field_0x170_clamped: &field_0x170_clamped,
            companion_species: &companion_species,
        };
        for &species_type_ptr in found_species.as_slice() {
            let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, get_from_memory(species_type_ptr + 0x1ec));
            self.recalc_phase_6_species(&inputs, record_ptr, species_type_ptr);
        }

        self.recalc_terminal_block(map_ptr, &histogram, &animals, overlap_tally, owned_tile_total);

        free_event_vector_buffer(animals.begin, animals.cap_end.wrapping_sub(animals.begin));
        clear_float_scratch_tree(float_cache_b);
        clear_float_scratch_tree(float_cache_a);
        destroy_suitability_scratch_tree(map_ptr);
        destroy_found_species_vector(&found_species);
    }

    /// Ports the terrain/scenery half of `recalculateCharacteristics`'s per-species scoring loop
    /// (`0x0044582d`-`0x004459ed`): `terrain_type_score` (`+0x0c`), the water tile adjustments
    /// (`+0x48`/`+0x4c`) and the `scenery_category_score` (`+0x10`) normalisation. Checked against raw
    /// disassembly.
    ///
    /// `histogram` is [`Self::recalc_phase_4_terrain_census`]'s 18-entry terrain tile tally.
    ///
    /// - **Tank habitat**: `terrain_type_score = 100.0` and the loop, `+0x48` and `+0x4c` are skipped
    ///   (`0x00493bfe`); `+0x48`/`+0x4c` keep their zero-initialised value.
    /// - **Otherwise**: `+0x4c = tile_count_adjustment(pred, getValue(cat, 9), histogram[9], owned_tile_total,
    ///   freshwater_filled_size)` and `+0x48` likewise with category 10 and `saltwater_filled_size`
    ///   (`cat = species_type+0x2d8`, `pred` = species vtable `+0xcc`). Then for each of the 18 categories,
    ///   `w = getValue(cat, i)`: if `w < 0` and `histogram[i] > 0`, `terrain_type_score += w`; if `w >= 0`,
    ///   `terrain_type_score += min(w, count * 100 / denom)`, with `count = histogram[i]` plus `+0x4c` for
    ///   `i == 9` / `+0x48` for `i == 10`, and `denom = +0x4c + owned_tile_total + +0x48`.
    /// - `terrain_type_score` is then capped at `100.0`.
    /// - `scenery_category_score /= max(1.0, owned_tile_total * 0.025) * 100.0`, capped at `100.0`, and
    ///   multiplied by `50.0` if still negative.
    ///
    /// A zero `denom` or `owned_tile_total` yields `inf`/`NaN` like vanilla's masked x87 division, without
    /// modelling NaN's compare flags; every habitat that reaches this loop has owned tiles.
    ///
    /// Must only be called on a live `ZTHabitat` reference, with `record_ptr`/`species_type_ptr` live.
    pub(crate) fn recalc_phase_6_terrain_scores(&self, record_ptr: u32, species_type_ptr: u32, owned_tile_total: i32, histogram: &[i32; 18], phase5: &RecalcPhase5Summary) {
        if self.is_tank() {
            save_to_memory::<f32>(record_ptr + 0xc, 100.0);
        } else {
            let category_ptr = species_type_ptr + 0x2d8;
            let get_category_value = |index: i32| unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, index) };
            let species_pred = unsafe { call_vtable_slot_noargs_ret_bool(species_type_ptr, 0xcc) };

            let fresh_adjustment = tile_count_adjustment(species_pred, get_category_value(9), histogram[9], owned_tile_total, phase5.freshwater_filled_size);
            save_to_memory::<i32>(record_ptr + 0x4c, fresh_adjustment);
            let salt_adjustment = tile_count_adjustment(species_pred, get_category_value(10), histogram[10], owned_tile_total, phase5.saltwater_filled_size);
            save_to_memory::<i32>(record_ptr + 0x48, salt_adjustment);

            let denom = fresh_adjustment.wrapping_add(owned_tile_total).wrapping_add(salt_adjustment);
            let mut terrain: f32 = get_from_memory(record_ptr + 0xc);
            for (index, &tiles) in histogram.iter().enumerate() {
                let count = match index {
                    9 => tiles.wrapping_add(fresh_adjustment),
                    10 => tiles.wrapping_add(salt_adjustment),
                    _ => tiles,
                };
                terrain = terrain_category_step(terrain, get_category_value(index as i32), tiles, count, denom);
            }
            save_to_memory::<f32>(record_ptr + 0xc, terrain);
        }
        let terrain: f32 = get_from_memory(record_ptr + 0xc);
        save_to_memory::<f32>(record_ptr + 0xc, if terrain < 100.0 { terrain } else { 100.0 });

        let scenery: f32 = get_from_memory(record_ptr + 0x10);
        save_to_memory::<f32>(record_ptr + 0x10, normalise_scenery_score(scenery, owned_tile_total));
    }

    /// Ports `score_percent_c` (`+0x24`) and its flags (`0x00445bf3`-`0x00445cdb`): the elevation score.
    /// `pct = elevation_tally * 50 / owned_tile_total` against the species threshold at `+0x358`:
    /// `need_elevation` (`+0x5c`) when `pct < threshold - 10`, else `too_much_elevation` (`+0x5d`) when
    /// `pct > threshold + 10`, `critical_elevation` (`+0x69`) when `|pct - threshold| > 20`, and
    /// `score = min(100, 100 - clamp(|pct - threshold|, 0, 50))`. Ghidra's decompile shows this formula's
    /// operands as uninitialised stack; the raw asm reads the elevation tally and `(float)owned_tile_total`
    /// (`[ESP+0x2c]`, stored at `0x00445972`).
    pub(crate) fn recalc_phase_6_elevation_score(&self, record_ptr: u32, species_type_ptr: u32, elevation_tally: i32, owned_tile_total: i32) {
        let threshold: i32 = get_from_memory(species_type_ptr + 0x358);
        let elevation = elevation_band(elevation_tally, owned_tile_total, threshold);
        set_flag_if(record_ptr + 0x5c, elevation.too_low);
        set_flag_if(record_ptr + 0x5d, elevation.too_high);
        set_flag_if(record_ptr + 0x69, elevation.critical);
        save_to_memory::<f32>(record_ptr + 0x24, elevation.score);
    }

    /// Ports the "reset to clean" block of `recalculateCharacteristics`'s per-species scoring loop
    /// (`0x00493c0e`-`0x00493c57`): for a tank habitat with an empty amphibious-neighbor set (the
    /// `std::set` size dword at habitat `+0x0c`, `[ESP+0x43]` in vanilla; the decompile's
    /// `param_17._3_1_`) and a species whose vtable `+0xcc` predicate is true, clears the six percentage
    /// flags plus `+0x5c`/`+0x5d`/`+0x69` and forces `score_percent_c` (`+0x24`) to `100.0`. Runs after
    /// [`Self::recalc_phase_6_species_scores`], whose flags it clears.
    pub(crate) fn recalc_phase_6_reset_tank_flags(&self, record_ptr: u32, species_type_ptr: u32) {
        let neighbor_set_size: u32 = get_from_memory(self as *const Self as u32 + 0xc);
        if !self.is_tank() || neighbor_set_size != 0 || !unsafe { call_vtable_slot_noargs_ret_bool(species_type_ptr, 0xcc) } {
            return;
        }
        for flag_offset in [0x5a, 0x5b, 0x68, 0x58, 0x59, 0x67, 0x5c, 0x5d, 0x69] {
            save_to_memory::<u8>(record_ptr + flag_offset, 0);
        }
        save_to_memory::<f32>(record_ptr + 0x24, 100.0);
    }

    /// Runs one species' phase-6 scoring in vanilla's own order: terrain/scenery, the percentage and tank
    /// scores, the elevation score, the tank reset block, then the shelter/toy scores.
    pub(crate) fn recalc_phase_6_species(&self, inputs: &RecalcPhase6Inputs, record_ptr: u32, species_type_ptr: u32) {
        self.recalc_phase_6_terrain_scores(record_ptr, species_type_ptr, inputs.owned_tile_total, inputs.histogram, inputs.phase5);
        self.recalc_phase_6_species_scores(record_ptr, species_type_ptr, inputs.owned_tile_total, inputs.phase5);
        self.recalc_phase_6_elevation_score(record_ptr, species_type_ptr, inputs.elevation_tally, inputs.owned_tile_total);
        Self::recalc_phase_6_companion_tally(record_ptr, species_type_ptr, inputs.companion_species);
        self.recalc_phase_6_reset_tank_flags(record_ptr, species_type_ptr);
        self.recalc_phase_6_placement_scores(
            record_ptr,
            species_type_ptr,
            inputs.float_cache_a,
            inputs.float_cache_b,
            inputs.field_0x16c_values,
            inputs.field_0x170_clamped,
        );
    }

    /// Ports the `sum_category_tally` (`+0x38`) walk (`.c` lines 1843-1863, vanilla node `+0x4c`): for every
    /// species in `companion_species` other than `species_type_ptr`'s own id, adds
    /// `getValue(cat, id) + getValue(cat, +0x1e8) + getValue(cat, +0x1e4)` to the record, with `cat` the
    /// species' own category list (`+0x2c0`) and `id` the companion's `+0x1ec`.
    /// `companion_species` is the found-species list as it stood before phase 3 extended it - vanilla
    /// bulk-copies it (`copy4`) into a snapshot vector, which is why a watch on appends to that vector
    /// never sees a write.
    pub(crate) fn recalc_phase_6_companion_tally(record_ptr: u32, species_type_ptr: u32, companion_species: &[u32]) {
        let category_ptr = species_type_ptr + 0x2c0;
        let own_id: i32 = get_from_memory(species_type_ptr + 0x1ec);
        for &companion in companion_species {
            let companion_id: i32 = get_from_memory(companion + 0x1ec);
            if companion_id == own_id {
                continue;
            }
            let attrib2: i32 = get_from_memory(companion + 0x1e4);
            let attrib1: i32 = get_from_memory(companion + 0x1e8);
            let by_id = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, companion_id) };
            let by_attrib1 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, attrib1) };
            let by_attrib2 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, attrib2) };
            let current: f32 = get_from_memory(record_ptr + 0x38);
            save_to_memory::<f32>(record_ptr + 0x38, current + by_attrib2.wrapping_add(by_attrib1).wrapping_add(by_id) as f32);
        }
    }

    /// Ports the shelter/toy half of `recalculateCharacteristics`'s per-species scoring loop: the two
    /// building-match counts, the `+0x5f`/`+0x6a` and `+0x61`/`+0x6b` flags, and `score_d` (`+0x30`) /
    /// `score_e` (`+0x34`). Everything was checked against raw disassembly (`0x004456fd`-`0x00445d9d` and
    /// its relocated stubs); the decompile's `param_16`/`param_18`/`param_27` names drift here.
    ///
    /// `float_cache_a`/`float_cache_b` are the two `map<int, _>` scratch trees ([`init_float_scratch_tree`])
    /// that [`Self::recalc_surrounding_species_flags`] reads as `f32`; this scoring loop reads the *same
    /// bits as `i32` counts* (`CMP [node+0x14], 4`, `IMUL`, `FILD`). `field_0x16c_values`/`field_0x170_clamped`
    /// are step (2)'s scratch arrays *after* [`Self::recalc_surrounding_animal_building_scan`] decremented them.
    ///
    /// - **Counts**: over the building list (`self.building_list_begin..end`, index-aligned with the scratch
    ///   arrays), for entries whose `getValue(cat, type+0x10c) + getValue(cat, vtable+0x20(type)) >= 0`
    ///   (`cat = species_type+0x2cc`): `shelter_count` += 1 when `type+0x16c != 0` equals the current
    ///   `field_0x16c_values[i]`; `toy_count` += 1 when `type+0x170 != 0` equals `field_0x170_clamped[i]`.
    ///   `toy_count` is forced to `0` for a show tank.
    /// - **Shelter flags**: `shelter_count > 0` sets `+0x5f` (`need_more_shelter`), `> 3` sets `+0x6a`
    ///   (`critical_shelter`). **Toy flags**: `toy_count > 1` sets `+0x61`, `> 2` sets `+0x6b`.
    /// - **`score_d`** (`+0x30`) = `min(100, 100 - penalty)`. If `+0x5e` (`need_space`, set from
    ///   `float_cache_a`) is set, `penalty` comes from `v = float_cache_a[key]`: `15 * v` for `4 <= v <= 6`,
    ///   and for `v > 6` `(X * 5 - 30) * 20` where `X` is `[ESP+0x68]` - a stack slot the scoring loop
    ///   never writes (its last write, `0x004455ce`, stores `0`), so `X = 0` and the term is `-600`,
    ///   which clamps `score_d` to `100`. That reading is from writer enumeration, not a live capture; the
    ///   Stage 8 live comparison test is what confirms it. The map path also sets `+0x6a` when `v > 7`.
    ///   Otherwise, when `+0x5f` is set: `penalty = 50 * shelter_count + 100` if `shelter_count > 2` else `0`,
    ///   then `* (shelter_count - 3)` when `+0x6a` is set; when `+0x5f` is clear the penalty is `0`.
    /// - **`score_e`** (`+0x34`) is the same shape with `+0x60` (`too_crowded`, from `float_cache_b`), `+0x61`,
    ///   `+0x6b`: map path `penalty = 15 * w` when `w = float_cache_b[key] > 3`; count path `(toy_count - 2)`
    ///   for the critical multiplier.
    ///
    /// Must only be called on a live `ZTHabitat` reference, with `record_ptr`/`species_type_ptr` live.
    pub(crate) fn recalc_phase_6_placement_scores(
        &self,
        record_ptr: u32,
        species_type_ptr: u32,
        float_cache_a: u32,
        float_cache_b: u32,
        field_0x16c_values: &[i32],
        field_0x170_clamped: &[i32],
    ) {
        let key: i32 = get_from_memory(species_type_ptr + 0x1ec);
        let (shelter_count, toy_count) = self.count_matching_buildings(species_type_ptr, field_0x16c_values, field_0x170_clamped);

        if shelter_count > 0 {
            set_flag_if(record_ptr + 0x5f, true);
            set_flag_if(record_ptr + 0x6a, shelter_count > 3);
        }
        let penalty_d = if get_from_memory::<u8>(record_ptr + 0x5e) != 0 {
            let v: i32 = get_from_memory(map_int_float_find_or_insert(float_cache_a, key));
            let penalty = match v {
                4..=6 => v.wrapping_mul(15) as f64,
                7.. => STALE_ESP_0X68.wrapping_mul(5).wrapping_sub(30).wrapping_mul(20) as f64,
                _ => 0.0,
            };
            set_flag_if(record_ptr + 0x6a, v > 7);
            penalty as f32
        } else if get_from_memory::<u8>(record_ptr + 0x5f) != 0 {
            count_penalty(shelter_count, get_from_memory::<u8>(record_ptr + 0x6a) != 0, 3)
        } else {
            0.0
        };
        save_to_memory::<f32>(record_ptr + 0x30, clamped_score(penalty_d));

        if toy_count > 1 {
            set_flag_if(record_ptr + 0x61, true);
            set_flag_if(record_ptr + 0x6b, toy_count > 2);
        }
        let penalty_e = if get_from_memory::<u8>(record_ptr + 0x60) != 0 {
            let w: i32 = get_from_memory(map_int_float_find_or_insert(float_cache_b, key));
            if w > 3 { w.wrapping_mul(15) as f32 } else { 0.0 }
        } else if get_from_memory::<u8>(record_ptr + 0x61) != 0 {
            count_penalty(toy_count, get_from_memory::<u8>(record_ptr + 0x6b) != 0, 2)
        } else {
            0.0
        };
        save_to_memory::<f32>(record_ptr + 0x34, clamped_score(penalty_e));
    }

    /// `(shelter_count, toy_count)` for [`Self::recalc_phase_6_placement_scores`]. The building list only
    /// holds entries that passed the `ZTBuilding` cast at insertion (see
    /// [`Self::build_building_type_scratch_arrays`]), so vanilla's null-type crash stub for a failed
    /// re-check is skipped rather than reproduced.
    fn count_matching_buildings(&self, species_type_ptr: u32, field_0x16c_values: &[i32], field_0x170_clamped: &[i32]) -> (i32, i32) {
        let category_ptr = species_type_ptr + 0x2cc;
        let mut shelter_count = 0;
        let mut toy_count = 0;

        for (index, addr) in (self.building_list_begin..self.building_list_end).step_by(4).enumerate() {
            let entity_ptr: u32 = get_from_memory(addr);
            if !unsafe { entity_type_matches(entity_ptr, RVA_BUILDING_TYPE_CHECK_ARG) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);

            let val1 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, get_from_memory(entity_type_ptr + 0x10c)) };
            let arg2 = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) } as i32;
            let val2 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg2) };
            if val1.wrapping_add(val2) < 0 {
                continue;
            }

            let shelter_key: i32 = get_from_memory(entity_type_ptr + 0x16c);
            if shelter_key != 0 && field_0x16c_values.get(index) == Some(&shelter_key) {
                shelter_count += 1;
            }
            let toy_key: i32 = get_from_memory(entity_type_ptr + 0x170);
            if toy_key != 0 && field_0x170_clamped.get(index) == Some(&toy_key) {
                toy_count += 1;
            }
        }

        if self.is_show_tank() {
            toy_count = 0;
        }
        (shelter_count, toy_count)
    }

    /// Ports `ZTHabitat::recalculateCharacteristics`'s own **second** `owned_tiles_ptr` walk (phase 4's
    /// terrain-census pass, real lines ~834-1000 - distinct from [`Self::recalc_phase_1_2`]'s own earlier
    /// animal/keeper census walk over the same list). See
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 7 for the full identification
    /// trail this is built from, including a disassembly-confirmed correction (this doc comment's own
    /// "Fourth follow-up pass" note) to that plan's own prior "mutually exclusive"/nesting-order framing.
    ///
    /// `species_vector_ptr` is the real vanilla `vector<ZTSpeciesType*>` begin/end pair this walk reads
    /// fresh **once per owned tile** (not once for the whole call) - [`Self::recalc_phase_1_2`]'s own
    /// `found_species_vec` out-param. `map_ptr` is the same function-local scratch
    /// `map<int, ZTHabitatSuitabilityRecord>` tree [`Self::additional_scenery_suitability_change`] already
    /// reads/writes, found-or-inserted per (tile, species) pair via
    /// [`map_int_habitatsuitability_find_or_insert`] exactly like that function does (a real, cheap,
    /// idempotent tree lookup - matching real vanilla's own redundant per-tile re-lookup rather than
    /// caching the record pointer across tiles).
    ///
    /// Returns `(histogram, overlap_tally, elevation_tally)`:
    /// - `histogram` is real vanilla's own `local_a88` - an 18-element per-habitat terrain-type tile-count
    ///   histogram (`histogram[tile->terrain_type_byte] += 1`, counted only when the tile's own flag bit
    ///   `0x0800_0000` is **clear** - a real vanilla-layout dword at `tile+0x80` this codebase's own
    ///   [`crate::ztmapview::BFTile`] currently only exposes as four opaque bytes,
    ///   `unknown_byte_1`..`_4`/`unknown_byte_1` at that same offset - read directly here as a raw dword
    ///   rather than widening that struct for one caller). This is exactly [`Self::set_animal_conditions`]'s
    ///   own `terrain_histogram_ptr` parameter. Computed once per tile, entirely independent of the species
    ///   loop below (confirmed via disassembly: the histogram increment at `0x00444fd5`-`0x00444fed` sits
    ///   *before* the per-species loop's own guard check at `0x0044500a`).
    /// - `overlap_tally` is real vanilla's own `iVar31` - a per-owned-tile count, incremented once per tile
    ///   when a single flag (`0x00445168`'s `TEST AL,AL`/`JNZ 0x004469dc`, real vanilla's own
    ///   `[ESP+0x3b]`) ends up set after the species loop below finishes. That flag is reset once per tile
    ///   (`0x00445001`, alongside `[ESP+0x33]`) and OR'd, once per occupant across *every* species and all
    ///   4 slots for this tile, with `entity_type_ptr+0x11c > 0` (confirmed at `0x0044692d`-`0x0044693a`,
    ///   `[ESP+0x3f]` in that deeper call frame - a consistent `+4` stack-offset delta from the `0x33`/`0x3b`
    ///   reset instructions confirms these are the same physical slots viewed from a deeper point in the
    ///   same tile iteration). This is exactly the previous pass's own `fVar24`/`puVar18` machinery's real
    ///   input (`0x004451a6`-`0x004451f4`) - the ratio/threshold computation itself (`iVar31 as f32 /
    ///   dVar45 as f32`) is still Stage 8's own job (`dVar45`, the total owned-tile count, isn't available
    ///   here).
    ///
    /// **Fourth follow-up pass correction**: the plan doc's own "Third follow-up pass" text described the
    /// tile-flag-gated `adv_ptr` contribution (below) and the 4-slot entity scan as mutually exclusive
    /// (opposite values of the same bit) and implied a per-found-species *outer* loop wrapping a per-tile
    /// *inner* walk. Both are wrong, corrected here via direct disassembly: the real nesting is tile
    /// **outer** (single pass, matching the already-shipped histogram above) with the species loop
    /// **inner** (walked fresh per tile); and the `adv_ptr` branch (`0x005c6632`-`0x005c667a`) ends with an
    /// unconditional `JMP 0x00445088` **into** the entity-slot loop's own entry point, not around it - so
    /// on a bit-set tile, the `adv_ptr` contribution is *additional* to the entity-slot scan, not an
    /// alternative to it. The plan doc's own "Third follow-up pass" claim that `iVar31` is fed by `OR`ing
    /// *two* flags (`entity_type_ptr+0x12c==0` alongside `+0x11c>0`) is also corrected above - only the
    /// `+0x11c>0` flag reaches the real `iVar31` increment; `entity_type_ptr+0x12c`'s own accumulator
    /// (`[ESP+0x33]`/`[ESP+0x37]`) is read nowhere this pass could find, and is left unread/unported here
    /// pending a real consumer being identified (may simply be dead, matching this function's own
    /// established "computed, never consumed" pattern elsewhere - e.g. [`Self::recalc_phase_5`]'s dead
    /// per-tank min/max water-level locals - but not confirmed dead the way that precedent was).
    ///
    /// **Per-tile, per-species body** (`category_ptr = species_type_ptr + 0x2cc`, `record_ptr =`
    /// [`map_int_habitatsuitability_find_or_insert`]`(map_ptr, species_type_ptr+0x1ec)`):
    /// - If the tile's own `0x0800_0000` flag bit is set: `adv_ptr = get_from_memory(tile_ptr + 0x4c)` (a
    ///   currently-unidentified polymorphic object - has its own vtable, dispatched at slot `0x20`; ruled
    ///   out `ZTAdvTerrainType` as a name, that class has no vtable-dispatched methods of its own anywhere
    ///   in the binary). `val_a = BFCategory::getValue(category_ptr, adv_ptr+0x10c)` (**unscaled**);
    ///   `val_b = BFCategory::getValue(category_ptr, vtable-slot-0x20(adv_ptr))`, scaled via the
    ///   `(x*100 + (x*100>>31 & 3))>>2` round-to-nearest-divide-by-4 bit trick (confirmed at
    ///   `0x005c6654`-`0x005c6663`, the same trick this plan's own "Rounding note" already documents for
    ///   phase 6); `record.scenery_category_score += val_a + val_b_scaled` (confirmed at
    ///   `0x005c666c`-`0x005c6673`).
    /// - Unconditionally, walk the tile's own 4 direct-occupant slots (`tile+0x4..+0x10`, same shape as
    ///   [`Self::add_to_building_list`]/[`Self::additional_scenery_suitability_change`]). Per non-null
    ///   occupant passing the `ZTSceneryType` cast gate ([`entity_type_matches`],
    ///   [`RVA_SCENERY_TYPE_CHECK_ARG`]): `val1/val2 = BFCategory::getValue(category_ptr,
    ///   entity_type_ptr+0x10c)`/`getValue(category_ptr, vtable-slot-0x20(entity_type_ptr))`;
    ///   `divisor = get_from_memory(entity_ptr + 0x150)` (**entity, not entity_type**); `record.
    ///   scenery_category_score += (val1+val2)*100/divisor` (plain truncating `IDIV`, added directly per
    ///   occupant, unlike [`Self::additional_scenery_suitability_change`]'s own accumulate-then-add-once
    ///   pattern - confirmed at `0x004468f6`-`0x00446911`; same zero-divisor deviation as that function
    ///   applies here too). If also `ZTBuilding` ([`RVA_BUILDING_TYPE_CHECK_ARG`]) and
    ///   `(entity_type_ptr+0x23e != 0 OR entity_type_ptr+0x170 > 0)` and `!self.has_bldg(entity_ptr)`:
    ///   append `entity_ptr` into `self.building_list` ([`Self::has_bldg`]'s own push idiom, confirmed at
    ///   `0x004480c4`-`0x0044810c`). If `val1+val2 >= 0`: bump `record.raw_building_count` (`+0x1c`) when
    ///   `entity_type_ptr+0x12b != 0`, and OR `entity_type_ptr+0x12a != 0` into a per-species tile flag.
    ///   After all 4 slots, if that flag is set: `record.raw_percent_a += 1` (`+0x14`, confirmed at
    ///   `0x00446974`-`0x0044697b`).
    ///
    /// Also truncates [`Self::building_list_begin`]`..`[`Self::building_list_end`] back to empty (real
    /// vanilla's own `vector_pod<>::erase(begin, begin, end)`, which keeps the buffer's allocated capacity;
    /// `building_list_cap_end` is left untouched) right before this same walk re-populates it - a second
    /// writer of that field alongside [`Self::add_to_building_list`].
    ///
    /// The third return value is the corner-elevation tally (real vanilla's `local_bf8`, the `[ESP+0xcc]`
    /// slot): per owned tile the corner-height spread plus one per side shared with this habitat, see
    /// [`Self::tile_elevation_contribution`]. It is not dead: it is `score_percent_c`'s numerator
    /// ([`Self::recalc_phase_6_elevation_score`]).
    ///
    /// **Deviation from real vanilla**: real vanilla indexes `local_a88` with the tile's raw terrain-type
    /// byte (`0..=255`) completely unchecked, into a stack array sized for exactly 18 real terrain types -
    /// an out-of-range byte would silently corrupt adjacent stack memory rather than trap. No real-game
    /// terrain type is known to reach that byte value in practice (the array's own size implies real
    /// vanilla data never does either), but a Rust `panic` on out-of-bounds indexing would be strictly
    /// worse (crashes the whole process) than vanilla's own silent corruption, so this port guards the
    /// index and drops the tally on an out-of-range value instead of replicating the overflow.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub(crate) fn recalc_phase_4_terrain_census(&self, species_vector_ptr: u32, map_ptr: u32) -> ([i32; 18], i32, i32) {
        write_live!(self, building_list_end, self.building_list_begin);

        let species_begin: u32 = get_from_memory(species_vector_ptr);
        let species_end: u32 = get_from_memory(species_vector_ptr + 4);

        let mut histogram = [0i32; 18];
        let mut overlap_tally: i32 = 0;
        let mut elevation_tally: i32 = 0;

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            let flags: u32 = get_from_memory(tile_ptr + 0x80);
            if flags & 0x0800_0000 == 0 {
                let terrain_type = (flags & 0xff) as usize;
                if let Some(count) = histogram.get_mut(terrain_type) {
                    *count += 1;
                }
            }

            let mut tile_overlap_flag = false;
            for species_addr in (species_begin..species_end).step_by(4) {
                let species_type_ptr: u32 = get_from_memory(species_addr);
                let key: i32 = get_from_memory(species_type_ptr + 0x1ec);
                let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, key);
                let category_ptr = species_type_ptr + 0x2cc;

                if flags & 0x0800_0000 != 0 {
                    let adv_ptr: u32 = get_from_memory(tile_ptr + 0x4c);
                    let val_a = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, get_from_memory(adv_ptr + 0x10c)) };
                    let raw_b = unsafe { call_entity_vtable_u32_noargs(adv_ptr, 0x20) } as i32;
                    let val_b = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, raw_b) };
                    let scaled = val_b.wrapping_mul(100);
                    let val_b_scaled = scaled.wrapping_add((scaled >> 31) & 3) >> 2;
                    let current: f32 = get_from_memory(record_ptr + 0x10);
                    save_to_memory::<f32>(record_ptr + 0x10, current + (val_a.wrapping_add(val_b_scaled)) as f32);
                }

                let mut tile_or_flag = false;
                for slot in (0x4u32..=0x10).step_by(4) {
                    let entity_ptr: u32 = get_from_memory(tile_ptr + slot);
                    if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                        continue;
                    }
                    let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);

                    let arg1: i32 = get_from_memory(entity_type_ptr + 0x10c);
                    let arg2 = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) } as i32;
                    let val1 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg1) };
                    let val2 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg2) };
                    let divisor: i32 = get_from_memory(entity_ptr + 0x150);
                    if divisor == 0 {
                        tracing::warn!(
                            "ZTHabitat::recalculateCharacteristics terrain census: entity {:#010x} has a zero +0x150 divisor, skipping its scenery contribution",
                            entity_ptr
                        );
                    } else {
                        let contribution = val1.wrapping_add(val2).wrapping_mul(100).wrapping_div(divisor);
                        let current: f32 = get_from_memory(record_ptr + 0x10);
                        save_to_memory::<f32>(record_ptr + 0x10, current + contribution as f32);
                    }

                    if unsafe { entity_type_matches(entity_ptr, RVA_BUILDING_TYPE_CHECK_ARG) } {
                        let flag: u8 = get_from_memory(entity_type_ptr + 0x23e);
                        let count: i32 = get_from_memory(entity_type_ptr + 0x170);
                        if (flag != 0 || count > 0) && !self.has_bldg(entity_ptr) {
                            vector_push_pool_alloc4(std::ptr::addr_of!(self.building_list_begin) as u32, entity_ptr);
                        }
                    }

                    if val1.wrapping_add(val2) >= 0 {
                        if get_from_memory::<u8>(entity_type_ptr + 0x12b) != 0 {
                            save_to_memory::<i32>(record_ptr + 0x1c, get_from_memory::<i32>(record_ptr + 0x1c).wrapping_add(1));
                        }
                        tile_or_flag |= get_from_memory::<u8>(entity_type_ptr + 0x12a) != 0;
                    }
                    tile_overlap_flag |= get_from_memory::<i32>(entity_type_ptr + 0x11c) > 0;
                }

                if tile_or_flag {
                    save_to_memory::<i32>(record_ptr + 0x14, get_from_memory::<i32>(record_ptr + 0x14).wrapping_add(1));
                }
            }

            elevation_tally = elevation_tally.wrapping_add(self.tile_elevation_contribution(tile_ptr));

            if tile_overlap_flag {
                overlap_tally = overlap_tally.wrapping_add(1);
            }
        }

        (histogram, overlap_tally, elevation_tally)
    }

    /// One owned tile's share of the census' corner-elevation tally (`0x004450fe`-`0x00445168`): the
    /// spread `max - min` of the tile's four corner heights ([`tile_corner_height`] at corners `1`, `3`, `5`,
    /// `7`), plus `1` for each of the tile's four sides (`tile+0x82` two-bit fields, values `2` and `3`)
    /// whose neighbouring tile belongs to this same habitat. A side with no neighbouring tile counts as
    /// another habitat's (`0x005acfbe` zeroes the compared pointer).
    fn tile_elevation_contribution(&self, tile_ptr: u32) -> i32 {
        let heights = [1, 3, 5, 7].map(|corner| tile_corner_height(tile_ptr, corner));
        let spread = heights.iter().max().unwrap() - heights.iter().min().unwrap();

        let side_bits: u8 = get_from_memory(tile_ptr + 0x82);
        let world = globals().ztworldmgr();
        let self_addr = self as *const Self as u32;
        let shared_sides = [0u32, 2, 4, 6]
            .into_iter()
            .filter(|&side| (side_bits >> side) & 3 > 1)
            .filter(|&side| {
                let neighbour_ptr = get_neighbour_raw(&world, tile_ptr, side);
                neighbour_ptr != 0
                    && globals()
                        .zthabitatmgr()
                        .get_habitat_ptr(get_from_memory::<i32>(neighbour_ptr + 0x34), get_from_memory::<i32>(neighbour_ptr + 0x38))
                        == self_addr
            })
            .count() as i32;
        spread.wrapping_add(shared_sides)
    }

    /// Ports `ZTHabitat::recalculateCharacteristics`'s "at least five distinct passes" breakdown's own step
    /// (2) (see `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 7, "Real,
    /// previously-unknown complication" blockquote) - real vanilla's `local_c1c`/`local_c10` scratch
    /// arrays, confirmed via direct disassembly at `0x0044523a`-`0x00448288` (real lines ~1056-1155,
    /// right after [`Self::recalc_phase_5`]'s own body - that method's caller already performs the
    /// `additionalScenerySuitabilityChange`/`addToBuildingList` calls this decompile region's own lines
    /// ~1015-1055 make, so this method's real body starts clean at `self.building_list`'s entry count).
    ///
    /// One pass over [`Self::building_list_begin`]`..`[`Self::building_list_end`], producing two
    /// same-length parallel arrays (real vanilla reserves both to the building list's own entry count up
    /// front via `PoolAlloc::allocate`, confirmed at `0x00448155`/`0x004481ad` - ported here as plain
    /// `Vec`s, matching this plan's own "STL/allocator note" that these two scratch arrays are safe,
    /// genuinely transient `Box`-backed equivalents). Per building-list entry (`entity_ptr`,
    /// `entity_type_ptr = get_from_memory(entity_ptr + 0x128)`):
    /// - First array: `entity_type_ptr+0x16c` if `entity_type_ptr+0x23e != 0`, else `0` (confirmed at
    ///   `0x00448207`-`0x00448217`).
    /// - Second array: `entity_type_ptr+0x170`, clamped to `0` if `<= 0` (confirmed at
    ///   `0x0044821b`-`0x0044822b`, real vanilla's own `count & ((count<1)-1)` bit trick) - read
    ///   **unconditionally**, regardless of the `+0x23e` branch above (both paths converge at
    ///   `0x00448217` before this read, confirmed via `0x0044828f`'s own `JMP 0x00448217`).
    ///
    /// **Deviation from real vanilla**: real vanilla re-checks `entity_type_ptr != 0 &&
    /// isCastClass(entity_type_ptr, CAST_ZTBuilding)` per entry (`0x004481e3`-`0x004481ff`), falling back to
    /// treating the entry as absent (both array values `0`) on failure - **and its own fallback path for
    /// "isCastClass returned false" (`0x005acfd3`) clobbers `EAX` to `0` and jumps back into the
    /// `entity_type_ptr+0x23e` read itself, i.e. real vanilla would dereference the near-null address
    /// `0x23e` on that path**, not skip the read. This is never actually reachable: every entry in
    /// [`Self::building_list_begin`] was itself gated on the identical `isCastClass(ZTBuilding)` check at
    /// insertion time, by both writers ([`Self::add_to_building_list`],
    /// [`Self::recalc_phase_4_terrain_census`]) - so a stored `entity_ptr` always has a non-null
    /// `entity_type_ptr` that always passes the same cast check, making the re-check (and its near-null
    /// fallback dereference) provably dead in practice. This port relies on that invariant and skips the
    /// re-check entirely rather than translating a deliberately-unreachable raw address dereference into
    /// Rust (which would require actually reading near-null memory to replicate byte-for-byte).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub(crate) fn build_building_type_scratch_arrays(&self) -> (Vec<i32>, Vec<i32>) {
        let mut field_0x16c_values = Vec::new();
        let mut field_0x170_clamped = Vec::new();

        for addr in (self.building_list_begin..self.building_list_end).step_by(4) {
            let entity_ptr: u32 = get_from_memory(addr);
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);

            let a: i32 = if get_from_memory::<u8>(entity_type_ptr + 0x23e) != 0 {
                get_from_memory(entity_type_ptr + 0x16c)
            } else {
                0
            };
            let raw_b: i32 = get_from_memory(entity_type_ptr + 0x170);

            field_0x16c_values.push(a);
            field_0x170_clamped.push(raw_b.max(0));
        }

        (field_0x16c_values, field_0x170_clamped)
    }

    /// Ports step (4) of `ZTHabitat::recalculateCharacteristics`'s own "at least five distinct passes"
    /// breakdown (see `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 7, "Real,
    /// previously-unknown complication" blockquote) - confirmed via live Ghidra MCP disassembly at
    /// `0x004452d7`-`0x004455b5` (real lines ~1158-1229 or so).
    ///
    /// `map_ptr` is the same function-local suitability scratch tree every other phase reads/writes.
    /// `field_0x16c_values`/`field_0x170_clamped` are [`Self::build_building_type_scratch_arrays`]'s own
    /// two returned arrays (step (2)) - **mutated in place here** (decremented), matching real vanilla's
    /// own `local_c1c`/`local_c10` (confirmed real vanilla treats them as consumable per-tile-type
    /// budgets, not static gates - see below).
    ///
    /// Calls `GET_SURROUNDING_ANIMALS(self)` (`generated.rs`'s `zthabitat::GET_SURROUNDING_ANIMALS`,
    /// `0x00446436` - a plain call-through, nothing detours it) and, per surrounding animal
    /// (`category_ptr = animal_type_ptr + 0x2cc`, `record_ptr =`
    /// [`map_int_habitatsuitability_find_or_insert`]`(map_ptr, animal_type_ptr+0x1ec)`):
    /// - Runs [`Self::scan_building_list_for_surrounding_animal`] against `field_0x16c_values`/
    ///   `float_cache_a`, gated on `animal_type_ptr+0x3d2` (confirmed at `0x00445412`).
    /// - Then, **unconditionally** (the `TEST CL,CL; JZ` at `0x00445451` only loops the *first* pass; both
    ///   `0x0044545d` and the no-match tails `0x00447064`/`0x00446fac` fall into the second), runs it again
    ///   against `field_0x170_clamped`/`float_cache_b`, gated on `animal_type_ptr+0x3d3` (`0x00445560`).
    ///
    /// **Both scratch-array flags are read off the *surrounding animal's* own `entity_type`, not the
    /// building-list entity's** - confirmed at `0x00445315` (`[ESP+0x28]` is saved once per animal,
    /// straight from `animal_ptr+0x128`, before either pass begins) and re-loaded at both gate sites.
    ///
    /// The two float caches ([`init_float_scratch_tree`]/[`map_int_float_find_or_insert`]) are written
    /// here, as `i32` counts: a pass that reaches the end of the building list without a match bumps
    /// `cache[species_id]` when its flag byte is set (`0x00447064`, `0x00446fac`). The `find`/
    /// `ITERATOR_ASSIGN` calls elsewhere in the block are value-type-agnostic and hit `map_ptr`.
    ///
    /// Returns the `GET_SURROUNDING_ANIMALS` vector: vanilla builds it once here and walks the same
    /// vector again in its terminal block ([`Self::recalc_terminal_block`]), freeing it only at the very
    /// end. The caller owns it and must release it with [`free_event_vector_buffer`]
    /// (`begin`, `cap_end - begin`) when the whole recalculation is done.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub(crate) fn recalc_surrounding_animal_building_scan(
        &self,
        map_ptr: u32,
        field_0x16c_values: &mut [i32],
        field_0x170_clamped: &mut [i32],
        float_cache_a: u32,
        float_cache_b: u32,
    ) -> VanillaEventVector {
        let mut animals = VanillaEventVector::rvo_target();
        self.get_surrounding_animals(animals.as_ptr() as u32);

        for addr in (animals.begin..animals.end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            let animal_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let category_ptr = animal_type_ptr + 0x2cc;
            let key: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, key);

            self.scan_building_list_for_surrounding_animal(record_ptr, category_ptr, animal_type_ptr, 0x3d2, field_0x16c_values, float_cache_a);
            self.scan_building_list_for_surrounding_animal(record_ptr, category_ptr, animal_type_ptr, 0x3d3, field_0x170_clamped, float_cache_b);
        }

        animals
    }

    /// Shared body of [`Self::recalc_surrounding_animal_building_scan`]'s own two near-identical passes
    /// over `self.building_list` (real vanilla repeats this whole sequence twice inline rather than
    /// sharing it - see that method's own doc comment for why porting it once, parameterized on
    /// `flag_offset`/`remaining_counts`, is still faithful). Per non-null occupant passing the
    /// `ZTSceneryType` cast gate ([`entity_type_matches`], [`RVA_SCENERY_TYPE_CHECK_ARG`]):
    /// `val1/val2 = BFCategory::getValue(category_ptr, entity_type_ptr+0x10c)`/`getValue(category_ptr,
    /// vtable-slot-0x20(entity_type_ptr))`; `divisor = get_from_memory(entity_ptr + 0x150)`;
    /// `record.scenery_category_score += (val1+val2)*100/divisor` (vanilla's `[node+0x24]`, plain truncating `IDIV`, added **unconditionally**
    /// regardless of `val1+val2`'s sign - confirmed at `0x004453fc`-`0x00445409`, the sign test happens
    /// strictly after the score is already added; same zero-divisor deviation as
    /// [`Self::additional_scenery_suitability_change`] applies here too). Only if `val1+val2 >= 0` **and**
    /// `animal_type_ptr+flag_offset != 0`: decrement `remaining_counts[index]` (by building-list position,
    /// matching [`Self::build_building_type_scratch_arrays`]'s own iteration order) if it's currently
    /// non-zero, returning `true` from this call if any entry was actually decremented.
    fn scan_building_list_for_surrounding_animal(
        &self,
        record_ptr: u32,
        category_ptr: u32,
        animal_type_ptr: u32,
        flag_offset: u32,
        remaining_counts: &mut [i32],
        float_cache: u32,
    ) {
        let mut matched = false;

        for (index, addr) in (self.building_list_begin..self.building_list_end).step_by(4).enumerate() {
            let entity_ptr: u32 = get_from_memory(addr);
            if !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);

            let arg1: i32 = get_from_memory(entity_type_ptr + 0x10c);
            let arg2 = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) } as i32;
            let val1 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg1) };
            let val2 = unsafe { BFCATEGORY_GET_VALUE.original()(category_ptr as *const u32, arg2) };
            let divisor: i32 = get_from_memory(entity_ptr + 0x150);
            if divisor == 0 {
                tracing::warn!(
                    "ZTHabitat::recalculateCharacteristics surrounding-animal scan: entity {:#010x} has a zero +0x150 divisor, skipping its scenery contribution",
                    entity_ptr
                );
            } else {
                let contribution = val1.wrapping_add(val2).wrapping_mul(100).wrapping_div(divisor);
                let current: f32 = get_from_memory(record_ptr + 0x10);
                save_to_memory::<f32>(record_ptr + 0x10, current + contribution as f32);
            }

            if val1.wrapping_add(val2) >= 0
                && get_from_memory::<u8>(animal_type_ptr + flag_offset) != 0
                && let Some(slot) = remaining_counts.get_mut(index)
                && *slot != 0
            {
                *slot -= 1;
                matched = true;
            }

            if matched {
                break;
            }
        }

        if !matched && get_from_memory::<u8>(animal_type_ptr + flag_offset) != 0 {
            let key: i32 = get_from_memory(animal_type_ptr + 0x1ec);
            let count_ptr = map_int_float_find_or_insert(float_cache, key);
            save_to_memory::<i32>(count_ptr, get_from_memory::<i32>(count_ptr) + 1);
        }
    }

    /// Ports step (5) of `ZTHabitat::recalculateCharacteristics`'s own "at least five distinct passes"
    /// breakdown (see `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 7 blockquotes) -
    /// `ZTHabitat::constructSurroundingSpeciesList` (`0x00446265`), confirmed via a full live Ghidra MCP
    /// disassembly of the function.
    ///
    /// - Truncates [`Self::surrounding_species_begin`]`..`[`Self::surrounding_species_end`] back to empty
    ///   (capacity untouched) - real vanilla's own opening block computes an element count as `end - end`
    ///   (`.asm`-confirmed both operand reads are the same, unmodified `+0x140` value read twice), so its
    ///   "grow" arm can never actually run; the only real effect of that block is its always-taken arm's
    ///   `end = begin`.
    /// - Appends every raw catalog-entry pointer in [`Self::species_list_begin`]`..`[`Self::species_list_end`]
    ///   directly - no dedup, plain field reads (`.asm`-confirmed no call through `getSpeciesList` for
    ///   `self`'s own list here, unlike the two neighbor walks below - `self` needs no lazy-recalculate
    ///   re-check mid-`recalculateCharacteristics`).
    /// - If `!self.is_tank()` (`self`'s own vtable `+0x20` dispatch, `0x00446307`-`0x0044630e`): walks
    ///   [`Self::amphibious_neighbors_head`] ([`walk_neighbor_tree`]); per neighbor (node `+0x10`), calls
    ///   through real vanilla `getSpeciesList` (`0x00410f26`, [`Self::species_list`]'s own callee, confirmed
    ///   via 3 real `CALL 0x00410f26` sites in this loop) and, for each entry passing that entry's own
    ///   vtable `+0xcc` predicate ([`call_entity_vtable_noargs`] - the same "species wants water"-shaped slot
    ///   [`Self::scan_building_list_for_surrounding_animal`]'s sibling `FUN_0044699a` call site documents),
    ///   appends it if not already present.
    /// - Unconditionally walks [`Self::show_neighbors_head`] the same way, but with **no** `+0xcc` gate -
    ///   every entry not already present gets appended.
    ///
    /// Both neighbor walks dedup by linear scan of the vector-so-far before appending
    /// ([`vector_push_pool_alloc4`] - real vanilla's own `PoolAlloc`-backed doubling-growth shape,
    /// `.asm`-confirmed identical at both call sites, including the `cap_end - begin` free-size math); the
    /// initial self-species-list copy does not dedup at all (real vanilla never checks it against anything,
    /// since the vector was just truncated to empty immediately before).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`]; each neighbor visited via `walk_neighbor_tree` must also be live.
    pub(crate) fn construct_surrounding_species_list(&self) {
        write_live!(self, surrounding_species_end, self.surrounding_species_begin);

        for addr in (self.species_list_begin..self.species_list_end).step_by(4) {
            let species_ptr: u32 = get_from_memory(addr);
            vector_push_pool_alloc4(std::ptr::addr_of!(self.surrounding_species_begin) as u32, species_ptr);
        }

        if !self.is_tank() {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
                for species_ptr in neighbor.species_list() {
                    if !unsafe { call_entity_vtable_noargs(species_ptr, 0xcc) } {
                        continue;
                    }
                    if !self.surrounding_species_contains(species_ptr) {
                        vector_push_pool_alloc4(std::ptr::addr_of!(self.surrounding_species_begin) as u32, species_ptr);
                    }
                }
            }
        }

        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            for species_ptr in neighbor.species_list() {
                if !self.surrounding_species_contains(species_ptr) {
                    vector_push_pool_alloc4(std::ptr::addr_of!(self.surrounding_species_begin) as u32, species_ptr);
                }
            }
        }
    }

    /// Membership test over [`Self::surrounding_species_begin`]/`_end`, same shape as [`Self::has_bldg`] -
    /// used by [`Self::construct_surrounding_species_list`]'s own two neighbor-walk dedup checks.
    fn surrounding_species_contains(&self, species_ptr: u32) -> bool {
        (self.surrounding_species_begin..self.surrounding_species_end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == species_ptr)
    }

    /// Ports the real first consumer of `local_c30`/`local_bc0` - `constructSurroundingSpeciesList`'s own
    /// follow-up walk back in `recalculateCharacteristics`'s own body, immediately after the call
    /// (`0x004455bb`-`0x004456e6`, confirmed via both a full decompile re-pull and cross-checked
    /// disassembly of the loop setup/entry). Per entry in the now-populated `self.surrounding_species`
    /// (keyed by `species_ptr+0x1ec`, the species id):
    /// - Looks up `float_cache_a[key]` (real vanilla's own inlined lower-bound-then-insert-default-`0.0`
    ///   dance - equivalent to [`map_int_float_find_or_insert`], which this port uses instead of
    ///   hand-rolling the dance, same precedent as every other consumer of that helper) and, if `> 0.0`
    ///   (`.asm`-confirmed `CMP dword,0; JG` on the value's raw bits - exactly equivalent to a positive-`f32`
    ///   test for any non-NaN value), looks up `map_int_habitatsuitability_find_or_insert(map_ptr, key)`
    ///   and sets its flag_pack byte at record `+0x5e` (node `+0x72` - the decompile's own `iVar31` there is
    ///   untyped/plain-`int`, so its `+0x72` is unscaled byte arithmetic).
    /// - Does the same against `float_cache_b`, setting a *different* flag_pack byte, record `+0x60` (node
    ///   `+0x74` - confirmed via the decompile's own `(dword *)in_stack_fffff388 + 0x1d`, where the `+0x1d`
    ///   scales by the pointer's `dword` element size before the final byte cast: `0x1d * 4 == 0x74`). Both
    ///   land inside the struct's already-established `0x58`-`0x6d` flag_pack range once corrected for the
    ///   record-vs-node `+0x14` offset this plan's own struct-layout table already documents.
    ///
    /// **Not yet ported**: real vanilla's `float_cache_a` branch (only) additionally accumulates
    /// `float_cache_a[key]`'s own raw value into a running total (`puVar52` in the decompile) that gets
    /// threaded as an argument into what this stage's own "scope correction" blockquote already confirmed
    /// is decompiler sibling-call codegen, not a real function call (`FUN_004469ba`/etc. are
    /// `recalculateCharacteristics`'s own body, reached by a real tail-`CALL`/`RET` pair). Its real
    /// destination lies inside phase 6's own big per-species scoring loop, which starts immediately after
    /// this walk and is entirely unexamined so far - left unported until that loop itself is traced.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`], after [`Self::construct_surrounding_species_list`] has already
    /// populated `self.surrounding_species`.
    pub(crate) fn recalc_surrounding_species_flags(&self, map_ptr: u32, float_cache_a: u32, float_cache_b: u32) {
        for addr in (self.surrounding_species_begin..self.surrounding_species_end).step_by(4) {
            let species_ptr: u32 = get_from_memory(addr);
            let key: i32 = get_from_memory(species_ptr + 0x1ec);

            let value_a: f32 = get_from_memory(map_int_float_find_or_insert(float_cache_a, key));
            if value_a > 0.0 {
                let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, key);
                save_to_memory::<u8>(record_ptr + 0x5e, 1);
            }

            let value_b: f32 = get_from_memory(map_int_float_find_or_insert(float_cache_b, key));
            if value_b > 0.0 {
                let record_ptr = map_int_habitatsuitability_find_or_insert(map_ptr, key);
                save_to_memory::<u8>(record_ptr + 0x60, 1);
            }
        }
    }

    /// Ports `ZTHabitat::setAnimalConditions` (`SET_ANIMAL_CONDITIONS`, `0x00447164`) - see
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 5. Translates one
    /// per-species [`ZTHabitatSuitabilityRecord`] (`record_ptr`) into `animal_ptr`'s own 128-bit
    /// low/critical condition flag word (`animal+0x260+0x38/0x3c` = "low" bits, `animal+0x260+0x40/0x44`
    /// = "critical" bits - a `ZTAnimal`-embedded field, not otherwise named in this pass's own scope).
    ///
    /// `terrain_histogram_ptr` is the real vanilla-layout `int[18]` per-habitat terrain-type tile-count
    /// array real vanilla calls this with (`recalculateCharacteristics`'s own second owned-tile walk,
    /// "local_a88" per the master plan's Stage 7 notes - not yet ported as of this stage). `extra_low_bit`/
    /// `extra_critical_bit` are the two lone bytes (`+4`/`+5`) real vanilla's own 5th argument struct
    /// (`ph_AdditionalConditionValues*`, Ghidra's own placeholder type with no real field layout) is
    /// actually read for by this function - every other byte of that struct is unread here, so Stage 7's
    /// own caller only needs to build these two bools, not the whole struct, to drive this port.
    ///
    /// Real body, confirmed directly against the live Ghidra project (decompile + full disassembly - this
    /// function's own decompile is corrupted throughout in the same way `recalculateCharacteristics`'s is,
    /// heavy `unaff_ESI`/`unaff_EDI`/`unaff_retaddr` register-tracking failure requiring a register-level
    /// disassembly trace to resolve correctly):
    /// - Every bit below is independently, unconditionally set-or-cleared from its own condition (real
    ///   vanilla's own read-modify-write always both ANDs the bit off and conditionally ORs it back on) -
    ///   so this port builds two fresh `u64` values (`low`/`critical`, bit 0 = `animal+0x298`'s own bit 0,
    ///   bit 32 = `animal+0x29c`'s own bit 0, and the same split for `critical`/`0x2a0`+`0x2a4`) and writes
    ///   all four dwords once at the end, rather than replicating the RMW pattern read-for-read.
    /// - Named condition bits (`record`'s 22 flag `bool`s, real field names/order confirmed via
    ///   `ZTSpeciesAttribs` - see [`ZTHabitatSuitabilityRecord`]'s own doc comment): each "critical"
    ///   flag sets BOTH of its corresponding low-word bit positions in the critical word (not just one) -
    ///   confirmed via disassembly, e.g. `criticalElevation` alone sets both the bit `tooMuchElevation`
    ///   would use and the bit `needElevation` would use, within the critical word only.
    ///   - bit 8 `tank_too_shallow`, bit 9 `tank_too_deep` (low); `critical_tank_depth` -> critical bits
    ///     8 AND 9.
    ///   - bit 11: real vanilla's own vtable slot `+0x28` on `self` (`ZTHabitat`), called twice - once
    ///     with no extra argument (result inverted into the low bit), once with the animal's own
    ///     `entity_type` pointer as a second thiscall argument IF `entity_type` is non-null and passes
    ///     [`type_check`]`(entity_type_ptr, `[`RVA_ANIMAL_TYPE_CHECK`]`)` else `0` (result inverted into
    ///     the critical bit). Base `ZTHabitat`'s own pole at this slot (`0x00446995`,
    ///     `OOAnalyzer::BFEntity::vf_return1`) is a shared, ~150-class "always return true" stub that
    ///     never reads its arguments (same shared-stub pattern [`Self::is_tank_base_default`]'s own doc
    ///     comment documents for slot `+0x20`) - dispatched live via
    ///     [`call_vtable_slot_noargs_ret_bool`]/[`call_vtable_slot_with_ptr_ret_bool`] rather than assumed
    ///     constant, in case a real override (e.g. `ZTTankExhibit`) behaves differently.
    ///   - bit 12 `need_foliage`, bit 13 `too_much_foliage` (low); `critical_foliage` -> critical bits 12
    ///     AND 13.
    ///   - bit 17 `need_elevation`, bit 18 `too_much_elevation` (low); `critical_elevation` -> critical
    ///     bits 17 AND 18.
    ///   - bit 19 `need_rocks`, bit 20 `too_many_rocks` (low); `critical_rocks` -> critical bits 19 AND
    ///     20.
    ///   - bit 57 `need_more_shelter`, bit 58 `need_space` (low); `critical_shelter` -> critical bits 57
    ///     AND 58.
    ///   - bit 59 `need_toys`, bit 60 `too_crowded` (low); `critical_toys` -> critical bits 59 AND 60.
    ///   - bit 61: `extra_low_bit` (low), `extra_critical_bit` (critical) - real vanilla's own
    ///     `param_4[+4]`/`param_4[+5]` byte reads.
    ///   - bit 63: `record.critical_water` -> low only (no critical-word counterpart - `criticalWater`
    ///     is itself already a critical-severity flag).
    /// - Per-category loop (bits 21-56, 18 categories x 2 bits each = "too low" bit at `21+2i`, "too
    ///   high" bit at `22+2i`; both bits are cleared in both words at the top of every iteration, tank
    ///   or not, and the set/compute arms are skipped when [`Self::is_tank`] - confirmed via two identical
    ///   `is_tank()`-equivalent vtable-`0x20` dispatches gating both branches): for each category `i` in
    ///   `0..18`, reads a per-species config value via `BFCategory::getValue(entity_type+0x2d8, i)` (real
    ///   vanilla's own THIRD distinct `getCategoryList`-shaped offset on `ZTAnimalType`, alongside the
    ///   already-used `+0x2c0`/`+0x2cc` - `entity_type` here is `0` when the same cast check gating bit 11
    ///   above fails, matching real vanilla's own un-null-checked pointer arithmetic on that case rather
    ///   than adding a defensive guard vanilla itself doesn't have).
    ///   - Config value `< 0` (species doesn't care about this category): if
    ///     `terrain_histogram_ptr[i] > 0`, sets the "too high" bit only - no critical-word effect.
    ///   - Config value `>= 0`: computes `pct = count * 100.0 / (record.occurrence_count +
    ///     record.fresh_water_tile_adjustment + record.salt_water_tile_adjustment)`, where `count` is
    ///     `terrain_histogram_ptr[i]` normally, but `record.fresh_water_tile_adjustment` for `i == 9` and
    ///     `record.salt_water_tile_adjustment` for `i == 10` (real vanilla's own two special-cased
    ///     category indices, confirmed via disassembly - presumably the freshwater/saltwater tile
    ///     categories). Sets "too low" (config below `pct - 3.0`) or "too high" (config above `pct +
    ///     3.0`) in the low word; if `|pct - config| > 6.0`, sets BOTH bits in the critical word too
    ///     (real vanilla's own `cls_0x4478e5::meth_0x44791e`/`meth_0x4478e5` calls - both fully resolved
    ///     this pass: trivial `this->mbr_0x2a0/0x2a4 |=/&= mask` and `this->mbr_0x298/0x29c |=/&= mask`
    ///     bit-set/clear helpers on the SAME animal condition word this function already writes directly,
    ///     not a separate "post a reason" call as earlier speculated).
    /// - After the loop, real vanilla reuses category 17's own "too low" bit position (bit 55, literal
    ///   `0x800000` relative to the high dword) for two final, unrelated conditions that OVERWRITE
    ///   whatever the loop itself left there: low bit 55 = `record.tank_or_visibility_score < 50.0`,
    ///   critical bit 55 = `record.tank_or_visibility_score < 25.0`. Ordering matters only for this one
    ///   bit - applied last in this port, matching real vanilla's own final-write-wins semantics.
    ///
    /// Must only be called on a live `ZTHabitat` reference (`self`'s own vtable slot `+0x28` is
    /// dispatched); `animal_ptr`/`record_ptr` must also be live.
    pub(crate) fn set_animal_conditions(&self, animal_ptr: u32, record_ptr: u32, terrain_histogram_ptr: u32, owned_tile_total: i32, extra_low_bit: bool, extra_critical_bit: bool) {
        let record = unsafe { ref_from_memory::<ZTHabitatSuitabilityRecord>(record_ptr) };
        let self_addr = self as *const Self as u32;

        fn apply_bit(word: &mut u64, bit: u32, val: bool) {
            if val {
                *word |= 1u64 << bit;
            } else {
                *word &= !(1u64 << bit);
            }
        }

        // Every condition is a read-modify-write of one bit in the animal's live words: bits this function
        // does not name keep their value.
        let word = |lo: u32| get_from_memory::<u32>(animal_ptr + lo) as u64 | (get_from_memory::<u32>(animal_ptr + lo + 4) as u64) << 32;
        let mut low: u64 = word(0x298);
        let mut critical: u64 = word(0x2a0);

        apply_bit(&mut low, 8, record.tank_too_shallow);
        apply_bit(&mut low, 9, record.tank_too_deep);
        apply_bit(&mut critical, 8, record.critical_tank_depth);
        apply_bit(&mut critical, 9, record.critical_tank_depth);

        apply_bit(&mut low, 12, record.need_foliage);
        apply_bit(&mut low, 13, record.too_much_foliage);
        apply_bit(&mut critical, 12, record.critical_foliage);
        apply_bit(&mut critical, 13, record.critical_foliage);

        apply_bit(&mut low, 17, record.need_elevation);
        apply_bit(&mut low, 18, record.too_much_elevation);
        apply_bit(&mut critical, 17, record.critical_elevation);
        apply_bit(&mut critical, 18, record.critical_elevation);

        apply_bit(&mut low, 19, record.need_rocks);
        apply_bit(&mut low, 20, record.too_many_rocks);
        apply_bit(&mut critical, 19, record.critical_rocks);
        apply_bit(&mut critical, 20, record.critical_rocks);

        apply_bit(&mut low, 57, record.need_more_shelter);
        apply_bit(&mut low, 58, record.need_space);
        apply_bit(&mut critical, 57, record.critical_shelter);
        apply_bit(&mut critical, 58, record.critical_shelter);

        apply_bit(&mut low, 59, record.need_toys);
        apply_bit(&mut low, 60, record.too_crowded);
        apply_bit(&mut critical, 59, record.critical_toys);
        apply_bit(&mut critical, 60, record.critical_toys);

        apply_bit(&mut low, 61, extra_low_bit);
        apply_bit(&mut critical, 61, extra_critical_bit);

        apply_bit(&mut low, 63, record.critical_water);
        // Set unconditionally, right after the critical-water test (`0x0044...` `OR [ESI+0x2a4], 0x80000000`).
        apply_bit(&mut critical, 63, true);

        let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
        let cast_ok = entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) };
        let type_arg = if cast_ok { entity_type_ptr } else { 0 };

        // Both real call sites push one stack argument (the slot's shared stub is `RET 4`); a no-argument
        // call here would let the callee pop four bytes of the caller's frame.
        let vtable_28_low = unsafe { call_vtable_slot_with_ptr_ret_bool(self_addr, 0x28, type_arg) };
        let vtable_28_critical = unsafe { call_vtable_slot_with_ptr_ret_bool(self_addr, 0x28, type_arg) };
        apply_bit(&mut low, 11, !vtable_28_low);
        apply_bit(&mut critical, 11, !vtable_28_critical);

        let is_tank = self.is_tank();
        let category_list_ptr = type_arg + 0x2d8;
        let denom = (owned_tile_total + record.fresh_water_tile_adjustment + record.salt_water_tile_adjustment) as f32;

        for i in 0..18i32 {
            let bit_lo = (21 + 2 * i) as u32;
            let bit_hi = (22 + 2 * i) as u32;
            // Vanilla clears both bits of every category, in both words, at the top of each iteration -
            // including for tanks - so a category that is back in range stops reporting.
            for bit in [bit_lo, bit_hi] {
                apply_bit(&mut low, bit, false);
                apply_bit(&mut critical, bit, false);
            }
            if is_tank {
                continue;
            }
            let config_value = unsafe { BFCATEGORY_GET_VALUE.original()(category_list_ptr as *const u32, i) };

            if config_value < 0 {
                let count = get_from_memory::<i32>(terrain_histogram_ptr + (i as u32) * 4);
                if count > 0 {
                    apply_bit(&mut low, bit_hi, true);
                }
            } else {
                let count = match i {
                    9 => get_from_memory::<i32>(terrain_histogram_ptr + (i as u32) * 4) + record.fresh_water_tile_adjustment,
                    10 => get_from_memory::<i32>(terrain_histogram_ptr + (i as u32) * 4) + record.salt_water_tile_adjustment,
                    _ => get_from_memory::<i32>(terrain_histogram_ptr + (i as u32) * 4),
                };
                let pct = (count as f32 * 100.0) / denom;
                let config = config_value as f32;

                if config < pct - 3.0 {
                    apply_bit(&mut low, bit_hi, true);
                } else if pct + 3.0 < config {
                    apply_bit(&mut low, bit_lo, true);
                }
                if (pct - config).abs() > 6.0 {
                    apply_bit(&mut critical, bit_lo, true);
                    apply_bit(&mut critical, bit_hi, true);
                }
            }
        }

        apply_bit(&mut low, 55, record.tank_or_visibility_score < 50.0);
        apply_bit(&mut critical, 55, record.tank_or_visibility_score < 25.0);

        unsafe {
            write_live_ptr((animal_ptr + 0x298) as *const u32, low as u32);
            write_live_ptr((animal_ptr + 0x29c) as *const u32, (low >> 32) as u32);
            write_live_ptr((animal_ptr + 0x2a0) as *const u32, critical as u32);
            write_live_ptr((animal_ptr + 0x2a4) as *const u32, (critical >> 32) as u32);
        }
    }

    /// Ports `ZTHabitat::checkEscapability` (`CHECK_ESCAPABILITY`, `0x00446088`) - see
    /// `zthabitat-recalculatecharacteristics-implementation-plan.md`'s Stage 6. Not yet called by
    /// anything - wired into the full `recalculateCharacteristics` call graph by Stage 8 alongside phases
    /// 1-6 and `setAnimalConditions`.
    ///
    /// Rate-limited: increments the shared global counter at [`RVA_CHECK_ESCAPABILITY_COUNTER`] on every
    /// call and returns immediately unless `unknown_flag_0x30` is already set or the counter reaches
    /// `0x1e` (30), at which point both are reset and the real scan runs. Confirmed directly against the
    /// real disassembly (this function's own `.c` rendering is clean; verified at the instruction level
    /// anyway given how much of the surrounding `recalculateCharacteristics` family turned out corrupted).
    ///
    /// Real body once the gate passes:
    /// - Clears `field_0x16c` (real vanilla `msvc_std::map<int, EscapabilityRecord>::clear`,
    ///   [`MSVC_TREE36_CLEAR`]) - see `field_0x16c`'s own doc comment (`pad6`) for the cache's shape.
    /// - Calls the still-un-ported [`GET_CLOSE_OUTSIDE_TILE`]; a null result ends the pass with the cache
    ///   left empty (already cleared above).
    /// - Reads the tile's own position triple (`tile+0x34/0x38/0x3c`, [`crate::ztmapview::BFTile`]'s
    ///   `pos` field) as the pathfind target. If the tile's own `+0x85` byte has bit `0x20` set, looks up
    ///   the [`ZTHabitatMgr::get_habitat_ptr`] occupant at that position and, when one exists, dispatches
    ///   its own vtable `+0x20` slot (discarding the result - real vanilla does too, confirmed via
    ///   disassembly: the call's `EAX` result is immediately clobbered by the next instruction) and adds
    ///   its `+0x188` field into the target's own Z component. **Deviation from real vanilla**: real
    ///   vanilla dereferences the occupant unconditionally with no null guard here; this port adds one
    ///   (`occupant != 0`), matching this codebase's established "Crash Fix" precedent for an analogous
    ///   unchecked-vanilla-dereference case ([`ZTHabitatMgr::find_better_gates_for_neighbors`]'s own
    ///   `+0x128` read) rather than replicating a null-pointer crash byte-for-byte.
    /// - Sets up `GLOBAL_ZTAIMgr`'s own pathfinding-query scratch fields (`+0x18`/`+0x1c`/`+0x20`) from
    ///   `GLOBAL_ZTWorldMgr`'s `map_x_size`/`map_y_size` - state the AIMgr's own embedded pathfinder
    ///   (`AIMgr+0x10`, dispatched below) reads internally. `ZTAIMgr` itself stays out of scope for
    ///   reimplementation (per this file's own established convention of exposing it only as `globals().
    ///   ztaimgr_ptr()`, an opaque handle) - these are raw offset pokes, not a modeled struct.
    /// - Builds one representative animal per distinct species present in the habitat (real vanilla's own
    ///   local `map<int, ZTAnimal*>` scratch tree, keyed by `entity_type_ptr` (`animal+0x128`) with
    ///   last-one-wins semantics over [`Self::get_all_animals`]`(false)`) - modeled here as a plain
    ///   `HashMap`, safe per this plan's own "genuinely transient, never touched by un-ported vanilla
    ///   code" rule (built and torn down entirely within this call, unlike `field_0x16c` itself).
    /// - For each representative animal: temporarily forces `animal+0x391` to `1` and dispatches the
    ///   animal's own vtable `+0x1ac` slot (real name `diagonalOK` per `generated.rs`'s `ztanimal::
    ///   DIAGONAL_OK`, `this->[+0x391]==0` - forcing the byte to `1` makes every real-vanilla override of
    ///   this slot that follows the same shape return `false`; dispatched live rather than called as a
    ///   fixed address, matching [`Self::set_animal_conditions`]'s own precedent, in case some animal
    ///   subclass overrides it differently), restores the byte, and stashes the (masked) result into
    ///   `GLOBAL_ZTAIMgr+0x5c`. Then calls the still-un-ported [`BFENTITY_GET_GRID_POS`] to convert the
    ///   animal's own world position into a 3-int grid position, and dispatches `GLOBAL_ZTAIMgr+0x10`'s
    ///   own embedded pathfinder object's vtable slot `0` with `(grid_pos, tile_pos, animal_ptr, 0)` -
    ///   same shared `(this, ptr, ptr, ptr, u32) -> bool` shape [`call_vtable_slot_ptr_ptr_ptr_u32_ret_
    ///   bool`]'s own doc comment already documents for the *other* `GLOBAL_ZTAIMgr` path-reachability
    ///   dispatch (`ZTHabitat::getNearestDirtPile`/`getNearestSickAnimal`'s `this=ai_mgr, vtable+0x1c`) -
    ///   confirmed as a different call site (different `this`, different slot) via the real stack-argument
    ///   trace, not assumed identical.
    /// - When that call returns `true`, inserts/overwrites `field_0x16c[entity_type_ptr]` ([`MSVC_TREE36_
    ///   OPERATOR_INDEX`], find-or-insert) with `{can_escape: true, close_outside_tile_ptr: tile_ptr}`.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds `field_0x16c`'s
    /// raw address and the [`GET_CLOSE_OUTSIDE_TILE`] call.
    pub(crate) fn check_escapability(&self) {
        let base = get_module_base("zoo.exe") as u32;
        let counter_addr = base + RVA_CHECK_ESCAPABILITY_COUNTER;
        let counter = get_from_memory::<i32>(counter_addr) + 1;
        save_to_memory(counter_addr, counter);

        if self.unknown_flag_0x30 == 0 && counter < 0x1e {
            return;
        }
        save_to_memory(counter_addr, 0i32);
        write_live!(self, unknown_flag_0x30, 0u8);

        let cache_head = self as *const Self as u32 + 0x16c;
        unsafe { MSVC_TREE36_CLEAR.original()(cache_head as *const u32) };

        let tile_ptr = self.get_close_outside_tile();
        if tile_ptr == 0 {
            return;
        }

        let mut tile_pos = [
            get_from_memory::<i32>(tile_ptr + 0x34),
            get_from_memory::<i32>(tile_ptr + 0x38),
            get_from_memory::<i32>(tile_ptr + 0x3c),
        ];

        if get_from_memory::<u8>(tile_ptr + 0x85) & 0x20 != 0 {
            let occupant = globals().zthabitatmgr().get_habitat_ptr(tile_pos[0], tile_pos[1]);
            if occupant != 0 {
                unsafe { call_vtable_slot_noargs(occupant, 0x20) };
                tile_pos[2] += get_from_memory::<i32>(occupant + 0x188);
            }
        }

        let aimgr = globals().ztaimgr_ptr() as u32;
        let worldmgr_ptr = globals().ztworldmgr_ptr() as u32;
        let world = globals().ztworldmgr();
        save_to_memory(aimgr + 0x18, worldmgr_ptr + 0x8);
        save_to_memory(aimgr + 0x1c, world.map_y_size);
        save_to_memory(aimgr + 0x20, world.map_x_size * world.map_y_size);

        let mut representative_by_species: HashMap<u32, u32> = HashMap::new();
        for animal_ptr in self.get_all_animals(false) {
            let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            representative_by_species.insert(entity_type_ptr, animal_ptr);
        }

        for (species_key, &animal_ptr) in &representative_by_species {
            let orig_flag = get_from_memory::<u8>(animal_ptr + 0x391);
            save_to_memory(animal_ptr + 0x391, 1u8);
            let diagonal_ok = unsafe { call_vtable_slot_noargs_ret_bool(animal_ptr, 0x1ac) };
            save_to_memory(animal_ptr + 0x391, orig_flag);
            save_to_memory(aimgr + 0x5c, diagonal_ok as u32 & 0xff);

            let mut grid_pos = [0i32; 3];
            unsafe { BFENTITY_GET_GRID_POS.original()(animal_ptr as *const u32, grid_pos.as_mut_ptr()) };

            let can_reach = unsafe {
                call_vtable_slot_ptr_ptr_ptr_u32_ret_bool(
                    aimgr + 0x10,
                    0,
                    grid_pos.as_ptr() as u32,
                    tile_pos.as_ptr() as u32,
                    animal_ptr,
                    0,
                )
            };

            if can_reach {
                let key = *species_key;
                let value_ptr = unsafe { MSVC_TREE36_OPERATOR_INDEX.original()(cache_head as *const u32, &key as *const u32) } as u32;
                save_to_memory(value_ptr, 1u8);
                save_to_memory(value_ptr + 8, tile_ptr);
            }
        }
    }

    /// Ports `ZTHabitat::removeViewingAreas` (`ZTHabitat_removeViewingAreas.c`, `generated.rs`'s
    /// `REMOVE_VIEWING_AREAS`): a no-op if `unknown_flag_0x2c` is set (the same guard
    /// `recalculateCharacteristics`'s own early-return uses). Otherwise destroys and frees every entry in
    /// [`Self::viewing_areas_begin`]/`_end` (call-through to real vanilla `ZTViewingArea::~ZTViewingArea` +
    /// `operator_delete`, same as [`Self::remove_viewing_area`]), then resets `_end` back to `_begin` -
    /// keeps the backing buffer rather than freeing it, unlike a full vector destructor.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds the vector field
    /// write at the end.
    pub fn remove_viewing_areas(&self) {
        if self.unknown_flag_0x2c != 0 {
            return;
        }
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;
        let mut cursor = begin;
        while cursor != end {
            let va_ptr: u32 = get_from_memory(cursor);
            if va_ptr != 0 {
                unsafe {
                    ZTVIEWINGAREA_DESTRUCTOR.original()(va_ptr as *const u32);
                    OPERATOR_DELETE.original()(va_ptr);
                }
            }
            cursor += 4;
        }
        save_to_memory(self_addr + 0x38, begin);
    }

    /// Ports `ZTHabitat::removeSpecies` (`ZTHabitat_removeSpecies.c`/`.asm`): scans the real vanilla
    /// `std::vector<(u32, Ambients*)>` at `ambients_begin`/`ambients_end` (the same vector
    /// [`Self::update`] already walks to play each entry's own ambient sound) for an 8-byte pair whose
    /// key matches `species_key`, no-oping if none is found. On a match, tears down the pair's own
    /// `Ambients*` (if non-null) via [`Ambients::destruct`] (a faithful field-level reimplementation of
    /// real vanilla `Ambients::~Ambients` - safe to call on an object either pole allocated, since both
    /// interpret the same real memory layout and free through the same real vanilla allocator) followed
    /// by `OPERATOR_DELETE` on the outer block, then shifts every later pair down by one slot
    /// (`.asm`-confirmed plain `memmove`-shaped copy, no reallocation) and decrements `ambients_end` by
    /// `8` - matches vanilla's own vector-erase exactly, no PoolAlloc/freelist involvement since this
    /// vector's own growth (real vanilla `ZTHabitat::addSpecies`, still un-ported - see
    /// `zthabitatmgr-implementation-plan.md`'s step 6f notes) never shrinks capacity on erase either.
    ///
    /// `species_key` is treated as an opaque equality key throughout (matching real vanilla, which never
    /// dereferences it here) - real callers pass the same catalog-entry pointer value `addSpecies` itself
    /// stored as each pair's key.
    pub fn remove_species(&self, species_key: u32) {
        let mut p = self.ambients_begin;
        while p != self.ambients_end {
            if get_from_memory::<u32>(p) == species_key {
                let ambients_ptr = get_from_memory::<u32>(p + 0x4);
                if ambients_ptr != 0 {
                    unsafe { (*(ambients_ptr as *mut Ambients)).destruct() };
                    unsafe { OPERATOR_DELETE.original()(ambients_ptr) };
                }

                let mut src = p + 0x8;
                let mut dst = p;
                while src != self.ambients_end {
                    save_to_memory(dst, get_from_memory::<u32>(src));
                    save_to_memory(dst + 0x4, get_from_memory::<u32>(src + 0x4));
                    src += 0x8;
                    dst += 0x8;
                }
                write_live!(self, ambients_end, self.ambients_end - 0x8);
                return;
            }
            p += 0x8;
        }
    }

    /// Ports `ZTHabitat::setDirtyCharacteristics` (`ZTHabitat_setDirtyCharacteristics.c`/`.asm`): sets
    /// `characteristics_dirty`, guarded so it never re-enters an already-dirty habitat (the same guard
    /// real vanilla's own recursive walk depends on to terminate over a neighbor graph that may contain
    /// cycles), then recursively marks every amphibious- and show-neighbor
    /// ([`walk_neighbor_tree`] over [`Self::amphibious_neighbors_head`]/[`Self::show_neighbors_head`],
    /// the same two sets step 6d already resolved) dirty in turn - both trees are walked (unlike
    /// [`Self::set_time_last_serviced`], which only walks the amphibious set), confirmed directly against
    /// the decompile's own two, near-identical tree-walk blocks.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds directly into
    /// the tree walk, and each neighbor visited must also be live (true for every `walk_neighbor_tree`
    /// entry, which reads real `ZTHabitat*` pointers directly out of the tree).
    ///
    /// Takes `&self` and writes the flag volatilely before recursing: a neighbour cycle re-enters this
    /// same habitat, so a `&mut self` here would alias.
    pub fn set_dirty_characteristics(&self) {
        if self.characteristics_dirty != 0 {
            return;
        }
        write_live!(self, characteristics_dirty, 1u8);
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
    }

    /// Ports `ZTHabitat::acceptDonation` (`ZTHabitat_acceptDonation.c`): adds `amount` to both
    /// `current_donations`/`total_donations`, then calls the already-ported
    /// [`crate::zoostatus::ZooStatus::increase_donations`]/[`crate::ztgamemgr::ZTGameMgr::add_cash`]
    /// directly on the real global `ZTGameMgr`/its embedded `ZooStatus` sub-object (`this + 0x10`) - the
    /// same `&GLOBAL_ZTGameMgr->field_0x10` access `zoostatus.rs`'s own `f_grant_donation` already
    /// establishes for this exact call pair, reused here rather than a real-vanilla call-through since
    /// both real methods are already faithful, live-tested Rust ports.
    pub fn accept_donation(&self, amount: f32) {
        write_live!(self, current_donations, self.current_donations + amount);
        write_live!(self, total_donations, self.total_donations + amount);
        let ztgamemgr_ptr = globals().ztgamemgr_ptr();
        let zoostatus_ptr = (ztgamemgr_ptr as u32 + 0x10) as *mut ZooStatus;
        unsafe { (*zoostatus_ptr).increase_donations(amount) };
        unsafe { (*ztgamemgr_ptr).add_cash(amount) };
    }

    /// Ports `ZTHabitat::setTimeLastServiced` (`ZTHabitat_setTimeLastServiced.c`/`.asm`): sets
    /// [`Self::time_last_serviced`] (`+0xec`), then - when `propagate` is set - recursively calls itself
    /// with `propagate = false` on every amphibious neighbor ([`walk_neighbor_tree`] over
    /// [`Self::amphibious_neighbors_head`] only, not `show_neighbors_head` too - confirmed directly
    /// against the decompile, which reads only `field_0x8`). Real vanilla's own decompile has no guard
    /// against visiting the same neighbor twice (unlike [`Self::set_dirty_characteristics`]'s own dirty-
    /// flag guard), but since the recursive call always passes `propagate = false`, the walk is only ever
    /// one level deep regardless - no risk of infinite recursion either way.
    pub fn set_time_last_serviced(&self, time: u32, propagate: bool) {
        write_live!(self, time_last_serviced, time);
        if propagate {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.set_time_last_serviced(time, false);
            }
        }
    }

    /// Ports `ZTHabitat::setDeterioration` (`ZTHabitat_setDeterioration.c`/`.asm`): a 3-way clamp on
    /// [`Self::deterioration`] - `0` forces `0`, `1` only applies when the current value is not already
    /// `2` (never lowers a `2` to a `1`), `2` always forces `2`, any other level is a no-op. Returns the
    /// resulting level (real vanilla's own full-`EAX` `dword` return, `.asm`-confirmed).
    pub fn set_deterioration(&self, level: u32) -> u32 {
        if level == 0 {
            write_live!(self, deterioration, 0u32);
        } else if level == 1 {
            if self.deterioration != 2 {
                write_live!(self, deterioration, 1u32);
            }
        } else if level == 2 {
            write_live!(self, deterioration, 2u32);
        }
        self.deterioration
    }

    /// Ports `ZTHabitat::triggerKeeperArrived` (`ZTHabitat_triggerKeeperArrived.c`/`.asm`): recalculates
    /// via the still-deferred real vanilla `recalculateCharacteristics` when `characteristics_dirty` is
    /// set (the same call-through shape every other dirty-gated method here uses), then inlines
    /// `setScheduledForService(this, false)` - real vanilla hardcodes the bool clear (the macOS `.c`
    /// passes `'\0'`, the Windows `.asm` feeds the branch a literal `PUSH 0x0`, leaving the increment
    /// arm dead), so the only live effect is [`Self::scheduled_service_counter`] decremented once,
    /// clamped at 0 - then walks the real vanilla `all_animals_begin`/`_end` vector calling the
    /// undetoured `ZTAnimal::setKeeperArrives` on every animal (which assigns `1` to the animal's own
    /// `+0x39c` flag byte only when its own `canService(animal, keeper)` low byte passes - the flag is
    /// assigned, never cleared, here). Real vanilla's loop has no null-animal skip, reproduced as-is.
    /// When `scheduled` is set, the same notification recurses into every amphibious neighbor with
    /// `scheduled = false` - real vanilla's own self-call sits at the (detoured) function address, so
    /// under a hook the recursion re-enters this port (same documented shape as
    /// [`Self::set_time_last_serviced`]'s propagation, and one level deep for the same reason).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::trigger_death_arrived`] - `self`'s own address is passed straight into the
    /// `recalculateCharacteristics` call-through above.
    pub fn trigger_keeper_arrived(&self, keeper_ptr: u32, scheduled: bool) {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        write_live!(self, scheduled_service_counter, self.scheduled_service_counter.wrapping_sub(1).max(0));
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            unsafe { SET_KEEPER_ARRIVES.original()(animal_ptr as *const u32, keeper_ptr as *const u32) };
        }
        if scheduled {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.trigger_keeper_arrived(keeper_ptr, false);
            }
        }
    }

    /// Ports `ZTHabitat::triggerDeathArrived` (`ZTHabitat_triggerDeathArrived.c`): recalculates via the
    /// still-deferred real vanilla `recalculateCharacteristics` when `characteristics_dirty` is set (the
    /// same call-through shape every other dirty-gated getter here already uses), then walks the real
    /// vanilla `all_animals_begin`/`_end` vector, setting each matching animal's own `+0x39d` byte (real
    /// vanilla's own "death arrived" flag - name not otherwise confirmed, no reader identified in this
    /// pass's own scope) to `1`. `species_key == 0` matches every animal in the vector unconditionally;
    /// otherwise only animals whose own catalog-entry pointer (`+0x128`) passes
    /// [`entity_type_matches`]([`RVA_ANIMAL_TYPE_CHECK`]) and whose species catalog id (`+0x1ec`, same
    /// offset [`ZTHabitatMgr::distinct_species_catalog_ids`] already uses) equals `species_key`.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - `self`'s own address is passed straight into the
    /// `recalculateCharacteristics` call-through above.
    pub fn trigger_death_arrived(&self, species_key: i32) {
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr == 0 {
                continue;
            }
            if species_key == 0 || Self::animal_species_matches(animal_ptr, species_key) {
                save_to_memory(animal_ptr + 0x39d, 1u8);
            }
        }
    }

    pub(crate) fn animal_species_matches(animal_ptr: u32, species_key: i32) -> bool {
        let type_matches = unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) };
        type_matches && get_from_memory::<i32>(get_from_memory::<u32>(animal_ptr + 0x128) + 0x1ec) == species_key
    }

    /// Checks whether `keeper_ptr`'s own assigned-species id list (`keeper_ptr+0x270`/`+0x274`, a real
    /// vanilla `std::vector<u32>` of catalog ids not otherwise modeled in this codebase) contains
    /// `animal_ptr`'s own numeric id (`+0x124`, `BFEntity::id` - see `ztworldmgr.rs`) - the shared "is this
    /// keeper actually assigned to service this specific animal" membership check
    /// `ZTHabitat_getNearestSickAnimal.c`/`_getNumSicklyAnimals.c` both inline identically.
    pub(crate) fn keeper_assigned_to_animal(keeper_ptr: u32, animal_ptr: u32) -> bool {
        let animal_id: u32 = get_from_memory(animal_ptr + 0x124);
        let begin: u32 = get_from_memory(keeper_ptr + 0x270);
        let end: u32 = get_from_memory(keeper_ptr + 0x274);
        (begin..end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == animal_id)
    }

    /// Checks whether `tile_ptr` is present in `keeper_ptr`'s own `std::vector<BFTile*>` exclusion list at
    /// `keeper_ptr+0x27c`(begin)/`+0x280`(end) - same linear-scan-for-membership idiom as
    /// [`Self::keeper_assigned_to_animal`], just over tile pointers. Confirmed via
    /// `ZTHabitat_getNearestDirtPile.asm`'s own inlined scan (real vanilla's "not found" polarity -
    /// presence in the list marks the tile as excluded).
    pub(crate) fn keeper_has_invalid_tile(keeper_ptr: u32, tile_ptr: u32) -> bool {
        let begin: u32 = get_from_memory(keeper_ptr + 0x27c);
        let end: u32 = get_from_memory(keeper_ptr + 0x280);
        (begin..end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == tile_ptr)
    }

    /// Ports `ZTHabitat::blockService` (`ZTHabitat_blockService.c`/`.asm`) - decides whether `keeper_ptr`
    /// (real vanilla only ever calls this with a genuine `ZTKeeper*`, hence the leading
    /// [`RVA_KEEPER_TYPE_CHECK_ARG`] guard) should currently be blocked from servicing this habitat.
    /// Returns real vanilla's own 3-state result: `0` once some assignable match this keeper is
    /// compatible with was found (or the trailing show-tank special case matches) - not blocked; `1` if
    /// this habitat owns fewer than 2 tiles, an early-out real vanilla takes before doing anything else;
    /// `2` the default fallback once every other check is exhausted.
    ///
    /// `check_tank_and_neighbors`/`skip_tank_depth_check` are real vanilla's own 3rd/2nd parameters -
    /// names inferred from their own usage (`check_tank_and_neighbors` gates the habitat/keeper-capability
    /// equivalence check below, the tank-depth check, and the amphibious-neighbor walk all at once;
    /// `skip_tank_depth_check` only matters when `check_tank_and_neighbors` is set, and additionally skips
    /// just the tank-depth check within it) - not confirmed against any real caller.
    ///
    /// Two vtable slots are called via raw pointer indirection with a documented-but-unconfirmed
    /// interpretation - see `zthabitatmgr-implementation-plan.md`'s step 6l write-up for the full
    /// identification trail:
    /// - `keeper_ptr`'s own vtable `+0x16c` (inherited unchanged from `BFUnit`, real name unconfirmed -
    ///   the one decompile referencing this address, `ZTGuest::tileFilter`, is a misattribution) - the
    ///   equivalence check below (`is_tank() == this predicate`) reads as "block on a habitat-type/
    ///   staff-capability mismatch", consistent with convergent evidence from several `ZTAnimal`/`BFUnit`
    ///   callers all gating water/movement-capability logic through the identical slot, but not confirmed
    ///   by name.
    /// - `BFEntityType`'s own vtable `+0x20` (`generated.rs::bfentitytype::NULLSUB_29`'s slot - see
    ///   `BFEntityType::isUserType`/`isUserTypeID`) - a per-subclass type/catalog-ID getter, confirmed via
    ///   `ZTKeeper_cleansUp.asm`'s own use of it (the returned ID is linear-searched against
    ///   `ZTKeeperType`'s own `std::vector<int>` at `+0x20c`/`+0x210`, right where `bfentitytype.rs`'s
    ///   already-modeled `ZTKeeperType` fields stop at `+0x208`). The trailing show-tank special case below
    ///   compares its raw return value against `0x2550`, a magic catalog-ID constant faithfully ported
    ///   without full identification (same treatment `ztshow.rs`'s own `STOP_0` already gives this exact
    ///   constant).
    ///
    /// A tank-specific field at `+0x1a8` (only read once [`Self::is_tank`] is confirmed, so always safely
    /// within `ZTTankExhibit`'s own `0x1e8`-byte allocation) is compared against
    /// `ZTKeeperType::clean_tank_threshold` (`+0x208`, already named in `bfentitytype.rs`) - real meaning
    /// of `+0x1a8` itself unconfirmed, a depth/size metric by inference only.
    pub fn block_service(&self, keeper_ptr: u32, skip_tank_depth_check: bool, check_tank_and_neighbors: bool) -> u32 {
        if keeper_ptr == 0 || !unsafe { entity_type_matches(keeper_ptr, RVA_KEEPER_TYPE_CHECK_ARG) } {
            return 0;
        }
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        if walk_tile_list(self.owned_tiles_ptr).count() < 2 {
            return 1;
        }

        if check_tank_and_neighbors {
            let keeper_predicate = unsafe { call_entity_vtable_noargs(keeper_ptr, 0x16c) };
            if self.is_tank() != keeper_predicate {
                return 2;
            }
        }

        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            if animal_ptr != 0 && low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                return 0;
            }
        }

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            if tile_ptr == 0 {
                continue;
            }
            let occupant_ptr: u32 = get_from_memory(tile_ptr + 0x10); // BFTile::entity_ptr
            if occupant_ptr != 0 && low_byte_bool(unsafe { CLEANS_UP.original()(keeper_ptr as *const u32, occupant_ptr as *const u32) }) {
                return 0;
            }
        }

        if check_tank_and_neighbors {
            if !skip_tank_depth_check && self.is_tank() {
                let keeper_entity_type: u32 = get_from_memory(keeper_ptr + 0x128);
                if keeper_entity_type != 0 {
                    let self_addr = self as *const Self as u32;
                    let tank_metric: i32 = get_from_memory(self_addr + 0x1a8);
                    let clean_tank_threshold: i32 = get_from_memory(keeper_entity_type + 0x208);
                    if tank_metric < clean_tank_threshold {
                        return 0;
                    }
                }
            }

            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let neighbor = unsafe { ref_from_memory::<Self>(neighbor_ptr) };
                for addr in (neighbor.all_animals_begin..neighbor.all_animals_end).step_by(4) {
                    let animal_ptr: u32 = get_from_memory(addr);
                    if animal_ptr == 0 || !unsafe { entity_type_matches(animal_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                        continue;
                    }
                    let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                    if unsafe { call_entity_vtable_noargs(entity_type_ptr, 0xcc) }
                        && low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) })
                    {
                        return 0;
                    }
                }
            }
        }

        if self.is_tank() && self.is_show_tank() {
            let keeper_entity_type: u32 = get_from_memory(keeper_ptr + 0x128);
            if keeper_entity_type != 0 && unsafe { call_entity_vtable_u32_noargs(keeper_entity_type, 0x20) } == 0x2550 {
                return 0;
            }
        }

        2
    }

    /// Ports `ZTHabitatMgr::highlightHabitat` (`ZTHabitatMgr_highlightHabitat.c`) - despite the
    /// `ZTHabitatMgr::` decompile namespace, the real function is a free `stdcall` helper taking a
    /// `ZTHabitat*` directly (no `this`), which is why this lives on `ZTHabitat` rather than
    /// `ZTHabitatMgr`. Walks the owned-tile list ([`walk_tile_list`]) setting bit `0x80` at each tile's
    /// own `+0x83` when `hilite` is set, or bit `0x10` at `+0x85` when it isn't - two different flags/
    /// bytes, not a single toggle, confirmed against the real decompile's own two distinct branches (see
    /// [`Self::unhighlight`] for the separate, differently-shaped "clear" function real vanilla exposes
    /// alongside this one).
    pub fn highlight(&self, hilite: bool) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if hilite {
                let flags: u8 = get_from_memory(tile + 0x83);
                save_to_memory(tile + 0x83, flags | 0x80);
            } else {
                let flags: u8 = get_from_memory(tile + 0x85);
                save_to_memory(tile + 0x85, flags | 0x10);
            }
        }
    }

    /// Ports `ZTHabitatMgr::unhighlightHabitat` (`ZTHabitatMgr_unhighlightHabitat.c`) - see
    /// [`Self::highlight`]'s own doc comment for why this lives on `ZTHabitat`. Clears both `+0x83` bit
    /// `0x80` and `+0x85` bit `0x10` on every owned tile - a genuinely different, not merely inverse,
    /// operation from `highlight(false)` (which only ever touches `+0x85`), confirmed directly against
    /// the real decompile.
    pub fn unhighlight(&self) {
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            let flags_83: u8 = get_from_memory(tile + 0x83);
            save_to_memory(tile + 0x83, flags_83 & 0x7f);
            let flags_85: u8 = get_from_memory(tile + 0x85);
            save_to_memory(tile + 0x85, flags_85 & 0xef);
        }
    }

    /// Ports `ZTHabitat::getShowInfoID` (`ZTHabitat_getShowInfoID.c`): the real body's upper 16 bits are
    /// leftover from the `ZTShowInfo` pointer's own high half (a partial-register decompiler artifact,
    /// not real dataflow - every real caller (`_setShowTimes.c` et al.) immediately truncates the result
    /// to `ushort` before use), so only the low 16 bits - `ZTShowInfo`'s own `field_0x70` id, per
    /// `ztshowinfo.rs`'s `set_show_info_id` - are reproduced here, zero-extended.
    pub fn get_show_info_id(&self) -> u16 {
        if self.zt_show_info_ptr == 0 {
            return 0;
        }
        get_from_memory::<u16>(self.zt_show_info_ptr + 0x70)
    }

    /// Ports `ZTHabitat::isShowStopped` (`ZTHabitat_isShowStopped.c`): `false` with no `ZTShowInfo`,
    /// otherwise delegates to the already-ported `ztshowinfo::is_stopped`.
    pub fn is_show_stopped(&self) -> bool {
        self.zt_show_info_ptr != 0 && ztshowinfo::is_stopped(self.zt_show_info_ptr)
    }

    /// Ports `ZTHabitat::getPopularity` (`ZTHabitat_getPopularity.c`/`.asm`, confirmed against the
    /// macOS decompile's same shape): `views / ticks_since_creation * popularity_scale_factor`, clamped
    /// to `[0, 100]` and truncated toward zero - `views` is `unknown_nt_time` (a running popularity-view
    /// accumulator, not really a timestamp despite its current field type; see
    /// `command_get_zt_habitats`'s own prior speculative use of this same field), `ticks_since_creation`
    /// is `ZTGameMgr::timeAgo(created_timestamp)`. The two float constants the real body clamps against
    /// (`DAT_00630d64`/`DAT_00630d5c`) aren't directly readable from the decompile, but the `.asm` itself
    /// pins both: `local_10[1] = 100.0` and `_DAT_00630d64` are the same underlying constant (one
    /// literal, one memory reference to it), and `local_8.dwLowDateTime = 0` reinterprets the same 4
    /// zeroed bytes as `0.0f` for the other clamp.
    ///
    /// **Truncated, not rounded**: the decompile's own `ROUND(*pFVar4)` pseudocode is misleading - the
    /// real `.asm` (`FLDCW` with `AH |= 0xc` before `FISTP`) temporarily forces the FPU's rounding-control
    /// bits to round-toward-zero before converting to int, i.e. a plain `(int)` truncation, not
    /// round-to-nearest. A prior version of this port used `.round()` here and failed
    /// `ZTHABITAT_GET_POPULARITY_LIVE` against a real, loaded zoo (`real=15, reimpl=16` - `.round()`
    /// rounding a `15.9x` result up while real vanilla truncates it down) until corrected to a plain `as
    /// i32` cast, which truncates the same way.
    ///
    /// The division and multiplication are done in `f64`, not `f32`: the real body computes both in one
    /// x87 FPU expression (`FDIVR` then `FMUL` over 80-bit extended-precision registers) with a single
    /// store to `float` only afterward - closer to `f64` intermediate precision than doing the division
    /// and multiplication as two independently-rounded `f32` operations.
    pub fn get_popularity(&self) -> i32 {
        let views = self.unknown_nt_time.to_raw() as f64;
        let ticks_since_creation = globals().ztgamemgr().time_ago(self.created_timestamp.to_raw());
        let scale = globals().zthabitatmgr().popularity_scale_factor;
        let raw = (views / ticks_since_creation as f64) * scale as f64;
        raw.clamp(0.0, 100.0) as i32
    }

    pub fn is_tank(&self) -> bool {
        self.vtable == Self::TANK_VTABLE_PTR
    }

    /// Ports `ZTHabitat::isTank` (vtable `+0x20`, `ZTHabitat_isTank.c`) base default: constant
    /// `false`. That slot's address (`0x004016d1`, `generated.rs`'s `standalone::VF_RETURN_FALSE`)
    /// is a 2-instruction `XOR AL,AL; RET` stub roughly 150 vtable slots across the binary point at
    /// (xref-confirmed; `BFEntity.md` alone already lists six), so any detour on it would run for
    /// all of those classes' own boolean predicates too - like [`Self::is_right_salinity`]'s shared
    /// base, this port never reads `self`, which keeps such a hook behavior-preserving.
    /// `ZTTankExhibit`'s override (`0x00401302`, `standalone::VF_RETURN_TRUE_0`) is likewise a
    /// constant stub and stays real vanilla; the battery's `ZTHABITAT_IS_TANK_LIVE` reaches it by
    /// dispatch to confirm [`Self::is_tank`]'s vtable-identity pointer check agrees with it.
    pub fn is_tank_base_default(&self) -> bool {
        false
    }

    pub fn is_show_tank(&self) -> bool {
        self.zt_show_info_ptr != 0
    }

    /// Ports `ZTHabitatMgr::doTankCheck` (`ZTHabitatMgr_doTankCheck.c`/`.asm`) - despite its
    /// `ZTHabitatMgr::` decompile namespace it's a real free `stdcall` helper taking only a bare
    /// `ZTHabitat*` and touching no manager state at all (confirmed directly against `generated.rs`'s
    /// own `DO_TANK_CHECK` signature, no `this`), the same misattribution pattern this file already
    /// documents for `highlightHabitat`/`replaceGate`/`clearStaffHabitat` - ported as a `ZTHabitat`
    /// method instead.
    ///
    /// For every entry in this habitat's own [`Self::boundary_tile_pairs`] (a fresh
    /// copy, matching real vanilla's own copy-before-iterate shape), resolves the fence connecting the pair in each direction
    /// ([`Self::tile_fence_in_direction`] + [`BFMAP_GET_DIRECTION_0`], only kept when a genuine
    /// fence-family member - [`RVA_FENCE_TYPE_CHECK_ARG`]), and requires **at least one side** to be a
    /// fence whose own `entity_type+0x193` byte is set ([`is_tank_wall`], preferring the "A to B" side
    /// when it qualifies, matching the decompile's own `if (fence_a != 0) ... else ...` branch order).
    /// The moment any pair fails that, this returns `false` immediately (matching real vanilla's own
    /// early-`break`); an empty pairs vector (e.g. a freshly-constructed single-tile habitat before its
    /// first `resize`) returns `true` by default, matching real vanilla's own `local_11 = true`
    /// initializer that the loop never gets a chance to flip - see [`ZTHabitatMgr::create_habitat`]'s
    /// own call site, which relies on exactly this default for a brand-new seed-tile habitat.
    pub fn do_tank_check(&self) -> bool {
        let pairs = self.boundary_tile_pairs();
        for (tile_a_ptr, tile_b_ptr) in pairs {
            // A boundary-tile-pair entry can hold a null tile pointer while the pairs are stale
            // (e.g. mid-bulldoze on a shared tank wall) - skipped defensively, same "dead in
            // practice, defend anyway" convention as `Self::get_outermost_tank`.
            if tile_a_ptr == 0 || tile_b_ptr == 0 {
                continue;
            }
            let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
            let tile_b = get_from_memory::<BFTile>(tile_b_ptr);

            let dir_ab = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
            let fence_a = if dir_ab != -1 {
                let f = Self::tile_fence_in_direction(&tile_a, dir_ab as u32);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let dir_ba = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_b_ptr as i32, tile_a_ptr as i32) };
            let fence_b = if dir_ba != -1 {
                let f = Self::tile_fence_in_direction(&tile_b, dir_ba as u32);
                if f != 0 && unsafe { entity_type_matches(f, RVA_FENCE_TYPE_CHECK_ARG) } { f } else { 0 }
            } else {
                0
            };

            let ok = if fence_a != 0 { is_tank_wall(fence_a) } else { fence_b != 0 && is_tank_wall(fence_b) };
            if !ok {
                return false;
            }
        }
        true
    }

    /// Ports `ZTHabitat::isRightSalinity` (vtable `+0x28`) base default. A plain `ZTHabitat` has no
    /// water/salinity concept at all (only `ZTTankExhibit` does - see [`ZTTankExhibit::is_right_salinity`],
    /// that class's own override of this same slot at a separate address); the base implementation is a
    /// constant `true`. That slot's address (`0x00446995`, `generated.rs`'s `bfentity::VF_RETURN1_1`) is a
    /// shared `return true` stub other vtables also point at, so its detour runs for those classes too -
    /// this port never reads `self`, which keeps that harmless.
    pub fn is_right_salinity(&self, _animal_type: *const u32) -> bool {
        true
    }

    /// Ports `ZTHabitat::validatePositions` (vtable slot, `ZTHabitat_validatePositions.c`/`.asm`): walks
    /// the owned-tile list ([`TileListNode`]/[`walk_tile_list`]), calling real vanilla
    /// `BFTile::validatePositions(tile, tree, false)` for each tile, then does the same for the
    /// gate-out tile if any. `tree` is `GLOBAL_ZTWorldMgr + 0x8` - real vanilla's own guard on it
    /// (`&GLOBAL_ZTWorldMgr->field_0x8 != 0`) is dead in practice (would require
    /// `GLOBAL_ZTWorldMgr == -8`), so this checks `GLOBAL_ZTWorldMgr` itself instead: equivalent for
    /// every real value, and additionally guards the one case real vanilla's own check cannot.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn validate_positions(&self) {
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }
        let tree = (world + 0x8) as *const u32;

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            unsafe { BFTILE_VALIDATE_POSITIONS.original()(tile as *const u32, tree, false) };
        }

        let gate_tile_ptr = self.gate_tile_out_ptr();
        if gate_tile_ptr != 0 {
            unsafe { BFTILE_VALIDATE_POSITIONS.original()(gate_tile_ptr as *const u32, tree, false) };
        }
    }

    /// Ports `ZTHabitat::removeHabitatTiles` (vtable slot, `ZTHabitat_removeHabitatTiles.c`/`.asm`): for
    /// every owned tile whose ownership-grid cell still points back at `self`, clears that cell and sets
    /// the tile's own `+0x85` bit `0x1` flag; then splices every list node onto the shared small-object
    /// freelist ([`TILE_LIST_NODE_FREELIST_HEAD_RVA`]) and resets the sentinel back to its empty
    /// (self-referencing) state - the exact same two-pass shape as real vanilla's own body. Frees only
    /// the `0x10`-byte [`TileListNode`]s themselves, never the `BFTile`s they point at (those are
    /// permanent map tiles owned by `GLOBAL_ZTWorldMgr`, not by this list) - so there is no cross-
    /// allocator hazard here (see `CLAUDE.md`'s own warning on this): every address this function frees
    /// was itself carved from this exact freelist bucket by real vanilla's own `cls_0x40143b`/bump
    /// allocator, and nothing here is ever `Box`-allocated.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - reads/writes the real ownership grid and mutates real vanilla's
    /// shared node pool in place.
    pub fn remove_habitat_tiles(&self) {
        let habitat_mgr = globals().zthabitatmgr();
        let self_addr = self as *const Self as u32;
        let sentinel = self.owned_tiles_ptr;

        for node in walk_tile_list(sentinel) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if tile == 0 {
                continue;
            }
            let x: i32 = get_from_memory(tile + 0x34);
            let y: i32 = get_from_memory(tile + 0x38);
            if let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(x, y) {
                let owner: u32 = get_from_memory(cell_addr);
                if owner == self_addr {
                    save_to_memory(cell_addr, 0u32);
                    let flags: u8 = get_from_memory(tile + 0x85);
                    save_to_memory(tile + 0x85, flags | 1);
                }
            }
        }

        let freelist_head_addr = get_module_base("zoo.exe") as u32 + TILE_LIST_NODE_FREELIST_HEAD_RVA;
        for node in walk_tile_list(sentinel) {
            let old_head: u32 = get_from_memory(freelist_head_addr);
            save_to_memory(node, old_head);
            save_to_memory(freelist_head_addr, node);
        }

        save_to_memory(sentinel, sentinel);
        save_to_memory(sentinel + 4, sentinel);
    }

    /// Ports `ZTHabitat::getTilesCopy` (`ZTHabitat_getTilesCopy.c`/`.asm`, `generated.rs`'s
    /// `GET_TILES_COPY`): default-constructs a fresh empty `list<uint>` into the caller's out-param -
    /// vanilla's own inline sequence `*out = 0; PoolAlloc::allocate(0xc); self-ref next/prev;
    /// *out = sentinel` (the same bucket-1 node pool [`TileListNode`] documents) - then one real
    /// vanilla `msvc_std::list<uint>::insert_range` of `[owned_tiles begin, end)` into it. The
    /// out-param is a `list`, not the master plan's glossed `vector` (both the Windows and macOS
    /// decompiles agree). Every node the copy contains is vanilla-allocated; this frees nothing - the
    /// caller (`ZTViewingArea::createOA`, the only call site) destroys the copy through vanilla's own
    /// list erase, so the cross-allocator rule holds trivially.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn get_tiles_copy(&self, out_list_ptr: u32) -> u32 {
        save_to_memory::<u32>(out_list_ptr, 0);
        let sentinel = unsafe { POOLALLOC_ALLOCATE.original()(0xc) } as u32;
        save_to_memory(sentinel, sentinel);
        save_to_memory(sentinel + 4, sentinel);
        save_to_memory(out_list_ptr, sentinel);

        let src_sentinel = self.owned_tiles_ptr;
        let src_head: u32 = get_from_memory(src_sentinel);
        let where_node: u32 = get_from_memory(out_list_ptr); // out begin() == the fresh sentinel
        unsafe {
            MSVC_LIST_UINT_INSERT_RANGE.original()(
                out_list_ptr as *const std::ffi::c_void,
                where_node as *const i32,
                src_head as *const i32,
                src_sentinel as *const i32,
            );
        }
        out_list_ptr
    }

    /// Ports `ZTHabitat::isShowNeighbor` (`ZTHabitat_isShowNeighbor.c`/`.asm`, `generated.rs`'s
    /// `IS_SHOW_NEIGHBOR` at `0x005a3585`): whether `neighbor_ptr` is a member of this habitat's
    /// show-neighbor set - a pure, read-only probe of the `std::set<ZTHabitat*>`
    /// (`less<ZTHabitat*>`) rooted at [`Self::show_neighbors_head`], answered when a show goal
    /// completes to decide whether the habitat being arrived at / left is show-linked.
    ///
    /// Real vanilla is the MSVC `_Tree::find` shape: a lower_bound descent from the head's
    /// `_Parent` (`+0x4`, the root) - descending `_Right` (`+0xc`) while `node->_Value` (`+0x10`)
    /// is unsigned-less than the key, else recording the node and descending `_Left` (`+0x8`) -
    /// then confirming the surviving candidate with `candidate != head && candidate->_Value <=
    /// key`. The loop guard is a plain null check, so an empty set (null root) exits immediately
    /// with the candidate still the head and the answer `false`; the `&&` short-circuit likewise
    /// keeps the head's own `+0x10` slot (never meaningful) unread. Node layout is the same one
    /// [`walk_neighbor_tree`] documents, and the tree itself stays un-ported - this only ever
    /// reads a shape real vanilla's own `addShowNeighbor`/`clearShowNeighbors` produced. The C
    /// decompiles mislabel `show_neighbors_head` (`this+0x14`) `zoo_entrance_y` - an OOAnalyzer
    /// type-propagation artifact, not a real field read. The macOS counterpart
    /// (`ZTHabitat_isShowNeighbor.c`) reaches the same answer through named
    /// `std::tree<ZTHabitat*>::find` on `this+0x14`, returning a branchless `found != end()`.
    ///
    /// Real vanilla's return is a low-byte-only bool (the C renders' `CONCAT31`/`& 0xffffff00`
    /// shapes pack garbage upper bytes around one real byte) - both callers,
    /// `ZTGoalGoToShow::complete` (Win call site `0x005a3897`) and
    /// `ZTGoalReturnFromShow::complete` (Win call site `0x005a375c`), consume it low byte only.
    /// The return is retyped to `bool` in the live Ghidra project, so the next `generated.rs`
    /// regeneration carries `-> bool` and [`crate::zthabitatmgr::hooks_zthabitatmgr`]'s temporary
    /// `as u32` detour widen drops with it (the same mechanical step `HAS_PORTAL_ANIMAL` already
    /// went through). The port returns a clean `bool`.
    pub fn is_show_neighbor(&self, neighbor_ptr: u32) -> bool {
        let head = self.show_neighbors_head;
        let mut candidate = head;
        let mut node: u32 = get_from_memory(head + 0x4); // head->_Parent = root
        while node != 0 {
            if get_from_memory::<u32>(node + 0x10) < neighbor_ptr {
                node = get_from_memory(node + 0xc); // _Right
            } else {
                candidate = node;
                node = get_from_memory(node + 0x8); // _Left
            }
        }
        candidate != head && get_from_memory::<u32>(candidate + 0x10) <= neighbor_ptr
    }

    /// Whether the habitat at `habitat_ptr` is a show tank: `isTank()` by real vtable dispatch (slot
    /// `+0x20`, which a subclass may override) and a non-null show-info pointer. The gate both
    /// `addShowNeighbor` and `removeShowNeighbor` apply to each side.
    fn is_show_tank_by_vtable(habitat_ptr: u32) -> bool {
        let is_tank = unsafe { call_vtable_slot_noargs_ret_bool(habitat_ptr, 0x20) };
        is_tank && get_from_memory::<u32>(habitat_ptr + 0x4) != 0
    }

    /// Ports `ZTHabitat::addAmphibiousNeighbor` (`generated.rs`'s `ADD_AMPHIBIOUS_NEIGHBOR`, `0x005078d3`;
    /// the macOS `ZTHabitat_addAmphibiousNeighbor.c` agrees): inserts `other_ptr` into the amphibious
    /// `std::set<ZTHabitat*>` ([`Self::amphibious_neighbors_head`], size at `+0xc`) when the two
    /// habitats' `isTank()` results differ. The return is true whenever that gate passes, whether or not
    /// the key was already present. Nodes come from `PoolAlloc` (see [`rb_set_insert`]), matching what
    /// vanilla's `clearAmphibiousNeighbors`/destructor free.
    pub fn add_amphibious_neighbor(&self, other_ptr: u32) -> bool {
        if other_ptr == 0 {
            return false;
        }
        let self_addr = self as *const Self as u32;
        let other_is_tank = unsafe { call_vtable_slot_noargs_ret_bool(other_ptr, 0x20) };
        let self_is_tank = unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) };
        if self_is_tank == other_is_tank {
            return false;
        }
        rb_set_insert(self_addr + 0x8, other_ptr);
        true
    }

    /// Ports `ZTHabitat::clearAmphibiousNeighbors` (`CLEAR_AMPHIBIOUS_NEIGHBORS`, `0x00417460`): calls
    /// real vanilla `removeAmphibiousNeighbor(neighbor, self)` on every member (un-ported - it edits the
    /// *neighbour's* set, not this one), then frees the whole tree.
    pub fn clear_amphibious_neighbors(&self) {
        let self_addr = self as *const Self as u32;
        let neighbors: Vec<u32> = walk_neighbor_tree(self.amphibious_neighbors_head).map(|node| get_from_memory(node + 0x10)).collect();
        for neighbor in neighbors {
            unsafe { ref_from_memory::<ZTHabitat>(neighbor) }.remove_amphibious_neighbor(self_addr);
        }
        rb_tree_clear(self_addr + 0x8, 0x14);
    }

    /// Ports `ZTHabitat::removeAmphibiousNeighbor` (`REMOVE_AMPHIBIOUS_NEIGHBOR`, `0x00507a90`): erases
    /// `other_ptr` from the amphibious set ([`rb_set_erase`]; vanilla's `equal_range` +
    /// `erase(first, last)`). Returns whether `other_ptr` is non-null, whether or not it was present.
    /// Vanilla's low byte is all the real callers read (the decompile packs garbage upper bytes).
    pub fn remove_amphibious_neighbor(&self, other_ptr: u32) -> bool {
        if other_ptr == 0 {
            return false;
        }
        rb_set_erase(self as *const Self as u32 + 0x8, other_ptr);
        true
    }

    /// Ports `ZTHabitat::isAmphibiousNeighbor` (`IS_AMPHIBIOUS_NEIGHBOR`, `0x00448c79`): whether
    /// `neighbor_ptr` is in the amphibious set - the same `lower_bound` + `key <= neighbor` shape as
    /// [`Self::is_show_neighbor`], over [`Self::amphibious_neighbors_head`].
    pub fn is_amphibious_neighbor(&self, neighbor_ptr: u32) -> bool {
        let head = self.amphibious_neighbors_head;
        let mut candidate = head;
        let mut node: u32 = get_from_memory(head + 0x4);
        while node != 0 {
            if get_from_memory::<u32>(node + 0x10) < neighbor_ptr {
                node = get_from_memory(node + 0xc);
            } else {
                candidate = node;
                node = get_from_memory(node + 0x8);
            }
        }
        candidate != head && get_from_memory::<u32>(candidate + 0x10) <= neighbor_ptr
    }

    /// Ports `ZTHabitat::getCloseOutsideTile` (`GET_CLOSE_OUTSIDE_TILE`, `0x00448cb7`). The Windows body
    /// differs from the macOS one (`ZTHabitat_getCloseOutsideTile.c`): it walks the tile list at
    /// `this+0x44` (sentinel pointer, `+0x8` payload per node, the shape [`walk_tile_list`] reads), looks
    /// up each tile's owning habitat through the habitat manager's tile table (`tile+0x34`/`+0x38` = x/y;
    /// a null tile counts as owner `0`), and keeps the tiles whose owner is not an amphibious neighbour.
    /// An empty candidate list returns null without touching the RNG; otherwise one LCG step picks the
    /// candidate at `(seed >> 0x10 & 0x7fff) % count` ([`Self::pick_random_candidate_tile`]).
    pub fn get_close_outside_tile(&self) -> u32 {
        let sentinel: u32 = get_from_memory(self as *const Self as u32 + 0x44);
        let habitat_mgr = globals().zthabitatmgr();
        let mut candidates: Vec<u32> = Vec::new();
        for node in walk_tile_list(sentinel) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            let owner = if tile == 0 {
                0
            } else {
                habitat_mgr.get_habitat_ptr(get_from_memory::<i32>(tile + 0x34), get_from_memory::<i32>(tile + 0x38))
            };
            if !self.is_amphibious_neighbor(owner) {
                candidates.push(tile);
            }
        }
        if candidates.is_empty() {
            return 0;
        }
        self.pick_random_candidate_tile(candidates)
    }

    /// Ports `ZTHabitat::getSurroundingAnimals` (`GET_SURROUNDING_ANIMALS`, `0x00446436`): builds into
    /// the caller's RVO vector `out_ptr` this habitat's own animals (`+0x6c`..`+0x70`, read raw - no
    /// recalculate), then, unless this habitat is a tank (vtable `+0x20`), every animal of each
    /// amphibious neighbour (in-order, via the neighbour's own [`Self::get_animals`]) whose home habitat
    /// is this one. The result buffer is `PoolAlloc`-sized to exactly its length, as vanilla's
    /// `vector<int>::buy(size)` leaves it, so callers can release it with [`free_event_vector_buffer`].
    pub fn get_surrounding_animals(&self, out_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let mut animals: Vec<u32> = (self.all_animals_begin..self.all_animals_end).step_by(4).map(get_from_memory::<u32>).collect();
        if !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor = unsafe { ref_from_memory::<ZTHabitat>(get_from_memory::<u32>(node + 0x10)) };
                let vector_addr = neighbor.get_animals();
                let (begin, end): (u32, u32) = (get_from_memory(vector_addr), get_from_memory(vector_addr + 4));
                let neighbor_animals: Vec<u32> = (begin..end).step_by(4).map(get_from_memory::<u32>).collect();
                for animal in neighbor_animals {
                    if unsafe { animal_home_habitat(animal) } == self_addr {
                        animals.push(animal);
                    }
                }
            }
        }
        write_exact_u32_vector(out_ptr, &animals);
    }

    /// Ports `ZTHabitat::addShowNeighbor` (`ADD_SHOW_NEIGHBOR`, `0x005ab1e7`, `ZTHabitat_addShowNeighbor.c`):
    /// when exactly one of the two habitats is a show tank ([`Self::is_show_tank_by_vtable`]) and both
    /// have the same `isTank()` result, inserts `other_ptr` into the show-neighbor set
    /// ([`Self::show_neighbors_head`], size at `+0x18`). If `other_ptr` is the show tank, every animal of
    /// this habitat and of each amphibious neighbour that passes its vtable `+0x228` predicate is
    /// registered with it via [`Self::add_show_unit`]. Returns whether the insert gate passed.
    pub fn add_show_neighbor(&self, other_ptr: u32) -> bool {
        if other_ptr == 0 {
            return false;
        }
        let self_addr = self as *const Self as u32;
        if Self::is_show_tank_by_vtable(other_ptr) == Self::is_show_tank_by_vtable(self_addr) {
            return false;
        }
        if unsafe { call_vtable_slot_noargs_ret_bool(other_ptr, 0x20) } != unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } {
            return false;
        }
        rb_set_insert(self_addr + 0x14, other_ptr);
        if Self::is_show_tank_by_vtable(other_ptr) {
            let register_show_units = |habitat: &ZTHabitat| {
                let animals: Vec<u32> = habitat.get_all_animals(false).collect();
                for animal in animals {
                    if unsafe { call_entity_vtable_noargs(animal, 0x228) } {
                        unsafe { ref_from_memory::<ZTHabitat>(other_ptr) }.add_show_unit(animal);
                    }
                }
            };
            register_show_units(self);
            let neighbors: Vec<u32> = walk_neighbor_tree(self.amphibious_neighbors_head).map(|node| get_from_memory(node + 0x10)).collect();
            for neighbor in neighbors {
                register_show_units(unsafe { ref_from_memory::<ZTHabitat>(neighbor) });
            }
        }
        true
    }

    /// Ports `ZTHabitat::clearShowNeighbors` (`CLEAR_SHOW_NEIGHBORS`, `0x00458a88`): for every member
    /// calls [`Self::remove_show_neighbor`]`(member, self)` and real vanilla `removeShowPortal(member, self)` and
    /// `removeShowPortal(self, member)` (un-ported), then frees the show-neighbor set and the
    /// show-portal map ([`Self::show_portal_map_head`], size at `+0x24`, `0x18`-byte nodes).
    pub fn clear_show_neighbors(&self) {
        let self_addr = self as *const Self as u32;
        let neighbors: Vec<u32> = walk_neighbor_tree(self.show_neighbors_head).map(|node| get_from_memory(node + 0x10)).collect();
        for neighbor in neighbors {
            let neighbor_habitat = unsafe { ref_from_memory::<ZTHabitat>(neighbor) };
            neighbor_habitat.remove_show_neighbor(self_addr);
            neighbor_habitat.remove_show_portal(self_addr);
            self.remove_show_portal(neighbor);
        }
        rb_tree_clear(self_addr + 0x14, 0x14);
        rb_tree_clear(self_addr + 0x20, 0x18);
    }

    /// Ports `ZTHabitat::addShowUnit` (`ADD_SHOW_UNIT`, `0x00458610`): registers `unit_ptr` with this
    /// habitat's own `ZTShowInfo` ([`ztshowinfo::add_unit`]) when it is a show exhibit (`+0x4`). Otherwise
    /// it forwards to every show neighbour when there are any, or - for a non-tank habitat with no show
    /// neighbours - to every amphibious neighbour. Returns the last forwarded result (low byte only is
    /// meaningful, as with vanilla), `0` for a null unit or an empty set. Vanilla's discarded
    /// `isTank()` call on each forwarded neighbour is skipped.
    pub fn add_show_unit(&self, unit_ptr: u32) -> u32 {
        if unit_ptr == 0 {
            return 0;
        }
        if self.zt_show_info_ptr != 0 {
            return ztshowinfo::add_unit(self.zt_show_info_ptr, unit_ptr) as u32;
        }
        let self_addr = self as *const Self as u32;
        let forward = |head: u32| -> u32 {
            let neighbors: Vec<u32> = walk_neighbor_tree(head).map(|node| get_from_memory(node + 0x10)).collect();
            let mut result = 0;
            for neighbor in neighbors {
                if neighbor != 0 {
                    result = unsafe { ref_from_memory::<ZTHabitat>(neighbor) }.add_show_unit(unit_ptr);
                } else {
                    result = 0;
                }
            }
            result
        };
        if get_from_memory::<u32>(self_addr + 0x18) != 0 {
            forward(self.show_neighbors_head)
        } else if !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } && get_from_memory::<u32>(self_addr + 0xc) != 0 {
            forward(self.amphibious_neighbors_head)
        } else {
            0
        }
    }

    /// Ports `ZTHabitat::removeShowUnit` (`REMOVE_SHOW_UNIT`, `0x004586b9`): the mirror of
    /// [`Self::add_show_unit`]. A show exhibit removes the unit from its `ZTShowInfo` by the unit's
    /// entity-type id (entity type vtable `+0x20`) and its id at `unit+0x124`, and returns `1`; other
    /// habitats forward to the show neighbours, else (non-tank) the amphibious neighbours. Unlike
    /// `addShowUnit` there is no null-unit guard. Low byte of the result only is meaningful.
    pub fn remove_show_unit(&self, unit_ptr: u32) -> u8 {
        if self.zt_show_info_ptr != 0 {
            let entity_type_ptr = get_from_memory::<u32>(unit_ptr + 0x128);
            let unit_id = get_from_memory::<u32>(unit_ptr + 0x124);
            let unit_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
            ztshowinfo::remove_unit(self.zt_show_info_ptr, unit_type_id, unit_id);
            return 1;
        }
        let self_addr = self as *const Self as u32;
        let forward = |head: u32| -> u8 {
            let neighbors: Vec<u32> = walk_neighbor_tree(head).map(|node| get_from_memory(node + 0x10)).collect();
            let mut result = 0;
            for neighbor in neighbors {
                // A null neighbour reads its `+0x4` in vanilla; neighbour sets never hold one.
                result = unsafe { ref_from_memory::<ZTHabitat>(neighbor) }.remove_show_unit(unit_ptr);
            }
            result
        };
        if get_from_memory::<u32>(self_addr + 0x18) != 0 {
            forward(self.show_neighbors_head)
        } else if !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } && get_from_memory::<u32>(self_addr + 0xc) != 0 {
            forward(self.amphibious_neighbors_head)
        } else {
            0
        }
    }

    /// Ports `ZTHabitat::removeShowNeighbor` (`REMOVE_SHOW_NEIGHBOR`, `0x005aa75e`): erases `other_ptr`
    /// from the show-neighbour set ([`rb_set_erase`]) and, if `other_ptr` is a show tank
    /// ([`Self::is_show_tank_by_vtable`]), removes every animal of this habitat and of each amphibious
    /// neighbour that passes its vtable `+0x228` predicate from `other_ptr`'s show via
    /// [`Self::remove_show_unit`]. Returns whether `other_ptr` is non-null.
    pub fn remove_show_neighbor(&self, other_ptr: u32) -> bool {
        if other_ptr == 0 {
            return false;
        }
        rb_set_erase(self as *const Self as u32 + 0x14, other_ptr);
        if Self::is_show_tank_by_vtable(other_ptr) {
            let other = unsafe { ref_from_memory::<ZTHabitat>(other_ptr) };
            let remove_show_units = |habitat: &ZTHabitat| {
                let animals: Vec<u32> = habitat.get_all_animals(false).collect();
                for animal in animals {
                    if unsafe { call_entity_vtable_noargs(animal, 0x228) } {
                        other.remove_show_unit(animal);
                    }
                }
            };
            remove_show_units(self);
            let neighbors: Vec<u32> = walk_neighbor_tree(self.amphibious_neighbors_head).map(|node| get_from_memory(node + 0x10)).collect();
            for neighbor in neighbors {
                remove_show_units(unsafe { ref_from_memory::<ZTHabitat>(neighbor) });
            }
        }
        true
    }

    /// Ports the free function `findBestRating` (`FIND_BEST_RATING`, `0x00498a05`, `cdecl`): walks the
    /// `std::set<ZTHabitat*>` whose container (`{header*, count}`) is at `set_container_ptr` in order
    /// and returns the habitat with the best rating for `animal_ptr` - the first one, then any strictly
    /// better. A tank member rates through `getHabitatRating(animal, is_show_set)`; a land member
    /// through the weighted sum of five `get*Suitability(species)` terms.
    ///
    /// The land sum follows the `.asm`, not the decompile: terrain/object/foliage/rock partial sums are
    /// each rounded to `f32` (`FSTP`) after the add, but the last add (elevation) stays unrounded in
    /// `ST0` for the compare against the `f32` best-so-far. x87 uses an 80-bit mantissa; `f64` stands
    /// in for it here (each product of two `f32`s is exact in both).
    ///
    /// Vanilla dereferences a null `ZTAnimal` cast in the land branch; the port uses species `0` there.
    pub fn find_best_rating(animal_ptr: u32, set_container_ptr: u32, is_show_set: bool) -> u32 {
        let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
        let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
        let module_base = get_module_base("zoo.exe") as u32;
        let weight = |rva: u32| get_from_memory::<f32>(module_base + rva) as f64;

        let members: Vec<u32> =
            walk_neighbor_tree(get_from_memory::<u32>(set_container_ptr)).map(|node| get_from_memory(node + 0x10)).collect();
        let mut best_rating: f32 = -1.0;
        let mut best: u32 = 0;
        for habitat_ptr in members {
            let habitat = habitat_ptr as *const u32;
            let rating: f64 = if unsafe { call_vtable_slot_noargs_ret_bool(habitat_ptr, 0x20) } {
                unsafe { ref_from_memory::<ZTHabitat>(habitat) }.habitat_rating_unrounded(animal_ptr, is_show_set)
            } else {
                let species = if animal_type != 0 { unsafe { call_entity_vtable_u32_noargs(animal_type, 0x20) } } else { 0 } as i32;
                let land = unsafe { ref_from_memory::<ZTHabitat>(habitat) };
                let terrain = land.get_terrain_suitability(species) as f64;
                let object = land.get_object_suitability(species) as f64;
                let foliage = land.get_foliage_density_suitability(species) as f64;
                let rock = land.get_rock_density_suitability(species) as f64;
                let elevation = land.get_elevation_suitability(species) as f64;
                let sum = (terrain * weight(RVA_SUITABILITY_WEIGHT_TERRAIN)) as f32;
                let sum = (object * weight(RVA_SUITABILITY_WEIGHT_OBJECT) + sum as f64) as f32;
                let sum = (foliage * weight(RVA_SUITABILITY_WEIGHT_FOLIAGE) + sum as f64) as f32;
                let sum = (rock * weight(RVA_SUITABILITY_WEIGHT_ROCK) + sum as f64) as f32;
                elevation * weight(RVA_SUITABILITY_WEIGHT_ELEVATION) + sum as f64
            };
            if (best_rating as f64) < rating || best == 0 {
                best_rating = rating as f32;
                best = habitat_ptr;
            }
        }
        best
    }

    /// Ports `ZTHabitat::needsService` (`NEEDS_SERVICE`, `0x0049d202`, `thiscall(keeper, include_neighbors)`
    /// low-byte bool; the decompile's `ZTHabitatMgr` vtable call and `this_03` aliasing are Ghidra
    /// artefacts, the `.asm` is followed here). In order:
    /// 1. With `include_neighbors`: bail out `false` unless `isTank()` equals the keeper's vtable `+0x16c`
    ///    predicate.
    /// 2. Walk own animals (`+0x6c`..`+0x70`); with `include_neighbors`, an animal whose type passes the
    ///    `+0xcc` predicate is skipped in a tank. An animal the keeper `canService` marks "serviceable";
    ///    with `include_neighbors` a sickly one (and any non-show-tank, unflagged (`+0x395`) hungry-and-
    ///    foodless animal whose home habitat is this one) means `true`.
    /// 3. Any owned tile (`+0x40`, payload `+0x10`) the keeper's vtable `+0x324` accepts: `true`.
    /// 4. Tank with the keeper predicate, `+0x188 > 0` and `+0x1a8 < DAT_006390a4` (the water-purity
    ///    threshold): `true`. `needsShowKeeper`: `true`.
    /// 5. When some animal was serviceable: `true` if never serviced (`+0xec == 0`) or the AI manager's
    ///    `+0xec` clock minus ours exceeds the habitat manager's `+0x64` (unsigned).
    /// 6. With `include_neighbors`: any amphibious neighbour needing service (`include_neighbors = false`).
    ///
    /// A null animal-type cast in step 2's predicate is a vanilla null dereference; the port treats it as false.
    pub fn needs_service(&self, keeper_ptr: u32, include_neighbors: bool) -> bool {
        let self_addr = self as *const Self as u32;
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let is_tank = || unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) };
        let keeper_predicate = || unsafe { call_vtable_slot_noargs_ret_bool(keeper_ptr, 0x16c) };
        if include_neighbors {
            if is_tank() && !keeper_predicate() {
                return false;
            }
            if !is_tank() && keeper_predicate() {
                return false;
            }
        }

        let mut needs_attention = false;
        let mut any_serviceable = false;
        let animals: Vec<u32> = self.get_all_animals(false).collect();
        for animal_ptr in animals {
            if needs_attention {
                return true;
            }
            if include_neighbors {
                let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
                if animal_type != 0 && unsafe { call_vtable_slot_noargs_ret_bool(animal_type, 0xcc) } && is_tank() {
                    continue;
                }
            }
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) {
                continue;
            }
            any_serviceable = true;
            if include_neighbors && low_byte_bool(unsafe { IS_SICKLY.original()(animal_ptr as *const u32) }) {
                needs_attention = true;
            }
            if get_from_memory::<u8>(animal_ptr + 0x395) == 0
                && !Self::is_show_tank_by_vtable(self_addr)
                && low_byte_bool(unsafe { IS_HUNGRY_AND_FOODLESS.original()(animal_ptr as *const u32) })
                && unsafe { animal_home_habitat(animal_ptr) } == self_addr
            {
                needs_attention = true;
            }
        }
        if needs_attention {
            return true;
        }

        let tiles: Vec<u32> = walk_tile_list(self.owned_tiles_ptr).collect();
        for node in tiles {
            let tile_ptr: u32 = get_from_memory(node + 0x8);
            let tile_value: u32 = get_from_memory(tile_ptr + 0x10);
            if unsafe { call_vtable_slot_with_ptr_ret_bool(keeper_ptr, 0x324, tile_value) } {
                return true;
            }
        }

        let module_base = get_module_base("zoo.exe") as u32;
        if is_tank()
            && keeper_predicate()
            && get_from_memory::<i32>(self_addr + 0x188) > 0
            && get_from_memory::<i32>(self_addr + 0x1a8) < get_from_memory::<i32>(module_base + RVA_WATER_CLEAN_THRESHOLD)
        {
            return true;
        }
        if self.needs_show_keeper(keeper_ptr) {
            return true;
        }
        if any_serviceable {
            if self.time_last_serviced == 0 {
                return true;
            }
            let service_interval: u32 = get_from_memory(globals().zthabitatmgr_ptr() as u32 + 0x64);
            let ai_mgr_ptr = globals().ztaimgr_ptr() as u32;
            let elapsed = if ai_mgr_ptr == 0 { 0 } else { get_from_memory::<u32>(ai_mgr_ptr + 0xec).wrapping_sub(self.time_last_serviced) };
            if elapsed > service_interval {
                return true;
            }
        }
        if include_neighbors {
            let neighbors: Vec<u32> =
                walk_neighbor_tree(get_from_memory::<u32>(self_addr + 0x8)).map(|node| get_from_memory(node + 0x10)).collect();
            for neighbor_ptr in neighbors {
                if unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr as *const u32) }.needs_service(keeper_ptr, false) {
                    return true;
                }
            }
        }
        false
    }

    /// Ports `ZTHabitat::getRandomHungryAnimal` (`GET_RANDOM_HUNGRY_ANIMAL`, `0x0049f01a`,
    /// `thiscall(keeper, include_neighbors, home_habitat) -> ZTAnimal*`). Walks this habitat's own animals
    /// (`+0x6c`..`+0x70`); an animal qualifies when the keeper `canService` it, its `+0x124` id is not in
    /// the keeper's excluded-id vector (`keeper+0x270`..`+0x274`), `getAmountKeeperFood(species key at
    /// type+0x3c8, include_neighbors)` is `<= 0`, its `+0x395` flag is clear and its home habitat equals
    /// `home_habitat` (compared as given, including null). A qualifying animal whose `+0x2b0` is at least
    /// the animal type's `+0x20c` is returned immediately; otherwise it joins a candidate list. A non-empty
    /// list advances the shared RNG (`DAT_00638060`) exactly once and returns a uniformly picked candidate
    /// (`(state >> 16 & 0x7fff) % len`). With no candidates and `include_neighbors`, the first amphibious
    /// neighbour (`+0x8`) returning a non-null animal (searched with `include_neighbors = false` and
    /// `home_habitat = self`) wins; otherwise null. The candidate list is a Rust `Vec`: nothing outside
    /// sees vanilla's `PoolAlloc` vector.
    pub fn get_random_hungry_animal(&self, keeper_ptr: u32, include_neighbors: bool, home_habitat_ptr: u32) -> u32 {
        let self_addr = self as *const Self as u32;
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let excluded_begin: u32 = get_from_memory(keeper_ptr + 0x270);
        let excluded_end: u32 = get_from_memory(keeper_ptr + 0x274);
        let is_excluded = |animal_ptr: u32| {
            let id: u32 = get_from_memory(animal_ptr + 0x124);
            (excluded_begin..excluded_end).step_by(4).any(|slot| get_from_memory::<u32>(slot) == id)
        };

        let mut candidates: Vec<u32> = Vec::new();
        let animals: Vec<u32> = self.get_all_animals(false).collect();
        for animal_ptr in animals {
            if !low_byte_bool(unsafe { CAN_SERVICE.original()(animal_ptr as *const u32, keeper_ptr as *const u32) }) || is_excluded(animal_ptr) {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
            let species_key: u32 = if animal_type != 0 { get_from_memory(animal_type + 0x3c8) } else { 0 };
            if self.get_amount_keeper_food(species_key, include_neighbors) > 0 {
                continue;
            }
            if get_from_memory::<u8>(animal_ptr + 0x395) != 0 || unsafe { animal_home_habitat(animal_ptr) } != home_habitat_ptr {
                continue;
            }
            candidates.push(animal_ptr);
            if animal_type != 0 && get_from_memory::<i32>(animal_ptr + 0x2b0) >= get_from_memory::<i32>(animal_type + 0x20c) {
                return animal_ptr;
            }
        }

        if !candidates.is_empty() {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let state = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, state);
            return candidates[((state >> 16) & 0x7fff) as usize % candidates.len()];
        }
        if include_neighbors {
            let neighbors: Vec<u32> =
                walk_neighbor_tree(get_from_memory::<u32>(self_addr + 0x8)).map(|node| get_from_memory(node + 0x10)).collect();
            for neighbor_ptr in neighbors {
                let found = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr as *const u32) }.get_random_hungry_animal(keeper_ptr, false, self_addr);
                if found != 0 {
                    return found;
                }
            }
        }
        0
    }

    /// Ports `ZTHabitat::generateFaces` (`GENERATE_FACES`, `0x004d9953`, `thiscall(species_type, smile,
    /// other_habitat) -> bool`): asks every animal of `species_type` in this habitat's own list
    /// (`+0x6c`..`+0x70`) to show a smile/frown face (`ZTAnimal::FUN_004d9a2f`, [`ANIMAL_SHOW_FACE`]), and
    /// for a land habitat visited directly (`other_habitat == 0`) with a species whose type passes the
    /// vtable `+0xcc` predicate, recurses into every amphibious neighbour (`+0x8`) with `other_habitat =
    /// self` and the same `smile` (the decompile shows `false`; the `.asm` pushes the caller's `[ESP+0x1c]`, the smile argument). Returns whether any animal
    /// matched (the decompile drops the `1` store; the `.asm` sets the result byte on every match). An
    /// animal is skipped when its `+0x395` flag is set, or when `other_habitat != 0` and its home habitat
    /// is not `other_habitat`. Does nothing while the game-paused flag is set.
    pub fn generate_faces(&self, species_type_ptr: u32, smile: bool, other_habitat_ptr: u32) -> bool {
        let self_addr = self as *const Self as u32;
        if get_from_memory::<u8>(get_module_base("zoo.exe") as u32 + RVA_GAME_PAUSED_FLAG) != 0 {
            return false;
        }
        if self.characteristics_dirty != 0 {
            self.recalculate_characteristics();
        }
        let mut any_matched = false;
        let animals: Vec<u32> = self.get_all_animals(false).collect();
        for animal_ptr in animals {
            if get_from_memory::<u8>(animal_ptr + 0x395) != 0 {
                continue;
            }
            if other_habitat_ptr != 0 && unsafe { animal_home_habitat(animal_ptr) } != other_habitat_ptr {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
            let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
            if animal_type == species_type_ptr {
                any_matched = true;
                unsafe { ANIMAL_SHOW_FACE.hooked()(animal_ptr as *const u32, smile) };
            }
        }
        if !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) }
            && other_habitat_ptr == 0
            && unsafe { call_vtable_slot_noargs_ret_bool(species_type_ptr, 0xcc) }
        {
            let neighbors: Vec<u32> =
                walk_neighbor_tree(get_from_memory::<u32>(self_addr + 0x8)).map(|node| get_from_memory(node + 0x10)).collect();
            for neighbor_ptr in neighbors {
                any_matched |= unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr as *const u32) }.generate_faces(species_type_ptr, smile, self_addr);
            }
        }
        any_matched
    }

    /// Ports `ZTHabitat::getHabitatRating` (`GET_HABITAT_RATING`, `0x00415dd7`, `thiscall(animal, bool)`
    /// returning `f32` in `ST0`). Vanilla's body is split across four fragments; the branches are:
    /// - no animal / no entity type: the `DAT_00630d5c` constant;
    /// - non-amphibious animal: show tank -> show-tank weighted sum, other tank -> plain-tank sum, else land;
    /// - amphibious animal: show tank -> show-tank sum, minus the penalty unless the show-neighbour
    ///   count (`+0x18`) is non-zero; other tank -> with `include_neighbors` and a non-empty amphibious
    ///   set (`+0xc`) the max of the neighbours' ratings (`include_neighbors = false`), else the plain-tank
    ///   sum minus the penalty; non-tank -> land.
    ///
    /// Vanilla dereferences a null `ZTAnimal` cast for the amphibious predicate; the port treats it as false.
    pub fn get_habitat_rating(&self, animal_ptr: u32, include_neighbors: bool) -> f32 {
        self.habitat_rating_unrounded(animal_ptr, include_neighbors) as f32
    }

    /// [`Self::get_habitat_rating`] before the final `f32` rounding: the value vanilla leaves in `ST0`
    /// (callers such as `findBestRating` compare it unrounded). Partial sums follow [`x87_weighted_sum`].
    fn habitat_rating_unrounded(&self, animal_ptr: u32, include_neighbors: bool) -> f64 {
        let self_addr = self as *const Self as u32;
        let module_base = get_module_base("zoo.exe") as u32;
        let weight = |rva: u32| get_from_memory::<f32>(module_base + rva) as f64;

        if animal_ptr == 0 {
            return weight(RVA_RATING_NO_ANIMAL);
        }
        let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
        if entity_type_ptr == 0 {
            return weight(RVA_RATING_NO_ANIMAL);
        }
        let species = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) } as i32;
        let is_amphibious_animal = unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) }
            && unsafe { call_vtable_slot_noargs_ret_bool(entity_type_ptr, 0xcc) };

        let land_sum = || {
            let terms = [
                (self.get_terrain_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_TERRAIN)),
                (self.get_object_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_OBJECT)),
                (self.get_foliage_density_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_FOLIAGE)),
                (self.get_rock_density_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_ROCK)),
                (self.get_elevation_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_ELEVATION)),
                (self.get_shelter_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_SHELTER)),
                (self.get_toy_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_LAND_TOY)),
            ];
            x87_weighted_sum(&terms)
        };
        let tank_sum = |weights: &TankRatingWeights| {
            let terms = [
                (self.get_tank_depth_suitability(species) as f64, weight(weights.depth)),
                (self.get_tank_cleanliness_suitability(species) as f64, weight(weights.cleanliness)),
                (self.get_tank_salinity_suitability(species) as f64, weight(weights.salinity)),
                (self.get_object_suitability(species) as f64, weight(weights.object)),
                (self.get_foliage_density_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_FOLIAGE)),
                (self.get_rock_density_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_ROCK)),
                (self.get_elevation_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_ELEVATION)),
                (self.get_shelter_suitability(species) as f64, weight(RVA_SUITABILITY_WEIGHT_SHELTER)),
                (self.get_toy_suitability(species) as f64, weight(weights.toy)),
            ];
            x87_weighted_sum(&terms)
        };
        let penalty = || weight(RVA_RATING_AMPHIBIOUS_TANK_PENALTY);

        if Self::is_show_tank_by_vtable(self_addr) {
            let rating = tank_sum(&SHOW_TANK_RATING_WEIGHTS);
            return if !is_amphibious_animal || get_from_memory::<u32>(self_addr + 0x18) != 0 { rating } else { ((rating as f32) - (penalty() as f32)) as f64 };
        }
        if !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } {
            return land_sum();
        }
        if !is_amphibious_animal {
            return tank_sum(&PLAIN_TANK_RATING_WEIGHTS);
        }
        if include_neighbors && get_from_memory::<u32>(self_addr + 0xc) != 0 {
            let neighbors: Vec<u32> =
                walk_neighbor_tree(get_from_memory::<u32>(self_addr + 0x8)).map(|node| get_from_memory(node + 0x10)).collect();
            let mut best = NEIGHBOR_RATING_SEED;
            for neighbor_ptr in neighbors {
                let rating = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr as *const u32) }.habitat_rating_unrounded(animal_ptr, false);
                if rating > best as f64 {
                    best = rating as f32;
                }
            }
            return best as f64;
        }
        ((tank_sum(&PLAIN_TANK_RATING_WEIGHTS) as f32) - (penalty() as f32)) as f64
    }

    /// Ports `ZTHabitat::getMostSuitableHabitat` (`GET_MOST_SUITABLE_HABITAT`, `0x004161f5`): for a tank
    /// and a non-null animal, the best of the amphibious neighbours (when the animal's type passes the
    /// entity-type vtable `+0xcc` predicate and the set is non-empty), else of the show neighbours (when
    /// this is a show tank with a non-empty set); otherwise `self`. Vanilla dereferences a null
    /// `ZTAnimal` cast for the predicate; the port treats it as false.
    pub fn get_most_suitable_habitat(&self, animal_ptr: u32) -> u32 {
        let self_addr = self as *const Self as u32;
        if animal_ptr == 0 || !unsafe { call_vtable_slot_noargs_ret_bool(self_addr, 0x20) } {
            return self_addr;
        }
        let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
        let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
        if animal_type != 0 && unsafe { call_vtable_slot_noargs_ret_bool(animal_type, 0xcc) } && get_from_memory::<u32>(self_addr + 0xc) != 0 {
            return Self::find_best_rating(animal_ptr, self_addr + 0x8, false);
        }
        if Self::is_show_tank_by_vtable(self_addr) && get_from_memory::<u32>(self_addr + 0x18) != 0 {
            return Self::find_best_rating(animal_ptr, self_addr + 0x14, true);
        }
        self_addr
    }

    /// Entity-type field a `ZTScenarioSimpleGoal` of kind `kind` (`goal+0x10`) compares against its
    /// target value (`goal+0x20`) in [`Self::scenario_goal_eval06`], per the `.asm`'s per-kind
    /// `CMP [type+off], [goal+0x20]`. Kinds outside `1..=6` carry no per-type filter.
    fn scenario_goal_type_field_offset(kind: i32) -> Option<u32> {
        match kind {
            1 => Some(0x1e4),
            2 => Some(0x1e8),
            3 => Some(0x1ec),
            4 => Some(0x1f4),
            5 => Some(0x1f8),
            6 => Some(0x1f0),
            _ => None,
        }
    }

    /// Ports `ZTScenarioSimpleGoal::eval06` (`EVAL06`, `0x0041da81`, `fastcall`, `this` in `ECX`; the
    /// `eval00` export is the same ICF-folded function). Counts the habitats whose animals' average
    /// habitat rating reaches `goal+0x18`, and returns `goal+0x18` when that count is at least `goal+0x1c`
    /// (and positive), else `0`.
    ///
    /// Per `exhibit_array` habitat: the animal list is the habitat's own animals followed by the animals
    /// of each member of the union of its amphibious (`+0x8`) and show (`+0x14`) neighbour sets, in
    /// ascending pointer order (vanilla copies the `+0x8` set and range-inserts the `+0x14` set into the
    /// copy, so a habitat in both contributes once). An animal counts when it has no home habitat or its
    /// home is this habitat, and its type passes the goal kind's field comparison
    /// ([`Self::scenario_goal_type_field_offset`]); each counted animal adds
    /// `ZTAnimal::getHabitatRating(animal, habitat)` to the sum, compared as `sum / counted` (signed
    /// integer division).
    ///
    /// Vanilla builds its temporaries (vector copy, set copy) with its own allocator; the port uses plain
    /// Rust collections because nothing outside the function sees them. Vanilla dereferences a null
    /// `ZTAnimalType` cast under a kind in `1..=6`; the port treats that animal as filtered out.
    pub fn scenario_goal_eval06(goal_ptr: u32) -> u32 {
        let goal_kind: i32 = get_from_memory(goal_ptr + 0x10);
        let rating_threshold: i32 = get_from_memory(goal_ptr + 0x18);
        let habitat_count_threshold: i32 = get_from_memory(goal_ptr + 0x1c);
        let goal_value: i32 = get_from_memory(goal_ptr + 0x20);
        let type_field_offset = Self::scenario_goal_type_field_offset(goal_kind);

        let mut qualifying_habitats = 0i32;
        let mut index = 0usize;
        loop {
            let exhibit_array = globals().zthabitatmgr().exhibit_array();
            if index >= exhibit_array.len() {
                break;
            }
            let habitat_ptr = exhibit_array.get_ptr(index);
            index += 1;
            let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };

            let mut animals: Vec<u32> = habitat.get_all_animals(false).collect();
            let neighbors: std::collections::BTreeSet<u32> = walk_neighbor_tree(habitat.amphibious_neighbors_head)
                .chain(walk_neighbor_tree(habitat.show_neighbors_head))
                .map(|node| get_from_memory::<u32>(node + 0x10))
                .collect();
            for neighbor in neighbors {
                let neighbor_animals: Vec<u32> = unsafe { ref_from_memory::<ZTHabitat>(neighbor) }.get_all_animals(false).collect();
                animals.extend(neighbor_animals);
            }

            let mut counted = 0i32;
            let mut rating_sum = 0i32;
            for &animal_ptr in &animals {
                let entity_type_ptr: u32 = get_from_memory(animal_ptr + 0x128);
                let animal_type = if entity_type_ptr != 0 && unsafe { type_check(entity_type_ptr, RVA_ANIMAL_TYPE_CHECK) } { entity_type_ptr } else { 0 };
                let home = unsafe { animal_home_habitat(animal_ptr) };
                if home != 0 && home != habitat_ptr {
                    continue;
                }
                if let Some(offset) = type_field_offset
                    && (animal_type == 0 || get_from_memory::<i32>(animal_type + offset) != goal_value)
                {
                    continue;
                }
                counted += 1;
                rating_sum += unsafe { openzt_detour::generated::ztanimal::GET_HABITAT_RATING.original()(animal_ptr as *const u32, habitat_ptr as *const u32) };
            }
            if counted > 0 && rating_threshold <= rating_sum / counted {
                qualifying_habitats += 1;
            }
        }

        if qualifying_habitats > 0 && habitat_count_threshold <= qualifying_habitats {
            rating_threshold as u32
        } else {
            0
        }
    }

    /// Whether `fence_ptr` is a live world entity (`BFWorldMgr::verifyEntity`, `0x00443ffa`).
    fn is_live_world_entity(fence_ptr: u32) -> bool {
        let world_ptr = globals().ztworldmgr_ptr() as *const u32;
        unsafe { BFWORLDMGR_VERIFY_ENTITY_0.original()(world_ptr, fence_ptr as *const u32) != 0 }
    }

    /// Ports `ZTHabitat::addShowPortal` (`ADD_SHOW_PORTAL`, `0x005ab354`, `ZTHabitat_addShowPortal.c`):
    /// registers the fence between `tile_a_ptr` (this habitat's side) and `tile_b_ptr` as this habitat's
    /// show portal towards the habitat that owns `tile_b_ptr`, in the `+0x20` `map<ZTHabitat*, ZTFence*>`.
    ///
    /// Bails (returns false) unless `tile_b` has an owner and each tile has exactly one fence among its
    /// four slots, the two tiles are adjacent ([`BFMAP_GET_DIRECTION_0`] != -1), `tile_a`'s fence in that
    /// direction and `tile_b`'s in the opposite one (`(dir - 4) & 7`, each slot `dir / 2`) are both
    /// fence-family members, and neither fence's type name starts with `'g'` (a gate). A fence already
    /// stored for that owner is told to drop its portal state (vtable `+0x138(0)`, when still a live world
    /// entity) before `tile_a`'s fence is told to take it (`+0x138(1)`) and stored.
    pub fn add_show_portal(&self, tile_a_ptr: u32, tile_b_ptr: u32) -> bool {
        if tile_b_ptr == 0 {
            return false;
        }
        let tile_b = get_from_memory::<BFTile>(tile_b_ptr);
        let owner = globals().zthabitatmgr().get_habitat_ptr(tile_b.pos.x, tile_b.pos.y);
        if owner == 0 {
            return false;
        }
        let tile_a = get_from_memory::<BFTile>(tile_a_ptr);
        let fence_count = |tile: &BFTile| [tile.north_fence, tile.east_fence, tile.south_fence, tile.west_fence].iter().filter(|&&f| f != 0).count();
        if fence_count(&tile_a) != 1 || fence_count(&tile_b) != 1 {
            return false;
        }
        let direction = unsafe { BFMAP_GET_DIRECTION_0.original()(tile_a_ptr as i32, tile_b_ptr as i32) };
        if direction == -1 {
            return false;
        }
        let fence_family_member = |fence: u32| if fence != 0 && unsafe { entity_type_matches(fence, RVA_FENCE_TYPE_CHECK_ARG) } { fence } else { 0 };
        let fence_a = fence_family_member(Self::fence_slot_by_index(&tile_a, direction / 2));
        let fence_b = fence_family_member(Self::fence_slot_by_index(&tile_b, ((direction - 4) & 7) / 2));
        if fence_a == 0 || fence_b == 0 {
            return false;
        }
        let is_gate = |fence: u32| get_from_memory::<u8>(get_from_memory::<u32>(get_from_memory::<u32>(fence + 0x128) + 0xa4)) == b'g';
        if is_gate(fence_a) || is_gate(fence_b) {
            return false;
        }

        let map_addr = self as *const Self as u32 + 0x20;
        if let Some(existing) = rb_find(map_addr, owner) {
            let previous_fence: u32 = get_from_memory(existing + 0x14);
            if Self::is_live_world_entity(previous_fence) {
                unsafe { call_vtable_slot_with_ptr(previous_fence, 0x138, 0) };
            }
        }
        unsafe { call_vtable_slot_with_ptr(fence_a, 0x138, 1) };
        let node = rb_map_find_or_insert(map_addr, owner);
        save_to_memory(node + 0x14, fence_a);
        true
    }

    /// Ports `ZTHabitat::removeShowPortal` (`REMOVE_SHOW_PORTAL`, `0x005aa4d9`): if `other_ptr` has a
    /// stored portal fence, tells it to drop its portal state (vtable `+0x138(0)`, when still a live world
    /// entity) and erases the entry. A no-op for an unknown habitat.
    pub fn remove_show_portal(&self, other_ptr: u32) {
        let map_addr = self as *const Self as u32 + 0x20;
        let Some(node) = rb_find(map_addr, other_ptr) else {
            return;
        };
        let fence: u32 = get_from_memory(node + 0x14);
        if fence != 0 && Self::is_live_world_entity(fence) {
            unsafe { call_vtable_slot_with_ptr(fence, 0x138, 0) };
        }
        rb_map_erase(map_addr, other_ptr);
    }

    /// Vtable address real vanilla's destructor resets `this` to before tearing down (the base
    /// `ZTHabitat` vtable, `0x00632100`).
    const BASE_VTABLE_PTR: u32 = 0x0063_2100;

    /// Ports `ZTHabitat::~ZTHabitat` (`generated.rs`'s `DESTRUCTOR_0`, `0x00458cab`, called by
    /// `DESTRUCTOR_1`'s scalar-deleting wrapper and by the `ZTTankExhibit` destructor), step for step in
    /// vanilla's order: reset the vtable; send maintenance-worker cleanup events; remove viewing areas
    /// and habitat tiles; delete every `Ambients` and empty the `+0x54`/`+0x60` vectors; clear the show
    /// exhibit state and amphibious neighbours; break the amphibious connection of every
    /// boundary tile-pair; clear the show neighbours; then free every container the constructor
    /// allocated, each back to the `PoolAlloc` size class vanilla uses.
    ///
    /// The container frees go through [`Self::free_pool_block`]/real vanilla's own container destructors
    /// (`~vector<T>`, `~list<uint>`, `~basic_string`, the species-suitability cache and `tree36::clear`),
    /// which are un-detoured, so every allocation stays on vanilla's allocator.
    pub fn destruct(&self) {
        let self_addr = self as *const Self as u32;
        #[cfg(feature = "reimplementation-tests")]
        DESTRUCT_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        write_live!(self, vtable, Self::BASE_VTABLE_PTR);

        self.send_maint_worker_cleanup_events();
        self.remove_viewing_areas();
        self.remove_habitat_tiles();

        let ambients_begin: u32 = get_from_memory(self_addr + 0x54);
        let ambients_end: u32 = get_from_memory(self_addr + 0x58);
        for entry in (ambients_begin..ambients_end).step_by(8) {
            let ambients_ptr: u32 = get_from_memory(entry + 4);
            if ambients_ptr != 0 {
                unsafe { ref_from_memory::<Ambients>(ambients_ptr) }.destruct();
                unsafe { OPERATOR_DELETE.original()(ambients_ptr) };
            }
        }
        save_to_memory(self_addr + 0x64, get_from_memory::<u32>(self_addr + 0x60));
        save_to_memory(self_addr + 0x58, ambients_begin);

        self.set_is_not_show_exhibit();
        self.clear_amphibious_neighbors();
        for (tile_a, tile_b) in support::take_boundary_tile_pairs(self_addr) {
            ZTHabitatMgr::break_amphibious_connection(tile_a, tile_b);
        }
        self.clear_show_neighbors();

        unsafe { MSVC_TREE36_CLEAR.original()((self_addr + 0x16c) as *const u32) };
        Self::free_pool_block(get_from_memory(self_addr + 0x16c), 0x24);
        support::remove_suitability_cache(self_addr + 0x148);
        unsafe {
            MSVC_BASIC_STRING_DTOR.original()((self_addr + 0x154) as *const std::ffi::c_void);
            SPECIES_SUITABILITY_CACHE_CLEAR.original()((self_addr + 0x148) as *const std::ffi::c_void);
            SPECIES_SUITABILITY_CACHE_DTOR.original()((self_addr + 0x148) as *const std::ffi::c_void);
            for vector_offset in [0x13c, 0x78, 0x6c, 0x60] {
                MSVC_VECTOR_T_4_DTOR.original()((self_addr + vector_offset) as *const u32);
            }
        }
        // `+0x54` (8-byte pairs) and `+0x48` (vanilla's boundary-pair vector, never allocated) free by capacity.
        for vector_offset in [0x54, 0x48] {
            let begin: u32 = get_from_memory(self_addr + vector_offset);
            let cap_end: u32 = get_from_memory(self_addr + vector_offset + 8);
            Self::free_pool_block(begin, cap_end.wrapping_sub(begin));
        }
        unsafe { MSVC_LIST_UINT_DTOR.original()((self_addr + 0x44) as *const std::ffi::c_void) };

        let owned_tiles_head = self.owned_tiles_ptr;
        let mut node: u32 = get_from_memory(owned_tiles_head);
        while node != owned_tiles_head {
            let next: u32 = get_from_memory(node);
            Self::free_pool_block(node, 0xc);
            node = next;
        }
        save_to_memory(owned_tiles_head, owned_tiles_head);
        save_to_memory(owned_tiles_head + 4, owned_tiles_head);
        Self::free_pool_block(owned_tiles_head, 0xc);

        unsafe { MSVC_VECTOR_T_4_DTOR.original()((self_addr + 0x34) as *const u32) };

        // The three `std::set`/`std::map` containers: every node was already freed by the clears
        // above, so only the header node (0x14/0x18 bytes, same size class) remains.
        for (container_offset, node_size) in [(0x20, 0x18), (0x14, 0x14), (0x8, 0x14)] {
            let head: u32 = get_from_memory(self_addr + container_offset);
            rb_erase_subtree(get_from_memory(head + 4), node_size);
            Self::free_pool_block(head, node_size);
        }
    }

    /// Returns `ptr` to `PoolAlloc`'s freelist for `byte_size` (or `operator_delete`s it above 0x80
    /// bytes); a null `ptr` is skipped - `PoolAlloc::deallocate(NULL, n)` writes through the null.
    fn free_pool_block(ptr: u32, byte_size: u32) {
        if ptr != 0 {
            unsafe { POOLALLOC_DEALLOCATE.original()(ptr as *const u32, byte_size) };
        }
    }

    /// Ports `ZTHabitat::getShowPortal` (`generated.rs`'s `GET_SHOW_PORTAL`, `0x0059e0a9`, confirmed
    /// against `private/resources/decompiles/ZTHabitat_getShowPortal.c`/`.asm` and independently
    /// corroborated by the macOS `ZTHabitat_getShowPortal.c`).
    ///
    /// **Tank branch** (`self.is_tank()` true, real vanilla's `virt_meth_0x4016d1_32` vtable dispatch):
    /// a lower_bound descent of [`Self::show_portal_map_head`], same shape as [`Self::is_show_neighbor`]'s
    /// descent but over the two-word `(ZTHabitat*, ZTFence*)` pair value at each node - `+0x10` is the
    /// key, `+0x14` the value. **This is a pure read, despite the plan's original sketch calling it a
    /// mutating find-or-insert**: the `.asm` only reaches `msvc_std::tree24::insert` (`0x005ab33a`'s
    /// fallthrough continuation) from inside the branch already gated on the manual descent finding an
    /// existing match (`aiStack_1c[0] != head && key < candidate->key` both false) - i.e. the insert path
    /// is only ever entered when the key is already present, so it always re-finds the same node instead
    /// of actually inserting a new one. The macOS `_getShowPortal.c`/`_addShowPortal.c`/`_removeShowPortal.c`
    /// trio confirms the same shape independently: `getShowPortal`'s own `find_or_insert` call is likewise
    /// only reached inside its own "already found" branch, while `addShowPortal` (which really does insert
    /// new entries) calls `find_or_insert` unconditionally at its tail. A miss falls straight through to
    /// `return 0` with no insertion. Never calls `.original()` - this is a same-file sibling read.
    ///
    /// **Non-tank branch**: recurses over every amphibious neighbor ([`walk_neighbor_tree`] over
    /// [`Self::amphibious_neighbors_head`]), short-circuiting on the first nonzero result - the real
    /// body's own `this->field_0xc != 0` guard is that set's `_Mysize` (confirmed via `.asm`: the generic
    /// `_Tree::insert` helper increments `handle+4` on every real insert), a pure optimization to skip an
    /// empty walk that [`walk_neighbor_tree`] already answers identically (an empty tree yields nothing),
    /// so it isn't separately modeled here.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`]; each neighbor visited must also be live (true for every
    /// `walk_neighbor_tree` entry, which reads real `ZTHabitat*` pointers directly out of the tree).
    pub fn get_show_portal(&self, other_ptr: u32) -> u32 {
        if self.is_tank() {
            let head = self.show_portal_map_head;
            let mut candidate = head;
            let mut node: u32 = get_from_memory(head + 0x4); // head->_Parent = root
            while node != 0 {
                if get_from_memory::<u32>(node + 0x10) < other_ptr {
                    node = get_from_memory(node + 0xc); // _Right
                } else {
                    candidate = node;
                    node = get_from_memory(node + 0x8); // _Left
                }
            }
            if candidate != head && get_from_memory::<u32>(candidate + 0x10) <= other_ptr {
                get_from_memory(candidate + 0x14)
            } else {
                0
            }
        } else {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
                let result = neighbor.get_show_portal(other_ptr);
                if result != 0 {
                    return result;
                }
            }
            0
        }
    }

    /// Ports `ZTHabitat::resetUnitAI` (vtable slot, `ZTHabitat_resetUnitAI.c`/`.asm`): for every owned
    /// tile, walks that tile's own occupant list (`BFTile::unit_list_ptr`, `+0x0` - the exact same
    /// [`TileListNode`] shape/pool as `owned_tiles_ptr`, one level more nested, confirmed directly
    /// against `.asm` not just the decompile - see that field's own doc comment), calling vtable slot
    /// `+0x100` on each occupant; then the same for the gate-out tile's own occupants, if any.
    ///
    /// Gated on the same `loadInProgress` flag [`RVA_APP_INIT_SUCCESS_BASE`]`+0x441` resolves elsewhere in
    /// this file (see that constant's own doc comment) - skips the whole body while a save is loading.
    ///
    /// The real body's `GLOBAL_ZTWorldMgr != -8` guard is, like [`Self::validate_positions`]'s own
    /// identical guard, dead in practice - checked here as `GLOBAL_ZTWorldMgr != 0` instead (equivalent
    /// for every real value, and additionally guards the one case the real check cannot).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn reset_unit_ai(&self) {
        let base = get_module_base("zoo.exe") as u32;
        let gate_flag: u8 = get_from_memory(base + RVA_APP_INIT_SUCCESS_BASE + 0x441);
        if gate_flag != 0 {
            return;
        }

        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }

        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile = get_from_memory::<TileListNode>(node).payload;
            if tile != 0 {
                reset_unit_ai_for_tile_occupants(tile);
            }
        }

        let gate_tile_ptr = self.gate_tile_out_ptr();
        if gate_tile_ptr != 0 {
            reset_unit_ai_for_tile_occupants(gate_tile_ptr);
        }
    }

    /// Ports `ZTHabitat::addHabitatTiles` (vtable slot) plus its two real workers,
    /// `addSeedsOnStack`/`addContiguousSpan` (`ZTHabitat_addSeedsOnStack.c`/`_addContiguousSpan.c`/
    /// `.asm`): a scanline flood-fill from `seed_tile_ptr` that claims ([`claim_tile`]) every tile
    /// reachable without crossing a wall ([`is_wall`]) - [`add_contiguous_span`] extends a horizontal
    /// (West/East) run from the seed, then for every tile in that run this checks the North/South
    /// neighbours ([`enqueue_if_claimable`]) and queues any newly-reachable tile for the same treatment,
    /// repeating until nothing's left to visit.
    ///
    /// The real function's third parameter is nominally "the other habitat", but the only real call site
    /// (`ZTHabitat::resize`: `(*this->vftptr_0x0->addHabitatTiles)(this, seed_tile, this)`) always passes
    /// `this` again - so this port only takes the seed tile and always targets `self`, rather than
    /// reproducing a parameter that's never actually different from `self` in practice. The detour below
    /// still matches the real 3-argument vtable signature and simply ignores the third.
    ///
    /// Uses a plain `Vec<u32>` as the flood-fill's scratch worklist rather than touching real vanilla's
    /// own scratch `std::deque` global (`DAT_0063b710` family) - that deque is purely transient within
    /// this call and visit order doesn't affect the final claimed set, so there's nothing to keep in sync
    /// with vanilla's own copy.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - mutates the real ownership grid and vanilla's own shared node pool
    /// in place (via [`insert_tile_list_node`]'s call-through to real vanilla's own list-insert, matching
    /// [`Self::remove_habitat_tiles`]'s own allocator-consistency reasoning).
    pub fn add_habitat_tiles(&self, seed_tile_ptr: u32) {
        if seed_tile_ptr == 0 {
            return;
        }
        let target = self as *const Self as u32;
        let habitat_mgr = globals().zthabitatmgr();
        let world = globals().ztworldmgr();
        let sentinel = self.owned_tiles_ptr;

        let mut worklist: Vec<u32> = vec![seed_tile_ptr];
        while let Some(tile_ptr) = worklist.pop() {
            let x: i32 = get_from_memory(tile_ptr + 0x34);
            let y: i32 = get_from_memory(tile_ptr + 0x38);
            if habitat_mgr.get_habitat_ptr(x, y) == target {
                continue;
            }

            let (low, high) = add_contiguous_span(world, habitat_mgr, sentinel, target, tile_ptr);

            let mut cursor = low;
            loop {
                enqueue_if_claimable(world, habitat_mgr, target, cursor, Direction::North, &mut worklist);
                enqueue_if_claimable(world, habitat_mgr, target, cursor, Direction::South, &mut worklist);
                if cursor == high {
                    break;
                }
                let next = get_neighbour_ptr(world, cursor, Direction::East);
                if next == 0 {
                    break;
                }
                cursor = next;
            }
        }
    }

    /// Ports `ZTHabitat::listen` (vtable `+0x10`, `ZTHabitat_listen.c`): drains the event list built by
    /// `getEvents` (vtable `+0x8`, still real vanilla - not yet ported) and releases its buffer back to
    /// wherever real vanilla's own allocator would - see [`free_event_vector_buffer`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn listen(&self) {
        let mut events = VanillaEventVector::rvo_target();
        unsafe { GET_EVENTS.original()(self as *const Self as *const u32, events.as_ptr()) };
        if events.begin != 0 {
            free_event_vector_buffer(events.begin, events.cap_end - events.begin);
        }
    }

    /// Ports `ZTHabitat::setIsShowExhibit` (vtable `+0x2c`, `ZTHabitat_setIsShowExhibit.c`): allocates
    /// and constructs a real `ZTShowInfo` (`ztshowinfo::CONSTRUCTOR_1`), registers it with
    /// `GLOBAL_ZTShowMgr`. `REGISTER_SHOW`/`UNREGISTER_SHOW` are already detoured by `ztshowmgr.rs`
    /// (Stage 9), so this calls `.hooked()` rather than `.original()` - the correct call-through for an
    /// external caller of an address it doesn't itself own (see `ztshow.rs`'s own `GET_NUM_UNITS.hooked()`
    /// precedent). On registration failure, tears the just-built `ZTShowInfo` back down via its own
    /// scalar-deleting-destructor slot and leaves `zt_show_info_ptr` null, matching vanilla exactly.
    ///
    /// Also constructs the two configured ambient sounds (`[sounds] startSound`/`endSound`) and acquires
    /// each through `GLOBAL_DX8SndMgr` - see [`construct_and_acquire_sound`]'s doc comment for why both
    /// are treated as `SNDSound`. Gated on [`looks_like_configured_sound_name`] rather than real
    /// vanilla's own bare `DAT_... != 0` check - see that function's own doc comment for why.
    ///
    /// Must only be called on a live `ZTHabitat` reference (own address passed to real vanilla calls),
    /// same precondition as [`Self::get_attractiveness`].
    pub fn set_is_show_exhibit(&self) {
        if self.zt_show_info_ptr != 0 {
            return;
        }

        let alloc = unsafe { OPERATOR_NEW.original()(0xa8) } as u32;
        let show_info = if alloc == 0 { 0 } else { unsafe { ZTSHOWINFO_CONSTRUCTOR.original()(alloc as *const u32) as u32 } };
        write_live!(self, zt_show_info_ptr, show_info);

        let zt_show_mgr = globals().ztshowmgr_ptr() as *const u32;
        if zt_show_mgr.is_null() || show_info == 0 {
            return;
        }

        let registered = unsafe { REGISTER_SHOW.hooked()(zt_show_mgr, show_info as *const u32, true) };
        if !registered {
            unsafe { ZTSHOWINFO_DESTRUCTOR.original()(show_info as *const u32, 1) };
            write_live!(self, zt_show_info_ptr, 0u32);
            return;
        }
        save_to_memory(show_info + 0xa0, self as *const Self as u32);

        let base = get_module_base("zoo.exe") as u32;
        let dx8_sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        let start_sound_name = base + START_SOUND_NAME_RVA;
        let end_sound_name = base + END_SOUND_NAME_RVA;

        if looks_like_configured_sound_name(start_sound_name) {
            write_live!(self, start_sound_ptr, construct_and_acquire_sound(dx8_sndmgr, start_sound_name));
        }
        if looks_like_configured_sound_name(end_sound_name) {
            write_live!(self, end_sound_ptr, construct_and_acquire_sound(dx8_sndmgr, end_sound_name));
        }
    }

    /// Ports `ZTHabitat::setIsNotShowExhibit` (vtable `+0x30`, `ZTHabitat_setIsNotShowExhibit.c`):
    /// unregisters and tears down the real `ZTShowInfo` `set_is_show_exhibit` built, clearing the
    /// showpanel UI's own selection first if it currently points at this habitat, then tears down both
    /// owned sounds via [`teardown_sound`].
    ///
    /// The `unregisterShow` call carries no `GLOBAL_ZTShowMgr` null check in the real decompile either -
    /// mirrored exactly rather than adding a guard vanilla itself doesn't have (same reasoning
    /// `ztshow.rs`'s own `RESOLVE_NEXT_SCHEDULED_SCRIPT_ID` doc comment gives for an identical unchecked
    /// vanilla read).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`].
    pub fn set_is_not_show_exhibit(&self) {
        if self.zt_show_info_ptr == 0 {
            return;
        }

        let base = get_module_base("zoo.exe") as u32;
        let currently_open_habitat: u32 = get_from_memory(base + SHOWPANEL_CURRENT_HABITAT_RVA);
        if currently_open_habitat == self as *const Self as u32 {
            unsafe { SET_EXHIBIT.original()(0) };
        }

        let show_id: u16 = get_from_memory(self.zt_show_info_ptr + 0x70);
        let zt_show_mgr = globals().ztshowmgr_ptr() as *const u32;
        unsafe { UNREGISTER_SHOW.hooked()(zt_show_mgr, show_id, std::ptr::null(), true) };

        unsafe { ZTSHOWINFO_DESTRUCTOR.original()(self.zt_show_info_ptr as *const u32, 1) };
        write_live!(self, zt_show_info_ptr, 0u32);

        teardown_sound(self.start_sound_ptr);
        write_live!(self, start_sound_ptr, 0u32);
        teardown_sound(self.end_sound_ptr);
        write_live!(self, end_sound_ptr, 0u32);
    }

    /// Ports `ZTHabitat::update` (vtable `+0x24`, `ZTHabitat_update.c`/`.asm`, confirmed against the
    /// macOS decompile's identical shape): plays every queued ambient sound
    /// (`ambients_begin`/`ambients_end`), advances the species-list/characteristics lazy-recalculate
    /// timers - rerolling each via the shared game RNG ([`lcg_next`]) and calling through to the
    /// still-un-ported real vanilla `reviseSpeciesList` and [`Self::recalculate_characteristics`] once
    /// its own threshold trips, the same dirty-flag/timer shape [`Self::get_attractiveness`] already relies on
    /// for `characteristics_dirty` - ticks every viewing area's own ambient state
    /// (`viewing_areas_begin`/`viewing_areas_end`), then calls this object's own ported
    /// [`Self::update_portals`] and finally its already-ported [`Self::listen`].
    ///
    /// `updatePortals` is a plain, non-virtual helper (not one of `ZTHabitat`'s 17 vtable slots),
    /// called directly as a same-file sibling rather than through `.original()`/`.hooked()` - its own
    /// address (`generated.rs`'s `UPDATE_PORTALS`) is itself detoured (see
    /// [`Self::update_portals`]'s own doc comment), so `.original()` here would re-enter that detour
    /// in release builds.
    ///
    /// `ZTTankExhibit` overrides this vtable slot with its own, separate address (`0x0049625f`) running
    /// the tank's water-level fill/drain tick after delegating back into this base port via
    /// `self.habitat.update(elapsed)` (`tank_exhibit.rs`, `generated.rs`'s `zttankexhibit::UPDATE`) -
    /// detouring only the base `ZTHabitat::update` address never intercepts a real tank's own tick, the
    /// same base-only-override pattern [`Self::is_right_salinity`] already relies on.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - `self`'s own address is passed straight into every real vanilla
    /// call-through above.
    pub fn update(&self, elapsed: u32) {
        let mut ambient_entry = self.ambients_begin;
        while ambient_entry != self.ambients_end {
            let ambient: u32 = get_from_memory(ambient_entry + 4);
            unsafe { ref_from_memory::<Ambients>(ambient) }.play(elapsed as i32, 0x50);
            ambient_entry += 8;
        }

        let species_list_timer = self.species_list_timer.wrapping_add(elapsed);
        write_live!(self, species_list_timer, species_list_timer);
        write_live!(self, characteristics_timer, self.characteristics_timer.wrapping_add(elapsed));

        if species_list_timer > 7999 {
            write_live!(self, species_list_dirty, 1u8);
        }
        if self.species_list_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            write_live!(self, species_list_timer, (rng >> 0x10 & 0x7fff) % 200);
            unsafe { REVISE_SPECIES_LIST.original()(self as *const Self as *const u32) };
        }

        if self.characteristics_timer > 6999 {
            write_live!(self, characteristics_dirty, 1u8);
        }
        if self.characteristics_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            write_live!(self, characteristics_timer, (rng >> 0x10 & 0x7fff) % 200);
            self.recalculate_characteristics();
        }

        let mut viewing_area_entry = self.viewing_areas_begin;
        while viewing_area_entry != self.viewing_areas_end {
            let viewing_area: u32 = get_from_memory(viewing_area_entry);
            unsafe { ZTVIEWINGAREA_UPDATE_AMBIENTS.original()(viewing_area as *const u32, elapsed as i32) };
            viewing_area_entry += 4;
        }

        self.update_portals();
        self.listen();
    }

    /// Ports `ZTHabitat::updatePortals` (`generated.rs`'s `UPDATE_PORTALS`, `0x0043578f`, confirmed
    /// against `private/resources/decompiles/ZTHabitat_updatePortals.c`/`.asm` and the macOS
    /// `ZTHabitat_updatePortals.c`, which agree exactly): builds the plan of every `+0x13c`
    /// portal-fence vtable dispatch a per-tick call would perform, without performing any of them.
    ///
    /// **Gate**: real vanilla dispatches vtable `+0x20` (`isTank`) then reads `this+0x4` - the port
    /// uses the already-established vtable-identity [`Self::is_tank`] check plus [`Self::is_show_tank`]
    /// (`zt_show_info_ptr != 0`), matching the macOS decompile's own combined-gate name `isShowTank`.
    /// Both poles of `is_tank`'s underlying vtable slots are constant-return stubs shared by ~150
    /// other classes' own predicates (never detoured), and the gate's equivalence is pinned live by
    /// `ZTHABITAT_IS_TANK_LIVE` and the multi-reimpl census leg.
    ///
    /// **Walk**: [`walk_neighbor_tree`] over [`Self::show_neighbors_head`] - the Windows `.c` render
    /// mislabels this field `zoo_entrance_y` (the same OOAnalyzer artifact the field's own doc comment
    /// documents); trust the `.asm`-confirmed `+0x14` offset already established there.
    ///
    /// **Per neighbor pair**: `has_portal` is [`Self::has_portal_animal`]`(self, neighbor)`, or - only
    /// if that's false - `has_portal_animal(neighbor, self)` (short-circuit, exactly this order).
    /// `portal_a = self.get_show_portal(neighbor)`; if non-null, a dispatch entry is pushed for it with
    /// `play_sound = true`. `portal_b = neighbor.get_show_portal(self)`; if non-null, a dispatch entry
    /// is pushed for it with `play_sound = (portal_a == 0)` - the sound only plays on the second leg
    /// when the first leg found no portal, so the effect fires once per pair, not twice.
    ///
    /// **Dispatch**: a genuine vtable `+0x13c` dispatch on the returned `ZTFence*`
    /// ([`call_vtable_slot_with_u8_u8`]), never a fixed address - `ZTTankWall` overrides that slot
    /// with `ZTTankWall::setIsOpenPortal` (`0x0059ea94`) while the base `ZTFence` slot is a `NULLSUB`
    /// (`0x00401115`); a fixed-address call would skip the override the same way the tank-water-level
    /// vtable-dispatch bug did. Building the full plan before dispatching anything is
    /// behavior-identical to vanilla's interleaved order here: each dispatch only mutates its own
    /// target fence's portal-open/animation state, which none of the four per-pair reads above
    /// consult.
    ///
    /// Live-memory note: this only reads `self` and each `walk_neighbor_tree` neighbor through
    /// `&self` methods - no writes, no `&mut`.
    pub(crate) fn portal_dispatch_plan(&self) -> Vec<PortalDispatch> {
        let mut plan = Vec::new();
        if !self.is_tank() || !self.is_show_tank() {
            return plan;
        }
        let self_ptr = self as *const Self as u32;
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            let mut has_portal = self.has_portal_animal(neighbor_ptr);
            if !has_portal {
                has_portal = neighbor.has_portal_animal(self_ptr);
            }
            let portal_a = self.get_show_portal(neighbor_ptr);
            if portal_a != 0 {
                plan.push(PortalDispatch { fence_ptr: portal_a, is_open: has_portal, play_sound: true });
            }
            let portal_b = neighbor.get_show_portal(self_ptr);
            if portal_b != 0 {
                plan.push(PortalDispatch { fence_ptr: portal_b, is_open: has_portal, play_sound: portal_a == 0 });
            }
        }
        plan
    }

    /// Executes [`Self::portal_dispatch_plan`]'s dispatch sequence for real. Detoured directly (see
    /// `hooks_zthabitatmgr::update_portals`) rather than left un-ported and called via `.original()` -
    /// once hooked, `.original()`/`.hooked()` on this same address would re-enter the detour in
    /// release builds (a raw address cast there, not routed through any trampoline).
    pub fn update_portals(&self) {
        for dispatch in self.portal_dispatch_plan() {
            unsafe { call_vtable_slot_with_u8_u8(dispatch.fence_ptr, 0x13c, dispatch.is_open as u8, dispatch.play_sound as u8) };
        }
    }

    /// Ports `ZTHabitat::save` (vtable `+0x1c`, `ZTHabitat_save.c`/`.asm`, confirmed identical on both
    /// platforms modulo the macOS decompile's own endian-swap noise): the "seed tile" position (the
    /// first owned tile - `owned_tiles_ptr`'s sentinel own `next`, then that node's `payload`, read
    /// exactly as vanilla's own double-pointer-chase does, including its lack of a guard for an empty
    /// tile list - "dead in practice" the same way `ZTHabitatMgr::save`'s own unguarded entrance-tile
    /// read is, since a real habitat is never saved without at least one owned tile), `exhibit_name`'s
    /// length-then-bytes (skipped entirely, not just zero-length, if the name is implausibly long -
    /// `>= 0x1000` bytes - matching vanilla's own dead branch rather than adding a guard it lacks), the
    /// entrance tile's position (or `(-1, -1)` with no entrance), `entrance_rotation`, the six
    /// donation/upkeep running totals, three unknown dwords, `created_timestamp`/`unknown_nt_time` (8
    /// bytes each), `time_last_serviced`, then `is_tank()`/`is_show_tank()` as trailing bytes - delegating to
    /// [`Self::is_tank`] rather than the real vtable `+0x20` slot it actually calls, per this file's
    /// existing precedent (see that method's own doc comment). Finally, if `is_show_tank()`, calls
    /// through to real vanilla `ZTShowInfo::save` (not yet ported).
    ///
    /// Every field's success is ANDed together and every write happens regardless of an earlier one
    /// failing - matches the real body's own flat structure (no early-exit branches besides the
    /// oversized-name case), unlike [`ZTHabitatMgr::save`]'s own per-exhibit loop.
    pub fn save(&self, file: *const i8) -> bool {
        let head_node: u32 = get_from_memory(self.owned_tiles_ptr);
        let seed_tile_ptr: u32 = get_from_memory(head_node + 8);
        let seed_x: i32 = get_from_memory(seed_tile_ptr + 0x34);
        let seed_y: i32 = get_from_memory(seed_tile_ptr + 0x38);
        let mut ok = write_bytes_to_file(&seed_x, file);
        ok &= write_bytes_to_file(&seed_y, file);

        let (name_start, name_end, _) = self.exhibit_name.raw_parts();
        let name_len = name_end.wrapping_sub(name_start);
        if name_len < 0x1000 {
            ok &= write_bytes_to_file(&name_len, file);
            if name_len != 0 {
                ok &= write_raw_bytes(name_start, name_len, file);
            }
        } else {
            ok = false;
        }

        let (entrance_x, entrance_y): (i32, i32) = if self.entrance_tile_ptr == 0 {
            (-1, -1)
        } else {
            (get_from_memory(self.entrance_tile_ptr + 0x34), get_from_memory(self.entrance_tile_ptr + 0x38))
        };
        ok &= write_bytes_to_file(&entrance_x, file);
        ok &= write_bytes_to_file(&entrance_y, file);
        ok &= write_bytes_to_file(&self.entrance_rotation, file);
        ok &= write_bytes_to_file(&self.current_donations, file);
        ok &= write_bytes_to_file(&self.last_donations, file);
        ok &= write_bytes_to_file(&self.total_donations, file);
        ok &= write_bytes_to_file(&self.current_upkeep, file);
        ok &= write_bytes_to_file(&self.last_upkeep, file);
        ok &= write_bytes_to_file(&self.total_upkeep, file);
        ok &= write_bytes_to_file(&self.unknown_u32_2, file);
        ok &= write_bytes_to_file(&self.unknown_u32_3, file);
        ok &= write_bytes_to_file(&self.unknown_u32_4, file);
        ok &= write_bytes_to_file(&self.created_timestamp, file);
        ok &= write_bytes_to_file(&self.unknown_nt_time, file);
        ok &= write_bytes_to_file(&self.time_last_serviced, file);

        ok &= write_bytes_to_file(&(self.is_tank() as u8), file);
        let is_show_tank = self.is_show_tank();
        ok &= write_bytes_to_file(&(is_show_tank as u8), file);
        if is_show_tank {
            ok &= ztshowinfo::show_info_save(self.zt_show_info_ptr, file);
        }

        ok
    }

    /// Ports `ZTHabitat::hiliteAmphibiousNeighbors` (`ZTHabitat_hiliteAmphibiousNeighbors.c`): walks
    /// [`Self::amphibious_neighbors_head`] via [`walk_neighbor_tree`] and calls the already-ported
    /// [`Self::highlight`]/[`Self::unhighlight`] on each neighbor - `unhighlight()` when `hilite` is
    /// `false`, `highlight(true)` otherwise, matching the real decompile's own two branches exactly (not
    /// a symmetric `highlight(hilite)`/`highlight(!hilite)` pair - see those methods' own doc comments for
    /// why they're genuinely different operations, not each other's inverse). Purely a read of the tree
    /// real vanilla's own `addAmphibiousNeighbor`/`clearAmphibiousNeighbors` (left un-ported) produced -
    /// no allocation, no mutation of the tree itself.
    pub fn hilite_amphibious_neighbors(&self, hilite: bool) {
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            if hilite {
                neighbor.highlight(true);
            } else {
                neighbor.unhighlight();
            }
        }
    }

    /// Ports `ZTHabitat::hiliteShowNeighbors` (`ZTHabitat_hiliteShowNeighbors.c`) - identical shape to
    /// [`Self::hilite_amphibious_neighbors`], walking [`Self::show_neighbors_head`] instead.
    pub fn hilite_show_neighbors(&self, hilite: bool) {
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            let neighbor = unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) };
            if hilite {
                neighbor.highlight(true);
            } else {
                neighbor.unhighlight();
            }
        }
    }

    /// Appends `(tile_a, tile_b)` to this habitat's boundary tile-pairs (see [`Self::boundary_tile_pairs`]).
    pub(crate) fn push_boundary_tile_pair(&self, tile_a: u32, tile_b: u32) {
        support::push_boundary_tile_pair(self as *const Self as u32, tile_a, tile_b);
    }

    /// Copy of this habitat's boundary tile-pairs, built by [`Self::create_edge_pairs`]. Owned by Rust:
    /// vanilla's `+0x48`/`+0x4c`/`+0x50` vector stays empty. A copy, so callers may re-enter code that
    /// rebuilds the pairs mid-iteration.
    pub fn boundary_tile_pairs(&self) -> Vec<(u32, u32)> {
        support::boundary_tile_pairs(self as *const Self as u32)
    }

    /// Ports `ZTHabitat::createEdgePairs` (`ZTHabitat_createEdgePairs.c`/`.asm`, `generated.rs`'s
    /// `CREATE_EDGE_PAIRS`): rebuilds [`Self::boundary_tile_pairs`] from scratch (real
    /// vanilla's own `end = begin` reset, keeping the existing buffer rather than deallocating it), then
    /// walks the owned-tile list ([`walk_tile_list`]) and, for every owned tile and each of its 4 cardinal
    /// neighbours ([`get_neighbour_ptr`]), pushes `(owned_tile, neighbour_tile)` ([`Self::push_boundary_tile_pair`])
    /// whenever the neighbour isn't owned by `self` - including when it doesn't exist (real vanilla's own
    /// address, `0` when out of map bounds).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    ///
    /// Verified against real vanilla via a direct call (`ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE`).
    pub fn create_edge_pairs(&self) {
        let self_addr = self as *const Self as u32;
        support::clear_boundary_tile_pairs(self_addr);

        let world = globals().ztworldmgr();
        let habitat_mgr = globals().zthabitatmgr();
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            for direction in [Direction::North, Direction::East, Direction::South, Direction::West] {
                let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction);
                let owner_ptr = if neighbour_ptr == 0 {
                    0
                } else {
                    let neighbour = get_from_memory::<BFTile>(neighbour_ptr);
                    habitat_mgr.get_habitat_ptr(neighbour.pos.x, neighbour.pos.y)
                };
                if owner_ptr != self_addr {
                    self.push_boundary_tile_pair(tile_ptr, neighbour_ptr);
                }
            }
        }
    }

    /// Ports `ZTHabitat::recalculateViewingAreas` (`ZTHabitat_recalculateViewingAreas.c`, `generated.rs`'s
    /// `RECALCULATE_VIEWING_AREAS`): calls real vanilla `ZTViewingArea::recalculateCharacteristics`
    /// (call-through - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase) on every
    /// [`Self::viewing_areas_begin`]/`_end` entry.
    pub fn recalculate_viewing_areas(&self) {
        let mut cursor = self.viewing_areas_begin;
        while cursor != self.viewing_areas_end {
            let va_ptr: u32 = get_from_memory(cursor);
            unsafe { ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS.original()(va_ptr as *const u32) };
            cursor += 4;
        }
    }

    /// `ZTHabitat_resize.asm`'s own default-tile chase for a null argument: [`Self::owned_tiles_ptr`]'s
    /// sentinel own `next`, then that node's `BFTile*` payload - identical arithmetic to [`Self::save`]'s
    /// seed-tile read, including its lack of a guard for an empty list (a habitat being resized always
    /// has one; real vanilla would read the sentinel's stale payload slot otherwise).
    fn default_resize_tile_ptr(&self) -> u32 {
        let head_node: u32 = get_from_memory(self.owned_tiles_ptr);
        get_from_memory(head_node + 0x8)
    }

    /// Ports `ZTHabitat::resize` (`ZTHabitat_resize.c`/`.asm`, `generated.rs`'s `RESIZE`): tears the
    /// habitat's tile ownership down and rebuilds it around `tile_ptr`, then refreshes every derived
    /// structure. `removeHabitatTiles`/`addHabitatTiles` are called through the receiver's own **vtable**
    /// (`+0x3c` no-arg, then `+0x38` with `(tile_ptr, this)` - confirmed at the `.asm` level), so a
    /// `ZTTankExhibit` receiver reaches the tank's own overrides (`0x00487ae3`/`0x00487b19`) rather than
    /// the base ports, exactly as real vanilla's dispatch does; the same dispatch on a base habitat
    /// reaches this codebase's own detoured ports. `createEdgePairs`/`recalculateViewingAreas` are direct
    /// calls to the already-ported [`Self::create_edge_pairs`]/[`Self::recalculate_viewing_areas`];
    /// `createViewingAreas` stays a real-vanilla call-through (still un-ported - see `pad2a1`'s field doc
    /// comment). Sets [`Self::characteristics_dirty`] unconditionally (the `.asm`'s own bare
    /// `MOV byte ptr [ESI+0x2d], 1` - not the guarded neighbor walk [`Self::set_dirty_characteristics`]
    /// does). Finally, when the rebuilt owned-tile list is empty, destroys the habitat outright via
    /// `ZTHabitatMgr::removeHabitat` (`REMOVE_HABITAT_0`, un-ported, on the `GLOBAL_ZTHabitatMgr` global) -
    /// so this method is destructive on its own main path, the same "no synthetic-safe live input" class
    /// as [`ZTHabitatMgr::morph_exhibit`].
    ///
    /// Two corrections to the stage plan's own decompile sketch, confirmed against the live Ghidra
    /// decompile of `0x0044b900` and corroborated by the macOS `ZTHabitat_resize.c` (whose list is a
    /// count field at `+0x44` rather than a walked sentinel - same "no owned tiles left" predicate):
    /// the null-argument default is the **first owned tile** (not a subhabitat-tree value), and the
    /// `removeHabitat` gate is the **owned-tile list** being empty (not the amphibious-neighbor set).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::create_edge_pairs`].
    pub fn resize(&self, tile_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let tile_ptr = if tile_ptr == 0 { self.default_resize_tile_ptr() } else { tile_ptr };
        unsafe { call_vtable_slot_noargs(self_addr, 0x3c) };
        unsafe { call_vtable_slot_with_ptr_ptr(self_addr, 0x38, tile_ptr, self_addr) };
        self.create_edge_pairs();
        unsafe { CREATE_VIEWING_AREAS.original()(self_addr as *const u32) };
        self.recalculate_viewing_areas();
        write_live!(self, characteristics_dirty, 1u8);
        if self.get_size(false) == 0 {
            let mgr = globals().zthabitatmgr_ptr();
            unsafe { REMOVE_HABITAT_0.original()(mgr as *const u32, self_addr as *const i32) };
        }
    }

    /// Ports `ZTHabitat::addViewingArea` (`ZTHabitat_addViewingArea.c`/`.asm`, `generated.rs`'s
    /// `ADD_VIEWING_AREA`): appends `va_ptr` to [`Self::viewing_areas_begin`]/`_end`, doubling the backing
    /// buffer (minimum `1`) through real vanilla's own `PoolAlloc::allocate` when full, freeing the old
    /// buffer via [`free_event_vector_buffer`] (the same small-object freelist-bucket/`operator_delete`
    /// split `ZTHabitat::listen`'s own event-vector teardown already uses - confirmed identical via this
    /// decompile's own `(&DAT_00638000)[...]`/`operator_delete` split). Always sets
    /// [`Self::characteristics_dirty`] afterward, matching real vanilla's own unconditional
    /// `this->field_0x2d = 1` on both the grow and no-grow paths.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn add_viewing_area(&self, va_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;
        let cap_end = self.viewing_areas_cap_end;

        if end == cap_end {
            let old_len = (end - begin) / 4;
            let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
            let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 4) } as u32;

            for i in 0..old_len {
                let value: u32 = get_from_memory(begin + i * 4);
                if new_buf != 0 {
                    save_to_memory(new_buf + i * 4, value);
                }
            }
            if new_buf != 0 {
                save_to_memory(new_buf + old_len * 4, va_ptr);
            }
            free_event_vector_buffer(begin, cap_end - begin);

            save_to_memory(self_addr + 0x34, new_buf);
            save_to_memory(self_addr + 0x38, new_buf + (old_len + 1) * 4);
            save_to_memory(self_addr + 0x3c, new_buf + new_cap * 4);
        } else {
            save_to_memory(end, va_ptr);
            save_to_memory(self_addr + 0x38, end + 4);
        }
        save_to_memory::<u8>(self_addr + 0x2d, 1);
    }

    /// Ports `ZTHabitat::removeViewingArea` (`ZTHabitat_removeViewingArea.c`/`.asm`, `generated.rs`'s
    /// `REMOVE_VIEWING_AREA`): finds `va_ptr` in [`Self::viewing_areas_begin`]/`_end` (no-op if absent),
    /// destroys and frees it (call-through to real vanilla `ZTViewingArea::~ZTViewingArea` +
    /// `operator_delete` - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase), shifts
    /// every later entry down by one slot, shrinks the vector's own `_end` by one pointer, and sets
    /// [`Self::characteristics_dirty`].
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn remove_viewing_area(&self, va_ptr: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.viewing_areas_begin;
        let end = self.viewing_areas_end;

        let mut cursor = begin;
        while cursor != end && get_from_memory::<u32>(cursor) != va_ptr {
            cursor += 4;
        }
        if cursor == end {
            return;
        }

        if va_ptr != 0 {
            unsafe {
                ZTVIEWINGAREA_DESTRUCTOR.original()(va_ptr as *const u32);
                OPERATOR_DELETE.original()(va_ptr);
            }
        }

        let mut write = cursor;
        let mut read = cursor + 4;
        while read != end {
            let value: u32 = get_from_memory(read);
            save_to_memory(write, value);
            write += 4;
            read += 4;
        }
        save_to_memory(self_addr + 0x38, end - 4);
        save_to_memory::<u8>(self_addr + 0x2d, 1);
    }

    /// Ports `ZTHabitat::removeFromAllVAs` (`ZTHabitat_removeFromAllVAs.c`/`.asm`, `generated.rs`'s
    /// `REMOVE_FROM_ALL_VAS`): for every [`Self::viewing_areas_begin`]/`_end` entry whose own tile-vector
    /// (`+0x40`/`+0x44`) contains `tile_ptr`, removes it (real vanilla `ZTViewingArea::removeTile`, called
    /// through - `ZTViewingArea` itself isn't reimplemented anywhere in this codebase); if that empties the
    /// viewing area's own tile vector, removes the viewing area itself ([`Self::remove_viewing_area`]) and
    /// restarts the scan from the beginning (real vanilla's own `goto`-driven restart, since the vector was
    /// just mutated out from under the walk).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn remove_from_all_vas(&self, tile_ptr: u32) {
        'restart: loop {
            let mut cursor = self.viewing_areas_begin;
            let end = self.viewing_areas_end;
            while cursor != end {
                let va_ptr: u32 = get_from_memory(cursor);
                let vec_begin: u32 = get_from_memory(va_ptr + 0x40);
                let vec_end: u32 = get_from_memory(va_ptr + 0x44);
                let contains = (vec_begin..vec_end).step_by(4).any(|addr| get_from_memory::<u32>(addr) == tile_ptr);
                if contains {
                    unsafe { ZTVIEWINGAREA_REMOVE_TILE.original()(va_ptr as *const u32, tile_ptr as i32) };
                    let new_begin: u32 = get_from_memory(va_ptr + 0x40);
                    let new_end: u32 = get_from_memory(va_ptr + 0x44);
                    if new_begin == new_end {
                        self.remove_viewing_area(va_ptr);
                        continue 'restart;
                    }
                }
                cursor += 4;
            }
            break;
        }
    }

    /// Ports `ZTHabitat::recreateOAs` (`ZTHabitat_recreateOAs.c`, `generated.rs`'s `RECREATE_OAS`): sets
    /// byte `+0x25` (a `ZTViewingArea`-level "needs recreate" flag - unrelated to [`Self::neighbor_dirty`]'s
    /// own `+0x25`, on a different struct, despite the coincidental shared offset) on every
    /// [`Self::viewing_areas_begin`]/`_end` entry.
    pub fn recreate_oas(&self) {
        let mut cursor = self.viewing_areas_begin;
        while cursor != self.viewing_areas_end {
            let va_ptr: u32 = get_from_memory(cursor);
            save_to_memory::<u8>(va_ptr + 0x25, 1);
            cursor += 4;
        }
    }

    /// Ports `ZTHabitat::pathPlaced` (`ZTHabitat_pathPlaced.c`/`.asm`, `generated.rs`'s `PATH_PLACED`):
    /// called when `tile_ptr` becomes a path tile (real vanilla `BFTile` `+0x83` bit `0x8`) - extends an
    /// existing, adjacent `self`-owned `ZTViewingArea` to cover it if one is short enough (span `< 4` tiles
    /// along its own axis), else creates a brand-new one.
    ///
    /// Early-returns if `tile_ptr` isn't actually in the zoo (real vanilla `BFTile::isInZoo`, called
    /// through). Otherwise gathers up to 4 candidate cardinal neighbours ([`get_neighbour_ptr`]) that are
    /// themselves path tiles and within an existing viewing area (`generated.rs`'s `standalone::TILE_WITHIN_AVA`,
    /// called through) - as a plain `Vec<(Direction, u32)>` local scratch (real vanilla backs the equivalent
    /// local list with its own small-object allocator; this is pure per-call scratch nothing else reads, so
    /// a plain Rust `Vec` carries no cross-allocator risk, same reasoning as [`ZTHabitatMgr::can_find_path`]'s
    /// own BFS frontier).
    ///
    /// For each candidate in turn, scans the neighbour tile's own cached-neighbour-habitat cell-row slots
    /// (the same 8 `+0x4..0x24` slots [`ZTHabitatMgr::habitat_tile_changed`] marks dirty) for a
    /// `ZTViewingArea` owned by `self`: if found, reads its current NS/EW extent (axis chosen by the
    /// candidate's own direction - North/South use `generated.rs`'s `ztviewingarea::GET_NSEXTENT`,
    /// East/West `GET_EWEXTENT`, both called through), clamps it to include the neighbour tile's own
    /// coordinate on that axis, and - if the resulting span would stay under 4 tiles - adds `tile_ptr` to
    /// that viewing area (`ztviewingarea::ADD_TILE`, called through) and returns immediately, matching real
    /// vanilla's own `goto`-past-the-new-VA-creation-code exactly.
    ///
    /// If every candidate's matching viewing area (if any) rejected the tile as too large, constructs a
    /// brand-new one (`operator_new(0x5c)` + real vanilla `ZTViewingArea::ZTViewingArea` constructor, both
    /// called through) and appends it via [`Self::add_viewing_area`] - but **not** when there were
    /// candidates and *none* of them ever found a matching viewing area at all (real vanilla's own
    /// `if (!bVar4) goto cleanup` skips the new-VA creation code entirely in that case, matched here as-is
    /// rather than "fixed" - a genuine, if surprising, real vanilla edge case, not a bug this port
    /// introduces). A new VA is still created, as usual, when there were no candidates at all.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    pub fn path_placed(&self, tile_ptr: u32) {
        if unsafe { BFTILE_IS_IN_ZOO.original()(tile_ptr as *const u32, 1) } == 0 {
            return;
        }

        let world = globals().ztworldmgr();
        let habitat_mgr = globals().zthabitatmgr();
        let self_addr = self as *const Self as u32;

        let mut candidates: Vec<(Direction, u32)> = Vec::new();
        for direction in [Direction::North, Direction::East, Direction::South, Direction::West] {
            let neighbour_ptr = get_neighbour_ptr(world, tile_ptr, direction);
            if neighbour_ptr == 0 {
                continue;
            }
            let flags: u8 = get_from_memory(neighbour_ptr + 0x83);
            if flags & 8 == 0 {
                continue;
            }
            if unsafe { TILE_WITHIN_AVA.original()(neighbour_ptr as i32, self_addr as i32) } == 0 {
                continue;
            }
            candidates.push((direction, neighbour_ptr));
        }

        if !candidates.is_empty() {
            let mut any_rejected = false;
            for (direction, neighbour_ptr) in &candidates {
                let neighbour = get_from_memory::<BFTile>(*neighbour_ptr);
                let Some(cell_addr) = habitat_mgr.get_habitat_cell_addr(neighbour.pos.x, neighbour.pos.y) else {
                    continue;
                };
                for slot in 0..8u32 {
                    let va_ptr: u32 = get_from_memory(cell_addr + 0x4 + slot * 4);
                    if va_ptr == 0 {
                        continue;
                    }
                    let owner: u32 = get_from_memory(va_ptr);
                    if owner != self_addr {
                        continue;
                    }
                    let is_ns = matches!(direction, Direction::North | Direction::South);
                    let mut low: i32 = 0;
                    let mut high: i32 = 0;
                    if is_ns {
                        unsafe { ZTVIEWINGAREA_GET_NSEXTENT.original()(va_ptr as *const u32, &mut low, &mut high) };
                    } else {
                        unsafe { ZTVIEWINGAREA_GET_EWEXTENT.original()(va_ptr as *const u32, &mut low, &mut high) };
                    }
                    let coord = if is_ns { neighbour.pos.y } else { neighbour.pos.x };
                    if coord < low {
                        low = coord;
                    }
                    if high < coord {
                        high = coord;
                    }
                    if (high - low) + 1 < 4 {
                        unsafe { ZTVIEWINGAREA_ADD_TILE.original()(va_ptr as *const u32, tile_ptr as *const std::ffi::c_void) };
                        return;
                    }
                    any_rejected = true;
                    break;
                }
            }
            if !any_rejected {
                return;
            }
        }

        let new_va = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
        if new_va != 0 {
            unsafe { ZTVIEWINGAREA_CONSTRUCTOR.original()(new_va as *const u32, self_addr as *const std::ffi::c_void, tile_ptr as *const std::ffi::c_void) };
            self.add_viewing_area(new_va);
        }
    }
}


/// Formats `time` as a UTC date, or as its raw tick count when it's outside `UtcDateTime`'s range.
fn display_file_time(time: FileTime) -> String {
    match UtcDateTime::try_from(time) {
        Ok(date) => date.to_string(),
        Err(_) => format!("<out of range: {:#x}>", time.to_raw()),
    }
}

impl fmt::Display for ZTHabitat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTHabitat {{",)?;
        writeln!(f, "  vtable: {:#x},", self.vtable)?;
        writeln!(f, "  zt_show_info_ptr: {:#x},", self.zt_show_info_ptr)?;
        writeln!(f, "  owned_tiles_ptr: {:#x},", self.owned_tiles_ptr)?;
        writeln!(f, "  entrance_tile_ptr: {:#x},", self.entrance_tile_ptr)?;
        writeln!(f, "  entrance_rotation: {:#x},", self.entrance_rotation)?;
        writeln!(f, "  characteristics_dirty: {},", self.characteristics_dirty)?;
        writeln!(f, "  time_last_serviced: {:#x},", self.time_last_serviced)?;
        writeln!(f, "  attractiveness: {},", self.attractiveness)?;
        writeln!(f, "  has_keeper_assigned_raw: {},", self.has_keeper_assigned_raw)?;
        writeln!(f, "  current_donations: {},", self.current_donations)?;
        writeln!(f, "  last_donations: {},", self.last_donations)?;
        writeln!(f, "  total_donations: {},", self.total_donations)?;
        writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        writeln!(f, "  unknown_u32_2: {:#x},", self.unknown_u32_2)?;
        writeln!(f, "  unknown_u32_3: {:#x},", self.unknown_u32_3)?;
        writeln!(f, "  unknown_u32_4: {:#x},", self.unknown_u32_4)?;
        writeln!(f, "  created_timestamp: {},", display_file_time(self.created_timestamp))?;
        writeln!(
            f,
            "  unknown_nt_time: {} ({}, {}, {}),",
            display_file_time(self.unknown_nt_time),
            self.unknown_nt_time.to_raw() as f64,
            self.unknown_nt_time.to_raw() as u32,
            (self.unknown_nt_time.to_raw() >> 32) as u32
        )?;
        writeln!(f, "  exhibit_name: {},", self.exhibit_name.copy_to_string())?;

        // writeln!(f, "  entrance_x: {},", self.entrance_x)?;
        // writeln!(f, "  entrance_y: {},", self.entrance_y)?;
        // writeln!(f, "  entrance_rotation: {},", self.entrance_rotation)?;
        // writeln!(f, "  unknown_ptr: {:#x},", self.unknown_ptr)?;
        // writeln!(f, "  unknown_ptr2: {:#x},", self.unknown_ptr2)?;
        // writeln!(f, "  unknown_ptr3: {:#x},", self.unknown_ptr3)?;
        // writeln!(f, "  current_donations: {},", self.current_donations)?;
        // writeln!(f, "  last_donations: {},", self.last_donations)?;
        // writeln!(f, "  total_donations: {},", self.total_donations)?;
        // writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        // writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        // writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        // writeln!(f, " unknown_ptr4: {:#x},", self.unknown_ptr4)?;
        // writeln!(f, " unknown_ptr5: {:#x},", self.unknown_ptr5)?;
        // writeln!(f, " unknown_ptr6: {:#x},", self.unknown_ptr6)?;
        // writeln!(f, " created_timestamp: {:#x},", self.created_timestamp)?;
        writeln!(f, "}}")
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{self, offset_of};

    use crate::zthabitat::support::test_fixtures::{leak_neighbor_set, leak_tile_list, leak_tree_node};

    use super::{
        clamped_score, corner_height, count_penalty, elevation_band, normalise_scenery_score, overlap_condition_bits, percent_band, terrain_category_step,
        tile_count_adjustment, PercentBand, ZTHabitat,
    };

    #[test]
    fn corner_height_decodes_packed_corners() {
        // packed = 0b11_10_01_01: corner 1 (bits 6-7) = 3, corner 3 (bits 4-5) = 2, corner 5 (bits 2-3) = 1, low = 1.
        let packed = 0b1110_0101;
        assert_eq!(corner_height(10, packed, 1), 10 + 3 - 1);
        assert_eq!(corner_height(10, packed, 3), 10 + 2 - 1);
        assert_eq!(corner_height(10, packed, 5), 10 + 1 - 1);
        assert_eq!(corner_height(10, packed, 7), 10);
    }

    #[test]
    fn elevation_band_flags_and_score() {
        // tally 200 over 100 tiles: pct = 100. threshold 100: perfect.
        let ok = elevation_band(200, 100, 100);
        assert_eq!(ok, PercentBand { too_low: false, too_high: false, critical: false, score: 100.0 });
        // pct 100 vs threshold 130: too low (100 < 120) and critical (30 > 20); score 100 - 30.
        let low = elevation_band(200, 100, 130);
        assert!(low.too_low && !low.too_high && low.critical);
        assert_eq!(low.score, 70.0);
        // pct 100 vs threshold 85: too high (100 > 95), not critical (15).
        let high = elevation_band(200, 100, 85);
        assert!(!high.too_low && high.too_high && !high.critical);
        assert_eq!(high.score, 85.0);
        // deviation clamps at 50.
        assert_eq!(elevation_band(0, 100, 90).score, 50.0);
    }

    #[test]
    fn overlap_condition_bits_thresholds() {
        assert_eq!(overlap_condition_bits(0, 100), (false, false));
        assert_eq!(overlap_condition_bits(5, 100), (false, false));
        assert_eq!(overlap_condition_bits(6, 100), (true, false));
        assert_eq!(overlap_condition_bits(10, 100), (true, false));
        assert_eq!(overlap_condition_bits(11, 100), (true, true));
    }

    #[test]
    fn tile_count_adjustment_gates_and_clamps() {
        assert_eq!(tile_count_adjustment(false, 50, 0, 100, 1000), 0);
        assert_eq!(tile_count_adjustment(true, 0, 0, 100, 1000), 0);
        assert_eq!(tile_count_adjustment(true, 100, 0, 100, 1000), 0);
        // (100 - 10) * 50 / 50 - 10 = 80, below the cap.
        assert_eq!(tile_count_adjustment(true, 50, 10, 100, 1000), 80);
        // capped by the filled size.
        assert_eq!(tile_count_adjustment(true, 50, 10, 100, 30), 30);
        // negative raw result clamps to 0: (10 - 40) * 50 / 50 - 40 = -70.
        assert_eq!(tile_count_adjustment(true, 50, 40, 10, 1000), 0);
    }

    #[test]
    fn terrain_category_step_penalty_and_bonus() {
        // Negative weight only applies when the species' terrain is actually present.
        assert_eq!(terrain_category_step(10.0, -5, 0, 0, 100), 10.0);
        assert_eq!(terrain_category_step(10.0, -5, 3, 3, 100), 5.0);
        // Non-negative weight adds min(weight, count * 100 / denom): 25% of tiles caps a weight of 40 at 25.
        assert_eq!(terrain_category_step(0.0, 40, 25, 25, 100), 25.0);
        assert_eq!(terrain_category_step(0.0, 10, 25, 25, 100), 10.0);
    }

    #[test]
    fn scenery_normalisation_scales_caps_and_penalises_negatives() {
        // 40 owned tiles: factor 1.0 -> divide by 100.
        assert_eq!(normalise_scenery_score(2500.0, 40), 25.0);
        // 20 owned tiles: factor 0.5 is floored at 1.0 -> still divide by 100.
        assert_eq!(normalise_scenery_score(2500.0, 20), 25.0);
        // 80 owned tiles: factor 2.0 -> divide by 200.
        assert_eq!(normalise_scenery_score(2500.0, 80), 12.5);
        // Cap at 100.
        assert_eq!(normalise_scenery_score(90000.0, 40), 100.0);
        // Negative results are multiplied by 50.
        assert_eq!(normalise_scenery_score(-100.0, 40), -50.0);
    }

    #[test]
    fn count_penalty_thresholds_and_critical_multiplier() {
        assert_eq!(count_penalty(2, false, 3), 0.0);
        assert_eq!(count_penalty(3, false, 3), 250.0);
        // critical shelter multiplies by (count - 3): 4 -> 300 * 1.
        assert_eq!(count_penalty(4, true, 3), 300.0);
        // critical with count <= 2: base 0, so the product is (negative * 0) == 0 in value.
        assert_eq!(count_penalty(1, true, 3), 0.0);
        // toy critical offset is 2.
        assert_eq!(count_penalty(3, true, 2), 250.0);
    }

    #[test]
    fn clamped_score_caps_at_100() {
        assert_eq!(clamped_score(0.0), 100.0);
        assert_eq!(clamped_score(-600.0), 100.0);
        assert_eq!(clamped_score(250.0), -150.0);
    }

    #[test]
    fn percent_band_within_tolerance_scores_full() {
        // 50 of 100 tiles vs. threshold 50: no flags, perfect score.
        let band = percent_band(50, 100.0, 50, 4.0);
        assert_eq!(band, PercentBand { too_low: false, too_high: false, critical: false, score: 100.0 });
    }

    #[test]
    fn percent_band_low_high_and_critical_flags() {
        // pct = 20 vs threshold 50: low + critical, deviation 30 -> 100 - 8*30.
        let low = percent_band(20, 100.0, 50, 4.0);
        assert!(low.too_low && !low.too_high && low.critical);
        assert_eq!(low.score, -140.0);

        // pct = 60 vs threshold 50: high, deviation 10 is critical (> 8).
        let high = percent_band(60, 100.0, 50, 4.0);
        assert!(!high.too_low && high.too_high && high.critical);

        // pct = 56 vs threshold 50: high but not critical.
        let mild = percent_band(56, 100.0, 50, 3.0);
        assert!(mild.too_high && !mild.critical);
        assert_eq!(mild.score, 100.0 - 6.0 * 6.0);
    }

    #[test]
    fn percent_band_zero_pct_with_positive_threshold_is_too_low() {
        // threshold 3 (< 4) so `pct < threshold - 4` alone would miss it; the `pct == 0` stub catches it.
        let band = percent_band(0, 100.0, 3, 4.0);
        assert!(band.too_low && !band.too_high && !band.critical);
        // threshold 0: nothing to be short of.
        let zero = percent_band(0, 100.0, 0, 4.0);
        assert!(!zero.too_low && !zero.too_high);
    }

    #[test]
    fn percent_band_deviation_clamps_at_fifty() {
        let band = percent_band(0, 100.0, 90, 4.0);
        assert_eq!(band.score, 100.0 - 8.0 * 50.0);
    }

    use crate::util::{ref_from_memory, save_to_memory};
    use crate::zthabitat::tank_exhibit::ZTTankExhibit;

    /// Host-safe fixture: `mem::zeroed()` is valid for every field of both structs (raw ints,
    /// `f32`s, `FileTime`s, the 3-pointer `ZTBufferString`), and only `vtable` is ever written.
    /// No test here calls through the fixture's vtable word as a function pointer - that would
    /// dereference into unmapped memory.
    fn fixture_habitat(vtable: u32) -> ZTHabitat {
        let mut habitat: ZTHabitat = unsafe { mem::zeroed() };
        habitat.vtable = vtable;
        habitat
    }

    /// Writes `state` into a leaked zeroed fake `ZTAnimal` block at the two offsets
    /// [`ZTHabitat::has_portal_animal`] reads (`+0x170` goal/action id, `+0x234` destination tile
    /// pointer) and returns the block's address. Leaked rather than stack-allocated so the raw
    /// address stays valid for the port's volatile reads, same pattern as the
    /// `reimplementation_tests` synthetic fixtures.
    fn leak_fake_animal(state: u32, target_tile_ptr: u32) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; 0x238].into_boxed_slice());
        let animal_ptr = block.as_ptr() as u32;
        save_to_memory(animal_ptr + 0x170, state);
        save_to_memory(animal_ptr + 0x234, target_tile_ptr);
        animal_ptr
    }

    /// A zeroed [`ZTHabitat`] whose `all_animals` vector spans `animals` (leaked so the raw
    /// begin/end pointers stay valid).
    fn fixture_habitat_with_animals(animals: &[u32]) -> ZTHabitat {
        let mut habitat: ZTHabitat = unsafe { mem::zeroed() };
        let block: &'static mut [u32] = Box::leak(animals.to_vec().into_boxed_slice());
        habitat.all_animals_begin = block.as_ptr() as u32;
        habitat.all_animals_end = block.as_ptr() as u32 + block.len() as u32 * 4;
        habitat
    }

    /// A zeroed [`ZTHabitat`] whose `show_neighbors_head` points at a leaked head node whose
    /// `_Parent` (`+0x4`, the root slot) holds `root`. Every show-tree test must set a real head -
    /// a null `show_neighbors_head` would make the port read address `0x4` in the host process
    /// (real vanilla cannot reach that either; its own set constructor always builds the head).
    fn fixture_habitat_with_show_tree(root: u32) -> ZTHabitat {
        let mut habitat: ZTHabitat = unsafe { mem::zeroed() };
        habitat.show_neighbors_head = leak_tree_node(root, 0, 0, 0);
        habitat
    }

    /// `hasPortalAnimal`'s `.asm` empty-vector early-out (`CMP ESI, EAX` → `XOR AL,AL`): no
    /// animals means `false` regardless of the requested target, null or not.
    #[test]
    fn empty_animals_vector_returns_false() {
        let habitat = fixture_habitat_with_animals(&[]);
        assert!(!habitat.has_portal_animal(0x0063_2100));
        assert!(!habitat.has_portal_animal(0));
    }

    /// Animals whose `+0x170` goal/action id is neither `0x85` nor `0x86` are skipped without
    /// touching their destination tile (full 32-bit compares in the `.asm`, so neighboring ids
    /// like `0x84`/`0x87` don't match either, and neither does `0x185`'s shared low byte).
    #[test]
    fn non_portal_animal_states_are_skipped() {
        for state in [0u32, 0x84, 0x87, 0x185] {
            let habitat = fixture_habitat_with_animals(&[leak_fake_animal(state, 0)]);
            assert!(!habitat.has_portal_animal(0), "state {state:#x} matched");
        }
    }

    /// A portal-state animal (`0x85`/`0x86`) with a null destination tile resolves to candidate
    /// habitat `0` - which matches only a null `target_habitat_ptr` (vanilla's own `0 == 0` arm,
    /// `.asm` `XOR EAX,EAX; JMP` to the compare) and loses against any non-null one. The null-tile
    /// arm never reaches the habitat grid, keeping these fixtures host-safe.
    #[test]
    fn portal_animal_null_tile_resolution() {
        for state in [0x85u32, 0x86] {
            let habitat = fixture_habitat_with_animals(&[leak_fake_animal(state, 0)]);
            assert!(habitat.has_portal_animal(0), "state {state:#x} should match a null target");
            assert!(!habitat.has_portal_animal(0x0059_e9a3), "state {state:#x} should not match a non-null target");
        }
    }

    /// The loop continues past non-portal animals and stops at the first match, so a portal
    /// animal in any position decides the answer.
    #[test]
    fn portal_animal_is_found_in_any_loop_position() {
        let habitat = fixture_habitat_with_animals(&[
            leak_fake_animal(0x84, 0),
            leak_fake_animal(0x86, 0),
        ]);
        assert!(habitat.has_portal_animal(0));

        let habitat = fixture_habitat_with_animals(&[
            leak_fake_animal(0x86, 0),
            leak_fake_animal(0x85, 0),
        ]);
        assert!(habitat.has_portal_animal(0));

        let habitat = fixture_habitat_with_animals(&[
            leak_fake_animal(0x85, 0),
            leak_fake_animal(0x86, 0),
        ]);
        assert!(!habitat.has_portal_animal(0x0059_e9a3));
    }

    /// `isShowNeighbor`'s `.asm` null-root guard (`MOV EAX,[ECX+0x4]` + `TEST EAX,EAX` into the
    /// loop tail): an empty set leaves the candidate at the head, so the tail's `candidate != head`
    /// arm fails and the answer is `false` for every key, null or not.
    #[test]
    fn empty_show_tree_returns_false() {
        let habitat = fixture_habitat_with_show_tree(0);
        assert!(!habitat.is_show_neighbor(0x0063_2100));
        assert!(!habitat.is_show_neighbor(0));
    }

    /// One-node set: an exact hit satisfies both tail arms; a key below the value passes the
    /// descent but fails the `candidate->_Value <= key` confirmation (the `.asm`'s JC-to-tail
    /// path); a key above the value descends right to null, so the candidate never leaves the head.
    #[test]
    fn show_neighbor_hit_and_miss_single_node() {
        let habitat = fixture_habitat_with_show_tree(leak_tree_node(0, 0, 0, 0x2000));
        assert!(habitat.is_show_neighbor(0x2000));
        assert!(!habitat.is_show_neighbor(0x1000));
        assert!(!habitat.is_show_neighbor(0x3000));
    }

    /// Three-node BST: hits resolve from either subtree and the root; a key falling between two
    /// nodes survives the descent but fails the confirmation compare; a key beyond the maximum
    /// descends right to null and leaves the candidate at the head.
    #[test]
    fn show_neighbor_multi_node_descent() {
        let left = leak_tree_node(0, 0, 0, 0x1000);
        let right = leak_tree_node(0, 0, 0, 0x3000);
        let root = leak_tree_node(0, left, right, 0x2000);
        save_to_memory(left + 0x4, root);
        save_to_memory(right + 0x4, root);
        let habitat = fixture_habitat_with_show_tree(root);

        assert!(habitat.is_show_neighbor(0x1000));
        assert!(habitat.is_show_neighbor(0x2000));
        assert!(habitat.is_show_neighbor(0x3000));
        assert!(!habitat.is_show_neighbor(0x2500));
        assert!(!habitat.is_show_neighbor(0x4000));
    }

    /// `less<ZTHabitat*>` is a plain unsigned compare, so a null key can only match a null value -
    /// a tree of real (non-null) pointers answers `false` for it: lower_bound(0) lands on the
    /// leftmost node and its value `<= 0` confirmation fails.
    #[test]
    fn null_target_never_matches_non_null_tree() {
        let left = leak_tree_node(0, 0, 0, 0x10_00);
        let right = leak_tree_node(0, 0, 0, 0x30_00);
        let root = leak_tree_node(0, left, right, 0x20_00);
        save_to_memory(left + 0x4, root);
        save_to_memory(right + 0x4, root);
        let habitat = fixture_habitat_with_show_tree(root);

        assert!(!habitat.is_show_neighbor(0));
    }

    /// Leaks a zeroed 24-byte MSVC map-node-shaped block and writes `parent`/`left`/`right`/`key`/
    /// `value` into the `+0x4`/`+0x8`/`+0xc`/`+0x10`/`+0x14` slots [`ZTHabitat::get_show_portal`]'s
    /// tank-branch descent reads, returning the block's address - the same layout as
    /// [`leak_tree_node`] with one extra trailing `value` word for the map's `ZTFence*` payload.
    fn leak_map_node(parent: u32, left: u32, right: u32, key: u32, value: u32) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; 0x18].into_boxed_slice());
        let node_ptr = block.as_ptr() as u32;
        save_to_memory(node_ptr + 0x4, parent);
        save_to_memory(node_ptr + 0x8, left);
        save_to_memory(node_ptr + 0xc, right);
        save_to_memory(node_ptr + 0x10, key);
        save_to_memory(node_ptr + 0x14, value);
        node_ptr
    }

    /// A tank-vtable [`ZTHabitat`] whose `show_portal_map_head` points at a leaked head node whose
    /// `_Parent` (`+0x4`) holds `root` - same reasoning as [`fixture_habitat_with_show_tree`], plus
    /// the tank vtable so [`ZTHabitat::is_tank`] routes `get_show_portal` into the map descent.
    fn fixture_tank_habitat_with_portal_map(root: u32) -> ZTHabitat {
        let mut habitat = fixture_habitat(ZTHabitat::TANK_VTABLE_PTR);
        habitat.show_portal_map_head = leak_map_node(0, 0, 0, 0, 0);
        save_to_memory(habitat.show_portal_map_head + 0x4, root);
        habitat
    }

    /// An empty portal map leaves the candidate at the head, so `get_show_portal` returns `0` for
    /// every key without ever reaching `msvc_std_tree24::insert` - the pure-read shape documented on
    /// [`ZTHabitat::get_show_portal`] itself.
    #[test]
    fn empty_portal_map_returns_zero() {
        let habitat = fixture_tank_habitat_with_portal_map(0);
        assert_eq!(habitat.get_show_portal(0x0063_2100), 0);
        assert_eq!(habitat.get_show_portal(0), 0);
    }

    /// One-entry map: an exact key hit returns the stored `ZTFence*` value; a key below or above
    /// the stored key both miss (below fails the `<=` confirmation, above descends right to null
    /// and never leaves the head), same descent shape as [`show_neighbor_hit_and_miss_single_node`].
    #[test]
    fn portal_map_hit_and_miss_single_node() {
        let node = leak_map_node(0, 0, 0, 0x2000, 0x9000_0000);
        let habitat = fixture_tank_habitat_with_portal_map(node);
        assert_eq!(habitat.get_show_portal(0x2000), 0x9000_0000);
        assert_eq!(habitat.get_show_portal(0x1000), 0);
        assert_eq!(habitat.get_show_portal(0x3000), 0);
    }

    /// Three-node BST: hits resolve from either subtree and the root to their own distinct values;
    /// a key falling between two nodes survives the descent but fails the confirmation compare.
    #[test]
    fn portal_map_multi_node_descent() {
        let left = leak_map_node(0, 0, 0, 0x1000, 0xa000_0001);
        let right = leak_map_node(0, 0, 0, 0x3000, 0xa000_0003);
        let root = leak_map_node(0, left, right, 0x2000, 0xa000_0002);
        save_to_memory(left + 0x4, root);
        save_to_memory(right + 0x4, root);
        let habitat = fixture_tank_habitat_with_portal_map(root);

        assert_eq!(habitat.get_show_portal(0x1000), 0xa000_0001);
        assert_eq!(habitat.get_show_portal(0x2000), 0xa000_0002);
        assert_eq!(habitat.get_show_portal(0x3000), 0xa000_0003);
        assert_eq!(habitat.get_show_portal(0x2500), 0);
        assert_eq!(habitat.get_show_portal(0x4000), 0);
    }

    /// A zeroed [`ZTHabitat`]-shaped leaked block whose `owned_tiles_ptr` (`+0x40`) points at a
    /// leaked `tiles` list - the block address doubles as a neighbor-set tree payload (real `getSize`
    /// reads the neighbor's own `ZTHabitat*` from node `+0x10` and recurses into its own `+0x40` list).
    fn leak_neighbor_habitat(tiles: &[u32]) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; std::mem::size_of::<ZTHabitat>()].into_boxed_slice());
        let habitat_ptr = block.as_ptr() as u32;
        save_to_memory(habitat_ptr + 0x40, leak_tile_list(tiles));
        habitat_ptr
    }

    /// A zeroed [`ZTHabitat`] with a leaked owned-tile list and a leaked amphibious-neighbor set
    /// whose payloads are the given neighbor habitat block addresses.
    fn fixture_habitat_with_tiles_and_neighbors(tiles: &[u32], neighbors: &[u32]) -> ZTHabitat {
        let mut habitat: ZTHabitat = unsafe { mem::zeroed() };
        habitat.owned_tiles_ptr = leak_tile_list(tiles);
        habitat.amphibious_neighbors_head = leak_neighbor_set(neighbors);
        habitat
    }

    /// `getSize`'s `.c` empty arms: an empty owned-tile list counts 0 with or without the neighbor
    /// walk, and an empty neighbor set adds nothing.
    #[test]
    fn get_size_empty_habitat_is_zero() {
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[], &[]);
        assert_eq!(habitat.get_size(false), 0);
        assert_eq!(habitat.get_size(true), 0);
    }

    /// The false arm is exactly the owned-tile node count; a set-but-empty neighbor set adds nothing
    /// to the true arm.
    #[test]
    fn get_size_counts_owned_tiles_without_subhabs() {
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[0x1000, 0x2000, 0x3000], &[]);
        assert_eq!(habitat.get_size(false), 3);
        assert_eq!(habitat.get_size(true), 3);
    }

    /// Each neighbor's own count is added exactly once (the tree's nodes are visited once each by
    /// the in-order walk; the neighbors' own tile lists are untouched by the sum).
    #[test]
    fn get_size_sums_each_neighbor_once() {
        let b = leak_neighbor_habitat(&[1, 2, 3, 4, 5]);
        let c = leak_neighbor_habitat(&[1, 2, 3, 4, 5, 6, 7]);
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[0x1000, 0x2000], &[b, c]);
        assert_eq!(habitat.get_size(false), 2);
        assert_eq!(habitat.get_size(true), 2 + 5 + 7);
    }

    /// Recursion re-passes `false` down: B's own neighbor set contains C, but `A.get_size(true)`
    /// sums A + B only (16, not 25) - the decompile's `getSize(neighbor, '\0')` argument, proven
    /// positively here without needing a crash-risk cycle fixture.
    #[test]
    fn get_size_passes_false_down() {
        let c = leak_neighbor_habitat(&[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let b = leak_neighbor_habitat(&[1, 2, 3, 4, 5, 6, 7]);
        save_to_memory(b + 0x8, leak_neighbor_set(&[c]));
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[1, 2, 3, 4, 5, 6, 7, 8, 9], &[b]);
        assert_eq!(habitat.get_size(false), 9);
        assert_eq!(habitat.get_size(true), 16);
    }

    /// A leaked tank-vtable [`ZTHabitat`]-shaped block whose `show_portal_map_head` holds a
    /// one-entry map keyed by `key` returning `value` - a recursion target for
    /// [`ZTHabitat::get_show_portal`]'s non-tank branch, which recurses into each amphibious
    /// neighbor's own `get_show_portal` rather than reading the map directly.
    fn leak_tank_neighbor_with_portal(key: u32, value: u32) -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; std::mem::size_of::<ZTHabitat>()].into_boxed_slice());
        let habitat_ptr = block.as_ptr() as u32;
        save_to_memory(habitat_ptr, ZTHabitat::TANK_VTABLE_PTR);
        let head = leak_map_node(0, 0, 0, 0, 0);
        let root = leak_map_node(0, 0, 0, key, value);
        save_to_memory(head + 0x4, root); // head._Parent = root
        save_to_memory(habitat_ptr + offset_of!(ZTHabitat, show_portal_map_head) as u32, head);
        habitat_ptr
    }

    /// An empty amphibious-neighbor set never recurses, so the non-tank branch returns `0` for any
    /// key - matching the real body's own `field_0xc != 0` guard without needing to model that
    /// field separately (see [`ZTHabitat::get_show_portal`]'s own doc comment).
    #[test]
    fn get_show_portal_non_tank_empty_neighbors_returns_zero() {
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[], &[]);
        assert_eq!(habitat.get_show_portal(0x2000), 0);
    }

    /// A single amphibious neighbor's own portal-map hit is returned as-is through the recursive
    /// call; a miss on that same neighbor still returns `0`.
    #[test]
    fn get_show_portal_non_tank_single_neighbor_recurses() {
        let neighbor = leak_tank_neighbor_with_portal(0x2000, 0x9000_0000);
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[], &[neighbor]);
        assert_eq!(habitat.get_show_portal(0x2000), 0x9000_0000);
        assert_eq!(habitat.get_show_portal(0x3000), 0);
    }

    /// Two neighbors, only the second holding a hit: the walk must continue past the first
    /// neighbor's own miss (`0`) rather than stopping there.
    #[test]
    fn get_show_portal_non_tank_finds_hit_past_earlier_miss() {
        let first = leak_tank_neighbor_with_portal(0x5000, 0xb000_0000);
        let second = leak_tank_neighbor_with_portal(0x2000, 0x9000_0000);
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[], &[first, second]);
        assert_eq!(habitat.get_show_portal(0x2000), 0x9000_0000);
    }

    /// A leaked, fully-addressable [`ZTHabitat`] for [`ZTHabitat::portal_dispatch_plan`] fixtures: tank
    /// vtable + non-null `zt_show_info_ptr` (satisfies the `is_tank() && is_show_tank()` gate), an
    /// empty `all_animals` vector, and a valid-but-empty `show_portal_map_head` (same reasoning as
    /// [`fixture_tank_habitat_with_portal_map`] - a null head would make `get_show_portal`'s descent
    /// read address `0x4` in the host process for any pair [`set_portal_map_entry`] doesn't cover).
    /// Leaked (not stack-built) because `portal_dispatch_plan` passes `self`'s own address into each
    /// neighbor's reciprocal `has_portal_animal`/`get_show_portal` call, which must resolve to a real,
    /// dereferenceable block.
    fn leak_tank_show_habitat() -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; std::mem::size_of::<ZTHabitat>()].into_boxed_slice());
        let habitat_ptr = block.as_ptr() as u32;
        save_to_memory(habitat_ptr, ZTHabitat::TANK_VTABLE_PTR);
        save_to_memory(habitat_ptr + offset_of!(ZTHabitat, zt_show_info_ptr) as u32, 0xdead_0000u32);
        save_to_memory(habitat_ptr + offset_of!(ZTHabitat, show_portal_map_head) as u32, leak_map_node(0, 0, 0, 0, 0));
        habitat_ptr
    }

    /// Overwrites `habitat_ptr`'s `show_neighbors_head` so
    /// [`walk_neighbor_tree`](crate::zthabitat::support::walk_neighbor_tree) visits exactly `neighbors`.
    fn set_show_neighbors(habitat_ptr: u32, neighbors: &[u32]) {
        save_to_memory(habitat_ptr + offset_of!(ZTHabitat, show_neighbors_head) as u32, leak_neighbor_set(neighbors));
    }

    /// Overwrites `habitat_ptr`'s `show_portal_map_head` (set to a valid empty map by
    /// [`leak_tank_show_habitat`]) with a map holding every `(key, value)` in `entries`, built as a
    /// right-leaning vine over keys sorted ascending - the lower-bound descent
    /// [`ZTHabitat::get_show_portal`] performs only depends on real BST ordering, and a sorted right
    /// vine (every left child null) satisfies that for arbitrary runtime key values (leaked-block
    /// addresses, whose relative order isn't known up front) without needing a real balanced tree -
    /// same trick as [`leak_neighbor_set`]'s own right vine.
    fn set_portal_map_entries(habitat_ptr: u32, entries: &[(u32, u32)]) {
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|(key, _)| *key);
        let head = leak_map_node(0, 0, 0, 0, 0);
        let mut root = head;
        let mut prev = head;
        for (key, value) in sorted {
            let node = leak_map_node(prev, 0, 0, key, value);
            save_to_memory(prev + 0xc, node); // prev._Right = node
            if root == head {
                root = node;
            }
            prev = node;
        }
        save_to_memory(head + 0x4, root); // head._Parent = root
        save_to_memory(habitat_ptr + offset_of!(ZTHabitat, show_portal_map_head) as u32, head);
    }

    /// Single-entry convenience wrapper over [`set_portal_map_entries`].
    fn set_portal_map_entry(habitat_ptr: u32, other_ptr: u32, fence_ptr: u32) {
        set_portal_map_entries(habitat_ptr, &[(other_ptr, fence_ptr)]);
    }

    /// Non-tank habitats never build a plan, regardless of any neighbor/portal-map state.
    #[test]
    fn portal_dispatch_plan_non_tank_is_empty() {
        assert!(fixture_habitat(0).portal_dispatch_plan().is_empty());
    }

    /// A tank habitat with no attached `ZTShowInfo` (`zt_show_info_ptr == 0`) fails the `is_show_tank`
    /// half of the gate even though `is_tank()` passes.
    #[test]
    fn portal_dispatch_plan_tank_without_show_info_is_empty() {
        let habitat = fixture_habitat(ZTHabitat::TANK_VTABLE_PTR);
        assert_eq!(habitat.zt_show_info_ptr, 0);
        assert!(habitat.portal_dispatch_plan().is_empty());
    }

    /// Both legs found: the first leg (`self -> neighbor`) always plays its sound; the second
    /// (`neighbor -> self`) does not, since the first already found a portal.
    #[test]
    fn portal_dispatch_plan_both_legs_found_second_leg_silent() {
        let self_ptr = leak_tank_show_habitat();
        let neighbor_ptr = leak_tank_show_habitat();
        set_show_neighbors(self_ptr, &[neighbor_ptr]);
        set_portal_map_entry(self_ptr, neighbor_ptr, 0x7000_0001);
        set_portal_map_entry(neighbor_ptr, self_ptr, 0x7000_0002);

        let plan = unsafe { ref_from_memory::<ZTHabitat>(self_ptr) }.portal_dispatch_plan();
        assert_eq!(plan.len(), 2);
        assert_eq!((plan[0].fence_ptr, plan[0].play_sound), (0x7000_0001, true));
        assert_eq!((plan[1].fence_ptr, plan[1].play_sound), (0x7000_0002, false));
        assert!(!plan[0].is_open && !plan[1].is_open); // both animal vectors empty
    }

    /// The first leg missing (`get_show_portal` returns 0, no entry pushed) makes the second leg the
    /// sound-playing "primary" - `portal_a == 0` at the point the second leg is built.
    #[test]
    fn portal_dispatch_plan_second_leg_plays_sound_when_first_leg_missing() {
        let self_ptr = leak_tank_show_habitat();
        let neighbor_ptr = leak_tank_show_habitat();
        set_show_neighbors(self_ptr, &[neighbor_ptr]);
        set_portal_map_entry(neighbor_ptr, self_ptr, 0x7000_0002);

        let plan = unsafe { ref_from_memory::<ZTHabitat>(self_ptr) }.portal_dispatch_plan();
        assert_eq!(plan.len(), 1);
        assert_eq!((plan[0].fence_ptr, plan[0].play_sound), (0x7000_0002, true));
    }

    /// Neither leg finds a portal: an empty plan, not two zero-fence entries - `get_show_portal == 0`
    /// must skip the push entirely on both sides.
    #[test]
    fn portal_dispatch_plan_no_portal_either_leg_is_empty() {
        let self_ptr = leak_tank_show_habitat();
        let neighbor_ptr = leak_tank_show_habitat();
        set_show_neighbors(self_ptr, &[neighbor_ptr]);

        let plan = unsafe { ref_from_memory::<ZTHabitat>(self_ptr) }.portal_dispatch_plan();
        assert!(plan.is_empty());
    }

    /// Two neighbors: entries appear in `walk_neighbor_tree` order, one pair's worth per neighbor.
    #[test]
    fn portal_dispatch_plan_multiple_neighbors_preserve_walk_order() {
        let self_ptr = leak_tank_show_habitat();
        let neighbor_a = leak_tank_show_habitat();
        let neighbor_b = leak_tank_show_habitat();
        set_show_neighbors(self_ptr, &[neighbor_a, neighbor_b]);
        set_portal_map_entries(self_ptr, &[(neighbor_a, 0xa000_0001), (neighbor_b, 0xb000_0001)]);

        let plan = unsafe { ref_from_memory::<ZTHabitat>(self_ptr) }.portal_dispatch_plan();
        let fences: Vec<u32> = plan.iter().map(|d| d.fence_ptr).collect();
        assert_eq!(fences, vec![0xa000_0001, 0xb000_0001]);
    }

    /// `ZTHabitat_resize.asm`'s null-argument default: the owned-tile sentinel's own `next` node's
    /// payload - `leak_tile_list` splices at the front, so of two tiles the second ends up first.
    #[test]
    fn resize_default_tile_is_first_owned_tile() {
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[0x1000, 0x2000], &[]);
        assert_eq!(habitat.default_resize_tile_ptr(), 0x2000);
    }

    /// A single-tile habitat's default is that tile.
    #[test]
    fn resize_default_tile_single_tile_habitat() {
        let habitat = fixture_habitat_with_tiles_and_neighbors(&[0x1000], &[]);
        assert_eq!(habitat.default_resize_tile_ptr(), 0x1000);
    }

    /// `ZTHabitat_isTank.c`/`.asm` - the base virtual is a constant `false` regardless of `self`.
    #[test]
    fn base_default_is_constant_false() {
        assert!(!fixture_habitat(0x00632100).is_tank_base_default());
        assert!(!fixture_habitat(ZTHabitat::TANK_VTABLE_PTR).is_tank_base_default());
    }

    /// `is_tank()` reads the object's first word (the real vftptr slot - offset 0) and answers
    /// `true` exactly for the `ZTTankExhibit` vtable (`0x006312bc`), `false` for the plain
    /// `ZTHabitat` vtable (`0x00632100`, per `private/docs/vtables/ZTHabitat.md`).
    #[test]
    fn vtable_identity_check_distinguishes_tank_exhibit() {
        assert_eq!(offset_of!(ZTHabitat, vtable), 0);
        assert_eq!(offset_of!(ZTTankExhibit, habitat), 0);

        assert!(!fixture_habitat(0x00632100).is_tank());
        assert!(!fixture_habitat(0).is_tank());

        let mut tank: ZTTankExhibit = unsafe { mem::zeroed() };
        tank.habitat.vtable = ZTHabitat::TANK_VTABLE_PTR;
        assert!(tank.is_tank());
    }

    /// A zeroed [`ZTHabitat`] whose `building_list_begin`/`_end` span `entries` (leaked so the raw
    /// begin/end pointers stay valid) - the exact vector shape [`ZTHabitat::has_bldg`] reads and
    /// [`ZTHabitat::add_to_building_list`]'s own dedup scan reproduces (parameterized over an
    /// arbitrary `other_ptr+0x78`/`+0x7c` pair instead of `self`'s own fields).
    fn fixture_habitat_with_building_list(entries: &[u32]) -> ZTHabitat {
        let mut habitat: ZTHabitat = unsafe { mem::zeroed() };
        let block: &'static mut [u32] = Box::leak(entries.to_vec().into_boxed_slice());
        habitat.building_list_begin = block.as_ptr() as u32;
        habitat.building_list_end = block.as_ptr() as u32 + block.len() as u32 * 4;
        habitat
    }

    /// `ZTHabitat_hasBldg.c`'s own membership scan - also the exact logic
    /// [`ZTHabitat::add_to_building_list`]'s dedup check reuses (parameterized over `other_ptr`
    /// instead of `self`), so this doubles as coverage for that shared shape.
    #[test]
    fn has_bldg_finds_present_entry_and_rejects_absent_one() {
        let habitat = fixture_habitat_with_building_list(&[0x1000, 0x2000, 0x3000]);
        assert!(habitat.has_bldg(0x2000));
        assert!(!habitat.has_bldg(0x4000));
    }

    /// An empty building list (`begin == end`) never matches anything, including a null probe -
    /// the same empty-vector early-out `add_clear_tiles`'s own `empty_animals_vector_returns_false`
    /// documents for a sibling vector shape.
    #[test]
    fn has_bldg_empty_list_never_matches() {
        let habitat = fixture_habitat_with_building_list(&[]);
        assert!(!habitat.has_bldg(0x1000));
        assert!(!habitat.has_bldg(0));
    }

    /// Zeroed [`ZTHabitat`] with [`ZTHabitat::deterioration`] preseeded - a plain field write on
    /// the owned stack local, same shape [`fixture_habitat`]'s own `vtable` write uses (the port's
    /// `write_live!` reads/writes the very same address).
    fn fixture_habitat_with_deterioration(current: u32) -> ZTHabitat {
        let mut habitat = fixture_habitat(0);
        habitat.deterioration = current;
        habitat
    }

    /// Level `0` forces the field to `0` from any current level, returning the resulting value
    /// (vanilla returns the full `EAX` `dword`, `.asm`-confirmed).
    #[test]
    fn set_deterioration_zero_forces_zero() {
        for current in [0u32, 1, 2] {
            let habitat = fixture_habitat_with_deterioration(current);
            assert_eq!(habitat.set_deterioration(0), 0, "current {current}");
            assert_eq!(habitat.deterioration, 0, "current {current}");
        }
    }

    /// Level `1` applies only when the current value isn't already `2` - vanilla never lowers a
    /// `2` to a `1` (the `.c`'s `mbr_0x134 != 2` guard).
    #[test]
    fn set_deterioration_one_never_lowers_two() {
        let habitat = fixture_habitat_with_deterioration(2);
        assert_eq!(habitat.set_deterioration(1), 2);
        assert_eq!(habitat.deterioration, 2);

        for current in [0u32, 1] {
            let habitat = fixture_habitat_with_deterioration(current);
            assert_eq!(habitat.set_deterioration(1), 1, "current {current}");
            assert_eq!(habitat.deterioration, 1, "current {current}");
        }
    }

    /// Level `2` always forces `2`.
    #[test]
    fn set_deterioration_two_forces_two() {
        for current in [0u32, 1, 2] {
            let habitat = fixture_habitat_with_deterioration(current);
            assert_eq!(habitat.set_deterioration(2), 2, "current {current}");
            assert_eq!(habitat.deterioration, 2, "current {current}");
        }
    }

    /// Any level other than `0`/`1`/`2` (full 32-bit compares in the `.asm`, so `3` and
    /// `u32::MAX` both fall through) is a no-op - field and return both stay at the current value.
    #[test]
    fn set_deterioration_other_levels_are_no_op() {
        for level in [3u32, u32::MAX] {
            for current in [0u32, 1, 2] {
                let habitat = fixture_habitat_with_deterioration(current);
                assert_eq!(habitat.set_deterioration(level), current, "current {current} level {level}");
                assert_eq!(habitat.deterioration, current, "current {current} level {level}");
            }
        }
    }
}
