//! `zoostatus` module - vanilla `ZooStatus` reimplementation.
//! ZooStatus is the finance/rating tracker `ZTGameMgr` embeds inline at its own +0x10.

pub mod config;
pub mod globals;
#[allow(clippy::module_inception)]
pub mod zoostatus;

pub use zoostatus::*;

use std::ffi::c_void;
use openzt_detour::generated::zoostatus::*;
use openzt_detour_macro::detour_mod;
use tracing::error;
use crate::{
    lua_fn,
    util::{mut_from_memory, ref_from_memory},
};

/// Stage 8 (extended by Stage 10) of the implementation plan: real detours for 36 of `zoostatus`'s 39
/// post-macOS-regen `generated.rs` addresses this file has a Rust port for, each routed onto the
/// `impl ZooStatus` method (or free function, for [`F_ZOO_MESSAGE`]'s this-less helper) of the same
/// name. Deliberately left un-hooked: the three addresses the plan's Status header documents as
/// real-vanilla call-throughs ([`FINANCE_CHECKS`]/[`F_CREATE_GUEST`]/[`F_CHANCE`] - blocked on
/// `ZTWorldMgr`/`ZTBuilding` reimplementation or on shared-RNG-stream parity, see [`ZooStatus::update`]'s
/// own doc comment). Stage 10 added the last five real Windows methods a fresh Ghidra pass recovered from
/// the macOS-only corpus (`GET_STATUS`/`HEAL_ANIMAL`/`PURCHASE_FOOD`/`INCREASE_ADMISSIONS`/
/// `INCREASE_ADMISSIONS_INCOME`) and renamed [`BUY_ANIMAL`]'s detour function from its old, partly-wrong
/// Stage 3 name (`spend_keeper_wages_0`) to [`ZooStatus::buy_animal`] - see that method's own doc comment
/// for the mislabeling this corrects.
///
/// No destructor to worry about (see the module's "Style decision" - `ZooStatus` has no vtable, no
/// separate constructor, and lives/dies with its enclosing `ZTGameMgr` block), so this is a single flat
/// block rather than the constructor/mutator/save-load split larger classes (`ZTThoughtMgr`) use.
#[detour_mod]
mod zoostatus_detours {
    use super::*;

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(this: *const u32, config: *const c_void) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.init(config);
    }

    #[detour(OVERRIDE)]
    unsafe extern "thiscall" fn override_config(this: *const u32, config: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.override_config(config as *const c_void);
    }

    #[detour(RESET_FINANCE_INFO)]
    unsafe extern "thiscall" fn reset_finance_info(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.reset_finance_info();
    }

    #[detour(SPEND_CONSTRUCTION)]
    unsafe extern "thiscall" fn spend_construction(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_construction(amount);
    }

    #[detour(SPEND_BUILDING_UPKEEP)]
    unsafe extern "thiscall" fn spend_building_upkeep(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_building_upkeep(amount);
    }

    #[detour(SPEND_GUIDE_WAGES)]
    unsafe extern "thiscall" fn spend_guide_wages(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_guide_wages(amount);
    }

    #[detour(BUY_ANIMAL)]
    unsafe extern "thiscall" fn buy_animal(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.buy_animal(amount);
    }

    #[detour(SPEND_KEEPER_WAGES)]
    unsafe extern "thiscall" fn spend_keeper_wages_1(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_keeper_wages_1(amount);
    }

    #[detour(SPEND_MAINT_WAGES)]
    unsafe extern "thiscall" fn spend_maint_wages(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_maint_wages(amount);
    }

    #[detour(SPEND_MARKETING)]
    unsafe extern "thiscall" fn spend_marketing(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_marketing(amount);
    }

    #[detour(SPEND_RESEARCH)]
    unsafe extern "thiscall" fn spend_research(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.spend_research(amount);
    }

    #[detour(REFUND_ANIMAL_COST)]
    unsafe extern "thiscall" fn refund_animal_cost(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.refund_animal_cost(amount);
    }

    #[detour(REFUND_CONSTRUCTION)]
    unsafe extern "thiscall" fn refund_construction(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.refund_construction(amount);
    }

    #[detour(INCREASE_DONATIONS)]
    unsafe extern "thiscall" fn increase_donations(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.increase_donations(amount);
    }

    #[detour(INCREASE_ENDOWMENT)]
    unsafe extern "thiscall" fn increase_endowment(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.increase_endowment(amount);
    }

    #[detour(INCREASE_SHOW_ADMISSION)]
    unsafe extern "thiscall" fn increase_show_admission(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.increase_show_admission(amount);
    }

    #[detour(BUY_PEOPLE_FOOD)]
    unsafe extern "thiscall" fn buy_people_food(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.buy_people_food(amount);
    }

    #[detour(CHANGE_ENDOWMENT_MEMBERS)]
    unsafe extern "thiscall" fn change_endowment_members(this: *const u32, delta: i32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.change_endowment_members(delta);
    }

    /// `fastcall`/single-`this`-register (per the plan's "Method inventory" table), declared `i32` in
    /// `generated.rs` rather than a pointer type - same shape as `ztgamemgr.rs`'s `START`/`STOP`
    /// detours, see [`ZooStatus::animal_escaped`]'s own doc comment.
    #[detour(ANIMAL_ESCAPED)]
    unsafe extern "fastcall" fn animal_escaped(this: i32) {
        unsafe { mut_from_memory::<ZooStatus>(this as *const u32) }.animal_escaped();
    }

    #[detour(ADMISSION_MESSAGE)]
    unsafe extern "thiscall" fn admission_message(this: *const u32, message_id: *const u32, param: u32) {
        unsafe { ref_from_memory::<ZooStatus>(this) }.admission_message(message_id, param);
    }

    #[detour(NEWGUEST_CHECKS)]
    unsafe extern "thiscall" fn newguest_checks(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.newguest_checks();
    }

    #[detour(MESSAGE_CHECKS)]
    unsafe extern "thiscall" fn message_checks(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.message_checks();
    }

    #[detour(RATING_CHECKS)]
    unsafe extern "thiscall" fn rating_checks(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.rating_checks();
    }

    #[detour(F_GRANT_DONATION)]
    unsafe extern "thiscall" fn f_grant_donation(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.f_grant_donation();
    }

    /// A this-less free-standing helper (per the plan's "Method inventory" table) - routes onto
    /// [`super::f_zoo_message`] directly rather than a `ZooStatus` method.
    #[detour(F_ZOO_MESSAGE)]
    unsafe extern "stdcall" fn f_zoo_message(message_id: *const u32, param_2: u32, tile: u32, entity: i32) {
        super::f_zoo_message(message_id, param_2, tile, entity);
    }

    #[detour(SET_ADULT_ADMISSION_PRICE)]
    unsafe extern "thiscall" fn set_adult_admission_price(this: *const u32, price: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.set_adult_admission_price(price);
    }

    /// `ZooStatus_showPrices.asm` (read in full) never touches the incoming `this`/ECX past its
    /// entry `PUSH %ECX` (pure stack-balancing, restored unused via the matching tail `POP %ECX`) -
    /// it instead reloads `GLOBAL_ZTGameMgr` itself and computes `ESI = GLOBAL_ZTGameMgr + 0x10`, the
    /// same "ignore `this`, use the live global" quirk [`ZooStatus::f_grant_donation`]'s own doc
    /// comment already documents for its donation-grant branch. The real caller's ECX at this call
    /// site is therefore never a `ZooStatus*` at all (observed live: an unrelated, non-pointer-aligned
    /// value), so treating `this` as one - as the previous version of this detour did - reads/derefs
    /// garbage and crashes. Match vanilla: derive the pointer from the global instead of from `this`.
    #[detour(SHOW_PRICES)]
    unsafe extern "thiscall" fn show_prices(_this: *const u32) {
        let ztgamemgr_ptr = crate::globals::globals().ztgamemgr_ptr();
        let zoostatus_ptr = (ztgamemgr_ptr as u32 + 0x10) as *const ZooStatus;
        unsafe { &*zoostatus_ptr }.show_prices();
    }

    #[detour(CALCULATE_SUMS)]
    unsafe extern "thiscall" fn calculate_sums(this: *const u32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.calculate_sums();
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, delta: i32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.update(delta);
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const i8) -> bool {
        unsafe { ref_from_memory::<ZooStatus>(this) }.save(file) != 0
    }

    /// `file` is declared `*const u8` in `generated.rs`; [`ZooStatus::load`] takes `*const u32` (matching
    /// `ztgamemgr.rs`'s own `load`'s file-handle type) - cast only, same handle either way.
    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u8, version: u32) -> bool {
        unsafe { mut_from_memory::<ZooStatus>(this) }.load(file as *const u32, version) != 0
    }

    #[detour(HEAL_ANIMAL)]
    unsafe extern "thiscall" fn heal_animal(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.heal_animal(amount);
    }

    #[detour(PURCHASE_FOOD)]
    unsafe extern "thiscall" fn purchase_food(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.purchase_food(amount);
    }

    #[detour(INCREASE_ADMISSIONS_INCOME)]
    unsafe extern "thiscall" fn increase_admissions_income(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.increase_admissions_income(amount);
    }

    #[detour(INCREASE_ADMISSIONS)]
    unsafe extern "thiscall" fn increase_admissions(this: *const u32, count: i32) {
        unsafe { mut_from_memory::<ZooStatus>(this) }.increase_admissions(count);
    }

    #[detour(GET_STATUS)]
    unsafe extern "thiscall" fn get_status(this: *const u32, category: i32, when: i32, index: i32) -> f32 {
        unsafe { ref_from_memory::<ZooStatus>(this) }.get_status(category, when, index)
    }
}

/// Registers this module's 36 live detours (see [`zoostatus_detours`]'s own doc comment for what's
/// deliberately excluded).
pub fn init() {
    if let Err(e) = unsafe { zoostatus_detours::init_detours() } {
        error!("Failed to initialise zoostatus detours: {e:?}");
    }

    lua_fn!(
        "check_calc_sums_fixture",
        "Checks whether the currently loaded zoo exercises every ZooStatus::calculate_sums/message_checks/rating_checks branch (for authoring live reimplementation tests)",
        "check_calc_sums_fixture()",
        || { Ok(describe_calculate_sums_fixture()) }
    );

    lua_fn!(
        "fixup_calc_sums_fixture",
        "Rewrites existing guest/animal entities' need/condition values so check_calc_sums_fixture()'s value-based conditions pass - save the game afterward to bake it into a .zoo",
        "fixup_calc_sums_fixture()",
        || { Ok(fixup_calc_sums_fixture()) }
    );
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// `(name, is_enabled)` per detour - see `zoostatus_detours::status`.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        zoostatus_detours::status()
    }
}

#[cfg(test)]
mod tests {
    use std::mem::{self, offset_of};

    use super::*;

    #[test]
    fn size_matches_confirmed_struct_tail() {
        assert_eq!(mem::size_of::<ZooStatus>(), 0x1180);
    }

    #[test]
    fn scalar_field_offsets_match_confirmed_asm_evidence() {
        assert_eq!(offset_of!(ZooStatus, rating_check_interval), 0x00);
        assert_eq!(offset_of!(ZooStatus, message_check_interval), 0x04);
        assert_eq!(offset_of!(ZooStatus, newguest_check_interval), 0x08);
        assert_eq!(offset_of!(ZooStatus, rating_check_elapsed), 0x0c);
        assert_eq!(offset_of!(ZooStatus, message_check_elapsed), 0x10);
        assert_eq!(offset_of!(ZooStatus, newguest_check_elapsed), 0x14);
        assert_eq!(offset_of!(ZooStatus, finance_check_pending), 0x18);
        assert_eq!(offset_of!(ZooStatus, zoo_rating_current), 0x1c);
        assert_eq!(offset_of!(ZooStatus, num_animals), 0x20);
        assert_eq!(offset_of!(ZooStatus, animal_condition_counter_1), 0x24);
        assert_eq!(offset_of!(ZooStatus, num_species), 0x28);
        assert_eq!(offset_of!(ZooStatus, num_tired_guests), 0x2c);
        assert_eq!(offset_of!(ZooStatus, num_hungry_guests), 0x30);
        assert_eq!(offset_of!(ZooStatus, num_thirst_guests), 0x34);
        assert_eq!(offset_of!(ZooStatus, num_guests_restroom_need), 0x38);
        assert_eq!(offset_of!(ZooStatus, guest_condition_counter_1), 0x3c);
        assert_eq!(offset_of!(ZooStatus, guest_condition_counter_2), 0x40);
        assert_eq!(offset_of!(ZooStatus, guest_tile_count), 0x44);
        assert_eq!(offset_of!(ZooStatus, field_0x48), 0x48);
        assert_eq!(offset_of!(ZooStatus, field_0x4c), 0x4c);
        assert_eq!(offset_of!(ZooStatus, field_0x50), 0x50);
        assert_eq!(offset_of!(ZooStatus, field_0x54), 0x54);
        assert_eq!(offset_of!(ZooStatus, field_0x58), 0x58);
        assert_eq!(offset_of!(ZooStatus, animal_rating_metric), 0x5c);
        assert_eq!(offset_of!(ZooStatus, guest_rating_metric), 0x60);
        assert_eq!(offset_of!(ZooStatus, non_blank_tile_fraction), 0x64);
        assert_eq!(offset_of!(ZooStatus, max_guests), 0x68);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x70), 0x70);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x74), 0x74);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x7c), 0x7c);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x84), 0x84);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x8c), 0x8c);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0x94), 0x94);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0xa0), 0xa0);
        assert_eq!(offset_of!(ZooStatus, message_threshold_0xa8), 0xa8);
        assert_eq!(offset_of!(ZooStatus, guest_type_arrival_multiplier), 0xac);
        assert_eq!(offset_of!(ZooStatus, donation_count_this_period), 0x120);
        assert_eq!(offset_of!(ZooStatus, donation_count_bound), 0x124);
        assert_eq!(offset_of!(ZooStatus, donation_amount_min), 0x128);
        assert_eq!(offset_of!(ZooStatus, donation_amount_max), 0x12c);
        assert_eq!(offset_of!(ZooStatus, donation_chance_percent), 0x130);
        assert_eq!(offset_of!(ZooStatus, species_rating_cap), 0x134);
        assert_eq!(offset_of!(ZooStatus, current_month_index), 0x14c);
        assert_eq!(offset_of!(ZooStatus, current_year_index), 0x150);
    }

    #[test]
    fn override_resolved_field_offsets_match_confirmed_config_key_evidence() {
        assert_eq!(offset_of!(ZooStatus, angry_animals_sick_change), 0x6c);
        assert_eq!(offset_of!(ZooStatus, angry_hungry_guests_change), 0x78);
        assert_eq!(offset_of!(ZooStatus, angry_thirsty_guests_change), 0x80);
        assert_eq!(offset_of!(ZooStatus, angry_bathroom_guests_change), 0x88);
        assert_eq!(offset_of!(ZooStatus, angry_souvenir_guests_change), 0x90);
        assert_eq!(offset_of!(ZooStatus, angry_remove_animal_change), 0x98);
        assert_eq!(offset_of!(ZooStatus, angry_tired_guests_change), 0x9c);
        assert_eq!(offset_of!(ZooStatus, angry_trash_guests_change), 0xa4);

        assert_eq!(offset_of!(ZooStatus, loan_available), 0xc0);
        assert_eq!(offset_of!(ZooStatus, high_zoo_value_change), 0xc4);
        assert_eq!(offset_of!(ZooStatus, low_zoo_value_change), 0xc8);
        assert_eq!(offset_of!(ZooStatus, high_zoo_value), 0xcc);
        assert_eq!(offset_of!(ZooStatus, low_zoo_value), 0xd0);
        assert_eq!(offset_of!(ZooStatus, high_species_threshold), 0xd4);
        assert_eq!(offset_of!(ZooStatus, happy_diverse_animals_change), 0xd8);
        assert_eq!(offset_of!(ZooStatus, low_species_threshold), 0xdc);
        assert_eq!(offset_of!(ZooStatus, angry_diverse_animals_change), 0xe0);
        assert_eq!(offset_of!(ZooStatus, high_avg_animal_happy_threshold), 0xe4);
        assert_eq!(offset_of!(ZooStatus, happy_animals_change), 0xe8);
        assert_eq!(offset_of!(ZooStatus, low_avg_animal_happy_threshold), 0xec);
        assert_eq!(offset_of!(ZooStatus, angry_animals_change), 0xf0);
        assert_eq!(offset_of!(ZooStatus, high_avg_guest_happy_threshold), 0xf4);
        assert_eq!(offset_of!(ZooStatus, happy_guest_change), 0xf8);
        assert_eq!(offset_of!(ZooStatus, low_avg_guest_happy_threshold), 0xfc);
        assert_eq!(offset_of!(ZooStatus, angry_guest_change), 0x100);
        assert_eq!(offset_of!(ZooStatus, item_cheap), 0x104);
        assert_eq!(offset_of!(ZooStatus, item_expensive), 0x108);
        assert_eq!(offset_of!(ZooStatus, high_zoo_esthetic), 0x10c);
        assert_eq!(offset_of!(ZooStatus, high_zoo_esthetic_change), 0x110);
        assert_eq!(offset_of!(ZooStatus, low_zoo_esthetic), 0x114);
        assert_eq!(offset_of!(ZooStatus, low_zoo_esthetic_change), 0x118);
        assert_eq!(offset_of!(ZooStatus, research_cost), 0x11c);

        assert_eq!(offset_of!(ZooStatus, membership_join_happiness), 0x138);
        assert_eq!(offset_of!(ZooStatus, membership_join_factor), 0x13c);
        assert_eq!(offset_of!(ZooStatus, endowment_gift_low), 0x140);
        assert_eq!(offset_of!(ZooStatus, endowment_gift_high), 0x144);
        assert_eq!(offset_of!(ZooStatus, membership_join_chance), 0x148);

        assert_eq!(offset_of!(ZooStatus, pricing_factor), 0x115c);
        assert_eq!(offset_of!(ZooStatus, donation_factor), 0x1160);
        assert_eq!(offset_of!(ZooStatus, building_use_cost_default), 0x1164);
        assert_eq!(offset_of!(ZooStatus, building_use_cost_max), 0x1168);
        assert_eq!(offset_of!(ZooStatus, zoo_doo_recycling_amount), 0x116c);
    }

    #[test]
    fn history_region_offsets_match_confirmed_zero_loop_geometry() {
        assert_eq!(offset_of!(ZooStatus, monthly_history), 0x154);
        assert_eq!(mem::size_of::<[[f32; 12]; 31]>(), 0x5d0);
        assert_eq!(offset_of!(ZooStatus, yearly_history), 0x724);
        assert_eq!(mem::size_of::<[[f32; 20]; 31]>(), 0x9b0);
        assert_eq!(offset_of!(ZooStatus, flat_totals), 0x10d4);
        assert_eq!(mem::size_of::<[f32; 31]>(), 0x7c);
    }

    #[test]
    fn tail_field_offsets_match_confirmed_asm_evidence() {
        assert_eq!(offset_of!(ZooStatus, admission_price), 0x1150);
        assert_eq!(offset_of!(ZooStatus, admission_price_min), 0x1154);
        assert_eq!(offset_of!(ZooStatus, admission_price_max), 0x1158);
        assert_eq!(offset_of!(ZooStatus, admission_income_multiplier), 0x1170);
        assert_eq!(offset_of!(ZooStatus, research_completion_percent), 0x1174);
        assert_eq!(offset_of!(ZooStatus, last_animal_escape_timestamp_low), 0x1178);
        assert_eq!(offset_of!(ZooStatus, last_animal_escape_timestamp_high), 0x117c);
    }

    #[test]
    fn zero_history_regions_clears_exactly_the_three_regions() {
        let mut buf = [0xAAu8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };

        status.zero_history_regions();

        assert!(buf[0x154..0x724].iter().all(|&b| b == 0), "monthly_history not fully zeroed");
        assert!(buf[0x724..0x10d4].iter().all(|&b| b == 0), "yearly_history not fully zeroed");
        assert!(buf[0x10d4..0x1150].iter().all(|&b| b == 0), "flat_totals not fully zeroed");
        assert!(buf[0x150..0x154].iter().all(|&b| b == 0xAA), "byte just before monthly_history was touched");
        assert!(buf[0x1150..0x1154].iter().all(|&b| b == 0xAA), "byte just after flat_totals was touched");
    }

    /// Asserts that, of the whole struct, exactly the given `(offset, expected_value)` `f32` slots
    /// changed from `before` to `after` - everything else must be byte-identical. Used by the Stage 3
    /// accumulator tests below to confirm each method's write shape touches its own 4-6 slots (per the
    /// plan's per-method offset table) and nothing else.
    fn assert_only_offsets_changed(before: &[u8], after: &[u8], changed: &[(usize, f32)]) {
        for &(offset, expected) in changed {
            let actual = f32::from_le_bytes(after[offset..offset + 4].try_into().unwrap());
            assert_eq!(actual, expected, "offset {:#x}: expected {}, got {}", offset, expected, actual);
        }
        let changed_ranges: Vec<std::ops::Range<usize>> = changed.iter().map(|&(o, _)| o..o + 4).collect();
        for i in 0..after.len() {
            if changed_ranges.iter().any(|r| r.contains(&i)) {
                continue;
            }
            assert_eq!(after[i], before[i], "unexpected byte change at offset {:#x}", i);
        }
    }

    #[test]
    fn spend_construction_touches_exactly_its_own_and_shared_slots() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.current_month_index = 2;
        status.current_year_index = 3;
        let before = buf;

        status.spend_construction(5.0);

        let month_off = 2 * 4;
        let year_off = 3 * 4;
        assert_only_offsets_changed(
            &before,
            &buf,
            &[
                (0x1e0 + month_off, 5.0),
                (0x3f0 + month_off, -5.0),
                (0x814 + year_off, 5.0),
                (0xb84 + year_off, -5.0),
                (0x10e0, 5.0),
                (0x110c, -5.0),
            ],
        );
    }

    #[test]
    fn refund_animal_cost_adds_to_the_shared_slot_instead_of_subtracting() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.current_month_index = 1;
        status.current_year_index = 0;
        let before = buf;

        status.refund_animal_cost(7.5);

        let month_off = 1 * 4;
        assert_only_offsets_changed(
            &before,
            &buf,
            &[
                (0x330 + month_off, 7.5),
                (0x3f0 + month_off, 7.5),
                (0xa44, 7.5),
                (0xb84, 7.5),
                (0x10fc, 7.5),
                (0x110c, 7.5),
            ],
        );
    }

    #[test]
    fn change_endowment_members_positive_delta_hits_base_and_positive_triples() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.current_month_index = 4;
        status.current_year_index = 5;
        let before = buf;

        status.change_endowment_members(4);

        let month_off = 4 * 4;
        let year_off = 5 * 4;
        assert_only_offsets_changed(
            &before,
            &buf,
            &[
                (0x540 + month_off, 4.0),
                (0xdb4 + year_off, 4.0),
                (0x1128, 4.0),
                (0x570 + month_off, 4.0),
                (0xe04 + year_off, 4.0),
                (0x112c, 4.0),
            ],
        );
    }

    #[test]
    fn change_endowment_members_negative_delta_hits_base_and_negative_triples() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.current_month_index = 4;
        status.current_year_index = 5;
        let before = buf;

        status.change_endowment_members(-4);

        let month_off = 4 * 4;
        let year_off = 5 * 4;
        assert_only_offsets_changed(
            &before,
            &buf,
            &[
                (0x540 + month_off, -4.0),
                (0xdb4 + year_off, -4.0),
                (0x1128, -4.0),
                // field -= (delta as f32); delta as f32 is negative, so this *adds* abs(delta).
                (0x5a0 + month_off, 4.0),
                (0xe54 + year_off, 4.0),
                (0x1130, 4.0),
            ],
        );
    }

    #[test]
    fn change_endowment_members_zero_delta_hits_only_the_base_triple() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.current_month_index = 4;
        status.current_year_index = 5;
        let before = buf;

        status.change_endowment_members(0);

        let month_off = 4 * 4;
        let year_off = 5 * 4;
        assert_only_offsets_changed(&before, &buf, &[(0x540 + month_off, 0.0), (0xdb4 + year_off, 0.0), (0x1128, 0.0)]);
    }

    #[test]
    fn price_tier_matches_the_confirmed_boundary_chain() {
        // boundary_0=100, boundary_1=80, boundary_2=60, boundary_3=40 - distinct so each branch is
        // exercised unambiguously.
        let boundaries = [100.0, 80.0, 60.0, 40.0];
        assert_eq!(ZooStatus::price_tier(150.0, boundaries), 0, "price > boundary_0");
        assert_eq!(ZooStatus::price_tier(90.0, boundaries), 1, "boundary_1 < price <= boundary_0");
        assert_eq!(ZooStatus::price_tier(70.0, boundaries), 2, "boundary_2 < price <= boundary_1");
        assert_eq!(ZooStatus::price_tier(50.0, boundaries), 3, "boundary_3 < price <= boundary_2");
        assert_eq!(ZooStatus::price_tier(30.0, boundaries), 4, "price <= boundary_3");
        // Exact boundary values: `boundary_0 < price` is strict, so price == boundary_0 falls through to
        // the `boundary_1 < price` check (still true at price=100 > boundary_1=80) - tier 1, not tier 0.
        assert_eq!(ZooStatus::price_tier(100.0, boundaries), 1, "price == boundary_0 falls through to tier 1");
        assert_eq!(ZooStatus::price_tier(60.0, boundaries), 3, "price == boundary_2 is tier 3 (<=, not <)");
        assert_eq!(ZooStatus::price_tier(40.0, boundaries), 4, "price == boundary_3 is tier 4 (<=, not <)");
    }

    #[test]
    fn newguest_dispatch_matches_the_derived_band_tier_table() {
        // Distinct multiplier values so a wrong index is caught, not just a wrong Option/bool.
        let m = [10, 20, 30, 40, 50];
        // One representative attendance value per band (see newguest_checks' own doc comment for the
        // band boundaries: >=0x51, 0x3c..=0x50, 0x1e..=0x3b, <=0x1d).
        let high = 0x60; // >= 0x51
        let mid_high = 0x40; // 0x3c..=0x50
        let mid_low = 0x30; // 0x1e..=0x3b
        let low = 0x10; // <= 0x1d

        // (attendance, price_tier, expected)
        let cases: [(i32, i32, Option<(i32, bool)>); 20] = [
            (low, 0, None),
            (low, 1, None),
            (low, 2, Some((m[0], false))),
            (low, 3, Some((m[1], false))),
            (low, 4, Some((m[1], false))),
            (mid_low, 0, None),
            (mid_low, 1, Some((m[0], false))),
            (mid_low, 2, Some((m[2], false))),
            (mid_low, 3, Some((m[2], false))),
            (mid_low, 4, Some((m[3], true))),
            (mid_high, 0, None),
            (mid_high, 1, Some((m[1], false))),
            (mid_high, 2, Some((m[2], false))),
            (mid_high, 3, Some((m[3], false))),
            (mid_high, 4, Some((m[4], true))),
            (high, 0, Some((m[0], false))),
            (high, 1, Some((m[2], false))),
            (high, 2, Some((m[3], false))),
            (high, 3, Some((m[3], true))),
            (high, 4, Some((m[4], true))),
        ];

        for (attendance, price_tier, expected) in cases {
            assert_eq!(
                ZooStatus::newguest_dispatch(attendance, price_tier, m),
                expected,
                "attendance={:#x}, price_tier={}",
                attendance,
                price_tier
            );
        }
    }

    #[test]
    fn set_adult_admission_price_clamps_into_bounds() {
        let mut buf = [0u8; mem::size_of::<ZooStatus>()];
        let status: &mut ZooStatus = unsafe { &mut *(buf.as_mut_ptr() as *mut ZooStatus) };
        status.admission_price_min = 10.0;
        status.admission_price_max = 100.0;

        status.set_adult_admission_price(50.0);
        assert_eq!(status.admission_price, 50.0, "within bounds: stored as-is");

        status.set_adult_admission_price(500.0);
        assert_eq!(status.admission_price, 100.0, "above max: clamped to max");

        status.set_adult_admission_price(1.0);
        assert_eq!(status.admission_price, 10.0, "below min: clamped to min");

        status.set_adult_admission_price(100.0);
        assert_eq!(status.admission_price, 100.0, "== max: falls through to the < check, stays at max");

        status.set_adult_admission_price(10.0);
        assert_eq!(status.admission_price, 10.0, "== min: the <= min branch stores min itself");
    }

    #[test]
    fn donation_outcome_covers_all_three_branches() {
        // count_after_increment == bound + 1: the one-time "no more donations" trigger.
        assert_eq!(donation_outcome(5.0, 4), DonationOutcome::BoundJustCrossed);
        // count_after_increment <= bound: grant, including the boundary itself.
        assert_eq!(donation_outcome(3.0, 4), DonationOutcome::Grant);
        assert_eq!(donation_outcome(4.0, 4), DonationOutcome::Grant);
        // count_after_increment > bound + 1: already past, silent no-op on every later call this period.
        assert_eq!(donation_outcome(6.0, 4), DonationOutcome::AlreadyPastBound);
        assert_eq!(donation_outcome(100.0, 4), DonationOutcome::AlreadyPastBound);
    }
}
