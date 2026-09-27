use getset::Getters;
use openzt_detour::generated::{bfcategory::GET_VALUE, zttankexhibit::IS_RIGHT_SALINITY};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::habitat::ZTHabitat;
use crate::util::LiveMemory;

/// `ZTTankExhibit`, the single-inheritance subclass of [`ZTHabitat`] real tank/show exhibits are
/// instantiated as (allocated via `operator_new(0x1e8)` by `ZTTankExhibit::cls_0x40183b`, confirmed at
/// the `.asm` level; see `ZTHabitat`'s own doc comment for the allocator bifurcation).
///
/// embeds the base class at offset 0 (matching real C++ single inheritance layout), so every
/// `ZTHabitat` field/method is reachable through it. Only ever construct this from a pointer already
/// confirmed via `ZTHabitat::is_tank()` - reading it from a real, plain `ZTHabitat` (0x178 bytes) would
/// over-read past that object's actual allocation, exactly the bug this struct split fixes.
///
/// `0x18c` onward was previously modeled as opaque padding; `ZTTankExhibit_isRightSalinity.c`/
/// `_update.c`/`_addRandomSparkle.c` (the last confirmed via Ghidra MCP identification of the former
/// `FUN_00496382`) prove almost all of it is real fields (sparkle-effect bookkeeping plus a
/// current/pending water-type pair), named below with the confidence each decompile actually supports -
/// several names are inferred from call shape, not confirmed by any setter/getter method name, and are
/// flagged as such.
#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTTankExhibit {
    pub habitat: ZTHabitat,   // 0x000 - 0x178
    pad_tank1: [u8; 0xc], // ----------------------- padding: 12 bytes (0x178-0x184)
    tank_height: u32,     // 0x184 // Actual structural tank height (ZTTankExhibit::getTankHeight/setTankHeight); not the field checkTankPlacement compares against, see water_level.
    water_level: u32,     // 0x188 // Current water level (ZTTankExhibit::getWaterLevel); this is what checkTankPlacement's height comparisons actually use. Also `addRandomSparkle`'s "tank has any water at all" gate (`!= 0`).
    current_water_type: i32, // 0x18c // Inferred, not confirmed by any method name: `isRightSalinity` flips its salinity comparison when this is `0` (read as a fresh/salt discriminant), and `update` overwrites it from `pending_water_type` exactly when the tank finishes draining (`water_level == 0`), immediately following with `updateAdjustmentCosts`+`fill` - i.e. this is the water type the tank is *currently* filled/configured for, applied lazily on drain rather than immediately on `setBaseTerrainType`.
    pending_water_type: i32, // 0x190 // Inferred: the requested water type staged by (the un-ported) `ZTTankExhibit::setBaseTerrainType`, compared against `current_water_type` and swapped in once the tank fully drains - see `current_water_type`'s own note.
    pad_tank2b: [u8; 0x4], // ----------------------- padding: 4 bytes (0x194-0x198, not read by isRightSalinity/update/addRandomSparkle)
    is_filled: bool,      // 0x198 // Set true by ZTTankExhibit::fill(), false by ZTTankExhibit::drain(). Confirmed against ZTTankExhibit_fill.c/ZTTankExhibit_drain.c/ZTMapView_checkTankPlacement.c (field_0x198, checked as `this_00->field_0x198 == '\0'`) - not flipped between platforms.
    pad_tank3a: [u8; 0xf], // ----------------------- padding: 15 bytes (0x199-0x1a8, not yet reverse-engineered)
    water_purity: i32,     // 0x1a8 // Inferred: current purity level - `update`'s countdown-expiry branch only fires while `> 0` and calls `setWaterPurity(this, water_purity - 1)` on expiry, matching the already-generated `zttankexhibit::SET_WATER_PURITY` setter's own shape (not yet decompiled to directly confirm it writes this exact offset).
    water_purity_timer: i32, // 0x1ac // Inferred: ticks down by `elapsed` each `update()` call; reloaded from `DAT_0063af04` and triggers the `water_purity` decrement above when it drops to `<= 0`.
    pad_tank3b: [u8; 0x14], // ----------------------- padding: 20 bytes (0x1b0-0x1c4, not yet reverse-engineered)
    sparkle_entities_begin: u32, // 0x1c4 // Inferred: begin pointer of a real vanilla `std::vector<std::pair<i32,BFEntity*>>` - `addRandomSparkle`'s own growth path (`cls_0x43f6a9::meth_0x43f6a9` when `end == cap_end`, else in-place `*end = 0; end[1] = entity`) is the same begin/end/cap_end vector-growth shape used elsewhere in this codebase (e.g. `ZTHabitat::viewing_areas_begin`), tracking each spawned sparkle entity this tank owns.
    sparkle_entities_end: u32,  // 0x1c8
    sparkle_entities_cap_end: u32, // 0x1cc
    sparkle_entity_ids_begin: u32, // 0x1d0 // Inferred: begin pointer of a parallel `std::vector<u32>` (`msvc_std::vector_pod<>::_Insert_n` growth path) storing each spawned sparkle's own `+0x124` id field, one entry per `sparkle_entities_*` entry above.
    sparkle_entity_ids_end: u32,   // 0x1d4
    sparkle_entity_ids_cap_end: u32, // 0x1d8
    salt_water_sparkle_vtable_cache: u32, // 0x1dc // Inferred: cached `BFEntity` vtable pointer for the "twaterwv"-named entity type (`BFWorldMgr::findTypes` result, looked up once and reused), constructed via its own vtable slot `+0x24` (a virtual-constructor idiom) when `addRandomSparkle` spawns a non-freshwater sparkle.
    fresh_water_sparkle_vtable_cache: u32, // 0x1e0 // Inferred: same shape as `salt_water_sparkle_vtable_cache`, for the "frshwav"-named entity type used when `addRandomSparkle`'s freshwater branch (`current_water_type == 0`) spawns instead.
    sparkle_spawn_timer: i32, // 0x1e4 // Inferred: ticks down by `elapsed` each `update()` call; on reaching `< 0`, `update` calls `addRandomSparkle` and reloads this from `DAT_006390b8 / owned_tile_count`.
    #[getset(skip)]
    pub _live: LiveMemory, // trailing ZST - see LiveMemory's own docs; required by this struct's real methods reading live game memory through &self (is_right_salinity now; update still pending).
}

const _: () = assert!(std::mem::size_of::<ZTTankExhibit>() == 0x1e8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, current_water_type) == 0x18c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, pending_water_type) == 0x190);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, is_filled) == 0x198);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity) == 0x1a8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity_timer) == 0x1ac);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entities_begin) == 0x1c4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entity_ids_begin) == 0x1d0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, salt_water_sparkle_vtable_cache) == 0x1dc);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, fresh_water_sparkle_vtable_cache) == 0x1e0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_spawn_timer) == 0x1e4);

impl std::ops::Deref for ZTTankExhibit {
    type Target = ZTHabitat;
    fn deref(&self) -> &ZTHabitat {
        &self.habitat
    }
}

/// `ZTAnimalType`'s embedded water-suitability category map head, at `animal_type + 0x2d8`
/// (`LEA ESI,[EAX+0x2d8]` in `ZTTankExhibit_isRightSalinity.asm`; the macOS build's own copy of the
/// function reads the equivalent `+0x2a8`). Kept a raw offset with a doc comment rather than a named
/// `ZTAnimalType` field, same precedent as the sibling `+0x2cc` site in
/// [`ZTHabitat::additional_scenery_suitability_change`]: it sits inside that struct's `pad11` region
/// (`openzt/src/bfentitytype/units.rs`) and is a *different* category map from `+0x2cc`'s.
const ANIMAL_TYPE_WATER_SUITABILITY_MAP_OFFSET: u32 = 0x2d8;

/// The two water-suitability category ids `isRightSalinity` compares (`PUSH 0xa` then `PUSH 0x9` at the
/// call sites). Raw ids only - no decompile names them, and which of them "means" fresh vs salt water is
/// unconfirmed either way, so don't guess.
const SALINITY_CATEGORY_10: i32 = 10;
const SALINITY_CATEGORY_9: i32 = 9;

impl ZTTankExhibit {
    /// Ports `ZTTankExhibit::isRightSalinity` (vtable `+0x28`, `ZTTankExhibit_isRightSalinity.c`/`.asm`,
    /// `generated.rs`'s `zttankexhibit::IS_RIGHT_SALINITY`) - the tank-specific override of the base
    /// [`ZTHabitat::is_right_salinity`] constant-`true` stub. Looks the animal type's two
    /// water-suitability category values up (real vanilla `BFCategory::getValue` - a pure map walk that
    /// answers `0` on a missing key, detoured nowhere, so `.original()` is a plain vanilla call in every
    /// build profile) and answers `category9 <= category10` (signed - real vanilla's `SETGE`), inverted
    /// when `current_water_type == 0` (the same freshwater discriminant `addRandomSparkle`'s freshwater
    /// branch reads). `animal_type_ptr` is the bare `ZTAnimalType*` (the `+0x128` inner-class pointer
    /// every other entity-type consumer here passes).
    pub fn is_right_salinity(&self, animal_type_ptr: u32) -> bool {
        let category_map = animal_type_ptr + ANIMAL_TYPE_WATER_SUITABILITY_MAP_OFFSET;
        let category_10 = unsafe { GET_VALUE.original()(category_map as *const u32, SALINITY_CATEGORY_10) };
        let category_9 = unsafe { GET_VALUE.original()(category_map as *const u32, SALINITY_CATEGORY_9) };
        Self::salinity_result(category_10, category_9, self.current_water_type)
    }

    /// [`Self::is_right_salinity`]'s comparison tail, split out so the (water type x comparison
    /// direction) combinations stay host-safe unit-testable - the `getValue` lookups themselves are real
    /// vanilla calls and are covered by the live comparison test instead.
    fn salinity_result(category_10: i32, category_9: i32, current_water_type: i32) -> bool {
        let suitable = category_9 <= category_10;
        if current_water_type == 0 { !suitable } else { suitable }
    }
}

/// `ZTTankExhibit::isRightSalinity`'s own fixed address (`0x004936df`) is referenced exactly once in the
/// whole binary - the `+0x28` slot of the tank vtable itself (`0x006312e4`; no direct callers, no other
/// vtable shares it) - so this flat detour intercepts only real tank exhibits. Non-tank dispatch reaches
/// the already-detoured base stub instead (see [`ZTHabitat::is_right_salinity`]).
#[detour_mod]
pub mod detours {
    use super::*;
    use crate::util::ref_from_memory;

    #[detour(IS_RIGHT_SALINITY)]
    unsafe extern "thiscall" fn is_right_salinity(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.is_right_salinity(animal_type as u32)
    }

    /// Release-safe path back to real vanilla for the live comparison test - see `ztawardmgr`'s
    /// `call_real` doc comments for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_right_salinity_real(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { IS_RIGHT_SALINITY_DETOUR.call(this, animal_type) }
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ZTTankExhibit detours: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonfresh_water_passes_when_category9_within_category10() {
        assert!(ZTTankExhibit::salinity_result(7, 5, 1));
    }

    #[test]
    fn nonfresh_water_fails_when_category9_exceeds_category10() {
        assert!(!ZTTankExhibit::salinity_result(5, 7, 1));
    }

    #[test]
    fn fresh_water_inverts_a_passing_comparison() {
        assert!(!ZTTankExhibit::salinity_result(7, 5, 0));
    }

    #[test]
    fn fresh_water_inverts_a_failing_comparison() {
        assert!(ZTTankExhibit::salinity_result(5, 7, 0));
    }

    #[test]
    fn equal_values_count_as_within() {
        assert!(ZTTankExhibit::salinity_result(6, 6, 1));
        assert!(!ZTTankExhibit::salinity_result(6, 6, 0));
    }

    #[test]
    fn comparison_is_signed_not_unsigned() {
        // Real vanilla compares with `SETGE`: category_10 = -1 loses to category_9 = 1 signed, but
        // would win a raw u32 comparison.
        assert!(!ZTTankExhibit::salinity_result(-1, 1, 1));
    }

    #[test]
    fn any_nonzero_water_type_skips_the_inversion() {
        assert!(ZTTankExhibit::salinity_result(7, 5, -3));
        assert!(ZTTankExhibit::salinity_result(7, 5, 2));
    }
}
