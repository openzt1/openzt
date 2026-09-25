use nt_time::{time::UtcDateTime, FileTime};
use openzt_detour::generated::{
        ambients::PLAY as AMBIENTS_PLAY,
        bfaimgr::CHECK_PATH as BFAIMGR_CHECK_PATH,
        bfentity::GET_TILE as BFENTITY_GET_TILE,bfmap::{GET_DIRECTION_0 as BFMAP_GET_DIRECTION_0, IS_CLOSE_DIRECTION as BFMAP_IS_CLOSE_DIRECTION},
        bftile::{
            IS_IN_ZOO as BFTILE_IS_IN_ZOO, VALIDATE_POSITIONS as BFTILE_VALIDATE_POSITIONS,
        },
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE, DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW, TILE_WITHIN_AVA},
        ztanimal::{CAN_SERVICE, IS_HUNGRY_AND_FOODLESS, IS_SICKLY, SET_FOOD, SET_KEEPER_ARRIVES, STOP_EATING},
        ztfence::{MAKE_FENCE as ZTFENCE_MAKE_FENCE, MAKE_GATE as ZTFENCE_MAKE_GATE},
        zthabitat::{
            GET_EVENTS, GET_SIZE, NEEDS_SERVICE,
            RECALCULATE_CHARACTERISTICS,
            REVISE_SPECIES_LIST, SEND_EVENT,
            UPDATE_PORTALS,
        },
        ztkeeper::CLEANS_UP,
        ztshowinfo::{CONSTRUCTOR_1 as ZTSHOWINFO_CONSTRUCTOR, DESTRUCTOR_1 as ZTSHOWINFO_DESTRUCTOR, SAVE as ZTSHOWINFO_SAVE},
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
use std::fmt;

use crate::{
    ambients::Ambients,
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, mut_from_memory, ref_from_memory, save_to_memory, ZTBufferString, ZTString},
    vanilla_vector::VanillaEventVector,
    zoostatus::ZooStatus,
    ztmapview::BFTile,
    ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG},
    ztshow::{call_entity_vtable_noargs, call_entity_vtable_u32_noargs, RVA_ANIMAL_TYPE_CHECK},
    ztshowinfo,
    ztworldmgr::Direction,
};
use super::mgr::zthabitatmgr::ZTHabitatMgr;
use super::support::*;

#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTHabitat {
    pub vtable: u32,                 // 0x000
    pub zt_show_info_ptr: u32,       // 0x004
    pub amphibious_neighbors_head: u32, // 0x008 // MSVC `std::set<ZTHabitat*>` head/sentinel node pointer for the amphibious-neighbor set - confirmed red-black-tree node layout (`+0x0`=color/isnil, `+0x4`=parent, `+0x8`=left, `+0xc`=right, `+0x10`=value) via `ZTHabitat_hiliteAmphibiousNeighbors.c`'s own in-order walk (see `walk_neighbor_tree`) and `addAmphibiousNeighbor`'s own STL insert helper (both left un-ported - see `Self::hilite_amphibious_neighbors`'s own doc comment). `ZTHabitat_getSize.c` independently walks this exact same field with identical node arithmetic - `getSize` itself remains out of this pass's scope, but this corrects `zthabitatmgr-implementation-plan.md`'s own earlier step 6h note (which speculated this was an unrelated nested-sub-habitat tree).
    pub pad1a_a1: [u8; 0x8],          // ----------------------- padding: 8 bytes
    pub show_neighbors_head: u32,    // 0x014 // Same shape as `amphibious_neighbors_head`, for the show-neighbor set (`ZTHabitat_hiliteShowNeighbors.c`/`addShowNeighbor`/`clearShowNeighbors`). The C decompiles mislabel this field `zoo_entrance_y` (an OOAnalyzer type-propagation artifact bleeding in a `ZTHabitatMgr`-shaped name) - trust the `.asm`-confirmed `+0x14` offset, named here for what it actually is.
    pub pad1a_a2: [u8; 0xd],          // ----------------------- padding: 13 bytes
    pub neighbor_dirty: u8,          // 0x025 // Set to 1 by `ZTHabitatMgr::habitatTileChanged`/`sceneryEntityChange` on every cached neighbor-habitat pointer found in a changed tile's own grid-cell row (see `ZTHabitatMgr::habitat_tile_changed`), and by the still-unported `ZTHabitat::recreateOAs`/`ZTHabitatMgr::pathRemoved`. No reader identified in this pass - real consumer not yet found in the decompile corpus.
    pub pad1a_b: [u8; 0x6],          // ----------------------- padding: 6 bytes
    pub unknown_flag_0x2c: u8,       // 0x02c // Gates ZTThought::ZTThought's acceptance of a passed-in habitat pointer (see ztthoughtmgr.rs); ZTHabitat::recalculateCharacteristics also early-returns when this is set. Meaning not otherwise confirmed.
    pub characteristics_dirty: u8,   // 0x02d // Gates the lazy `recalculateCharacteristics` call in getAttractiveness/hasKeeperAssigned (see ZTHabitat_getAttractiveness.c/ZTHabitat_hasKeeperAssigned.c) - distinct from unknown_flag_0x2c above.
    pub pad1b_a: [u8; 0x2],          // ----------------------- padding: 2 bytes
    pub unknown_flag_0x30: u8,       // 0x030 // Set to `1` by `ZTHabitatMgr::fenceRemoved` (`Self::fence_removed`) on either side of a removed fence whenever that side's own `unknown_flag_0x2c` is clear (i.e. it's a real, non-"world" habitat) - a third distinct flag alongside `unknown_flag_0x2c`/`characteristics_dirty`, never cleared or read anywhere in this pass's own scope, so its real meaning/consumer is unconfirmed.
    pub species_list_dirty: u8,      // 0x031 // Gates the lazy `reviseSpeciesList` call in update() once species_list_timer crosses its threshold - same dirty-flag/timer shape as characteristics_dirty/characteristics_timer, cleared by real vanilla reviseSpeciesList's own (still un-ported) body as a side effect.
    pub pad1b_b: [u8; 0x2],          // ----------------------- padding: 2 bytes
    pub viewing_areas_begin: u32,    // 0x034 // Begin pointer of the real vanilla std::vector<ZTViewingArea*> update() walks to tick each entry's own ambient state.
    pub viewing_areas_end: u32,      // 0x038
    pub viewing_areas_cap_end: u32,  // 0x03c // The vector's own capacity end - written by `ZTHabitat::addViewingArea`'s own growth path (see `Self::add_viewing_area`), previously undifferentiated padding.
    pub owned_tiles_ptr: u32,        // 0x040 // Pointer to the sentinel node of this habitat's owned-tile list (see TileListNode below), not a BFTile* itself - see getSize/removeHabitatTiles/validatePositions/resetUnitAI/createEdgePairs, all of which walk it identically.
    pub pad2a1: [u8; 0x4],            // ----------------------- padding: 4 bytes (0x044 - a real field per `ZTHabitat_createViewingAreas.c`'s own use of it as a second tile-list-shaped container, but that function is deferred - see `Self::create_viewing_areas`'s absence - so this stays unidentified padding rather than a guessed name/shape)
    pub boundary_tile_pairs_begin: u32, // 0x048 // Begin pointer of the real vanilla `std::vector<std::pair<BFTile*,BFTile*>>` `updateAmphibiousNeighbors_1`/`updateShowNeighbors_1`/`doShowCheck` each independently snapshot-and-iterate (see `ZTHabitatMgr::update_amphibious_neighbors`/`update_show_neighbors`/`do_show_check`), and `ZTHabitat::createEdgePairs` (see `Self::create_edge_pairs`) populates - confirmed via all these decompiles reading/writing the identical `field_0x48`/`field_0x4c`/`field_0x50` triple.
    pub boundary_tile_pairs_end: u32,   // 0x04c
    pub boundary_tile_pairs_cap_end: u32, // 0x050 // The vector's own capacity end - written by `Self::create_edge_pairs`'s own growth path, previously undifferentiated padding.
    pub ambients_begin: u32,         // 0x054 // Begin pointer of the real vanilla std::vector<(u32, Ambients*)> update() walks to play each entry's own ambient sound - see viewing_areas_begin's own doc comment for the same only-begin/end-modeled reasoning.
    pub ambients_end: u32,           // 0x058
    pub pad2b_a: [u8; 0x4],          // ----------------------- padding: 4 bytes (cap_end of the ambients vector above)
    pub species_list_begin: u32,     // 0x060 // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSpeciesList exposes (`&this->field_0x60`) - lazily recalculated via the same characteristics_dirty gate as get_attractiveness/has_keeper_assigned. See Self::species_list.
    pub species_list_end: u32,       // 0x064
    pub pad2b_b1: [u8; 0x4],         // ----------------------- padding: 4 bytes (cap_end of the species-list vector above)
    pub all_animals_begin: u32,      // 0x06c // Begin pointer of the real vanilla std::vector<ZTAnimal*> ZTHabitat::getAllAnimals exposes (`&this->field_0x6c`, `.asm`-confirmed `LEA EAX,[ESI+0x6c]`) - unlike species_list_begin/end this vector is never recalculated here, only optionally sorted in place by Self::get_all_animals.
    pub all_animals_end: u32,        // 0x070
    pub all_animals_cap_end: u32,    // 0x074 // The animals vector's own capacity end - never observed written by any function ported so far (`get_all_animals`'s own sort never grows it), carried here for completeness alongside the vector's other two fields.
    pub building_list_begin: u32,    // 0x078 // Begin pointer of a real vanilla std::vector<BFEntity*> - `ZTHabitat::hasBldg` (`Self::has_bldg`) is this vector's only reader ported so far; its writer, `ZTHabitat::addToBuildingList`, is still un-ported (mac-only, no Windows decompile as of this pass - see `zthabitatmgr-implementation-plan.md`'s "Guest/animal-experience queries" bulk).
    pub building_list_end: u32,      // 0x07c
    pub building_list_cap_end: u32,  // 0x080 // The building-list vector's own capacity end - never written by any function ported so far.
    pub characteristics_timer: u32,  // 0x084 // Elapsed-time accumulator update() advances every tick; past 6999 sets characteristics_dirty and rerolls to a random 0..200 value via the shared game RNG.
    pub species_list_timer: u32,     // 0x088 // Same shape as characteristics_timer, gating species_list_dirty/reviseSpeciesList at threshold 7999.
    pub entrance_tile_ptr: u32,      // 0x08c
    pub entrance_rotation: u32,      // 0x090
    pub num_animals: i32,            // 0x094 // ZTHabitat::getNumAnimals's own cached direct-occupant count (`.asm`-confirmed `MOV EAX,[EDI+0x94]`) - not itself gated by characteristics_dirty/recalculateCharacteristics (unlike attractiveness/has_keeper_assigned_raw), so presumably kept current some other way (e.g. incremented directly wherever an animal is added/removed) not otherwise traced by this pass.
    pub num_angry_animals: i32,      // 0x098 // ZTHabitat::getNumAngryAnimals's cached angry-animal count (`.asm`-confirmed `MOV %EBX,[EDI+0x98]`) - zeroed and re-tallied by recalculateCharacteristics's own animal census (one increment per animal whose `+0x3aa` flag byte is set, per ZTHabitat_recalculateCharacteristics.c). See Self::get_num_angry_animals.
    pub pad3a_a: [u8; 0x4],          // ----------------------- padding: 4 bytes (0x09c - recalculateCharacteristics's census increments it once per animal ZTAnimal::isUnhappyForReproduction reports true for, but no reader appears anywhere in the corpus yet)
    pub num_sick_animals: i32,       // 0x0a0 // ZTHabitat::getNumSickAnimals's cached sick-animal count - zeroed and re-tallied by the same recalculateCharacteristics census pass (one increment per animal whose `+0x3a7` flag byte is set). See Self::get_num_sick_animals.
    pub pad3a_b: [u8; 0x4],          // ----------------------- padding: 4 bytes (0x0a4 - recalculateCharacteristics's census increments it once per animal ZTAnimal::isHungry reports true for, but no reader appears anywhere in the corpus yet)
    pub keeper_food_category_amounts: [i32; 16], // 0x0a8 // Per-category cached "amount of keeper food left out" tally (`ZTHabitat_getAmountKeeperFood.c`'s own `&this->field_0xa8 + category * 4`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same `18-factor per-tile suitability` recalculation `zthabitatmgr-implementation-plan.md`'s step 6m documents as this function's own writer. See Self::get_amount_keeper_food.
    pub avg_animal_happiness: i32,   // 0x0e8 // ZTHabitat::getAvgAnimalHappiness's cached average animal happiness (`.asm`-confirmed `MOV %EAX,[ESI+0xe8]`) - zeroed (when num_animals is 0) or set to sum-of-happiness/num_animals by recalculateCharacteristics's own animal census (one `animal+0x18` happiness read per animal, per ZTHabitat_recalculateCharacteristics.c). See Self::get_avg_animal_happiness.
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
    pub unknown_nt_time: FileTime,   // 0x128
    pub pad5a: [u8; 0x1],            // ----------------------- padding: 1 byte
    pub has_keeper_assigned_raw: u8, // 0x131 // ZTHabitat::hasKeeperAssigned's cached result, recomputed by recalculateCharacteristics when characteristics_dirty is set.
    pub is_being_serviced_raw: u8,   // 0x132 // ZTHabitat::isBeingServiced's cached result (`this->mbr_0x132`), recomputed by recalculateCharacteristics when characteristics_dirty is set - same shape as has_keeper_assigned_raw. See Self::is_being_serviced.
    pub pad5b1b: [u8; 0x9],          // ----------------------- padding: 9 bytes (0x133-0x13c, not yet reverse-engineered)
    pub surrounding_species_begin: u32, // 0x13c // Begin pointer of the real vanilla std::vector<catalog-entry*> ZTHabitat::getSurroundingSpecies exposes (`&this->field_0x13c`, `.asm`-confirmed `LEA EAX,[ESI+0x13c]`), same characteristics_dirty-gated lazy-recalculate shape as species_list_begin/end. Populated by the still-un-ported ZTHabitat::constructSurroundingSpeciesList (unions this habitat's own amphibious/show neighbors' species lists - see zthabitatmgr-implementation-plan.md's step 6f notes), called internally by recalculateCharacteristics.
    pub surrounding_species_end: u32,   // 0x140
    pub pad5b2: [u8; 0x10],          // ----------------------- padding: 16 bytes (0x144-0x154: cap_end of the vector above at 0x144, then a msvc_std::map<int, ZTHabitatSuitabilityRecord> tree handle (node pointer + count) at 0x148/0x14c - ZTHabitat's own per-species suitability-scoring cache, keyed by ZTAnimalType::species and holding a 112-byte scoring record (elevation range, neighbor-habitat-tile count, per-scenery/building BFCategory scores, tank water-level min/max, ~15 sub-factors), whole-tree-replaced at the end of every recalculateCharacteristics pass (see ZTHabitat::speciesSuitabilityCache_alloc/_clear/_dtor and cls_0x40143b-disambiguation-handover.md). ZTHabitatSuitabilityRecord's own internal field layout is not yet reverse-engineered.)
    pub exhibit_name: ZTBufferString, // 0x154 // 3-pointer (start/end/buffer_end) buffer string - ZTHabitat::ZTHabitat zero-inits all of field_0x154/0x158/0x15c before allocating, and field_0x160 is a distinct, separately-referenced pointer (ZTHabitat::playShowStartSound etc.) right after it. Was previously mis-typed as the 2-pointer ZTBoundedString, which shifted every field below 4 bytes early.
    pub start_sound_ptr: u32,        // 0x160 // Real vanilla SNDSound* for the configured `[sounds] startSound`, built/acquired by set_is_show_exhibit and torn down by set_is_not_show_exhibit.
    pub end_sound_ptr: u32,          // 0x164 // Real vanilla SNDSound* for the configured `[sounds] endSound` - see start_sound_ptr.
    pub tank_walk_visited_marker: u8, // 0x168 // Scratch cycle-detection flag shared by `getOutermostTank`/`getNeedyNestedTank`'s own gate-chain/boundary-pair walks (`ZTHabitat_getOutermostTank.c`/`.asm`, `ZTHabitat_getNeedyNestedTank.c`/`.asm`): set to `1` on entry, checked before recursing into a candidate neighbour to stop a cycle. Real meaning outside these two calls unconfirmed; part of the ctor's own 0x168-0x178 zero-init range (see pad6's own note).
    pub pad6: [u8; 0xf],             // ----------------------- padding: 15 bytes (0x169-0x178: the real head is a msvc_std::tree36 sentinel at +0x16c the ctor zero-inits and ~ZTHabitat tears down via tree36::clear/_Erase, not yet individually reverse-engineered)
}

// `ZTHabitatMgr::createHabitat` (`ZTHabitatMgr_createHabitat.c`) allocates a plain `ZTHabitat` with
// `operator_new(0x178)` and a `ZTTankExhibit` (single-inheritance subclass) with a *separate*
// `operator_new(0x1e8)` - i.e. `ZTHabitat` itself is only 0x178 bytes; `tank_height`/`water_level`/
// `is_filled` (used to live directly on this struct) only exist on the derived `ZTTankExhibit`, from
// 0x178 onward - see that struct below. Putting them here made every full-struct copy of a real,
// non-tank habitat (`get_from_memory::<ZTHabitat>`) over-read past its true 0x178-byte allocation.
const _: () = assert!(std::mem::size_of::<ZTHabitat>() == 0x178);

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

impl ZTHabitat {
    pub(crate) const TANK_VTABLE_PTR: u32 = 0x006312bc;
    pub fn get_gate_tile_in(&self) -> Option<BFTile> {
        if self.entrance_tile_ptr == 0 {
            return None;
        }
        // info!("ZTHabitat: {}", self);
        // info!("Entrance tile ptr: {:#x}", self.entrance_tile_ptr);
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);
        // info!("Entrance tile: {}", tile);

        let zthm = globals().zthabitatmgr();
        if let Some(gate_habitat) = zthm.get_habitat_by_tile(&tile)
            && gate_habitat == *self {
                return Some(tile);
            }
        let ztwm = globals().ztworldmgr();
        ztwm.get_neighbour(&tile, Direction::from(self.entrance_rotation))
    }

    /// Ports `ZTHabitat::getGateTileOut` (`ZTHabitat_getGateTileOut.c`) - the exact mirror of
    /// [`Self::get_gate_tile_in`]'s own two branches: when the entrance tile currently belongs to this
    /// habitat, "out" is the neighbour tile stepped through the gate (vs. "in"'s own tile itself); when
    /// it doesn't, "out" is the entrance tile itself (vs. "in"'s own neighbour).
    pub fn get_gate_tile_out(&self) -> Option<BFTile> {
        if self.entrance_tile_ptr == 0 {
            return None;
        }
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);

        let zthm = globals().zthabitatmgr();
        if let Some(gate_habitat) = zthm.get_habitat_by_tile(&tile)
            && gate_habitat == *self {
                let ztwm = globals().ztworldmgr();
                return ztwm.get_neighbour(&tile, Direction::from(self.entrance_rotation));
            }
        Some(tile)
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
        let gate_tile_ptr = self
            .get_gate_tile_in()
            .map(|tile| globals().ztworldmgr().get_ptr_from_bftile(&tile))
            .unwrap_or(0);
        Self::get_adjacent_clear_tile(unit_ptr, gate_tile_ptr)
    }

    /// Ports `ZTHabitat::getGateTilePassOut` (`ZTHabitat_getGateTilePassOut.c`/`.asm`,
    /// `generated.rs`'s `GET_GATE_TILE_PASS_OUT`) - [`Self::get_gate_tile_pass_in`]'s exact mirror
    /// with "out" in place of "in" ([`Self::get_gate_tile_out`]). Same ride-through `EAX` return,
    /// consumed by `ZTGuest`/`ZTGuide`/`ZTStaff::pickRandomDest` (`ZTGuest_pickRandomDest.asm`'s own
    /// `TEST EAX, EAX` / `ADD EAX, 0x34` reads the picked tile's position fields straight out of the
    /// return). Same dead `MOV ECX, ESI` in the `.asm`, same null-gate pass-through.
    pub fn get_gate_tile_pass_out(&self, unit_ptr: u32) -> u32 {
        let gate_tile_ptr = self
            .get_gate_tile_out()
            .map(|tile| globals().ztworldmgr().get_ptr_from_bftile(&tile))
            .unwrap_or(0);
        Self::get_adjacent_clear_tile(unit_ptr, gate_tile_ptr)
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
            unsafe { mut_from_memory::<Self>(current_ptr) }.tank_walk_visited_marker = 1;
            let gate_tile = unsafe { ref_from_memory::<Self>(current_ptr) }.get_gate_tile_out();
            let next_ptr = match gate_tile {
                Some(tile) => mgr.get_habitat_ptr(tile.pos.x, tile.pos.y),
                None => 0,
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
    /// search over the amphibious-connected tank chain (via [`Self::boundary_tile_pairs_begin`]/`_end`,
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
        unsafe { mut_from_memory::<Self>(self_ptr) }.tank_walk_visited_marker = 1;

        if !self.is_tank() {
            return 0;
        }

        if low_byte_bool(unsafe { NEEDS_SERVICE.original()(self_ptr as *const u32, keeper_ptr as *const u32, true) }) {
            let keepers_habitat = unsafe { ZTUNIT_GET_HABITAT.original()(keeper_ptr as *const u32) } as u32;
            if keepers_habitat != self_ptr {
                return self_ptr;
            }
        }

        let mgr = globals().zthabitatmgr();
        let ztwm = globals().ztworldmgr();
        let pairs = ZTHabitatMgr::snapshot_boundary_tile_pairs(self.boundary_tile_pairs_begin, self.boundary_tile_pairs_end);
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
            let gate_tile_out_ptr = match candidate.get_gate_tile_out() {
                Some(tile) => ztwm.get_ptr_from_bftile(&tile),
                None => 0,
            };
            if gate_tile_out_ptr != first_ptr {
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
        match Direction::from(direction_raw) {
            Direction::North => tile.north_fence,
            Direction::East => tile.east_fence,
            Direction::South => tile.south_fence,
            Direction::West => tile.west_fence,
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

    /// Ports `ZTHabitat::moveGateTo`'s internal 2-arg helper (`ZTHabitat_moveGateTo_0.c`/`.asm`,
    /// `generated.rs`'s `MOVE_GATE_TO_0` at `0x0046616c` - the C decompile's own struct-offset math is
    /// garbled, e.g. the final `setName` call's real target is confirmed via `.asm` as vtable `+0x1c`, not
    /// the decompile's own nested-base guess). Demotes this habitat's current gate (still called via
    /// [`GET_GATE`]`.original()` here, not [`Self::get_gate`] - see that method's own doc comment) back
    /// into a plain fence and promotes `candidate_fence_ptr` into the new gate -
    /// updating [`Self::entrance_tile_ptr`]/[`Self::entrance_rotation`] and giving the new gate this
    /// habitat's own name - but only when `candidate_fence_ptr` genuinely separates two different
    /// habitats (its own tile and the neighbour stepped through by its own rotation belong to different
    /// [`ZTHabitatMgr::get_habitat_ptr`] owners). No-op (`false`) for a null candidate or one that doesn't
    /// satisfy that check.
    ///
    /// **Must only be called on a live `ZTHabitat` reference** - `self`'s address is written into
    /// directly (`entrance_tile_ptr`/`entrance_rotation`) and passed to real vanilla `getGate`/`setName`,
    /// same precondition [`Self::get_attractiveness`] documents.
    ///
    /// **Correctness bug fixed (2026-09-17).** Two independent bugs found via live crash-capture, both
    /// now fixed:
    ///
    /// 1. [`Self::move_gate_to`]'s own candidate-discovery previously used
    ///    [`Self::tile_fence_in_direction`] (a `Direction`-enum match, `0` for any non-cardinal value) to
    ///    find `candidate_fence_ptr` - but real vanilla `ZTHabitat_moveGateTo_1.c` resolves it via the
    ///    same raw `direction/2` array-index scheme [`Self::get_gate`] already uses
    ///    (`*(&tile->field_0x14 + (direction/2)*4)`), which - unlike `tile_fence_in_direction` - still
    ///    resolves a valid `0..=3` slot for a diagonal/non-cardinal `EDirection` (integer-truncated onto
    ///    the preceding cardinal's own slot) instead of treating it as "no candidate". Fixed by switching
    ///    `move_gate_to` to [`Self::fence_slot_by_index`], matching `get_gate`'s own approach.
    /// 2. This function's own real vanilla body (`ZTFence::makeGate` -> `setHealthy` ->
    ///    `dirtyHabitatEscapability`) unconditionally dereferences `+0x2c` on whatever
    ///    `ZTHabitatMgr::get_habitat_ptr` returns for `candidate_fence_ptr`'s own tile, with **no null
    ///    guard at all** (`mov al, byte ptr [esi+0x2c]` with `esi=0`) - a genuine bug in real vanilla
    ///    itself, confirmed directly in `ZTFence_dirtyHabitatEscapability.c`. Live crash-captured twice by
    ///    calling this directly (bypassing `move_gate_to`'s own candidate discovery) with a habitat's own
    ///    current gate as `candidate_fence_ptr`: that fence's own tile (per `BFEntity::getTile`) is the
    ///    *outside* tile, owned by no habitat, while the neighbour stepped through by its rotation is the
    ///    *inside* tile the habitat itself owns - `owner == 0`, `neighbour_owner != 0`, so the
    ///    `owner != neighbour_owner` check alone let it through into the unguarded real vanilla
    ///    dereference. Guarded here the same "dead in practice, defend anyway" way this file already
    ///    handles other unguarded-real-vanilla-deref edge cases (e.g. [`Self::get_outermost_tank`]):
    ///    require `owner` itself to be non-null before proceeding.
    ///
    /// Ports `ZTHabitat::moveGateTo(ZTFence*)` (`ZTHabitat_moveGateTo_0.c`/`.asm`, `MOVE_GATE_TO_0` at `0x0046616c`).
    pub fn move_gate_to_fence(&self, candidate_fence_ptr: u32) -> bool {
        if candidate_fence_ptr == 0 {
            return false;
        }
        let zthm = globals().zthabitatmgr();
        let world = globals().ztworldmgr();

        let tile_ptr = unsafe { BFENTITY_GET_TILE.original()(candidate_fence_ptr as *const u32) } as u32;
        let rotation: u32 = get_from_memory(candidate_fence_ptr + 0x12c);
        let neighbour_ptr = if tile_ptr != 0 { get_neighbour_ptr(world, tile_ptr, Direction::from(rotation)) } else { 0 };

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
    /// `ZTHabitat_getAttractiveness.c`, same shape on both platforms): lazily recomputes via the real,
    /// still-un-ported `recalculateCharacteristics` when `characteristics_dirty` is set, then returns the
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.attractiveness
    }

    /// Ports `ZTHabitat::hasKeeperAssigned` (`ZTHabitat_hasKeeperAssigned.c`) - same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::get_attractiveness`]; see that
    /// method's doc comment for the live-reference precondition this one shares.
    pub fn has_keeper_assigned(&self) -> bool {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let begin = self.species_list_begin;
        let end = self.species_list_end;
        (begin..end).step_by(4).map(get_from_memory::<u32>)
    }

    /// Ports `ZTHabitat::getSurroundingSpecies` (`ZTHabitat_getSurroundingSpecies.c`/`.asm`): same
    /// `characteristics_dirty`-gated lazy-recalculate shape as [`Self::species_list`], yielding raw
    /// catalog-entry pointers from `surrounding_species_begin`/`_end` (`&this->field_0x13c`,
    /// `.asm`-confirmed `LEA EAX,[ESI+0x13c]`) instead. Populated by the still-un-ported
    /// `ZTHabitat::constructSurroundingSpeciesList` (unions this habitat's own amphibious- and
    /// show-neighbor sets' [`Self::species_list`]s - see `zthabitatmgr-implementation-plan.md`'s step 6f
    /// notes for why that union-builder itself is left un-ported this pass), reached transitively through
    /// the same `.original()` `recalculateCharacteristics` call-through below - no separate call-through
    /// is needed here for that reason.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::species_list`].
    pub fn surrounding_species(&self) -> impl Iterator<Item = u32> {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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

    /// Ports `ZTHabitat::getRandomTile` (`ZTHabitat_getRandomTile.c`/`.asm`, `generated.rs`'s
    /// `GET_RANDOM_TILE`): picks a random tile from the habitat's owned-tile list. Gets the count
    /// through the same `getSize(this, false)` call real vanilla makes ([`GET_SIZE`] undetoured
    /// `.original()` call-through, the same shape [`Self::fence_removed`]'s neighbor-size reads use -
    /// its false arm is exactly the owned-tile node count [`walk_tile_list`] walks), bails null on an
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
        let count = unsafe { GET_SIZE.original()(self as *const Self as *const u32, false) };
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
        let gate_tile_ptr = self
            .get_gate_tile_in()
            .map(|tile| globals().ztworldmgr().get_ptr_from_bftile(&tile))
            .unwrap_or(0);
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        let mut total = self.keeper_food_category_amounts[category as usize];
        if include_neighbors {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                total += unsafe { ref_from_memory::<ZTHabitat>(neighbor_ptr) }.get_amount_keeper_food(category, false);
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
        if self.is_tank() {
            return;
        }
        let self_addr = self as *const Self as u32;
        for node in walk_tile_list(self.owned_tiles_ptr) {
            let tile_ptr = get_from_memory::<TileListNode>(node).payload;
            let entity_ptr: u32 = get_from_memory(tile_ptr + 0x10);
            if entity_ptr == 0 || !unsafe { entity_type_matches(entity_ptr, RVA_SCENERY_TYPE_CHECK_ARG) } {
                continue;
            }
            let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
            let flag_a: u32 = get_from_memory(entity_type_ptr + 0x11c);
            let flag_b: u8 = get_from_memory(entity_type_ptr + 0x12c);
            if flag_a != 0 || flag_b != 0 {
                unsafe { SEND_EVENT.original()(self_addr as *const u32, 0x2730, 0, 0x4d, tile_ptr, 0, 1) };
            }
        }
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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

    /// Ports `ZTHabitat::removeViewingAreas` (`ZTHabitat_removeViewingAreas.c`, `generated.rs`'s
    /// `REMOVE_VIEWING_AREAS`): a no-op if `unknown_flag_0x2c` is set (the same guard
    /// `recalculateCharacteristics`'s own early-return uses). Otherwise destroys and frees every entry in
    /// [`Self::viewing_areas_begin`]/`_end` (call-through to real vanilla `ZTViewingArea::~ZTViewingArea` +
    /// `operator_delete`, same as [`Self::remove_viewing_area`]), then resets `_end` back to `_begin` -
    /// keeps the backing buffer rather than freeing it, unlike a full vector destructor.
    ///
    /// Must only be called on a live `ZTHabitat` reference - `self`'s own address feeds the vector field
    /// write at the end.
    pub fn remove_viewing_areas(&mut self) {
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
    pub fn remove_species(&mut self, species_key: u32) {
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
                self.ambients_end -= 0x8;
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
    pub fn set_dirty_characteristics(&mut self) {
        if self.characteristics_dirty != 0 {
            return;
        }
        self.characteristics_dirty = 1;
        for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
        for node in walk_neighbor_tree(self.show_neighbors_head) {
            let neighbor_ptr: u32 = get_from_memory(node + 0x10);
            unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_dirty_characteristics();
        }
    }

    /// Ports `ZTHabitat::acceptDonation` (`ZTHabitat_acceptDonation.c`): adds `amount` to both
    /// `current_donations`/`total_donations`, then calls the already-ported
    /// [`crate::zoostatus::ZooStatus::increase_donations`]/[`crate::ztgamemgr::ZTGameMgr::add_cash`]
    /// directly on the real global `ZTGameMgr`/its embedded `ZooStatus` sub-object (`this + 0x10`) - the
    /// same `&GLOBAL_ZTGameMgr->field_0x10` access `zoostatus.rs`'s own `f_grant_donation` already
    /// establishes for this exact call pair, reused here rather than a real-vanilla call-through since
    /// both real methods are already faithful, live-tested Rust ports.
    pub fn accept_donation(&mut self, amount: f32) {
        self.current_donations += amount;
        self.total_donations += amount;
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
    pub fn set_time_last_serviced(&mut self, time: u32, propagate: bool) {
        self.time_last_serviced = time;
        if propagate {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.set_time_last_serviced(time, false);
            }
        }
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
    pub fn trigger_keeper_arrived(&mut self, keeper_ptr: u32, scheduled: bool) {
        if self.characteristics_dirty != 0 {
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }
        self.scheduled_service_counter = (self.scheduled_service_counter.wrapping_sub(1)).max(0);
        for addr in (self.all_animals_begin..self.all_animals_end).step_by(4) {
            let animal_ptr: u32 = get_from_memory(addr);
            unsafe { SET_KEEPER_ARRIVES.original()(animal_ptr as *const u32, keeper_ptr as *const u32) };
        }
        if scheduled {
            for node in walk_neighbor_tree(self.amphibious_neighbors_head) {
                let neighbor_ptr: u32 = get_from_memory(node + 0x10);
                unsafe { mut_from_memory::<ZTHabitat>(neighbor_ptr) }.trigger_keeper_arrived(keeper_ptr, false);
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
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
    /// For every entry in this habitat's own [`Self::boundary_tile_pairs_begin`]/`_end` (a fresh
    /// snapshot via [`ZTHabitatMgr::snapshot_boundary_tile_pairs`], matching real vanilla's own
    /// copy-before-iterate shape), resolves the fence connecting the pair in each direction
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
        let pairs = ZTHabitatMgr::snapshot_boundary_tile_pairs(self.boundary_tile_pairs_begin, self.boundary_tile_pairs_end);
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
    /// water/salinity concept at all (only `ZTTankExhibit` does - see that class's own override,
    /// `zttankexhibit::IS_RIGHT_SALINITY`, left un-ported); the base implementation is a constant `true`.
    /// That slot's address (`0x00446995`, `generated.rs`'s `bfentity::VF_RETURN1_1`) is a shared
    /// `return true` stub other vtables also point at, so its detour runs for those classes too - this
    /// port never reads `self`, which keeps that harmless.
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

        if let Some(gate_tile) = self.get_gate_tile_out() {
            let gate_tile_ptr = globals().ztworldmgr().get_ptr_from_bftile(&gate_tile);
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
    pub fn remove_habitat_tiles(&mut self) {
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

        if let Some(gate_tile) = self.get_gate_tile_out() {
            let gate_tile_ptr = globals().ztworldmgr().get_ptr_from_bftile(&gate_tile);
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
    pub fn add_habitat_tiles(&mut self, seed_tile_ptr: u32) {
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
    pub fn set_is_show_exhibit(&mut self) {
        if self.zt_show_info_ptr != 0 {
            return;
        }

        let alloc = unsafe { OPERATOR_NEW.original()(0xa8) } as u32;
        let show_info = if alloc == 0 { 0 } else { unsafe { ZTSHOWINFO_CONSTRUCTOR.original()(alloc as *const u32) as u32 } };
        self.zt_show_info_ptr = show_info;

        let zt_show_mgr = globals().ztshowmgr_ptr() as *const u32;
        if zt_show_mgr.is_null() || show_info == 0 {
            return;
        }

        let registered = unsafe { REGISTER_SHOW.hooked()(zt_show_mgr, show_info as *const u32, true) };
        if !registered {
            unsafe { ZTSHOWINFO_DESTRUCTOR.original()(show_info as *const u32, 1) };
            self.zt_show_info_ptr = 0;
            return;
        }
        save_to_memory(show_info + 0xa0, self as *const Self as u32);

        let base = get_module_base("zoo.exe") as u32;
        let dx8_sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        let start_sound_name = base + START_SOUND_NAME_RVA;
        let end_sound_name = base + END_SOUND_NAME_RVA;

        if looks_like_configured_sound_name(start_sound_name) {
            self.start_sound_ptr = construct_and_acquire_sound(dx8_sndmgr, start_sound_name);
        }
        if looks_like_configured_sound_name(end_sound_name) {
            self.end_sound_ptr = construct_and_acquire_sound(dx8_sndmgr, end_sound_name);
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
    pub fn set_is_not_show_exhibit(&mut self) {
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
        self.zt_show_info_ptr = 0;

        teardown_sound(self.start_sound_ptr);
        self.start_sound_ptr = 0;
        teardown_sound(self.end_sound_ptr);
        self.end_sound_ptr = 0;
    }

    /// Ports `ZTHabitat::update` (vtable `+0x24`, `ZTHabitat_update.c`/`.asm`, confirmed against the
    /// macOS decompile's identical shape): plays every queued ambient sound
    /// (`ambients_begin`/`ambients_end`), advances the species-list/characteristics lazy-recalculate
    /// timers - rerolling each via the shared game RNG ([`lcg_next`]) and calling through to the
    /// still-un-ported real vanilla `reviseSpeciesList`/`recalculateCharacteristics` once its own
    /// threshold trips, the same dirty-flag/timer shape [`Self::get_attractiveness`] already relies on
    /// for `characteristics_dirty` - ticks every viewing area's own ambient state
    /// (`viewing_areas_begin`/`viewing_areas_end`), then calls through to the still-un-ported real
    /// vanilla `updatePortals` and finally this object's own, already-ported [`Self::listen`].
    ///
    /// `updatePortals` is a plain, non-virtual helper (not one of `ZTHabitat`'s 17 vtable slots) -
    /// deliberately left un-ported and called via `.original()`: its own body is an `isTank`-gated
    /// portal-list check this port doesn't need to understand to reproduce `update` itself.
    ///
    /// `ZTTankExhibit` overrides this vtable slot with its own, separate address (`0x0049625f`, out of
    /// scope for this pass per `zthabitatmgr-implementation-plan.md`) - detouring only the base
    /// `ZTHabitat::update` address never intercepts a real tank's own tick, the same base-only-override
    /// pattern [`Self::is_right_salinity`] already relies on.
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as
    /// [`Self::get_attractiveness`] - `self`'s own address is passed straight into every real vanilla
    /// call-through above.
    pub fn update(&mut self, elapsed: u32) {
        let mut ambient_entry = self.ambients_begin;
        while ambient_entry != self.ambients_end {
            let ambient: u32 = get_from_memory(ambient_entry + 4);
            unsafe { AMBIENTS_PLAY.original()(ambient as *const u32, elapsed as i32, 0x50) };
            ambient_entry += 8;
        }

        let species_list_timer = self.species_list_timer.wrapping_add(elapsed);
        self.species_list_timer = species_list_timer;
        self.characteristics_timer = self.characteristics_timer.wrapping_add(elapsed);

        if species_list_timer > 7999 {
            self.species_list_dirty = 1;
        }
        if self.species_list_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            self.species_list_timer = (rng >> 0x10 & 0x7fff) % 200;
            unsafe { REVISE_SPECIES_LIST.original()(self as *const Self as *const u32) };
        }

        if self.characteristics_timer > 6999 {
            self.characteristics_dirty = 1;
        }
        if self.characteristics_dirty != 0 {
            let rng_addr = get_module_base("zoo.exe") as u32 + GAME_RNG_RVA;
            let rng = lcg_next(get_from_memory::<u32>(rng_addr));
            save_to_memory(rng_addr, rng);
            self.characteristics_timer = (rng >> 0x10 & 0x7fff) % 200;
            unsafe { RECALCULATE_CHARACTERISTICS.original()(self as *const Self as *const u32) };
        }

        let mut viewing_area_entry = self.viewing_areas_begin;
        while viewing_area_entry != self.viewing_areas_end {
            let viewing_area: u32 = get_from_memory(viewing_area_entry);
            unsafe { ZTVIEWINGAREA_UPDATE_AMBIENTS.original()(viewing_area as *const u32, elapsed as i32) };
            viewing_area_entry += 4;
        }

        unsafe { UPDATE_PORTALS.original()(self as *const Self as *const u32) };
        self.listen();
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
            ok &= unsafe { ZTSHOWINFO_SAVE.original()(self.zt_show_info_ptr as *const u32, file) };
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

    /// Push-back helper for [`Self::create_edge_pairs`]'s own [`Self::boundary_tile_pairs_begin`]/`_end`/
    /// `_cap_end` vector - real vanilla's own `PoolAlloc::allocate`/`PoolAlloc::deallocate` doubling growth
    /// (`ZTHabitat_createEdgePairs.c`), called through rather than reimplemented, matching
    /// [`ZTHabitatMgr::add_habitat`]'s own established precedent for the identical allocator pair. The
    /// decompile's own general "insert in the middle" shift helper (`FUN_00411192`) never actually executes
    /// for this call site (insertion is always at the vector's own current end - the loop that would call it
    /// is unconditionally empty here), so it's sidestepped entirely rather than needing to be identified.
    pub(crate) fn push_boundary_tile_pair(&self, tile_a: u32, tile_b: u32) {
        let self_addr = self as *const Self as u32;
        let begin = self.boundary_tile_pairs_begin;
        let end = self.boundary_tile_pairs_end;
        let cap_end = self.boundary_tile_pairs_cap_end;

        if end == cap_end {
            let old_len = (end - begin) / 8;
            let new_cap = if old_len == 0 { 1 } else { old_len * 2 };
            let new_buf = unsafe { POOLALLOC_ALLOCATE.original()(new_cap * 8) } as u32;

            for i in 0..old_len {
                let a: u32 = get_from_memory(begin + i * 8);
                let b: u32 = get_from_memory(begin + i * 8 + 4);
                if new_buf != 0 {
                    save_to_memory(new_buf + i * 8, a);
                    save_to_memory(new_buf + i * 8 + 4, b);
                }
            }
            if new_buf != 0 {
                save_to_memory(new_buf + old_len * 8, tile_a);
                save_to_memory(new_buf + old_len * 8 + 4, tile_b);
            }
            // Real vanilla calls `PoolAlloc::deallocate` unconditionally here, even for `begin==0` (a
            // null-pointer, zero-length free) - matched as-is, same reasoning as `ZTHabitatMgr::add_habitat`'s
            // own identical unconditional-deallocate precedent.
            unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, cap_end - begin) };

            save_to_memory(self_addr + 0x48, new_buf);
            save_to_memory(self_addr + 0x4c, new_buf + (old_len + 1) * 8);
            save_to_memory(self_addr + 0x50, new_buf + new_cap * 8);
        } else {
            save_to_memory(end, tile_a);
            save_to_memory(end + 4, tile_b);
            save_to_memory(self_addr + 0x4c, end + 8);
        }
    }

    /// Ports `ZTHabitat::createEdgePairs` (`ZTHabitat_createEdgePairs.c`/`.asm`, `generated.rs`'s
    /// `CREATE_EDGE_PAIRS`): rebuilds [`Self::boundary_tile_pairs_begin`]/`_end` from scratch (real
    /// vanilla's own `end = begin` reset, keeping the existing buffer rather than deallocating it), then
    /// walks the owned-tile list ([`walk_tile_list`]) and, for every owned tile and each of its 4 cardinal
    /// neighbours ([`get_neighbour_ptr`]), pushes `(owned_tile, neighbour_tile)` ([`Self::push_boundary_tile_pair`])
    /// whenever the neighbour isn't owned by `self` - including when it doesn't exist (real vanilla's own
    /// address, `0` when out of map bounds).
    ///
    /// Must only be called on a live `ZTHabitat` reference, same precondition as [`Self::get_attractiveness`].
    ///
    /// Verified correct against real vanilla via a direct call
    /// (`ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE`), but its own `#[detour]` is deliberately **not**
    /// installed - see `hooks_zthabitatmgr::create_edge_pairs`'s own doc comment for the live-battery hang
    /// this caused when hooked, bisected directly against the other 8 stage-6h detours.
    pub fn create_edge_pairs(&mut self) {
        let self_addr = self as *const Self as u32;
        save_to_memory(self_addr + 0x4c, self.boundary_tile_pairs_begin);

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
    pub fn add_viewing_area(&mut self, va_ptr: u32) {
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
    pub fn remove_viewing_area(&mut self, va_ptr: u32) {
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
    pub fn remove_from_all_vas(&mut self, tile_ptr: u32) {
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
    pub fn path_placed(&mut self, tile_ptr: u32) {
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


impl PartialEq for ZTHabitat {
    fn eq(&self, other: &Self) -> bool {
        self.owned_tiles_ptr == other.owned_tiles_ptr
            && self.entrance_rotation == other.entrance_rotation
            && self.entrance_tile_ptr == other.entrance_tile_ptr
            && self.exhibit_name.copy_to_string() == other.exhibit_name.copy_to_string()
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
        writeln!(f, "  created_timestamp: {},", UtcDateTime::try_from(self.created_timestamp).unwrap())?;
        writeln!(
            f,
            "  unknown_nt_time: {} ({}, {}, {}),",
            UtcDateTime::try_from(self.unknown_nt_time).unwrap(),
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

