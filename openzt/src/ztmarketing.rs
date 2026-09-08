//! Structs and methods for the vanilla `ZTMarketingMgr`/`ZTMarketing`/`ZTMarketingFundingLevel`
//! classes, which drive the zoo's marketing spend: a single funding-level index selects a
//! `(name, benefit, cost)` entry from a flat table, and `ZTMarketingMgr::update` periodically spends
//! `cost` dollars per in-game day.
//!
//! The funding-level mutators (`increase_funding`/`decrease_funding`/`set_funding_level`), `update`,
//! `save`/`load`, `getFundingText`, the `.cfg`-driven config-loading pipeline
//! (`marketing_config_reimplementation`, below), and the vtable destructor (`marketing_dtor_detour`,
//! below) are all natively reimplemented. Only `ZTMarketingMgr::create`/`instantiate` (construction, not
//! teardown) remain unreimplemented - `ztmarketingmgr::CONSTRUCTOR`/`CREATE_ZTMARKETING_MGR` in
//! `generated.rs` are the confirmed addresses, just not yet redirected onto a Rust-side allocation path.

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

/// One entry in a `ZTMarketing`'s flat funding-level table.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ZTMarketingFundingLevel {
    name: i32,    // 0x0 - display-name string id
    benefit: i32, // 0x4 - read/stored, not used elsewhere in this module
    cost: f32,    // 0x8 - in-game-day cash cost this level charges
}

impl ZTMarketingFundingLevel {
    pub fn name_id(&self) -> i32 {
        self.name
    }

    /// Display name/template (e.g. `"%s"`-shaped), resolved through the string table.
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

/// Shared verbatim with `ZTResearchBranch::update`'s own `DAYS_TO_FUNDING_SCALE`.
const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;

/// Resolves `GLOBAL_ZTGameMgr` fresh from memory on every call rather than caching it, so the
/// `reimplementation_tests` harness can redirect reads by patching the raw slot.
fn global_ztgamemgr_ptr() -> *mut crate::ztgamemgr::ZTGameMgr {
    get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + 0x0023_8048) as *mut crate::ztgamemgr::ZTGameMgr
}

/// The zoo's single marketing campaign. Owned (by pointer) by `ZTMarketingMgr`; there is exactly one
/// instance, unlike `ZTResearchMgr`'s branch/category/program tree. Allocated as 28 bytes
/// (`operator_new(0x1c)`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTMarketing {
    config_file: BFConfigFile,  // 0x00 - inherited BFConfigFile base (see bfconfigfile.rs)
    current_funding_level: u32, // 0x0c - index into the funding-level table below
    vector_start: u32, // 0x10 - inline ZTMarketingFundingLevel table start (stride 0xc), MSVC 3-pointer vector
    vector_end: u32,   // 0x14
    vector_capacity_end: u32, // 0x18 - destructor frees [vector_start, vector_capacity_end)
}

impl ZTMarketing {
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

    /// The vanilla `"$400"`-style formatted text for the currently selected funding level.
    /// Out-of-range index (including a negative index read as `u32`) returns an empty string. Unlike
    /// `ZTResearchBranch::funding_text`, there's no day-scale pre-multiply - `cost` is passed straight
    /// into the money conversion.
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

    /// The `isFundingMaxed` check vanilla inlines at every call site rather than keeping as a separate
    /// function.
    pub fn is_funding_maxed(&self) -> bool {
        let count = self.funding_level_count() as u32;
        self.current_funding_level.wrapping_add(1) >= count
    }

    /// The `isFundingMined` check - same "inlined at call sites" story as `is_funding_maxed`.
    pub fn is_funding_mined(&self) -> bool {
        self.current_funding_level == 0
    }

    /// An empty table always resets to index `0`; otherwise increments while there's room, else
    /// saturates at `count - 1`. Returns `true` iff the table was empty or the index is at/past the
    /// last entry after the operation.
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

    /// Decrements only when the table is non-empty and the index isn't already `0`; otherwise resets
    /// to `0`. Returns `true` iff the index ended up (or already was) `0`.
    pub fn decrease_funding(&mut self) -> bool {
        if self.funding_level_count() > 0 && self.current_funding_level != 0 {
            self.current_funding_level -= 1;
            false
        } else {
            self.current_funding_level = 0;
            true
        }
    }

    /// Unlike `increase_funding`/`decrease_funding`, out-of-range input resets to `0` rather than
    /// saturating at the last entry.
    pub fn set_funding_level(&mut self, level: u32) {
        if level < self.funding_level_count() as u32 {
            self.current_funding_level = level;
        } else {
            self.current_funding_level = 0;
        }
    }

    /// `days` in-game days of the current funding level's `cost`, read unchecked - matching vanilla's
    /// own raw pointer arithmetic with no guard against an empty/out-of-range table. If affordable
    /// against `GLOBAL_ZTGameMgr`'s live budget, spends it via `ZooStatus::spendMarketing` then
    /// `ZTGameMgr::subtractCash`.
    ///
    /// The empty-table case is guarded here, unlike vanilla: `increase_funding`/`decrease_funding`/
    /// `set_funding_level` all keep `current_funding_level < funding_level_count()` whenever the table
    /// is non-empty, so an empty table is the only way this could see an invalid index.
    pub fn update(&self, days: u32) {
        if self.funding_level_count() == 0 {
            return;
        }
        let level = self.funding_level(self.current_funding_level as usize);
        let cash_delta = days as f32 * level.cost() * DAYS_TO_FUNDING_SCALE;
        let game_mgr = unsafe { &mut *global_ztgamemgr_ptr() };
        if cash_delta <= game_mgr.cash() {
            game_mgr.spend_marketing(cash_delta);
            game_mgr.subtract_cash(cash_delta);
        }
    }
}

/// The zoo's marketing manager - owns exactly one `ZTMarketing` by pointer. Allocated as 16 bytes
/// (`operator_new(0x10)`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTMarketingMgr {
    vtable: u32,           // 0x00
    flag: u8,              // 0x04 - zeroed by the constructor; purpose unknown
    _pad: [u8; 3],         // 0x05
    tick_accumulator: u32, // 0x08 - accumulates ticks in ZTMarketingMgr::update, converted to an in-game day count once enough have accrued
    marketing_ptr: u32,    // 0x0c - pointer to the single owned ZTMarketing, null until loadConfigurations succeeds
}

/// Pure prediction for `ZTMarketingMgr::update`'s accumulator/day-count bookkeeping. Same shape as
/// `ztresearch::predict_update`: `elapsed_ticks * 0x1c20 / 60000` days, threshold `0x167` (359).
/// `tick_accumulator` wraps via plain 32-bit addition, matching vanilla's own wraparound behavior.
pub(crate) fn predict_mgr_update(tick_accumulator_before: u32, delta_ticks: u32) -> (u32, u32) {
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

    /// Exposed for the live `reimplementation_tests` comparison harness.
    pub(crate) fn tick_accumulator(&self) -> u32 {
        self.tick_accumulator
    }

    /// Exposed for the live `reimplementation_tests` comparison harness, to seed a synthetic manager's
    /// accumulator before comparing `ZTMarketingMgr::update` against the reimplementation.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_tick_accumulator(&mut self, value: u32) {
        self.tick_accumulator = value;
    }

    /// Exposed for the live `reimplementation_tests` comparison harness, to check `marketing_ptr`'s raw
    /// non-null/null state without constructing a reference to memory that may already be freed - see
    /// `clear_configurations`'s doc comment on why the pointer is deliberately left dangling rather than
    /// nulled.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn marketing_ptr_raw(&self) -> u32 {
        self.marketing_ptr
    }

    /// Once enough ticks have accrued (`predict_mgr_update`), `tick_accumulator` resets to `0` and the
    /// owned `ZTMarketing` (if any) is advanced by the elapsed day count.
    pub fn update(&mut self, delta_ticks: u32) {
        let (new_tick_accumulator, days) = predict_mgr_update(self.tick_accumulator, delta_ticks);
        self.tick_accumulator = new_tick_accumulator;
        if days > 0
            && let Some(marketing) = self.marketing()
        {
            marketing.update(days);
        }
    }

    /// Calls `ZTMarketingMgr::save`. `file` is whatever file-handle pointer the original
    /// `WriteBytesToFile` calls expect. By default this address is detoured onto
    /// `marketing_save_reimplementation`'s native reimplementation, and `.hooked()` deliberately
    /// re-enters that hook, exactly like a vanilla caller would; with no detour installed the
    /// address still holds genuine vanilla code.
    pub fn save(&self, file: *const u32) -> bool {
        unsafe { ztmarketingmgr::SAVE.hooked()((self as *const Self) as *const u32, file) }
    }

    /// Calls `ZTMarketingMgr::load` - the save-file counterpart to `save()`, with the same
    /// deliberate re-entry via `.hooked()`.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        unsafe { ztmarketingmgr::LOAD.hooked()((self as *mut Self) as *const u32, file, version) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `ZTMarketing` with a dummy funding table of `level_count` zeroed entries - only the
    /// table's length matters to `increase_funding`/`decrease_funding`/`set_funding_level`.
    fn marketing_with(current_funding_level: u32, level_count: usize) -> ZTMarketing {
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        // Not real memory - fine here since these methods never dereference vector_start.
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
        // Not reachable via increase_funding/decrease_funding/set_funding_level alone, but confirm
        // this saturates rather than panicking/wrapping.
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
/// little-endian `u32`, the current funding-level index (`0` if no `ZTMarketing` is loaded).
///
/// Promoted to the live path (see `detours` below): by default `ZTMarketingMgr::save`/`load` are
/// detoured to run this module's logic directly against the real save stream, rather than calling
/// `.original()`.
pub(crate) mod marketing_save_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::*;

    /// The save-format version at which `ZTMarketingMgr::load` starts reading the stream at all
    /// (strictly greater than, not `>=`). Below this, `load` never touches the stream or the current
    /// funding-level index.
    const MIN_VERSION_WITH_MARKETING_DATA: u32 = 0x3a;

    /// Predicts `ZTMarketingMgr::load`'s effect on the current funding-level index and its own return
    /// value, given whatever value the stream read produced (`None` models a read failure - `load`
    /// returns `false` immediately, without touching the index) and the `ZTMarketing`'s current
    /// funding-level table length. Out-of-range values reset to `0`, matching
    /// `ZTMarketing::set_funding_level`.
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

    /// Detours `ZTMarketingMgr::save`/`load` onto this module's native reimplementation.
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
            let ok = unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, 4, 1, file as *const i8) } == 1;
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
            let ok = unsafe { DEALLOCATE.hooked()(&mut buf as *mut u32 as *const u32, 4, 1, file as *const u8) };
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

    /// Installs the `save`/`load` detour. Called from `ztmarketing::init()`.
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise marketing-save-reimplementation detours: {e:?}");
        }
    }
}

/// Detours `ZTMarketingMgr::update` onto `ZTMarketingMgr::update`/`ZTMarketing::update` above.
pub(crate) mod marketing_update_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::*;

    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztmarketingmgr::UPDATE;

        use super::*;
        use crate::util::mut_from_memory;

        /// Vanilla's own `int` return here is a decompiler artifact (leftover `EAX` from an
        /// intermediate multiply) - the function is logically `void`, so this always returns `0`.
        #[detour(UPDATE)]
        unsafe extern "thiscall" fn update(this: *const u32, delta_ticks: u32) -> i32 {
            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            mgr.update(delta_ticks);
            0
        }
    }

    /// Installs the `update` detour. Called unconditionally from `ztmarketing::init()`.
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise marketing-update-reimplementation detours: {e:?}");
        }
    }
}

/// Native reimplementation of the `.cfg`-driven marketing config loading
/// (`ZTMarketingMgr::loadConfigurations`/`ZTMarketing::loadConfiguration`/`ZTMarketingMgr::clearConfigurations`),
/// built on the `openzt-configparser` INI parser. There's only ever one `ZTMarketing` instance, so
/// `loadConfiguration`/`clearConfiguration` stay plain Rust methods and are never detoured on their
/// own - only the two `ZTMarketingMgr`-level entry points are.
///
/// Promoted to the live path unconditionally, with no shadow-mode/vanilla-fallback flag.
mod marketing_config_reimplementation {
    use openzt_configparser::ini::Ini;
    use openzt_detour_macro::detour_mod;
    use tracing::{error, info};

    use super::*;
    use crate::{encoding_utils::decode_game_text, resource_manager::lazyresourcemap::get_file};

    /// Loads and parses a resource-relative `.cfg` path using vanilla's comment convention (`;` only,
    /// unlike `legacy_loading.rs`'s more lenient mod-`.cfg` parsing).
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

    /// A value that trims to empty is treated as absent, matching `BFConfigFile::addKeyVal`.
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

    /// One `[<block>] name=/cost=/benefit=` funding-level entry.
    fn load_funding_level(ini: &Ini, block: &str) -> ZTMarketingFundingLevel {
        ZTMarketingFundingLevel {
            name: first_parse(ini, block, "name").unwrap_or_default(),
            benefit: first_parse(ini, block, "benefit").unwrap_or_default(),
            cost: first_parse(ini, block, "cost").unwrap_or_default(),
        }
    }

    /// Reads the `[marketing] funding=<block>...` block-name list and loads one
    /// `ZTMarketingFundingLevel` per named block, in order.
    fn parse_funding_levels(ini: &Ini) -> Vec<ZTMarketingFundingLevel> {
        values(ini, "marketing", "funding").iter().map(|block| load_funding_level(ini, block)).collect()
    }

    /// Reads the top-level file's `[marketing] marketing=<path>` value - the resource-relative path to
    /// the actual funding `.cfg` file (e.g. `mktgnorm.cfg`) - or `None` if absent. Distinct from the
    /// `funding` key `parse_funding_levels` reads one file down.
    fn resolve_funding_cfg_path(ini: &Ini) -> Option<String> {
        first(ini, "marketing", "marketing")
    }

    #[cfg(test)]
    mod parse_tests {
        use super::*;

        fn parse_ini(raw: &str) -> Ini {
            let mut ini = Ini::new_cs();
            ini.set_comment_symbols(&[';']);
            ini.read(raw.to_string()).expect("test INI should parse");
            ini
        }

        #[test]
        fn parses_funding_block_list_with_name_cost_benefit() {
            let ini = parse_ini(
                "[marketing]\n\
                 funding = none\n\
                 funding = min\n\
                 funding = normal\n\
                 funding = max\n\
                 \n\
                 [none]\n\
                 name = 23100\n\
                 cost = 0.0\n\
                 benefit = 0\n\
                 \n\
                 [min]\n\
                 name = 23101\n\
                 cost = 50.0\n\
                 benefit = 1\n\
                 \n\
                 [normal]\n\
                 name = 23102\n\
                 cost = 100.0\n\
                 benefit = 2\n\
                 \n\
                 [max]\n\
                 name = 23103\n\
                 cost = 200.0\n\
                 benefit = 3\n",
            );

            let levels = parse_funding_levels(&ini);
            assert_eq!(levels.len(), 4);
            assert_eq!((levels[0].name_id(), levels[0].cost(), levels[0].benefit()), (23100, 0.0, 0));
            assert_eq!((levels[1].name_id(), levels[1].cost(), levels[1].benefit()), (23101, 50.0, 1));
            assert_eq!((levels[2].name_id(), levels[2].cost(), levels[2].benefit()), (23102, 100.0, 2));
            assert_eq!((levels[3].name_id(), levels[3].cost(), levels[3].benefit()), (23103, 200.0, 3));
        }

        /// A file with only the top-level `marketing=<path>` key under `[marketing]`, and no `funding`
        /// key, must parse to an empty table - not panic.
        #[test]
        fn wrong_key_produces_empty_vec_not_a_panic_or_malformed_table() {
            let ini = parse_ini("[marketing]\nmarketing = none min normal max\n");
            let levels = parse_funding_levels(&ini);
            assert!(levels.is_empty());
        }

        #[test]
        fn missing_funding_key_produces_empty_vec_not_a_panic() {
            let ini = parse_ini("[marketing]\nunrelated = 1\n");
            let levels = parse_funding_levels(&ini);
            assert!(levels.is_empty());
        }

        #[test]
        fn resolves_top_level_marketing_path() {
            let ini = parse_ini("[marketing]\nmarketing = mktgnorm.cfg\n");
            assert_eq!(resolve_funding_cfg_path(&ini), Some("mktgnorm.cfg".to_string()));
        }

        #[test]
        fn resolve_funding_cfg_path_returns_none_when_absent() {
            let ini = parse_ini("[marketing]\nfunding = none\n");
            assert_eq!(resolve_funding_cfg_path(&ini), None);
        }
    }

    /// Leaks `vec` into a fresh 3-pointer funding table (all-zero if empty).
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
    /// the destructor's own range.
    fn free_funding_table(marketing: &ZTMarketing) {
        if marketing.vector_start == 0 {
            return;
        }
        let stride = size_of::<ZTMarketingFundingLevel>() as u32;
        let cap = ((marketing.vector_capacity_end - marketing.vector_start) / stride) as usize;
        drop(unsafe { Vec::<ZTMarketingFundingLevel>::from_raw_parts(marketing.vector_start as *mut ZTMarketingFundingLevel, cap, cap) });
    }

    impl ZTMarketing {
        /// Unconditionally resets the current funding-level index to `0` and empties the funding table
        /// first, then, if `path` opens and parses, reads the `[marketing] marketing=<block>` list and
        /// appends one `ZTMarketingFundingLevel` per named block. The old buffer is freed and replaced
        /// with a freshly built one rather than reusing its capacity in place.
        pub(crate) fn load_configuration(&mut self, path: &str) -> bool {
            self.current_funding_level = 0;
            free_funding_table(self);
            self.vector_start = 0;
            self.vector_end = 0;
            self.vector_capacity_end = 0;

            let Some(ini) = read_cfg(path) else {
                return false;
            };
            let levels = parse_funding_levels(&ini);
            let (start, end, capacity_end) = funding_table_from_vec(levels);
            self.vector_start = start;
            self.vector_end = end;
            self.vector_capacity_end = capacity_end;
            true
        }
    }

    impl ZTMarketingMgr {
        /// `ZTMarketing` instances in this pipeline are constructed/destroyed via Rust's own allocator
        /// (`Box::new`/`Box::from_raw`) rather than the native constructor/destructor - the
        /// `#[repr(C)]` struct is layout-equivalent, and nothing else in this path touches the embedded
        /// `BFConfigFile` through vanilla code.
        ///
        /// Resets the tick accumulator to `0` and, if a `ZTMarketing` exists, destroys and frees it, but
        /// deliberately does **not** null `marketing_ptr` afterward - a real vanilla quirk (the pointer
        /// is left dangling until a caller like `load_configurations` immediately overwrites it).
        pub(crate) fn clear_configurations(&mut self) {
            self.tick_accumulator = 0;
            if self.marketing_ptr != 0 {
                let ptr = self.marketing_ptr as *mut ZTMarketing;
                free_funding_table(unsafe { &*ptr });
                drop(unsafe { Box::from_raw(ptr) });
            }
        }

        /// The destructor body: frees the owned `ZTMarketing` (and its funding table) exactly like
        /// `clear_configurations`, since there's nothing left to tear down beyond what that method
        /// already does. Never frees `self` (`this`) itself - see `marketing_dtor_detour`'s doc comment
        /// for why.
        pub(crate) fn destroy(&mut self) {
            self.clear_configurations();
        }

        /// Always calls `clear_configurations` first, then bails out (returning `false`, with
        /// `marketing_ptr` left dangling) if the top-level file can't be opened. Otherwise allocates a
        /// fresh, default `ZTMarketing`, attaches it to `marketing_ptr` immediately, reads the
        /// `[marketing] marketing=<path>` value giving the actual funding `.cfg` file, and calls
        /// `ZTMarketing::load_configuration` on it. On failure, `set_funding_level(0)` is called on the
        /// already-attached `ZTMarketing` before returning `false`.
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
            info!("marketing-config-reimplementation: resolved funding cfg path '{cfg_path}' from top-level file");
            let ok = unsafe { &mut *marketing_ptr }.load_configuration(&cfg_path);
            if !ok {
                unsafe { &mut *marketing_ptr }.set_funding_level(0);
            }
            ok
        }
    }

    /// Detours `ZTMarketingMgr::loadConfigurations`/`clearConfigurations` onto this module's native
    /// reimplementation - the only two entry points that need detouring.
    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztmarketingmgr::{CLEAR_CONFIGURATIONS, LOAD_CONFIGURATIONS};

        use super::*;
        use crate::util::mut_from_memory;

        #[detour(LOAD_CONFIGURATIONS)]
        unsafe extern "thiscall" fn load_configurations(this: *const u32, path: *const i8) -> u32 {
            let path_str = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();

            // Parse the top-level file independently before mutating anything; fall back to vanilla
            // entirely if it fails, since nothing has been mutated yet.
            if super::read_cfg(&path_str).is_none() {
                error!("marketing-config-reimplementation: failed to independently parse '{path_str}', falling back to vanilla");
                return unsafe { LOAD_CONFIGURATIONS_DETOUR.call(this, path) };
            }

            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            let ok = mgr.load_configurations(&path_str);
            let level_count = mgr.marketing().map(|m| m.funding_levels().len()).unwrap_or(0);
            info!("marketing-config-reimplementation: loadConfigurations(\"{path_str}\") replaced natively -> {ok} ({level_count} funding levels)");
            ok as u32
        }

        #[detour(CLEAR_CONFIGURATIONS)]
        unsafe extern "thiscall" fn clear_configurations(this: *const u32) {
            let mgr = unsafe { mut_from_memory::<ZTMarketingMgr>(this) };
            mgr.clear_configurations();
        }
    }

    /// Installs the `loadConfigurations`/`clearConfigurations` detour. Called from
    /// `ztmarketing::init()`.
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise marketing-config-reimplementation detours: {e:?}");
        }
    }
}

/// Detours `ZTMarketingMgr`'s vtable destructor slot - the scalar deleting destructor at `0x00504f89`
/// (`ZTMARKETING_MGR_1` in `generated.rs`) - onto [`ZTMarketingMgr::destroy`]. Vanilla's own version of
/// this function calls the real destructor body (`ZTMARKETING_MGR_0`, `0x00504f73`), then conditionally
/// calls `operator delete` on `this` if the caller-supplied flag byte's low bit is set. Left undetoured,
/// that real body runs `operator delete` on `marketing_ptr` - the funding-table buffer and `ZTMarketing`
/// struct our own `marketing_config_reimplementation` allocates through Rust's global allocator - the
/// same cross-allocator hazard CLAUDE.md's "Live Reimplementation-Comparison Tests" section documents
/// for `ZTThoughtMgr`. Since `ZTMarketingMgr` is a process-lifetime singleton and no address for the real
/// vanilla `operator delete` this class would use is known or needed, this reimplementation only ever
/// frees the funding table and the `Box`-allocated `ZTMarketing`, never the flag-gated `this` itself.
/// `ZTMARKETING_MGR_0` (the real destructor body's own address, only ever reached indirectly through
/// this wrapper) is intentionally left un-detoured: nothing else in vanilla calls it directly.
mod marketing_dtor_detour {
    use openzt_detour::generated::ztmarketingmgr::ZTMARKETING_MGR_1;
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;
    use crate::util::mut_from_memory;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(ZTMARKETING_MGR_1)]
        unsafe extern "thiscall" fn ztmarketingmgr_dtor(this: *const u32, _flags: u8) -> *const u32 {
            unsafe { mut_from_memory::<ZTMarketingMgr>(this) }.destroy();
            this
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztmarketingmgr destructor detour: {e:?}");
        }
    }
}

/// registers the marketing module's live detours
pub fn init() {
    marketing_save_reimplementation::init();
    marketing_config_reimplementation::init();
    marketing_update_reimplementation::init();
    marketing_dtor_detour::init();
}

/// Synthetic `ZTMarketing` construction/teardown for the live `reimplementation_tests` comparison
/// harness. Every allocation goes through Rust's own allocator, never spliced into any real
/// `ZTMarketingMgr`.
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

    /// Builds a standalone `ZTMarketing` with `level_count` dummy (zeroed) funding-level entries.
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

    /// Builds a standalone `ZTMarketing` for the funding-text comparison test, with `levels` as the
    /// funding table verbatim, in order.
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

    /// Builds a standalone `ZTMarketingMgr` - **not** the real live singleton - wired to own
    /// `marketing_ptr` (or none, if null).
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
    /// restores whatever it held before this call.
    pub(crate) fn with_ztgamemgr_cash<R>(cash: f32, f: impl FnOnce() -> R) -> R {
        let game_mgr = unsafe { &mut *global_ztgamemgr_ptr() };
        let original = game_mgr.cash();
        game_mgr.set_cash(cash);

        let result = f();

        unsafe { &mut *global_ztgamemgr_ptr() }.set_cash(original);
        result
    }

    /// Exposed for `reimplementation_tests` to null-check `GLOBAL_ZTGameMgr`'s raw slot before running
    /// a comparison.
    pub(crate) fn ztgamemgr_ptr_is_null() -> bool {
        global_ztgamemgr_ptr().is_null()
    }
}
