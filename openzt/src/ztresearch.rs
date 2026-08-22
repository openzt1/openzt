//! Structs and methods for the vanilla `ZTResearchMgr` / `ZTResearchBranch` / `ZTResearchCategory` /
//! `ZTResearchProgram` classes, which drive the zoo's research tree: picking which program a branch
//! (e.g. "Animal Care") is currently working towards, tracking funding and progress, and applying a
//! program's effect once it completes.
//!
//! Field layouts below are derived from the decompiles in `resources/decompiles/ZTResearch*` and
//! `resources/decompiles/_forceResearch.c` / `_clickResearch.c`. Byte ranges with no decompiled
//! evidence are left as `padN` placeholders. `ZTResearchMgr`'s layout is additionally confirmed by
//! `openzt-detour/src/structs.rs` (size `0x18`).
//!
//! Traversal/read-only calculations (`get_branch`/`get_category`/`get_program`, funding level
//! selection, `pct_remaining_on_program`/`days_remaining_on_program`, `set_effect_discount`) are
//! reimplemented natively in Rust since their vanilla logic is simple and fully understood. Functions
//! with complex or only
//! partially understood side effects (completion dispatch, save/load, RNG-based program selection)
//! call into the original game code via the addresses already registered in
//! `openzt-detour/src/generated.rs` rather than risk a subtly wrong reimplementation.

use std::{
    ffi::{c_char, CStr, CString},
    fmt,
    mem::size_of,
};

use num_enum::TryFromPrimitive;
use openzt_detour::generated::{
    bfuimgr, standalone, uicontrol, ztresearchbranch, ztresearchcategory, ztresearchmgr, ztresearchprogram, ztui_expansionselect, ztui_zoostatus,
};
use tracing::{error, info, warn};
use windows::{
    core::{PCSTR, PSTR},
    Win32::Globalization::{GetCurrencyFormatA, CURRENCYFMTA},
};

use crate::{
    bfconfigfile::BFConfigFile,
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    string_registry::load_string_by_id,
    util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory, ZTArray, ZTBufferString, ZTString},
};

/// Upper bounds used purely to stop `command_list_research` from looping forever/crashing on a
/// garbage count if the global address or a pointer chain turns out to be wrong; real vanilla data
/// is well under these.
const MAX_REASONABLE_BRANCHES: usize = 32;
const MAX_REASONABLE_CATEGORIES: usize = 128;
const MAX_REASONABLE_PROGRAMS: usize = 512;

/// The kind of effect a `ZTResearchProgram` applies once it completes, dispatched by the vanilla
/// `ZTResearchProgram::onCompletion` switch statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(i32)]
pub enum ZTResearchEffectKind {
    /// Calls `setAvail` (unlocks a building/scenery entity) then reports completion directly.
    UnlockEntity = 0,
    BuildingUpgrade = 1,
    EntityCharacteristic = 2,
    GenusCharacteristic = 3,
    FamilyCharacteristic = 4,
    FoodCharacteristic = 5,
    TrickAvailable = 6,
    /// Applies a percentage discount to matching programs' `target_cost`; see `ZTResearchMgr::set_effect_discount`.
    EffectDiscount = 7,
}

/// One entry in a `ZTResearchBranch`'s inline funding-level table (`funding_table_start..funding_table_end`,
/// stride `0xc`, *not* a `ZTArray` of pointers). Loaded from one of the named sub-blocks a branch's
/// `.cfg` `funding=` list references (e.g. `funding=normal` -> a `[normal]` block with its own
/// `name`/`cost`/`work` keys). Selecting a higher index via
/// `ZTResearchBranch::increase_funding`/`decrease_funding` changes the `rate` used when computing
/// research progress.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ZTResearchFundingLevel {
    name_id: i32, // 0x0 - confirmed: the sub-block's `name` string id (e.g. 23100 = "%s none", 23101 = "%s min", 23102 = "%s normal", 23103 = "%s max")
    rate: f32,    // 0x4 - confirmed: the sub-block's `work` value (e.g. none=0, min=21, normal=30, max=45); used as a divisor by the vanilla days/pct-remaining calculations, must be > 0 for this level to be considered active
    cost: f32,    // 0x8 - confirmed: the sub-block's `cost` value (e.g. none=0, min=400, normal=1000, max=2000)
}

impl ZTResearchFundingLevel {
    pub fn name_id(&self) -> i32 {
        self.name_id
    }

    /// Display name, resolved through the same string table `get_string()` uses (e.g. "%s none").
    pub fn name(&self) -> Option<String> {
        load_string_by_id(self.name_id as u32)
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn cost(&self) -> f32 {
        self.cost
    }
}

/// A single research program, e.g. "Improved Elephant Enclosure". Owned (by pointer) by a
/// `ZTResearchCategory`'s `program_array`. Loaded by `ZTResearchProgram::loadProgram`, which treats
/// `this` directly as a `BFConfigFile*` (i.e. `ZTResearchProgram` inherits `BFConfigFile`, which
/// occupies the first `0xc` bytes) and reads a `.cfg` block of the form
/// `name`/`desc`/`icon`/`entityIcon`/`cost`/`order`/`target`/`effect`/`effectval1`/`effectval2`/
/// `effectval3`/`helpid`.
#[derive(Debug)]
#[repr(C)]
pub struct ZTResearchProgram {
    config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`); `kind_tag` observed to always be 6 for research programs
    cached_name: ZTBufferString, // 0x0c - confirmed: built via `BFApp::buildString` from `id` (`name`) right after it's loaded
    cached_desc: ZTBufferString, // 0x18 - confirmed: built via `BFApp::buildString` from `desc_id` (`desc`) right after it's loaded
    desc_id: i32,                // 0x24 - confirmed: the `.cfg` `desc` string id, resolves via `get_string()`
    icon_ptr: u32,               // 0x28 - confirmed: the `.cfg` `icon` field, a raw C string pointer (only valid when non-null)
    entity_icon_ptr: u32,        // 0x2c - confirmed: the `.cfg` `entityIcon` field, a raw C string pointer; also the value `onCompletion` passes to `setBuildingUpgrade` as the building type name when `effect_kind == BuildingUpgrade`
    id: i32,                     // 0x30 - confirmed: the `.cfg` `name` string id, matched by `ZTResearchMgr::get_program`
    target_cost: f32,            // 0x34 - confirmed: the `.cfg` `cost` field
    current_progress: f32,       // 0x38 - not set by `loadProgram` (starts at 0 from the constructor); funding accumulated so far, complete once current_progress >= target_cost
    priority: u32,               // 0x3c - confirmed: the `.cfg` `order` field; tie-breaker used by `ZTResearchBranch::pick_random_program` when choosing the next not-yet-started program
    target_id: i32,              // 0x40 - confirmed: the `.cfg` `target` field; sentinel -1 when unset
    effect_kind_raw: i32,        // 0x44 - confirmed: the `.cfg` `effect` field; see `ZTResearchEffectKind`; sentinel -1 when unset; dispatches `ZTResearchProgram::on_completion`
    effect_param_0: i32,         // 0x48 - confirmed: the `.cfg` `effectval1` field
    effect_param_1: i32,         // 0x4c - confirmed: the `.cfg` `effectval2` field
    effect_param_2: i32,         // 0x50 - confirmed: the `.cfg` `effectval3` field
    help_id: i32,                // 0x54 - confirmed: the `.cfg` `helpid` field (only set if present; 0 by default from the constructor)
}

/// The underlying manager calls `ZTResearchProgram::on_completion`/`reset` dispatch into (still
/// opaque calls into the original game code, same as everywhere else those aren't independently
/// reimplemented in OpenZT), abstracted so `dispatch_on_completion`/`dispatch_reset`'s own
/// branching/bookkeeping logic - which case fires, and whether the zoostatus completed-research list
/// gets touched - can be pure-tested against a mock, without touching real game memory. Every method
/// but the two `add`/`remove_completed_research` notifications takes `&ZTResearchProgram` rather than
/// individual fields, matching how vanilla reads straight out of `this`. `LiveResearchEffects` below
/// is the only real implementation.
trait ResearchEffects {
    fn set_avail(&mut self, target_id: i32, avail: bool);
    fn set_building_upgrade(&mut self, program: &ZTResearchProgram, install: bool) -> bool;
    fn set_entity_characteristic(&mut self, program: &ZTResearchProgram) -> bool;
    fn set_genus_characteristic(&mut self, program: &ZTResearchProgram) -> bool;
    fn set_family_characteristic(&mut self, program: &ZTResearchProgram) -> bool;
    fn set_food_characteristic(&mut self, program: &ZTResearchProgram) -> bool;
    fn set_trick_available(&mut self, program: &ZTResearchProgram) -> bool;
    fn set_effect_discount(&mut self, program: &ZTResearchProgram) -> bool;
    fn add_completed_research(&mut self, program_ptr: *mut ZTResearchProgram);
    fn remove_completed_research(&mut self, program_ptr: *mut ZTResearchProgram);
}

/// Pure dispatch decision for `ZTResearchProgram::onCompletion`, per
/// `resources/decompiles/ZTResearchProgram_onCompletion.c`: every valid `effect_kind` calls exactly
/// one underlying effect function, then notifies `ZTUI::zoostatus` iff that call reports success. An
/// invalid `effect_kind_raw` (anything `ZTResearchEffectKind::try_from` rejects) is a no-op.
fn dispatch_on_completion(effects: &mut impl ResearchEffects, program: &mut ZTResearchProgram) -> bool {
    let success = match program.effect_kind() {
        Some(ZTResearchEffectKind::UnlockEntity) => {
            effects.set_avail(program.target_id, true);
            true
        }
        Some(ZTResearchEffectKind::BuildingUpgrade) => effects.set_building_upgrade(program, true),
        Some(ZTResearchEffectKind::EntityCharacteristic) => effects.set_entity_characteristic(program),
        Some(ZTResearchEffectKind::GenusCharacteristic) => effects.set_genus_characteristic(program),
        Some(ZTResearchEffectKind::FamilyCharacteristic) => effects.set_family_characteristic(program),
        Some(ZTResearchEffectKind::FoodCharacteristic) => effects.set_food_characteristic(program),
        Some(ZTResearchEffectKind::TrickAvailable) => effects.set_trick_available(program),
        Some(ZTResearchEffectKind::EffectDiscount) => effects.set_effect_discount(program),
        None => return false,
    };
    if success {
        effects.add_completed_research(program as *mut ZTResearchProgram);
    }
    success
}

/// Pure dispatch decision for `ZTResearchProgram::reset`, per
/// `resources/decompiles/ZTResearchProgram_reset.c`/`.asm`. Always zeroes `current_progress` first,
/// unconditionally - confirmed via `.asm`: `this->current_progress = 0` runs before the switch even
/// dispatches, for every `effect_kind_raw` including invalid ones. Beyond that, notably asymmetric
/// with `dispatch_on_completion` above:
/// - `UnlockEntity` (0): calls `setAvail(target_id, false)`, then unconditionally removes the program
///   from the completed-research list.
/// - `BuildingUpgrade` (1): calls `setBuildingUpgrade(..., install=false)`; only removes from the
///   completed-research list if that call reports success.
/// - Unset (`-1`) and `EntityCharacteristic`..`EffectDiscount` (2..=7): no underlying call at all -
///   just unconditionally removes the program from the completed-research list. The C decompile only
///   shows explicit case labels for `-1,2,3,4,5,6`, leaving `7` looking like it falls into the
///   "invalid" default - but that's the decompiler under-reporting the jump table: confirmed **live**
///   (`ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`'s comparison against real vanilla `reset()`) that
///   `EffectDiscount` (`7`) returns success here too, meaning the compiled jump table's 9th slot
///   aliases to the very same block `-1`/`2..=6` use, just like every one of them skipping the
///   effect-specific call entirely (unlike `on_completion`, which does call the matching
///   `set*Characteristic`/`setTrickAvailable`/`setEffectDiscount` for every one of these).
/// - Anything outside `-1..=7`: a genuine no-op (beyond the unconditional `current_progress` reset
///   above) - this is the real out-of-range default, confirmed by the `.asm`'s `JA` guard.
fn dispatch_reset(effects: &mut impl ResearchEffects, program: &mut ZTResearchProgram) -> bool {
    program.current_progress = 0.0;
    match program.effect_kind_raw {
        0 => {
            effects.set_avail(program.target_id, false);
            effects.remove_completed_research(program as *mut ZTResearchProgram);
            true
        }
        1 => {
            let success = effects.set_building_upgrade(program, false);
            if success {
                effects.remove_completed_research(program as *mut ZTResearchProgram);
            }
            success
        }
        -1 | 2..=7 => {
            effects.remove_completed_research(program as *mut ZTResearchProgram);
            true
        }
        _ => false,
    }
}

/// The real `ResearchEffects`: calls straight into the addresses `on_completion`/`reset` dispatch to
/// in vanilla, via `openzt-detour/src/generated.rs`. `set_building_upgrade`/`set_trick_available`/
/// `set_effect_discount` used to mask their raw return value to its low byte before checking success:
/// per their own decompiles, they return `CONCAT31(garbage, success_flag)` - the upper 3 bytes are
/// whatever was left over in EAX from an unrelated prior call, not part of the real result. Confirmed
/// live: without the mask, `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`'s live comparison test
/// intermittently reported a spurious success for e.g. `TrickAvailable` even when the real call's
/// actual (low-byte) result was failure. A Ghidra regen has since retyped all six of these (this trio
/// included) to return `bool` rather than `u32`/`i8`/etc - matching the real, single-byte C++ `bool`
/// these functions actually return - so the manual mask is gone: `bool`'s `extern "cdecl"` ABI already
/// reads only the low byte (`AL`) the same way vanilla's own callers do, which is exactly what the
/// mask was manually working around. Re-confirmed live post-regen against the same
/// `ZTRESEARCHPROGRAM_ON_COMPLETION_RESET` test that originally caught the garbage-upper-bytes bug.
struct LiveResearchEffects;

impl ResearchEffects for LiveResearchEffects {
    fn set_avail(&mut self, target_id: i32, avail: bool) {
        unsafe { standalone::SET_AVAIL.original()(target_id, avail as u32) }
    }

    fn set_building_upgrade(&mut self, program: &ZTResearchProgram, install: bool) -> bool {
        unsafe {
            standalone::SET_BUILDING_UPGRADE.original()(
                program.target_id,
                program.id,
                program.entity_icon_ptr as *const i8,
                program.effect_param_0,
                program.effect_param_1,
                program.effect_param_2,
                install as i8,
            )
        }
    }

    fn set_entity_characteristic(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe {
            standalone::SET_ENTITY_CHARACTERISTIC.original()(
                program.target_id,
                program.effect_param_0,
                program.effect_param_1,
                program.effect_param_2 as u8,
            )
        }
    }

    fn set_genus_characteristic(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe {
            standalone::SET_GENUS_CHARACTERISTIC.original()(
                program.target_id,
                program.effect_param_0,
                program.effect_param_1,
                program.effect_param_2 as i8,
            )
        }
    }

    fn set_family_characteristic(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe {
            standalone::SET_FAMILY_CHARACTERISTIC.original()(
                program.target_id,
                program.effect_param_0,
                program.effect_param_1,
                program.effect_param_2 as i8,
            )
        }
    }

    fn set_food_characteristic(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe {
            standalone::SET_FOOD_CHARACTERISTIC.original()(
                program.target_id,
                program.effect_param_0,
                program.effect_param_1,
                program.effect_param_2 as u32,
            )
        }
    }

    fn set_trick_available(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe { standalone::SET_TRICK_AVAILABLE.original()(program.target_id, program.effect_param_0 as u32) }
    }

    fn set_effect_discount(&mut self, program: &ZTResearchProgram) -> bool {
        unsafe { standalone::SET_EFFECT_DISCOUNT.original()(program.target_id, program.effect_param_0, program.effect_param_1, program.effect_param_2) }
    }

    fn add_completed_research(&mut self, program_ptr: *mut ZTResearchProgram) {
        unsafe { ztui_zoostatus::ADD_COMPLETED_RESEARCH.original()(program_ptr as i32) }
    }

    fn remove_completed_research(&mut self, program_ptr: *mut ZTResearchProgram) {
        unsafe { ztui_zoostatus::REMOVE_COMPLETED_RESEARCH.original()(program_ptr as *const i32) }
    }
}

impl ZTResearchProgram {
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Display name, resolved through the same string table `get_string()` uses. Confirmed against
    /// the live game and against `ZTResearchProgram::loadProgram` (built from `id`/`name` via
    /// `BFApp::buildString` into `cached_name`).
    pub fn name(&self) -> Option<String> {
        load_string_by_id(self.id as u32)
    }

    /// The load-time cached copy of `name()`'s text (see `loadProgram`), rather than a fresh
    /// `get_string()` lookup.
    pub fn cached_name(&self) -> String {
        self.cached_name.copy_to_string()
    }

    /// The `.cfg` `desc` field's text, resolved through `get_string()`. Confirmed against the live
    /// game and against `loadProgram`.
    pub fn desc(&self) -> Option<String> {
        load_string_by_id(self.desc_id as u32)
    }

    /// The load-time cached copy of `desc()`'s text (see `loadProgram`), rather than a fresh
    /// `get_string()` lookup.
    pub fn cached_desc(&self) -> String {
        self.cached_desc.copy_to_string()
    }

    /// `BFConfigFile`'s "has data" flag; effectively always `true` for any program you can reach
    /// through `ZTResearchMgr`/`ZTResearchCategory`, since `loadProgram` bails out before finishing
    /// construction otherwise.
    pub fn is_config_loaded(&self) -> bool {
        self.config_file.is_loaded()
    }

    /// `BFConfigFile`'s internal "kind of config" tag; confirmed to always be `6` for research
    /// programs so far. Not useful for anything program-specific, just here for completeness.
    pub fn config_kind_tag(&self) -> u8 {
        self.config_file.kind_tag()
    }

    /// The `.cfg` `icon` field, if set.
    pub fn icon(&self) -> Option<String> {
        (self.icon_ptr != 0).then(|| unsafe { CStr::from_ptr(self.icon_ptr as *const c_char) }.to_string_lossy().into_owned())
    }

    /// The `.cfg` `entityIcon` field, if set. See `building_type_name` for its reuse as a building
    /// type name string when `effect_kind == BuildingUpgrade`.
    pub fn entity_icon(&self) -> Option<String> {
        (self.entity_icon_ptr != 0)
            .then(|| unsafe { CStr::from_ptr(self.entity_icon_ptr as *const c_char) }.to_string_lossy().into_owned())
    }

    /// The building type name passed to `setBuildingUpgrade` on completion - actually just
    /// `entity_icon()`, reused for this purpose. Only meaningful when
    /// `effect_kind() == Some(ZTResearchEffectKind::BuildingUpgrade)`; returns `None` otherwise to
    /// match what `onCompletion` itself does (every other effect kind ignores this field).
    pub fn building_type_name(&self) -> Option<String> {
        if self.effect_kind() != Some(ZTResearchEffectKind::BuildingUpgrade) {
            return None;
        }
        self.entity_icon()
    }

    pub fn target_cost(&self) -> f32 {
        self.target_cost
    }

    pub fn current_progress(&self) -> f32 {
        self.current_progress
    }

    pub fn is_complete(&self) -> bool {
        self.current_progress >= self.target_cost
    }

    pub fn priority(&self) -> u32 {
        self.priority
    }

    pub fn target_id(&self) -> i32 {
        self.target_id
    }

    pub fn effect_kind(&self) -> Option<ZTResearchEffectKind> {
        ZTResearchEffectKind::try_from(self.effect_kind_raw).ok()
    }

    pub fn effect_params(&self) -> (i32, i32, i32) {
        (self.effect_param_0, self.effect_param_1, self.effect_param_2)
    }

    /// The raw `.cfg` `helpid` field. Prefer this over `tooltip()` for on-screen UI tooltips - the
    /// game's help system has separate short/long text variants selected by a user setting (see
    /// `ui::tooltip::tooltip_text_from_id`'s `LONG_TOOLTIP_ID_OFFSET`), which only UI code knows how
    /// to apply; `tooltip()` below does a plain lookup that ignores that preference.
    pub fn help_id(&self) -> i32 {
        self.help_id
    }

    /// The `.cfg` `helpid` field's text, resolved via a direct (non-long/short-aware) string lookup.
    /// Fine for debug/console output; UI code displaying an on-screen tooltip should use `help_id()`
    /// with `ui::vanilla_main`'s tooltip helpers instead, so the long-tooltip user setting is honored.
    pub fn tooltip(&self) -> Option<String> {
        load_string_by_id(self.help_id as u32)
    }

    /// Reimplementation of `ZTResearchProgram::onCompletion`: dispatches on `effect_kind` into one of
    /// several other managers (building/entity/genus/family/food/trick/discount - still opaque calls
    /// into the original game code, same as everywhere else in OpenZT those aren't independently
    /// reimplemented) and reports the completion to `ZTUI::zoostatus` if the underlying call reports
    /// success. See `dispatch_on_completion` for the dispatch logic itself and
    /// `ZTResearchProgram_onCompletion.c` for the source this was reimplemented from. Returns `1` on
    /// success, `0` otherwise (vanilla's raw return value has undefined garbage in its upper 3 bytes -
    /// only the low byte, i.e. this success flag, is ever meaningful).
    pub fn on_completion(&mut self) -> u32 {
        dispatch_on_completion(&mut LiveResearchEffects, self) as u32
    }

    /// Reimplementation of `ZTResearchProgram::reset`. Always zeroes `current_progress`, regardless
    /// of `effect_kind`. See `dispatch_reset` for the rest of the dispatch logic and
    /// `ZTResearchProgram_reset.c` for the source this was reimplemented from - notably, unlike
    /// `on_completion`, only `UnlockEntity`/`BuildingUpgrade` effects call back into their underlying
    /// manager; every other valid effect kind (`EntityCharacteristic` through `EffectDiscount`) just
    /// unconditionally removes the program from the completed-research list with no attempt to undo
    /// the effect. Returns `1` on success, `0` otherwise (see `on_completion`'s doc comment on why the
    /// raw vanilla return value isn't reproduced exactly).
    pub fn reset(&mut self) -> u32 {
        dispatch_reset(&mut LiveResearchEffects, self) as u32
    }

    pub fn load_program(&mut self, reader: *const u32) -> u32 {
        unsafe { ztresearchprogram::LOAD_PROGRAM.original()((self as *mut Self) as *const u32, reader) }
    }
}

#[cfg(test)]
mod effect_dispatch_tests {
    use super::*;

    /// Records every `ResearchEffects` call made against it, in order, and returns caller-configured
    /// canned results for the calls that report success/failure.
    #[derive(Debug, Default)]
    struct MockEffects {
        calls: Vec<String>,
        set_building_upgrade_result: bool,
        set_entity_characteristic_result: bool,
        set_genus_characteristic_result: bool,
        set_family_characteristic_result: bool,
        set_food_characteristic_result: bool,
        set_trick_available_result: bool,
        set_effect_discount_result: bool,
    }

    impl ResearchEffects for MockEffects {
        fn set_avail(&mut self, target_id: i32, avail: bool) {
            self.calls.push(format!("set_avail({target_id}, {avail})"));
        }

        fn set_building_upgrade(&mut self, _program: &ZTResearchProgram, install: bool) -> bool {
            self.calls.push(format!("set_building_upgrade(install={install})"));
            self.set_building_upgrade_result
        }

        fn set_entity_characteristic(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_entity_characteristic".to_string());
            self.set_entity_characteristic_result
        }

        fn set_genus_characteristic(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_genus_characteristic".to_string());
            self.set_genus_characteristic_result
        }

        fn set_family_characteristic(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_family_characteristic".to_string());
            self.set_family_characteristic_result
        }

        fn set_food_characteristic(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_food_characteristic".to_string());
            self.set_food_characteristic_result
        }

        fn set_trick_available(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_trick_available".to_string());
            self.set_trick_available_result
        }

        fn set_effect_discount(&mut self, _program: &ZTResearchProgram) -> bool {
            self.calls.push("set_effect_discount".to_string());
            self.set_effect_discount_result
        }

        fn add_completed_research(&mut self, _program_ptr: *mut ZTResearchProgram) {
            self.calls.push("add_completed_research".to_string());
        }

        fn remove_completed_research(&mut self, _program_ptr: *mut ZTResearchProgram) {
            self.calls.push("remove_completed_research".to_string());
        }
    }

    fn program_with(effect_kind_raw: i32) -> ZTResearchProgram {
        ZTResearchProgram {
            config_file: BFConfigFile::default(),
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            desc_id: 0,
            icon_ptr: 0,
            entity_icon_ptr: 0,
            id: 0,
            target_cost: 0.0,
            current_progress: 0.0,
            priority: 0,
            target_id: 42,
            effect_kind_raw,
            effect_param_0: 1,
            effect_param_1: 2,
            effect_param_2: 3,
            help_id: 0,
        }
    }

    #[test]
    fn on_completion_unlock_entity_always_succeeds_and_notifies() {
        let mut program = program_with(ZTResearchEffectKind::UnlockEntity as i32);
        let mut mock = MockEffects::default();
        assert!(dispatch_on_completion(&mut mock, &mut program));
        assert_eq!(mock.calls, vec!["set_avail(42, true)", "add_completed_research"]);
    }

    #[test]
    fn on_completion_building_upgrade_notifies_only_on_success() {
        for success in [true, false] {
            let mut program = program_with(ZTResearchEffectKind::BuildingUpgrade as i32);
            let mut mock = MockEffects { set_building_upgrade_result: success, ..Default::default() };
            assert_eq!(dispatch_on_completion(&mut mock, &mut program), success);
            let mut expected = vec!["set_building_upgrade(install=true)".to_string()];
            if success {
                expected.push("add_completed_research".to_string());
            }
            assert_eq!(mock.calls, expected);
        }
    }

    #[test]
    fn on_completion_dispatches_each_remaining_valid_effect_kind() {
        let cases = [
            (ZTResearchEffectKind::EntityCharacteristic, "set_entity_characteristic"),
            (ZTResearchEffectKind::GenusCharacteristic, "set_genus_characteristic"),
            (ZTResearchEffectKind::FamilyCharacteristic, "set_family_characteristic"),
            (ZTResearchEffectKind::FoodCharacteristic, "set_food_characteristic"),
            (ZTResearchEffectKind::TrickAvailable, "set_trick_available"),
            (ZTResearchEffectKind::EffectDiscount, "set_effect_discount"),
        ];
        for (kind, expected_call) in cases {
            for success in [true, false] {
                let mut program = program_with(kind as i32);
                let mut mock = MockEffects::default();
                match kind {
                    ZTResearchEffectKind::EntityCharacteristic => mock.set_entity_characteristic_result = success,
                    ZTResearchEffectKind::GenusCharacteristic => mock.set_genus_characteristic_result = success,
                    ZTResearchEffectKind::FamilyCharacteristic => mock.set_family_characteristic_result = success,
                    ZTResearchEffectKind::FoodCharacteristic => mock.set_food_characteristic_result = success,
                    ZTResearchEffectKind::TrickAvailable => mock.set_trick_available_result = success,
                    ZTResearchEffectKind::EffectDiscount => mock.set_effect_discount_result = success,
                    _ => unreachable!(),
                }
                assert_eq!(dispatch_on_completion(&mut mock, &mut program), success);
                let mut expected = vec![expected_call.to_string()];
                if success {
                    expected.push("add_completed_research".to_string());
                }
                assert_eq!(mock.calls, expected, "kind={kind:?}, success={success}");
            }
        }
    }

    #[test]
    fn on_completion_invalid_effect_kind_is_a_no_op() {
        for kind in [-1, 8, i32::MIN, i32::MAX] {
            let mut program = program_with(kind);
            let mut mock = MockEffects::default();
            assert!(!dispatch_on_completion(&mut mock, &mut program));
            assert!(mock.calls.is_empty());
        }
    }

    #[test]
    fn reset_always_zeroes_current_progress_regardless_of_effect_kind() {
        for kind in [-1, 0, 1, 2, 6, 7, 8, i32::MIN, i32::MAX] {
            let mut program = program_with(kind);
            program.current_progress = 123.0;
            let mut mock = MockEffects::default();
            dispatch_reset(&mut mock, &mut program);
            assert_eq!(program.current_progress, 0.0, "kind={kind}");
        }
    }

    #[test]
    fn reset_unlock_entity_always_succeeds_and_notifies() {
        let mut program = program_with(ZTResearchEffectKind::UnlockEntity as i32);
        let mut mock = MockEffects::default();
        assert!(dispatch_reset(&mut mock, &mut program));
        assert_eq!(mock.calls, vec!["set_avail(42, false)", "remove_completed_research"]);
    }

    #[test]
    fn reset_building_upgrade_notifies_only_on_success() {
        for success in [true, false] {
            let mut program = program_with(ZTResearchEffectKind::BuildingUpgrade as i32);
            let mut mock = MockEffects { set_building_upgrade_result: success, ..Default::default() };
            assert_eq!(dispatch_reset(&mut mock, &mut program), success);
            let mut expected = vec!["set_building_upgrade(install=false)".to_string()];
            if success {
                expected.push("remove_completed_research".to_string());
            }
            assert_eq!(mock.calls, expected);
        }
    }

    /// Confirmed via `ZTResearchProgram_reset.c`/`.asm`: unlike `on_completion`, `reset` does NOT call
    /// `setEntityCharacteristic`/`setGenusCharacteristic`/`setFamilyCharacteristic`/
    /// `setFoodCharacteristic`/`setTrickAvailable` at all for these kinds - it just unconditionally
    /// removes the program from the completed-research list, same as the unset (`-1`) case.
    #[test]
    fn reset_unset_and_non_reversible_kinds_notify_without_calling_the_underlying_effect() {
        for kind in [-1, 2, 3, 4, 5, 6, ZTResearchEffectKind::EffectDiscount as i32] {
            let mut program = program_with(kind);
            let mut mock = MockEffects::default();
            assert!(dispatch_reset(&mut mock, &mut program), "kind={kind}");
            assert_eq!(mock.calls, vec!["remove_completed_research"], "kind={kind}");
        }
    }

    /// Confirmed live (`ZTRESEARCHPROGRAM_ON_COMPLETION_RESET`'s comparison against real vanilla
    /// `reset()`): the C decompile only shows explicit case labels through `6`, making `7`
    /// (`EffectDiscount`) look like it falls into the "invalid" default - but the compiled jump
    /// table's 9th slot actually aliases to the same block `-1`/`2..=6` use (see
    /// `reset_unset_and_non_reversible_kinds_notify_without_calling_the_underlying_effect` above).
    /// Only genuinely out-of-range values are a no-op.
    #[test]
    fn reset_out_of_range_is_a_no_op() {
        for kind in [8, i32::MIN, i32::MAX] {
            let mut program = program_with(kind);
            let mut mock = MockEffects::default();
            assert!(!dispatch_reset(&mut mock, &mut program), "kind={kind}");
            assert!(mock.calls.is_empty(), "kind={kind}");
        }
    }
}

impl fmt::Display for ZTResearchProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTResearchProgram {{")?;
        writeln!(f, "  id: {},", self.id)?;
        writeln!(f, "  name: {:?},", self.name())?;
        writeln!(f, "  desc: {:?},", self.desc())?;
        writeln!(f, "  icon: {:?},", self.icon())?;
        writeln!(f, "  entity_icon: {:?},", self.entity_icon())?;
        writeln!(f, "  building_type_name: {:?},", self.building_type_name())?;
        writeln!(f, "  target_cost: {},", self.target_cost)?;
        writeln!(f, "  current_progress: {},", self.current_progress)?;
        writeln!(f, "  priority: {},", self.priority)?;
        writeln!(f, "  target_id: {},", self.target_id)?;
        writeln!(f, "  effect_kind: {:?} ({}),", self.effect_kind(), self.effect_kind_raw)?;
        writeln!(f, "  effect_params: {:?},", self.effect_params())?;
        writeln!(f, "  tooltip: {:?},", self.tooltip())?;
        write!(f, "}}")
    }
}

/// A group of related research programs, e.g. "Elephants". Owned (by pointer) by a
/// `ZTResearchBranch`'s `category_array`. Loaded by `ZTResearchCategory::loadCategory` (which, like
/// `ZTResearchProgram`, treats `this` directly as a `BFConfigFile*`) from a `.cfg` block of the form
/// `name`/`desc`/`icon`/`helpid`/`expansion`, plus a `program=` list (-> `program_array`).
/// `ZTResearchBranch` shares an almost identical layout (see its own doc comment).
#[derive(Debug)]
#[repr(C)]
pub struct ZTResearchCategory {
    config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`)
    id: i32,                     // 0x0c - confirmed by `ZTResearchCategory::loadCategory` (loaded from the `.cfg` `name` key); matched by `ZTResearchMgr::get_category`; doubles as the display-name string id (`get_string(id)` returns the category's name - this is what the in-game UI mislabels "Program")
    cached_name: ZTBufferString, // 0x10 - confirmed by `loadCategory`: built from `id`/`name` via `BFApp::buildString` right after it's loaded
    cached_desc: ZTBufferString, // 0x1c - confirmed by `loadCategory`: built from the `.cfg` `desc` field the same way (the raw `desc` string id is only ever a local variable in `loadCategory` - unlike `ZTResearchProgram`, it's never stored on the object itself)
    icon_ptr: u32,               // 0x28 - confirmed by `loadCategory`: the `.cfg` `icon` field, a raw C string pointer (only valid when non-null) - not a numeric id as previously guessed
    help_id: i32,                // 0x2c - confirmed: matches `.cfg` `helpid=` exactly (tested against `helpid=24213`) and matches `loadCategory`'s `getInt(..., "helpid", ...)` call storing here
    expansion_id: i32,           // 0x30 - confirmed by `loadCategory` (`.cfg` `expansion` key); +1 is passed to `ZTUI::expansionselect::isExpansionDisabled` when picking a program
    enabled: u8,                 // 0x34 - unlocked/available flag; gates `ZTResearchBranch::pick_random_program` and is persisted by `ZTResearchMgr::save`; not set by `loadCategory` itself
    pad2: [u8; 0x38 - 0x35],     // 0x35 - alignment padding
    program_array: ZTArray<ZTResearchProgram>, // 0x38
}

impl ZTResearchCategory {
    pub fn id(&self) -> i32 {
        self.id
    }

    /// See `ZTResearchProgram::is_config_loaded`.
    pub fn is_config_loaded(&self) -> bool {
        self.config_file.is_loaded()
    }

    /// Display name, resolved through the same string table `get_string()` uses. Confirmed against
    /// the live game.
    pub fn name(&self) -> Option<String> {
        load_string_by_id(self.id as u32)
    }

    /// The load-time cached copy of `name()`'s text (see `loadCategory`), rather than a fresh
    /// `get_string()` lookup.
    pub fn cached_name(&self) -> String {
        self.cached_name.copy_to_string()
    }

    /// The `.cfg` `desc` field's cached text, if any (empty when `desc=` was blank).
    pub fn desc(&self) -> String {
        self.cached_desc.copy_to_string()
    }

    /// The `.cfg` `icon` field, if set.
    pub fn icon(&self) -> Option<String> {
        (self.icon_ptr != 0).then(|| unsafe { CStr::from_ptr(self.icon_ptr as *const c_char) }.to_string_lossy().into_owned())
    }

    /// The raw `.cfg` `helpid` field. Prefer this over `tooltip()` for on-screen UI tooltips - see
    /// `ZTResearchProgram::help_id`'s doc comment for why.
    pub fn help_id(&self) -> i32 {
        self.help_id
    }

    /// The category's tooltip/help text, resolved from `help_id` through `get_string()`. Confirmed
    /// against the live game (`help_id` matches a known `.cfg` `helpid=` value exactly). Fine for
    /// debug/console output; UI tooltips should use `help_id()` instead (see `ZTResearchProgram::help_id`).
    pub fn tooltip(&self) -> Option<String> {
        load_string_by_id(self.help_id as u32)
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled != 0
    }

    /// Sets the unlocked/available flag - toggled by the in-game research category checklist UI
    /// (a multi-select list, not a single "current category" picker), and read by
    /// `ZTResearchBranch::pick_random_program`/persisted by `ZTResearchMgr::save` like the rest of
    /// the category's state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled as u8;
    }

    pub fn expansion_id(&self) -> i32 {
        self.expansion_id
    }

    pub fn program_count(&self) -> usize {
        self.program_array.len()
    }

    pub fn program(&self, index: usize) -> &'static ZTResearchProgram {
        unsafe { ref_from_memory(self.program_array.get_ptr(index)) }
    }

    pub fn program_mut(&self, index: usize) -> &'static mut ZTResearchProgram {
        unsafe { mut_from_memory(self.program_array.get_ptr(index)) }
    }

    pub fn programs(&self) -> impl Iterator<Item = &'static ZTResearchProgram> + '_ {
        (0..self.program_count()).map(move |i| self.program(i))
    }

    pub fn programs_mut(&self) -> impl Iterator<Item = &'static mut ZTResearchProgram> + '_ {
        (0..self.program_count()).map(move |i| self.program_mut(i))
    }

    /// Calls the vanilla `ZTResearchCategory::loadCategory`; used while reading a mod/save's
    /// research definitions. `reader` is whatever stream/buffer pointer the original expects.
    pub fn load_category(&mut self, reader: *const i32) -> u32 {
        unsafe { ztresearchcategory::LOAD_CATEGORY.original()((self as *mut Self) as *const u32, reader) }
    }

    pub fn clear(&mut self) {
        unsafe { ztresearchcategory::CLEAR_CATEGORY.original()((self as *mut Self) as *const u32) }
    }
}

impl fmt::Display for ZTResearchCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTResearchCategory {{")?;
        writeln!(f, "  id: {},", self.id)?;
        writeln!(f, "  name: {:?},", self.name())?;
        writeln!(f, "  desc: {:?},", self.desc())?;
        writeln!(f, "  icon: {:?},", self.icon())?;
        writeln!(f, "  tooltip: {:?},", self.tooltip())?;
        writeln!(f, "  enabled: {},", self.is_enabled())?;
        writeln!(f, "  expansion_id: {},", self.expansion_id)?;
        writeln!(f, "  program_count: {},", self.program_count())?;
        write!(f, "}}")
    }
}

/// A top-level research branch, e.g. "Animal Care" or "Guest Amenities". Owned (by pointer) by
/// `ZTResearchMgr`'s `branch_array`. Loaded by `ZTResearchBranch::loadBranch` (which, like
/// `ZTResearchProgram`, treats `this` directly as a `BFConfigFile*`) from a `.cfg` block of the form
/// `name`/`desc`/`icon`/`noprogicon` (no `helpid`, no `expansion` - those are category-only), plus a
/// `category=` list (-> `category_array`) and a `funding=` list (-> the funding table, each entry
/// naming a sub-block with its own `name`/`cost`/`work` keys).
#[derive(Debug)]
#[repr(C)]
pub struct ZTResearchBranch {
    config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`)
    id: i32,                     // 0x0c - confirmed by `loadBranch` (`.cfg` `name` key); matched by `ZTResearchMgr::get_branch`; doubles as the display-name string id (`get_string(id)` returns the branch's name, e.g. "Animal Care")
    cached_name: ZTBufferString, // 0x10 - confirmed by `loadBranch`: built from `id`/`name` via `BFApp::buildString` right after it's loaded
    cached_desc: ZTBufferString, // 0x1c - confirmed by `loadBranch`: built from the `.cfg` `desc` field the same way (the raw `desc` string id is never stored on the object itself, same as `ZTResearchCategory`)
    icon_ptr: u32,               // 0x28 - confirmed by `loadBranch`: the `.cfg` `icon` field, a raw C string pointer
    noprogicon_ptr: u32,         // 0x2c - confirmed by `loadBranch`: the `.cfg` `noprogicon` field (the icon shown when no program is being researched), a raw C string pointer
    current_category_ptr: u32,                   // 0x30 - selected category; cleared then reassigned by `pick_random_program`
    current_program_ptr: u32,                    // 0x34 - selected program; read directly by `days_remaining_on_program`/`pct_remaining_on_program`
    category_array: ZTArray<ZTResearchCategory>, // 0x38 - confirmed by `loadBranch` (populated from the `.cfg` `category=` list) and `pick_random_program`
    current_funding_level: i32,                  // 0x44 - index into the funding table, clamped by `increase_funding`/`decrease_funding`; reset to 0 by `loadBranch`
    funding_table_start: u32,                    // 0x48 - confirmed by `loadBranch`: inline `ZTResearchFundingLevel` table (stride 0xc, populated from the `.cfg` `funding=` list), *not* a `ZTArray` of pointers
    funding_table_end: u32,                      // 0x4c - confirmed by `loadBranch`
    funding_table_capacity: u32,                  // 0x50 - confirmed by `loadBranch` (checked to decide whether the table needs to grow); unused by us
}

impl ZTResearchBranch {
    pub fn id(&self) -> i32 {
        self.id
    }

    /// See `ZTResearchProgram::is_config_loaded`.
    pub fn is_config_loaded(&self) -> bool {
        self.config_file.is_loaded()
    }

    /// Display name, resolved through the same string table `get_string()` uses. Confirmed against
    /// the live game.
    pub fn name(&self) -> Option<String> {
        load_string_by_id(self.id as u32)
    }

    /// The load-time cached copy of `name()`'s text (see `loadBranch`), rather than a fresh
    /// `get_string()` lookup.
    pub fn cached_name(&self) -> String {
        self.cached_name.copy_to_string()
    }

    /// The `.cfg` `desc` field's cached text, if any (empty when `desc=` was blank).
    pub fn desc(&self) -> String {
        self.cached_desc.copy_to_string()
    }

    /// The `.cfg` `icon` field, if set.
    pub fn icon(&self) -> Option<String> {
        (self.icon_ptr != 0).then(|| unsafe { CStr::from_ptr(self.icon_ptr as *const c_char) }.to_string_lossy().into_owned())
    }

    /// The `.cfg` `noprogicon` field (the icon shown when no program is being researched), if set.
    pub fn noprogicon(&self) -> Option<String> {
        (self.noprogicon_ptr != 0)
            .then(|| unsafe { CStr::from_ptr(self.noprogicon_ptr as *const c_char) }.to_string_lossy().into_owned())
    }

    pub fn category_count(&self) -> usize {
        self.category_array.len()
    }

    pub fn category(&self, index: usize) -> &'static ZTResearchCategory {
        unsafe { ref_from_memory(self.category_array.get_ptr(index)) }
    }

    pub fn category_mut(&self, index: usize) -> &'static mut ZTResearchCategory {
        unsafe { mut_from_memory(self.category_array.get_ptr(index)) }
    }

    pub fn categories(&self) -> impl Iterator<Item = &'static ZTResearchCategory> + '_ {
        (0..self.category_count()).map(move |i| self.category(i))
    }

    pub fn categories_mut(&self) -> impl Iterator<Item = &'static mut ZTResearchCategory> + '_ {
        (0..self.category_count()).map(move |i| self.category_mut(i))
    }

    pub fn current_category(&self) -> Option<&'static ZTResearchCategory> {
        (self.current_category_ptr != 0).then(|| unsafe { ref_from_memory(self.current_category_ptr) })
    }

    pub fn current_program(&self) -> Option<&'static ZTResearchProgram> {
        (self.current_program_ptr != 0).then(|| unsafe { ref_from_memory(self.current_program_ptr) })
    }

    /// Mutable counterpart to `current_program`, used by `update` to accumulate progress on the
    /// selected program.
    fn current_program_mut(&self) -> Option<&'static mut ZTResearchProgram> {
        (self.current_program_ptr != 0).then(|| unsafe { mut_from_memory(self.current_program_ptr) })
    }

    pub fn current_funding_level(&self) -> i32 {
        self.current_funding_level
    }

    fn funding_level_count(&self) -> usize {
        ((self.funding_table_end - self.funding_table_start) as usize) / size_of::<ZTResearchFundingLevel>()
    }

    fn funding_level(&self, index: usize) -> ZTResearchFundingLevel {
        get_from_memory(self.funding_table_start + (index * size_of::<ZTResearchFundingLevel>()) as u32)
    }

    /// Every entry in the funding-level table, for inspecting `unknown_0`/`unknown_8` (`rate` at
    /// offset `0x4` is the only confirmed field so far).
    pub fn funding_levels(&self) -> Vec<ZTResearchFundingLevel> {
        (0..self.funding_level_count()).map(|i| self.funding_level(i)).collect()
    }

    pub fn current_funding_rate(&self) -> Option<f32> {
        let index = self.current_funding_level as usize;
        (index < self.funding_level_count()).then(|| self.funding_level(index).rate())
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchBranch::increaseFunding`. Confirmed against
    /// `ZTResearchBranch_increaseFunding.asm`: the `current_funding_level + 1 < count` guard is an
    /// **unsigned** comparison (`CMP`/`JNC`, not a signed `JGE`), so a negative `current_funding_level`
    /// whose `+1` is still negative (i.e. `<= -2`) wraps to a huge `u32` and fails the guard, falling
    /// through to the `count - 1` clamp below - the same clamp an already-at-the-top level hits.
    /// `current_funding_level == -1` doesn't diverge (`-1 + 1 == 0`, non-negative either way). Found
    /// live via `ZTRESEARCHBRANCH_FUNDING`: a naive signed `<` here (this function's original form)
    /// disagreed with real vanilla for `current_funding_level <= -2`.
    pub fn increase_funding(&mut self) {
        let count = self.funding_level_count() as i32;
        if count == 0 {
            self.current_funding_level = 0;
        } else if (self.current_funding_level.wrapping_add(1) as u32) < count as u32 {
            self.current_funding_level = self.current_funding_level.wrapping_add(1);
        } else {
            self.current_funding_level = count - 1;
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchBranch::decreaseFunding`.
    pub fn decrease_funding(&mut self) {
        if self.funding_level_count() > 0 && self.current_funding_level != 0 {
            self.current_funding_level -= 1;
        } else {
            self.current_funding_level = 0;
        }
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchBranch::pctRemainingOnProgram`. Returns `None`
    /// when there is no selected program or the active funding level isn't contributing (rate <= 0),
    /// mirroring the vanilla function's `-1` sentinel return.
    ///
    /// The float-to-int conversion deliberately does *not* round to nearest, despite Ghidra's
    /// decompile naming its helper `ROUND()`, and does *not* use Rust's plain `as i32` saturating
    /// cast either - both confirmed live (`ZTRESEARCHBRANCH_PCT_DAYS_REMAINING` in
    /// `reimplementation_tests`):
    ///
    /// - It **truncates toward zero**, not round-to-nearest: confirmed live for
    ///   `target_cost=-8576.077, current_progress=-4133.11` (true value ≈`51.807`), where vanilla
    ///   returned `51`, not the `52` `.round()` (or any nearest-rounding) would give. This matches
    ///   `private/resources/decompiles/ZTResearchBranch_pctRemainingOnProgram.asm` exactly: right
    ///   before `FISTP`, it does `FSTCW`/`OR AH,0xc`/`FLDCW` to force the x87 rounding-control field
    ///   to `11` (round-toward-zero) for just that one instruction, then restores the original
    ///   control word - the classic MSVC codegen for a plain C `(int)x` cast (whose standard-mandated
    ///   semantics *are* truncation toward zero), not an actual `round()` call. `f32::trunc()` below
    ///   reproduces this directly.
    /// - For a value that doesn't fit a 64-bit integer (NaN, ±Infinity, or a magnitude beyond
    ///   `i64`'s range), x87's masked-invalid-operation behavior makes `FISTP` store the "integer
    ///   indefinite" pattern `0x8000_0000_0000_0000` into its 64-bit destination (`FISTP` only has a
    ///   64-bit integer store form - x87 has no 32-bit one - so the real return value is the low
    ///   dword of that 64-bit store, the rest discarded). That pattern's low dword is `0`, confirmed
    ///   live for `target_cost=0.0` - *not*
    ///   `i32::MIN`/`i32::MAX`, which is what `f32 as i32`'s saturating cast would (wrongly) produce
    ///   for -Infinity/+Infinity.
    pub fn pct_remaining_on_program(&self) -> Option<i32> {
        let program = self.current_program()?;
        let rate = self.current_funding_rate()?;
        if rate <= 0.0 {
            return None;
        }
        let raw = (((program.target_cost - program.current_progress) * 100.0) / program.target_cost).trunc();
        Some(if raw.is_finite() { raw as i64 as i32 } else { 0 })
    }

    /// Reimplementation of `OOAnalyzer::ZTResearchBranch::daysRemainingOnProgram`. Same guards as
    /// `pct_remaining_on_program`; the scale constant it multiplies by (`DAT_00635030`) is confirmed
    /// to be `30.0`.
    pub fn days_remaining_on_program(&self) -> Option<f32> {
        let program = self.current_program()?;
        let rate = self.current_funding_rate()?;
        if rate <= 0.0 {
            return None;
        }
        Some((program.target_cost - program.current_progress) * 30.0 / rate)
    }

    /// Native reimplementation of `ZTResearchBranch::update`'s eligibility/progress/cash
    /// state-transition core, per `resources/decompiles/ZTResearchBranch_update.c`/`.asm` (the `.asm`
    /// was needed to get the real byte offsets right - see `ZTResearchMgr::always_check_expansion`'s
    /// doc comment for one place the `.c` decompile's own pointer-arithmetic scaling was actively
    /// misleading). Applies `days` in-game days of progress/cost to the currently selected program (see
    /// `predict_branch_progress`), spending cash via `ZTGameMgr::spend_research`/`subtract_cash` (in
    /// that order, matching vanilla) when affordable, then, on completion, dispatches `on_completion()`
    /// (native, from Phase A) and picks the next program via `pick_random_program()` (still a call into
    /// the original implementation - see its own doc comment on why).
    ///
    /// The eligibility gate mirrors vanilla exactly: with `ZTResearchMgr::always_check_expansion()` or
    /// `getAnyExpansionsDisabled()` true, a null `current_category` short-circuits straight to
    /// `pick_random_program()` (matching vanilla's `iVar1 == 0` guard before it would otherwise
    /// dereference the category to call `isExpansionDisabled`); with neither true, `isExpansionDisabled`
    /// is never called at all (vanilla skips straight past it), matching this method's `eligible = true`
    /// fallback below.
    ///
    /// UI feedback (icon animation, the "research complete"/"no more research" confirm dialog) is
    /// called via address like every other UI surface in this file that isn't independently
    /// reimplemented - **except** the confirm dialog's own caption text, which vanilla sets via an
    /// indirect vtable call (`BFApp::buildString` into a stack buffer, then a virtual dispatch through
    /// the label element's own vtable at offset `0xc4` - past `UIElement`'s own confirmed 49-entry
    /// vtable entirely, i.e. some derived class's real override that isn't independently reverse
    /// engineered here) this deliberately does not replicate: the dialog still appears (gated on the
    /// label element existing, matching vanilla's own null check), just without vanilla's
    /// dynamically-substituted caption text. This is a cosmetic gap only - every gameplay-affecting
    /// side effect (cash, progress, completion effects, program selection) still happens exactly as
    /// vanilla does.
    pub fn update(&mut self, days: u32) {
        let should_check_expansion =
            global_always_check_expansion() || unsafe { ztui_expansionselect::GET_ANY_EXPANSIONS_DISABLED.original()() != 0 };

        let category = self.current_category();
        if should_check_expansion && category.is_none() {
            self.pick_random_program();
            return;
        }
        let eligible = if should_check_expansion {
            unsafe { ztui_expansionselect::IS_EXPANSION_DISABLED.original()(category.expect("checked above").expansion_id + 1) == 0 }
        } else {
            true
        };

        let category_enabled = category.map(ZTResearchCategory::is_enabled).unwrap_or(false);
        if category.is_none() || self.current_program().is_none() || !eligible || !category_enabled {
            self.pick_random_program();
            return;
        }

        let level = self.funding_level(self.current_funding_level as usize);
        let cash = unsafe { &*global_ztgamemgr_ptr() }.cash();
        let (cash_delta, progress_delta) = predict_branch_progress(days, level.cost(), level.rate(), cash);
        if cash_delta != 0.0 || progress_delta != 0.0 {
            unsafe { &mut *global_ztgamemgr_ptr() }.spend_research(cash_delta);
            unsafe { &mut *global_ztgamemgr_ptr() }.subtract_cash(cash_delta);
            self.current_program_mut().expect("checked above").current_progress += progress_delta;
        }

        let program = self.current_program().expect("checked above");
        if program.current_progress < program.target_cost {
            return;
        }
        self.current_program_mut().expect("checked above").on_completion();

        let icon = get_research_dialog_element(RESEARCH_DIALOG_ICON_ELEMENT_ID);
        let label_present = get_research_dialog_element(RESEARCH_DIALOG_LABEL_ELEMENT_ID).is_some();
        show_research_dialog(icon, label_present);

        self.pick_random_program();
        if self.current_program_ptr != 0 {
            return;
        }
        show_research_dialog(icon, label_present);
    }

    /// Calls the vanilla `ZTResearchBranch::pickRandomProgram`, which selects the branch's next
    /// active program (preferring one already in progress) using the game's own RNG stream.
    /// Reimplementing this natively risks desyncing that RNG stream from the rest of the game, so
    /// it's left as a call into the original implementation.
    pub fn pick_random_program(&mut self) {
        unsafe { ztresearchbranch::PICK_RANDOM_PROGRAM.original()((self as *mut Self) as *const u32) }
    }

    /// Calls the vanilla `ZTResearchBranch::loadBranch`, which reads a `.cfg` file (the same shape
    /// as this struct's own doc comment) and populates this branch's fields/`category_array`/funding
    /// table from it - the branch-level counterpart to `ZTResearchCategory::load_category`/
    /// `ZTResearchProgram::load_program`. `path` is a null-terminated path string.
    pub fn load_branch(&mut self, path: *const i8) -> u32 {
        unsafe { ztresearchbranch::LOAD_BRANCH.original()((self as *mut Self) as *const u32, path) }
    }

    /// Calls the vanilla `ZTResearchBranch::clearBranch`: resets `id` to `-1`, empties the cached
    /// name/desc buffers, zeroes `icon`/`noprogicon`/`current_category`/`current_program`, destroys
    /// and frees every category, and resets `category_array`/the funding table to empty (keeping
    /// their allocated capacity).
    pub fn clear_branch(&mut self) {
        unsafe { ztresearchbranch::CLEAR_BRANCH.original()((self as *mut Self) as *const u32) }
    }

    /// The vanilla "$400 (Min)"-style formatted text for the *currently selected* funding level (per
    /// `resources/decompiles/ZTResearchBranch_getFundingText.c`, this always uses
    /// `current_funding_level` - there's no way to ask for an arbitrary level's text). Out of range
    /// (checked as `uint`, so a negative `current_funding_level` also lands here - same idiom as
    /// `current_funding_rate`) returns an empty string, matching vanilla's empty heap-allocated
    /// `std::string` in that branch.
    pub fn funding_text(&self) -> String {
        let index = self.current_funding_level as usize;
        if index >= self.funding_level_count() {
            return String::new();
        }
        let level = self.funding_level(index);
        // Confirmed live: despite Ghidra's `ROUND()` label, the underlying FISTP-with-overridden-
        // control-word idiom truncates toward zero here (e.g. `-29506.857 * (1/30) = -983.56..` prints
        // as `-$983`, not `-$984`) - a plain `as i32` cast already truncates toward zero in Rust, so no
        // `.round()` is needed (or correct) here.
        let money_value = (level.cost() * MONTHLY_TO_DAILY_COST_SCALE) as i32;
        let money_text = get_money_text(money_value);
        match level.name() {
            Some(template) => template.replacen("%s", &money_text, 1),
            None => money_text,
        }
    }
}

/// Pure, no-live-game-dependency tests for `ZTResearchBranch::pct_remaining_on_program`/
/// `days_remaining_on_program` - both `&self`-only, reading nothing but `this`'s own
/// `current_program_ptr`/`current_funding_level`/funding table, so real `ZTResearchProgram`/
/// `ZTResearchBranch` instances can be built directly on Rust's own allocator without any of
/// `reimplementation_tests::live_support`'s machinery (which is feature-gated behind
/// `reimplementation-tests`/`proptest`, unavailable to a plain `cargo test` run). Also covered live
/// in `reimplementation_tests` (`ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`) against the real
/// `ztresearchbranch::PCT_REMAINING_ON_PROGRAM`/`DAYS_REMAINING_ON_PROGRAM` - originally these
/// `FunctionDef`s' auto-detected signatures were wrong (`-> i64` and no return type at all), which
/// is why that live test didn't exist at first; a Ghidra regen has since fixed both to their
/// confirmed-correct `-> i32`/`-> f32`.
#[cfg(test)]
mod pct_days_remaining_tests {
    use super::*;

    fn build_test_program(target_cost: f32, current_progress: f32) -> *mut ZTResearchProgram {
        Box::into_raw(Box::new(ZTResearchProgram {
            config_file: BFConfigFile::default(),
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            desc_id: 0,
            icon_ptr: 0,
            entity_icon_ptr: 0,
            id: 0,
            target_cost,
            current_progress,
            priority: 0,
            target_id: -1,
            effect_kind_raw: -1,
            effect_param_0: 0,
            effect_param_1: -1,
            effect_param_2: 0,
            help_id: 0,
        }))
    }

    fn destroy_test_program(ptr: *mut ZTResearchProgram) {
        drop(unsafe { Box::from_raw(ptr) });
    }

    /// Leaks `rates` into a fresh funding-table buffer (`name_id`/`cost` fixed to `0` - neither method
    /// under test reads them, only `rate`), returning its `(start, end, capacity_end)` raw parts.
    fn funding_table_from_rates(rates: &[f32]) -> (u32, u32, u32) {
        if rates.is_empty() {
            return (0, 0, 0);
        }
        let mut table: Vec<ZTResearchFundingLevel> = rates.iter().map(|&rate| ZTResearchFundingLevel { name_id: 0, rate, cost: 0.0 }).collect();
        let stride = size_of::<ZTResearchFundingLevel>() as u32;
        let ptr = table.as_mut_ptr() as u32;
        let len = table.len() as u32;
        std::mem::forget(table);
        (ptr, ptr + len * stride, ptr + len * stride)
    }

    fn free_funding_table(start: u32, end: u32) {
        if start == 0 {
            return;
        }
        let stride = size_of::<ZTResearchFundingLevel>() as u32;
        let len = ((end - start) / stride) as usize;
        drop(unsafe { Vec::<ZTResearchFundingLevel>::from_raw_parts(start as *mut ZTResearchFundingLevel, len, len) });
    }

    fn build_test_branch(current_program_ptr: u32, current_funding_level: i32, rates: &[f32]) -> ZTResearchBranch {
        let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_rates(rates);
        ZTResearchBranch {
            config_file: BFConfigFile::default(),
            id: 0,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            noprogicon_ptr: 0,
            current_category_ptr: 0,
            current_program_ptr,
            category_array: ZTArray::from_raw_parts(0, 0, 0),
            current_funding_level,
            funding_table_start,
            funding_table_end,
            funding_table_capacity,
        }
    }

    fn destroy_test_branch(branch: &ZTResearchBranch) {
        free_funding_table(branch.funding_table_start, branch.funding_table_end);
    }

    #[test]
    fn no_current_program_returns_none() {
        let branch = build_test_branch(0, 0, &[30.0]);
        assert_eq!(branch.pct_remaining_on_program(), None);
        assert_eq!(branch.days_remaining_on_program(), None);
        destroy_test_branch(&branch);
    }

    #[test]
    fn out_of_range_funding_level_returns_none() {
        let program = build_test_program(100.0, 50.0);
        // One level in the table, but `current_funding_level` points past the end.
        let branch = build_test_branch(program as u32, 1, &[30.0]);
        assert_eq!(branch.pct_remaining_on_program(), None);
        assert_eq!(branch.days_remaining_on_program(), None);
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn negative_funding_level_returns_none() {
        let program = build_test_program(100.0, 50.0);
        let branch = build_test_branch(program as u32, -1, &[30.0]);
        assert_eq!(branch.pct_remaining_on_program(), None);
        assert_eq!(branch.days_remaining_on_program(), None);
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn zero_rate_returns_none() {
        let program = build_test_program(100.0, 50.0);
        let branch = build_test_branch(program as u32, 0, &[0.0]);
        assert_eq!(branch.pct_remaining_on_program(), None);
        assert_eq!(branch.days_remaining_on_program(), None);
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn negative_rate_returns_none() {
        let program = build_test_program(100.0, 50.0);
        let branch = build_test_branch(program as u32, 0, &[-5.0]);
        assert_eq!(branch.pct_remaining_on_program(), None);
        assert_eq!(branch.days_remaining_on_program(), None);
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn zero_target_cost_with_zero_progress_is_a_zero_over_zero_nan_that_becomes_zero() {
        let program = build_test_program(0.0, 0.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        // (0.0 - 0.0) * 100.0 / 0.0 == NaN; NaN.round() is still NaN, not `is_finite()`, so
        // `pct_remaining_on_program` returns 0 - matching vanilla's x87 `FISTP` "integer
        // indefinite" behavior for a value that can't convert to an integer (confirmed live via
        // `ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`; see that method's own doc comment).
        assert_eq!(branch.pct_remaining_on_program(), Some(0));
        // (0.0 - 0.0) * 30.0 / rate == 0.0 exactly (the numerator is a real zero, not a NaN), so
        // this case doesn't exercise that conversion for `days_remaining_on_program` at all (it
        // returns `f32`, not `i32`, so there's no int conversion to begin with).
        assert_eq!(branch.days_remaining_on_program(), Some(0.0));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn zero_target_cost_with_positive_progress_is_negative_infinity_that_becomes_zero() {
        let program = build_test_program(0.0, 5.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        // (0.0 - 5.0) * 100.0 / 0.0 == -inf, not `is_finite()`, so `pct_remaining_on_program`
        // returns 0. Confirmed live (`ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`) against real vanilla,
        // which disagreed with this crate's older `f32 as i32` saturating-cast implementation here
        // (that cast saturates -inf to `i32::MIN`, but vanilla's x87 `FISTP` "integer indefinite"
        // value's low dword - the only part any real caller reads - is 0, not `i32::MIN`).
        assert_eq!(branch.pct_remaining_on_program(), Some(0));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn zero_target_cost_with_negative_progress_is_positive_infinity_that_becomes_zero() {
        let program = build_test_program(0.0, -5.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        // (0.0 - (-5.0)) * 100.0 / 0.0 == +inf, not `is_finite()`, so `pct_remaining_on_program`
        // returns 0 - the same x87 `FISTP` "integer indefinite" behavior as the positive-progress
        // case above, confirmed live the same way (vanilla does not saturate to `i32::MAX` here).
        assert_eq!(branch.pct_remaining_on_program(), Some(0));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn current_progress_greater_than_target_cost_returns_negative_values() {
        let program = build_test_program(100.0, 150.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        assert_eq!(branch.pct_remaining_on_program(), Some(-50));
        assert_eq!(branch.days_remaining_on_program(), Some(-50.0));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn pct_truncates_toward_zero_at_the_boundary() {
        // (200.0 - 1.0) * 100.0 / 200.0 == 99.5, which `f32::trunc` truncates toward zero to 99.0 -
        // vanilla does *not* round to nearest here, confirmed live (see `pct_remaining_on_program`'s
        // own doc comment for the live case that caught this).
        let program = build_test_program(200.0, 1.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        assert_eq!(branch.pct_remaining_on_program(), Some(99));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn pct_truncates_toward_zero_for_negative_values() {
        // (-8576.077 - -4133.11) * 100.0 / -8576.077 == 51.8067..., which `f32::trunc` truncates
        // toward zero to 51.0. The live case that first caught the round-vs-truncate mismatch.
        let program = build_test_program(-8576.077, -4133.11);
        let branch = build_test_branch(program as u32, 0, &[3.9904687]);
        assert_eq!(branch.pct_remaining_on_program(), Some(51));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }

    #[test]
    fn normal_in_progress_case_matches_hand_computed_values() {
        let program = build_test_program(1000.0, 250.0);
        let branch = build_test_branch(program as u32, 0, &[30.0]);
        // (1000.0 - 250.0) * 100.0 / 1000.0 == 75.0
        assert_eq!(branch.pct_remaining_on_program(), Some(75));
        // (1000.0 - 250.0) * 30.0 / 30.0 == 750.0
        assert_eq!(branch.days_remaining_on_program(), Some(750.0));
        destroy_test_branch(&branch);
        destroy_test_program(program);
    }
}

/// `DAT_00630d78`, confirmed by reading the installed `zoo.exe`'s `.data` section directly (float
/// bytes `45 2e c2 37`, value `2.3148148e-5`) - **not** the same constant as `funding_text`'s
/// `MONTHLY_TO_DAILY_COST_SCALE` (`1.0/30.0`) despite both scaling a `cost`-shaped field by an elapsed
/// time unit; empirically `1.0 / 43200.0` to `f32` precision. Confirmed shared verbatim by
/// `ZTMarketing::update` too (`resources/decompiles/ZTMarketing_update.c` references the exact same
/// `_DAT_00630d78`, in the exact same `days * cost * scale` shape, right down to reusing
/// `ZTGameMgr::subtractCash`/an embedded `ZooStatus` "spend" call) - a shared days-to-funding-delta
/// scale used by more than just research.
const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;

/// Pure prediction for one `ZTResearchBranch::update(days)` call's progress/cash effect on the
/// currently-selected program, restricted to the "doesn't complete this call" case - `update` itself
/// handles completion (`on_completion`/`pick_random_program`/UI) using this function's result. Per
/// `resources/decompiles/ZTResearchBranch_update.c`/`.asm`: `cash_delta`/`progress_delta` are always
/// computed from `days`/the current funding level's `cost`/`rate`, but only actually applied - cash
/// subtracted, progress accumulated - when `cash_delta <= available_cash`; insufficient cash leaves
/// both unchanged for this call (silently - no partial progress, no debt), signalled here by returning
/// `(0.0, 0.0)`.
fn predict_branch_progress(days: u32, funding_cost: f32, funding_rate: f32, available_cash: f32) -> (f32, f32) {
    let cash_delta = days as f32 * funding_cost * DAYS_TO_FUNDING_SCALE;
    if cash_delta <= available_cash {
        let progress_delta = days as f32 * funding_rate * DAYS_TO_FUNDING_SCALE;
        (cash_delta, progress_delta)
    } else {
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod predict_branch_progress_tests {
    use super::*;

    #[test]
    fn affordable_case_scales_both_deltas() {
        let (cash_delta, progress_delta) = predict_branch_progress(10, 1000.0, 30.0, f32::MAX);
        assert!((cash_delta - 10.0 * 1000.0 * DAYS_TO_FUNDING_SCALE).abs() < f32::EPSILON);
        assert!((progress_delta - 10.0 * 30.0 * DAYS_TO_FUNDING_SCALE).abs() < f32::EPSILON);
    }

    #[test]
    fn insufficient_cash_leaves_both_deltas_zero() {
        // cash_delta for this input is well above the tiny available_cash below.
        assert_eq!(predict_branch_progress(1000, 1_000_000.0, 30.0, 1.0), (0.0, 0.0));
    }

    #[test]
    fn exactly_affordable_boundary_still_applies() {
        let cash_delta = 5.0 * 100.0 * DAYS_TO_FUNDING_SCALE;
        let (applied_cash, applied_progress) = predict_branch_progress(5, 100.0, 30.0, cash_delta);
        assert_eq!(applied_cash, cash_delta);
        assert!(applied_progress > 0.0);
    }

    #[test]
    fn zero_days_is_a_harmless_no_progress_no_op() {
        assert_eq!(predict_branch_progress(0, 1000.0, 30.0, 0.0), (0.0, 0.0));
    }

    #[test]
    fn negative_funding_cost_still_gates_on_the_same_comparison() {
        // A negative `cost` (not expected from real `.cfg` data, but the comparison itself has no
        // sign guard in vanilla) produces a negative `cash_delta`, which is always `<= available_cash`
        // for any non-negative budget - so it applies, "refunding" cash.
        let (cash_delta, progress_delta) = predict_branch_progress(10, -100.0, 30.0, 0.0);
        assert!(cash_delta < 0.0);
        assert!(progress_delta > 0.0);
    }
}

/// `GLOBAL_BFUIMgr`'s own fixed address (`0x00635c54`, confirmed via `private/docs/vtables/BFUIMgr.md`:
/// "address confirmed via its own constructor overwriting `BFMgr`'s vtable") - a plain static object,
/// not a pointer slot (every call site takes its address directly, e.g.
/// `BFUIMgr::getElement((BFUIMgr*)&GLOBAL_BFUIMgr, ...)`), unlike `GLOBAL_ZTResearchMgr`/
/// `GLOBAL_ZTGameMgr` which are one level of pointer indirection away from the real singleton.
fn global_bfuimgr() -> *const u32 {
    (get_module_base("zoo.exe") as u32 + 0x0023_5c54) as *const u32
}

/// The two dialog-`45000` element ids `ZTResearchBranch::update` looks up (`DAT_0063b94e`/
/// `DAT_0063b942+2`, both confirmed by reading the installed `zoo.exe`'s `.data` section directly: raw
/// `u16`s `2`/`6` respectively), each added to the shared dialog id `45000` also passed to
/// `confirmDialog` directly. Fixed data-section literals (not runtime/locale-dependent like Phase E's
/// `CURRENCYFMTA` fields), so hardcoded rather than read live.
const RESEARCH_DIALOG_ICON_ELEMENT_ID: i32 = 45000 + 2;
const RESEARCH_DIALOG_LABEL_ELEMENT_ID: i32 = 45000 + 6;
const RESEARCH_DIALOG_ID: i32 = 45000;

/// `s_ui/sharedui/exclaim/exclaim` - the icon animation `ZTResearchBranch::update` plays for both the
/// "research complete" and "no more research" dialogs (and, per
/// `resources/decompiles/ZTAnimal_showEscapedAnimalAlert.c`, the escaped-animal alert too - the same
/// shared dialog idiom reused elsewhere in the game). A fixed asset path literal, not something read
/// from game memory.
const RESEARCH_EXCLAIM_ANIMATION: &[u8] = b"ui/sharedui/exclaim/exclaim\0";

/// Looks up one of `ZTResearchBranch::update`'s two dialog-`45000` elements via the real
/// `BFUIMgr::getElement`, returning `None` for a null result (matching vanilla's own null checks
/// before touching either element further).
fn get_research_dialog_element(id: i32) -> Option<*const u32> {
    let element = unsafe { bfuimgr::GET_ELEMENT_0.original()(global_bfuimgr(), id) };
    (!element.is_null()).then_some(element)
}

/// The icon-animation + confirm-dialog tail `ZTResearchBranch::update` runs on program completion,
/// once for "research complete" and (reusing the very same `icon`/`label_present` values, not
/// re-fetched - matching vanilla, which reuses the same two elements for both) again for "no more
/// research" if `pick_random_program` didn't find a new one. See `ZTResearchBranch::update`'s own doc
/// comment for why the dialog's caption text itself is deliberately not set here.
fn show_research_dialog(icon: Option<*const u32>, label_present: bool) {
    if let Some(icon) = icon {
        unsafe {
            uicontrol::SET_ANIMATION.original()(icon, RESEARCH_EXCLAIM_ANIMATION.as_ptr() as *const i8, true);
        }
    }
    if label_present {
        unsafe {
            bfuimgr::CONFIRM_DIALOG_0.original()(global_bfuimgr(), RESEARCH_DIALOG_ID, 0u32, 0i8, 1i8, 0i32);
        }
    }
}

/// `DAT_00635040`, confirmed by reading the installed `zoo.exe`'s `.data` section directly (float
/// bytes `89 88 08 3d`, exactly `1.0f32 / 30.0f32`'s bit pattern) - the reciprocal of the `30.0`
/// day-scale constant `days_remaining_on_program` already confirms elsewhere in this file, consistent
/// with `cost` being a *monthly* figure (per the `.cfg` `cost=` examples, e.g. `min=400`) that
/// `funding_text` displays as a *daily* cost.
const MONTHLY_TO_DAILY_COST_SCALE: f32 = 1.0 / 30.0;

/// Reimplementation of the specific `bfinternat::getMoneyText` overload `ZTResearchBranch::getFundingText`
/// calls - confirmed against the installed `zoo.exe`'s real machine code (not just the decompile) at
/// address `0x0040eca1`: formats `value` with a plain `%d` (a whole-dollar amount, no cents - unlike
/// the sibling overload at `0x004ef4d4`, which takes a float and formats with `%.2f`), then hands that
/// numeral string to `GetCurrencyFormatA`. `getFundingText` always passes `useGrouping = false` for this
/// call, which - per the same disassembly - temporarily forces `CURRENCYFMTA::Grouping` to `0` around
/// the call (confirmed live: the real output has no thousands separator). Every other `CURRENCYFMTA`
/// field, including `NumDigits` (confirmed live to be `0` in the running game, not the `2` the *other*
/// overload's decompile forces - the two must not be confused), plus the locale id, is read live from
/// the exact fixed globals vanilla itself reads/mutates around this same call site
/// (`DAT_0063806c`/`lpFormat_0063b3a8`), rather than hardcoded from the static image, so this matches
/// whatever the running game's own locale-init code has set them to.
///
/// `pub(crate)`: reused verbatim by `ztmarketing::ZTMarketing::funding_text` - per the implementation
/// plan's item 3, `ZTMarketing::getFundingText` calls this exact same `bfinternat::getMoneyText`
/// overload (confirmed at the same `0x0040eca1` address), just with a different pre-scale on `cost`
/// (none, vs. `MONTHLY_TO_DAILY_COST_SCALE` here) - no reason to duplicate the `CURRENCYFMTA`/live-global
/// plumbing a second time.
pub(crate) fn get_money_text(value: i32) -> String {
    let base = get_module_base("zoo.exe") as u32;
    let num_digits = get_from_memory::<u32>(base + 0x0023_b3a8);
    let leading_zero = get_from_memory::<u32>(base + 0x0023_b3ac);
    let decimal_sep = get_from_memory::<u32>(base + 0x0023_b3b4);
    let thousand_sep = get_from_memory::<u32>(base + 0x0023_b3b8);
    let negative_order = get_from_memory::<u32>(base + 0x0023_b3bc);
    let positive_order = get_from_memory::<u32>(base + 0x0023_b3c0);
    let currency_symbol = get_from_memory::<u32>(base + 0x0023_b3c4);
    let locale = get_from_memory::<u32>(base + 0x0023_806c);

    let format = CURRENCYFMTA {
        NumDigits: num_digits,
        LeadingZero: leading_zero,
        Grouping: 0,
        lpDecimalSep: PSTR(decimal_sep as *mut u8),
        lpThousandSep: PSTR(thousand_sep as *mut u8),
        NegativeOrder: negative_order,
        PositiveOrder: positive_order,
        lpCurrencySymbol: PSTR(currency_symbol as *mut u8),
    };

    let Ok(value_cstr) = CString::new(value.to_string()) else {
        return String::new();
    };
    let mut buffer = [0u8; 0x200];
    let written = unsafe {
        GetCurrencyFormatA(locale, 0, PCSTR(value_cstr.as_ptr() as *const u8), Some(&format as *const CURRENCYFMTA), Some(&mut buffer))
    };
    if written <= 0 {
        return String::new();
    }
    crate::encoding_utils::decode_game_text(&buffer[..(written as usize - 1)])
}

impl fmt::Display for ZTResearchBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTResearchBranch {{")?;
        writeln!(f, "  id: {},", self.id)?;
        writeln!(f, "  name: {:?},", self.name())?;
        writeln!(f, "  desc: {:?},", self.desc())?;
        writeln!(f, "  icon: {:?},", self.icon())?;
        writeln!(f, "  noprogicon: {:?},", self.noprogicon())?;
        writeln!(f, "  category_count: {},", self.category_count())?;
        writeln!(f, "  current_funding_level: {},", self.current_funding_level)?;
        writeln!(f, "  current_category_ptr: {:#x},", self.current_category_ptr)?;
        writeln!(f, "  current_program_ptr: {:#x},", self.current_program_ptr)?;
        writeln!(f, "  pct_remaining_on_program: {:?},", self.pct_remaining_on_program())?;
        writeln!(f, "  days_remaining_on_program: {:?},", self.days_remaining_on_program())?;
        write!(f, "}}")
    }
}

/// The global research manager, one per game. Confirmed size `0x18` bytes (`openzt-detour/src/structs.rs`).
#[derive(Debug)]
#[repr(C)]
pub struct ZTResearchMgr {
    pad0: [u8; 0x8],                          // 0x00 - vtable? see `always_check_expansion` below for the flag byte both `ZTResearchBranch::update`/`pickRandomProgram` read via pointer arithmetic that lands just past this struct's own confirmed 0x18 bytes, not a real field of it
    elapsed_ticks: u32,                       // 0x08 - accumulates `ZTResearchMgr::update`'s delta; once ~359 in-game days have accrued, every branch is updated and this resets to 0
    branch_array: ZTArray<ZTResearchBranch>,  // 0x0c
}

/// Pure prediction for `ZTResearchMgr::update`'s accumulator/day-count bookkeeping, per
/// `resources/decompiles/ZTResearchMgr_update.c`/`.asm`. `delta_ticks` is added to
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
fn predict_update(elapsed_ticks_before: u32, delta_ticks: u32) -> (u32, u32) {
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
fn global_ztgamemgr_ptr() -> *mut crate::ztgamemgr::ZTGameMgr {
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
fn global_always_check_expansion() -> bool {
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
    /// fields entirely (previously misdocumented here as literal byte offset `0x01`, before this
    /// pointer-arithmetic scaling was worked out for Phase F). Read raw via pointer arithmetic rather
    /// than modeled as a struct field, to avoid inflating `ZTResearchMgr`'s own independently-confirmed
    /// size.
    ///
    /// When `true`, `update`/`pick_random_program` always run the per-category `isExpansionDisabled`
    /// check instead of only when `getAnyExpansionsDisabled()` is true; the flag's own deeper purpose
    /// is otherwise unconfirmed.
    fn always_check_expansion(&self) -> bool {
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

    /// Mutable counterpart to `get_branch`, used by `research_save_reimplementation`'s promoted
    /// `load` detour to apply a saved `current_funding_level` to the matching branch.
    fn get_branch_mut(&self, id: i32) -> Option<&'static mut ZTResearchBranch> {
        self.branches_mut().find(|branch| branch.id == id)
    }

    /// Mutable counterpart to `get_category`, used by `research_save_reimplementation`'s promoted
    /// `load` detour to apply a saved `enabled` flag to the matching category.
    fn get_category_mut(&self, id: i32) -> Option<&'static mut ZTResearchCategory> {
        self.branches_mut().flat_map(|branch| branch.categories_mut()).find(|category| category.id == id)
    }

    /// Mutable counterpart to `get_program`, used by `research_save_reimplementation`'s promoted
    /// `load` detour to apply a saved `current_progress` to the matching program.
    fn get_program_mut(&self, id: i32) -> Option<&'static mut ZTResearchProgram> {
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
    /// (`serialize(&snapshot_mgr(self))`, written via `standalone::WRITE_BYTES_TO_FILE`); under the
    /// `vanilla-research-save` feature no detour is installed and `.original()` reaches genuine
    /// vanilla code instead.
    pub fn save(&self, file: *const u32) -> bool {
        unsafe { ztresearchmgr::SAVE.original()((self as *const Self) as *const u32, file) != 0 }
    }

    /// Calls `ZTResearchMgr::load` - the save-file counterpart to `save()`. Per
    /// `resources/decompiles/ZTResearchMgr_load.c`/`.asm`, `load` always starts by resetting every
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
    /// behavior described above (reading the stream via `standalone::DEALLOCATE`); under the
    /// `vanilla-research-save` feature no detour is installed and `.original()` reaches genuine
    /// vanilla code instead.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        unsafe { ztresearchmgr::LOAD.original()((self as *mut Self) as *const u32, file, version) }
    }

    /// Reimplementation of `ZTResearchMgr::forceResearch` (the class-level half of the "research
    /// cheat"). Per `resources/decompiles/ZTResearchMgr_forceResearch.c`: for every branch, for every
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
        unsafe { ztresearchmgr::CLEAR_BRANCHES.original()((self as *mut Self) as *const u32) }
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
/// "force research" cheat actually triggers, per `resources/decompiles/_forceResearch.c`. It calls
/// `ZTResearchMgr::forceResearch(GLOBAL_ZTResearchMgr, false)` (the `continue_program` flag is
/// hardcoded `false` here - use `ZTResearchMgr::force_research` directly if you need `true`) and
/// then notifies every `ZTWorldMgr` entity of the change, so the world/UI actually refreshes; plain
/// `ZTResearchMgr::force_research` alone does not do that last step. Takes no arguments and needs no
/// `ZTResearchMgr` reference - it looks up the global instance itself.
pub fn force_research_cheat() {
    unsafe { standalone::FORCE_RESEARCH.original()() }
}

/// a command that prints the top-level ZTResearchMgr summary
/// usage: `get_ztresearchmgr`
fn command_get_ztresearchmgr(_args: Vec<&str>) -> Result<String, CommandError> {
    Ok(format!("{}", globals().ztresearchmgr()))
}

/// a command that resolves the ZTResearchMgr global step by step, logging each hop, so a crash
/// during resolution shows exactly which step it happened on instead of taking the whole game down
/// silently. Doesn't dereference the manager itself, only the pointer chain that finds it.
/// usage: `debug_research_ptr`
fn command_debug_research_ptr(_args: Vec<&str>) -> Result<String, CommandError> {
    let base = get_module_base("zoo.exe");
    info!("[research-debug] module base (\"zoo.exe\") = {:#x}", base);

    let global_slot = base as u32 + 0x0023_9010;
    info!("[research-debug] global slot address (base + 0x239010) = {:#x}", global_slot);

    let raw_slot_value = get_from_memory::<u32>(global_slot);
    info!("[research-debug] value stored at global slot = {:#x}", raw_slot_value);

    if raw_slot_value == 0 {
        error!("[research-debug] global slot is null; ZTResearchMgr not constructed yet, or the address/indirection is wrong");
        return Err(CommandError::new("global slot at base + 0x239010 is null".to_string()));
    }

    let mgr_ptr = globals().ztresearchmgr_ptr();
    info!("[research-debug] globals().ztresearchmgr_ptr() = {:#x}", mgr_ptr as u32);

    if mgr_ptr.is_null() {
        error!("[research-debug] resolved ZTResearchMgr pointer is null");
        return Err(CommandError::new("resolved ZTResearchMgr pointer is null".to_string()));
    }

    Ok(format!(
        "module_base={:#x} global_slot={:#x} raw_slot_value={:#x} mgr_ptr={:#x}",
        base, global_slot, raw_slot_value, mgr_ptr as u32
    ))
}

/// a command that prints every branch/category/program in the research tree, for verifying the
/// struct layouts in this file line up with the live game. Every pointer is logged and null-checked
/// before it's dereferenced, and iteration counts are sanity-checked against
/// `MAX_REASONABLE_*`, so if this crashes the log will show exactly which (branch, category,
/// program) index it happened on rather than taking the whole game down blind.
/// usage: `list_research`
fn command_list_research(_args: Vec<&str>) -> Result<String, CommandError> {
    let mgr_ptr = globals().ztresearchmgr_ptr();
    info!("[research-debug] ztresearchmgr_ptr() = {:#x}", mgr_ptr as u32);
    if mgr_ptr.is_null() {
        error!("[research-debug] ZTResearchMgr pointer is null; run debug_research_ptr() for details");
        return Err(CommandError::new("ZTResearchMgr pointer is null".to_string()));
    }

    let mgr = globals().ztresearchmgr();
    info!("[research-debug] mgr.elapsed_ticks = {}", mgr.elapsed_ticks);

    let branch_count = mgr.branch_array.len();
    info!("[research-debug] mgr.branch_array.len() = {}", branch_count);
    if branch_count > MAX_REASONABLE_BRANCHES {
        error!(
            "[research-debug] branch_array.len() = {} exceeds MAX_REASONABLE_BRANCHES ({}); bailing out before dereferencing to avoid a crash - the global address/indirection is probably wrong",
            branch_count, MAX_REASONABLE_BRANCHES
        );
        return Err(CommandError::new(format!("implausible branch count {}", branch_count)));
    }

    let mut result = format!("{}\n", mgr);

    for i in 0..branch_count {
        let branch_ptr = mgr.branch_array.get_ptr(i);
        info!("[research-debug] branch[{}] ptr = {:#x}", i, branch_ptr);
        if branch_ptr == 0 {
            warn!("[research-debug] branch[{}] is null, skipping", i);
            continue;
        }
        let branch = mgr.branch(i);
        result.push_str(&format!("{}\n", branch));

        for (level_index, level) in branch.funding_levels().iter().enumerate() {
            result.push_str(&format!(
                "  FundingLevel[{}]: name={:?} rate(work)={} cost={}\n",
                level_index,
                level.name(),
                level.rate(),
                level.cost()
            ));
        }

        let category_count = branch.category_array.len();
        info!("[research-debug] branch[{}] (id={}) category_array.len() = {}", i, branch.id, category_count);
        if category_count > MAX_REASONABLE_CATEGORIES {
            error!(
                "[research-debug] branch[{}].category_array.len() = {} exceeds MAX_REASONABLE_CATEGORIES ({}); skipping this branch's categories",
                i, category_count, MAX_REASONABLE_CATEGORIES
            );
            continue;
        }

        for j in 0..category_count {
            let category_ptr = branch.category_array.get_ptr(j);
            info!("[research-debug] branch[{}].category[{}] ptr = {:#x}", i, j, category_ptr);
            if category_ptr == 0 {
                warn!("[research-debug] branch[{}].category[{}] is null, skipping", i, j);
                continue;
            }
            let category = branch.category(j);
            result.push_str(&format!("{}\n", category));

            let program_count = category.program_array.len();
            info!(
                "[research-debug] branch[{}].category[{}] (id={}) program_array.len() = {}",
                i, j, category.id, program_count
            );
            if program_count > MAX_REASONABLE_PROGRAMS {
                error!(
                    "[research-debug] branch[{}].category[{}].program_array.len() = {} exceeds MAX_REASONABLE_PROGRAMS ({}); skipping this category's programs",
                    i, j, program_count, MAX_REASONABLE_PROGRAMS
                );
                continue;
            }

            for k in 0..program_count {
                let program_ptr = category.program_array.get_ptr(k);
                info!("[research-debug] branch[{}].category[{}].program[{}] ptr = {:#x}", i, j, k, program_ptr);
                if program_ptr == 0 {
                    warn!("[research-debug] branch[{}].category[{}].program[{}] is null, skipping", i, j, k);
                    continue;
                }
                let program = category.program(k);
                result.push_str(&format!("{}\n", program));
            }
        }
    }

    Ok(result)
}

/// a command that prints each branch's currently selected category/program and funding level
/// usage: `current_research`
fn command_current_research(_args: Vec<&str>) -> Result<String, CommandError> {
    let mgr = globals().ztresearchmgr();
    let mut result = String::new();

    for branch in mgr.branches() {
        result.push_str(&format!(
            "Branch {} ({:?}) - funding level {}",
            branch.id(),
            branch.name(),
            branch.current_funding_level()
        ));
        match branch.current_funding_rate() {
            Some(rate) => result.push_str(&format!(" (rate {}):\n", rate)),
            None => result.push_str(" (rate unavailable):\n"),
        }

        match branch.current_category() {
            Some(category) => {
                result.push_str(&format!("  category: id={} name={:?}\n", category.id(), category.name()))
            }
            None => result.push_str("  category: none selected\n"),
        }

        match branch.current_program() {
            Some(program) => {
                result.push_str(&format!(
                    "  program: id={}, progress={:.1}/{:.1}",
                    program.id(),
                    program.current_progress(),
                    program.target_cost()
                ));
                match branch.pct_remaining_on_program() {
                    Some(pct) => result.push_str(&format!(", {}% remaining", pct)),
                    None => result.push_str(", % remaining unavailable"),
                }
                match branch.days_remaining_on_program() {
                    Some(days) => result.push_str(&format!(", {:.1} days remaining\n", days)),
                    None => result.push_str(", days remaining unavailable\n"),
                }
            }
            None => result.push_str("  program: none selected\n"),
        }
    }

    Ok(result)
}

/// a command that would restore research progress from the active save file via `ZTResearchMgr::load`.
/// Currently unsupported from the console: `load` needs a real open save-file stream pointer (which it
/// dereferences directly to check the stream's EOF flag - not just something it forwards opaquely) plus
/// the save's format version, neither of which the console has access to. Previously this command called
/// `load()` with neither argument, which - since `load` is a 2-stack-arg `thiscall` (`ret 0x8`) - silently
/// corrupted the stack on return; that bug is fixed by requiring both arguments explicitly rather than by
/// guessing values that would just crash instead.
/// usage: `load_research`
fn command_load_research(_args: Vec<&str>) -> Result<String, CommandError> {
    Err(CommandError::new(
        "load_research is currently unsupported: ZTResearchMgr::load needs a real save-file stream pointer and version, which aren't available from the console".to_string(),
    ))
}

/// a command that triggers the same "force research" cheat the in-game button does (completes every
/// branch's current program and refreshes the world/UI) via `force_research_cheat`
/// usage: `force_research`
fn command_force_research(_args: Vec<&str>) -> Result<String, CommandError> {
    force_research_cheat();
    Ok("Forced research completion".to_string())
}

/// registers the Lua functions
pub fn init() {
    // get_ztresearchmgr() - no args
    lua_fn!("get_ztresearchmgr", "Returns ZTResearchMgr debug info", "get_ztresearchmgr()", || {
        match command_get_ztresearchmgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // debug_research_ptr() - no args
    lua_fn!(
        "debug_research_ptr",
        "Resolves the ZTResearchMgr global step by step, logging each hop",
        "debug_research_ptr()",
        || {
            match command_debug_research_ptr(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // list_research() - no args
    lua_fn!(
        "list_research",
        "Lists every research branch/category/program, logging every pointer as it's walked",
        "list_research()",
        || {
            match command_list_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // current_research() - no args
    lua_fn!(
        "current_research",
        "Shows each branch's currently selected category/program and funding level",
        "current_research()",
        || {
            match command_current_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // load_research() - no args
    lua_fn!(
        "load_research",
        "Restores research progress from the active save file",
        "load_research()",
        || {
            match command_load_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // force_research() - no args
    lua_fn!(
        "force_research",
        "Triggers the same 'force research' cheat the in-game button does",
        "force_research()",
        || {
            match command_force_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    research_config_reimplementation::init();
    research_save_reimplementation::init();
}

/// Native reimplementation of the `.cfg`-driven research tree loading
/// (`ZTResearchBranch::loadBranch`/`ZTResearchCategory::loadCategory`/`ZTResearchProgram::loadProgram`),
/// built on top of the existing generic `openzt-configparser` INI parser rather than a bespoke port of
/// `BFConfigFile`'s parsing engine.
///
/// By default (the `#[cfg(not(feature = "vanilla-research-config"))]` arm) this module owns the full
/// construction *and* destruction lifecycle of `ZTResearchBranch`/`ZTResearchCategory`/
/// `ZTResearchProgram` objects: `loadBranches` parses every manifest entry itself and splices
/// Rust-allocated objects directly into `branch_array`/`category_array`/`program_array`, and the six
/// vanilla lifecycle functions (`clearBranches`/`clearBranch`/`~ZTResearchBranch`/`clearCategory`/
/// `~ZTResearchCategory`/`~ZTResearchProgram`) are replaced outright with equivalent Rust logic that
/// frees the same Rust-allocated memory - so no vanilla `delete` ever runs against Rust-allocated memory
/// and no vanilla `new` output is ever freed with a mismatched deallocator. If parsing fails partway
/// through a manifest, the whole call falls back to vanilla's own `loadBranches` (see `replace_branches`).
///
/// With `--features vanilla-research-config`, only `loadBranches` is detoured, and it reverts to its
/// original shadow-mode behavior: the vanilla function always runs first and its result is always what's
/// returned, while this module independently parses the same files and logs any mismatch - useful for
/// re-validating the parsing/field logic shared by both arms without risking gameplay. The other six
/// lifecycle detours become plain passthroughs to vanilla in this arm.
mod research_config_reimplementation {
    use openzt_configparser::ini::Ini;
    use openzt_detour_macro::detour_mod;
    use tracing::{debug, error, info};

    use super::*;
    use crate::{encoding_utils::decode_game_text, resource_manager::lazyresourcemap::get_file};

    #[derive(Debug, Default)]
    struct ReimplementedProgram {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        entity_icon: Option<String>,
        cost: f32,
        order: i32,
        target: i32,
        effect: i32,
        effect_params: (i32, i32, i32),
        help_id: i32,
    }

    #[derive(Debug, Default)]
    struct ReimplementedFundingLevel {
        name_id: i32,
        rate: f32,
        cost: f32,
    }

    #[derive(Debug, Default)]
    struct ReimplementedCategory {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        help_id: i32,
        expansion_id: i32,
        programs: Vec<ReimplementedProgram>,
    }

    #[derive(Debug, Default)]
    struct ReimplementedBranch {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        noprogicon: Option<String>,
        funding: Vec<ReimplementedFundingLevel>,
        categories: Vec<ReimplementedCategory>,
    }

    /// Loads and parses a resource-relative `.cfg` path the same way `legacy_loading.rs` does for
    /// mod `.cfg` files, except with vanilla's actual comment convention (`;` only - `BFConfigFile::parse`
    /// never treats `#`/`:` as comments, unlike the leniency OpenZT's own mod loader allows).
    fn read_cfg(path: &str) -> Option<Ini> {
        let Some((_, data)) = get_file(path) else {
            error!("research-config-reimplementation: resource '{path}' not found");
            return None;
        };
        let text = decode_game_text(&data);
        let mut ini = Ini::new_cs();
        ini.set_comment_symbols(&[';']);
        match ini.read(text) {
            Ok(_) => Some(ini),
            Err(e) => {
                error!("research-config-reimplementation: failed to parse '{path}': {e}");
                None
            }
        }
    }

    /// All values for a repeated key, dropping any that trim to empty. Confirmed against the vanilla
    /// `.cfg` source (e.g. `icon=` with nothing after it in `research/branres.cfg`) and
    /// `BFConfigFile::addKeyVal` (`BFConfigFile_addKeyVal.c`): a value that trims to nothing is never
    /// pushed onto the key's value vector at all, so from `getString`/`getStringList`'s perspective an
    /// empty `icon=` line is indistinguishable from no `icon=` line - both leave the vector empty.
    /// `Ini::get_vec` keeps the empty string, so this filters it back out to match.
    fn values(ini: &Ini, section: &str, key: &str) -> Vec<String> {
        ini.get_vec(section, key).unwrap_or_default().into_iter().filter(|v| !v.trim().is_empty()).collect()
    }

    /// `BFConfigFile::getString`/`getInt`/`getFloat` all return the *first* value for a repeated key
    /// (see `BFConfigFile_getString.c`); `Ini::get` returns the *last* one instead, so pull from
    /// `values` directly to match vanilla.
    fn first(ini: &Ini, section: &str, key: &str) -> Option<String> {
        values(ini, section, key).into_iter().next()
    }

    fn first_parse<T: std::str::FromStr>(ini: &Ini, section: &str, key: &str) -> Option<T> {
        first(ini, section, key)?.parse().ok()
    }

    fn load_program(path: &str) -> Option<ReimplementedProgram> {
        let ini = read_cfg(path)?;
        Some(ReimplementedProgram {
            name_id: first_parse(&ini, "research", "name").unwrap_or_default(),
            desc_id: first_parse(&ini, "research", "desc").unwrap_or_default(),
            icon: first(&ini, "research", "icon"),
            entity_icon: first(&ini, "research", "entityIcon"),
            cost: first_parse(&ini, "research", "cost").unwrap_or_default(),
            order: first_parse(&ini, "research", "order").unwrap_or_default(),
            target: first_parse(&ini, "research", "target").unwrap_or(-1),
            effect: first_parse(&ini, "research", "effect").unwrap_or(-1),
            effect_params: (
                first_parse(&ini, "research", "effectval1").unwrap_or_default(),
                first_parse(&ini, "research", "effectval2").unwrap_or_default(),
                first_parse(&ini, "research", "effectval3").unwrap_or_default(),
            ),
            help_id: first_parse(&ini, "research", "helpid").unwrap_or_default(),
        })
    }

    fn load_category(path: &str) -> Option<ReimplementedCategory> {
        let ini = read_cfg(path)?;
        let programs = values(&ini, "category", "program").iter().filter_map(|p| load_program(p)).collect();
        Some(ReimplementedCategory {
            name_id: first_parse(&ini, "category", "name").unwrap_or_default(),
            desc_id: first_parse(&ini, "category", "desc").unwrap_or_default(),
            icon: first(&ini, "category", "icon"),
            help_id: first_parse(&ini, "category", "helpid").unwrap_or_default(),
            expansion_id: first_parse(&ini, "category", "expansion").unwrap_or_default(),
            programs,
        })
    }

    fn load_branch(ini: &Ini) -> ReimplementedBranch {
        let categories = values(ini, "branch", "category").iter().filter_map(|p| load_category(p)).collect();
        let funding = values(ini, "branch", "funding")
            .iter()
            .map(|block| ReimplementedFundingLevel {
                name_id: first_parse(ini, block, "name").unwrap_or_default(),
                rate: first_parse(ini, block, "work").unwrap_or_default(),
                cost: first_parse(ini, block, "cost").unwrap_or_default(),
            })
            .collect();
        ReimplementedBranch {
            name_id: first_parse(ini, "branch", "name").unwrap_or_default(),
            desc_id: first_parse(ini, "branch", "desc").unwrap_or_default(),
            icon: first(ini, "branch", "icon"),
            noprogicon: first(ini, "branch", "noprogicon"),
            funding,
            categories,
        }
    }

    /// Resolves a string id the same way `load_string_by_id` does, defaulting to an empty string
    /// (matching how `cached_name`/`cached_desc` read back when nothing was ever cached) instead of
    /// `None`, so it can be compared directly against `cached_name()`/`cached_desc()`.
    fn resolved(id: i32) -> String {
        load_string_by_id(id as u32).unwrap_or_default()
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_program(path: &str, live: &ZTResearchProgram, reimpl: &ReimplementedProgram, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.cached_desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.cached_desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.entity_icon() != reimpl.entity_icon {
            mismatches.push(format!("{path}: entityIcon {:?} != {:?}", live.entity_icon(), reimpl.entity_icon));
        }
        if (live.target_cost() - reimpl.cost).abs() > f32::EPSILON {
            mismatches.push(format!("{path}: cost {} != {}", live.target_cost(), reimpl.cost));
        }
        if live.priority() != reimpl.order as u32 {
            mismatches.push(format!("{path}: order {} != {}", live.priority(), reimpl.order));
        }
        if live.target_id() != reimpl.target {
            mismatches.push(format!("{path}: target {} != {}", live.target_id(), reimpl.target));
        }
        if live.effect_kind_raw != reimpl.effect {
            mismatches.push(format!("{path}: effect {} != {}", live.effect_kind_raw, reimpl.effect));
        }
        if live.effect_params() != reimpl.effect_params {
            mismatches.push(format!("{path}: effect params {:?} != {:?}", live.effect_params(), reimpl.effect_params));
        }
        if live.help_id() != reimpl.help_id {
            mismatches.push(format!("{path}: helpid {} != {}", live.help_id(), reimpl.help_id));
        }
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_category(path: &str, live: &ZTResearchCategory, reimpl: &ReimplementedCategory, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.help_id() != reimpl.help_id {
            mismatches.push(format!("{path}: helpid {} != {}", live.help_id(), reimpl.help_id));
        }
        if live.expansion_id() != reimpl.expansion_id {
            mismatches.push(format!("{path}: expansion {} != {}", live.expansion_id(), reimpl.expansion_id));
        }
        if live.program_count() != reimpl.programs.len() {
            mismatches.push(format!("{path}: program count {} != {}", live.program_count(), reimpl.programs.len()));
        }
        for (i, (live_program, reimpl_program)) in live.programs().zip(reimpl.programs.iter()).enumerate() {
            compare_program(&format!("{path}[program {i}]"), live_program, reimpl_program, mismatches);
        }
    }

    /// Compares a just-`loadBranch`-ed live branch against the reimplementation's parse of the same
    /// file. `categories_before` is the branch's `category_count()` *before* this call to `loadBranch`
    /// ran.
    ///
    /// `loadBranch` never clears the branch first (unlike `loadCategory`, whose first line is
    /// `clearCategory`) - it only resets `current_funding_level`. `category_array` is append-only
    /// across repeated `loadBranch` calls on the same branch object (confirmed: `research/branres.cfg`
    /// and `research/brandres.cfg` - base game vs. the dinosaur pack's branch file - both declare
    /// `name=23050` and get loaded into the literal same object, confirmed via the raw `this` pointer
    /// logged from both calls being identical), so it's compared positionally against just the entries
    /// newly appended by *this* call, skipping however many were already there.
    ///
    /// The funding table is different: it's overwritten in place on every call, not accumulated -
    /// confirmed by poisoning an existing entry's `name_id` with an impossible sentinel right before
    /// calling the original `loadBranch` and observing it always comes back overwritten (with the
    /// correct value, not the sentinel), even though the table's start pointer/capacity/count never
    /// change across calls. So it's compared directly, in full, against this file's own parsed funding
    /// list - the same plain "overwritten every call" treatment as the `name`/`desc`/`icon` header
    /// fields, not the accumulate-and-skip treatment `category_array` needs.
    #[cfg(feature = "vanilla-research-config")]
    fn compare_branch(path: &str, live: &ZTResearchBranch, reimpl: &ReimplementedBranch, categories_before: usize, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.noprogicon() != reimpl.noprogicon {
            mismatches.push(format!("{path}: noprogicon {:?} != {:?}", live.noprogicon(), reimpl.noprogicon));
        }
        let live_funding = live.funding_levels();
        if live_funding.len() != reimpl.funding.len() {
            mismatches.push(format!("{path}: funding level count {} != {}", live_funding.len(), reimpl.funding.len()));
        }
        for (i, (live_level, reimpl_level)) in live_funding.iter().zip(reimpl.funding.iter()).enumerate() {
            if live_level.name_id() != reimpl_level.name_id {
                mismatches.push(format!("{path}[funding {i}]: name id {} != {}", live_level.name_id(), reimpl_level.name_id));
            }
            if (live_level.rate() - reimpl_level.rate).abs() > f32::EPSILON {
                mismatches.push(format!("{path}[funding {i}]: work/rate {} != {}", live_level.rate(), reimpl_level.rate));
            }
            if (live_level.cost() - reimpl_level.cost).abs() > f32::EPSILON {
                mismatches.push(format!("{path}[funding {i}]: cost {} != {}", live_level.cost(), reimpl_level.cost));
            }
        }
        let new_category_count = live.category_count().saturating_sub(categories_before);
        if new_category_count != reimpl.categories.len() {
            mismatches.push(format!(
                "{path}: newly appended category count {} != {} (already had {categories_before} before this call)",
                new_category_count,
                reimpl.categories.len()
            ));
        }
        for (i, (live_category, reimpl_category)) in live.categories().skip(categories_before).zip(reimpl.categories.iter()).enumerate() {
            compare_category(&format!("{path}[category {i}]"), live_category, reimpl_category, mismatches);
        }
    }

    /// Peeks a branch file's own declared `[branch] name=<id>` without doing a full load - the same
    /// thing `ZTResearchMgr::loadBranches` does per manifest entry (via a throwaway `BFConfigFile`)
    /// before deciding whether to reuse an existing branch or create a new one.
    #[cfg(feature = "vanilla-research-config")]
    fn peek_branch_id(path: &str) -> Option<i32> {
        let ini = read_cfg(path)?;
        first_parse(&ini, "branch", "name")
    }

    /// The manifest's own `[branches] branch=...` list (confirmed against the real
    /// `config.ztd!research.cfg` - `[branches]` section, repeated `branch=` key - not the
    /// `[branch] category=...`/`funding=...` shape every other file in this family uses).
    fn load_manifest(ini: &Ini) -> Vec<String> {
        values(ini, "branches", "branch")
    }

    /// One manifest entry, resolved against live state *before* `loadBranches` runs: which branch file,
    /// what id it declares, and how many categories that id's branch already had (0 for an id that
    /// doesn't exist yet, i.e. this entry will create a new branch rather than extend an existing one).
    #[cfg(feature = "vanilla-research-config")]
    struct ManifestEntry {
        path: String,
        id: i32,
        categories_before: usize,
    }

    #[cfg(feature = "vanilla-research-config")]
    fn manifest_entries(manifest_ini: &Ini, mgr_before: &ZTResearchMgr) -> Vec<ManifestEntry> {
        load_manifest(manifest_ini)
            .into_iter()
            .filter_map(|path| {
                let Some(id) = peek_branch_id(&path) else {
                    error!("research-config-reimplementation: failed to peek id from '{path}' listed in manifest");
                    return None;
                };
                let categories_before = mgr_before.get_branch(id).map(|b| b.category_count()).unwrap_or(0);
                Some(ManifestEntry { path, id, categories_before })
            })
            .collect()
    }

    /// Compares the whole outcome of a `loadBranches` call: that `branch_array` grew by exactly the
    /// number of genuinely new ids (an id already present before this call, or repeated within the same
    /// manifest, is a reuse and shouldn't grow the count), and, for every manifest entry, the full
    /// branch/category/program content behind its (possibly reused) branch object - the same
    /// `compare_branch` used to run from a per-`loadBranch`-call detour, now run once per manifest entry
    /// from the single top-level `loadBranches` hook instead.
    #[cfg(feature = "vanilla-research-config")]
    fn compare_load_branches(manifest_path: &str, ids_before: &std::collections::HashSet<i32>, mgr_after: &ZTResearchMgr, entries: &[ManifestEntry], mismatches: &mut Vec<String>) {
        let new_id_count = entries.iter().map(|e| e.id).collect::<std::collections::HashSet<_>>().difference(ids_before).count();
        let expected_count = ids_before.len() + new_id_count;
        if mgr_after.branch_count() != expected_count {
            mismatches.push(format!(
                "{manifest_path}: branch_array has {} entries after, expected {} ({} before + {} new)",
                mgr_after.branch_count(),
                expected_count,
                ids_before.len(),
                new_id_count
            ));
        }

        for entry in entries {
            let Some(live_branch) = mgr_after.get_branch(entry.id) else {
                mismatches.push(format!("{}: branch id {} not found in branch_array after loadBranches", entry.path, entry.id));
                continue;
            };
            let Some(ini) = read_cfg(&entry.path) else {
                error!("research-config-reimplementation: failed to independently parse '{}' for comparison", entry.path);
                continue;
            };
            let reimpl = load_branch(&ini);
            compare_branch(&entry.path, live_branch, &reimpl, entry.categories_before, mismatches);
        }
    }

    /// Low-level raw-memory helpers shared by `construction` and `destruction`: reconstructing an
    /// owning `Vec` from a live struct's raw start/end/(buffer_end|capacity) pointers, and the
    /// reverse (leaking a `Vec`'s buffer back into those same three raw pointers). Every allocation
    /// in this file goes through Rust's own allocator (`Box`/`Vec`), never vanilla's game heap - see
    /// the module doc comment above for why. `ZTArray<T>`/`ZTBufferString`'s 3-pointer shape
    /// (`start_ptr`/`end_ptr`/`buffer_end_ptr`) is the same read-side layout already validated
    /// extensively elsewhere in this file; these helpers just add the write side.
    #[cfg(not(feature = "vanilla-research-config"))]
    mod raw_mem {
        use std::ffi::{c_char, CString};

        use super::*;

        /// Allocates a null-terminated buffer for `text` and wraps it in a `ZTBufferString`. The
        /// null terminator sits immediately at `end_ptr` (matching a plain C string), with
        /// `buffer_end_ptr` covering whatever spare capacity the backing `Vec` happened to allocate -
        /// harmless slack, not a correctness requirement, since every reader in this file
        /// (`copy_to_string`/etc.) stops at `end_ptr` regardless.
        pub(super) fn alloc_buffer_string(text: &str) -> ZTBufferString {
            let mut bytes = text.as_bytes().to_vec();
            let len = bytes.len() as u32;
            bytes.push(0);
            let cap = bytes.capacity() as u32;
            let ptr = bytes.as_mut_ptr() as u32;
            std::mem::forget(bytes);
            ZTBufferString::from_raw_parts(ptr, ptr + len, ptr + cap)
        }

        pub(super) fn free_buffer_string(s: &ZTBufferString) {
            let (start, _end, buffer_end) = s.raw_parts();
            if start == 0 {
                return;
            }
            let cap = (buffer_end - start) as usize;
            drop(unsafe { Vec::<u8>::from_raw_parts(start as *mut u8, cap, cap) });
        }

        /// Allocates an owned, null-terminated C string for `text` (0 for `None`) for
        /// `icon_ptr`/`entity_icon_ptr`/`noprogicon_ptr`. Unlike vanilla - whose equivalent fields
        /// point into a shared, separately-owned config string table that's never freed per-object
        /// (see every `~ZTResearch*` decompile: none of them touch these fields) - we own this
        /// allocation outright, so `free_owned_cstring` (not vanilla) is responsible for reclaiming
        /// it, both when overwriting an existing value and during full teardown.
        pub(super) fn alloc_owned_cstring(text: Option<&str>) -> u32 {
            match text.and_then(|t| CString::new(t).ok()) {
                Some(cstring) => cstring.into_raw() as u32,
                None => 0,
            }
        }

        pub(super) fn free_owned_cstring(ptr: u32) {
            if ptr != 0 {
                drop(unsafe { CString::from_raw(ptr as *mut c_char) });
            }
        }

        /// Reconstructs an owning `Vec<u32>` of raw pointers from a `ZTArray<T>`'s raw parts, or an
        /// empty `Vec` if the array was never allocated (`start_ptr == 0` - vanilla's own
        /// representation of an empty array; see e.g. `ZTResearchCategory::ZTResearchCategory`'s
        /// constructor decompile).
        pub(super) fn vec_from_ptr_array<T>(array: &ZTArray<T>) -> Vec<u32> {
            let (start, end, buffer_end) = array.raw_parts();
            if start == 0 {
                return Vec::new();
            }
            let len = ((end - start) / 4) as usize;
            let cap = ((buffer_end - start) / 4) as usize;
            unsafe { Vec::from_raw_parts(start as *mut u32, len, cap) }
        }

        /// Inverse of `vec_from_ptr_array`: leaks `vec`'s buffer into a fresh `ZTArray<T>` (all-zero
        /// raw parts if empty, matching vanilla's own empty-array representation).
        pub(super) fn ptr_array_from_vec<T>(mut vec: Vec<u32>) -> ZTArray<T> {
            if vec.is_empty() {
                return ZTArray::from_raw_parts(0, 0, 0);
            }
            let ptr = vec.as_mut_ptr() as u32;
            let len = vec.len() as u32;
            let cap = vec.capacity() as u32;
            std::mem::forget(vec);
            ZTArray::from_raw_parts(ptr, ptr + len * 4, ptr + cap * 4)
        }

        /// Fully frees a pointer array's own backing buffer (the pointers it contained must already
        /// have been destroyed by the caller - this only reclaims the array-of-pointers allocation
        /// itself). Used only by the full-teardown paths in `destruction`; the "reset in place, keep
        /// capacity" paths never call this.
        pub(super) fn free_ptr_array<T>(array: &ZTArray<T>) {
            let (start, _end, buffer_end) = array.raw_parts();
            if start == 0 {
                return;
            }
            let cap = ((buffer_end - start) / 4) as usize;
            drop(unsafe { Vec::<u32>::from_raw_parts(start as *mut u32, cap, cap) });
        }

        /// Leaks `vec` into `ZTResearchBranch`'s funding-table raw parts (all-zero if empty).
        pub(super) fn funding_table_from_vec(mut vec: Vec<ZTResearchFundingLevel>) -> (u32, u32, u32) {
            if vec.is_empty() {
                return (0, 0, 0);
            }
            let stride = size_of::<ZTResearchFundingLevel>() as u32;
            let ptr = vec.as_mut_ptr() as u32;
            let len = vec.len() as u32;
            let cap = vec.capacity() as u32;
            std::mem::forget(vec);
            (ptr, ptr + len * stride, ptr + cap * stride)
        }

        /// Frees a branch's funding-table buffer (the inline `ZTResearchFundingLevel` block, *not* a
        /// `ZTArray` of pointers - see the field's own doc comment on `ZTResearchBranch`).
        pub(super) fn free_funding_table(branch: &ZTResearchBranch) {
            let start = branch.funding_table_start;
            if start == 0 {
                return;
            }
            let stride = size_of::<ZTResearchFundingLevel>() as u32;
            let cap = ((branch.funding_table_capacity - start) / stride) as usize;
            drop(unsafe { Vec::<ZTResearchFundingLevel>::from_raw_parts(start as *mut ZTResearchFundingLevel, cap, cap) });
        }
    }

    /// Builds fresh `ZTResearchBranch`/`ZTResearchCategory`/`ZTResearchProgram` objects (and extends
    /// existing branches) directly from the parsed `Reimplemented*` values, entirely on Rust's own
    /// allocator. See the module doc comment above for why this replaces vanilla's `loadBranches`
    /// wholesale rather than calling into it.
    #[cfg(not(feature = "vanilla-research-config"))]
    mod construction {
        use super::{raw_mem::*, *};

        /// Builds a fresh program and populates every field from `reimpl`. Mirrors the tail of
        /// vanilla `ZTResearchProgram::loadProgram`, which always ends by calling `reset()` - for
        /// `UnlockEntity`/`BuildingUpgrade` effects that's what actually marks the target
        /// entity/building unavailable until the program completes (`setAvail`/`setBuildingUpgrade`);
        /// skipping it would silently leave every such target available from the start. `reset()` is
        /// left as a call into the original implementation (like `on_completion`/`pick_random_program`
        /// elsewhere in this file) since its downstream effects aren't independently reimplemented.
        fn construct_program(reimpl: &ReimplementedProgram) -> *mut ZTResearchProgram {
            let program = Box::new(ZTResearchProgram {
                config_file: BFConfigFile::default(),
                cached_name: alloc_buffer_string(&resolved(reimpl.name_id)),
                cached_desc: alloc_buffer_string(&resolved(reimpl.desc_id)),
                desc_id: reimpl.desc_id,
                icon_ptr: alloc_owned_cstring(reimpl.icon.as_deref()),
                entity_icon_ptr: alloc_owned_cstring(reimpl.entity_icon.as_deref()),
                id: reimpl.name_id,
                target_cost: reimpl.cost,
                current_progress: 0.0,
                priority: reimpl.order as u32,
                target_id: reimpl.target,
                effect_kind_raw: reimpl.effect,
                effect_param_0: reimpl.effect_params.0,
                effect_param_1: reimpl.effect_params.1,
                effect_param_2: reimpl.effect_params.2,
                help_id: reimpl.help_id,
            });
            let ptr = Box::into_raw(program);
            unsafe { (*ptr).reset() };
            ptr
        }

        /// Builds a fresh category, including its full `program_array`. `enabled` is always forced to
        /// `1`, matching `ZTResearchCategory::clearCategory` - called unconditionally as
        /// `loadCategory`'s first line in vanilla, so every loaded category ends up enabled regardless
        /// of anything in the `.cfg` (there is no `enabled=` key in this file format at all).
        fn construct_category(reimpl: &ReimplementedCategory) -> *mut ZTResearchCategory {
            let mut programs = Vec::with_capacity(reimpl.programs.len());
            for program in &reimpl.programs {
                programs.push(construct_program(program) as u32);
            }
            let category = Box::new(ZTResearchCategory {
                config_file: BFConfigFile::default(),
                id: reimpl.name_id,
                cached_name: alloc_buffer_string(&resolved(reimpl.name_id)),
                cached_desc: alloc_buffer_string(&resolved(reimpl.desc_id)),
                icon_ptr: alloc_owned_cstring(reimpl.icon.as_deref()),
                help_id: reimpl.help_id,
                expansion_id: reimpl.expansion_id,
                enabled: 1,
                pad2: [0; 3],
                program_array: ptr_array_from_vec(programs),
            });
            Box::into_raw(category)
        }

        /// Appends a freshly-constructed branch pointer to `mgr.branch_array`.
        fn append_branch(mgr: &mut ZTResearchMgr, ptr: u32) {
            let mut branches = vec_from_ptr_array(&mgr.branch_array);
            branches.push(ptr);
            mgr.branch_array = ptr_array_from_vec(branches);
        }

        /// Applies one manifest entry's parsed branch to `mgr`: reuses the existing branch object if
        /// one with this id already exists (matching vanilla's own id-based reuse in `loadBranches`),
        /// or allocates a fresh one and appends it to `branch_array` otherwise. Header fields and the
        /// funding table are overwritten every call; `category_array` is append-only across repeated
        /// calls on the same branch - see `compare_branch`'s doc comment (the shadow-mode arm, above)
        /// for how this was confirmed against the live game.
        fn apply_branch(mgr: &mut ZTResearchMgr, reimpl: &ReimplementedBranch) {
            let id = reimpl.name_id;
            // Materialize the lookup into a plain (borrow-free) raw pointer before matching on it -
            // `branches_mut()`'s returned iterator otherwise keeps `mgr` immutably borrowed for the
            // whole `match`, conflicting with the `None` arm's `append_branch(mgr, ...)`.
            let found: Option<*mut ZTResearchBranch> = mgr.branches_mut().find(|b| b.id() == id).map(|b| b as *mut ZTResearchBranch);
            let ptr: *mut ZTResearchBranch = match found {
                Some(existing) => existing,
                None => {
                    let branch = Box::new(ZTResearchBranch {
                        config_file: BFConfigFile::default(),
                        id,
                        cached_name: alloc_buffer_string(""),
                        cached_desc: alloc_buffer_string(""),
                        icon_ptr: 0,
                        noprogicon_ptr: 0,
                        current_category_ptr: 0,
                        current_program_ptr: 0,
                        category_array: ZTArray::from_raw_parts(0, 0, 0),
                        current_funding_level: 0,
                        funding_table_start: 0,
                        funding_table_end: 0,
                        funding_table_capacity: 0,
                    });
                    let ptr = Box::into_raw(branch);
                    append_branch(mgr, ptr as u32);
                    ptr
                }
            };

            let branch = unsafe { &mut *ptr };

            branch.id = id;
            free_buffer_string(&branch.cached_name);
            branch.cached_name = alloc_buffer_string(&resolved(reimpl.name_id));
            free_buffer_string(&branch.cached_desc);
            branch.cached_desc = alloc_buffer_string(&resolved(reimpl.desc_id));
            free_owned_cstring(branch.icon_ptr);
            branch.icon_ptr = alloc_owned_cstring(reimpl.icon.as_deref());
            free_owned_cstring(branch.noprogicon_ptr);
            branch.noprogicon_ptr = alloc_owned_cstring(reimpl.noprogicon.as_deref());

            let mut categories = vec_from_ptr_array(&branch.category_array);
            for category in &reimpl.categories {
                categories.push(construct_category(category) as u32);
            }
            branch.category_array = ptr_array_from_vec(categories);

            free_funding_table(branch);
            let mut funding = Vec::with_capacity(reimpl.funding.len());
            for level in &reimpl.funding {
                funding.push(ZTResearchFundingLevel { name_id: level.name_id, rate: level.rate, cost: level.cost });
            }
            let (start, end, capacity) = funding_table_from_vec(funding);
            branch.funding_table_start = start;
            branch.funding_table_end = end;
            branch.funding_table_capacity = capacity;
            branch.current_funding_level = 0;

            // Mirrors the tail of vanilla `ZTResearchBranch::loadBranch`, which always ends by
            // calling `pickRandomProgram` - left as a call into the original implementation since it
            // consumes the game's RNG stream (see `pick_random_program`'s doc comment elsewhere in
            // this file for why reimplementing it natively risks desyncing that stream).
            branch.pick_random_program();
        }

        /// Parses every manifest entry fully, touching nothing but local state - no `ZTResearchMgr`
        /// access at all, deliberately, so this is always safe to fall back from (a parse failure or a
        /// panic here can never leave `mgr` partially mutated). `apply_all` below is the mutating half,
        /// and is *not* safe to fall back from once it's started - see its own doc comment.
        pub(super) fn parse_manifest(manifest_path: &str) -> Option<Vec<ReimplementedBranch>> {
            let manifest_ini = read_cfg(manifest_path)?;
            let paths = load_manifest(&manifest_ini);

            let mut parsed = Vec::with_capacity(paths.len());
            for path in &paths {
                let ini = read_cfg(path)?;
                parsed.push(load_branch(&ini));
            }
            Some(parsed)
        }

        /// Splices every already-parsed branch into `mgr`. Only ever called after `parse_manifest`
        /// has fully succeeded, so this itself can't "fail" in the ordinary sense - the only way it
        /// stops partway through is a panic, which the caller must *not* respond to by falling back to
        /// vanilla (see the module doc comment above: `mgr` may already be partially mutated with
        /// Rust-allocated objects by that point, and vanilla's `loadBranches` has no way to know that -
        /// it would either reuse/extend our still-being-built branch mid-construction or duplicate
        /// work already done, neither of which is safe).
        pub(super) fn apply_all(mgr: &mut ZTResearchMgr, parsed: &[ReimplementedBranch]) {
            for reimpl in parsed {
                apply_branch(mgr, reimpl);
            }
        }
    }

    /// Faithful translations of the decompiled `clearBranch`/`clearCategory`/`~ZTResearchBranch`/
    /// `~ZTResearchCategory`/`~ZTResearchProgram`/`clearBranches` into Rust, using `Box`/`Vec`
    /// reclamation instead of vanilla's free helpers (see the module doc comment above). Every
    /// `loaded`/`tree_root`-gated branch in those decompiles is skipped here on purpose: every object
    /// this module ever constructs (`construction`, above) leaves both fields at 0 forever, so those
    /// branches never fire on real vanilla objects and would be actively unsafe to run against ours
    /// (vanilla's `BFConfigFile::release` dereferences `tree_root` unconditionally).
    #[cfg(not(feature = "vanilla-research-config"))]
    mod destruction {
        use super::{raw_mem::*, *};

        /// Calls `destroy` on every non-null pointer in a raw `start_ptr..end_ptr` pointer array
        /// (stride 4) without touching the array's own backing buffer - the "iterate and free
        /// contents, keep capacity" half shared by every `reset_*_contents`/`clear_branches` below.
        fn destroy_each_ptr<T>(start_ptr: u32, end_ptr: u32, mut destroy: impl FnMut(*mut T)) {
            let mut addr = start_ptr;
            while addr < end_ptr {
                let ptr = get_from_memory::<u32>(addr) as *mut T;
                if !ptr.is_null() {
                    destroy(ptr);
                }
                addr += 4;
            }
        }

        /// Full teardown of a program - there's no "clear in place" for programs, only full destroy
        /// (see `ZTResearchProgram::~ZTResearchProgram`'s decompile).
        pub(super) unsafe fn destroy_program(ptr: *mut ZTResearchProgram) {
            if ptr.is_null() {
                return;
            }
            let program = unsafe { &*ptr };
            free_buffer_string(&program.cached_name);
            free_buffer_string(&program.cached_desc);
            free_owned_cstring(program.icon_ptr);
            free_owned_cstring(program.entity_icon_ptr);
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Mirrors `ZTResearchCategory::clearCategory`: resets the category to an empty, disabled-id
        /// (but still `enabled`) state in place, destroying every program but keeping
        /// `program_array`'s own backing buffer.
        pub(super) fn reset_category_contents(category: &mut ZTResearchCategory) {
            category.id = -1;
            free_buffer_string(&category.cached_name);
            category.cached_name = alloc_buffer_string("");
            free_buffer_string(&category.cached_desc);
            category.cached_desc = alloc_buffer_string("");
            category.enabled = 1;
            free_owned_cstring(category.icon_ptr);
            category.icon_ptr = 0;
            category.help_id = 0;
            category.expansion_id = 0;
            let (start, end, buffer_end) = category.program_array.raw_parts();
            destroy_each_ptr::<ZTResearchProgram>(start, end, |p| unsafe { destroy_program(p) });
            category.program_array = ZTArray::from_raw_parts(start, start, buffer_end);
        }

        /// Mirrors `ZTResearchCategory::~ZTResearchCategory`: `reset_category_contents` then frees
        /// `program_array`/`cached_desc`/`cached_name`'s own buffers and the category object itself.
        pub(super) unsafe fn destroy_category(ptr: *mut ZTResearchCategory) {
            if ptr.is_null() {
                return;
            }
            let category = unsafe { &mut *ptr };
            reset_category_contents(category);
            free_ptr_array(&category.program_array);
            free_buffer_string(&category.cached_desc);
            free_buffer_string(&category.cached_name);
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Mirrors `ZTResearchBranch::clearBranch`: resets the branch to an empty, id `-1` state in
        /// place, destroying every category but keeping `category_array`'s own backing buffer, and
        /// emptying (but not freeing) the funding table.
        pub(super) fn reset_branch_contents(branch: &mut ZTResearchBranch) {
            branch.id = -1;
            free_buffer_string(&branch.cached_name);
            branch.cached_name = alloc_buffer_string("");
            free_buffer_string(&branch.cached_desc);
            branch.cached_desc = alloc_buffer_string("");
            free_owned_cstring(branch.icon_ptr);
            branch.icon_ptr = 0;
            free_owned_cstring(branch.noprogicon_ptr);
            branch.noprogicon_ptr = 0;
            branch.current_category_ptr = 0;
            branch.current_program_ptr = 0;
            let (start, end, buffer_end) = branch.category_array.raw_parts();
            destroy_each_ptr::<ZTResearchCategory>(start, end, |p| unsafe { destroy_category(p) });
            branch.category_array = ZTArray::from_raw_parts(start, start, buffer_end);
            branch.current_funding_level = 0;
            branch.funding_table_end = branch.funding_table_start;
        }

        /// Mirrors `ZTResearchBranch::~ZTResearchBranch`: `reset_branch_contents` then frees the
        /// funding table/`category_array`/`cached_desc`/`cached_name`'s own buffers and the branch
        /// object itself.
        pub(super) unsafe fn destroy_branch(ptr: *mut ZTResearchBranch) {
            if ptr.is_null() {
                return;
            }
            let branch = unsafe { &mut *ptr };
            reset_branch_contents(branch);
            free_funding_table(branch);
            free_ptr_array(&branch.category_array);
            free_buffer_string(&branch.cached_desc);
            free_buffer_string(&branch.cached_name);
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Mirrors `ZTResearchMgr::clearBranches`: destroys and frees every branch, then resets
        /// `branch_array` to empty (keeping its own backing buffer) and zeroes `elapsed_ticks`.
        pub(super) fn clear_branches(mgr: &mut ZTResearchMgr) {
            let (start, end, buffer_end) = mgr.branch_array.raw_parts();
            destroy_each_ptr::<ZTResearchBranch>(start, end, |p| unsafe { destroy_branch(p) });
            mgr.branch_array = ZTArray::from_raw_parts(start, start, buffer_end);
            mgr.elapsed_ticks = 0;
        }

        /// Pure, no-live-game-dependency tests for this module's postconditions (see each function's
        /// own doc comment above for what it's meant to do). A genuine live A/B comparison against real
        /// vanilla destructors isn't viable here - vanilla's own clear/destroy logic expects to free
        /// vanilla-game-heap-allocated children, while every builder in this crate (including this test
        /// module's own, below) is Rust-`Box`/`Vec`-allocated; calling `.original()` on a Rust-allocated
        /// tree would be undefined behavior. As a descendant of `destruction`, this module reaches every
        /// `pub(super)` function here, plus every `raw_mem::*` helper (via `destruction`'s own
        /// `use super::{raw_mem::*, *};`), with no visibility changes needed. Deliberately builds its
        /// own small test-only trees (distinct from `reimplementation_tests::live_support`, matching
        /// that module's own documented reason for not sharing code across the opposite-feature-flag
        /// boundary) that populate real non-null `cached_name`/`cached_desc`/`icon_ptr` content, so
        /// these tests actually exercise `free_buffer_string`/`free_owned_cstring`'s non-null path -
        /// `live_support`'s builders never do, since they leave those fields zeroed for live-comparison
        /// safety.
        #[cfg(test)]
        mod tests {
            use super::*;

            fn build_test_program(id: i32) -> *mut ZTResearchProgram {
                Box::into_raw(Box::new(ZTResearchProgram {
                    config_file: BFConfigFile::default(),
                    cached_name: alloc_buffer_string("program name"),
                    cached_desc: alloc_buffer_string("program desc"),
                    desc_id: 0,
                    icon_ptr: alloc_owned_cstring(Some("icon.bmp")),
                    entity_icon_ptr: alloc_owned_cstring(Some("entity_icon.bmp")),
                    id,
                    target_cost: 0.0,
                    current_progress: 0.0,
                    priority: 0,
                    target_id: -1,
                    effect_kind_raw: -1,
                    effect_param_0: 0,
                    effect_param_1: -1,
                    effect_param_2: 0,
                    help_id: 0,
                }))
            }

            fn build_test_category(id: i32, program_count: usize) -> *mut ZTResearchCategory {
                let programs: Vec<u32> = (0..program_count).map(|i| build_test_program(i as i32) as u32).collect();
                Box::into_raw(Box::new(ZTResearchCategory {
                    config_file: BFConfigFile::default(),
                    id,
                    cached_name: alloc_buffer_string("category name"),
                    cached_desc: alloc_buffer_string("category desc"),
                    icon_ptr: alloc_owned_cstring(Some("category_icon.bmp")),
                    help_id: 0,
                    expansion_id: 0,
                    enabled: 1,
                    pad2: [0; 3],
                    program_array: ptr_array_from_vec(programs),
                }))
            }

            fn build_test_branch(id: i32, category_count: usize, funding_level_count: usize) -> *mut ZTResearchBranch {
                let categories: Vec<u32> = (0..category_count).map(|i| build_test_category(i as i32, 0) as u32).collect();
                let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: 0.0, cost: 0.0 }; funding_level_count];
                let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
                Box::into_raw(Box::new(ZTResearchBranch {
                    config_file: BFConfigFile::default(),
                    id,
                    cached_name: alloc_buffer_string("branch name"),
                    cached_desc: alloc_buffer_string("branch desc"),
                    icon_ptr: alloc_owned_cstring(Some("branch_icon.bmp")),
                    noprogicon_ptr: alloc_owned_cstring(Some("noprogicon.bmp")),
                    current_category_ptr: 0,
                    current_program_ptr: 0,
                    category_array: ptr_array_from_vec(categories),
                    current_funding_level: 0,
                    funding_table_start,
                    funding_table_end,
                    funding_table_capacity,
                }))
            }

            #[test]
            fn reset_category_contents_clears_and_reallocates_in_place() {
                let category_ptr = build_test_category(42, 2);
                let category = unsafe { &mut *category_ptr };
                let original_program_array_capacity = category.program_array.capacity();
                assert!(original_program_array_capacity > 0);

                reset_category_contents(category);

                assert_eq!(category.id, -1);
                let (name_start, _, _) = category.cached_name.raw_parts();
                assert_ne!(name_start, 0, "cached_name must be re-allocated, not left null");
                assert_eq!(category.cached_name.copy_to_string(), "");
                let (desc_start, _, _) = category.cached_desc.raw_parts();
                assert_ne!(desc_start, 0, "cached_desc must be re-allocated, not left null");
                assert_eq!(category.cached_desc.copy_to_string(), "");
                assert_eq!(category.enabled, 1);
                assert_eq!(category.icon_ptr, 0);
                assert_eq!(category.program_array.len(), 0);
                assert_eq!(category.program_array.capacity(), original_program_array_capacity);

                // Manual teardown - `reset_category_contents` only resets in place, it doesn't free
                // the category object or its own remaining buffers.
                free_ptr_array(&category.program_array);
                free_buffer_string(&category.cached_desc);
                free_buffer_string(&category.cached_name);
                drop(unsafe { Box::from_raw(category_ptr) });
            }

            #[test]
            fn reset_branch_contents_clears_and_reallocates_in_place() {
                let branch_ptr = build_test_branch(7, 2, 3);
                let branch = unsafe { &mut *branch_ptr };
                branch.current_category_ptr = 0x1234; // arbitrary non-zero sentinel, cleared unconditionally
                branch.current_program_ptr = 0x5678;
                let original_category_array_capacity = branch.category_array.capacity();
                let original_funding_table_capacity = branch.funding_table_capacity;
                assert!(original_category_array_capacity > 0);

                reset_branch_contents(branch);

                assert_eq!(branch.id, -1);
                let (name_start, _, _) = branch.cached_name.raw_parts();
                assert_ne!(name_start, 0, "cached_name must be re-allocated, not left null");
                assert_eq!(branch.cached_name.copy_to_string(), "");
                let (desc_start, _, _) = branch.cached_desc.raw_parts();
                assert_ne!(desc_start, 0, "cached_desc must be re-allocated, not left null");
                assert_eq!(branch.cached_desc.copy_to_string(), "");
                assert_eq!(branch.icon_ptr, 0);
                assert_eq!(branch.noprogicon_ptr, 0);
                assert_eq!(branch.current_category_ptr, 0);
                assert_eq!(branch.current_program_ptr, 0);
                assert_eq!(branch.category_array.len(), 0);
                assert_eq!(branch.category_array.capacity(), original_category_array_capacity);
                assert_eq!(branch.current_funding_level, 0);
                assert_eq!(branch.funding_table_end, branch.funding_table_start);
                assert_eq!(branch.funding_table_capacity, original_funding_table_capacity, "funding table capacity must be retained, not freed");

                // Manual teardown - `reset_branch_contents` only resets in place, it doesn't free the
                // branch object or its own remaining buffers.
                free_funding_table(branch);
                free_ptr_array(&branch.category_array);
                free_buffer_string(&branch.cached_desc);
                free_buffer_string(&branch.cached_name);
                drop(unsafe { Box::from_raw(branch_ptr) });
            }

            #[test]
            fn destroy_program_does_not_panic() {
                let program_ptr = build_test_program(1);
                unsafe { destroy_program(program_ptr) };
            }

            #[test]
            fn destroy_category_does_not_panic() {
                let category_ptr = build_test_category(1, 2);
                unsafe { destroy_category(category_ptr) };
            }

            #[test]
            fn destroy_branch_does_not_panic() {
                let branch_ptr = build_test_branch(1, 2, 2);
                unsafe { destroy_branch(branch_ptr) };
            }

            #[test]
            fn clear_branches_empties_array_and_resets_elapsed_ticks() {
                let branches: Vec<u32> = (0..2).map(|i| build_test_branch(i, 1, 1) as u32).collect();
                let mut mgr = ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 123, branch_array: ptr_array_from_vec(branches) };
                let original_branch_array_capacity = mgr.branch_array.capacity();
                assert!(original_branch_array_capacity > 0);

                clear_branches(&mut mgr);

                assert_eq!(mgr.elapsed_ticks(), 0);
                assert_eq!(mgr.branch_array.len(), 0);
                assert_eq!(mgr.branch_array.capacity(), original_branch_array_capacity);

                free_ptr_array(&mgr.branch_array);
            }
        }
    }

    /// Shadow-mode arm: only `loadBranches` observes/compares; the six lifecycle functions are plain
    /// passthroughs to vanilla (there's nothing to shadow-test on the destruction side - see the module
    /// doc comment above).
    #[cfg(feature = "vanilla-research-config")]
    #[detour_mod]
    mod detours {
        use std::{collections::HashSet, ffi::CStr};

        use openzt_detour::generated::{
            ztresearchbranch::{CLEAR_BRANCH, ZTRESEARCH_BRANCH},
            ztresearchcategory::{CLEAR_CATEGORY, ZTRESEARCH_CATEGORY},
            ztresearchmgr::{CLEAR_BRANCHES, LOAD_BRANCHES},
            ztresearchprogram::ZTRESEARCH_PROGRAM,
        };

        use super::*;
        use crate::util::ref_from_memory;

        #[detour(LOAD_BRANCHES)]
        unsafe extern "thiscall" fn load_branches(this: *const u32, manifest_path: *const i8) -> u32 {
            let manifest_path_str = unsafe { CStr::from_ptr(manifest_path) }.to_string_lossy().into_owned();

            // Read the manifest ourselves before calling the original, so we know which branch files
            // (and, by peeking each one's own declared id, which existing-vs-new outcome) to expect -
            // independent of vanilla, matching this module's usual "compute a prediction, then check it
            // against what actually happened" shape.
            let (ids_before, entries) = match super::read_cfg(&manifest_path_str) {
                Some(manifest_ini) => {
                    let mgr_before = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
                    let ids_before: HashSet<i32> = mgr_before.branches().map(|b| b.id()).collect();
                    let entries = super::manifest_entries(&manifest_ini, mgr_before);
                    (Some(ids_before), Some(entries))
                }
                None => {
                    error!("research-config-reimplementation: failed to independently parse manifest '{manifest_path_str}'");
                    (None, None)
                }
            };

            let result = unsafe { LOAD_BRANCHES_DETOUR.call(this, manifest_path) };
            debug!("research-config-reimplementation: ZTResearchMgr::loadBranches(\"{manifest_path_str}\") called, this={:#x}, vanilla result: {result}", this as u32);

            if result == 0 {
                return result;
            }
            let (Some(ids_before), Some(entries)) = (ids_before, entries) else {
                return result;
            };

            let mgr_after = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
            let mut mismatches = Vec::new();
            super::compare_load_branches(&manifest_path_str, &ids_before, mgr_after, &entries, &mut mismatches);
            if mismatches.is_empty() {
                if !entries.is_empty() {
                    info!(
                        "research-config-reimplementation: '{manifest_path_str}' matches reimplementation ({} branches in manifest)",
                        entries.len()
                    );
                } else {
                    debug!("research-config-reimplementation: '{manifest_path_str}' matches reimplementation (0 branches in manifest)");
                }
            } else {
                for mismatch in &mismatches {
                    error!("research-config-reimplementation mismatch: {mismatch}");
                }
            }

            result
        }

        #[detour(CLEAR_BRANCHES)]
        unsafe extern "thiscall" fn clear_branches(this: *const u32) {
            unsafe { CLEAR_BRANCHES_DETOUR.call(this) }
        }

        #[detour(CLEAR_BRANCH)]
        unsafe extern "thiscall" fn clear_branch(this: *const u32) {
            unsafe { CLEAR_BRANCH_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCH_BRANCH)]
        unsafe extern "thiscall" fn ztresearch_branch_dtor(this: *const u32) {
            unsafe { ZTRESEARCH_BRANCH_DETOUR.call(this) }
        }

        #[detour(CLEAR_CATEGORY)]
        unsafe extern "thiscall" fn clear_category(this: *const u32) {
            unsafe { CLEAR_CATEGORY_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCH_CATEGORY)]
        unsafe extern "thiscall" fn ztresearch_category_dtor(this: *const u32) {
            unsafe { ZTRESEARCH_CATEGORY_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCH_PROGRAM)]
        unsafe extern "thiscall" fn ztresearch_program_dtor(this: *const u32) {
            unsafe { ZTRESEARCH_PROGRAM_DETOUR.call(this) }
        }
    }

    /// Default arm: full construction/destruction replacement - see the module doc comment above.
    #[cfg(not(feature = "vanilla-research-config"))]
    #[detour_mod]
    mod detours {
        use std::{ffi::CStr, panic::AssertUnwindSafe};

        use openzt_detour::generated::{
            ztresearchbranch::{CLEAR_BRANCH, ZTRESEARCH_BRANCH},
            ztresearchcategory::{CLEAR_CATEGORY, ZTRESEARCH_CATEGORY},
            ztresearchmgr::{CLEAR_BRANCHES, LOAD_BRANCHES},
            ztresearchprogram::ZTRESEARCH_PROGRAM,
        };

        use super::*;
        use crate::util::mut_from_memory;

        #[detour(LOAD_BRANCHES)]
        unsafe extern "thiscall" fn load_branches(this: *const u32, manifest_path: *const i8) -> u32 {
            let manifest_path_str = unsafe { CStr::from_ptr(manifest_path) }.to_string_lossy().into_owned();
            debug!("research-config-reimplementation: loadBranches(\"{manifest_path_str}\") called, this={:#x}", this as u32);

            // Parsing touches no `ZTResearchMgr` state at all, so a parse failure or panic here can
            // never leave `mgr` partially mutated - always safe to fall back to vanilla.
            let parsed = match std::panic::catch_unwind(AssertUnwindSafe(|| super::construction::parse_manifest(&manifest_path_str))) {
                Ok(Some(parsed)) => parsed,
                Ok(None) | Err(_) => {
                    error!("research-config-reimplementation: failed to parse '{manifest_path_str}', falling back to vanilla");
                    return unsafe { LOAD_BRANCHES_DETOUR.call(this, manifest_path) };
                }
            };

            // Applying mutates `mgr` directly - once it's started, `mgr` may already contain
            // partially-constructed Rust-allocated branches, so a panic here must NOT fall back to
            // vanilla (see `apply_all`'s doc comment): that would mean vanilla re-processing the same
            // manifest against a `mgr` it doesn't know is already half-updated.
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            let branch_count = parsed.len();
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::construction::apply_all(mgr, &parsed))).is_err() {
                error!("research-config-reimplementation: panic while applying '{manifest_path_str}'; branch_array may be partially updated, not falling back");
            } else if branch_count > 0 {
                info!("research-config-reimplementation: replaced loadBranches(\"{manifest_path_str}\") natively ({branch_count} branches in manifest)");
            } else {
                debug!("research-config-reimplementation: replaced loadBranches(\"{manifest_path_str}\") natively (0 branches in manifest)");
            }
            1
        }

        #[detour(CLEAR_BRANCHES)]
        unsafe extern "thiscall" fn clear_branches(this: *const u32) {
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::clear_branches(mgr))).is_err() {
                error!("research-config-reimplementation: panic while clearing branches; memory may be leaked");
            }
        }

        #[detour(CLEAR_BRANCH)]
        unsafe extern "thiscall" fn clear_branch(this: *const u32) {
            let branch = unsafe { mut_from_memory::<ZTResearchBranch>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::reset_branch_contents(branch))).is_err() {
                error!("research-config-reimplementation: panic while resetting branch contents; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCH_BRANCH)]
        unsafe extern "thiscall" fn ztresearch_branch_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchBranch;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_branch(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying branch; memory may be leaked");
            }
        }

        #[detour(CLEAR_CATEGORY)]
        unsafe extern "thiscall" fn clear_category(this: *const u32) {
            let category = unsafe { mut_from_memory::<ZTResearchCategory>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::reset_category_contents(category))).is_err() {
                error!("research-config-reimplementation: panic while resetting category contents; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCH_CATEGORY)]
        unsafe extern "thiscall" fn ztresearch_category_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchCategory;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_category(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying category; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCH_PROGRAM)]
        unsafe extern "thiscall" fn ztresearch_program_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchProgram;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_program(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying program; memory may be leaked");
            }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise research-config-reimplementation detours: {e:?}");
        }
    }
}

/// Native reimplementation of `ZTResearchMgr::save`/`load`'s save-file persistence format - a small,
/// self-describing stream of `(kind, id, value)` tuples capturing each branch's `current_funding_level`,
/// each category's `enabled` flag, and each program's `current_progress`. Confirmed byte-for-byte from
/// `resources/decompiles/ZTResearchMgr_save.c` and cross-checked against `ZTResearchMgr_load.c`/`.asm`.
///
/// **Promoted to the live path** (see `detours` below): by default `ZTResearchMgr::save`/`load` are
/// detoured to run this module's logic directly against the real save stream (via
/// `standalone::WRITE_BYTES_TO_FILE`/`DEALLOCATE`, the same `fwrite`/`fread`-shaped primitives vanilla
/// itself goes through), rather than calling `.original()`. The `vanilla-research-save` feature keeps
/// the pre-promotion behavior available (no detour installed at all - `ZTResearchMgr::save`/`load`'s
/// `.original()` calls reach genuine vanilla code) for regression comparison, mirroring
/// `research_config_reimplementation`'s `vanilla-research-config` convention.
///
/// `load`'s actual behavior is considerably more than "read the stream and apply it" - see
/// `predict_load`'s doc comment - including two side effects the pure `predict_load` helper
/// deliberately does **not** model, but the live `detours::load` below does perform, natively, using
/// already-promoted machinery: `ZTResearchProgram::on_completion()` (called on any program whose
/// `current_progress` ends up `>= target_cost`, from Phase A's `on_completion`) and
/// `ZTResearchBranch::pick_random_program()` (called on every branch, consuming the game's RNG stream -
/// still a call into the original implementation, see its own doc comment on why). `live_support` below
/// neutralizes `on_completion` for its synthetic programs by fixing `effect_kind_raw` to an
/// always-unset value (see `live_support::build_program`) so the live proptest-vs-`.original()`
/// comparison in `reimplementation_tests` stays side-effect-tolerant.
pub(crate) mod research_save_reimplementation {
    use std::collections::HashMap;

    #[cfg(not(feature = "vanilla-research-save"))]
    use openzt_detour_macro::detour_mod;

    use super::*;

    /// One `(kind, id, value)` tuple from the save stream. `Program`'s value is stored as raw `f32`
    /// bits (not `f32` itself) so records - and the `PartialEq`/`HashMap` machinery used to compare
    /// them - don't have to special-case `NaN != NaN`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum SaveRecord {
        Branch { id: i32, current_funding_level: i32 },
        Category { id: i32, enabled: u8 },
        Program { id: i32, current_progress_bits: u32 },
    }

    /// Walks a manager's `branch_array`/`category_array`/`program_array` in the same nested order
    /// `ZTResearchMgr::save` does (branch, then each of its categories, then each category's
    /// programs), producing one record per branch/category/program.
    pub(crate) fn snapshot_mgr(mgr: &ZTResearchMgr) -> Vec<SaveRecord> {
        let mut records = Vec::new();
        for branch in mgr.branches() {
            records.push(SaveRecord::Branch { id: branch.id, current_funding_level: branch.current_funding_level });
            for category in branch.categories() {
                records.push(SaveRecord::Category { id: category.id, enabled: category.enabled });
                for program in category.programs() {
                    records.push(SaveRecord::Program { id: program.id, current_progress_bits: program.current_progress.to_bits() });
                }
            }
        }
        records
    }

    /// The exact byte stream `ZTResearchMgr::save` writes for `records`: a leading `int32 0` header, one
    /// `(kind, id, value)` tuple per record (`kind` `0`/`1`/`2` for `Branch`/`Category`/`Program`; a
    /// `Category`'s `enabled` is a single byte, everything else is a little-endian `int32`/`float32`),
    /// and a trailing `int32 -1` terminator. No counts are ever written - the stream is fully
    /// self-describing via the `kind` tag.
    pub(crate) fn serialize(records: &[SaveRecord]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + records.len() * 12);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        for record in records {
            match *record {
                SaveRecord::Branch { id, current_funding_level } => {
                    bytes.extend_from_slice(&0i32.to_le_bytes());
                    bytes.extend_from_slice(&id.to_le_bytes());
                    bytes.extend_from_slice(&current_funding_level.to_le_bytes());
                }
                SaveRecord::Category { id, enabled } => {
                    bytes.extend_from_slice(&1i32.to_le_bytes());
                    bytes.extend_from_slice(&id.to_le_bytes());
                    bytes.push(enabled);
                }
                SaveRecord::Program { id, current_progress_bits } => {
                    bytes.extend_from_slice(&2i32.to_le_bytes());
                    bytes.extend_from_slice(&id.to_le_bytes());
                    bytes.extend_from_slice(&current_progress_bits.to_le_bytes());
                }
            }
        }
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        bytes
    }

    fn read_i32(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
        let chunk = bytes.get(*cursor..*cursor + 4)?;
        *cursor += 4;
        Some(i32::from_le_bytes(chunk.try_into().unwrap()))
    }

    /// The inverse of `serialize`. Returns `None` on any malformed stream: truncated (a size/count
    /// doesn't fit), an unrecognized `kind` (anything other than `0`/`1`/`2`/the `-1` terminator), or
    /// trailing bytes left over after the terminator.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Vec<SaveRecord>> {
        let mut cursor = 0usize;
        read_i32(bytes, &mut cursor)?; // header, discarded - matches `load`, which reads but never uses it
        let mut records = Vec::new();
        loop {
            let kind = read_i32(bytes, &mut cursor)?;
            if kind < 0 {
                break;
            }
            let id = read_i32(bytes, &mut cursor)?;
            match kind {
                0 => {
                    let current_funding_level = read_i32(bytes, &mut cursor)?;
                    records.push(SaveRecord::Branch { id, current_funding_level });
                }
                1 => {
                    let enabled = *bytes.get(cursor)?;
                    cursor += 1;
                    records.push(SaveRecord::Category { id, enabled });
                }
                2 => {
                    let current_progress_bits = read_i32(bytes, &mut cursor)? as u32;
                    records.push(SaveRecord::Program { id, current_progress_bits });
                }
                _ => return None,
            }
        }
        (cursor == bytes.len()).then_some(records)
    }

    /// The funding level/enabled flag/current progress `ZTResearchMgr::load` ends up with for every
    /// branch/category/program id it knows about (not just the ones a stream record touched).
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) struct PredictedState {
        pub(crate) funding_levels: HashMap<i32, i32>,
        pub(crate) enabled: HashMap<i32, u8>,
        pub(crate) progress_bits: HashMap<i32, u32>,
    }

    /// The save-game format version at which `ZTResearchMgr::load` starts reading/writing research
    /// data at all - below this, `load` still runs its unconditional reset (and the
    /// `on_completion`/`pick_random_program` tail) but never touches `file`. Shared between
    /// `predict_load` (the pure prediction) and `detours::load` (the live promoted implementation)
    /// below so the threshold can't drift between the two.
    const MIN_VERSION_WITH_RESEARCH_DATA: u32 = 0x28;

    /// Predicts what `ZTResearchMgr::load` leaves in `current_funding_level`/`enabled`/
    /// `current_progress` (excluding the `on_completion`/`pick_random_program` side effects - see the
    /// module doc comment above), given the ids it already knows about and the stream's records.
    ///
    /// Confirmed from `ZTResearchMgr_load.c`/`.asm`: **every** branch/category/program is reset first,
    /// unconditionally (`current_funding_level = 0`, `enabled = 1`, `current_progress = 0.0`),
    /// completely independent of `version` or the stream (this happens before either is even looked
    /// at). Only if `version >= 0x28` is the stream then read and applied on top, record by record, in
    /// order; a record whose `id` doesn't match any known branch/category/program is silently skipped
    /// (mirrors `getBranch`/`getCategory`/`getProgram` returning null). A `Branch` record's value is
    /// clamped to `0` if it's `>=` that branch's own funding-level count, using an **unsigned**
    /// comparison (so a negative saved value is always clamped, per the decompiled
    /// `-(uint)(value < count) & value`).
    pub(crate) fn predict_load(
        branch_ids: &[i32],
        category_ids: &[i32],
        program_ids: &[i32],
        funding_level_counts: &HashMap<i32, usize>,
        records: &[SaveRecord],
        version: u32,
    ) -> PredictedState {
        let mut funding_levels: HashMap<i32, i32> = branch_ids.iter().map(|&id| (id, 0)).collect();
        let mut enabled: HashMap<i32, u8> = category_ids.iter().map(|&id| (id, 1)).collect();
        let mut progress_bits: HashMap<i32, u32> = program_ids.iter().map(|&id| (id, 0.0f32.to_bits())).collect();

        if version >= MIN_VERSION_WITH_RESEARCH_DATA {
            for record in records {
                match *record {
                    SaveRecord::Branch { id, current_funding_level } => {
                        if let Some(slot) = funding_levels.get_mut(&id) {
                            let count = funding_level_counts.get(&id).copied().unwrap_or(0) as u32;
                            *slot = if (current_funding_level as u32) < count { current_funding_level } else { 0 };
                        }
                    }
                    SaveRecord::Category { id, enabled: value } => {
                        if let Some(slot) = enabled.get_mut(&id) {
                            *slot = value;
                        }
                    }
                    SaveRecord::Program { id, current_progress_bits } => {
                        if let Some(slot) = progress_bits.get_mut(&id) {
                            *slot = current_progress_bits;
                        }
                    }
                }
            }
        }

        PredictedState { funding_levels, enabled, progress_bits }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn serialize_parse_round_trip() {
            let records = vec![
                SaveRecord::Branch { id: 100, current_funding_level: 2 },
                SaveRecord::Category { id: 200, enabled: 1 },
                SaveRecord::Program { id: 300, current_progress_bits: 12.5f32.to_bits() },
                SaveRecord::Branch { id: -5, current_funding_level: -1 },
                SaveRecord::Category { id: i32::MAX, enabled: 0 },
                SaveRecord::Program { id: i32::MIN, current_progress_bits: f32::NAN.to_bits() },
            ];
            let bytes = serialize(&records);
            assert_eq!(parse(&bytes), Some(records));
        }

        #[test]
        fn empty_record_list_round_trips() {
            let bytes = serialize(&[]);
            assert_eq!(bytes.len(), 8); // header + terminator only
            assert_eq!(parse(&bytes), Some(vec![]));
        }

        #[test]
        fn parse_rejects_truncated_stream() {
            let mut bytes = serialize(&[SaveRecord::Branch { id: 1, current_funding_level: 2 }]);
            bytes.pop();
            assert_eq!(parse(&bytes), None);
        }

        #[test]
        fn parse_rejects_unknown_kind() {
            let mut bytes = 0i32.to_le_bytes().to_vec();
            bytes.extend_from_slice(&3i32.to_le_bytes()); // no such kind
            bytes.extend_from_slice(&0i32.to_le_bytes());
            assert_eq!(parse(&bytes), None);
        }

        #[test]
        fn parse_rejects_trailing_garbage() {
            let mut bytes = serialize(&[]);
            bytes.push(0xff);
            assert_eq!(parse(&bytes), None);
        }

        #[test]
        fn predict_load_resets_untouched_ids_and_applies_matching_records() {
            let funding_level_counts = HashMap::from([(1, 3usize)]);
            let records = vec![
                SaveRecord::Branch { id: 1, current_funding_level: 2 },
                SaveRecord::Branch { id: 1, current_funding_level: 99 }, // out of range: clamps to 0, and overrides the record above
                SaveRecord::Category { id: 10, enabled: 0 },
                SaveRecord::Program { id: 100, current_progress_bits: 5.0f32.to_bits() },
                SaveRecord::Branch { id: 999, current_funding_level: 1 }, // no matching branch: ignored
            ];
            let predicted = predict_load(&[1, 2], &[10, 20], &[100], &funding_level_counts, &records, 0x28);
            assert_eq!(predicted.funding_levels, HashMap::from([(1, 0), (2, 0)]));
            assert_eq!(predicted.enabled, HashMap::from([(10, 0), (20, 1)]));
            assert_eq!(predicted.progress_bits, HashMap::from([(100, 5.0f32.to_bits())]));
        }

        #[test]
        fn predict_load_below_version_threshold_only_resets() {
            let funding_level_counts = HashMap::from([(1, 3usize)]);
            let records = vec![SaveRecord::Branch { id: 1, current_funding_level: 2 }];
            let predicted = predict_load(&[1], &[], &[], &funding_level_counts, &records, 0x27);
            assert_eq!(predicted.funding_levels, HashMap::from([(1, 0)]));
        }
    }

    /// Live-stream read primitives `detours::load` uses to walk `file` record by record - the read
    /// counterpart to `standalone::WRITE_BYTES_TO_FILE`, which `detours::save` calls directly since a
    /// single whole-buffer write needs no incremental helper. Mirrors
    /// `reimplementation_tests::io_redirect`'s redirect target exactly (`standalone::DEALLOCATE`, the
    /// `fread`-shaped primitive - the name is a decompiler artifact, not descriptive) so the two stay
    /// interchangeable: in a `reimplementation-tests` build with a capture/replay window active,
    /// `DEALLOCATE.original()` calls here transparently hit `io_redirect`'s in-memory buffer instead of
    /// a real file, exactly like every other call to that address.
    #[cfg(not(feature = "vanilla-research-save"))]
    mod stream_io {
        use openzt_detour::generated::standalone::DEALLOCATE;

        use crate::util::get_from_memory;

        /// CRT `FILE`-shaped EOF flag: bit `0x20` of the `_flag` word at offset `0xc`, dereferenced
        /// directly from `file` exactly like `ZTResearchMgr_load.c`/`.asm` does. Checked before every
        /// record read, as a defensive backstop alongside the stream's own `-1` terminator, in case a
        /// stream (e.g. an old/foreign save) ends without ever writing one.
        pub(super) fn is_eof(file: *const u32) -> bool {
            get_from_memory::<u32>((file as u32) + 0xc) & 0x20 != 0
        }

        pub(super) fn read_i32(file: *const u32) -> Option<i32> {
            let mut buf = 0i32;
            let ok = unsafe { DEALLOCATE.original()(&mut buf as *mut i32 as *const u32, 4, 1, file as *const u8) };
            (ok == 1).then_some(buf)
        }

        pub(super) fn read_u8(file: *const u32) -> Option<u8> {
            let mut buf = 0u8;
            let ok = unsafe { DEALLOCATE.original()(&mut buf as *mut u8 as *const u32, 1, 1, file as *const u8) };
            (ok == 1).then_some(buf)
        }
    }

    /// Detours `ZTResearchMgr::save`/`load` onto this module's native reimplementation - the default,
    /// promoted arm (see the module doc comment above). `save` computes its whole byte buffer purely
    /// from already-owned `ZTResearchMgr` state before writing anything, so there's nothing to roll
    /// back if that computation ever panicked; `load` starts mutating `this` (via
    /// `ZTResearchProgram::reset()`) as its very first step, matching vanilla's own unconditional
    /// reset, so - like `research_config_reimplementation`'s `apply_all` arm - there is no safe
    /// fallback to vanilla once that begins.
    #[cfg(not(feature = "vanilla-research-save"))]
    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztresearchmgr::{LOAD, SAVE};
        use tracing::{error, warn};

        use super::{stream_io, *};
        use crate::util::{mut_from_memory, ref_from_memory};

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> u8 {
            let mgr = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
            let bytes = serialize(&snapshot_mgr(mgr));

            let ok = unsafe { standalone::WRITE_BYTES_TO_FILE.original()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) };
            if !ok {
                error!("research-save-reimplementation: WriteBytesToFile failed writing {} research bytes", bytes.len());
            }
            ok as u8
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };

            // Unconditional reset, regardless of `version` or what (if anything) `file` holds - matches
            // `ZTResearchMgr_load.c`: every branch's `current_funding_level` to `0`, every category's
            // `enabled` to `1`, and `ZTResearchProgram::reset()` (not just zeroing `current_progress`)
            // on every program.
            for branch in mgr.branches_mut() {
                branch.current_funding_level = 0;
                for category in branch.categories_mut() {
                    category.set_enabled(true);
                    for program in category.programs_mut() {
                        program.reset();
                    }
                }
            }

            let mut read_ok = true;
            if version >= MIN_VERSION_WITH_RESEARCH_DATA {
                read_ok = stream_io::read_i32(file).is_some(); // header, discarded - matches parse()/predict_load
                while read_ok && !stream_io::is_eof(file) {
                    let Some(kind) = stream_io::read_i32(file) else {
                        read_ok = false;
                        break;
                    };
                    if kind < 0 {
                        break; // terminator: stop reading, fall through to the tail below
                    }
                    let Some(id) = stream_io::read_i32(file) else {
                        read_ok = false;
                        break;
                    };
                    if kind > 2 {
                        read_ok = false; // unrecognized kind: matches parse() rejecting it, but here it
                        break; //          also aborts the whole load, same as a genuine read failure
                    }
                    match kind {
                        0 => {
                            let Some(value) = stream_io::read_i32(file) else {
                                read_ok = false;
                                break;
                            };
                            if let Some(branch) = mgr.get_branch_mut(id) {
                                let count = branch.funding_level_count() as u32;
                                branch.current_funding_level = if (value as u32) < count { value } else { 0 };
                            }
                        }
                        1 => {
                            let Some(value) = stream_io::read_u8(file) else {
                                read_ok = false;
                                break;
                            };
                            if let Some(category) = mgr.get_category_mut(id) {
                                category.set_enabled(value != 0);
                            }
                        }
                        2 => {
                            let Some(value) = stream_io::read_i32(file) else {
                                read_ok = false;
                                break;
                            };
                            if let Some(program) = mgr.get_program_mut(id) {
                                program.current_progress = f32::from_bits(value as u32);
                            }
                        }
                        _ => unreachable!("kind already range-checked above"),
                    }
                }
            }

            if !read_ok {
                warn!("research-save-reimplementation: ZTResearchMgr::load stream read failed (version {version}); aborting without the on_completion/pick_random_program tail, matching vanilla");
                return false;
            }

            // Tail: matches `ZTResearchMgr_load.c` - runs regardless of `version`/whether any records
            // were actually read, using the already-native `on_completion` from Phase A.
            for program in mgr.branches_mut().flat_map(|b| b.categories_mut()).flat_map(|c| c.programs_mut()) {
                if program.is_complete() {
                    program.on_completion();
                }
            }
            for branch in mgr.branches_mut() {
                branch.pick_random_program();
            }

            true
        }
    }

    /// Installs the `save`/`load` detour (the `not(vanilla-research-save)` default arm above). Under
    /// `vanilla-research-save`, deliberately installs nothing at all: `ZTResearchMgr::save`/`load`'s
    /// `.original()` calls (see `ztresearch::ZTResearchMgr::save`/`load`) then reach genuine,
    /// untouched vanilla code, keeping the live proptest-vs-`.original()` comparison in
    /// `reimplementation_tests` meaningful - unlike `research_config_reimplementation`'s shadow arm,
    /// there's no always-on production comparison/logging to install here, since that live-comparison
    /// battery already covers it.
    #[cfg(not(feature = "vanilla-research-save"))]
    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise research-save-reimplementation detours: {e:?}");
        }
    }

    #[cfg(feature = "vanilla-research-save")]
    pub fn init() {}

    /// Synthetic `ZTResearchBranch`/`ZTResearchCategory`/`ZTResearchProgram` construction/teardown for
    /// the live `reimplementation_tests` comparison harness. Deliberately **not** shared with
    /// `research_config_reimplementation::construction`/`raw_mem` above - that module is compiled only
    /// under `not(vanilla-research-config)`, the opposite of this file's usual feature gate, and reusing
    /// it here would tie this test-only code to an unrelated feature flag. Every allocation here goes
    /// through Rust's own allocator, mirroring that module's own rationale for doing the same.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod live_support {
        use super::*;

        /// A program to splice into a synthetic category. `effect_kind_raw` is caller-controlled -
        /// most call sites still pin it to `-1` (unset), which keeps `ZTResearchProgram::on_completion()`
        /// (triggered by `load` whenever `current_progress` ends up `>= target_cost`) a guaranteed
        /// no-op regardless of what values a test case generates for `current_progress`/`target_cost`,
        /// instead of risking a dispatch into `setAvail`/`setBuildingUpgrade`/etc. with ids that don't
        /// correspond to any real entity. `ZTRESEARCHMGR_LOAD`'s own tree-building deliberately varies
        /// this field instead, reusing `build_standalone_program`'s already-proven-safe sentinel field
        /// values (`target_id`/`effect_param_0..2`) for every kind, so `on_completion`'s dispatch is
        /// exercised for real rather than staying a guaranteed no-op.
        pub(crate) struct GeneratedProgram {
            pub(crate) id: i32,
            pub(crate) target_cost: f32,
            pub(crate) current_progress: f32,
            pub(crate) effect_kind_raw: i32,
        }

        /// A category to splice into a synthetic branch. `expansion_id` is always fixed to `0` (the
        /// common "no specific expansion" default real `.cfg` categories use) rather than generated,
        /// since `pick_random_program` - called on every branch as part of `load`'s tail, see the
        /// module doc comment above - passes it to `ZTUI::expansionselect::isExpansionDisabled`, and an
        /// arbitrary generated id risks indexing that lookup out of bounds.
        pub(crate) struct GeneratedCategory {
            pub(crate) id: i32,
            pub(crate) enabled: u8,
            pub(crate) programs: Vec<GeneratedProgram>,
        }

        /// A branch to splice into `ZTResearchMgr::branch_array`. `funding_level_count` controls how
        /// many (dummy) entries the branch's funding table has, to exercise `predict_load`'s clamp rule;
        /// the entries' own content is never read by `save`/`load`, only the table's size.
        pub(crate) struct GeneratedBranch {
            pub(crate) id: i32,
            pub(crate) current_funding_level: i32,
            pub(crate) funding_level_count: usize,
            pub(crate) categories: Vec<GeneratedCategory>,
        }

        fn ptr_array_from_vec<T>(mut vec: Vec<u32>) -> ZTArray<T> {
            if vec.is_empty() {
                return ZTArray::from_raw_parts(0, 0, 0);
            }
            let ptr = vec.as_mut_ptr() as u32;
            let len = vec.len() as u32;
            let cap = vec.capacity() as u32;
            std::mem::forget(vec);
            ZTArray::from_raw_parts(ptr, ptr + len * 4, ptr + cap * 4)
        }

        fn vec_from_ptr_array<T>(array: &ZTArray<T>) -> Vec<u32> {
            let (start, end, buffer_end) = array.raw_parts();
            if start == 0 {
                return Vec::new();
            }
            let len = ((end - start) / 4) as usize;
            let cap = ((buffer_end - start) / 4) as usize;
            unsafe { Vec::from_raw_parts(start as *mut u32, len, cap) }
        }

        fn funding_table_from_vec(mut vec: Vec<ZTResearchFundingLevel>) -> (u32, u32, u32) {
            if vec.is_empty() {
                return (0, 0, 0);
            }
            let stride = size_of::<ZTResearchFundingLevel>() as u32;
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
            let stride = size_of::<ZTResearchFundingLevel>() as u32;
            let cap = ((capacity_end - start) / stride) as usize;
            drop(unsafe { Vec::<ZTResearchFundingLevel>::from_raw_parts(start as *mut ZTResearchFundingLevel, cap, cap) });
        }

        fn build_program(spec: &GeneratedProgram) -> *mut ZTResearchProgram {
            // Same "no matching live entity" sentinel `build_standalone_program` uses for every
            // `effect_kind_raw` - see that function's own doc comment for why `-1` in particular.
            const NO_MATCHING_ENTITY: i32 = -1;
            let program = Box::new(ZTResearchProgram {
                config_file: BFConfigFile::default(),
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                desc_id: 0,
                icon_ptr: 0,
                entity_icon_ptr: 0,
                id: spec.id,
                target_cost: spec.target_cost,
                current_progress: spec.current_progress,
                priority: 0,
                target_id: NO_MATCHING_ENTITY,
                effect_kind_raw: spec.effect_kind_raw,
                effect_param_0: 0,
                effect_param_1: NO_MATCHING_ENTITY,
                effect_param_2: 0,
                help_id: 0,
            });
            Box::into_raw(program)
        }

        fn destroy_program(ptr: *mut ZTResearchProgram) {
            if ptr.is_null() {
                return;
            }
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Builds a standalone `ZTResearchProgram` for the `ON_COMPLETION`/`RESET` live comparison
        /// test in `reimplementation_tests` - deliberately not wired into any category/branch/mgr,
        /// since `on_completion`/`reset` only ever touch `this`. Every field but `effect_kind_raw` is
        /// fixed to a value that makes every underlying effect call in
        /// `dispatch_on_completion`/`dispatch_reset` a safe no-op: `target_id` (and, for
        /// `EffectDiscount`'s reuse of `effect_param_1` as an entity-type id - see
        /// `ResearchEffects::set_effect_discount`'s underlying `_setEffectDiscount.c`) is set to a
        /// value with no matching live `BFEntityType`, since `setAvail`/`setBuildingUpgrade`/
        /// `set*Characteristic`/`setTrickAvailable`/`setEffectDiscount` all walk `GLOBAL_ZTWorldMgr`'s
        /// entity list looking for a match before touching anything else (see their own decompiles).
        /// Uses `-1`, not an extreme value like `i32::MIN`, for this - `-1` is the well-precedented
        /// "unset"/no-target sentinel real `.cfg`-loaded programs already use throughout this file
        /// (see `target_id`'s own field doc comment), so vanilla's id-lookup path is guaranteed to
        /// already handle it gracefully; an extreme value risks tripping an unrelated edge case (e.g.
        /// an unchecked hash/index derived from the id) in code that was never written to expect one.
        pub(crate) fn build_standalone_program(effect_kind_raw: i32) -> *mut ZTResearchProgram {
            const NO_MATCHING_ENTITY: i32 = -1;
            Box::into_raw(Box::new(ZTResearchProgram {
                config_file: BFConfigFile::default(),
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                desc_id: 0,
                icon_ptr: 0,
                entity_icon_ptr: 0,
                id: 0,
                target_cost: 0.0,
                current_progress: 0.0,
                priority: 0,
                target_id: NO_MATCHING_ENTITY,
                effect_kind_raw,
                effect_param_0: 0,
                effect_param_1: NO_MATCHING_ENTITY,
                effect_param_2: 0,
                help_id: 0,
            }))
        }

        pub(crate) fn destroy_standalone_program(ptr: *mut ZTResearchProgram) {
            if ptr.is_null() {
                return;
            }
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Builds a standalone `ZTResearchBranch` for the `FUNDING_TEXT` live comparison test - not
        /// spliced into any `ZTResearchMgr`'s `branch_array`, since `getFundingText`/`funding_text`
        /// only ever read `this` (no `GLOBAL_ZTResearchMgr` dependency, unlike `load`'s tail). `levels`
        /// becomes the branch's inline funding table verbatim, in order; `rate` is always fixed to
        /// `0.0` since `funding_text` never reads it (only `cost`/`name_id` feed its output).
        pub(crate) fn build_standalone_funding_branch(current_funding_level: i32, levels: &[(i32, f32)]) -> *mut ZTResearchBranch {
            let funding_table = levels.iter().map(|&(name_id, cost)| ZTResearchFundingLevel { name_id, rate: 0.0, cost }).collect();
            let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
            Box::into_raw(Box::new(ZTResearchBranch {
                config_file: BFConfigFile::default(),
                id: 0,
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                icon_ptr: 0,
                noprogicon_ptr: 0,
                current_category_ptr: 0,
                current_program_ptr: 0,
                category_array: ZTArray::from_raw_parts(0, 0, 0),
                current_funding_level,
                funding_table_start,
                funding_table_end,
                funding_table_capacity,
            }))
        }

        pub(crate) fn destroy_standalone_funding_branch(ptr: *mut ZTResearchBranch) {
            if ptr.is_null() {
                return;
            }
            let branch = unsafe { &*ptr };
            free_funding_table(branch.funding_table_start, branch.funding_table_capacity);
            drop(unsafe { Box::from_raw(ptr) });
        }

        fn build_category(spec: &GeneratedCategory) -> *mut ZTResearchCategory {
            let mut programs = Vec::with_capacity(spec.programs.len());
            for program in &spec.programs {
                programs.push(build_program(program) as u32);
            }
            let category = Box::new(ZTResearchCategory {
                config_file: BFConfigFile::default(),
                id: spec.id,
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                icon_ptr: 0,
                help_id: 0,
                expansion_id: 0,
                enabled: spec.enabled,
                pad2: [0; 3],
                program_array: ptr_array_from_vec(programs),
            });
            Box::into_raw(category)
        }

        fn destroy_category(ptr: *mut ZTResearchCategory) {
            if ptr.is_null() {
                return;
            }
            let category = unsafe { &*ptr };
            for program_ptr in vec_from_ptr_array(&category.program_array) {
                destroy_program(program_ptr as *mut ZTResearchProgram);
            }
            drop(unsafe { Box::from_raw(ptr) });
        }

        fn build_branch(spec: &GeneratedBranch) -> *mut ZTResearchBranch {
            let mut categories = Vec::with_capacity(spec.categories.len());
            for category in &spec.categories {
                categories.push(build_category(category) as u32);
            }
            let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: 0.0, cost: 0.0 }; spec.funding_level_count];
            let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
            let branch = Box::new(ZTResearchBranch {
                config_file: BFConfigFile::default(),
                id: spec.id,
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                icon_ptr: 0,
                noprogicon_ptr: 0,
                current_category_ptr: 0,
                current_program_ptr: 0,
                category_array: ptr_array_from_vec(categories),
                current_funding_level: spec.current_funding_level,
                funding_table_start,
                funding_table_end,
                funding_table_capacity,
            });
            Box::into_raw(branch)
        }

        fn destroy_branch(ptr: *mut ZTResearchBranch) {
            if ptr.is_null() {
                return;
            }
            let branch = unsafe { &*ptr };
            for category_ptr in vec_from_ptr_array(&branch.category_array) {
                destroy_category(category_ptr as *mut ZTResearchCategory);
            }
            free_funding_table(branch.funding_table_start, branch.funding_table_capacity);
            drop(unsafe { Box::from_raw(ptr) });
        }

        /// Temporarily replaces `mgr.branch_array` with freshly-built branches from `specs`, runs `f`,
        /// then destroys the synthetic branches and restores `mgr.branch_array` to exactly what it held
        /// before this call (raw parts, not contents - the same save/restore convention
        /// `research_config_reimplementation`'s shadow-mode arm uses for the same field). Safe to call
        /// repeatedly with different `specs`, e.g. once per generated proptest case.
        pub(crate) fn with_synthetic_branches<R>(mgr: &mut ZTResearchMgr, specs: &[GeneratedBranch], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
            let original_branch_array_raw_parts = mgr.branch_array.raw_parts();

            let mut branch_ptrs = Vec::with_capacity(specs.len());
            for spec in specs {
                branch_ptrs.push(build_branch(spec) as u32);
            }
            mgr.branch_array = ptr_array_from_vec(branch_ptrs);

            let result = f(mgr);

            for branch_ptr in vec_from_ptr_array(&mgr.branch_array) {
                destroy_branch(branch_ptr as *mut ZTResearchBranch);
            }
            let (start, end, buffer_end) = original_branch_array_raw_parts;
            mgr.branch_array = ZTArray::from_raw_parts(start, end, buffer_end);

            result
        }

        /// Builds a fully synthetic, standalone `ZTResearchMgr` - **not** the real live singleton at
        /// `globals().ztresearchmgr_ptr()` - runs `f` with `specs` spliced into it via
        /// `with_synthetic_branches`, then frees it. Empirically, the real singleton is still null at
        /// the `LOAD_LANG_DLLS` injection point this harness runs from (confirmed live: calling
        /// `globals().ztresearchmgr_ptr()` there returns a null pointer), so comparison tests can't
        /// depend on it existing yet. `ZTResearchMgr::save` never reads anything but `this`, so it's
        /// always safe to call on a standalone instance built this way; `load` needs the extra care in
        /// `with_global_ztresearchmgr_ptr` below.
        pub(crate) fn with_standalone_mgr<R>(specs: &[GeneratedBranch], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
            let mut mgr = Box::new(ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) });
            with_synthetic_branches(&mut mgr, specs, f)
        }

        /// Address of the fixed global slot `globals().ztresearchmgr_ptr()` reads (`base + 0x239010`,
        /// no further indirection - see `ztresearch::command_debug_research_ptr`, which walks this same
        /// slot for diagnostics). `ZTResearchBranch::pick_random_program` reads this slot directly (as
        /// `GLOBAL_ZTResearchMgr`, confirmed via `ZTResearchBranch_pickRandomProgram.c`/`.asm` and
        /// `ZTApp_exit_override.c`), not `this` - so it's a real, load-bearing dependency independent of
        /// whatever `ZTResearchMgr` a caller is invoking `load`/`pick_random_program` on.
        fn global_slot_address() -> u32 {
            get_module_base("zoo.exe") as u32 + 0x0023_9010
        }

        /// Temporarily points the real `GLOBAL_ZTResearchMgr` slot at `mgr`, runs `f`, then restores
        /// whatever the slot held before this call. Needed because `ZTResearchMgr::load`'s tail
        /// unconditionally calls `ZTResearchBranch::pick_random_program` on every branch, which
        /// dereferences that global directly (see `global_slot_address`'s doc comment) - at this early
        /// injection point the real singleton isn't constructed yet (see `with_standalone_mgr`'s doc
        /// comment), so without this the dereference would read a null/garbage pointer. `save` never
        /// reads this slot, so this wrapper is only needed around `load` calls.
        pub(crate) fn with_global_ztresearchmgr_ptr<R>(mgr: &mut ZTResearchMgr, f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
            let slot = global_slot_address();
            let original = get_from_memory::<u32>(slot);
            save_to_memory(slot, mgr as *mut ZTResearchMgr as u32);

            let result = f(mgr);

            save_to_memory(slot, original);
            result
        }

        /// Builds a standalone branch/category/program/funding-level for the `ZTRESEARCHBRANCH_UPDATE`
        /// live comparison test - one category (enabled, `expansion_id` fixed to `0` like
        /// `GeneratedCategory` elsewhere in this module, for the same "keep `isExpansionDisabled` a safe,
        /// deterministic call" reason) holding one program, wired up as the branch's own
        /// `current_category`/`current_program` (unlike `build_branch` above, which always leaves those
        /// null - `ZTResearchBranch::update` needs both set to do anything), plus a one-entry funding
        /// table at `current_funding_level = 0`. Returns the branch pointer; `destroy_update_test_branch`
        /// frees everything transitively.
        pub(crate) fn build_update_test_branch(target_cost: f32, initial_progress: f32, funding_rate: f32, funding_cost: f32) -> *mut ZTResearchBranch {
            let program_ptr = build_program(&GeneratedProgram { id: 0, target_cost, current_progress: initial_progress, effect_kind_raw: -1 });

            let category_ptr = Box::into_raw(Box::new(ZTResearchCategory {
                config_file: BFConfigFile::default(),
                id: 0,
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                icon_ptr: 0,
                help_id: 0,
                expansion_id: 0,
                enabled: 1,
                pad2: [0; 3],
                program_array: ptr_array_from_vec(vec![program_ptr as u32]),
            }));

            let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: funding_rate, cost: funding_cost }];
            let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);

            Box::into_raw(Box::new(ZTResearchBranch {
                config_file: BFConfigFile::default(),
                id: 0,
                cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
                cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
                icon_ptr: 0,
                noprogicon_ptr: 0,
                current_category_ptr: category_ptr as u32,
                current_program_ptr: program_ptr as u32,
                category_array: ptr_array_from_vec(vec![category_ptr as u32]),
                current_funding_level: 0,
                funding_table_start,
                funding_table_end,
                funding_table_capacity,
            }))
        }

        pub(crate) fn destroy_update_test_branch(ptr: *mut ZTResearchBranch) {
            destroy_branch(ptr);
        }

        /// Splices one `build_update_test_branch` branch into a standalone `ZTResearchMgr`, runs `f`
        /// with `GLOBAL_ZTResearchMgr` pointed at it (`ZTResearchBranch::update` reads the global's own
        /// `always_check_expansion` flag directly, same as `pick_random_program` - see
        /// `with_global_ztresearchmgr_ptr`'s own doc comment), then tears everything down.
        /// `Box::new(ZTResearchMgr {..})` alone (as `with_standalone_mgr` uses) allocates exactly
        /// `ZTResearchMgr`'s own confirmed `0x18` bytes - reading `always_check_expansion`'s flag byte at
        /// offset `0x18` on such an instance reads whatever uninitialized heap byte happens to follow,
        /// which every *other* live test tolerates (it doesn't change their outcome either way - see
        /// `GeneratedCategory`'s own `expansion_id = 0` fixed-value doc comment) but would make
        /// `ZTRESEARCHBRANCH_UPDATE` non-deterministic, since it's the first comparison whose outcome
        /// (whether `isExpansionDisabled` even runs) actually depends on that flag. This wrapper adds an
        /// explicit, zeroed byte right after `ZTResearchMgr`'s own fields so the flag reads a
        /// deterministic `false` instead.
        #[repr(C)]
        struct MgrWithZeroedExpansionFlag {
            mgr: ZTResearchMgr,
            always_check_expansion_flag: u8,
            _pad: [u8; 3],
        }

        pub(crate) fn with_update_test_branch<R>(
            target_cost: f32,
            initial_progress: f32,
            funding_rate: f32,
            funding_cost: f32,
            f: impl FnOnce(&mut ZTResearchMgr) -> R,
        ) -> R {
            let mut wrapper = Box::new(MgrWithZeroedExpansionFlag {
                mgr: ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) },
                always_check_expansion_flag: 0,
                _pad: [0; 3],
            });
            let branch_ptr = build_update_test_branch(target_cost, initial_progress, funding_rate, funding_cost);
            wrapper.mgr.branch_array = ptr_array_from_vec(vec![branch_ptr as u32]);

            let result = with_global_ztresearchmgr_ptr(&mut wrapper.mgr, f);

            destroy_update_test_branch(branch_ptr);
            result
        }

        /// One branch's synthetic state for `build_update_test_branches`/`with_update_test_branches` -
        /// the N-branch generalization of `build_update_test_branch`/`with_update_test_branch` above,
        /// used by `ZTRESEARCHMGR_UPDATE`'s nonzero-branch-count extension to exercise
        /// `ZTResearchMgr::update` actually iterating multiple branches and threading the correct `days`
        /// count to each - something the zero-branch `with_standalone_mgr(&[], ...)` case structurally
        /// can't cover.
        #[derive(Debug)]
        pub(crate) struct UpdateTestBranchSpec {
            pub(crate) target_cost: f32,
            pub(crate) initial_progress: f32,
            pub(crate) funding_rate: f32,
            pub(crate) funding_cost: f32,
        }

        /// Builds one `build_update_test_branch`-shaped branch per `specs` entry, in order.
        /// `destroy_update_test_branch` frees each returned pointer individually.
        fn build_update_test_branches(specs: &[UpdateTestBranchSpec]) -> Vec<*mut ZTResearchBranch> {
            specs
                .iter()
                .map(|spec| build_update_test_branch(spec.target_cost, spec.initial_progress, spec.funding_rate, spec.funding_cost))
                .collect()
        }

        /// Splices one `build_update_test_branch`-shaped branch per `specs` entry into a standalone
        /// `ZTResearchMgr` (via the same `MgrWithZeroedExpansionFlag` wrapper `with_update_test_branch`
        /// uses, for the same deterministic-`always_check_expansion`-flag reason), runs `f` with
        /// `GLOBAL_ZTResearchMgr` pointed at it, then tears everything down.
        pub(crate) fn with_update_test_branches<R>(specs: &[UpdateTestBranchSpec], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
            let mut wrapper = Box::new(MgrWithZeroedExpansionFlag {
                mgr: ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) },
                always_check_expansion_flag: 0,
                _pad: [0; 3],
            });
            let branch_ptrs = build_update_test_branches(specs);
            wrapper.mgr.branch_array = ptr_array_from_vec(branch_ptrs.iter().map(|&ptr| ptr as u32).collect());

            let result = with_global_ztresearchmgr_ptr(&mut wrapper.mgr, f);

            for ptr in branch_ptrs {
                destroy_update_test_branch(ptr);
            }
            result
        }

        /// Temporarily pins the real, live `ZTGameMgr` singleton's budget to `cash`, runs `f`, then
        /// restores whatever it held before this call. Used by the `ZTRESEARCHBRANCH_UPDATE` comparison
        /// so both the real and reimplemented `ZTResearchBranch::update` calls see the exact same
        /// available cash, regardless of what either call's own `subtractCash` side effect (or anything
        /// else running in the live game) did to the real budget in between - deliberately mutates the
        /// real singleton in place rather than constructing/redirecting to a synthetic one, since
        /// `subtractCash` also calls `ZTUI::main::setMoneyText`, which - unlike the narrowly-scoped
        /// vanilla calls the rest of this file's live tests exercise - is a real UI refresh that likely
        /// depends on other parts of `ZTGameMgr`/the wider UI state actually being initialized.
        pub(crate) fn with_ztgamemgr_cash<R>(cash: f32, f: impl FnOnce() -> R) -> R {
            let gamemgr = unsafe { &mut *global_ztgamemgr_ptr() };
            let original = gamemgr.cash();
            gamemgr.set_cash(cash);

            let result = f();

            unsafe { &mut *global_ztgamemgr_ptr() }.set_cash(original);
            result
        }

        /// Exposed for `reimplementation_tests` to null-check `GLOBAL_ZTGameMgr`'s raw slot before
        /// running the `ZTRESEARCHBRANCH_UPDATE` comparison - mirrors the existing
        /// `globals().ztworldmgr_ptr().is_null()` guard `run_on_completion_reset_test_and_exit` already
        /// uses for `GLOBAL_ZTWorldMgr`.
        pub(crate) fn ztgamemgr_ptr_is_null() -> bool {
            global_ztgamemgr_ptr().is_null()
        }
    }
}
