//! Structs and methods for the vanilla `ZTMarketingMgr`/`ZTMarketing`/`ZTMarketingFundingLevel`
//! classes, which drive the zoo's marketing spend: a single funding-level index selects a
//! `(name, benefit, cost)` entry from a flat table, and `ZTMarketingMgr::update` periodically spends
//! `cost` dollars per in-game day. Structurally much simpler than `ZTResearchMgr` (see
//! `openzt/plans/ztmarketing-implementation-plan.md`): no branch/category/program tree, no
//! `effect_kind` dispatch, no RNG.
//!
//! Field layouts below are confirmed directly from Windows decompiles in `resources/decompiles/
//! ZTMarketing*`/`ZTMarketingMgr*` (not just macOS-only guesses) - see the plan's "Known layout"
//! section for the full evidence trail. `GLOBAL_ZTMarketingMgr`'s own address (RVA `0x00239000`,
//! single-level pointer indirection) was confirmed by querying the project's Ghidra database
//! directly (OOAnalyzer-assigned symbol `GLOBAL_ZTMarketingMgr`) and cross-checked against three
//! already-known-good globals resolved the same way (`GLOBAL_ZTResearchMgr` -> `0x00239010`,
//! `GLOBAL_ZTGameMgr` -> `0x00238048`, `GLOBAL_ZTAdvTerrainMgr` -> `0x00238058`), all of which match
//! the addresses already hard-coded in `globals.rs`.
//!
//! Only the funding-level mutators (`increase_funding`/`decrease_funding`/`set_funding_level`) are
//! natively reimplemented so far - the rest of the plan's items (`update`, `save`/`load`,
//! `getFundingText`, the config-loading pipeline) are still TODO.

use std::mem::size_of;

use openzt_detour::generated::ztmarketingmgr;
use tracing::error;

use crate::{
    bfconfigfile::BFConfigFile,
    globals::get_module_base,
    string_registry::load_string_by_id,
    util::{get_from_memory, mut_from_memory, ref_from_memory},
    ztresearch::get_money_text,
};

/// One entry in a `ZTMarketing`'s flat funding-level table. Confirmed field-for-field by
/// `resources/decompiles/ZTMarketing_loadConfiguration.c`'s writes.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ZTMarketingFundingLevel {
    name: i32,    // 0x0 - confirmed: the level's display-name string id
    benefit: i32, // 0x4 - confirmed: read and stored by loadConfiguration but not read by update/save/load/getFundingText/setFundingLevel; likely consumed elsewhere in the game
    cost: f32,    // 0x8 - confirmed: the in-game-day cash cost this level charges (see ZTMarketing::update)
}

impl ZTMarketingFundingLevel {
    pub fn name_id(&self) -> i32 {
        self.name
    }

    /// Display name/template (e.g. `"%s"`-shaped), resolved through the same string table
    /// `ZTResearchFundingLevel::name` uses - see that method's own doc comment.
    pub fn name(&self) -> Option<String> {
        load_string_by_id(self.name as u32)
    }

    pub fn benefit(&self) -> i32 {
        self.benefit
    }

    pub fn cost(&self) -> f32 {
        self.cost
    }
}

/// `1.0 / 43200.0`, confirmed shared verbatim with `ZTResearchBranch::update`'s own
/// `DAYS_TO_FUNDING_SCALE` (see that constant's doc comment in `ztresearch.rs` for how it was
/// confirmed against the installed `zoo.exe`'s `.data` section) - `ZTMarketing::update` references the
/// exact same `_DAT_00630d78` global, in the same `days * cost * scale` shape.
const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;

/// Resolves `GLOBAL_ZTGameMgr` fresh from its raw memory slot on every call, rather than going through
/// `globals()`'s `CachedGlobalInstance` (which resolves the pointer chain once and caches it forever) -
/// same reasoning as `ztresearch::global_ztgamemgr_ptr`, whose own doc comment explains why this
/// matters for the `reimplementation_tests` live-comparison harness (it patches this same raw slot to
/// redirect reads, which a cached accessor can't observe).
fn global_ztgamemgr_ptr() -> *mut crate::ztgamemgr::ZTGameMgr {
    get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + 0x0023_8048) as *mut crate::ztgamemgr::ZTGameMgr
}

/// The zoo's single marketing campaign. Owned (by pointer) by `ZTMarketingMgr`; there is exactly one
/// instance, unlike `ZTResearchMgr`'s branch/category/program tree. Confirmed `operator_new(0x1c)` -
/// 28 bytes total - via `ZTMarketingMgr_loadConfigurations.c`'s own allocation call.
#[derive(Debug)]
#[repr(C)]
pub struct ZTMarketing {
    config_file: BFConfigFile,  // 0x00 - the inherited BFConfigFile base (see bfconfigfile.rs)
    current_funding_level: u32, // 0x0c - confirmed: index into the funding-level table below
    vector_start: u32, // 0x10 - confirmed: inline ZTMarketingFundingLevel table start (stride 0xc), a proper 3-pointer MSVC vector - not a ZTArray of pointers
    vector_end: u32,   // 0x14 - confirmed
    vector_capacity_end: u32, // 0x18 - confirmed: the destructor frees [vector_start, vector_capacity_end), not just [vector_start, vector_end)
}

impl ZTMarketing {
    /// See `ZTResearchProgram::is_config_loaded`'s doc comment for the shared `BFConfigFile` shape.
    pub fn is_config_loaded(&self) -> bool {
        self.config_file.is_loaded()
    }

    pub fn current_funding_level(&self) -> u32 {
        self.current_funding_level
    }

    fn funding_level_count(&self) -> usize {
        ((self.vector_end - self.vector_start) as usize) / size_of::<ZTMarketingFundingLevel>()
    }

    fn funding_level(&self, index: usize) -> ZTMarketingFundingLevel {
        get_from_memory(self.vector_start + (index * size_of::<ZTMarketingFundingLevel>()) as u32)
    }

    pub fn funding_levels(&self) -> Vec<ZTMarketingFundingLevel> {
        (0..self.funding_level_count()).map(|i| self.funding_level(i)).collect()
    }

    /// The vanilla "$400"-style formatted text for the *currently selected* funding level, per
    /// `resources/decompiles/ZTMarketing_getFundingText.c`. Bounds-checked the same way
    /// `ZTResearchBranch::funding_text` is (out-of-range - including a negative index read as `u32` -
    /// returns an empty string, matching vanilla's empty `BFString`-shaped triple in that branch).
    /// Unlike research's sibling method, there's **no** day-scale pre-multiply here - the raw `cost`
    /// field is passed straight into the money conversion (see the implementation plan's item 3: the
    /// decompile confirms no `1/30` or `1/43200` constant appears in `getFundingText` itself, only in
    /// `update`). Despite that, the same truncate-toward-zero FISTP idiom research's own `funding_text`
    /// already confirmed applies here too (same `getMoneyText` overload, same call shape) - a plain
    /// `as i32` cast in Rust already truncates toward zero, so no explicit rounding is needed.
    pub fn funding_text(&self) -> String {
        let index = self.current_funding_level as usize;
        if index >= self.funding_level_count() {
            return String::new();
        }
        let level = self.funding_level(index);
        let money_text = get_money_text(level.cost() as i32);
        match level.name() {
            Some(template) => template.replacen("%s", &money_text, 1),
            None => money_text,
        }
    }

    /// The `isFundingMaxed` boundary check vanilla inlines at every call site (`increaseFunding`'s own
    /// saturation guard, and `_updateMarketingInfo`'s increase-button enable/disable logic) rather than
    /// keeping as a separate function on Windows - see the implementation plan's item 0/2 analysis for
    /// why this is exposed as a plain query instead of a detoured `.original()`-backed method.
    pub fn is_funding_maxed(&self) -> bool {
        let count = self.funding_level_count() as u32;
        self.current_funding_level.wrapping_add(1) >= count
    }

    /// The `isFundingMined` boundary check - same "inlined away on Windows" story as `is_funding_maxed`.
    pub fn is_funding_mined(&self) -> bool {
        self.current_funding_level == 0
    }

    /// Reimplementation of `OOAnalyzer::ZTMarketing::increaseFunding`, per
    /// `resources/decompiles/ZTMarketing_increaseFunding.c`. An empty table always resets to index `0`
    /// (even though the index is already `0` from the constructor - vanilla doesn't special-case this);
    /// otherwise increments while there's room, else saturates at `count - 1`. Returns vanilla's masked
    /// low-byte return value: `true` iff the table was empty or the index was already at/past the last
    /// entry (i.e. exactly what a standalone `isFundingMaxed()` call would report *after* the operation).
    pub fn increase_funding(&mut self) -> bool {
        let count = self.funding_level_count() as u32;
        if count == 0 {
            self.current_funding_level = 0;
            return true;
        }
        if self.current_funding_level.wrapping_add(1) < count {
            self.current_funding_level = self.current_funding_level.wrapping_add(1);
            false
        } else {
            self.current_funding_level = count - 1;
            true
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTMarketing::decreaseFunding`, per
    /// `resources/decompiles/ZTMarketing_decreaseFunding.c`. Decrements only when the table is
    /// non-empty and the index isn't already `0`; otherwise resets to `0`. Returns vanilla's masked
    /// low-byte return value: `true` iff the index ended up (or already was) `0`.
    pub fn decrease_funding(&mut self) -> bool {
        if self.funding_level_count() > 0 && self.current_funding_level != 0 {
            self.current_funding_level -= 1;
            false
        } else {
            self.current_funding_level = 0;
            true
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTMarketing::setFundingLevel`, per
    /// `resources/decompiles/ZTMarketing_setFundingLevel.c`. Unlike `increase_funding`/
    /// `decrease_funding`, out-of-range input resets to `0` rather than saturating at the last entry -
    /// a genuinely different vanilla behavior per entry point, not something to unify away.
    pub fn set_funding_level(&mut self, level: u32) {
        if level < self.funding_level_count() as u32 {
            self.current_funding_level = level;
        } else {
            self.current_funding_level = 0;
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTMarketing::update`, per
    /// `resources/decompiles/ZTMarketing_update.c`: `days` in-game days of the *current* funding
    /// level's `cost` (unlike `getFundingText`, which bounds-checks the index, this reads
    /// `funding_level(current_funding_level)` completely unchecked - matching vanilla's own raw
    /// `*(this + 0x10) + 8 + *(this + 0xc) * 0xc` pointer arithmetic exactly, with no guard against an
    /// empty/out-of-range table). If affordable against `GLOBAL_ZTGameMgr`'s live budget, spends it via
    /// `ZooStatus::spendMarketing` then `ZTGameMgr::subtractCash` - the same two-call shape and shared
    /// `DAYS_TO_FUNDING_SCALE` constant `ZTResearchBranch::update` uses (see that method's own doc
    /// comment in `ztresearch.rs`), just with no progress/completion side effect at all.
    pub fn update(&self, days: u32) {
        let level = self.funding_level(self.current_funding_level as usize);
        let cash_delta = days as f32 * level.cost() * DAYS_TO_FUNDING_SCALE;
        let game_mgr = unsafe { &mut *global_ztgamemgr_ptr() };
        if cash_delta <= game_mgr.cash() {
            game_mgr.spend_marketing(cash_delta);
            game_mgr.subtract_cash(cash_delta);
        }
    }
}

/// The zoo's marketing manager - owns exactly one `ZTMarketing` by pointer. Confirmed
/// `operator_new(0x10)` - 16 bytes total - via `resources/decompiles/_CreateZTMarketingMgr.c` and
/// `ZTMarketingMgr_ZTMarketingMgr.c` (the constructor).
#[derive(Debug)]
#[repr(C)]
pub struct ZTMarketingMgr {
    vtable: u32,           // 0x00
    flag: u8,              // 0x04 - zeroed by the constructor; purpose not yet identified
    _pad: [u8; 3],         // 0x05
    tick_accumulator: u32, // 0x08 - accumulates ticks in ZTMarketingMgr::update, converted to an in-game day count once enough have accrued
    marketing_ptr: u32,    // 0x0c - pointer to the single owned ZTMarketing, null until loadConfigurations succeeds
}

/// Pure prediction for `ZTMarketingMgr::update`'s accumulator/day-count bookkeeping, per
/// `resources/decompiles/ZTMarketingMgr_update.c`/`.asm`. Bit-for-bit the same shape as
/// `ztresearch::predict_update` (`elapsed_ticks * 0x1c20 / 60000` days, threshold `0x167` = 359) -
/// confirmed directly from the decompile rather than assumed: the implementation plan flagged this
/// threshold as needing independent confirmation (research's own literal could easily have been
/// `0x168`/360 instead), and it turned out to be the exact same `0x167` constant. `tick_accumulator`
/// wraps via plain 32-bit addition (the decompile's `dword`-typed accumulator), matching vanilla's own
/// wraparound behavior exactly - see `ztresearch::predict_update`'s doc comment for why this uses
/// `wrapping_add`/`wrapping_mul` rather than a widened intermediate.
fn predict_mgr_update(tick_accumulator_before: u32, delta_ticks: u32) -> (u32, u32) {
    let tick_accumulator = tick_accumulator_before.wrapping_add(delta_ticks);
    let days = tick_accumulator.wrapping_mul(0x1c20) / 60000;
    if days > 0x167 {
        (0, days)
    } else {
        (tick_accumulator, 0)
    }
}

#[cfg(test)]
mod predict_mgr_update_tests {
    use super::*;

    #[test]
    fn accumulates_without_crossing_threshold() {
        assert_eq!(predict_mgr_update(100, 50), (150, 0));
    }

    #[test]
    fn day_count_of_359_does_not_trigger() {
        assert_eq!(predict_mgr_update(0, 2999), (2999, 0));
    }

    #[test]
    fn day_count_of_360_resets_and_returns_days() {
        assert_eq!(predict_mgr_update(0, 3000), (0, 360));
    }

    #[test]
    fn tick_accumulator_wraps_on_accumulation() {
        assert_eq!(predict_mgr_update(u32::MAX, 1), (0, 0));
    }
}

impl ZTMarketingMgr {
    pub fn marketing(&self) -> Option<&'static ZTMarketing> {
        (self.marketing_ptr != 0).then(|| unsafe { ref_from_memory(self.marketing_ptr) })
    }

    pub fn marketing_mut(&self) -> Option<&'static mut ZTMarketing> {
        (self.marketing_ptr != 0).then(|| unsafe { mut_from_memory(self.marketing_ptr) })
    }

    /// Exposed for the live `reimplementation_tests` comparison harness - see `predict_mgr_update`.
    pub(crate) fn tick_accumulator(&self) -> u32 {
        self.tick_accumulator
    }

    /// Exposed for the live `reimplementation_tests` comparison harness, to seed a synthetic manager's
    /// accumulator before comparing `ZTMarketingMgr::update` against the reimplementation - see
    /// `predict_mgr_update`.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_tick_accumulator(&mut self, value: u32) {
        self.tick_accumulator = value;
    }

    /// Native reimplementation of `ZTMarketingMgr::update`'s accumulator/day-count bookkeeping (see
    /// `predict_mgr_update`): once enough ticks have accrued, `tick_accumulator` resets to `0` and the
    /// owned `ZTMarketing` (if any) is advanced by the elapsed day count via `ZTMarketing::update` -
    /// unlike `ZTResearchMgr::update`'s branch loop, there's only ever the single owned instance to
    /// advance, matching vanilla's own `if (this->ZTMarketing != nullptr)` null guard exactly.
    pub fn update(&mut self, delta_ticks: u32) {
        let (new_tick_accumulator, days) = predict_mgr_update(self.tick_accumulator, delta_ticks);
        self.tick_accumulator = new_tick_accumulator;
        if days > 0 {
            if let Some(marketing) = self.marketing() {
                marketing.update(days);
            }
        }
    }

    /// Calls the vanilla `ZTMarketingMgr::save`. `file` is whatever file-handle pointer the original
    /// `WriteBytesToFile` calls expect. By default (see `marketing_save_reimplementation`) this
    /// address is detoured onto that module's native reimplementation; without that detour installed
    /// this reaches genuine vanilla code instead.
    pub fn save(&self, file: *const u32) -> bool {
        unsafe { ztmarketingmgr::SAVE.original()((self as *const Self) as *const u32, file) }
    }

    /// Calls the vanilla `ZTMarketingMgr::load` - the save-file counterpart to `save()`. See
    /// `marketing_save_reimplementation` for the exact behavior and its native reimplementation.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        unsafe { ztmarketingmgr::LOAD.original()((self as *mut Self) as *const u32, file, version) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `ZTMarketing` with a dummy funding table of `level_count` zeroed entries, matching
    /// what `increase_funding`/`decrease_funding`/`set_funding_level` actually read (only the table's
    /// *length* matters to these methods, never an entry's own content).
    fn marketing_with(current_funding_level: u32, level_count: usize) -> ZTMarketing {
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        // Not real memory - fine for these tests since increase_funding/decrease_funding/
        // set_funding_level never dereference vector_start, only compute the table length from the
        // three pointers' difference.
        let vector_start = 0x1000;
        let vector_end = vector_start + level_count as u32 * stride;
        ZTMarketing {
            config_file: BFConfigFile::default(),
            current_funding_level,
            vector_start,
            vector_end,
            vector_capacity_end: vector_end,
        }
    }

    #[test]
    fn increase_funding_empty_table_always_resets_to_zero_and_reports_maxed() {
        for start in [0, 1, 5] {
            let mut m = marketing_with(start, 0);
            assert!(m.increase_funding());
            assert_eq!(m.current_funding_level(), 0);
        }
    }

    #[test]
    fn increase_funding_increments_while_below_top_index() {
        let mut m = marketing_with(0, 3);
        assert!(!m.increase_funding());
        assert_eq!(m.current_funding_level(), 1);
        assert!(!m.increase_funding());
        assert_eq!(m.current_funding_level(), 2);
    }

    #[test]
    fn increase_funding_saturates_at_top_index_once_reached() {
        let mut m = marketing_with(2, 3);
        assert!(m.increase_funding());
        assert_eq!(m.current_funding_level(), 2);
    }

    #[test]
    fn increase_funding_saturates_when_already_past_top_index() {
        // Not reachable via increase_funding/decrease_funding/set_funding_level alone, but
        // increaseFunding's own guard is an unsigned comparison with no separate "in range" check -
        // confirm the reimplementation saturates rather than panicking/wrapping oddly.
        let mut m = marketing_with(10, 3);
        assert!(m.increase_funding());
        assert_eq!(m.current_funding_level(), 2);
    }

    #[test]
    fn decrease_funding_decrements_while_above_zero() {
        let mut m = marketing_with(2, 3);
        assert!(!m.decrease_funding());
        assert_eq!(m.current_funding_level(), 1);
        assert!(!m.decrease_funding());
        assert_eq!(m.current_funding_level(), 0);
    }

    #[test]
    fn decrease_funding_at_zero_resets_to_zero_and_reports_mined() {
        let mut m = marketing_with(0, 3);
        assert!(m.decrease_funding());
        assert_eq!(m.current_funding_level(), 0);
    }

    #[test]
    fn decrease_funding_empty_table_resets_to_zero_and_reports_mined() {
        let mut m = marketing_with(5, 0);
        assert!(m.decrease_funding());
        assert_eq!(m.current_funding_level(), 0);
    }

    #[test]
    fn set_funding_level_sets_in_range_index() {
        let mut m = marketing_with(0, 3);
        m.set_funding_level(2);
        assert_eq!(m.current_funding_level(), 2);
    }

    #[test]
    fn set_funding_level_out_of_range_resets_to_zero_not_saturate() {
        let mut m = marketing_with(1, 3);
        m.set_funding_level(3);
        assert_eq!(m.current_funding_level(), 0);
        m.set_funding_level(100);
        assert_eq!(m.current_funding_level(), 0);
    }

    #[test]
    fn set_funding_level_on_empty_table_always_resets_to_zero() {
        let mut m = marketing_with(0, 0);
        m.set_funding_level(0);
        assert_eq!(m.current_funding_level(), 0);
    }

    #[test]
    fn is_funding_maxed_matches_index_plus_one_at_or_past_count() {
        let m = marketing_with(2, 3);
        assert!(m.is_funding_maxed());
        let m = marketing_with(1, 3);
        assert!(!m.is_funding_maxed());
    }

    #[test]
    fn is_funding_mined_matches_index_zero() {
        assert!(marketing_with(0, 3).is_funding_mined());
        assert!(!marketing_with(1, 3).is_funding_mined());
    }
}

/// Native reimplementation of `ZTMarketingMgr::save`/`load`'s save-file persistence: a single
/// little-endian `u32` - the current funding-level index (`0` if no `ZTMarketing` is loaded) -
/// confirmed byte-for-byte from `resources/decompiles/ZTMarketingMgr_save.c`/`_load.c`. Far simpler
/// than `ztresearch::research_save_reimplementation` (no records, no completion tail, no RNG - see the
/// implementation plan's item 5), so this module skips straight to a single promoted phase instead of
/// that module's separate shadow-then-promote rollout.
///
/// **Promoted to the live path** (see `detours` below): by default `ZTMarketingMgr::save`/`load` are
/// detoured to run this module's logic directly against the real save stream (via
/// `standalone::WRITE_BYTES_TO_FILE`/`DEALLOCATE`, the same primitives
/// `research_save_reimplementation` uses), rather than calling `.original()`.
pub(crate) mod marketing_save_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::*;

    /// The save-format version at which `ZTMarketingMgr::load` starts reading the stream at all - per
    /// `ZTMarketingMgr_load.c`'s `0x3a < param_2` guard (strictly greater than, not `>=`). Below this,
    /// `load` never touches the stream *or* the current funding-level index - unlike
    /// `ZTResearchMgr::load`, which always resets first regardless of version.
    const MIN_VERSION_WITH_MARKETING_DATA: u32 = 0x3a;

    /// Predicts `ZTMarketingMgr::load`'s effect on the current funding-level index and its own return
    /// value, given whatever value the stream read produced (`None` models a genuine read failure -
    /// `load` returns `false` immediately in that case, without touching the index) and the
    /// `ZTMarketing`'s current funding-level table length. The read value's clamp uses the exact same
    /// bounds logic as `ZTMarketing::set_funding_level` (out-of-range resets to `0`, not saturate) -
    /// per `ZTMarketingMgr_load.c`'s own `-(uint)(value < count) & value`-shaped guard, so the live
    /// detour below just calls `set_funding_level` directly rather than duplicating this logic.
    pub(crate) fn predict_load(version: u32, read_value: Option<u32>, funding_level_count: usize, index_before: u32) -> (bool, u32) {
        if version <= MIN_VERSION_WITH_MARKETING_DATA {
            return (true, index_before);
        }
        match read_value {
            None => (false, index_before),
            Some(value) => {
                let index = if (value as usize) < funding_level_count { value } else { 0 };
                (true, index)
            }
        }
    }

    #[cfg(test)]
    mod predict_load_tests {
        use super::*;

        #[test]
        fn below_threshold_never_reads_and_leaves_index_untouched() {
            assert_eq!(predict_load(0x3a, Some(999), 3, 7), (true, 7));
            assert_eq!(predict_load(0, Some(0), 0, 7), (true, 7));
        }

        #[test]
        fn above_threshold_applies_in_range_value() {
            assert_eq!(predict_load(0x3b, Some(1), 3, 7), (true, 1));
        }

        #[test]
        fn above_threshold_out_of_range_value_resets_to_zero() {
            assert_eq!(predict_load(0x3b, Some(3), 3, 7), (true, 0));
            assert_eq!(predict_load(0x3b, Some(u32::MAX), 0, 7), (true, 0));
        }

        #[test]
        fn read_failure_reports_false_and_leaves_index_untouched() {
            assert_eq!(predict_load(0x3b, None, 3, 7), (false, 7));
        }
    }

    /// Detours `ZTMarketingMgr::save`/`load` onto this module's native reimplementation - the default,
    /// promoted arm (see the module doc comment above). Mirrors
    /// `ztresearch::research_save_reimplementation::detours` in shape, but `load` needs no unconditional
    /// reset/tail: an out-of-range or below-threshold read is a pure no-op on the index (see
    /// `predict_load`'s doc comment), so there is nothing to roll back either way.
    #[detour_mod]
    mod detours {
        use openzt_detour::generated::{
            standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE},
            ztmarketingmgr::{LOAD, SAVE},
        };

        use super::*;
        use crate::util::{mut_from_memory, ref_from_memory};

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
            let mgr = unsafe { ref_from_memory::<ZTMarketingMgr>(this) };
            let index = mgr.marketing().map(|m| m.current_funding_level()).unwrap_or(0);
            let bytes = index.to_le_bytes();
            let ok = unsafe { WRITE_BYTES_TO_FILE.original()(bytes.as_ptr() as *const u32, 4, 1, file as *const i8) };
            if !ok {
                error!("marketing-save-reimplementation: WriteBytesToFile failed writing the funding-level index");
            }
            ok
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            if version <= MIN_VERSION_WITH_MARKETING_DATA {
                return true;
            }

            let mut buf = 0u32;
            let ok = unsafe { DEALLOCATE.original()(&mut buf as *mut u32 as *const u32, 4, 1, file as *const u8) };
            if ok != 1 {
                return false;
            }

            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            if let Some(marketing) = mgr.marketing_mut() {
                marketing.set_funding_level(buf);
            }
            true
        }
    }

    /// Installs the `save`/`load` detour (the arm above). Called from `ztmarketing::init()`.
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise marketing-save-reimplementation detours: {e:?}");
        }
    }
}

/// Native reimplementation of the `.cfg`-driven marketing config loading
/// (`ZTMarketingMgr::loadConfigurations`/`ZTMarketing::loadConfiguration`/`ZTMarketingMgr::clearConfigurations`),
/// built on the same `openzt-configparser` INI parser `ztresearch::research_config_reimplementation`
/// uses for the much larger research tree - but far simpler here: there's only ever one `ZTMarketing`
/// instance (no manifest-of-many, no category/program tree), and nothing outside this pipeline ever
/// calls `ZTMarketing::loadConfiguration`/`clearConfiguration` or constructs/destroys a `ZTMarketing`
/// directly (implementation plan item 6/7), so those stay plain Rust methods below, never detoured on
/// their own - only the two `ZTMarketingMgr`-level entry points are.
///
/// **Promoted to the live path unconditionally**, no shadow-mode/vanilla-fallback feature flag - same
/// "no need for a separate staging phase" reasoning `marketing_save_reimplementation` already used
/// (item 5's doc comment): there's no completion-tail/RNG dependency here either, just parsing and a
/// straight struct-field splice.
mod marketing_config_reimplementation {
    use openzt_configparser::ini::Ini;
    use openzt_detour_macro::detour_mod;
    use tracing::{debug, error};

    use super::*;
    use crate::{encoding_utils::decode_game_text, resource_manager::lazyresourcemap::get_file};

    /// Loads and parses a resource-relative `.cfg` path with vanilla's actual comment convention
    /// (`;` only, unlike `legacy_loading.rs`'s more lenient mod-`.cfg` parsing) - identical to
    /// `ztresearch::research_config_reimplementation`'s own `read_cfg`.
    fn read_cfg(path: &str) -> Option<Ini> {
        let Some((_, data)) = get_file(path) else {
            error!("marketing-config-reimplementation: resource '{path}' not found");
            return None;
        };
        let text = decode_game_text(&data);
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';']);
        match ini.read(text) {
            Ok(_) => Some(ini),
            Err(e) => {
                error!("marketing-config-reimplementation: failed to parse '{path}': {e}");
                None
            }
        }
    }

    /// A value that trims to empty is indistinguishable from absent, matching `BFConfigFile::addKeyVal`.
    /// See `ztresearch::research_config_reimplementation::values`'s doc comment for the full evidence
    /// trail (the same `Ini::get_vec` vs. vanilla discrepancy applies here).
    fn values(ini: &Ini, section: &str, key: &str) -> Vec<String> {
        ini.get_vec(section, key).unwrap_or_default().into_iter().filter(|v| !v.trim().is_empty()).collect()
    }

    /// `BFConfigFile::getString`/`getInt`/`getFloat` return the *first* value for a repeated key;
    /// `Ini::get` returns the *last* - pull from `values` directly to match vanilla.
    fn first(ini: &Ini, section: &str, key: &str) -> Option<String> {
        values(ini, section, key).into_iter().next()
    }

    fn first_parse<T: std::str::FromStr>(ini: &Ini, section: &str, key: &str) -> Option<T> {
        first(ini, section, key)?.parse().ok()
    }

    /// One `[<block>] name=/cost=/benefit=` funding-level entry - per the implementation plan's item 6,
    /// `ZTMarketing::loadConfiguration` reads `getInt("name")`/`getFloat("cost")`/`getInt("benefit")`
    /// for each block named by the file's own `[marketing] marketing=<block>...` list.
    fn load_funding_level(ini: &Ini, block: &str) -> ZTMarketingFundingLevel {
        ZTMarketingFundingLevel {
            name: first_parse(ini, block, "name").unwrap_or_default(),
            benefit: first_parse(ini, block, "benefit").unwrap_or_default(),
            cost: first_parse(ini, block, "cost").unwrap_or_default(),
        }
    }

    /// Leaks `vec` into a fresh 3-pointer funding-table (all-zero if empty) - same shape as
    /// `live_support::funding_table_from_vec`, duplicated here since that copy is
    /// `#[cfg(feature = "reimplementation-tests")]`-gated and this module must exist unconditionally
    /// (mirrors `ztresearch::research_config_reimplementation`'s own duplicate of this same helper).
    fn funding_table_from_vec(mut vec: Vec<ZTMarketingFundingLevel>) -> (u32, u32, u32) {
        if vec.is_empty() {
            return (0, 0, 0);
        }
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        let ptr = vec.as_mut_ptr() as u32;
        let len = vec.len() as u32;
        let cap = vec.capacity() as u32;
        std::mem::forget(vec);
        (ptr, ptr + len * stride, ptr + cap * stride)
    }

    /// Frees a `ZTMarketing`'s funding-table buffer - `[vector_start, vector_capacity_end)`, matching
    /// the destructor's own range (see the implementation plan's "Known layout" section).
    fn free_funding_table(marketing: &ZTMarketing) {
        if marketing.vector_start == 0 {
            return;
        }
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        let cap = ((marketing.vector_capacity_end - marketing.vector_start) / stride) as usize;
        drop(unsafe { Vec::<ZTMarketingFundingLevel>::from_raw_parts(marketing.vector_start as *mut ZTMarketingFundingLevel, cap, cap) });
    }

    impl ZTMarketing {
        /// Reimplementation of `ZTMarketing::loadConfiguration`, per the implementation plan's item 6:
        /// unconditionally resets the current funding-level index to `0` and empties the funding table
        /// first (matching vanilla's own unconditional `clearConfiguration` call, which runs *before*
        /// `path` is even opened - so a failure below still leaves the table empty, not whatever it
        /// held previously), then, if `path` opens and parses, reads the `[marketing] marketing=<block>`
        /// list and appends one `ZTMarketingFundingLevel` per named block. The old buffer is freed and
        /// replaced with a freshly built one rather than reusing its capacity in place - behaviorally
        /// equivalent to vanilla's manual grow-and-copy `push_back` (same resulting elements, same
        /// order), which is all a live comparison could observe; see the plan for why the capacity-reuse
        /// mechanics themselves don't need bit-for-bit replication.
        pub(crate) fn load_configuration(&mut self, path: &str) -> bool {
            self.current_funding_level = 0;
            free_funding_table(self);
            self.vector_start = 0;
            self.vector_end = 0;
            self.vector_capacity_end = 0;

            let Some(ini) = read_cfg(path) else {
                return false;
            };
            let levels: Vec<ZTMarketingFundingLevel> = values(&ini, "marketing", "marketing").iter().map(|block| load_funding_level(&ini, block)).collect();
            let (start, end, capacity_end) = funding_table_from_vec(levels);
            self.vector_start = start;
            self.vector_end = end;
            self.vector_capacity_end = capacity_end;
            true
        }
    }

    impl ZTMarketingMgr {
        /// Reimplementation of `ZTMarketingMgr::clearConfigurations`: resets the tick accumulator to
        /// `0` and, if a `ZTMarketing` exists, destroys and frees it - but, per the implementation
        /// plan's item 6, deliberately does **not** null `marketing_ptr` afterward. This is a real
        /// vanilla quirk (the pointer is left dangling until a caller like `load_configurations`
        /// immediately overwrites it), not a decompile artifact to "fix" - any caller relying on
        /// post-clear null-safety here would already be relying on undefined vanilla behavior, so this
        /// reimplementation doesn't add defensive nulling vanilla itself doesn't do.
        pub(crate) fn clear_configurations(&mut self) {
            self.tick_accumulator = 0;
            if self.marketing_ptr != 0 {
                let ptr = self.marketing_ptr as *mut ZTMarketing;
                free_funding_table(unsafe { &*ptr });
                drop(unsafe { Box::from_raw(ptr) });
            }
        }

        /// Reimplementation of `ZTMarketingMgr::loadConfigurations`, per the implementation plan's item
        /// 6: always calls `clear_configurations` first, then bails out (returning `false`, with
        /// `marketing_ptr` left dangling - see that method's own doc comment) if the top-level file
        /// can't be opened. Otherwise allocates a fresh, default `ZTMarketing` (index `0`, empty table -
        /// matching the vanilla constructor), attaches it to `marketing_ptr` immediately, reads the
        /// `[marketing] marketing=<path>` value giving the *actual* funding `.cfg` file, and calls
        /// `ZTMarketing::load_configuration` on it. On failure, `set_funding_level(0)` is called on the
        /// already-attached, partially-populated `ZTMarketing` before returning `false` - matching
        /// vanilla's own explicit safety reset; on success the index is left untouched at the ctor's
        /// `0` rather than re-asserted, also matching vanilla (functionally identical either way, but
        /// worth replicating faithfully per the plan).
        pub(crate) fn load_configurations(&mut self, path: &str) -> bool {
            self.clear_configurations();
            let Some(top_ini) = read_cfg(path) else {
                return false;
            };

            let marketing = Box::new(ZTMarketing {
                config_file: BFConfigFile::default(),
                current_funding_level: 0,
                vector_start: 0,
                vector_end: 0,
                vector_capacity_end: 0,
            });
            let marketing_ptr = Box::into_raw(marketing);
            self.marketing_ptr = marketing_ptr as u32;

            let cfg_path = first(&top_ini, "marketing", "marketing").unwrap_or_default();
            let ok = unsafe { &mut *marketing_ptr }.load_configuration(&cfg_path);
            if !ok {
                unsafe { &mut *marketing_ptr }.set_funding_level(0);
            }
            ok
        }
    }

    /// Detours `ZTMarketingMgr::loadConfigurations`/`clearConfigurations` onto this module's native
    /// reimplementation - the only two entry points that need detouring (see the module doc comment
    /// above for why `ZTMarketing::loadConfiguration`/`clearConfiguration` don't need their own).
    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztmarketingmgr::{CLEAR_CONFIGURATIONS, LOAD_CONFIGURATIONS};

        use super::*;
        use crate::util::mut_from_memory;

        #[detour(LOAD_CONFIGURATIONS)]
        unsafe extern "thiscall" fn load_configurations(this: *const u32, path: *const i8) -> u32 {
            let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();

            // Peek the top-level file independently before mutating anything - if our own INI reader
            // can't open/parse it (a resource lookup gap, or a format edge case vanilla's own parser
            // tolerates that ours doesn't), fall back to vanilla entirely rather than risk diverging.
            // Mirrors `ztresearch::research_config_reimplementation`'s own `load_branches` detour, which
            // applies the same "parse first, mutate second, fall back only pre-mutation" rule.
            if super::read_cfg(&path_str).is_none() {
                error!("marketing-config-reimplementation: failed to independently parse '{path_str}', falling back to vanilla");
                return unsafe { LOAD_CONFIGURATIONS_DETOUR.call(this, path) };
            }

            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            let ok = mgr.load_configurations(&path_str);
            debug!("marketing-config-reimplementation: loadConfigurations(\"{path_str}\") replaced natively -> {ok}");
            ok as u32
        }

        #[detour(CLEAR_CONFIGURATIONS)]
        unsafe extern "thiscall" fn clear_configurations(this: *const u32) {
            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            mgr.clear_configurations();
        }
    }

    /// Installs the `loadConfigurations`/`clearConfigurations` detour (the arm above). Called from
    /// `ztmarketing::init()`.
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise marketing-config-reimplementation detours: {e:?}");
        }
    }
}

/// registers the marketing module's live detours
pub fn init() {
    marketing_save_reimplementation::init();
    marketing_config_reimplementation::init();
}

/// Synthetic `ZTMarketing` construction/teardown for the live `reimplementation_tests` comparison
/// harness. Every allocation goes through Rust's own allocator (never spliced into any real
/// `ZTMarketingMgr`), since `increaseFunding`/`decreaseFunding`/`setFundingLevel` only ever read/write
/// `this` - unlike research's `load`, there's no `GLOBAL_ZTMarketingMgr` dependency to worry about.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    fn funding_table_from_vec(mut vec: Vec<ZTMarketingFundingLevel>) -> (u32, u32, u32) {
        if vec.is_empty() {
            return (0, 0, 0);
        }
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        let ptr = vec.as_mut_ptr() as u32;
        let len = vec.len() as u32;
        let cap = vec.capacity() as u32;
        std::mem::forget(vec);
        (ptr, ptr + len * stride, ptr + cap * stride)
    }

    fn free_funding_table(start: u32, capacity_end: u32) {
        if start == 0 {
            return;
        }
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        let cap = ((capacity_end - start) / stride) as usize;
        drop(unsafe { Vec::<ZTMarketingFundingLevel>::from_raw_parts(start as *mut ZTMarketingFundingLevel, cap, cap) });
    }

    /// Builds a standalone `ZTMarketing` with `level_count` dummy (zeroed) funding-level entries -
    /// only the table's length matters to `increaseFunding`/`decreaseFunding`/`setFundingLevel`, never
    /// an entry's own content.
    pub(crate) fn build_standalone_marketing(current_funding_level: u32, level_count: usize) -> *mut ZTMarketing {
        let table = vec![ZTMarketingFundingLevel { name: 0, benefit: 0, cost: 0.0 }; level_count];
        let (vector_start, vector_end, vector_capacity_end) = funding_table_from_vec(table);
        Box::into_raw(Box::new(ZTMarketing {
            config_file: BFConfigFile::default(),
            current_funding_level,
            vector_start,
            vector_end,
            vector_capacity_end,
        }))
    }

    pub(crate) fn destroy_standalone_marketing(ptr: *mut ZTMarketing) {
        if ptr.is_null() {
            return;
        }
        let marketing = unsafe { &*ptr };
        free_funding_table(marketing.vector_start, marketing.vector_capacity_end);
        drop(unsafe { Box::from_raw(ptr) });
    }

    /// Builds a standalone `ZTMarketing` with a single funding-level entry at index `0` (matching
    /// `current_funding_level`'s default) whose `cost` is `funding_cost` - used by the
    /// `ZTMARKETING_UPDATE` live comparison test, which needs a real, non-empty table for
    /// `ZTMarketing::update`'s unchecked `funding_level(current_funding_level)` read (see that
    /// method's own doc comment on `ztmarketing.rs`) to be safe.
    pub(crate) fn build_standalone_marketing_with_cost(funding_cost: f32) -> *mut ZTMarketing {
        let table = vec![ZTMarketingFundingLevel { name: 0, benefit: 0, cost: funding_cost }];
        let (vector_start, vector_end, vector_capacity_end) = funding_table_from_vec(table);
        Box::into_raw(Box::new(ZTMarketing {
            config_file: BFConfigFile::default(),
            current_funding_level: 0,
            vector_start,
            vector_end,
            vector_capacity_end,
        }))
    }

    /// Builds a standalone `ZTMarketing` for the `ZTMARKETING_GET_FUNDING_TEXT` live comparison test -
    /// not spliced into any `ZTMarketingMgr` (`getFundingText`/`funding_text` only ever read `this`, no
    /// `GLOBAL_ZTMarketingMgr` dependency). `levels` becomes the funding table verbatim, in order -
    /// mirrors `ztresearch::research_save_reimplementation::live_support::build_standalone_funding_branch`.
    pub(crate) fn build_standalone_marketing_with_levels(current_funding_level: u32, levels: &[(i32, f32)]) -> *mut ZTMarketing {
        let table: Vec<ZTMarketingFundingLevel> = levels.iter().map(|&(name, cost)| ZTMarketingFundingLevel { name, benefit: 0, cost }).collect();
        let (vector_start, vector_end, vector_capacity_end) = funding_table_from_vec(table);
        Box::into_raw(Box::new(ZTMarketing {
            config_file: BFConfigFile::default(),
            current_funding_level,
            vector_start,
            vector_end,
            vector_capacity_end,
        }))
    }

    /// Builds a standalone `ZTMarketingMgr` - **not** the real live singleton at
    /// `globals().ztmarketingmgr_ptr()` - wired to own `marketing_ptr` (or none, if null). Unlike
    /// `ztresearch::research_save_reimplementation::live_support::with_standalone_mgr`, `update`/
    /// `save`/`load` never dereference `GLOBAL_ZTMarketingMgr` itself (only `this`/the owned
    /// `ZTMarketing`), so - unlike research's `pick_random_program` - there's no global slot to patch
    /// for any of these comparisons; a `vtable`/`flag` of `0` is fine since none of these calls are
    /// ever reached through a virtual dispatch in these tests.
    pub(crate) fn build_standalone_marketing_mgr(tick_accumulator: u32, marketing_ptr: *mut ZTMarketing) -> *mut ZTMarketingMgr {
        Box::into_raw(Box::new(ZTMarketingMgr { vtable: 0, flag: 0, _pad: [0; 3], tick_accumulator, marketing_ptr: marketing_ptr as u32 }))
    }

    pub(crate) fn destroy_standalone_marketing_mgr(ptr: *mut ZTMarketingMgr) {
        if ptr.is_null() {
            return;
        }
        drop(unsafe { Box::from_raw(ptr) });
    }

    /// Temporarily pins the real, live `ZTGameMgr` singleton's budget to `cash`, runs `f`, then
    /// restores whatever it held before this call - mirrors
    /// `ztresearch::research_save_reimplementation::live_support::with_ztgamemgr_cash` exactly (see
    /// that function's own doc comment for why this mutates the real singleton in place rather than a
    /// synthetic one). Used by the `ZTMARKETING_UPDATE` comparison test so both the real and
    /// reimplemented `ZTMarketing::update` calls see the exact same available cash.
    pub(crate) fn with_ztgamemgr_cash<R>(cash: f32, f: impl FnOnce() -> R) -> R {
        let game_mgr = unsafe { &mut *global_ztgamemgr_ptr() };
        let original = game_mgr.cash();
        game_mgr.set_cash(cash);

        let result = f();

        unsafe { &mut *global_ztgamemgr_ptr() }.set_cash(original);
        result
    }

    /// Exposed for `reimplementation_tests` to null-check `GLOBAL_ZTGameMgr`'s raw slot before running
    /// the `ZTMARKETING_UPDATE` comparison - mirrors
    /// `ztresearch::research_save_reimplementation::live_support::ztgamemgr_ptr_is_null`.
    pub(crate) fn ztgamemgr_ptr_is_null() -> bool {
        global_ztgamemgr_ptr().is_null()
    }
}
