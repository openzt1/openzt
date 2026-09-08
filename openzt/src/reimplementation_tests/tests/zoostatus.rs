//! Compares real vanilla `ZooStatus` against its Rust reimplementation (production file
//! `openzt/src/zoostatus.rs`): detour wiring/trampoline-routing checks, accumulator/getter
//! comparisons, the message/rating check battery, pricing/override behavior, and save/load
//! (modern + legacy) round-trips. Most fixtures embed the `ZooStatus` inside a standalone
//! `ZTGameMgr` block (vanilla embeds it at `self+0x10`), so this file builds its fixtures
//! through `gamemgr_live_support` rather than `zoostatus_live_support` - see
//! `run_zoostatus_init_test`'s doc comment.

use std::io::Write;
use std::mem::size_of;

use tracing::error;

use openzt_detour::FunctionDef;
use openzt_detour::generated::bfconfigfile::{
    CONSTRUCTOR_0 as BFCONFIGFILE_CONSTRUCTOR_0, RELEASE as BFCONFIGFILE_RELEASE,
};
use openzt_detour::generated::zoostatus::{
    BUY_ANIMAL as ZOOSTATUS_BUY_ANIMAL, BUY_PEOPLE_FOOD as ZOOSTATUS_BUY_PEOPLE_FOOD,
    CALCULATE_SUMS as ZOOSTATUS_CALCULATE_SUMS, CHANGE_ENDOWMENT_MEMBERS as ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS,
    F_GRANT_DONATION as ZOOSTATUS_F_GRANT_DONATION, HEAL_ANIMAL as ZOOSTATUS_HEAL_ANIMAL,
    INCREASE_ADMISSIONS as ZOOSTATUS_INCREASE_ADMISSIONS,
    INCREASE_ADMISSIONS_INCOME as ZOOSTATUS_INCREASE_ADMISSIONS_INCOME,
    INCREASE_DONATIONS as ZOOSTATUS_INCREASE_DONATIONS, INCREASE_ENDOWMENT as ZOOSTATUS_INCREASE_ENDOWMENT,
    INCREASE_SHOW_ADMISSION as ZOOSTATUS_INCREASE_SHOW_ADMISSION, INIT as ZOOSTATUS_INIT, LOAD as ZOOSTATUS_LOAD,
    MESSAGE_CHECKS as ZOOSTATUS_MESSAGE_CHECKS, NEWGUEST_CHECKS as ZOOSTATUS_NEWGUEST_CHECKS,
    OVERRIDE as ZOOSTATUS_OVERRIDE, PURCHASE_FOOD as ZOOSTATUS_PURCHASE_FOOD,
    RATING_CHECKS as ZOOSTATUS_RATING_CHECKS, REFUND_ANIMAL_COST as ZOOSTATUS_REFUND_ANIMAL_COST,
    REFUND_CONSTRUCTION as ZOOSTATUS_REFUND_CONSTRUCTION, SAVE as ZOOSTATUS_SAVE,
    SET_ADULT_ADMISSION_PRICE as ZOOSTATUS_SET_ADULT_ADMISSION_PRICE,
    SPEND_BUILDING_UPKEEP as ZOOSTATUS_SPEND_BUILDING_UPKEEP, SPEND_CONSTRUCTION as ZOOSTATUS_SPEND_CONSTRUCTION,
    SPEND_GUIDE_WAGES as ZOOSTATUS_SPEND_GUIDE_WAGES, SPEND_KEEPER_WAGES as ZOOSTATUS_SPEND_KEEPER_WAGES_1,
    SPEND_MAINT_WAGES as ZOOSTATUS_SPEND_MAINT_WAGES, SPEND_MARKETING as ZOOSTATUS_SPEND_MARKETING,
    SPEND_RESEARCH as ZOOSTATUS_SPEND_RESEARCH,
};

use crate::globals::get_module_base;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};
use crate::ztgamemgr::live_support as gamemgr_live_support;
use crate::ztmarketing::live_support as marketing_live_support;
use crate::zoostatus::{live_support as zoostatus_live_support, ZooStatus, GET_STATUS_FIXED};

/// `ZOOSTATUS_DETOURS_ENABLED` - wiring check: `reimplementation_tests::init()` installs
/// `zoostatus::init()`, and this asserts all 36 of its detours actually report enabled (see
/// `zoostatus.rs`'s own `zoostatus_detours` module doc comment for the full list and for the three
/// addresses deliberately left un-hooked). Without it, a silently-failed `init_detours()` (error
/// logged, game continues on vanilla) would leave the whole battery green while every hooked
/// production path runs vanilla - the trampoline-based comparisons below can't distinguish that from
/// a working hook, mirroring `MENUMUSICHANDLER_DETOURS_ENABLED`'s own doc comment. Runs before the
/// other `ZOOSTATUS_*` tests so a wiring failure is visible first.
pub(crate) fn run_zoostatus_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_DETOURS_ENABLED";
    let mut disabled: Vec<&'static str> = Vec::new();
    for (name, enabled) in zoostatus_live_support::detour_status() {
        if !enabled {
            disabled.push(name);
        }
    }
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE` - debug-only anti-regression for `openzt-detour`'s
/// hook registry, mirroring `MENUMUSICHANDLER_ORIGINAL_ROUTES_TO_TRAMPOLINE`'s own doc comment:
/// `FunctionDef::original()` must return the *real vanilla* function (routed through the detour's
/// trampoline) even for the 36 addresses this battery has itself hooked, not silently re-enter our
/// own Rust detours. For each of them, asserts the registry holds a trampoline, that `.original()`
/// returns exactly that pointer value, and that it differs from the raw address (`zoo.exe` has no
/// ASLR, so an un-routed raw cast would compare equal - pointer equality can't pass vacuously here).
/// Also asserts zero registry overflows: a full slot array fails open into exactly the raw-cast
/// behavior this test guards against. Release builds cfg this out (the raw cast is release's
/// documented `.original()`); the release battery is still run once-off since every existing
/// `ZOOSTATUS_*` comparison test's vanilla pole goes through `.original()` directly rather than a
/// `real_*` trampoline (none was needed - see the `crate::zoostatus::init()` call site's own comment).
#[cfg(debug_assertions)]
pub(crate) fn run_zoostatus_original_routes_to_trampoline_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::zoostatus as zs;

    /// `.original()`'s return value as a raw pointer value. The pointer is only inspected,
    /// never called.
    fn original_ptr<T>(def: &FunctionDef<T>) -> usize
    where
        T: retour::Function,
    {
        let original = unsafe { def.original() };
        original.to_ptr() as usize
    }

    let test_name = "ZOOSTATUS_ORIGINAL_ROUTES_TO_TRAMPOLINE";
    let hooked: [(&'static str, u32, usize); 36] = [
        ("INIT", zs::INIT.address, original_ptr(&zs::INIT)),
        ("OVERRIDE", zs::OVERRIDE.address, original_ptr(&zs::OVERRIDE)),
        ("RESET_FINANCE_INFO", zs::RESET_FINANCE_INFO.address, original_ptr(&zs::RESET_FINANCE_INFO)),
        ("SPEND_CONSTRUCTION", zs::SPEND_CONSTRUCTION.address, original_ptr(&zs::SPEND_CONSTRUCTION)),
        ("SPEND_BUILDING_UPKEEP", zs::SPEND_BUILDING_UPKEEP.address, original_ptr(&zs::SPEND_BUILDING_UPKEEP)),
        ("SPEND_GUIDE_WAGES", zs::SPEND_GUIDE_WAGES.address, original_ptr(&zs::SPEND_GUIDE_WAGES)),
        ("BUY_ANIMAL", zs::BUY_ANIMAL.address, original_ptr(&zs::BUY_ANIMAL)),
        ("SPEND_KEEPER_WAGES", zs::SPEND_KEEPER_WAGES.address, original_ptr(&zs::SPEND_KEEPER_WAGES)),
        ("SPEND_MAINT_WAGES", zs::SPEND_MAINT_WAGES.address, original_ptr(&zs::SPEND_MAINT_WAGES)),
        ("SPEND_MARKETING", zs::SPEND_MARKETING.address, original_ptr(&zs::SPEND_MARKETING)),
        ("SPEND_RESEARCH", zs::SPEND_RESEARCH.address, original_ptr(&zs::SPEND_RESEARCH)),
        ("REFUND_ANIMAL_COST", zs::REFUND_ANIMAL_COST.address, original_ptr(&zs::REFUND_ANIMAL_COST)),
        ("REFUND_CONSTRUCTION", zs::REFUND_CONSTRUCTION.address, original_ptr(&zs::REFUND_CONSTRUCTION)),
        ("INCREASE_DONATIONS", zs::INCREASE_DONATIONS.address, original_ptr(&zs::INCREASE_DONATIONS)),
        ("INCREASE_ENDOWMENT", zs::INCREASE_ENDOWMENT.address, original_ptr(&zs::INCREASE_ENDOWMENT)),
        ("INCREASE_SHOW_ADMISSION", zs::INCREASE_SHOW_ADMISSION.address, original_ptr(&zs::INCREASE_SHOW_ADMISSION)),
        ("BUY_PEOPLE_FOOD", zs::BUY_PEOPLE_FOOD.address, original_ptr(&zs::BUY_PEOPLE_FOOD)),
        ("CHANGE_ENDOWMENT_MEMBERS", zs::CHANGE_ENDOWMENT_MEMBERS.address, original_ptr(&zs::CHANGE_ENDOWMENT_MEMBERS)),
        ("ANIMAL_ESCAPED", zs::ANIMAL_ESCAPED.address, original_ptr(&zs::ANIMAL_ESCAPED)),
        ("ADMISSION_MESSAGE", zs::ADMISSION_MESSAGE.address, original_ptr(&zs::ADMISSION_MESSAGE)),
        ("NEWGUEST_CHECKS", zs::NEWGUEST_CHECKS.address, original_ptr(&zs::NEWGUEST_CHECKS)),
        ("MESSAGE_CHECKS", zs::MESSAGE_CHECKS.address, original_ptr(&zs::MESSAGE_CHECKS)),
        ("RATING_CHECKS", zs::RATING_CHECKS.address, original_ptr(&zs::RATING_CHECKS)),
        ("F_GRANT_DONATION", zs::F_GRANT_DONATION.address, original_ptr(&zs::F_GRANT_DONATION)),
        ("F_ZOO_MESSAGE", zs::F_ZOO_MESSAGE.address, original_ptr(&zs::F_ZOO_MESSAGE)),
        ("SET_ADULT_ADMISSION_PRICE", zs::SET_ADULT_ADMISSION_PRICE.address, original_ptr(&zs::SET_ADULT_ADMISSION_PRICE)),
        ("SHOW_PRICES", zs::SHOW_PRICES.address, original_ptr(&zs::SHOW_PRICES)),
        ("CALCULATE_SUMS", zs::CALCULATE_SUMS.address, original_ptr(&zs::CALCULATE_SUMS)),
        ("UPDATE", zs::UPDATE.address, original_ptr(&zs::UPDATE)),
        ("SAVE", zs::SAVE.address, original_ptr(&zs::SAVE)),
        ("LOAD", zs::LOAD.address, original_ptr(&zs::LOAD)),
        ("HEAL_ANIMAL", zs::HEAL_ANIMAL.address, original_ptr(&zs::HEAL_ANIMAL)),
        ("PURCHASE_FOOD", zs::PURCHASE_FOOD.address, original_ptr(&zs::PURCHASE_FOOD)),
        ("INCREASE_ADMISSIONS_INCOME", zs::INCREASE_ADMISSIONS_INCOME.address, original_ptr(&zs::INCREASE_ADMISSIONS_INCOME)),
        ("INCREASE_ADMISSIONS", zs::INCREASE_ADMISSIONS.address, original_ptr(&zs::INCREASE_ADMISSIONS)),
        ("GET_STATUS_FIXED", GET_STATUS_FIXED.address, original_ptr(&GET_STATUS_FIXED)),
    ];

    let mut failures: Vec<String> = Vec::new();
    for (name, address, original) in hooked {
        match openzt_detour::trampoline_for(address) {
            Some(trampoline) => {
                if original != trampoline {
                    failures.push(format!(
                        "{name} ({address:#010x}): .original() = {original:#010x} != registered trampoline {trampoline:#010x}"
                    ));
                }
                if original == address as usize {
                    failures.push(format!(
                        "{name} ({address:#010x}): .original() equals the raw address - routing fell back to the raw cast"
                    ));
                }
            }
            None => failures.push(format!(
                "{name} ({address:#010x}): no trampoline registered - detour() did not publish, or the registry overflowed"
            )),
        }
    }
    let overflow = openzt_detour::registry_overflow_count();
    if overflow != 0 {
        failures.push(format!("{overflow} address(es) failed to register in the hook registry (capacity overflow - fail-open raw casts)"));
    }

    finish_test(test_name, failures, failure_log)
}

/// `ZOOSTATUS_INIT` - `zoostatus-implementation-plan.md` Stage 2's live comparison: builds two
/// standalone `ZTGameMgr` blocks (same harness `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` uses), zeroes each
/// one's *embedded `ZooStatus` sub-region only* (`+0x10..+0x1190`, matching that test's own
/// "`ZooStatus::init` reads at least one field before ever writing it" finding -
/// [`crate::zoostatus::ZooStatus::admission_price`] here specifically, never written before
/// `setAdultAdmissionPrice` reads it back), then runs real vanilla `ZOOSTATUS_INIT.original()`
/// against one and [`ZooStatus::init`] against the other, both with a null config pointer.
///
/// **Null config is deliberately safe, not a shortcut**: `ZooStatus::override`'s decompile
/// (`ZooStatus_override.c`, read in full) null-checks its `param_1` as the very first statement and
/// returns immediately otherwise - so passing null exercises `init`'s own unconditional writes plus
/// a real, genuinely-taken call into `override`/`setAdultAdmissionPrice` (not skipped), without
/// needing a real `BFConfigFile` instance built for this test the way
/// `ZTGAMEMGR_SET_NEW_GAME_DEFAULTS` needs one.
///
/// Masked byte ranges (documented, not silently swallowed):
/// - `+0x68..+0x6c` (`max_guests`): `ZooStatus::init`'s own `BFIniFile::read` of `AI`/`maxGuests` is
///   a real, untouched dependency (constructing its `std::string` arguments - see `ztgamemgr.rs`'s
///   `initMenuMusic` doc comment for the same class of gap) - [`ZooStatus::init`] hardcodes the
///   vanilla default (`1000`) instead. Masked defensively; in practice the real pole should read the
///   same default unless the live environment's ini actually overrides this key.
/// - `+0x1178..+0x1180` (`last_animal_escape_timestamp_*`): both poles call the real
///   [`GET_OLD_DATE`](openzt_detour::generated::standalone::GET_OLD_DATE) independently, a couple of
///   CPU cycles apart - genuinely time-dependent, not a porting bug.
pub(crate) fn run_zoostatus_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_INIT";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        ZOOSTATUS_INIT.original()(real_zoostatus_ptr as *const u32, std::ptr::null());
        (*reimpl_zoostatus_ptr).init(std::ptr::null());
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let excluded_ranges: [std::ops::Range<usize>; 2] = [0x68..0x6c, 0x1178..0x1180];

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_ACCUMULATORS` - `zoostatus-implementation-plan.md` Stage 3's live comparison: builds
/// two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT` uses), zeroes each one's embedded
/// `ZooStatus` sub-region, then seeds [`ZooStatus::current_month_index`]/
/// [`ZooStatus::current_year_index`] to non-default values (`5`/`7`, not `init`'s own `1`/`0`)
/// directly via raw offset writes on both instances identically - deliberately exercising the
/// dynamic `LEA [ECX+EAX*4+<offset>]` index-scaling every one of these 15 methods does, not just the
/// zero-index case `init`'s own default would otherwise leave untested.
///
/// Runs every one of the 14 "simple accumulator" methods' real `.original()` against one instance and
/// the matching [`ZooStatus`] method against the other, with the same varied (including negative and
/// fractional) `f32` amount each time, then [`ZooStatus::change_endowment_members`] three times
/// (positive/negative/zero delta) to cover its three-way branch - both real
/// [`ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS`] and the Rust port take the same `i32` sequence. A single
/// full-struct byte comparison at the end catches any divergence across the whole run (no masking
/// needed - none of these methods touch [`ZooStatus::admission_price`]/the escape timestamp, the only
/// fields `ZOOSTATUS_INIT`'s own test had to mask).
///
/// **Extended (Stage 10)** with the five real Windows methods a fresh Ghidra pass recovered from the
/// macOS-only corpus: [`ZooStatus::buy_animal`] (the renamed, previously-mislabeled
/// `SPEND_KEEPER_WAGES_0`), [`ZooStatus::heal_animal`], [`ZooStatus::purchase_food`],
/// [`ZooStatus::increase_admissions_income`] (all four fit this same shared-instance byte-diff
/// harness, same as every other accumulator method) and [`ZooStatus::increase_admissions`] (the
/// `i32`-count outlier, run once here rather than getting its own test - same reasoning as
/// `change_endowment_members` sharing this harness instead of a dedicated one).
/// [`ZooStatus::get_status`] (the sixth recovered method, a pure reader) has its own dedicated test,
/// `ZOOSTATUS_GET_STATUS`, since it doesn't fit this write-and-diff shape.
pub(crate) fn run_zoostatus_accumulators_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_ACCUMULATORS";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            save_to_memory(ptr as u32 + 0x14c, 5i32);
            save_to_memory(ptr as u32 + 0x150, 7i32);
        }

        ZOOSTATUS_SPEND_CONSTRUCTION.original()(real_zoostatus_ptr as *const u32, 1234.5);
        (*reimpl_zoostatus_ptr).spend_construction(1234.5);
        ZOOSTATUS_SPEND_BUILDING_UPKEEP.original()(real_zoostatus_ptr as *const u32, 42.25);
        (*reimpl_zoostatus_ptr).spend_building_upkeep(42.25);
        ZOOSTATUS_SPEND_GUIDE_WAGES.original()(real_zoostatus_ptr as *const u32, 99.0);
        (*reimpl_zoostatus_ptr).spend_guide_wages(99.0);
        ZOOSTATUS_BUY_ANIMAL.original()(real_zoostatus_ptr as *const u32, 17.5);
        (*reimpl_zoostatus_ptr).buy_animal(17.5);
        ZOOSTATUS_HEAL_ANIMAL.original()(real_zoostatus_ptr as *const u32, 8.25);
        (*reimpl_zoostatus_ptr).heal_animal(8.25);
        ZOOSTATUS_PURCHASE_FOOD.original()(real_zoostatus_ptr as *const u32, 4.5);
        (*reimpl_zoostatus_ptr).purchase_food(4.5);
        ZOOSTATUS_INCREASE_ADMISSIONS_INCOME.original()(real_zoostatus_ptr as *const u32, 320.0);
        (*reimpl_zoostatus_ptr).increase_admissions_income(320.0);
        ZOOSTATUS_INCREASE_ADMISSIONS.original()(real_zoostatus_ptr as *const u32, 7);
        (*reimpl_zoostatus_ptr).increase_admissions(7);
        ZOOSTATUS_SPEND_KEEPER_WAGES_1.original()(real_zoostatus_ptr as *const u32, 250.0);
        (*reimpl_zoostatus_ptr).spend_keeper_wages_1(250.0);
        ZOOSTATUS_SPEND_MAINT_WAGES.original()(real_zoostatus_ptr as *const u32, 3.75);
        (*reimpl_zoostatus_ptr).spend_maint_wages(3.75);
        ZOOSTATUS_SPEND_MARKETING.original()(real_zoostatus_ptr as *const u32, 500.0);
        (*reimpl_zoostatus_ptr).spend_marketing(500.0);
        ZOOSTATUS_SPEND_RESEARCH.original()(real_zoostatus_ptr as *const u32, 1000.0);
        (*reimpl_zoostatus_ptr).spend_research(1000.0);
        ZOOSTATUS_REFUND_ANIMAL_COST.original()(real_zoostatus_ptr as *const u32, 60.0);
        (*reimpl_zoostatus_ptr).refund_animal_cost(60.0);
        ZOOSTATUS_REFUND_CONSTRUCTION.original()(real_zoostatus_ptr as *const u32, 80.0);
        (*reimpl_zoostatus_ptr).refund_construction(80.0);
        ZOOSTATUS_INCREASE_DONATIONS.original()(real_zoostatus_ptr as *const u32, 25.0);
        (*reimpl_zoostatus_ptr).increase_donations(25.0);
        ZOOSTATUS_INCREASE_ENDOWMENT.original()(real_zoostatus_ptr as *const u32, 5000.0);
        (*reimpl_zoostatus_ptr).increase_endowment(5000.0);
        ZOOSTATUS_INCREASE_SHOW_ADMISSION.original()(real_zoostatus_ptr as *const u32, 12.0);
        (*reimpl_zoostatus_ptr).increase_show_admission(12.0);
        ZOOSTATUS_BUY_PEOPLE_FOOD.original()(real_zoostatus_ptr as *const u32, 6.5);
        (*reimpl_zoostatus_ptr).buy_people_food(6.5);

        for delta in [3i32, -4i32, 0i32] {
            ZOOSTATUS_CHANGE_ENDOWMENT_MEMBERS.original()(real_zoostatus_ptr as *const u32, delta);
            (*reimpl_zoostatus_ptr).change_endowment_members(delta);
        }
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_GET_STATUS` - `zoostatus-implementation-plan.md` Stage 10's live comparison for
/// [`ZooStatus::get_status`], the one recovered macOS-only method that's a pure reader rather than a
/// mutator, so it doesn't fit `ZOOSTATUS_ACCUMULATORS`' write-and-diff harness. Builds a single
/// standalone `ZTGameMgr` block (same harness the other `ZOOSTATUS_*` tests use), zeroes its embedded
/// `ZooStatus` sub-region, then fills every 4-byte word of the whole struct with a distinct non-zero
/// float (`index + 0.5`) - not representative game data, just enough that a wrong offset/stride reads
/// back a detectably different value instead of coincidentally matching a zeroed neighbor - and seeds
/// [`ZooStatus::current_month_index`]/[`ZooStatus::current_year_index`] to non-default values (`5`/`7`,
/// matching `ZOOSTATUS_ACCUMULATORS`' own convention) to exercise the `index == -1` default-cursor
/// path for real.
///
/// Calls real vanilla [`GET_STATUS_FIXED`] (the locally-corrected `FunctionDef` - see its own doc
/// comment for why `generated.rs`'s own `GET_STATUS` entry can't be used) and [`ZooStatus::get_status`]
/// against the *same* memory for a spread of `(category, when, index)` triples: monthly (`when = 0`)
/// and yearly (`when = 1`) each with an explicit `index` and with `index = -1` (default-cursor), at
/// category `0` (the row-0 discrepancy case - see [`ZooStatus::get_status`]'s own doc comment), a
/// mid-range category, and category `30` (the last row, right up against the region boundary); flat
/// (`when = 2`) at a few categories; and `when = 3` to hit the real fallback branch (returns
/// `zoostatus.rs`'s `raw_globals::ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA` global unconditionally).
/// Every category/index value here stays within the struct's real bounds (verified by hand against
/// `get_status`'s own addressing - see that method's doc comment), so this never reads outside the
/// allocated block.
pub(crate) fn run_zoostatus_get_status_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_GET_STATUS";

    let mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let zoostatus_ptr = (mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(zoostatus_ptr as *mut u8, 0, zoostatus_size);
        for i in 0..(zoostatus_size / 4) {
            save_to_memory(zoostatus_ptr as u32 + (i * 4) as u32, (i as f32) + 0.5);
        }
        save_to_memory(zoostatus_ptr as u32 + 0x14c, 5i32);
        save_to_memory(zoostatus_ptr as u32 + 0x150, 7i32);
    }

    let cases: &[(i32, i32, i32)] = &[
        (0, 0, 0),
        (0, 0, -1),
        (5, 0, 3),
        (30, 0, 11),
        (0, 1, 0),
        (0, 1, -1),
        (5, 1, 12),
        (30, 1, 19),
        (0, 2, -1),
        (5, 2, -1),
        (30, 2, -1),
        (0, 3, -1),
    ];

    let mut mismatches: Vec<(i32, i32, i32, f32, f32)> = Vec::new();
    for &(category, when, index) in cases {
        let real = unsafe { GET_STATUS_FIXED.original()(zoostatus_ptr as *const u32, category, when, index) };
        let reimpl = unsafe { (*zoostatus_ptr).get_status(category, when, index) };
        if real != reimpl {
            mismatches.push((category, when, index, real, reimpl));
        }
    }

    let failed = !mismatches.is_empty();
    if failed {
        error!("{}: {} mismatch(es) (category, when, index, real, reimpl): {:?}", test_name, mismatches.len(), mismatches);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} mismatch(es): {:?}\n", test_name, mismatches.len(), mismatches).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(mgr_ptr);
    failed
}

/// `ZOOSTATUS_NEWGUEST_CHECKS_SMOKE` - `zoostatus-implementation-plan.md` Stage 4's live coverage for
/// [`ZooStatus::newguest_checks`]. Upgraded from a crash/hang-only smoke test to a real full-struct
/// byte comparison against real vanilla `ZOOSTATUS_NEWGUEST_CHECKS.original()`, by forcing
/// [`Self::guest_type_arrival_multiplier`] to `[0; 5]` on both poles - whichever of the five entries
/// [`Self::newguest_dispatch`] would otherwise select, this pins `F_CHANCE`'s roll parameter to `0`,
/// which (per [`Self::newguest_checks`]'s own doc comment on the `& 0xff == 0` masking) deterministically
/// fails on both poles regardless of the real, shared `fChance` RNG stream's actual draw - so
/// `F_CREATE_GUEST` (the cross-allocator freelist hazard) and [`Self::admission_message`]'s real UI
/// path (the hang this test's earlier smoke-only form was added to guard against - see
/// `ZOOSTATUS_CHECKS`'s own doc comment for the incident) are both **provably never reached**, on
/// either pole, independent of live `GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTMarketingMgr`/escaped-animal-list
/// state. Both poles still read that same live state identically, so whichever branch it steers them
/// into (early return on no occupied habitats/an escaped animal, or the full price-tier write +
/// band/tier dispatch), they take the *same* branch - giving a genuine byte-for-byte comparison of
/// [`Self::newguest_check_elapsed`]'s reset and [`Self::field_0x48`]'s tier write whenever live state
/// happens to reach that path, and of the early-return no-op otherwise. The band/tier dispatch
/// *table* itself stays additionally covered independent of live state by `#[cfg(test)]`'s
/// `price_tier_matches_the_confirmed_boundary_chain`/`newguest_dispatch_matches_the_derived_band_tier_table`.
pub(crate) fn run_zoostatus_newguest_checks_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_NEWGUEST_CHECKS_SMOKE";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            (*ptr).init(std::ptr::null());
            // Pins F_CHANCE's roll parameter to 0 - see this function's own doc comment.
            (*ptr).guest_type_arrival_multiplier = [0; 5];
        }

        ZOOSTATUS_NEWGUEST_CHECKS.original()(real_zoostatus_ptr as *const u32);
        (*reimpl_zoostatus_ptr).newguest_checks();
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_CHECKS` - `zoostatus-implementation-plan.md` Stage 4's live comparison for the two
/// fully-native methods this stage ports, [`ZooStatus::message_checks`]/[`ZooStatus::rating_checks`].
/// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS`
/// use), zeroes each one's embedded `ZooStatus` sub-region, seeds both identically with representative
/// values for every field these two methods read (guest/animal counts, message thresholds,
/// `species_rating_cap`, `field_0x4c`, a past escape timestamp, non-default
/// `current_month_index`/`current_year_index`), then runs real vanilla
/// `MESSAGE_CHECKS.original()`/`RATING_CHECKS.original()` against one and
/// [`ZooStatus::message_checks`]/[`ZooStatus::rating_checks`] against the other, in the same order
/// vanilla's own `update` would call them.
///
/// `rating_checks` calls [`ZooStatus::calculate_sums`] (native as of Stage 5) - the real pole's
/// `RATING_CHECKS.original()` calls real vanilla `calculateSums` internally, and the reimpl pole's
/// `rating_checks()` calls the Rust port; since that call only reads live global manager state
/// (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTResearchMgr`) and never touches anything but
/// `this`, and both poles read the exact same live globals, the many counters it overwrites
/// (`num_animals`/`num_species`/the guest-condition counters/`non_blank_tile_fraction`/
/// `research_completion_percent`/etc.) end up byte-identical without needing to be pre-seeded here -
/// this is, incidentally, a second live comparison point for [`ZooStatus::calculate_sums`] beyond
/// `ZOOSTATUS_CALCULATE_SUMS`'s own dedicated test below.
///
/// Masked byte range (documented, not silently swallowed): `+0x1178..+0x1180`
/// (`last_animal_escape_timestamp_*`) - only actually touched if the live escaped-animal list happens
/// to be non-empty at test time, in which case both poles independently call the real
/// `ZTGameMgr::getDate`/[`ZooStatus::animal_escaped`] a couple of CPU cycles apart (same genuinely
/// time-dependent masking `ZOOSTATUS_INIT` already applies to the same field).
///
/// **`message_checks` deliberately never triggers a real `fZooMessage` here** - every threshold-check
/// input above is chosen so none of its eight frequency checks or two guest-rating-band checks
/// evaluate true, and the live `ZTGameMgr` singleton's cash is temporarily pinned comfortably positive
/// (`with_ztgamemgr_cash`, restored afterward) to keep its own three cash-band checks quiet too. A
/// first draft of this test picked a `guest_rating_metric`/`field_0xfc` pair that satisfied one of
/// those checks, which called real vanilla `BFUIMgr::displayMessage` against the live game and hung
/// the whole reimplementation-test run (a real vanilla UI call working as intended against synthetic
/// test data, not a bug in this port) - this test exists to compare arithmetic, not to exercise real
/// UI side effects.
pub(crate) fn run_zoostatus_checks_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_CHECKS";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            let status = &mut *ptr;
            status.num_animals = 40;
            status.animal_condition_counter_1 = 4;
            status.num_species = 12;
            status.species_rating_cap = 44;
            status.num_tired_guests = 3;
            status.num_hungry_guests = 5;
            status.num_thirst_guests = 2;
            status.num_guests_restroom_need = 6;
            status.guest_condition_counter_1 = 1;
            status.guest_condition_counter_2 = 2;
            status.guest_tile_count = 15;
            status.field_0x4c = 12000;
            status.animal_rating_metric = 20;
            // Kept clear of field_0xf4/field_0xfc below so no `fZooMessage` fires - see this
            // function's own doc comment.
            status.guest_rating_metric = 50;
            status.non_blank_tile_fraction = 0.6;
            status.message_threshold_0x70 = 0.5;
            status.message_threshold_0x74 = 0.5;
            status.message_threshold_0x7c = 0.5;
            status.message_threshold_0x84 = 0.5;
            status.message_threshold_0x8c = 0.5;
            status.message_threshold_0x94 = 0.5;
            status.message_threshold_0xa0 = 0.5;
            status.message_threshold_0xa8 = 0.5;
            status.current_month_index = 5;
            status.current_year_index = 7;
            // A plausible past escape timestamp - a real FILETIME comfortably before "now", so
            // rating_checks' hours-since-escape decay term exercises a real, non-zero value.
            status.last_animal_escape_timestamp_low = 0;
            status.last_animal_escape_timestamp_high = 0x01c00000;

            let base = ptr as u32;
            save_to_memory(base + 0xf4, 200i32);
            save_to_memory(base + 0xfc, -50i32);
        }

        // Pin the live ZTGameMgr singleton's cash comfortably positive (avoiding message_checks'
        // three cash-band `fZooMessage` branches - see the seed-value comment above) and restore it
        // afterward, matching the established `with_ztgamemgr_cash` convention.
        marketing_live_support::with_ztgamemgr_cash(50000.0, || {
            ZOOSTATUS_MESSAGE_CHECKS.original()(real_zoostatus_ptr as *const u32);
            (*reimpl_zoostatus_ptr).message_checks();
        });

        ZOOSTATUS_RATING_CHECKS.original()(real_zoostatus_ptr as *const u32);
        (*reimpl_zoostatus_ptr).rating_checks();
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    // Byte-exclusion table for the diff below, kept as ranges so new exclusions are one-line
    // entries (clippy would rather this were a Vec of contained bytes or a filled array - both
    // change what it means). +0x1178..0x1180 = `last_animal_escape_timestamp_*`, genuinely
    // time-dependent when the live escaped-animal list is non-empty (see this test's doc comment).
    #[allow(clippy::single_range_in_vec_init)]
    let excluded_ranges: [std::ops::Range<usize>; 1] = [0x1178..0x1180];

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter(|i| !excluded_ranges.iter().any(|r| r.contains(i)))
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_F_GRANT_DONATION_NO_OP` - live comparison for [`ZooStatus::f_grant_donation`]'s
/// `AlreadyPastBound` branch (see `zoostatus.rs`'s own `donation_outcome` doc comment for the
/// three-way split it dispatches through). This is deliberately the *only* branch of that function
/// this battery compares live: the other two (`BoundJustCrossed`/`Grant`) both call through to real
/// vanilla `BFUIMgr::displayMessage`, which `ZOOSTATUS_CHECKS`'s own doc comment already documents
/// hanging the whole reimplementation-test run when accidentally reached live - not a risk worth
/// re-taking here. The two message-firing branches instead get direct, live-call-free coverage from
/// `donation_outcome`'s own `#[cfg(test)]` unit test in `zoostatus.rs`.
///
/// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS`
/// use), zeroes each one's embedded `ZooStatus` sub-region, seeds `donation_count_this_period`/
/// `donation_count_bound` on both so the post-increment count (`11.0`) is neither `<= bound` (`3`) nor
/// exactly `bound + 1` (`4`) - guaranteeing the silent no-op branch and nothing else - then runs real
/// vanilla `F_GRANT_DONATION.original()` against one and [`ZooStatus::f_grant_donation`] against the
/// other, with a full-struct byte comparison after. No masking needed: this branch touches nothing but
/// `donation_count_this_period` itself, on `self` - never the live `GLOBAL_ZTGameMgr` singleton.
pub(crate) fn run_zoostatus_f_grant_donation_no_op_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_F_GRANT_DONATION_NO_OP";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            let status = &mut *ptr;
            status.donation_count_this_period = 10.0;
            status.donation_count_bound = 3;
        }

        ZOOSTATUS_F_GRANT_DONATION.original()(real_zoostatus_ptr as *const u32);
        (*reimpl_zoostatus_ptr).f_grant_donation();
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_PRICING` - `zoostatus-implementation-plan.md` Stage 5's live comparison for
/// [`ZooStatus::set_adult_admission_price`]. Builds two standalone `ZTGameMgr` blocks (same harness
/// `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS` use), zeroes each one's embedded `ZooStatus` sub-region,
/// seeds `admission_price_min`/`_max` identically on both, then runs real vanilla
/// `SET_ADULT_ADMISSION_PRICE.original()` against one and [`ZooStatus::set_adult_admission_price`]
/// against the other across five representative prices in one sequence (below min, at min, mid-range,
/// at max, above max - exercising every branch of the clamp), with a full-struct byte comparison
/// after each call. No masking needed - this method touches nothing but `admission_price` itself.
pub(crate) fn run_zoostatus_pricing_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_PRICING";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            let status = &mut *ptr;
            status.admission_price_min = 10.0;
            status.admission_price_max = 100.0;
        }

        for price in [1.0f32, 10.0, 55.0, 100.0, 250.0] {
            ZOOSTATUS_SET_ADULT_ADMISSION_PRICE.original()(real_zoostatus_ptr as *const u32, price);
            (*reimpl_zoostatus_ptr).set_adult_admission_price(price);
        }
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_CALCULATE_SUMS` - `zoostatus-implementation-plan.md` Stage 5's dedicated live
/// comparison for [`ZooStatus::calculate_sums`] (see also `ZOOSTATUS_CHECKS`'s own doc comment for a
/// second, incidental comparison point via `rating_checks`). Builds two standalone `ZTGameMgr`
/// blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_ACCUMULATORS` use), zeroes each one's embedded
/// `ZooStatus` sub-region, seeds non-default `current_month_index`/`current_year_index` (`4`/`6`) on
/// both identically (exercising the dynamic history-slot addressing the same way
/// `ZOOSTATUS_ACCUMULATORS` does), then runs real vanilla `CALCULATE_SUMS.original()` against one and
/// [`ZooStatus::calculate_sums`] against the other, with a full-struct byte comparison. No masking
/// needed: `calculate_sums` only ever reads live global manager state
/// (`GLOBAL_ZTWorldMgr`/`GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTResearchMgr`) - both poles read the exact
/// same live globals in the same process, so every counter it writes ends up byte-identical, and it
/// never touches [`ZooStatus::admission_price`]/the escape timestamp (the only genuinely
/// non-deterministic fields elsewhere in this struct).
pub(crate) fn run_zoostatus_calculate_sums_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_CALCULATE_SUMS";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        for ptr in [real_zoostatus_ptr, reimpl_zoostatus_ptr] {
            save_to_memory(ptr as u32 + 0x14c, 4i32);
            save_to_memory(ptr as u32 + 0x150, 6i32);
        }

        ZOOSTATUS_CALCULATE_SUMS.original()(real_zoostatus_ptr as *const u32);
        (*reimpl_zoostatus_ptr).calculate_sums();
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// `ZOOSTATUS_SHOW_PRICES_SMOKE` - `zoostatus-implementation-plan.md` Stage 5's live coverage for
/// [`ZooStatus::show_prices`]. **Not** a byte-comparison - `show_prices` reads `self` but writes
/// nothing back into it (see that method's own doc comment), so there is no struct state to diff.
/// Builds one standalone `ZTGameMgr` block, runs real [`ZooStatus::init`] to get real, in-bounds
/// `admission_price`/`_min`/`_max` values, then calls [`ZooStatus::show_prices`] against the live
/// `GLOBAL_BFUIMgr` singleton and its real `0x105e`/`0x1061`/`0x1062`/`0x1063`/`0x105f` UI elements -
/// confirms the real `bfinternat::setMoneyText`/`BFUIMgr::getElement`/`UIElement::enable`/`disable`
/// call chain doesn't crash or hang, matching the precedent `ZOOSTATUS_NEWGUEST_CHECKS_SMOKE` set for
/// methods with real UI/live-singleton side effects a byte-diff can't usefully cover.
pub(crate) fn run_zoostatus_show_prices_smoke_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_SHOW_PRICES_SMOKE";

    let mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null\n", test_name).as_bytes());
        }
        return true;
    }

    let zoostatus_ptr = (mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    unsafe {
        std::ptr::write_bytes(zoostatus_ptr as *mut u8, 0, size_of::<ZooStatus>());
        (*zoostatus_ptr).init(std::ptr::null());
        (*zoostatus_ptr).show_prices();
    }

    write_success_line(failure_log, test_name);
    gamemgr_live_support::destroy_standalone_mgr(mgr_ptr);
    false
}

/// `ZOOSTATUS_OVERRIDE` - `zoostatus-implementation-plan.md` Stage 6's live comparison for
/// [`ZooStatus::override_config`]. Builds two standalone `ZTGameMgr` blocks (same harness
/// `ZOOSTATUS_INIT`/`ZOOSTATUS_PRICING` use), zeroes each one's embedded `ZooStatus` sub-region, then
/// constructs one real, shared `BFConfigFile` over the actual shipped `economy.cfg`
/// (`BFCONFIGFILE_CONSTRUCTOR_0`, the same construct-and-parse pattern
/// `ztshowmgr.rs`'s `initShowParams` test already established for `shows.cfg` - see that test's own
/// doc comment; `economy.cfg`'s filename address comes from `ZTScenarioMgr_commonSetup.c`'s own
/// `BFConfigFile::attempt` call site, the only other place in the decompile corpus that opens this
/// file). Runs real vanilla `OVERRIDE.original()` against one `ZooStatus` and
/// [`ZooStatus::override_config`] against the other, both reading the *same* live, already-parsed
/// config object (pure reads - safe to reuse across both calls), then full-struct byte-compares.
///
/// **Also verifies the one write `override` makes outside `this`**: the `cAdultAdmission` config
/// list gets copied into a real process-global (`0x6392ac..0x6392c0`, see
/// [`zoostatus::raw_globals::PRICE_TIER_BOUNDARY_0_RVA`] and
/// [`ZooStatus::override_config`]'s own doc comment for the full evidence trail), which a pure
/// `ZooStatus`-struct byte diff can't see at all - both poles write to the *same* shared process
/// memory, so the second call's write would silently clobber the first's if compared naively. This
/// captures the global's value right after each pole's own call instead (`real_boundaries`/
/// `reimpl_boundaries`), and additionally checks the real pole's own result against `economy.cfg`'s
/// known shipped values (`49`/`29`/`19`/`9`/`0`) - a positive check that the write actually happened,
/// not just that both poles agree by both being silent no-ops.
pub(crate) fn run_zoostatus_override_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_OVERRIDE";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    // `ZTScenarioMgr_commonSetup.c`'s own call site uses `PTR_s_economy_cfg_00641bf4` - the `PTR_`
    // prefix means `0x641bf4` is a *pointer variable* holding the real string's address, not the
    // string bytes themselves (unlike `ZooStatus_override.c`'s own bare `s_<text>_<addr>` section/key
    // literals, which are direct addresses - see `override_config_keys`). One extra dereference is
    // needed here. Also the shared small-object freelist head `BFConfigFile`'s own ctor/dtor pop/push
    // uses (`DAT_0063800c`) - the same freelist `ztshowmgr.rs`'s `CONFIG_FREELIST_HEAD_RVA` reads,
    // kept as its own local copy per this codebase's "not shared even with each other" convention.
    const ECONOMY_CFG_FILENAME_PTR_RVA: u32 = 0x00641bf4 - 0x400000;
    const CONFIG_FREELIST_HEAD_RVA: u32 = 0x0063800c - 0x400000;

    let base = get_module_base("zoo.exe") as u32;
    let economy_cfg_filename_ptr: u32 = get_from_memory(base + ECONOMY_CFG_FILENAME_PTR_RVA);
    let config = std::mem::MaybeUninit::<crate::bfconfigfile::BFConfigFile>::uninit();
    let config_ptr = config.as_ptr() as *const u32;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);

        BFCONFIGFILE_CONSTRUCTOR_0.original()(config_ptr, economy_cfg_filename_ptr as *const u8);
    }

    if get_from_memory::<i32>(config_ptr as u32 + 0x4) == 0 {
        error!("{}: economy.cfg failed to load - real vanilla BFConfigFile has no data", test_name);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: economy.cfg failed to load\n", test_name).as_bytes());
        }
        unsafe { BFCONFIGFILE_RELEASE.original()(config_ptr) };
        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        return true;
    }

    // `override`'s `cAdultAdmission` copy loop writes into a *global* (`0x6392ac..0x6392c0`, see
    // `zoostatus.rs`'s `raw_globals::PRICE_TIER_BOUNDARY_0_RVA..=_4_RVA`), not into `this` - the
    // whole-struct byte comparison below can't see that write at all, since both poles share the same
    // process memory and the second call's write would silently clobber the first's. Capture it after
    // each pole's own call instead, so a divergence there is caught directly rather than masked.
    const PRICE_TIER_BOUNDARY_BASE_RVA: u32 = 0x006392ac - 0x400000;
    let read_price_tier_boundaries = || -> [f32; 5] {
        std::array::from_fn(|i| get_from_memory(base + PRICE_TIER_BOUNDARY_BASE_RVA + (i as u32) * 4))
    };

    unsafe {
        ZOOSTATUS_OVERRIDE.original()(real_zoostatus_ptr as *const u32, config_ptr);
    }
    let real_boundaries = read_price_tier_boundaries();

    unsafe {
        (*reimpl_zoostatus_ptr).override_config(config_ptr as *const std::ffi::c_void);
    }
    let reimpl_boundaries = read_price_tier_boundaries();

    unsafe {
        BFCONFIGFILE_RELEASE.original()(config_ptr);
    }

    // ~BFConfigFile's inlined dtor tail - see `ztshowmgr.rs`'s `init_show_params` for the same pattern.
    let tree_root: u32 = get_from_memory(config_ptr as u32);
    if tree_root != 0 {
        let freelist_head = (base + CONFIG_FREELIST_HEAD_RVA) as *mut u32;
        unsafe {
            let head = *freelist_head;
            *(tree_root as *mut u32) = head;
            *freelist_head = tree_root;
        }
    }

    let real_bytes = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };

    let mut mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes[i] != reimpl_bytes[i] { Some((i, real_bytes[i], reimpl_bytes[i])) } else { None })
        .collect();

    // `economy.cfg`'s real, shipped `cAdultAdmission` values - both a positive check that this write
    // actually happened (not just "both poles agree by both being no-ops") and the byte-level
    // real-vs-reimpl comparison the struct diff above can't reach.
    let expected_boundaries: [f32; 5] = [49.0, 29.0, 19.0, 9.0, 0.0];
    if real_boundaries != expected_boundaries {
        error!("{}: real vanilla's own price-tier-boundary globals don't match economy.cfg's known values: {:?}", test_name, real_boundaries);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: real vanilla price-tier-boundary globals unexpected: {:?}\n", test_name, real_boundaries).as_bytes());
        }
        mismatches.push((usize::MAX, 0, 0));
    }
    if reimpl_boundaries != real_boundaries {
        error!("{}: price-tier-boundary globals mismatch after override_config (real, reimpl): {:?} vs {:?}", test_name, real_boundaries, reimpl_boundaries);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: price-tier-boundary globals mismatch (real, reimpl): {:?} vs {:?}\n", test_name, real_boundaries, reimpl_boundaries).as_bytes());
        }
        mismatches.push((usize::MAX, 0, 0));
    }

    let failed = !mismatches.is_empty();
    if failed {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es), first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
    } else {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
    failed
}

/// Writes distinct, non-default values into every field [`ZooStatus::save`]/[`ZooStatus::load`]
/// actually touch - a different value per history-array slot (`row*100+col` style), so an
/// offset/indexing bug anywhere in the three array regions surfaces as a mismatch instead of being
/// masked by every slot sharing one value. Used identically on both the real and reimplemented
/// instance by `ZOOSTATUS_SAVE_LOAD`.
fn seed_zoostatus_for_save_load(ptr: *mut ZooStatus) {
    unsafe {
        (*ptr).rating_check_elapsed = 111;
        (*ptr).message_check_elapsed = 222;
        (*ptr).newguest_check_elapsed = 333;
        (*ptr).finance_check_pending = true;
        (*ptr).zoo_rating_current = 42;
        (*ptr).field_0x48 = 3;
        (*ptr).field_0x50 = 555;
        (*ptr).field_0x54 = 666;
        (*ptr).donation_count_this_period = 12.5;
        (*ptr).current_month_index = 5;
        (*ptr).current_year_index = 7;
        for (row, months) in (*ptr).monthly_history.iter_mut().enumerate() {
            for (col, v) in months.iter_mut().enumerate() {
                *v = row as f32 * 100.0 + col as f32;
            }
        }
        for (row, years) in (*ptr).yearly_history.iter_mut().enumerate() {
            for (col, v) in years.iter_mut().enumerate() {
                *v = row as f32 * 1000.0 + col as f32 * 10.0;
            }
        }
        for (i, v) in (*ptr).flat_totals.iter_mut().enumerate() {
            *v = i as f32 * 7.5;
        }
        (*ptr).admission_price = 49.5;
        (*ptr).last_animal_escape_timestamp_low = 0xdeadbeef;
        (*ptr).last_animal_escape_timestamp_high = 0x12345678;
    }
}

/// `ZOOSTATUS_SAVE_LOAD` - `zoostatus-implementation-plan.md` Stage 7's live comparison for
/// [`ZooStatus::save`]/[`ZooStatus::load`] (current-version path only, `version >= 0x47` - see
/// [`ZooStatus::load`]'s own doc comment for why older versions are out of this stage's scope).
///
/// Builds two standalone `ZTGameMgr` blocks (same harness `ZOOSTATUS_INIT`/`ZOOSTATUS_OVERRIDE`
/// use), zeroes each one's embedded `ZooStatus` sub-region, then seeds both identically via
/// [`seed_zoostatus_for_save_load`]. Runs real `SAVE.original()` against one and [`ZooStatus::save`]
/// against the other, both captured via `io_redirect` - the two byte streams must be identical,
/// since a real-vs-reimpl format difference would otherwise only surface later, indirectly, as a
/// `load` mismatch. Then builds two more fresh, zeroed standalone blocks and replays each side's
/// own captured bytes into real `LOAD.original()`/[`ZooStatus::load`] respectively
/// (`version = 0x47`, the exact boundary this stage's scope claims to support), and full-struct
/// byte-compares the results - `load` only ever writes into fields `save` persisted, so a
/// fresh-zeroed destination needs no masking: every byte either round-trips to its seeded value or
/// stays zero on both sides identically.
pub(crate) fn run_zoostatus_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_SAVE_LOAD";

    let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})", test_name, real_mgr_ptr, reimpl_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null (real={:?}, reimpl={:?})\n", test_name, real_mgr_ptr, reimpl_mgr_ptr).as_bytes());
        }
        if !real_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        }
        if !reimpl_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        }
        return true;
    }

    let zoostatus_size = size_of::<ZooStatus>();
    let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;

    unsafe {
        std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);
    }
    seed_zoostatus_for_save_load(real_zoostatus_ptr);
    seed_zoostatus_for_save_load(reimpl_zoostatus_ptr);

    let dummy_file: u32 = 0;

    io_redirect::begin_capture();
    unsafe { ZOOSTATUS_SAVE.original()(real_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const i8) };
    let real_bytes = io_redirect::end_capture();

    io_redirect::begin_capture();
    let _ = unsafe { (*reimpl_zoostatus_ptr).save(&dummy_file as *const u32 as *const i8) };
    let reimpl_bytes = io_redirect::end_capture();

    gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);

    if real_bytes != reimpl_bytes {
        error!("{}: save byte mismatch, real len={} reimpl len={}", test_name, real_bytes.len(), reimpl_bytes.len());
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: save byte mismatch, real len={} reimpl len={}\n", test_name, real_bytes.len(), reimpl_bytes.len()).as_bytes());
        }
        return true;
    }

    let real_load_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    let reimpl_load_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
    if real_load_mgr_ptr.is_null() || reimpl_load_mgr_ptr.is_null() {
        error!("{}: CREATE_ZTGAME_MGR returned null for the load pair (real={:?}, reimpl={:?})", test_name, real_load_mgr_ptr, reimpl_load_mgr_ptr);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null for the load pair\n", test_name).as_bytes());
        }
        if !real_load_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(real_load_mgr_ptr);
        }
        if !reimpl_load_mgr_ptr.is_null() {
            gamemgr_live_support::destroy_standalone_mgr(reimpl_load_mgr_ptr);
        }
        return true;
    }

    let real_load_zoostatus_ptr = (real_load_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    let reimpl_load_zoostatus_ptr = (reimpl_load_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
    unsafe {
        std::ptr::write_bytes(real_load_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        std::ptr::write_bytes(reimpl_load_zoostatus_ptr as *mut u8, 0, zoostatus_size);
    }

    const CURRENT_VERSION: u32 = 0x47;

    io_redirect::begin_replay(real_bytes.clone());
    let real_load_ok = unsafe { ZOOSTATUS_LOAD.original()(real_load_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const u8, CURRENT_VERSION) };
    io_redirect::end_replay();

    io_redirect::begin_replay(reimpl_bytes.clone());
    let reimpl_load_ok = unsafe { (*reimpl_load_zoostatus_ptr).load(&dummy_file as *const u32, CURRENT_VERSION) };
    io_redirect::end_replay();

    let mut failed = false;
    if (real_load_ok & 0xff != 0) != (reimpl_load_ok & 0xff != 0) {
        error!("{}: load ok mismatch (real={:#x}, reimpl={:#x})", test_name, real_load_ok, reimpl_load_ok);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: load ok mismatch (real={:#x}, reimpl={:#x})\n", test_name, real_load_ok, reimpl_load_ok).as_bytes());
        }
        failed = true;
    }

    let real_bytes_after = unsafe { std::slice::from_raw_parts(real_load_zoostatus_ptr as *const u8, zoostatus_size) };
    let reimpl_bytes_after = unsafe { std::slice::from_raw_parts(reimpl_load_zoostatus_ptr as *const u8, zoostatus_size) };
    let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
        .filter_map(|i| if real_bytes_after[i] != reimpl_bytes_after[i] { Some((i, real_bytes_after[i], reimpl_bytes_after[i])) } else { None })
        .collect();
    if !mismatches.is_empty() {
        let shown = &mismatches[..mismatches.len().min(32)];
        error!("{}: {} byte mismatch(es) after load (offset, real, reimpl), first {}: {:?}", test_name, mismatches.len(), shown.len(), shown);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {} byte mismatch(es) after load, first {}: {:?}\n", test_name, mismatches.len(), shown.len(), shown).as_bytes());
        }
        failed = true;
    }

    if !failed {
        write_success_line(failure_log, test_name);
    }

    gamemgr_live_support::destroy_standalone_mgr(real_load_mgr_ptr);
    gamemgr_live_support::destroy_standalone_mgr(reimpl_load_mgr_ptr);
    failed
}

/// Builds a synthetic pre-`0x47` `ZooStatus::load` byte stream by hand, following real vanilla's own
/// exact per-version read order (`ZooStatus_load.c`/`.asm`, see [`ZooStatus::load`]'s own doc comment
/// for the full per-threshold breakdown this mirrors): every field/slot a given `version` actually
/// reads gets a distinct, recognizable value (`category*100+month`-style for history slots) so an
/// offset/loop-bound bug anywhere surfaces as a mismatch rather than being masked by a shared value.
/// `version < 0x26` slots are encoded as raw `i32` (matching that range's genuine `FILD` int->float
/// conversion); `version >= 0x26` slots are encoded as literal `f32` bytes (matching that range's plain
/// reinterpreting read) - using the wrong encoding for a given version would make real vanilla's own
/// `LOAD.original()` interpret the stream differently than intended, so this distinction is load-bearing
/// for the test's own correctness, not just style.
fn build_legacy_load_bytes(version: u32, category_count: i32, year_count: i32) -> Vec<u8> {
    let mut buf = Vec::new();
    if version <= 0xc {
        return buf;
    }

    buf.extend_from_slice(&111i32.to_le_bytes()); // rating_check_elapsed
    buf.extend_from_slice(&222i32.to_le_bytes()); // message_check_elapsed
    buf.extend_from_slice(&333i32.to_le_bytes()); // newguest_check_elapsed

    if version < 0x17 {
        buf.extend_from_slice(&400_000i32.to_le_bytes()); // > 360000 -> finance_check_pending = true
    } else {
        buf.push(1u8); // finance_check_pending
    }

    buf.extend_from_slice(&42i32.to_le_bytes()); // zoo_rating_current
    buf.extend_from_slice(&3i32.to_le_bytes()); // field_0x48
    buf.extend_from_slice(&555i32.to_le_bytes()); // field_0x50
    buf.extend_from_slice(&666i32.to_le_bytes()); // field_0x54

    if version < 0x18 {
        buf.extend_from_slice(&0xdeadi32.to_le_bytes()); // discarded
    }

    buf.extend_from_slice(&12.5f32.to_le_bytes()); // donation_count_this_period

    if version < 0x17 {
        return buf;
    }

    buf.extend_from_slice(&5i32.to_le_bytes()); // current_month_index
    buf.extend_from_slice(&7i32.to_le_bytes()); // current_year_index
    buf.extend_from_slice(&category_count.to_le_bytes());
    buf.extend_from_slice(&year_count.to_le_bytes());

    if version < 0x26 {
        for category in 0..category_count.max(0) {
            for month in 0..12 {
                buf.extend_from_slice(&(category * 100 + month).to_le_bytes());
            }
        }
        for category in 0..category_count.max(0) {
            for year in 0..year_count.max(0) {
                buf.extend_from_slice(&(category * 1000 + year * 10).to_le_bytes());
            }
        }
        for category in 0..category_count.max(0) {
            buf.extend_from_slice(&(category * 7).to_le_bytes());
        }
    } else {
        for category in 0..category_count.max(0) {
            for month in 0..12 {
                buf.extend_from_slice(&(category as f32 * 100.0 + month as f32).to_le_bytes());
            }
        }
        for category in 0..category_count.max(0) {
            for year in 0..year_count.max(0) {
                buf.extend_from_slice(&(category as f32 * 1000.0 + year as f32 * 10.0).to_le_bytes());
            }
        }
        for category in 0..category_count.max(0) {
            buf.extend_from_slice(&(category as f32 * 7.5).to_le_bytes());
        }
    }

    if version >= 0x27 {
        buf.extend_from_slice(&49.5f32.to_le_bytes()); // admission_price
    }

    if version < 0x47 {
        return buf;
    }

    buf.extend_from_slice(&0xdeadbeefu32.to_le_bytes());
    buf.extend_from_slice(&0x12345678u32.to_le_bytes());

    buf
}

/// `ZOOSTATUS_LOAD_LEGACY` - Stage 9 of `zoostatus-implementation-plan.md`: live comparison of
/// [`ZooStatus::load`]'s full pre-`0x47` version range (`ZOOSTATUS_SAVE_LOAD` already covers the
/// `>= 0x47` current-format path). For each `(version, category_count, year_count)` below - chosen to
/// exercise every threshold this stage added, plus the `category_count`/`year_count` edge shapes real
/// old saves could plausibly hit - builds a synthetic byte stream via [`build_legacy_load_bytes`],
/// replays the identical bytes into a fresh, zeroed-then-identically-seeded standalone `ZooStatus` via
/// both real `LOAD.original()` and this port's [`ZooStatus::load`], and full-struct byte-compares the
/// result (masking the escape timestamp for `version < 0x47`, since both sides re-seed it from a live,
/// non-deterministic [`GET_OLD_DATE`] call rather than reading it from the stream).
///
/// Cases, by what each is chosen to exercise:
/// - `version = 8`: below the `0xc` floor - nothing read at all, seeded sentinel values must survive
///   untouched.
/// - `version = 0x10`: header-scalars-only path (`< 0x17`), including the `raw > 360_000` derivation
///   for `finance_check_pending` and the pre-`0x18` discard read.
/// - `version = 0x17`, `category_count = 10`, `year_count = 15`: enters the real migration path
///   (`< 0x26`) with the pre-`0x19` row-14 differencing active, and `category_count < 15` so row 14's
///   differenced value is never clobbered by that category's own straight write (see
///   [`ZooStatus::load`]'s own doc comment for why `>= 15` would clobber it).
/// - `version = 0x19`, `category_count = 35`, `year_count = 25`: still the migration path but past the
///   differencing cutoff (pure `i32`->`f32` conversion only), with `category_count`/`year_count` both
///   deliberately over this struct's own `31`/`20` geometry to exercise per-slot discard.
/// - `version = 0x26`, `category_count = 5`, `year_count = 3`: the new generalized-but-permissive
///   straight-copy path, short enough to exercise the trailing zero-fill.
/// - `version = 0x2a`, `category_count = 31`, `year_count = 20`: same path at this struct's own exact
///   geometry, past the `0x27` admission-price-read threshold.
/// - `version = 0x46`: same path, right at the boundary below the already-covered `0x47` fast path.
pub(crate) fn run_zoostatus_load_legacy_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZOOSTATUS_LOAD_LEGACY";
    let dummy_file: u32 = 0;
    let zoostatus_size = size_of::<ZooStatus>();

    let cases: [(u32, i32, i32); 7] = [
        (8, 0, 0),
        (0x10, 0, 0),
        (0x17, 10, 15),
        (0x19, 35, 25),
        (0x26, 5, 3),
        (0x2a, 31, 20),
        (0x46, 31, 20),
    ];

    let mut any_failed = false;

    for (version, category_count, year_count) in cases {
        let real_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        let reimpl_mgr_ptr = gamemgr_live_support::build_standalone_mgr();
        if real_mgr_ptr.is_null() || reimpl_mgr_ptr.is_null() {
            error!("{}: CREATE_ZTGAME_MGR returned null for version {:#x} (real={:?}, reimpl={:?})", test_name, version, real_mgr_ptr, reimpl_mgr_ptr);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: CREATE_ZTGAME_MGR returned null for version {:#x}\n", test_name, version).as_bytes());
            }
            if !real_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
            }
            if !reimpl_mgr_ptr.is_null() {
                gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
            }
            any_failed = true;
            continue;
        }

        let real_zoostatus_ptr = (real_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        let reimpl_zoostatus_ptr = (reimpl_mgr_ptr as u32 + 0x10) as *mut ZooStatus;
        unsafe {
            std::ptr::write_bytes(real_zoostatus_ptr as *mut u8, 0, zoostatus_size);
            std::ptr::write_bytes(reimpl_zoostatus_ptr as *mut u8, 0, zoostatus_size);
        }
        // Seed a recognizable non-zero baseline on both sides identically, so a version whose real
        // control flow leaves some region untouched (e.g. `version <= 0xc`, or any category/year row
        // past `category_count`/`year_count`) proves it by both sides keeping this sentinel, not by
        // both merely staying zero (which a bug that skips a *write* could also produce).
        seed_zoostatus_for_save_load(real_zoostatus_ptr);
        seed_zoostatus_for_save_load(reimpl_zoostatus_ptr);

        let bytes = build_legacy_load_bytes(version, category_count, year_count);

        io_redirect::begin_replay(bytes.clone());
        let real_ok = unsafe { ZOOSTATUS_LOAD.original()(real_zoostatus_ptr as *const u32, &dummy_file as *const u32 as *const u8, version) };
        io_redirect::end_replay();

        io_redirect::begin_replay(bytes);
        let reimpl_ok = unsafe { (*reimpl_zoostatus_ptr).load(&dummy_file as *const u32, version) };
        io_redirect::end_replay();

        let mut failed = false;
        if (real_ok & 0xff != 0) != (reimpl_ok & 0xff != 0) {
            error!("{}: version {:#x} load ok mismatch (real={:#x}, reimpl={:#x})", test_name, version, real_ok, reimpl_ok);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: version {:#x} load ok mismatch (real={:#x}, reimpl={:#x})\n", test_name, version, real_ok, reimpl_ok).as_bytes());
            }
            failed = true;
        }

        let real_bytes_after = unsafe { std::slice::from_raw_parts(real_zoostatus_ptr as *const u8, zoostatus_size) };
        let reimpl_bytes_after = unsafe { std::slice::from_raw_parts(reimpl_zoostatus_ptr as *const u8, zoostatus_size) };
        let timestamp_offset = std::mem::offset_of!(ZooStatus, last_animal_escape_timestamp_low);
        let mismatches: Vec<(usize, u8, u8)> = (0..zoostatus_size)
            .filter(|&i| version >= 0x47 || !(timestamp_offset..timestamp_offset + 8).contains(&i))
            .filter_map(|i| if real_bytes_after[i] != reimpl_bytes_after[i] { Some((i, real_bytes_after[i], reimpl_bytes_after[i])) } else { None })
            .collect();
        if !mismatches.is_empty() {
            let shown = &mismatches[..mismatches.len().min(32)];
            error!("{}: version {:#x} (category_count={}, year_count={}) {} byte mismatch(es) (offset, real, reimpl), first {}: {:?}", test_name, version, category_count, year_count, mismatches.len(), shown.len(), shown);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: version {:#x} (category_count={}, year_count={}) {} byte mismatch(es), first {}: {:?}\n", test_name, version, category_count, year_count, mismatches.len(), shown.len(), shown).as_bytes());
            }
            failed = true;
        }

        gamemgr_live_support::destroy_standalone_mgr(real_mgr_ptr);
        gamemgr_live_support::destroy_standalone_mgr(reimpl_mgr_ptr);
        any_failed |= failed;
    }

    if !any_failed {
        write_success_line(failure_log, test_name);
    }
    any_failed
}
