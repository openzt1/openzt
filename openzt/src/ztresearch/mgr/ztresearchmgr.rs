use std::fmt;

use openzt_detour::generated::{standalone, ztresearchmgr};

use crate::globals::get_module_base;
use crate::util::{get_from_memory, mut_from_memory, ref_from_memory, ZTArray};
use crate::ztresearch::models::{ZTResearchBranch, ZTResearchCategory, ZTResearchEffectKind, ZTResearchProgram};

/// The global research manager, one per game. Confirmed size `0x18` bytes (`openzt-detour/src/structs.rs`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTResearchMgr {
    pub(crate) pad0: [u8; 0x8],                          // 0x00 - vtable? see `always_check_expansion` below for the flag byte both `ZTResearchBranch::update`/`pickRandomProgram` read via pointer arithmetic that lands just past this struct's own confirmed 0x18 bytes, not a real field of it
    pub(crate) elapsed_ticks: u32,                       // 0x08 - accumulates `ZTResearchMgr::update`'s delta; once ~359 in-game days have accrued, every branch is updated and this resets to 0
    pub(crate) branch_array: ZTArray<ZTResearchBranch>,  // 0x0c
}

/// Pure prediction for `ZTResearchMgr::update`'s accumulator/day-count bookkeeping, per
/// `private/resources/decompiles/ZTResearchMgr_update.c`/`.asm`. `delta_ticks` is added to
/// `elapsed_ticks_before` using plain 32-bit wrapping arithmetic (confirmed by the `.c`/`.asm`'s
/// `dword`-typed accumulator - it really does wrap, not saturate or widen). The result is then
/// converted to a day count via `(elapsed_ticks * 0x1c20) / 60000`; the `.asm`'s
/// `LEA`/`SHL`/multiply-by-`0x45e7b273`/`SHR` sequence is the standard "divide by constant" reciprocal-
/// multiplication idiom applied to the **already 32-bit-wrapped** product `elapsed_ticks * 0x1c20` (the
/// `LEA`/`SHL` chain computing that product is itself plain 32-bit register arithmetic, so it silently
/// wraps for large `elapsed_ticks` before the division ever happens) - so this reimplementation
/// deliberately uses `wrapping_mul` rather than a widened 64-bit product, to match vanilla exactly
/// including that overflow quirk. Once the day count exceeds `0x167` (359), the real function zeroes
/// `elapsed_ticks` and advances every branch by that many days; returns `(new_elapsed_ticks, 0)` when no
/// threshold crossing happens (no branch update), or `(0, days)` when one does.
///
/// The real function's return value is not modeled here at all: per its own decompile, `dVar1` starts
/// as `elapsed_ticks * 0x147ae260` but gets unconditionally overwritten with `this->branch_array`'s raw
/// pointer on every loop iteration whenever the threshold is crossed - a decompiler/register-reuse
/// artifact, not a meaningful return value the game relies on.
pub(crate) fn predict_update(elapsed_ticks_before: u32, delta_ticks: u32) -> (u32, u32) {
    let elapsed_ticks = elapsed_ticks_before.wrapping_add(delta_ticks);
    let days = elapsed_ticks.wrapping_mul(0x1c20) / 60000;
    if days > 0x167 {
        (0, days)
    } else {
        (elapsed_ticks, 0)
    }
}

#[cfg(test)]
mod predict_update_tests {
    use super::*;

    #[test]
    fn accumulates_without_crossing_threshold() {
        assert_eq!(predict_update(100, 50), (150, 0));
    }

    #[test]
    fn day_count_of_359_does_not_trigger() {
        // accumulated=2999 -> 2999*7200/60000 = 359 (floor); the real function only triggers when
        // the day count is *greater than* 359 (`0x167 < uVar4`, not `<=`).
        assert_eq!(predict_update(0, 2999), (2999, 0));
    }

    #[test]
    fn day_count_of_360_resets_and_returns_days() {
        // accumulated=3000 -> 3000*7200/60000 = 360 exactly, crossing the threshold.
        assert_eq!(predict_update(0, 3000), (0, 360));
    }

    #[test]
    fn elapsed_ticks_wraps_on_accumulation() {
        assert_eq!(predict_update(u32::MAX, 1), (0, 0));
    }

    #[test]
    fn ticks_to_day_conversion_wraps_like_vanilla() {
        // 596524 * 0x1c20 = 4294972800, which overflows u32::MAX (4294967295) by 5504 - so the
        // wrapped product divided by 60000 gives 0 days, even though the true (non-wrapping) division
        // would be ~71583 days, well past the threshold. This deliberately replicates the vanilla
        // overflow quirk rather than "fixing" it.
        assert_eq!(predict_update(0, 596_524), (596_524, 0));
    }
}

/// Resolves `GLOBAL_ZTGameMgr` fresh from its raw memory slot, for the same reason
/// `global_always_check_expansion` below bypasses `globals()`'s cached resolution instead of using
/// `globals().ztgamemgr_ptr()` directly - see that function's own doc comment. `ZTResearchBranch::update`
/// needs this for both the affordability check and `subtract_cash`.
pub(crate) fn global_ztgamemgr_ptr() -> *mut crate::ztgamemgr::ZTGameMgr {
    get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + 0x0023_8048) as *mut crate::ztgamemgr::ZTGameMgr
}

/// Resolves `GLOBAL_ZTResearchMgr` fresh from its raw memory slot and reads its
/// `ZTResearchMgr::always_check_expansion` flag - used by `ZTResearchBranch::update` instead of
/// `globals().ztresearchmgr()`, whose `CachedGlobalInstance` resolves the pointer chain **once** and
/// caches it forever, unlike vanilla's own `MOV EAX, GLOBAL_ZTResearchMgr` (a fresh read every call).
/// That mismatch is invisible in real gameplay (there is only ever one real singleton, and the cache
/// resolves to it correctly once constructed), but breaks
/// `reimplementation_tests::live_support::with_global_ztresearchmgr_ptr`'s test-time redirection - which
/// patches this same raw slot, exactly like vanilla reads it, but has no way to invalidate `Globals`'
/// separate cache. Returns `false` if the global hasn't been constructed yet (`globals()`'s own accessors
/// null-check the same way; vanilla itself has no such guard here, but nothing calls `ZTResearchBranch::
/// update` before the real singleton exists either way).
pub(crate) fn global_always_check_expansion() -> bool {
    let mgr_ptr = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + 0x0023_9010);
    if mgr_ptr != 0 {
        unsafe { &*(mgr_ptr as *const ZTResearchMgr) }.always_check_expansion()
    } else {
        false
    }
}

impl ZTResearchMgr {
    /// The flag byte `ZTResearchBranch::update`/`pickRandomProgram` both read via
    /// `GLOBAL_ZTResearchMgr+1` pointer arithmetic in the `.c` decompiles - since `ZTResearchMgr` itself
    /// is typed as `0x18` bytes there (its own confirmed size), that `+1` scales to byte offset `0x18`,
    /// confirmed directly against the real `.asm` for both call sites (`MOV %CL, byte ptr [EAX + 0x18]`
    /// in `ZTResearchBranch_update.asm`; `*(char *)(local_1c + 1)` with `local_1c` typed
    /// `ZTResearchMgr *` in `ZTResearchBranch_pickRandomProgram.c`) - one byte past this struct's own
    /// fields entirely. Read raw via pointer arithmetic rather
    /// than modeled as a struct field, to avoid inflating `ZTResearchMgr`'s own independently-confirmed
    /// size.
    ///
    /// When `true`, `update`/`pick_random_program` always run the per-category `isExpansionDisabled`
    /// check instead of only when `getAnyExpansionsDisabled()` is true; the flag's own deeper purpose
    /// is otherwise unconfirmed.
    pub(crate) fn always_check_expansion(&self) -> bool {
        get_from_memory::<u8>((self as *const Self as u32) + 0x18) != 0
    }

    /// Exposed for the live `reimplementation_tests` comparison harness - see `predict_update`.
    pub(crate) fn elapsed_ticks(&self) -> u32 {
        self.elapsed_ticks
    }

    /// Exposed for the live `reimplementation_tests` comparison harness, to seed a synthetic manager's
    /// accumulator before comparing `ZTResearchMgr::update` against the reimplementation - see
    /// `predict_update`.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn set_elapsed_ticks(&mut self, value: u32) {
        self.elapsed_ticks = value;
    }

    pub fn branch_count(&self) -> usize {
        self.branch_array.len()
    }

    pub fn branch(&self, index: usize) -> &'static ZTResearchBranch {
        unsafe { ref_from_memory(self.branch_array.get_ptr(index)) }
    }

    pub fn branch_mut(&self, index: usize) -> &'static mut ZTResearchBranch {
        unsafe { mut_from_memory(self.branch_array.get_ptr(index)) }
    }

    pub fn branches(&self) -> impl Iterator<Item = &'static ZTResearchBranch> + '_ {
        (0..self.branch_count()).map(move |i| self.branch(i))
    }

    pub fn branches_mut(&self) -> impl Iterator<Item = &'static mut ZTResearchBranch> + '_ {
        (0..self.branch_count()).map(move |i| self.branch_mut(i))
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchMgr::getBranch`.
    pub fn get_branch(&self, id: i32) -> Option<&'static ZTResearchBranch> {
        self.branches().find(|branch| branch.id == id)
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchMgr::getCategory`.
    pub fn get_category(&self, id: i32) -> Option<&'static ZTResearchCategory> {
        self.branches().flat_map(|branch| branch.categories()).find(|category| category.id == id)
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchMgr::getProgram`.
    pub fn get_program(&self, id: i32) -> Option<&'static ZTResearchProgram> {
        self.branches()
            .flat_map(|branch| branch.categories())
            .flat_map(|category| category.programs())
            .find(|program| program.id == id)
    }

    /// Mutable counterpart to `get_branch`, used by `research_save_reimplementation`'s `load` detour to apply a saved `current_funding_level` to the matching branch.
    pub(crate) fn get_branch_mut(&self, id: i32) -> Option<&'static mut ZTResearchBranch> {
        self.branches_mut().find(|branch| branch.id == id)
    }

    /// Mutable counterpart to `get_category`, used by `research_save_reimplementation`'s `load` detour to apply a saved `enabled` flag to the matching category.
    pub(crate) fn get_category_mut(&self, id: i32) -> Option<&'static mut ZTResearchCategory> {
        self.branches_mut().flat_map(|branch| branch.categories_mut()).find(|category| category.id == id)
    }

    /// Mutable counterpart to `get_program`, used by `research_save_reimplementation`'s `load` detour to apply a saved `current_progress` to the matching program.
    pub(crate) fn get_program_mut(&self, id: i32) -> Option<&'static mut ZTResearchProgram> {
        self.branches_mut()
            .flat_map(|branch| branch.categories_mut())
            .flat_map(|category| category.programs_mut())
            .find(|program| program.id == id)
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchMgr::setEffectDiscount`: applies a percentage
    /// discount to the `target_cost` of every program whose effect kind matches `kind`.
    pub fn set_effect_discount(&self, kind: ZTResearchEffectKind, discount_pct: i32) {
        for program in self.branches_mut().flat_map(|b| b.categories_mut()).flat_map(|c| c.programs_mut()) {
            if program.effect_kind_raw == kind as i32 {
                program.target_cost = (100 - discount_pct) as f32 * program.target_cost * 0.01;
            }
        }
    }

    /// Native reimplementation of `ZTResearchMgr::update`'s accumulator/day-count bookkeeping (see
    /// `predict_update`): `delta_ticks` is added to `elapsed_ticks`; once enough time has accrued,
    /// `elapsed_ticks` resets to `0` and every branch is advanced by the elapsed day count via
    /// `ZTResearchBranch::update` - still a call into the original implementation (see its own doc
    /// comment), same as everywhere else in this file that isn't independently reimplemented.
    pub fn update(&mut self, delta_ticks: u32) {
        let (new_elapsed_ticks, days) = predict_update(self.elapsed_ticks, delta_ticks);
        self.elapsed_ticks = new_elapsed_ticks;
        if days > 0 {
            for branch in self.branches_mut() {
                branch.update(days);
            }
        }
    }

    /// Calls `ZTResearchMgr::save`. `file` is whatever file-handle pointer the original
    /// `WriteBytesToFile` calls expect. By default (see `research_save_reimplementation::detours`)
    /// this address is detoured onto that module's native reimplementation
    /// (`serialize(&snapshot_mgr(self))`, written via `standalone::WRITE_BYTES_TO_FILE`), and
    /// `.hooked()` deliberately re-enters that hook, exactly like a vanilla caller would; under the
    /// `vanilla-research-save` feature no detour is installed and the address still holds genuine
    /// vanilla code.
    pub fn save(&self, file: *const u32) -> bool {
        unsafe { ztresearchmgr::SAVE.hooked()((self as *const Self) as *const u32, file) }
    }

    /// Calls `ZTResearchMgr::load` - the save-file counterpart to `save()`. Per
    /// `private/resources/decompiles/ZTResearchMgr_load.c`/`.asm`, `load` always starts by resetting every
    /// branch's `current_funding_level` to `0`, every category's `enabled` to `1`, and calling
    /// `ZTResearchProgram::reset()` on every program (which itself zeroes `current_progress` and, for
    /// `UnlockEntity`/`BuildingUpgrade` effects, calls back into the building/entity managers) -
    /// unconditionally, regardless of `version` or what's in the stream. Only if `version >= 0x28`
    /// does it then read a stream of `(kind, id, value)` tuples from `file` (`kind` 0 = a branch's
    /// `current_funding_level`, clamped to `0` if the saved value is `>=` that branch's own
    /// funding-level count; 1 = a category's `enabled` flag; 2 = a program's `current_progress`) and
    /// apply each one to the matching branch/category/program found via
    /// `get_branch`/`get_category`/`get_program` (an id with no match is silently skipped). Finally,
    /// regardless of `version`, it calls `ZTResearchProgram::on_completion()` on any program whose
    /// `current_progress >= target_cost` and `ZTResearchBranch::pick_random_program()` on every
    /// branch (consuming the game's RNG stream). Does **not** load research definitions from `.cfg`
    /// files - that's `ZTResearchBranch::load_branch`/`ZTResearchCategory::load_category`/
    /// `ZTResearchProgram::load_program`. By default this address is detoured onto
    /// `research_save_reimplementation::detours::load`, a native reimplementation of exactly the
    /// behavior described above (reading the stream via `standalone::DEALLOCATE`), and `.hooked()`
    /// deliberately re-enters that hook, exactly like a vanilla caller would; under the
    /// `vanilla-research-save` feature no detour is installed and the address still holds genuine
    /// vanilla code.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        unsafe { ztresearchmgr::LOAD.hooked()((self as *mut Self) as *const u32, file, version) }
    }

    /// Reimplementation of `ZTResearchMgr::forceResearch` (the class-level half of the "research
    /// cheat"). Per `private/resources/decompiles/ZTResearchMgr_forceResearch.c`: for every branch, for every
    /// category, for every program (**not** just each branch's currently-selected program - the
    /// decompile walks every category's full `program_array`), calls `ZTResearchProgram::on_completion`
    /// unconditionally, then, only if `continue_program` is `true`, sets that program's
    /// `current_progress` to its `target_cost` (this is the "optionally carrying remaining progress"
    /// behavior - it does not check whether the program was already complete); once every category in
    /// a branch has been processed, calls `ZTResearchBranch::pick_random_program` once for that branch
    /// (left as a call into the original - see `pick_random_program`'s own doc comment on why). Unlike
    /// the actual in-game cheat button, this does *not* refresh the world/UI afterward - use the free
    /// function `force_research_cheat()` for that (it calls the vanilla standalone cheat function with
    /// `continue_program` hardcoded to `false`, matching what the button does, plus the refresh).
    pub fn force_research(&mut self, continue_program: bool) {
        for branch in self.branches_mut() {
            for category in branch.categories_mut() {
                for program in category.programs_mut() {
                    program.on_completion();
                    if continue_program {
                        program.current_progress = program.target_cost;
                    }
                }
            }
            branch.pick_random_program();
        }
    }

    /// Calls the vanilla `ZTResearchMgr::clearBranches`: destroys and frees every branch (and
    /// everything under it), then resets `branch_array` to empty.
    pub fn clear_branches(&mut self) {
        unsafe { ztresearchmgr::CLEAR_BRANCHES.hooked()((self as *mut Self) as *const u32) }
    }
}

impl fmt::Display for ZTResearchMgr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTResearchMgr {{")?;
        writeln!(f, "  elapsed_ticks: {},", self.elapsed_ticks)?;
        writeln!(f, "  branch_count: {},", self.branch_count())?;
        write!(f, "}}")
    }
}

/// Calls the vanilla standalone `forceResearch` cheat-console function - the one the in-game
/// "force research" cheat actually triggers, per `private/resources/decompiles/_forceResearch.c`. It calls
/// `ZTResearchMgr::forceResearch(GLOBAL_ZTResearchMgr, false)` (the `continue_program` flag is
/// hardcoded `false` here - use `ZTResearchMgr::force_research` directly if you need `true`) and
/// then notifies every `ZTWorldMgr` entity of the change, so the world/UI actually refreshes; plain
/// `ZTResearchMgr::force_research` alone does not do that last step. Takes no arguments and needs no
/// `ZTResearchMgr` reference - it looks up the global instance itself.
pub fn force_research_cheat() {
    unsafe { standalone::FORCE_RESEARCH.original()() }
}
