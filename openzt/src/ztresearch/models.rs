use std::{
    ffi::{c_char, CStr, CString},
    fmt,
    mem::size_of,
};

use num_enum::TryFromPrimitive;
use openzt_detour::generated::{
    bfuimgr, standalone, uicontrol, ztresearchbranch, ztresearchcategory, ztresearchprogram, ztui_expansionselect, ztui_zoostatus,
};
use windows::{
    core::{PCSTR, PSTR},
    Win32::Globalization::{GetCurrencyFormatA, CURRENCYFMTA},
};

use super::config::research_config_reimplementation;
use super::mgr::ztresearchmgr::{global_always_check_expansion, global_ztgamemgr_ptr};
use crate::{
    bfconfigfile::BFConfigFile,
    globals::get_module_base,
    string_registry::load_string_by_id,
    util::{get_from_memory, mut_from_memory, ref_from_memory, ZTArray, ZTBufferString, ZTString},
};

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
    pub(crate) name_id: i32, // 0x0 - confirmed: the sub-block's `name` string id (e.g. 23100 = "%s none", 23101 = "%s min", 23102 = "%s normal", 23103 = "%s max")
    pub(crate) rate: f32,    // 0x4 - confirmed: the sub-block's `work` value (e.g. none=0, min=21, normal=30, max=45); used as a divisor by the vanilla days/pct-remaining calculations, must be > 0 for this level to be considered active
    pub(crate) cost: f32,    // 0x8 - confirmed: the sub-block's `cost` value (e.g. none=0, min=400, normal=1000, max=2000)
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
    pub(crate) config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`); `kind_tag` observed to always be 6 for research programs
    pub(crate) cached_name: ZTBufferString, // 0x0c - confirmed: built via `BFApp::buildString` from `id` (`name`) right after it's loaded
    pub(crate) cached_desc: ZTBufferString, // 0x18 - confirmed: built via `BFApp::buildString` from `desc_id` (`desc`) right after it's loaded
    pub(crate) desc_id: i32,                // 0x24 - confirmed: the `.cfg` `desc` string id, resolves via `get_string()`
    pub(crate) icon_ptr: u32,               // 0x28 - confirmed: the `.cfg` `icon` field, a raw C string pointer (only valid when non-null)
    pub(crate) entity_icon_ptr: u32,        // 0x2c - confirmed: the `.cfg` `entityIcon` field, a raw C string pointer; also the value `onCompletion` passes to `setBuildingUpgrade` as the building type name when `effect_kind == BuildingUpgrade`
    pub(crate) id: i32,                     // 0x30 - confirmed: the `.cfg` `name` string id, matched by `ZTResearchMgr::get_program`
    pub(crate) target_cost: f32,            // 0x34 - confirmed: the `.cfg` `cost` field
    pub(crate) current_progress: f32,       // 0x38 - not set by `loadProgram` (starts at 0 from the constructor); funding accumulated so far, complete once current_progress >= target_cost
    pub(crate) priority: u32,               // 0x3c - confirmed: the `.cfg` `order` field; tie-breaker used by `ZTResearchBranch::pick_random_program` when choosing the next not-yet-started program
    pub(crate) target_id: i32,              // 0x40 - confirmed: the `.cfg` `target` field; sentinel -1 when unset
    pub(crate) effect_kind_raw: i32,        // 0x44 - confirmed: the `.cfg` `effect` field; see `ZTResearchEffectKind`; sentinel -1 when unset; dispatches `ZTResearchProgram::on_completion`
    pub(crate) effect_param_0: i32,         // 0x48 - confirmed: the `.cfg` `effectval1` field
    pub(crate) effect_param_1: i32,         // 0x4c - confirmed: the `.cfg` `effectval2` field
    pub(crate) effect_param_2: i32,         // 0x50 - confirmed: the `.cfg` `effectval3` field
    pub(crate) help_id: i32,                // 0x54 - confirmed: the `.cfg` `helpid` field (only set if present; 0 by default from the constructor)
}

/// Abstraction over the underlying manager calls `ZTResearchProgram::on_completion`/`reset` dispatch
/// into (still opaque calls into the original game code, same as everywhere else those aren't
/// independently reimplemented in OpenZT), so `dispatch_on_completion`/`dispatch_reset`'s own
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
/// `private/resources/decompiles/ZTResearchProgram_onCompletion.c`: every valid `effect_kind` calls exactly
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
/// `private/resources/decompiles/ZTResearchProgram_reset.c`/`.asm`. Always zeroes `current_progress` first,
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
/// in vanilla, via `openzt-detour/src/generated.rs`. The `bool` returns here match the real,
/// single-byte C++ `bool` these functions return: only the low byte (`AL`) is the actual result -
/// the upper 3 bytes of `EAX` are undefined leftover garbage, and Rust's `bool` `extern "cdecl"` ABI
/// reads only `AL`, exactly how vanilla's own callers treat the value.
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

    pub fn load_program(&mut self, reader: *const u32) -> bool {
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
    pub(crate) config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`)
    pub(crate) id: i32,                     // 0x0c - confirmed by `ZTResearchCategory::loadCategory` (loaded from the `.cfg` `name` key); matched by `ZTResearchMgr::get_category`; doubles as the display-name string id (`get_string(id)` returns the category's name - this is what the in-game UI mislabels "Program")
    pub(crate) cached_name: ZTBufferString, // 0x10 - confirmed by `loadCategory`: built from `id`/`name` via `BFApp::buildString` right after it's loaded
    pub(crate) cached_desc: ZTBufferString, // 0x1c - confirmed by `loadCategory`: built from the `.cfg` `desc` field the same way (the raw `desc` string id is only ever a local variable in `loadCategory` - unlike `ZTResearchProgram`, it's never stored on the object itself)
    pub(crate) icon_ptr: u32,               // 0x28 - confirmed by `loadCategory`: the `.cfg` `icon` field, a raw C string pointer (only valid when non-null)
    pub(crate) help_id: i32,                // 0x2c - confirmed: matches `.cfg` `helpid=` exactly (tested against `helpid=24213`) and matches `loadCategory`'s `getInt(..., "helpid", ...)` call storing here
    pub(crate) expansion_id: i32,           // 0x30 - confirmed by `loadCategory` (`.cfg` `expansion` key); +1 is passed to `ZTUI::expansionselect::isExpansionDisabled` when picking a program
    pub(crate) enabled: u8,                 // 0x34 - unlocked/available flag; gates `ZTResearchBranch::pick_random_program` and is persisted by `ZTResearchMgr::save`; not set by `loadCategory` itself
    pub(crate) pad2: [u8; 0x38 - 0x35],     // 0x35 - alignment padding
    pub(crate) program_array: ZTArray<ZTResearchProgram>, // 0x38
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
    pub fn load_category(&mut self, reader: *const i32) -> bool {
        unsafe { ztresearchcategory::LOAD_CATEGORY.original()((self as *mut Self) as *const u32, reader) }
    }

    /// Clears this category back to an empty, disabled-id state in place. Which implementation runs
    /// depends on which arm of `research_config_reimplementation` is active: with `--features
    /// vanilla-research-config` this category is still real-vanilla-allocated, so real vanilla's own
    /// `clearCategory` runs (via `call_original_clear_category`, not `.original()` - see that function's
    /// own doc comment for why); by default this category may be `Box`-allocated by this module's own
    /// `loadBranches` reimplementation, so the matching Rust logic
    /// (`destruction::reset_category_contents`) runs instead - calling real vanilla's `clearCategory` on
    /// a `Box`-allocated category would be a cross-allocator hazard.
    #[cfg(feature = "vanilla-research-config")]
    pub fn clear(&mut self) {
        unsafe { research_config_reimplementation::detours::call_original_clear_category((self as *mut Self) as *const u32) }
    }

    /// See the `vanilla-research-config` arm's own doc comment on this method for why the two arms
    /// differ.
    #[cfg(not(feature = "vanilla-research-config"))]
    pub fn clear(&mut self) {
        research_config_reimplementation::destruction::reset_category_contents(self)
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
    pub(crate) config_file: BFConfigFile,   // 0x00 - the inherited `BFConfigFile` base (see `bfconfigfile.rs`)
    pub(crate) id: i32,                     // 0x0c - confirmed by `loadBranch` (`.cfg` `name` key); matched by `ZTResearchMgr::get_branch`; doubles as the display-name string id (`get_string(id)` returns the branch's name, e.g. "Animal Care")
    pub(crate) cached_name: ZTBufferString, // 0x10 - confirmed by `loadBranch`: built from `id`/`name` via `BFApp::buildString` right after it's loaded
    pub(crate) cached_desc: ZTBufferString, // 0x1c - confirmed by `loadBranch`: built from the `.cfg` `desc` field the same way (the raw `desc` string id is never stored on the object itself, same as `ZTResearchCategory`)
    pub(crate) icon_ptr: u32,               // 0x28 - confirmed by `loadBranch`: the `.cfg` `icon` field, a raw C string pointer
    pub(crate) noprogicon_ptr: u32,         // 0x2c - confirmed by `loadBranch`: the `.cfg` `noprogicon` field (the icon shown when no program is being researched), a raw C string pointer
    pub(crate) current_category_ptr: u32,                   // 0x30 - selected category; cleared then reassigned by `pick_random_program`
    pub(crate) current_program_ptr: u32,                    // 0x34 - selected program; read directly by `days_remaining_on_program`/`pct_remaining_on_program`
    pub(crate) category_array: ZTArray<ZTResearchCategory>, // 0x38 - confirmed by `loadBranch` (populated from the `.cfg` `category=` list) and `pick_random_program`
    pub(crate) current_funding_level: i32,                  // 0x44 - index into the funding table, clamped by `increase_funding`/`decrease_funding`; reset to 0 by `loadBranch`
    pub(crate) funding_table_start: u32,                    // 0x48 - confirmed by `loadBranch`: inline `ZTResearchFundingLevel` table (stride 0xc, populated from the `.cfg` `funding=` list), *not* a `ZTArray` of pointers
    pub(crate) funding_table_end: u32,                      // 0x4c - confirmed by `loadBranch`
    pub(crate) funding_table_capacity: u32,                  // 0x50 - confirmed by `loadBranch` (checked to decide whether the table needs to grow); unused by us
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
    pub(crate) fn current_program_mut(&self) -> Option<&'static mut ZTResearchProgram> {
        (self.current_program_ptr != 0).then(|| unsafe { mut_from_memory(self.current_program_ptr) })
    }

    pub fn current_funding_level(&self) -> i32 {
        self.current_funding_level
    }

    pub(crate) fn funding_level_count(&self) -> usize {
        ((self.funding_table_end - self.funding_table_start) as usize) / size_of::<ZTResearchFundingLevel>()
    }

    pub(crate) fn funding_level(&self, index: usize) -> ZTResearchFundingLevel {
        get_from_memory(self.funding_table_start + (index * size_of::<ZTResearchFundingLevel>()) as u32)
    }

    /// Every entry in the funding-level table.
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
    /// `current_funding_level == -1` doesn't diverge (`-1 + 1 == 0`, non-negative either way). A naive
    /// signed `<` would disagree with vanilla for `current_funding_level <= -2` (confirmed live via
    /// `ZTRESEARCHBRANCH_FUNDING`).
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
    /// The float-to-int conversion is neither round-to-nearest (despite Ghidra's decompile naming its
    /// helper `ROUND()`) nor Rust's plain saturating `as i32` cast (both confirmed live via
    /// `ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`):
    ///
    /// - It **truncates toward zero**: right before `FISTP`,
    ///   `private/resources/decompiles/ZTResearchBranch_pctRemainingOnProgram.asm` does
    ///   `FSTCW`/`OR AH,0xc`/`FLDCW` to force the x87 rounding-control field to round-toward-zero for
    ///   just that one instruction, then restores it - the classic MSVC codegen for a plain C
    ///   `(int)x` cast, which `f32::trunc()` below reproduces directly.
    /// - For a value that doesn't fit a 64-bit integer (NaN, ±Infinity, or a magnitude beyond
    ///   `i64`'s range), x87's masked-invalid-operation behavior makes `FISTP` store the "integer
    ///   indefinite" pattern `0x8000_0000_0000_0000` (`FISTP` only has a 64-bit integer store form,
    ///   so the real return value is that pattern's low dword): `0` - not the `i32::MIN`/`i32::MAX`
    ///   a saturating cast would produce for -Infinity/+Infinity.
    pub fn pct_remaining_on_program(&self) -> Option<i32> {
        let program = self.current_program()?;
        let rate = self.current_funding_rate()?;
        if rate <= 0.0 {
            return None;
        }
        // The chain runs in `f64` to approximate the x87 FPU's own 80-bit extended-precision
        // intermediates: vanilla never rounds the intermediate product down to `f32` before dividing,
        // and that extra per-op `f32` rounding flips the truncated integer at the boundary -
        // live-reproduced for `target_cost=-0.7881632, current_progress=0.0` (mathematically exactly
        // `100`, but strict per-op `f32` gives `99.99999237`, truncating to `99`; `f64` reproduces
        // vanilla's `100`).
        let target_cost = program.target_cost as f64;
        let current_progress = program.current_progress as f64;
        let raw = (((target_cost - current_progress) * 100.0) / target_cost).trunc();
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
    /// state-transition core, per `private/resources/decompiles/ZTResearchBranch_update.c`/`.asm` (the `.asm`
    /// was needed to get the real byte offsets right - see `ZTResearchMgr::always_check_expansion`'s
    /// doc comment for one place the `.c` decompile's own pointer-arithmetic scaling was actively
    /// misleading). Applies `days` in-game days of progress/cost to the currently selected program (see
    /// `predict_branch_progress`), spending cash via `ZTGameMgr::spend_research`/`subtract_cash` (in
    /// that order, matching vanilla) when affordable, then, on completion, dispatches `on_completion()`
    /// (native) and picks the next program via `pick_random_program()` (still a call into
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
            global_always_check_expansion() || unsafe { ztui_expansionselect::GET_ANY_EXPANSIONS_DISABLED.original()() };

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
    pub fn load_branch(&mut self, path: *const i8) -> bool {
        unsafe { ztresearchbranch::LOAD_BRANCH.original()((self as *mut Self) as *const u32, path) }
    }

    /// Resets `id` to `-1`, empties the cached name/desc buffers, zeroes
    /// `icon`/`noprogicon`/`current_category`/`current_program`, destroys and frees every category, and
    /// resets `category_array`/the funding table to empty (keeping their allocated capacity). Which
    /// implementation runs depends on which arm of `research_config_reimplementation` is active - see
    /// `ZTResearchCategory::clear`'s own doc comment for the full reasoning (same cross-allocator
    /// argument applies here, `Box`-allocated by default vs. real-vanilla-allocated under
    /// `--features vanilla-research-config`).
    #[cfg(feature = "vanilla-research-config")]
    pub fn clear_branch(&mut self) {
        unsafe { research_config_reimplementation::detours::call_original_clear_branch((self as *mut Self) as *const u32) }
    }

    #[cfg(not(feature = "vanilla-research-config"))]
    pub fn clear_branch(&mut self) {
        research_config_reimplementation::destruction::reset_branch_contents(self)
    }

    /// The vanilla "$400 (Min)"-style formatted text for the *currently selected* funding level (per
    /// `private/resources/decompiles/ZTResearchBranch_getFundingText.c`, this always uses
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
/// `ztresearchbranch::PCT_REMAINING_ON_PROGRAM`/`DAYS_REMAINING_ON_PROGRAM`.
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
        // returns 0 (confirmed live via `ZTRESEARCHBRANCH_PCT_DAYS_REMAINING`) - not `i32::MIN`, which
        // a saturating `f32 as i32` cast would give for -inf: vanilla's x87 `FISTP` "integer
        // indefinite" value's low dword - the only part any real caller reads - is 0.
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
/// `ZTMarketing::update` too (`private/resources/decompiles/ZTMarketing_update.c` references the exact same
/// `_DAT_00630d78`, in the exact same `days * cost * scale` shape, right down to reusing
/// `ZTGameMgr::subtractCash`/an embedded `ZooStatus` "spend" call) - a shared days-to-funding-delta
/// scale used by more than just research.
const DAYS_TO_FUNDING_SCALE: f32 = 1.0 / 43200.0;

/// Pure prediction for one `ZTResearchBranch::update(days)` call's progress/cash effect on the
/// currently-selected program, restricted to the "doesn't complete this call" case - `update` itself
/// handles completion (`on_completion`/`pick_random_program`/UI) using this function's result. Per
/// `private/resources/decompiles/ZTResearchBranch_update.c`/`.asm`: `cash_delta`/`progress_delta` are always
/// computed from `days`/the current funding level's `cost`/`rate`, but only actually applied - cash
/// subtracted, progress accumulated - when `cash_delta <= available_cash`; insufficient cash leaves
/// both unchanged for this call (silently - no partial progress, no debt), signalled here by returning
/// `(0.0, 0.0)`.
pub(crate) fn predict_branch_progress(days: u32, funding_cost: f32, funding_rate: f32, available_cash: f32) -> (f32, f32) {
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

/// `GLOBAL_BFUIMgr`'s own fixed address, `0x00638de0` - a plain static object, not a pointer slot
/// (every call site takes its address directly, e.g. `BFUIMgr::getElement((BFUIMgr*)&GLOBAL_BFUIMgr,
/// ...)`), unlike `GLOBAL_ZTResearchMgr`/`GLOBAL_ZTGameMgr` which are one level of pointer indirection
/// away from the real singleton. `0x00635c54` is `BFUIMgr`'s own vtable address, not the object
/// itself - do not confuse the two.
fn global_bfuimgr() -> *const u32 {
    (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32
}

/// The two dialog-`45000` element ids `ZTResearchBranch::update` looks up (`DAT_0063b94e`/
/// `DAT_0063b942+2`, both confirmed by reading the installed `zoo.exe`'s `.data` section directly: raw
/// `u16`s `2`/`6` respectively), each added to the shared dialog id `45000` also passed to
/// `confirmDialog` directly. Fixed data-section literals (not runtime/locale-dependent like
/// `get_money_text`'s `CURRENCYFMTA` fields, which are read live), so hardcoded rather than read live.
const RESEARCH_DIALOG_ICON_ELEMENT_ID: i32 = 45000 + 2;
const RESEARCH_DIALOG_LABEL_ELEMENT_ID: i32 = 45000 + 6;
const RESEARCH_DIALOG_ID: i32 = 45000;

/// `s_ui/sharedui/exclaim/exclaim` - the icon animation `ZTResearchBranch::update` plays for both the
/// "research complete" and "no more research" dialogs (and, per
/// `private/resources/decompiles/ZTAnimal_showEscapedAnimalAlert.c`, the escaped-animal alert too - the same
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
/// `pub(crate)`: also used by `ztmarketing::ZTMarketing::funding_text`, which calls the same
/// `bfinternat::getMoneyText` overload with a different pre-scale on `cost` (none, vs.
/// `MONTHLY_TO_DAILY_COST_SCALE` here).
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
