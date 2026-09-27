use getset::Getters;
use openzt_detour::generated::{
    bfcategory::GET_VALUE,
    zttankexhibit::{
        ADD_RANDOM_SPARKLE, FILL, IS_RIGHT_SALINITY, REMOVE_DEAD_SPARKLES, SET_WATER_LEVEL, SET_WATER_PURITY, UPDATE, UPDATE_ADJUSTMENT_COSTS,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::habitat::ZTHabitat;
use super::support::walk_tile_list;
use crate::globals::get_module_base;
use crate::util::{get_from_memory, LiveMemory};
use crate::write_live;

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
    pub _live: LiveMemory, // trailing ZST - see LiveMemory's own docs; required by this struct's real methods reading live game memory through &self (is_right_salinity, update).
}

const _: () = assert!(std::mem::size_of::<ZTTankExhibit>() == 0x1e8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, tank_height) == 0x184);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_level) == 0x188);
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

/// `DAT_006390ac` - the settings global real vanilla's rise branch adds to `tank_height` to form the
/// rise target (`water_level` rises one level per tick while below `tank_height + this`). Data global,
/// so resolved at runtime as `zoo.exe` base + RVA; read live, never hardcoded (`0` at rest).
const WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA: u32 = 0x0023_90ac; // DAT_006390ac - 0 at rest

/// `DAT_0063af04` - the purity-countdown reload: `update` resets `water_purity_timer` to this each
/// time it drops below 1, just before dropping `water_purity` by one (data global, `1000` at rest).
const WATER_PURITY_TIMER_RELOAD_RVA: u32 = 0x0023_af04; // DAT_0063af04 - 1000 at rest

/// `DAT_006390b8` - the sparkle-spawn interval numerator: `update` reloads `sparkle_spawn_timer` to
/// this divided by the owned-tile count each time it drops below 0 (data global, `40000` at rest).
const SPARKLE_SPAWN_INTERVAL_NUMERATOR_RVA: u32 = 0x0023_90b8; // DAT_006390b8 - 40000 at rest

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

    /// Ports `ZTTankExhibit::update` (the tank vtable's `+0x24` override of the base
    /// [`ZTHabitat::update`] slot, `generated.rs`'s `zttankexhibit::UPDATE` at `0x0049625f`,
    /// `ZTTankExhibit_update.c`/`.asm` - the macOS decompile confirms the clean if/else shape behind
    /// the Windows `.c`'s duplicated `is_filled` test, a cold-block artifact): composes the base port
    /// (real vanilla's own flat `CALL ZTHabitat::update` reaches that same port at runtime, the base
    /// address being detoured), then runs the tank tick:
    ///
    /// - **rise/drain on `is_filled`**: while filled and below `tank_height + DAT_006390ac`, rise one
    ///   level per tick; while drained and above zero, fall one level per tick; falling to zero with a
    ///   different `pending_water_type` staged swaps it into `current_water_type` and rebuilds
    ///   (`updateAdjustmentCosts` + `fill`). A filled tank at its target skips the drain branch
    ///   entirely.
    /// - **purity countdown**: while water is present and `water_purity` is positive, count
    ///   `water_purity_timer` down by `elapsed`; below 1, reload it from `DAT_0063af04` and drop
    ///   `water_purity` by one.
    /// - **sparkle lifecycle**: retire dead sparkles (`removeDeadSparkles`), count `sparkle_spawn_timer`
    ///   down by `elapsed`; below 0, spawn one (`addRandomSparkle`) and reload the timer from
    ///   `DAT_006390b8 / owned_tile_count` (unsigned division, walking the owned-tile sentinel exactly
    ///   like [`walk_tile_list`]).
    ///
    /// Every water-level/purity mutation is a real `setWaterLevel`/`setWaterPurity` call-through, never
    /// a field write - the setters clamp, manage water ripples, `removeOwnedTransients` on drain-to-zero
    /// and can `recreateOAs`/post UI messages. Reads are plain field loads (`getWaterLevel` is a trivial
    /// `[ECX+0x188]` return, and the drain path re-reads the field *after* `setWaterLevel` returns
    /// precisely because the setter clamps - the field is the truth for the fill-transition check).
    ///
    /// [`Self::sparkle_timer_reload`] reloads 0 for a tile-less tank where real vanilla's `DIV` would
    /// raise `#DE` - unreachable live (a habitat with no owned tiles is never ticked) and a Rust panic
    /// can't reproduce a hardware divide fault safely.
    ///
    /// Must only be called on a live `ZTTankExhibit` reference, same precondition as
    /// [`Self::is_right_salinity`] - `self`'s own address is passed straight into every real vanilla
    /// call-through.
    pub fn update(&self, elapsed: u32) {
        self.habitat.update(elapsed);
        let this = self as *const Self as *const u32;

        if self.is_filled {
            let rise_delta: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_RISE_TARGET_ABOVE_TANK_HEIGHT_RVA);
            if Self::rise_needed(self.water_level as i32, self.tank_height as i32, rise_delta) {
                unsafe { SET_WATER_LEVEL.original()(this, self.water_level as i32 + 1) };
            }
        } else if self.water_level as i32 > 0 {
            unsafe { SET_WATER_LEVEL.original()(this, self.water_level as i32 - 1) };
            if Self::fill_transition_due(self.water_level as i32, self.current_water_type, self.pending_water_type) {
                write_live!(self, current_water_type, self.pending_water_type);
                unsafe { UPDATE_ADJUSTMENT_COSTS.original()(this) };
                unsafe { FILL.original()(this) };
            }
        }

        if self.water_level as i32 > 0 && self.water_purity > 0 {
            let purity_before = self.water_purity;
            let timer = self.water_purity_timer.wrapping_sub(elapsed as i32);
            write_live!(self, water_purity_timer, timer);
            if Self::purity_timer_expired(timer) {
                let reload: i32 = get_from_memory(get_module_base("zoo.exe") as u32 + WATER_PURITY_TIMER_RELOAD_RVA);
                write_live!(self, water_purity_timer, reload);
                unsafe { SET_WATER_PURITY.original()(this, purity_before - 1) };
            }
        }

        unsafe { REMOVE_DEAD_SPARKLES.original()(this, elapsed) };

        let sparkle_timer = self.sparkle_spawn_timer.wrapping_sub(elapsed as i32);
        write_live!(self, sparkle_spawn_timer, sparkle_timer);
        if Self::sparkle_timer_expired(sparkle_timer) {
            unsafe { ADD_RANDOM_SPARKLE.original()(this as i32) };
            let numerator: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + SPARKLE_SPAWN_INTERVAL_NUMERATOR_RVA);
            let tile_count = walk_tile_list(self.owned_tiles_ptr).count() as u32;
            write_live!(self, sparkle_spawn_timer, Self::sparkle_timer_reload(numerator, tile_count));
        }
    }

    /// [`Self::update`]'s rise-branch decision - real vanilla's signed `CMP/JL` against
    /// `tank_height + DAT_006390ac` (`ZTTankExhibit_update.asm`).
    fn rise_needed(water_level: i32, tank_height: i32, rise_delta: i32) -> bool {
        water_level < tank_height + rise_delta
    }

    /// [`Self::update`]'s purity-expiry decision - real vanilla's signed `JLE` (`< 1`, i.e. `<= 0`).
    fn purity_timer_expired(new_timer: i32) -> bool {
        new_timer < 1
    }

    /// [`Self::update`]'s sparkle-expiry decision - real vanilla's `JS` (strictly negative), one step
    /// stricter than [`Self::purity_timer_expired`]'s `<= 0` boundary.
    fn sparkle_timer_expired(new_timer: i32) -> bool {
        new_timer < 0
    }

    /// [`Self::update`]'s drain-completion transition: fires only at zero water with a pending water
    /// type that differs from the currently applied one.
    fn fill_transition_due(water_level: i32, current_water_type: i32, pending_water_type: i32) -> bool {
        water_level == 0 && current_water_type != pending_water_type
    }

    /// [`Self::update`]'s sparkle-timer reload - real vanilla's unsigned `DIV`
    /// (`DAT_006390b8 / owned_tile_count`, `XOR EDX,EDX; DIV EDI` in the `.asm`). A zero tile count
    /// reloads 0 where real vanilla would raise `#DE` - see [`Self::update`]'s doc comment.
    fn sparkle_timer_reload(numerator: u32, tile_count: u32) -> i32 {
        numerator.checked_div(tile_count).unwrap_or(0) as i32
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

    /// `ZTTankExhibit::update`'s own fixed address (`0x0049625f`) is referenced exactly once in the
    /// whole binary - the `+0x24` slot of the tank vtable itself (`0x006312e0`; no direct callers, no
    /// other vtable shares it) - so this detour intercepts every real tank's per-tick tick through
    /// `ZTHabitatMgr::update`'s vtable dispatch, while non-tank habitats still reach the base
    /// `ZTHabitat::update` detour.
    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        unsafe { ref_from_memory::<ZTTankExhibit>(this) }.update(elapsed);
    }

    /// Release-safe path back to real vanilla for the live comparison test - see `ztawardmgr`'s
    /// `call_real` doc comments for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn is_right_salinity_real(this: *const u32, animal_type: *const u32) -> bool {
        unsafe { IS_RIGHT_SALINITY_DETOUR.call(this, animal_type) }
    }

    /// Release-safe path back to real vanilla for the live comparison test - see
    /// [`is_right_salinity_real`] above for why `.original()` cannot be used here.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn update_real(this: *const u32, elapsed: u32) {
        unsafe { UPDATE_DETOUR.call(this, elapsed) }
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

    #[test]
    fn rise_comparison_is_signed_not_unsigned() {
        // Real vanilla compares with `JL`: a negative water level is below any target and must rise; a
        // raw u32 compare would read it as huge and skip the rise.
        assert!(ZTTankExhibit::rise_needed(-1, 5, 0));
        assert!(ZTTankExhibit::rise_needed(4, 5, 0));
        assert!(!ZTTankExhibit::rise_needed(5, 5, 0));
        assert!(!ZTTankExhibit::rise_needed(6, 5, 0));
    }

    #[test]
    fn rise_target_includes_the_settings_delta() {
        assert!(ZTTankExhibit::rise_needed(7, 5, 3));
        assert!(!ZTTankExhibit::rise_needed(8, 5, 3));
    }

    #[test]
    fn purity_timer_expires_at_one_not_zero() {
        // Real vanilla's `JLE`: `<= 0` counts as expired, `1` does not.
        assert!(ZTTankExhibit::purity_timer_expired(0));
        assert!(ZTTankExhibit::purity_timer_expired(-100));
        assert!(!ZTTankExhibit::purity_timer_expired(1));
    }

    #[test]
    fn sparkle_timer_expires_only_strictly_negative() {
        // Real vanilla's `JS` - one step stricter than the purity timer's boundary above, which is why
        // the two helpers are separate.
        assert!(!ZTTankExhibit::sparkle_timer_expired(0));
        assert!(!ZTTankExhibit::sparkle_timer_expired(1));
        assert!(ZTTankExhibit::sparkle_timer_expired(-1));
        assert!(ZTTankExhibit::sparkle_timer_expired(-100));
    }

    #[test]
    fn fill_transition_needs_zero_water_and_a_different_pending_type() {
        assert!(ZTTankExhibit::fill_transition_due(0, 1, 2));
        assert!(!ZTTankExhibit::fill_transition_due(0, 2, 2));
        assert!(!ZTTankExhibit::fill_transition_due(1, 1, 2));
        assert!(!ZTTankExhibit::fill_transition_due(-1, 1, 2));
    }

    #[test]
    fn sparkle_reload_is_unsigned_division() {
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(40_000, 4), 10_000);
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(7, 2), 3); // integer division truncates
        // Unsigned: a numerator above i32::MAX stays positive, and a count of 1 keeps it as-is (the
        // u32 result lands in the i32 field bit-for-bit).
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(0xffff_ffff, 2), 0x7fff_ffff);
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(0xffff_ffff, 1), -1);
    }

    #[test]
    fn sparkle_reload_guards_the_zero_tile_count() {
        // Documented deviation: vanilla's DIV by zero raises #DE; unreachable live, guarded to 0 here.
        assert_eq!(ZTTankExhibit::sparkle_timer_reload(40_000, 0), 0);
    }
}
