//! `ZooStatus` reimplementation - Stage 1 (struct only) of
//! `openzt/plans/zoostatus-implementation-plan.md`. `ZooStatus` is the finance/rating tracker
//! `ZTGameMgr` embeds **inline** (not by pointer) at `this+0x10` - it has no vtable, no destructor and
//! no separate constructor (`ZooStatus::init` does 100% of construction plus config-driven `override`
//! at its tail), and it never allocates/frees anything of its own, so none of `CLAUDE.md`'s
//! cross-allocator hazards apply here - the entire risk surface is getting the byte layout right.
//!
//! Every offset below comes from reading `ZooStatus_init.asm`, `_save.c`/`.asm`, `_calculateSums.asm`
//! and every `spend*`/`refund*`/`increase*`/`buyPeopleFood`/`changeEndowmentMembers`/`newguestChecks`/
//! `messageChecks`/`ratingChecks`/`fCreateGuest`/`fGrantDonation`/`setAdultAdmissionPrice`/`showPrices`
//! `.asm` directly (see the plan's "Struct layout" section for the full derivation and per-offset
//! default-value table) - none of it is re-derived here. Fields without a resolved semantic role yet
//! are left as sized-but-unnamed padding; later stages (per the plan's staging) name them as the
//! methods that own them get ported.
//!
//! **Correction to previously-shipped code, applied here**: `ztgamemgr.rs`'s old embedded-view struct
//! asserted `size_of() == 0x1150` and named the field at `+0x44` `num_guests`. The size assertion was
//! wrong - `init.asm`/`calculateSums.asm` confirm real fields through `+0x117c` (struct size at least
//! `0x1180`, see [`ZooStatus`]'s tail fields below). The `+0x44` rename went through two rounds: a
//! Stage-0/Stage-4 pass first renamed it `escaped_animal_tile_count` (reasoning it was an
//! escape-condition counter, not a guest headcount), then Stage 5's full `calculateSums.asm` read
//! overturned *that* - it's incremented once per live tile whose entity passes the `ZTGuestType` check,
//! i.e. it genuinely is guest-related after all. See [`ZooStatus::guest_tile_count`]'s own doc comment
//! for the full evidence trail; `ZTGameMgr`'s own mirrored copy is renamed to match.

use std::{ffi::{c_void, CStr}, mem};

use openzt_detour::FunctionDef;
use openzt_detour::generated::{
    bfapp::LOAD_STRING,
    bfconfigfile::{GET_FLOAT, GET_INT},
    bfinternat::{GET_MONEY_TEXT_0, SET_MONEY_TEXT_0},
    bfuimgr::{DISPLAY_MESSAGE_0, DISPLAY_MESSAGE_1, GET_ELEMENT_0},
    msvc_std::RAND,
    msvc_std_basic_string::{BASIC_STRING_0, BASIC_STRING_2},
    standalone::{DEALLOCATE, GET_OLD_DATE, WRITE_BYTES_TO_FILE},
    uielement::{DISABLE, ENABLE},
    zoostatus::{
        ADMISSION_MESSAGE, ANIMAL_ESCAPED, BUY_ANIMAL, BUY_PEOPLE_FOOD, CALCULATE_SUMS, CHANGE_ENDOWMENT_MEMBERS, F_CHANCE, F_CREATE_GUEST,
        F_GRANT_DONATION, F_ZOO_MESSAGE, FINANCE_CHECKS, HEAL_ANIMAL, INCREASE_ADMISSIONS, INCREASE_ADMISSIONS_INCOME, INCREASE_DONATIONS,
        INCREASE_ENDOWMENT, INCREASE_SHOW_ADMISSION, INIT, LOAD, MESSAGE_CHECKS, NEWGUEST_CHECKS, OVERRIDE, PURCHASE_FOOD, RATING_CHECKS,
        REFUND_ANIMAL_COST, REFUND_CONSTRUCTION, RESET_FINANCE_INFO, SAVE, SET_ADULT_ADMISSION_PRICE, SHOW_PRICES, SPEND_BUILDING_UPKEEP,
        SPEND_CONSTRUCTION, SPEND_GUIDE_WAGES, SPEND_KEEPER_WAGES, SPEND_MAINT_WAGES, SPEND_MARKETING, SPEND_RESEARCH, UPDATE,
    },
    ztapp::GET_APP,
    zthabitat::GET_NUM_ANIMALS,
    zthabitatmgr::GET_NUM_SPECIES,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    lua_fn,
    util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory},
    ztworldmgr::IVec3,
};

/// Vanilla-layout-compatible view of `ZooStatus`, embedded inline inside `ZTGameMgr` at `+0x10`. Pure
/// scalars and fixed-size arrays - no STL containers, see the module doc comment.
#[derive(Debug)]
#[repr(C)]
pub struct ZooStatus {
    /// **Resolved (Stage 6)**: `override`'s `[checks]` section reads `rating`/`message`/`newguest` (in
    /// that order) straight into these three offsets via `BFConfigFile::getInt` - confirmed against the
    /// real shipped `economy.cfg` (`config.ztd`), whose `[checks]` block is
    /// `rating=4000`/`message=10000`/`newguest=2000`/`finance=360000`. [`Self::update`]'s own dispatch
    /// (`self.field_0xc > self.config_budget_0x00`, etc.) already treats these as tick-count intervals a
    /// paired elapsed-ticks accumulator is compared against, not budgets - this pass's real config-file
    /// evidence confirms that reading and overturns this field's original "starting
    /// research/marketing/wage budget" guess. Default `10000` (`init.asm`, confirmed integer via a plain
    /// `MOV` with no FPU instruction), overridden by `override` whenever a real config is supplied. Not
    /// part of `save`/`load`'s field list.
    pub(crate) rating_check_interval: i32, // 0x00
    /// See [`Self::rating_check_interval`] - `override`'s `[checks]`/`message` key.
    pub(crate) message_check_interval: i32, // 0x04
    /// See [`Self::rating_check_interval`] - `override`'s `[checks]`/`newguest` key. (The real
    /// `economy.cfg`'s fourth `[checks]` key, `finance`, is read by `financeChecks` itself, which stays a
    /// vanilla call-through in this plan - not this struct's concern yet.)
    pub(crate) newguest_check_interval: i32, // 0x08
    /// Saved/loaded (`save.c`'s `field_0xc`). [`Self::update`] increments this by `delta` every tick and
    /// compares it against [`Self::rating_check_interval`] to decide whether `ratingChecks` is due -
    /// [`Self::rating_checks`] resets it back to `0` once it runs.
    pub(crate) rating_check_elapsed: i32, // 0x0c
    /// See [`Self::rating_check_elapsed`] - paired with [`Self::message_check_interval`];
    /// [`Self::message_checks`] resets it.
    pub(crate) message_check_elapsed: i32, // 0x10
    /// See [`Self::rating_check_elapsed`] - paired with [`Self::newguest_check_interval`];
    /// [`Self::newguest_checks`] resets it.
    pub(crate) newguest_check_elapsed: i32, // 0x14
    /// Saved/loaded, 1 byte. `ZTGameMgr::updateSim`'s finance-check trigger flag.
    pub(crate) finance_check_pending: bool, // 0x18
    _pad_0x19: [u8; 3],
    /// Saved/loaded. Current zoo rating, read directly (no formula) by `ZTGameMgr::updateSim` and
    /// passed straight to `ZTUI::main::setZooRating`.
    pub(crate) zoo_rating_current: i32, // 0x1c
    /// Live animal count, recomputed from scratch every `calculateSums` call by walking
    /// `GLOBAL_ZTWorldMgr`'s tile array. Not part of `save`'s field list (derived, not persisted).
    pub(crate) num_animals: u16, // 0x20
    _pad_0x22: [u8; 2],
    /// A per-animal condition flag count: `calculateSums` increments this once per live animal tile
    /// whose own `+0x3a7` byte flag is set (confirmed `.asm`, Stage 5 - same tile this animal's own
    /// `+0x2a8` "score" field feeds into [`Self::animal_rating_metric`]'s averaging). Genuinely
    /// animal-side, unlike [`Self::guest_condition_counter_1`]/[`Self::guest_condition_counter_2`]
    /// below (see those fields' own doc comments for the correction this pass made).
    pub(crate) animal_condition_counter_1: u16, // 0x24
    _pad_0x26: [u8; 2],
    /// Refreshed each `calculateSums` call via `zthabitatmgr::GET_NUM_SPECIES`.
    pub(crate) num_species: u16, // 0x28
    _pad_0x2a: [u8; 2],
    pub(crate) num_tired_guests: u16, // 0x2c
    _pad_0x2e: [u8; 2],
    pub(crate) num_hungry_guests: u16, // 0x30
    _pad_0x32: [u8; 2],
    pub(crate) num_thirst_guests: u16, // 0x34
    _pad_0x36: [u8; 2],
    pub(crate) num_guests_restroom_need: u16, // 0x38
    _pad_0x3a: [u8; 2],
    /// **Correction (Stage 5), overturning this struct's earlier Stage-0/Stage-4 guess**: this and
    /// [`Self::guest_condition_counter_2`] were originally named `animal_condition_counter_2`/`_3` on
    /// the assumption that `calculateSums`' `DAT_00638700`-gated branch was an "animal is
    /// loose/escaped" check. `calculateSums.asm` (read in full, Stage 5) shows that branch is instead
    /// entered once per live **guest** tile (`DAT_00638700` is independently confirmed
    /// `ztmegatilemgr.rs`'s `RVA_GUEST_TYPE_CHECK_ARG`/"`ZTGuestType` check", used the same way by that
    /// module's own guest-occupant walk) - inside it, `calculateSums` increments
    /// [`Self::num_hungry_guests`]/[`Self::num_thirst_guests`]/[`Self::num_guests_restroom_need`]/
    /// [`Self::num_tired_guests`] (already correctly named) plus this field, gated on the guest's own
    /// `+0x33c` byte flag. Pure rename, same bytes/offset - `messageChecks`/`admissionMessage`'s
    /// already-shipped, live-tested logic is unaffected (it only ever read/compared the raw field, never
    /// depended on the label).
    pub(crate) guest_condition_counter_1: u16, // 0x3c
    _pad_0x3e: [u8; 2],
    /// See [`Self::guest_condition_counter_1`]'s doc comment - same correction, gated on a live guest's
    /// own `+0x26c`-then-`+0x10` nested-pointer nonzero check rather than a flag byte.
    pub(crate) guest_condition_counter_2: u16, // 0x40
    _pad_0x42: [u8; 2],
    /// **Correction (Stage 5)**: renamed from `escaped_animal_tile_count` - see
    /// [`Self::guest_condition_counter_1`]'s doc comment for the full evidence trail.
    /// `calculateSums.asm` increments this once per live tile whose entity passes the `ZTGuestType`
    /// check (`DAT_00638700`), i.e. it *is* a live guest count after all (the Stage-0/Stage-4 "not a
    /// guest headcount" conclusion was wrong) - `ztgamemgr.rs`'s original `num_guests` name was closer
    /// to the truth than this struct's own "correction" of it. Kept as a fresh name rather than reverted
    /// straight back to `num_guests`, since it's specifically a *tile-walk* count from `calculateSums`,
    /// not necessarily identical to a "total guests in the zoo" figure another system might expose.
    /// `messageChecks`/`admissionMessage` both compare it against the literal `10`. Not part of `save`'s
    /// field list (derived, not persisted).
    pub(crate) guest_tile_count: u16, // 0x44
    _pad_0x46: [u8; 2],
    /// Saved/loaded (`save.c`'s `field_0x48`). `newguestChecks` also writes a `0`-`4` state-machine
    /// value at this same offset; whether that's the persisted field itself or a distinct transient
    /// use of the same bytes is unresolved (the plan's own narrative flags this as a to-be-reconciled
    /// discrepancy) - not a Stage 1 blocker, but don't assume either reading without rechecking the
    /// `.asm` when `newguestChecks` itself is ported.
    pub(crate) field_0x48: i32, // 0x48
    /// Default `10000` (plain integer, same load as [`Self::rating_check_interval`]'s trio), unresolved
    /// role, not part of `save`'s field list.
    pub(crate) field_0x4c: i32, // 0x4c
    /// Saved/loaded (`save.c`'s `field_0x50`).
    pub(crate) field_0x50: i32, // 0x50
    /// Saved/loaded (`save.c`'s `field_0x54`). Distinct from [`Self::guest_tile_count`] -
    /// erratum in an earlier draft of the plan wrongly conflated the two.
    pub(crate) field_0x54: i32, // 0x54
    /// Zeroed by both `init` and `calculateSums` alongside [`Self::non_blank_tile_fraction`], no
    /// further semantic resolved.
    pub(crate) field_0x58: i32, // 0x58
    /// Raw animal-happiness metric `ZTGameMgr::updateSim` feeds through `((metric + 100) * 100) / 200`
    /// before calling `ZTUI::main::setAnimalRating`.
    pub(crate) animal_rating_metric: i32, // 0x5c
    /// Same shape as [`Self::animal_rating_metric`], feeding `ZTUI::main::setGuestRating`.
    pub(crate) guest_rating_metric: i32, // 0x60
    /// A normalized "fraction of non-blank map tiles" ratio, computed by `calculateSums`. Not part of
    /// `save`'s field list.
    pub(crate) non_blank_tile_fraction: f32, // 0x64
    /// Default `1000`, overridden by the `AI`/`maxGuests` config setting.
    pub(crate) max_guests: i32, // 0x68
    /// **Resolved (Stage 6)**: `override`'s `[characteristics]`/`cAngryAnimalsSickChange` key
    /// (`BFConfigFile::getInt`). Default `0` (`init.asm`).
    pub(crate) angry_animals_sick_change: i32, // 0x6c
    /// One of eight per-need-counter message-frequency thresholds `messageChecks` `FMUL`s against a
    /// small tile-condition counter before a `fZooMessage` call. Default `0.5`. Counter pairing resolved -
    /// see [`Self::message_checks`]'s own doc comment for the full per-field mapping.
    /// **Resolved (Stage 6)**: `override`'s `[characteristics]`/`cPctSick` key (`BFConfigFile::getFloat`).
    pub(crate) message_threshold_0x70: f32,
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctProtestors` key.
    pub(crate) message_threshold_0x74: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngryHungryGuestsChange` key. Default `0`.
    pub(crate) angry_hungry_guests_change: i32, // 0x78
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctHungry` key.
    pub(crate) message_threshold_0x7c: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngryThirstyGuestsChange` key. Default `0`.
    pub(crate) angry_thirsty_guests_change: i32, // 0x80
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctThirsty` key.
    pub(crate) message_threshold_0x84: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngryBathroomGuestsChange` key. Default `0`.
    pub(crate) angry_bathroom_guests_change: i32, // 0x88
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctBathroom` key.
    pub(crate) message_threshold_0x8c: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngrySouvenirGuestsChange` key. Default `0`.
    pub(crate) angry_souvenir_guests_change: i32, // 0x90
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctSouvenir` key.
    pub(crate) message_threshold_0x94: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngryRemoveAnimalChange` key. Default `0`.
    pub(crate) angry_remove_animal_change: i32, // 0x98
    /// **Resolved (Stage 6)**: `override`'s `cAngryTiredGuestsChange` key. Default `0`.
    pub(crate) angry_tired_guests_change: i32, // 0x9c
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctTired` key.
    pub(crate) message_threshold_0xa0: f32,
    /// **Resolved (Stage 6)**: `override`'s `cAngryTrashGuestsChange` key. Default `0`.
    pub(crate) angry_trash_guests_change: i32, // 0xa4
    /// See [`Self::message_threshold_0x70`] - `override`'s `cPctTrash` key.
    pub(crate) message_threshold_0xa8: f32,
    /// Five contiguous per-guest-type arrival multipliers, default `1` each, read by `newguestChecks`
    /// in its arrival-probability calc. **Confirmed `i32`, not `f32`**: `init.asm` loads them via plain
    /// `MOV %EBP,0x1` (the literal integer `1`, bit pattern `0x00000001`) with no accompanying FPU
    /// instruction, not the `0x3f800000` bit pattern `1.0f` would need - overturns this plan's original
    /// assumption (which inferred `f32` from `newguestChecks`' `FMUL` usage alone, without checking
    /// `init`'s own write). Whoever ports `newguestChecks` (Stage 4) should expect an int-to-float
    /// conversion (`FILD`) at the read site, not a raw float reinterpretation.
    pub(crate) guest_type_arrival_multiplier: [i32; 5], // 0xac..0xc0
    /// **Resolved (Stage 6)**: `override`'s `[characteristics]` section keys, `BFConfigFile::getInt`/
    /// `getFloat` in vanilla's own call order. This whole `0xc0..0x120` header cluster (24 `i32`/`f32`
    /// slots) is exactly and only these 24 keys - no leftover unclaimed bytes. Default `0` unless noted.
    pub(crate) loan_available: i32, // 0xc0, cLoanAvailable
    pub(crate) high_zoo_value_change: i32, // 0xc4, cHighZooValueChange
    pub(crate) low_zoo_value_change: i32, // 0xc8, cLowZooValueChange
    /// Default `97400.0`. `override`'s `cHighZooValue` key.
    pub(crate) high_zoo_value: f32, // 0xcc
    /// `override`'s `cLowZooValue` key.
    pub(crate) low_zoo_value: f32, // 0xd0
    /// Default `10`. `override`'s `cHighSpeciesThreshold` key.
    pub(crate) high_species_threshold: i32, // 0xd4
    pub(crate) happy_diverse_animals_change: i32, // 0xd8, cHappyDiverseAnimalsChange
    /// Default `2`. `override`'s `cLowSpeciesThreshold` key.
    pub(crate) low_species_threshold: i32, // 0xdc
    pub(crate) angry_diverse_animals_change: i32, // 0xe0, cAngryDiverseAnimalsChange
    /// Default `90`. `override`'s `cHighAvgAnimalHappyThreshold` key.
    pub(crate) high_avg_animal_happy_threshold: i32, // 0xe4
    /// Default `10`. `override`'s `cHappyAnimalsChange` key.
    pub(crate) happy_animals_change: i32, // 0xe8
    pub(crate) low_avg_animal_happy_threshold: i32, // 0xec, cLowAvgAnimalHappyThreshold
    pub(crate) angry_animals_change: i32, // 0xf0, cAngryAnimalsChange
    /// Default `90`. `override`'s `cHighAvgGuestHappyThreshold` key.
    pub(crate) high_avg_guest_happy_threshold: i32, // 0xf4
    /// Default `10`. `override`'s `cHappyGuestChange` key.
    pub(crate) happy_guest_change: i32, // 0xf8
    pub(crate) low_avg_guest_happy_threshold: i32, // 0xfc, cLowAvgGuestHappyThreshold
    pub(crate) angry_guest_change: i32, // 0x100, cAngryGuestChange
    /// Default `0.7999...`. `override`'s `cItemCheap` key.
    pub(crate) item_cheap: f32, // 0x104
    /// Default `1.2`. `override`'s `cItemExpensive` key.
    pub(crate) item_expensive: f32, // 0x108
    /// Default `0.2`. `override`'s `cHighZooEsthetic` key.
    pub(crate) high_zoo_esthetic: f32, // 0x10c
    pub(crate) high_zoo_esthetic_change: i32, // 0x110, cHighZooEstheticChange
    /// `override`'s `cLowZooEsthetic` key.
    pub(crate) low_zoo_esthetic: f32, // 0x114
    pub(crate) low_zoo_esthetic_change: i32, // 0x118, cLowZooEstheticChange
    /// Default `1000.0`. `override`'s `cResearchCost` key.
    pub(crate) research_cost: f32, // 0x11c
    /// Saved/loaded (`save.c`'s `field_0x120`). Running donation counter this period, read/written by
    /// `fGrantDonation`. **Corrected to `f32`, not `i32`** - `init.asm`'s own default (a plain `MOV` of
    /// `EBX`, which is `0` at that point) is bit-ambiguous between the two, but `fGrantDonation`'s own
    /// `.asm` settles it unambiguously: `FLD`/`FADD DAT_00635490`/`FST` against this offset (a real FPU
    /// load-add-store sequence, not `MOV`/`ADD`), and `DAT_00635490` is independently confirmed `1.0` by
    /// `ztsoundscape.rs`. The `.c` decompile's own `local_628`/`local_62c` naming obscured this - ground
    /// truth came from the `.asm` directly (see [`Self::f_grant_donation`]'s doc comment for the full
    /// increment/compare logic this field drives).
    pub(crate) donation_count_this_period: f32, // 0x120
    /// Bound against [`Self::donation_count_this_period`] in `fGrantDonation`. Default `3`, not part of
    /// `save`'s field list.
    pub(crate) donation_count_bound: i32, // 0x124
    /// Donation-amount roll range read by `fGrantDonation` (via the unresolved `FUN_0040f103` helper).
    /// **Confirmed `i32`, not `f32`** (same `init.asm` plain-`MOV`-of-a-clean-integer evidence as
    /// [`Self::guest_type_arrival_multiplier`] - `MOV %EDX,0x2710`/`MOV %ECX,0x4e20`, not the float bit
    /// patterns `10000.0f`/`20000.0f` would need). Default `10000`, not part of `save`'s field list.
    pub(crate) donation_amount_min: i32, // 0x128
    /// See [`Self::donation_amount_min`]. Default `20000`.
    pub(crate) donation_amount_max: i32, // 0x12c
    /// `ZooStatus::update`'s per-tick donation-roll parameter (`fChance(this->field_0x130)`,
    /// `zoostatus_update.asm`, confirmed) - the percent chance (out of 100) of a spontaneous donation each
    /// tick the live budget is below `DAT_00635128`. Default `0` (per `init.asm`) - never fires unless
    /// something else in `override`'s config-driven tail (not yet ported) sets it.
    pub(crate) donation_chance_percent: i32, // 0x130
    /// `ratingChecks`' cap on how many species count toward the species-rating bonus
    /// (`min(num_species, species_rating_cap) * 10 / species_rating_cap`, `ZooStatus_ratingChecks.asm`,
    /// confirmed). Default `44`.
    pub(crate) species_rating_cap: i32, // 0x134
    /// **Resolved (Stage 6)**: `override`'s `[characteristics]`/`cMembershipJoinHappiness` key. Default
    /// `80`. Also read by `financeChecks` (an endowment-lottery loop bound), which stays a vanilla
    /// call-through this stage (see [`Self::update`]'s doc comment).
    pub(crate) membership_join_happiness: i32, // 0x138
    /// Default `10`. `override`'s `cMembershipJoinFactor` key.
    pub(crate) membership_join_factor: i32, // 0x13c
    /// Default `10000`. `override`'s `cEndowmentGiftLow` key. Also read by `financeChecks` (a
    /// random-endowment-income roll range).
    pub(crate) endowment_gift_low: i32, // 0x140
    /// Default `20000`. `override`'s `cEndowmentGiftHigh` key. See [`Self::endowment_gift_low`].
    pub(crate) endowment_gift_high: i32, // 0x144
    /// Default `50`. `override`'s `cMembershipJoinChance` key.
    pub(crate) membership_join_chance: i32, // 0x148
    /// Saved/loaded (`save.c`'s `mbr_0x14c`). Rolling write-cursor into [`Self::monthly_history`]'s
    /// 31-category rows (`ZTAwardMgr::elapsed_metric` also raw-reads this, `ZTGameMgr`-relative
    /// `+0x15c`). Default `1` at init.
    pub(crate) current_month_index: i32, // 0x14c
    /// Saved/loaded (`save.c`'s `mbr_0x150`). Rolling write-cursor into [`Self::yearly_history`]'s
    /// 31-category rows (`ZTAwardMgr::elapsed_metric` also raw-reads this, `ZTGameMgr`-relative
    /// `+0x160`). Default `0` at init.
    pub(crate) current_year_index: i32, // 0x150
    /// Monthly rolling-history region: 31 categories (one per accumulator method/metric, e.g.
    /// `spendConstruction`'s own row), 12 months each, indexed `[category][current_month_index]`. Real,
    /// confirmed-dense geometry from `init.asm`'s zero-loop (base `0x154`, size `0x5d0` = `31*12*4`) -
    /// which category owns which row index is not yet resolved (see the plan's "Open questions" -
    /// per-method row offsets don't align to this region's own zero-loop start under the naive
    /// grid model, a real but non-blocking puzzle), so later stages should keep using each method's own
    /// confirmed absolute offset rather than a computed `row*0x30 + column*4` until that's sorted out.
    pub(crate) monthly_history: [[f32; 12]; 31], // 0x154, size 0x5d0
    /// Yearly rolling-history region: same 31 categories, 20 years each, indexed
    /// `[category][current_year_index]`. Base `0x724`, size `0x9b0` = `31*20*4`. Same row-mapping caveat
    /// as [`Self::monthly_history`].
    pub(crate) yearly_history: [[f32; 20]; 31], // 0x724, size 0x9b0
    /// Flat all-time-total region: 31 individually-addressed slots, one per category, not indexed by
    /// month/year. Base `0x10d4`, size `0x7c` = `31*4`. 21 of 31 slots are mapped to a specific method
    /// already (see the plan's "The remaining 18 methods" section); the other 10 are unmapped, non-
    /// blocking.
    pub(crate) flat_totals: [f32; 31], // 0x10d4, size 0x7c
    /// Saved/loaded. Current adult admission price, clamped into
    /// `[`[`Self::admission_price_min`]`, `[`Self::admission_price_max`]`]` by
    /// `setAdultAdmissionPrice`. Defaults to `49.0` when loading a pre-`0x47`-version save.
    pub(crate) admission_price: f32, // 0x1150
    /// Default `0.0`. Lower bound `setAdultAdmissionPrice`/`showPrices` clamp/compare against.
    pub(crate) admission_price_min: f32, // 0x1154
    /// Default `100.0`. Upper bound `setAdultAdmissionPrice`/`showPrices` clamp/compare against.
    pub(crate) admission_price_max: f32, // 0x1158
    /// **Resolved (Stage 6)**: `override`'s `[characteristics]`/`cPricingFactor` key
    /// (`BFConfigFile::getFloat` into `this[0xd].field_0x18`, i.e. `0xd*0x154+0x18 = 0x115c` under the
    /// confirmed stride - see the plan's `calculateSums` section for the stride derivation). Default
    /// `1.0`.
    pub(crate) pricing_factor: f32, // 0x115c
    /// Default a small negative float (`0x2a9c5fff`). `override`'s `cDonationFactor` key
    /// (`this[0xd].field_0x1c` = `0x1160`).
    pub(crate) donation_factor: f32, // 0x1160
    /// Default `29.0`. `override`'s `cBuildingUseCostDefault` key (`this[0xd].field_0x20` = `0x1164`).
    pub(crate) building_use_cost_default: f32, // 0x1164
    /// Default `100.0`. `override`'s `cBuildingUseCostMax` key (`this[0xd].field_0x24` = `0x1168`).
    pub(crate) building_use_cost_max: f32, // 0x1168
    /// Default `50.0`. `override`'s `cZooDooRecyclingAmount` key (`this[0xd].field_0x28` = `0x116c`).
    pub(crate) zoo_doo_recycling_amount: f32, // 0x116c
    /// Default `1.0`. `fCreateGuest` computes admission income as
    /// `admission_price * admission_income_multiplier`, plausibly a per-guest-type price ratio.
    pub(crate) admission_income_multiplier: f32, // 0x1170
    /// Research-completion percentage (`0..=100`), zeroed by `init`, computed by `calculateSums` from a
    /// live `GLOBAL_ZTResearchMgr` walk. Not part of `save`'s field list.
    ///
    /// **Correction to this struct's original Stage 1 field type**: `calculateSums.asm` stores this via a
    /// plain `MOV dword ptr [ESI+0x1174], EAX` after an `IDIV` (an integer division result), and the
    /// divide-by-zero fallback path stores the literal integer `100` (`MOV EAX,0x64`) the same way - both
    /// are genuine integer stores, not `FSTP`. `ratingChecks.asm` reads it back the same way (`MOV EAX,
    /// dword ptr [ESI+0x1174]`, not `FLD`). The original Stage 1 struct declared this `f32`, inferred from
    /// the plan's own narrative description ("a percentage") rather than checked against the writer's own
    /// instruction - same class of mistake `init`'s own type corrections (Stage 2) already found twice.
    pub(crate) research_completion_percent: i32, // 0x1174
    /// Last-animal-escape timestamp (`getOldDate()`'s 8-byte result, low/high dwords kept separate to
    /// avoid a `u64`'s stricter alignment requirement shifting this `#[repr(C)]` struct's later
    /// layout). Written by `animalEscaped` with the current date each time an animal escapes; seeded at
    /// `init`/re-seeded by `load` for saves old enough to have never stored a real value.
    /// `ratingChecks` reads it back through `ZTGameMgr::hoursAgo` as an escape-recency decay window.
    /// Saved/loaded.
    pub(crate) last_animal_escape_timestamp_low: u32, // 0x1178
    pub(crate) last_animal_escape_timestamp_high: u32, // 0x117c
}

const _: () = assert!(mem::size_of::<ZooStatus>() == 0x1180);

/// Raw process-global data addresses Stage 4's `messageChecks`/`ratingChecks`/`update` read directly
/// (real vanilla tunable constants/counters that live outside `ZooStatus`'s own memory, not `ZooStatus`
/// fields) - resolved at runtime as `get_module_base("zoo.exe") + RVA` per `CLAUDE.md`'s data-vs-code
/// address distinction (Ghidra VAs need no adjustment for *code*, since `zoo.exe` has no ASLR and always
/// loads at `0x400000`, but a *data* address still goes through this base+RVA form for consistency with
/// this codebase's established convention, e.g. `ztgamemgr.rs`'s `DAT_006394B8_RVA`).
mod raw_globals {
    /// `ratingChecks`' "non-blank-tile-fraction cap" scale factor (`f32`,
    /// `non_blank_tile_fraction * DAT_00630d60`).
    pub(super) const ATTENDANCE_FRACTION_SCALE_RVA: u32 = 0x00630d60 - 0x400000;
    /// `ratingChecks`' cap-vs-floor comparison threshold (`f32`) for the attendance-fraction term.
    pub(super) const ATTENDANCE_FRACTION_FLOOR_RVA: u32 = 0x00630d64 - 0x400000;
    /// `ratingChecks`' comparison threshold (`f32`) selecting between the attendance-fraction term and the
    /// research-decay fallback.
    pub(super) const ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA: u32 = 0x00630d5c - 0x400000;
    /// `ratingChecks`' final decay-penalty scale factor (`f32`).
    pub(super) const RATING_DECAY_SCALE_RVA: u32 = 0x00630d74 - 0x400000;
    /// `ratingChecks`' escape-recency decay baseline (`i32`, `DAT_006392a0 - DAT_00639294 *
    /// (hoursSinceEscape / 24)`).
    pub(super) const ESCAPE_DECAY_BASELINE_RVA: u32 = 0x006392a0 - 0x400000;
    /// `ratingChecks`' escape-recency decay-per-day rate (`i32`), paired with
    /// [`ESCAPE_DECAY_BASELINE_RVA`].
    pub(super) const ESCAPE_DECAY_PER_DAY_RVA: u32 = 0x00639294 - 0x400000;
    /// The escaped-animal `std::list<ZTAnimal*>`'s head-pointer *global variable* - **not** the sentinel
    /// node's own address. `ratingChecks.asm` loads this via `MOV ECX, dword ptr DAT_00638fb0` (a value
    /// read of the global), then dereferences *that* value to reach the sentinel; `count_escaped_animals`
    /// must apply the same extra indirection - see that method's own inline comment for a real bug this
    /// corrected (an earlier draft treated this RVA's own address as the sentinel, which could walk
    /// garbage pointers forever). Read-only walk of a live vanilla container - no allocation/freeing on
    /// our side, so none of `CLAUDE.md`'s cross-allocator hazards apply (same shape as `calculateSums`'
    /// `ZTWorldMgr` tile-array walk).
    pub(super) const ESCAPED_ANIMAL_LIST_SENTINEL_RVA: u32 = 0x00638fb0 - 0x400000;
    /// `ZooStatus::update`'s donation-roll gate: only rolls [`super::F_CHANCE`] when the live budget
    /// (`ZTGameMgr::cash`) is below this threshold (`f32`).
    pub(super) const DONATION_CASH_THRESHOLD_RVA: u32 = 0x00635128 - 0x400000;

    /// `newguestChecks`' four admission-price tier boundaries (`f32`, compared against
    /// [`super::ZooStatus::admission_price`] in a `< , < , <=(<=), else` chain - see
    /// [`super::ZooStatus::newguest_checks`]'s doc comment for the exact chain). **Resolved (Stage 6,
    /// second pass)**: these are the first four of `override`'s `[characteristics]`/`cAdultAdmission`
    /// config-list values (`ZooStatus_override.c`'s now-decompiled copy loop, `0x6392ac..0x6392c0`) -
    /// config-driven, not hardcoded, contrary to this constant's original assumption. See
    /// [`super::ZooStatus::override_config`]'s doc comment for the full write-side evidence.
    pub(super) const PRICE_TIER_BOUNDARY_0_RVA: u32 = 0x006392ac - 0x400000;
    pub(super) const PRICE_TIER_BOUNDARY_1_RVA: u32 = 0x006392b0 - 0x400000;
    pub(super) const PRICE_TIER_BOUNDARY_2_RVA: u32 = 0x006392b4 - 0x400000;
    pub(super) const PRICE_TIER_BOUNDARY_3_RVA: u32 = 0x006392b8 - 0x400000;
    /// The `cAdultAdmission` list's fifth and last value (real `economy.cfg`: `0`) - copied by `override`
    /// alongside [`PRICE_TIER_BOUNDARY_0_RVA`]`..`[`PRICE_TIER_BOUNDARY_3_RVA`] into the same contiguous
    /// `0x6392ac..0x6392c0` global range, but **not currently read by any already-ported method**
    /// ([`super::ZooStatus::price_tier`]'s confirmed `.asm`-derived `< , < , <=, <=` chain only ever
    /// touches the first four) - kept here, named, since `override` must still write it faithfully
    /// regardless of who (if anyone, in this decompile corpus) reads it.
    pub(super) const PRICE_TIER_BOUNDARY_4_RVA: u32 = 0x006392bc - 0x400000;
    /// `newguestChecks`' "double the marketing benefit" event flag (`bool`, stored as a byte).
    pub(super) const DOUBLE_MARKETING_BENEFIT_FLAG_RVA: u32 = 0x006392c1 - 0x400000;
    /// `newguestChecks`' flat `+30` attendance-bonus event flag (`bool`, stored as a byte) - paired with
    /// [`DOUBLE_MARKETING_BENEFIT_FLAG_RVA`].
    pub(super) const FLAT_ATTENDANCE_BONUS_FLAG_RVA: u32 = 0x006392c0 - 0x400000;
    /// `GLOBAL_BFUIMgr`'s RVA - **is** the live `BFUIMgr` singleton's own address directly, no
    /// dereference (a statically-embedded object, not a separate heap pointer variable), read by
    /// [`super::f_zoo_message`]/[`super::display_message_string`]/[`super::ZooStatus::show_prices`].
    ///
    /// **Correction (Stage 5)**: this constant's doc comment originally claimed one dereference was
    /// needed (reasoning from `ZooStatus_fZooMessage.asm`'s `MOV %ECX, GLOBAL_BFUIMgr` by analogy with
    /// `GLOBAL_ZTGameMgr`'s own confirmed-dereferencing pattern) - but that reasoning was never actually
    /// live-tested end to end: every call site that read this global before Stage 5
    /// ([`super::f_zoo_message`] via `admission_message`/`message_checks`, `display_message_string` via
    /// `f_grant_donation`) was only ever exercised in live tests along a path that happened to never
    /// actually trigger the real dispatch (see `ZOOSTATUS_CHECKS`/`ZOOSTATUS_NEWGUEST_CHECKS_SMOKE`'s own
    /// doc comments). [`super::ZooStatus::show_prices`] was the first unconditional live call through
    /// this global, and crashed the whole reimplementation-test battery outright with the dereferencing
    /// version - root-caused to this constant, not `show_prices`' own logic, by cross-checking against
    /// four other, independently-written, already-live-working modules that all read this exact same
    /// address (`0x0023_8de0` RVA) as `this` directly with **no** dereference: `ztresearch.rs`/
    /// `ztshowui.rs`/`ztthoughtmgr.rs`/`ztawardmgr.rs`'s own `global_bfuimgr()` helpers. Fixed to match;
    /// the disassembly's bare `MOV reg, GLOBAL_BFUIMgr` (no `dword ptr [...]`) turns out to be this
    /// disassembler's rendering of an address-of load here, not a value load - genuinely ambiguous
    /// notation between the two cases without inspecting raw bytes, and the wrong case was picked
    /// originally. Single-purpose to this module for now, matching `ztgamemgr.rs`'s own precedent for
    /// not promoting a single-consumer global into `globals.rs`'s shared `CachedGlobalInstance` registry
    /// until a second caller needs it (the four sibling modules above each keep their own copy of this
    /// same address for the same reason - not shared even with each other).
    pub(super) const GLOBAL_BFUIMGR_RVA: u32 = 0x00638de0 - 0x400000;

    /// `calculateSums`' entity-type-check argument for "is this a guest" (`&DAT_00638700` in the
    /// decompile) - independently confirmed `ztmegatilemgr.rs`'s `RVA_GUEST_TYPE_CHECK_ARG`
    /// (`ZTGuestType` check, same vtable-slot-`0x1c` mechanism), which is what overturned this struct's
    /// earlier `escaped_animal_tile_count`/`animal_condition_counter_2`/`_3` naming - see
    /// [`super::ZooStatus::guest_tile_count`]'s doc comment.
    pub(super) const GUEST_TYPE_CHECK_RVA: u32 = 0x00638700 - 0x400000;
    /// `calculateSums`' entity-type-check argument for "is this an animal" (`&DAT_00638690`) - the same
    /// "is animal-ish" check `ztshow.rs`'s `RVA_ANIMAL_TYPE_CHECK`/`ztthoughtmgr.rs`'s
    /// `resolve_object_own_habitat_ptr` already use.
    pub(super) const ANIMAL_TYPE_CHECK_RVA: u32 = 0x00638690 - 0x400000;
    /// `calculateSums`' entity-type-check argument for the third tile-content category
    /// (`&DAT_00638670`) - independently confirmed `ztmegatilemgr.rs`'s `RVA_SCENERY_TYPE_CHECK_ARG`,
    /// and further pinned down here by `private/docs/vtables/ZTBuildingType.md`'s `+0x1c` override
    /// (`isCastClass`) plus its `+0xa4` slot (`getPurchaseCost`, inherited from `ZTSceneryType`) exactly
    /// matching the vtable call `calculateSums` makes on a tile that passes this check - so this branch
    /// is specifically "is this a building" (buildings' purchase cost feeds [`super::ZooStatus`]'s
    /// `field_0x4c` attendance/value accumulator), not scenery in general.
    pub(super) const BUILDING_TYPE_CHECK_RVA: u32 = 0x00638670 - 0x400000;
    /// `calculateSums`' guest-need threshold (`i32`, compared via plain `CMP`/`SETG` against each live
    /// guest's own hunger/thirst/restroom-need/tiredness field - not a float compare, despite those
    /// fields plausibly being percentage-like) - one guest tile increments
    /// [`super::ZooStatus::num_hungry_guests`]/etc. whenever its own raw field exceeds this.
    pub(super) const GUEST_NEED_THRESHOLD_RVA: u32 = 0x00639024 - 0x400000;
    /// `showPrices`' child-admission-price scale factor (`f32`, `admission_price * this` feeds the
    /// `0x1062` UI element's money text).
    pub(super) const CHILD_ADMISSION_PRICE_SCALE_RVA: u32 = 0x00630d54 - 0x400000;
}

/// Section/key string-literal addresses [`ZooStatus::override_config`] passes to real vanilla
/// `BFConfigFile::getInt`/`getFloat` (Stage 6). Every address comes directly from `ZooStatus_override.c`'s
/// own Ghidra-assigned symbol name (`s_<text>_<address>`) - not re-derived, just transcribed - and is
/// resolved at runtime the same `get_module_base("zoo.exe") + RVA` way as [`raw_globals`] (string
/// literals are read-only *data*, not code, so they get the same base+RVA treatment - see that module's
/// own doc comment). Real vanilla call order is preserved exactly in [`ZooStatus::override_config`], even
/// though the write order doesn't affect the result (each key targets a disjoint field) - this just keeps
/// the port a transcription, not a reordering.
mod override_config_keys {
    pub(super) const CHECKS_SECTION_RVA: u32 = 0x00641b68 - 0x400000; // "checks"
    pub(super) const RATING_KEY_RVA: u32 = 0x00641b70 - 0x400000; // "rating"
    pub(super) const MESSAGE_KEY_RVA: u32 = 0x00641b60 - 0x400000; // "message"
    pub(super) const NEWGUEST_KEY_RVA: u32 = 0x00641b54 - 0x400000; // "newguest"

    /// "characteristics" - the section every key below lives in.
    pub(super) const CHARACTERISTICS_SECTION_RVA: u32 = 0x0063f5c4 - 0x400000;

    pub(super) const C_ANGRY_ANIMALS_SICK_CHANGE_KEY_RVA: u32 = 0x00641bd8 - 0x400000;
    pub(super) const C_PCT_SICK_KEY_RVA: u32 = 0x00641bcc - 0x400000;
    pub(super) const C_PCT_PROTESTORS_KEY_RVA: u32 = 0x00641bbc - 0x400000;
    pub(super) const C_ANGRY_HUNGRY_GUESTS_CHANGE_KEY_RVA: u32 = 0x00641ba0 - 0x400000;
    pub(super) const C_PCT_HUNGRY_KEY_RVA: u32 = 0x00641b94 - 0x400000;
    pub(super) const C_ANGRY_THIRSTY_GUESTS_CHANGE_KEY_RVA: u32 = 0x00641b78 - 0x400000;
    pub(super) const C_PCT_THIRSTY_KEY_RVA: u32 = 0x006420b4 - 0x400000;
    pub(super) const C_ANGRY_BATHROOM_GUESTS_CHANGE_KEY_RVA: u32 = 0x00642098 - 0x400000;
    pub(super) const C_PCT_BATHROOM_KEY_RVA: u32 = 0x00642088 - 0x400000;
    pub(super) const C_ANGRY_SOUVENIR_GUESTS_CHANGE_KEY_RVA: u32 = 0x0064206c - 0x400000;
    pub(super) const C_PCT_SOUVENIR_KEY_RVA: u32 = 0x0064205c - 0x400000;
    pub(super) const C_ANGRY_REMOVE_ANIMAL_CHANGE_KEY_RVA: u32 = 0x00642040 - 0x400000;
    pub(super) const C_ANGRY_TIRED_GUESTS_CHANGE_KEY_RVA: u32 = 0x00642028 - 0x400000;
    pub(super) const C_PCT_TIRED_KEY_RVA: u32 = 0x0064201c - 0x400000;
    pub(super) const C_ANGRY_TRASH_GUESTS_CHANGE_KEY_RVA: u32 = 0x00642004 - 0x400000;
    pub(super) const C_PCT_TRASH_KEY_RVA: u32 = 0x00641ff8 - 0x400000;
    pub(super) const C_CREATE_GUEST_CHANCE_VERY_LOW_KEY_RVA: u32 = 0x00641fdc - 0x400000;
    pub(super) const C_CREATE_GUEST_CHANCE_LOW_KEY_RVA: u32 = 0x00641fc4 - 0x400000;
    pub(super) const C_CREATE_GUEST_CHANCE_MED_KEY_RVA: u32 = 0x00641fac - 0x400000;
    pub(super) const C_CREATE_GUEST_CHANCE_HIGH_KEY_RVA: u32 = 0x00641f94 - 0x400000;
    pub(super) const C_CREATE_GUEST_CHANCE_VERY_HIGH_KEY_RVA: u32 = 0x00641f78 - 0x400000;
    pub(super) const C_LOAN_AVAILABLE_KEY_RVA: u32 = 0x00641f68 - 0x400000;
    pub(super) const C_HIGH_ZOO_VALUE_CHANGE_KEY_RVA: u32 = 0x00641f54 - 0x400000;
    pub(super) const C_LOW_ZOO_VALUE_CHANGE_KEY_RVA: u32 = 0x00641f40 - 0x400000;
    pub(super) const C_HIGH_ZOO_VALUE_KEY_RVA: u32 = 0x00641f30 - 0x400000;
    pub(super) const C_LOW_ZOO_VALUE_KEY_RVA: u32 = 0x00641f20 - 0x400000;
    pub(super) const C_HIGH_SPECIES_THRESHOLD_KEY_RVA: u32 = 0x00641f08 - 0x400000;
    pub(super) const C_HAPPY_DIVERSE_ANIMALS_CHANGE_KEY_RVA: u32 = 0x00641eec - 0x400000;
    pub(super) const C_LOW_SPECIES_THRESHOLD_KEY_RVA: u32 = 0x00641ed4 - 0x400000;
    pub(super) const C_ANGRY_DIVERSE_ANIMALS_CHANGE_KEY_RVA: u32 = 0x00641eb8 - 0x400000;
    pub(super) const C_HIGH_AVG_ANIMAL_HAPPY_THRESHOLD_KEY_RVA: u32 = 0x00641e98 - 0x400000;
    pub(super) const C_HAPPY_ANIMALS_CHANGE_KEY_RVA: u32 = 0x00641e84 - 0x400000;
    pub(super) const C_LOW_AVG_ANIMAL_HAPPY_THRESHOLD_KEY_RVA: u32 = 0x00641e68 - 0x400000;
    pub(super) const C_ANGRY_ANIMALS_CHANGE_KEY_RVA: u32 = 0x00641e54 - 0x400000;
    pub(super) const C_HIGH_AVG_GUEST_HAPPY_THRESHOLD_KEY_RVA: u32 = 0x00641e38 - 0x400000;
    pub(super) const C_HAPPY_GUEST_CHANGE_KEY_RVA: u32 = 0x00641e24 - 0x400000;
    pub(super) const C_LOW_AVG_GUEST_HAPPY_THRESHOLD_KEY_RVA: u32 = 0x00641e08 - 0x400000;
    pub(super) const C_ANGRY_GUEST_CHANGE_KEY_RVA: u32 = 0x00641df4 - 0x400000;
    pub(super) const C_ITEM_CHEAP_KEY_RVA: u32 = 0x00641de8 - 0x400000;
    pub(super) const C_ITEM_EXPENSIVE_KEY_RVA: u32 = 0x00641dd8 - 0x400000;
    pub(super) const C_HIGH_ZOO_ESTHETIC_KEY_RVA: u32 = 0x00641dc4 - 0x400000;
    pub(super) const C_HIGH_ZOO_ESTHETIC_CHANGE_KEY_RVA: u32 = 0x00641dac - 0x400000;
    pub(super) const C_LOW_ZOO_ESTHETIC_KEY_RVA: u32 = 0x00641d9c - 0x400000;
    pub(super) const C_LOW_ZOO_ESTHETIC_CHANGE_KEY_RVA: u32 = 0x00641d84 - 0x400000;
    pub(super) const C_RESEARCH_COST_KEY_RVA: u32 = 0x00641d74 - 0x400000;
    pub(super) const C_ADMISSION_MULTIPLE_KEY_RVA: u32 = 0x00641d60 - 0x400000;
    pub(super) const C_DONATIONS_AVAIL_KEY_RVA: u32 = 0x00641d50 - 0x400000;
    pub(super) const C_DONATION_LOW_KEY_RVA: u32 = 0x00641d40 - 0x400000;
    pub(super) const C_DONATION_HIGH_KEY_RVA: u32 = 0x00641d30 - 0x400000;
    pub(super) const C_DONATION_CHANCE_KEY_RVA: u32 = 0x00641d20 - 0x400000;
    pub(super) const C_SPECIES_AVAILABLE_KEY_RVA: u32 = 0x00641d0c - 0x400000;
    pub(super) const C_MEMBERSHIP_JOIN_FACTOR_KEY_RVA: u32 = 0x00641cf4 - 0x400000;
    pub(super) const C_MEMBERSHIP_JOIN_HAPPINESS_KEY_RVA: u32 = 0x00641cd8 - 0x400000;
    pub(super) const C_ENDOWMENT_GIFT_LOW_KEY_RVA: u32 = 0x00641cc4 - 0x400000;
    pub(super) const C_ENDOWMENT_GIFT_HIGH_KEY_RVA: u32 = 0x00641cb0 - 0x400000;
    pub(super) const C_MEMBERSHIP_JOIN_CHANCE_KEY_RVA: u32 = 0x00641c98 - 0x400000;
    pub(super) const C_PRICING_FACTOR_KEY_RVA: u32 = 0x00641c88 - 0x400000;
    pub(super) const C_DONATION_FACTOR_KEY_RVA: u32 = 0x00641c78 - 0x400000;
    pub(super) const C_BUILDING_USE_COST_DEFAULT_KEY_RVA: u32 = 0x00641c60 - 0x400000;
    pub(super) const C_BUILDING_USE_COST_MAX_KEY_RVA: u32 = 0x00641c4c - 0x400000;
    pub(super) const C_ZOO_DOO_RECYCLING_AMOUNT_KEY_RVA: u32 = 0x00641c34 - 0x400000;
    pub(super) const C_MIN_ADULT_ADMISSION_PRICE_KEY_RVA: u32 = 0x00641c1c - 0x400000;
    pub(super) const C_MAX_ADULT_ADMISSION_PRICE_KEY_RVA: u32 = 0x00641c04 - 0x400000;

    /// "cAdultAdmission" - a real, always-present 5-`f32` list in the shipped `economy.cfg` (confirmed by
    /// inspecting the live game install locally, not committed anywhere in this repo). Read via
    /// [`super::GET_FLOAT_LIST_FIXED`] and copied into [`super::raw_globals::PRICE_TIER_BOUNDARY_0_RVA`]`..`
    /// [`super::raw_globals::PRICE_TIER_BOUNDARY_4_RVA`] - see [`super::ZooStatus::override_config`]'s own
    /// doc comment for the full resolution of what was originally an opaque `FUN_00591c0f` tail-call.
    pub(super) const C_ADULT_ADMISSION_KEY_RVA: u32 = 0x00641b44 - 0x400000;
}

/// Reimplementation of `ZooStatus::fZooMessage` (`0x0049ce6b`), Stage 4. Per `ZooStatus_fZooMessage.c`
/// (read in full): a pure one-line forward to `BFUIMgr::displayMessage` (`generated.rs`'s
/// `bfuimgr::DISPLAY_MESSAGE_0` - confirmed against `BFUIMgr_displayMessage_0.asm`'s `RET 0x18`, six
/// stack args past the thiscall `this`, matching the six params here exactly) on the live
/// `GLOBAL_BFUIMgr` singleton, with the trailing `(true, false)` flags hardcoded exactly as vanilla's
/// call site does. Takes no `this` and touches no `ZooStatus` state, matching the plan's own conclusion
/// (`fChance`/`fZooMessage` are free-standing helpers, not real methods).
///
/// A free function, not a `ZooStatus` method, for the same reason. Resolves the one dependency that
/// previously forced every caller ([`ZooStatus::admission_message`], [`ZooStatus::message_checks`]) to
/// stay a call-through to real vanilla `F_ZOO_MESSAGE` - see [`raw_globals::GLOBAL_BFUIMGR_RVA`]'s doc
/// comment for where the address came from.
/// A real, vanilla-allocator-owned `std::string` - `{char* ptr; u32 len; u32 capacity}`, 12 bytes, no
/// small-string-optimization buffer (same confirmed layout as `ztgamemgr_menumusichandler.rs`'s own
/// `VanillaString`, which this mirrors - not shared cross-module since each class-reimplementation file
/// in this codebase stays otherwise self-contained). [`Self::new`] builds one via real vanilla
/// `msvc_std_basic_string::BASIC_STRING_2` (iterator-range constructor); [`Self::rvo_target`] instead
/// hands a zeroed instance to a vanilla function using the hidden-return-pointer (RVO) convention (e.g.
/// [`GET_MONEY_TEXT_0`]) to construct into directly - either way, [`Drop`] always tears it down through
/// the real vanilla destructor, never Rust's allocator (`CLAUDE.md`'s cross-allocator hazard).
#[repr(C)]
struct VanillaString {
    ptr: *mut u8,
    len: u32,
    capacity: u32,
}

impl VanillaString {
    fn new(s: &str) -> Self {
        let mut this = VanillaString { ptr: std::ptr::null_mut(), len: 0, capacity: 0 };
        let start = s.as_ptr();
        let end = unsafe { start.add(s.len()) };
        unsafe {
            BASIC_STRING_2.original()(&mut this as *mut VanillaString as *const c_void, start as *const u32, end as i32, 0);
        }
        this
    }

    /// A zeroed instance suitable as an RVO out-parameter for a vanilla function that placement-constructs
    /// its return value at the address it's given (the caller-supplies-uninitialized-destination-memory
    /// convention MSVC uses for by-value `std::string` returns) - the callee never reads this memory
    /// before writing it, so a zeroed instance is a safe destination.
    fn rvo_target() -> Self {
        VanillaString { ptr: std::ptr::null_mut(), len: 0, capacity: 0 }
    }

    fn as_ptr(&self) -> *const u32 {
        self as *const VanillaString as *const u32
    }

    fn as_str(&self) -> std::borrow::Cow<'_, str> {
        if self.ptr.is_null() || self.len == 0 {
            return std::borrow::Cow::Borrowed("");
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len as usize) };
        String::from_utf8_lossy(bytes)
    }
}

impl Drop for VanillaString {
    fn drop(&mut self) {
        unsafe { BASIC_STRING_0.original()(self as *mut VanillaString as *const c_void) };
    }
}

/// Real vanilla `BFConfigFile::getFloatList` (`0x00591a5a`), corrected locally rather than via
/// `generated.rs`'s own `bfconfigfile::GET_FLOAT_LIST` entry, which is missing a parameter.
/// `generated.rs`'s entry declares `fn(this, section, key) -> *const u32` (2 explicit args), matching
/// this decompile's own `.meta` (`getFloatList(BFConfigFile*, char*, char*)`) - but `ZooStatus_override.asm`'s
/// raw push order for this exact call site (`PUSH key; PUSH section; PUSH <local-vector-address>; MOV
/// ECX,this; CALL`, i.e. 3 real stack args, reversed for right-to-left push order into declaration order
/// `(this, out_vector, section, key)`) proves a fourth argument - a hidden out-parameter pointer for the
/// `std::vector<float>` return value, the standard MSVC ABI for a member function returning a non-POD by
/// value - is real and the `.meta`/`generated.rs` 2-param shape simply dropped it. Per `CLAUDE.md`,
/// `generated.rs` itself is never hand-edited for this - this local, corrected `FunctionDef` is the
/// sanctioned workaround ([`FunctionDef::new`]) until a future regen fixes the real entry.
const GET_FLOAT_LIST_FIXED: FunctionDef<unsafe extern "thiscall" fn(*const u32, *mut u32, u32, u32) -> *mut u32> = FunctionDef::new(0x00591a5a);

/// Real vanilla `ZooStatus::getStatus` (`0x0041dd64`), corrected locally rather than via `generated.rs`'s
/// own `zoostatus::GET_STATUS` entry, which declares the return type `*const f32`. `ZooStatus_getStatus.c`'s
/// own `float10 *` return type is the same return-by-hidden-pointer decompiler artifact this plan's
/// Status header already flagged when the regen first landed (an x87/`float10` scalar return rendered as
/// a pointer) - but `ZooStatus_getStatus.asm` (read in full, the `.c` decompile itself is unusable, pure
/// pointer-arithmetic noise on a `float10*`) settles it unambiguously: every path ends in a plain `FLD
/// float ptr [...]` immediately before `RET 0xc`, the standard x87-register (`ST(0)`) scalar-float return
/// convention every other `-> f32` `FunctionDef` in this codebase already uses (e.g. `ztguest.rs`'s
/// `F_ESTHETIC_BONUS_MEGATILE`, already hooked live) - never a return-by-pointer/RVO convention. Per
/// `CLAUDE.md`, `generated.rs` itself is never hand-edited for this - this local, corrected `FunctionDef`
/// is the sanctioned workaround ([`FunctionDef::new`]) until a future regen fixes the real entry. `pub(crate)`
/// so `reimplementation_tests` can drive the real vanilla pole directly for `ZOOSTATUS_GET_STATUS`.
pub(crate) const GET_STATUS_FIXED: FunctionDef<unsafe extern "thiscall" fn(*const u32, i32, i32, i32) -> f32> = FunctionDef::new(0x0041dd64);

/// A real, vanilla-allocator-owned `std::vector<float>` - the standard MSVC 3-pointer layout
/// (`{begin, end, cap_end}`, 12 bytes) [`GET_FLOAT_LIST_FIXED`] constructs into via the same
/// hidden-return-pointer (RVO) convention [`VanillaString::rvo_target`] uses.
///
/// **Deliberately never freed.** `ZooStatus_override.c`'s own teardown for this exact buffer (read in
/// full) is a real, concrete algorithm - not ambiguous decompiler noise - but it pushes the buffer onto
/// vanilla's small-object freelist (`DAT_00638000`-indexed, bucketed by `(byte_capacity - 1) >> 3`) for
/// anything `<= 0x80` bytes, and only calls `operator_delete` for larger ones. That freelist's exact
/// semantics are the same one `ztshow.rs`'s own `start` doc comment already flags as "not independently
/// confirmed" and deliberately skips reproducing, for the same reason: this codebase has no working,
/// live-tested example of writing to it, and getting the bucket math or protocol wrong risks exactly the
/// cross-allocator heap corruption class `CLAUDE.md`'s Reimplementation Pattern section warns about.
/// Calling `operator_delete` unconditionally instead would dodge that specific risk but isn't obviously
/// correct either - real vanilla deliberately avoids calling it for small buffers, which only makes sense
/// if `operator_delete` doesn't actually know how to reclaim a block this allocator produced. Per
/// `CLAUDE.md`'s own guidance for exactly this situation ("build a leak-only teardown path ... rather
/// than reusing the normal cleanup ... deliberately leak anything vanilla's own allocator produced"),
/// this type has no `Drop` impl at all - the ~20-32 byte buffer is leaked once per [`ZooStatus::override_config`]
/// call (a handful of times per real game session, not a hot path), never freed through either allocator.
#[repr(C)]
struct VanillaFloatVector {
    begin: *mut f32,
    end: *mut f32,
    cap_end: *mut f32,
}

impl VanillaFloatVector {
    fn rvo_target() -> Self {
        VanillaFloatVector { begin: std::ptr::null_mut(), end: std::ptr::null_mut(), cap_end: std::ptr::null_mut() }
    }

    fn as_ptr(&mut self) -> *mut u32 {
        self as *mut VanillaFloatVector as *mut u32
    }

    fn as_slice(&self) -> &[f32] {
        if self.begin.is_null() {
            return &[];
        }
        let len = unsafe { self.end.offset_from(self.begin) } as usize;
        unsafe { std::slice::from_raw_parts(self.begin, len) }
    }
}

/// The vanilla `((x + 100) * scale) / 200` idiom, shared by two independent call sites that would
/// otherwise be two hand-written transcriptions of the same formula with different scale constants:
/// [`Self::rating_checks`]' own animal/guest score contributions (`scale = 25`, unconditional) and
/// `ztgamemgr.rs`'s `rating_from_metric` (`scale = 100`, gated behind a population != 0 check before
/// calling this). Pulled out so a future edit to one can't silently diverge from the other.
pub(crate) fn scaled_rating_metric(metric: i32, scale: i32) -> i32 {
    (metric + 100) * scale / 200
}

/// The three outcomes [`ZooStatus::f_grant_donation`] picks between, given `count_after_increment`
/// (`donation_count_this_period` immediately after this call's own `+= 1.0`) and `bound`
/// (`donation_count_bound`). Pulled out as its own pure function so this branch selection has direct unit
/// test coverage independent of any live call - see [`ZooStatus::f_grant_donation`]'s own doc comment for
/// why a live comparison test can't safely exercise the two message-firing variants.
#[derive(Debug, PartialEq, Eq)]
enum DonationOutcome {
    /// This call's increment pushed the counter exactly one past the bound - the one-time "no more
    /// donations this period" message trigger.
    BoundJustCrossed,
    /// Counter is still within bound - roll and grant a donation.
    Grant,
    /// Counter was already past the bound before this call - silent no-op.
    AlreadyPastBound,
}

fn donation_outcome(count_after_increment: f32, bound: i32) -> DonationOutcome {
    if count_after_increment == (bound + 1) as f32 {
        DonationOutcome::BoundJustCrossed
    } else if count_after_increment > bound as f32 {
        DonationOutcome::AlreadyPastBound
    } else {
        DonationOutcome::Grant
    }
}

/// Loads a game string by id through real vanilla `ZTApp::getApp`/`BFApp::loadString`, matching
/// `ZooStatus_fGrantDonation.asm`'s own call shape exactly (`loadString` takes no length argument - it
/// writes into a fixed-size caller buffer, so this uses the same `512`-byte size vanilla's own stack
/// buffer (`local_600`) does; no more/less safe than vanilla's own call here). The buffer is
/// Rust-owned/stack-allocated, so there's no cross-allocator concern - vanilla only ever writes bytes
/// into memory we supplied, never allocates/frees anything of its own here.
fn load_localized_string(id: u32) -> String {
    let app_ptr = unsafe { GET_APP.original()() };
    let mut buffer = [0u8; 512];
    unsafe { LOAD_STRING.original()(app_ptr, id as *const u32, buffer.as_mut_ptr()) };
    unsafe { CStr::from_ptr(buffer.as_ptr() as *const i8) }.to_string_lossy().into_owned()
}

/// Real vanilla `bfinternat::getMoneyText` - the same localized currency formatting vanilla's own
/// `fGrantDonation` uses for the donation-amount text, via the RVO convention (see
/// [`VanillaString::rvo_target`]).
fn format_money_text(amount: u32) -> String {
    let out = VanillaString::rvo_target();
    unsafe { GET_MONEY_TEXT_0.original()(out.as_ptr(), amount, 0) };
    out.as_str().into_owned()
}

/// Real vanilla `BFUIMgr::displayMessage` (the `std::string`-message overload,
/// `bfuimgr::DISPLAY_MESSAGE_1`) on the live `GLOBAL_BFUIMgr` singleton - the sibling of [`f_zoo_message`]
/// for the two call sites in [`ZooStatus::f_grant_donation`] that pass a formatted message string rather
/// than a plain string-id.
fn display_message_string(message: *const u32, priority: i32) {
    let bfuimgr_ptr = get_module_base("zoo.exe") as u32 + raw_globals::GLOBAL_BFUIMGR_RVA;
    unsafe { DISPLAY_MESSAGE_1.original()(bfuimgr_ptr as *const u32, message as *const i32, priority, std::ptr::null(), std::ptr::null(), true, false) };
}

fn f_zoo_message(message_id: *const u32, param_2: u32, tile: u32, entity: i32) {
    let bfuimgr_ptr = get_module_base("zoo.exe") as u32 + raw_globals::GLOBAL_BFUIMGR_RVA;
    unsafe {
        DISPLAY_MESSAGE_0.original()(
            bfuimgr_ptr as *const u32,
            message_id as u32,
            param_2 as i32,
            tile as *const u32,
            entity as *const u32,
            true,
            false,
        )
    };
}

/// Reads `entity_ptr + 0x128` (a `BFEntity`'s own "type" pointer) and dispatches that type's vtable slot
/// `0x1c` with the class-id sentinel at `type_check_arg_rva` - the "isKindOf"-style RTTI check
/// [`ZooStatus::calculate_sums`] uses to classify each live world entity. Same mechanism as
/// `ztmegatilemgr.rs`'s `entity_type_matches`/`ztshow.rs`'s `type_check` - each module keeps its own
/// copy rather than sharing one, matching this codebase's established per-module-duplication convention
/// for single-purpose raw address helpers (see `zoostatus.rs`'s own `GLOBAL_BFUIMGR_RVA` doc comment for
/// the same precedent). Returns `false` if the type pointer is null - `calculateSums.asm` has no such
/// guard before its *first* dispatch of this check (only before a later, redundant "confirm" dispatch -
/// see [`ZooStatus::calculate_sums`]'s doc comment for why that second call is skipped entirely here),
/// but a null type pointer is not observed in practice for any live world entity.
fn entity_type_matches(entity_ptr: u32, type_check_arg_rva: u32) -> bool {
    let entity_type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
    if entity_type_ptr == 0 {
        return false;
    }
    let vtable: u32 = get_from_memory(entity_type_ptr);
    let check_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
    let arg = get_module_base("zoo.exe") as u32 + type_check_arg_rva;
    check_fn(entity_type_ptr, arg)
}

/// Console diagnostic (`check_calc_sums_fixture()`) for whoever next builds a live-populated-world
/// comparison test for [`Self::calculate_sums`]/[`Self::message_checks`]/[`Self::rating_checks`] (the
/// existing `ZOOSTATUS_CALCULATE_SUMS`/`ZOOSTATUS_CHECKS` tests run before `run_load_live_zoo`, so today
/// they only exercise the zero-entity/zero-research degenerate branches - see
/// `zoostatus-implementation-plan.md`'s open live-test items). Read-only: walks
/// `GLOBAL_ZTWorldMgr`'s live entity array and map tiles exactly the way [`Self::calculate_sums`] does,
/// plus the live `GLOBAL_ZTResearchMgr` tree, and reports whether the *currently loaded* zoo actually
/// hits every condition a real comparison test would need to exercise - never writes to any entity or to
/// any `ZooStatus`. Meant to be run against a candidate test-save zoo (`OPENZT_TEST_ZOO`/
/// `reimplementation-test-zoo.zoo`) before wiring it into the battery, to confirm it's worth the effort
/// rather than discovering after the fact that (say) no guest is ever below the hunger threshold.
fn describe_calculate_sums_fixture() -> String {
    let base = get_module_base("zoo.exe") as u32;
    let need_threshold: i32 = get_from_memory(base + raw_globals::GUEST_NEED_THRESHOLD_RVA);

    let world = globals().ztworldmgr();

    let (mut guest_count, mut animal_count, mut building_count) = (0u32, 0u32, 0u32);
    let (mut guests_over_hunger, mut guests_under_hunger) = (0u32, 0u32);
    let (mut guests_over_thirst, mut guests_under_thirst) = (0u32, 0u32);
    let (mut guests_over_restroom, mut guests_under_restroom) = (0u32, 0u32);
    let (mut guests_over_tired, mut guests_under_tired) = (0u32, 0u32);
    let mut guest_flag_0x33c_count = 0u32;
    let mut guest_subflag_0x26c_count = 0u32;
    let mut animal_flag_0x3a7_count = 0u32;

    let mut entity_addr = world.entity_array_start();
    while entity_addr != world.entity_array_end() {
        let entity_ptr: u32 = get_from_memory(entity_addr);
        entity_addr += 4;
        if entity_ptr == 0 {
            continue;
        }

        if entity_type_matches(entity_ptr, raw_globals::GUEST_TYPE_CHECK_RVA) {
            guest_count += 1;

            let hunger: i32 = get_from_memory(entity_ptr + 0x2b0);
            if hunger > need_threshold { guests_over_hunger += 1 } else { guests_under_hunger += 1 }
            let thirst: i32 = get_from_memory(entity_ptr + 0x2b8);
            if thirst > need_threshold { guests_over_thirst += 1 } else { guests_under_thirst += 1 }
            let restroom: i32 = get_from_memory(entity_ptr + 0x2c8);
            if restroom > need_threshold { guests_over_restroom += 1 } else { guests_under_restroom += 1 }
            let tired: i32 = get_from_memory(entity_ptr + 0x2c0);
            if tired > need_threshold { guests_over_tired += 1 } else { guests_under_tired += 1 }

            let flag: u8 = get_from_memory(entity_ptr + 0x33c);
            if flag != 0 {
                guest_flag_0x33c_count += 1;
            }
            let sub_object: u32 = get_from_memory(entity_ptr + 0x26c);
            let sub_flag: i32 = get_from_memory(sub_object + 0x10);
            if sub_flag != 0 {
                guest_subflag_0x26c_count += 1;
            }
        } else if entity_type_matches(entity_ptr, raw_globals::ANIMAL_TYPE_CHECK_RVA) {
            animal_count += 1;
            let flag: u8 = get_from_memory(entity_ptr + 0x3a7);
            if flag != 0 {
                animal_flag_0x3a7_count += 1;
            }
        } else if entity_type_matches(entity_ptr, raw_globals::BUILDING_TYPE_CHECK_RVA) {
            building_count += 1;
        }
    }

    let mut blank_tiles: u32 = 0;
    for y in 0..world.map_y_size {
        for x in 0..world.map_x_size {
            let Some(tile) = world.get_tile_from_pos(IVec3::new(x as i32, y as i32, 0)) else { continue };
            let tile_addr = world.get_ptr_from_bftile(&tile);
            let a: u32 = get_from_memory(tile_addr + 4);
            let b: u32 = get_from_memory(tile_addr + 8);
            let c: u32 = get_from_memory(tile_addr + 0xc);
            let entity_ptr: u32 = get_from_memory(tile_addr + 0x10);
            if a == 0 && b == 0 && c == 0 && entity_ptr == 0 {
                blank_tiles += 1;
            }
        }
    }
    let total_tiles = world.map_x_size * world.map_y_size;
    let non_blank_tiles = total_tiles.saturating_sub(blank_tiles);

    let num_species: i32 = unsafe { GET_NUM_SPECIES.original()(globals().zthabitatmgr_ptr() as *const u32) };

    let (mut research_total, mut research_complete) = (0i32, 0i32);
    for branch in globals().ztresearchmgr().branches() {
        for category in branch.categories() {
            for program in category.programs() {
                research_total += 1;
                if program.is_complete() {
                    research_complete += 1;
                }
            }
        }
    }

    let line = |ok: bool, label: &str, detail: String| format!("[{}] {label}: {detail}\n", if ok { "OK" } else { "MISSING" });

    let mut out = String::from("=== calculate_sums / message_checks / rating_checks live-test fixture check ===\n");
    out.push_str(&line(guest_count > 0, "guests present", format!("{guest_count} guest(s)")));
    out.push_str(&line(
        guests_over_hunger > 0 && guests_under_hunger > 0,
        "hunger spread",
        format!("{guests_over_hunger} over / {guests_under_hunger} under threshold ({need_threshold})"),
    ));
    out.push_str(&line(
        guests_over_thirst > 0 && guests_under_thirst > 0,
        "thirst spread",
        format!("{guests_over_thirst} over / {guests_under_thirst} under threshold ({need_threshold})"),
    ));
    out.push_str(&line(
        guests_over_restroom > 0 && guests_under_restroom > 0,
        "restroom-need spread",
        format!("{guests_over_restroom} over / {guests_under_restroom} under threshold ({need_threshold})"),
    ));
    out.push_str(&line(
        guests_over_tired > 0 && guests_under_tired > 0,
        "tiredness spread",
        format!("{guests_over_tired} over / {guests_under_tired} under threshold ({need_threshold})"),
    ));
    out.push_str(&line(
        guest_flag_0x33c_count > 0,
        "guest_condition_counter_1 (guest +0x33c flag)",
        format!("{guest_flag_0x33c_count} guest(s) flagged"),
    ));
    out.push_str(&line(
        guest_subflag_0x26c_count > 0,
        "guest_condition_counter_2 (guest +0x26c sub-object flag)",
        format!("{guest_subflag_0x26c_count} guest(s) flagged"),
    ));
    out.push_str(&line(animal_count > 0, "animals present", format!("{animal_count} animal(s)")));
    out.push_str(&line(
        animal_flag_0x3a7_count > 0,
        "animal_condition_counter_1 (animal +0x3a7 flag)",
        format!("{animal_flag_0x3a7_count} animal(s) flagged"),
    ));
    out.push_str(&line(building_count > 0, "buildings present", format!("{building_count} building(s)")));
    out.push_str(&line(
        non_blank_tiles > 0,
        "non-blank map tiles present",
        format!("{non_blank_tiles} non-blank / {total_tiles} total tiles ({}x{})", world.map_x_size, world.map_y_size),
    ));
    out.push_str(&line(num_species > 0, "species present", format!("{num_species} species")));
    out.push_str(&line(research_total > 0, "research programs present", format!("{research_total} program(s)")));
    out.push_str(&line(
        research_complete > 0 && research_complete < research_total,
        "research mix (some complete, some not)",
        format!("{research_complete}/{research_total} complete"),
    ));

    out
}

/// Console diagnostic (`fixup_calc_sums_fixture()`), companion to [`describe_calculate_sums_fixture`]/
/// `check_calc_sums_fixture()`. Mutates the *values* on guest/animal entities already present in the
/// currently loaded zoo so the value-based conditions [`describe_calculate_sums_fixture`] checks for
/// pass - alternates existing guests between "every tracked need urgent" and "every tracked need
/// satisfied" (also setting one guest's `+0x33c` flag and one guest's `+0x26c` sub-object flag along the
/// way) and sets one animal's `+0x3a7` flag. After running this, save the game normally through the
/// game's own UI/`save()` to bake the changes into a `.zoo` file for `OPENZT_TEST_ZOO`.
///
/// **Deliberately does not touch anything else the checklist covers**, and this is a real scope
/// boundary, not laziness:
/// - **Guest/animal/building *presence*, and non-blank map tiles** can't be fabricated by a memory
///   write - creating a new entity needs `ZTWorldMgr`'s entity-creation path, which is the exact
///   cross-allocator/freelist hazard [`ZooStatus::f_create_guest`]'s/`financeChecks`'s own doc comments
///   already track as unsafe to do from Rust today (see `zoostatus-implementation-plan.md`'s "Open
///   risks"). If the checklist reports these missing, place more of that entity type in-game before
///   saving - there's no safe shortcut here.
/// - **The research completion mix** is left alone here too, even though no entity creation is
///   involved: forcing `current_progress >= target_cost` on a live `ZTResearchProgram` changes real,
///   vanilla-owned state that vanilla's own update loop polls and reacts to (`on_completion` dispatch -
///   building unlocks, discounts, etc.) - letting real research funding complete something naturally
///   (or loading a save that already has mixed progress) is the safe way to get this condition, not a
///   poke from here.
fn fixup_calc_sums_fixture() -> String {
    let base = get_module_base("zoo.exe") as u32;
    let need_threshold: i32 = get_from_memory(base + raw_globals::GUEST_NEED_THRESHOLD_RVA);
    let above = need_threshold.saturating_add(1);
    let below = (need_threshold - 1).max(0);

    let world = globals().ztworldmgr();

    let mut guest_index = 0u32;
    let mut animal_touched = false;
    let mut guest_flag_0x33c_set = false;
    let mut guest_flag_0x26c_set = false;

    let mut entity_addr = world.entity_array_start();
    while entity_addr != world.entity_array_end() {
        let entity_ptr: u32 = get_from_memory(entity_addr);
        entity_addr += 4;
        if entity_ptr == 0 {
            continue;
        }

        if entity_type_matches(entity_ptr, raw_globals::GUEST_TYPE_CHECK_RVA) {
            let value = if guest_index.is_multiple_of(2) { above } else { below };
            save_to_memory(entity_ptr + 0x2b0, value);
            save_to_memory(entity_ptr + 0x2b8, value);
            save_to_memory(entity_ptr + 0x2c8, value);
            save_to_memory(entity_ptr + 0x2c0, value);
            guest_index += 1;

            if !guest_flag_0x33c_set {
                save_to_memory(entity_ptr + 0x33c, 1u8);
                guest_flag_0x33c_set = true;
            }
            if !guest_flag_0x26c_set {
                let sub_object: u32 = get_from_memory(entity_ptr + 0x26c);
                if sub_object != 0 {
                    save_to_memory(sub_object + 0x10, 1i32);
                    guest_flag_0x26c_set = true;
                }
            }
        } else if !animal_touched && entity_type_matches(entity_ptr, raw_globals::ANIMAL_TYPE_CHECK_RVA) {
            save_to_memory(entity_ptr + 0x3a7, 1u8);
            animal_touched = true;
        }
    }

    let mut out = String::from("=== calculate_sums fixture fixup ===\n");
    out.push_str(&format!("guests rewritten: {guest_index} (needs threshold {need_threshold}, alternating {above}/{below})\n"));
    if guest_index < 2 {
        out.push_str("  NOTE: fewer than 2 guests present - a needs spread needs at least 2 to show both sides of the threshold\n");
    }
    out.push_str(&format!("guest +0x33c flag set: {guest_flag_0x33c_set}\n"));
    out.push_str(&format!(
        "guest +0x26c sub-object flag set: {guest_flag_0x26c_set}{}\n",
        if guest_flag_0x26c_set { "" } else { " (no guest had a non-null +0x26c sub-object)" }
    ));
    out.push_str(&format!("animal +0x3a7 flag set: {animal_touched}\n"));
    out.push_str("Not touched (arrange in-game before saving, see this function's own doc comment): guest/animal/building presence, non-blank tiles, research completion mix.\n");
    out.push_str("Save the game now (normal in-game save) to bake these changes into a .zoo file.\n");
    out
}

/// Writes `value`'s raw bytes through real vanilla `WriteBytesToFile` (`standalone::WRITE_BYTES_TO_FILE`,
/// the `fwrite`-shaped primitive every `*::save` in this codebase goes through - `true` on success).
/// `.hooked()`, not `.original()`: a `reimplementation-tests` build's `io_redirect` module detours this
/// exact address to redirect the write into an in-memory capture buffer when a capture window is
/// active, and `.hooked()` is this codebase's established way to reach whatever real address currently
/// holds (see `CLAUDE.md`'s Reimplementation Pattern section) - `.original()` would bypass that
/// redirect entirely in a debug build's trampoline-routed table.
fn write_bytes_to_file<T>(value: &T, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(value as *const T as *const u32, mem::size_of::<T>() as u32, 1, file) == 1 }
}

/// Reads `value`'s raw bytes through real vanilla `deallocate` (`standalone::DEALLOCATE`, the
/// `fread`-shaped primitive despite its misleading decompiler-given name - see
/// [`write_bytes_to_file`]'s counterpart doc comment for the full `.hooked()`/`io_redirect` reasoning,
/// which applies identically here).
fn read_bytes<T>(value: &mut T, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(value as *mut T as *const u32, mem::size_of::<T>() as u32, 1, file as *const u8) == 1 }
}

impl ZooStatus {
    /// Reimplementation of `ZooStatus::init` (`0x004c2683`), Stage 2 of the implementation plan. Per
    /// `ZooStatus_init.asm`/`.c` (both read in full) - ground truth is the `.asm`, since the `.c`'s
    /// `undefined4` fields don't distinguish int from float (every constant here was classified by
    /// checking whether its `MOV` load is a clean small integer or a real IEEE754 bit pattern, not by
    /// the decompiler's own rendering - see the two type corrections in this struct's field docs that
    /// this pass uncovered).
    ///
    /// One real dependency is deliberately a call-through, not reimplemented, matching every other
    /// not-yet-ported sibling method in this codebase:
    /// - [`GET_OLD_DATE`] seeds the escape timestamp - already a resolved, always-safe-to-call
    ///   dependency (see the plan's Dependencies section).
    ///
    /// [`Self::set_adult_admission_price`] is now native (Stage 5), called directly here rather than
    /// through real vanilla `SET_ADULT_ADMISSION_PRICE`. [`Self::override_config`] is now native too
    /// (Stage 6), called exactly where vanilla's own `init` calls it, with exactly the same arguments -
    /// see that method's own doc comment.
    ///
    /// Deliberately **not** reproduced: `init`'s own two `BFIniFile::read` calls for
    /// `AI`/`cEscapedAnimalChange` and `AI`/`cEscapedAnimalTime` (write into process globals, not into
    /// `this` - genuinely out of `ZooStatus`'s own scope) and the `AI`/`maxGuests` read that seeds
    /// [`Self::max_guests`] (writes into `this`, but constructing the real `BFIniFile::read` call's
    /// `std::string` arguments is an untouched dependency in this codebase - see `ztgamemgr.rs`'s
    /// `initMenuMusic` doc comment for the same class of gap already flagged there). `max_guests` is
    /// hardcoded to its vanilla default (`1000`) instead - see the live test's masked-byte-range note
    /// for the resulting comparison caveat.
    ///
    /// Byte-order-independent: every write here is either an unconditional constant or (for
    /// [`Self::admission_price`]) deliberately **not written at all**, matching vanilla exactly -
    /// `setAdultAdmissionPrice` is called with whatever was already at that offset before `init` ran
    /// (vanilla's own `init` never writes `+0x1150` itself either), so this method must not assign
    /// [`Self::admission_price`] before that call.
    pub fn init(&mut self, config: *const c_void) {
        self.rating_check_interval = 10000;
        self.message_check_interval = 10000;
        self.newguest_check_interval = 10000;
        self.rating_check_elapsed = 0;
        self.message_check_elapsed = 0;
        self.newguest_check_elapsed = 0;
        self.finance_check_pending = false;
        self.zoo_rating_current = 0;
        self.num_animals = 0;
        // num_species/animal_condition_counter_1 (+0x24/+0x28) are deliberately left untouched -
        // `init.asm` does not zero them either (they're always freshly recomputed by `calculateSums`).
        self.num_tired_guests = 0;
        self.num_hungry_guests = 0;
        self.num_thirst_guests = 0;
        self.num_guests_restroom_need = 0;
        self.guest_condition_counter_1 = 0;
        self.guest_condition_counter_2 = 0;
        self.guest_tile_count = 0;
        self.field_0x48 = 2;
        self.field_0x4c = 10000;
        self.field_0x50 = 0;
        self.field_0x54 = 0;
        self.field_0x58 = 0;
        self.animal_rating_metric = 0;
        self.guest_rating_metric = 0;
        self.non_blank_tile_fraction = 0.0;
        self.research_completion_percent = 0;

        self.zero_history_regions();
        self.current_month_index = 1;
        self.current_year_index = 0;

        // AI/maxGuests config read skipped - see this method's doc comment.
        self.max_guests = 1000;

        self.message_threshold_0x70 = 0.5;
        self.message_threshold_0x74 = 0.5;
        self.message_threshold_0x7c = 0.5;
        self.message_threshold_0x84 = 0.5;
        self.message_threshold_0x8c = 0.5;
        self.message_threshold_0x94 = 0.5;
        self.message_threshold_0xa0 = 0.5;
        self.message_threshold_0xa8 = 0.5;
        self.angry_animals_sick_change = 0;
        self.angry_hungry_guests_change = 0;
        self.angry_thirsty_guests_change = 0;
        self.angry_bathroom_guests_change = 0;
        self.angry_souvenir_guests_change = 0;
        self.angry_remove_animal_change = 0;
        self.angry_tired_guests_change = 0;
        self.angry_trash_guests_change = 0;

        self.guest_type_arrival_multiplier = [1; 5];

        self.loan_available = 0;
        self.high_zoo_value_change = 0;
        self.low_zoo_value_change = 0;
        self.high_zoo_value = f32::from_bits(0x47c35000); // 97400.0
        self.low_zoo_value = 0.0;
        self.high_species_threshold = 10;
        self.happy_diverse_animals_change = 0;
        self.low_species_threshold = 2;
        self.angry_diverse_animals_change = 0;
        self.high_avg_animal_happy_threshold = 90;
        self.happy_animals_change = 10;
        self.low_avg_animal_happy_threshold = 0;
        self.angry_animals_change = 0;
        self.high_avg_guest_happy_threshold = 90;
        self.happy_guest_change = 10;
        self.low_avg_guest_happy_threshold = 0;
        self.angry_guest_change = 0;
        self.item_cheap = f32::from_bits(0x3f4ccccd); // 0.7999...
        self.item_expensive = f32::from_bits(0x3f99999a); // 1.2

        self.donation_count_this_period = 0.0;
        self.donation_count_bound = 3;
        self.donation_amount_min = 10000;
        self.donation_amount_max = 20000;

        self.donation_chance_percent = 0;
        self.species_rating_cap = 44;
        self.membership_join_happiness = 80;
        self.membership_join_factor = 10;
        self.endowment_gift_low = 10000;
        self.endowment_gift_high = 20000;
        self.membership_join_chance = 50;
        self.high_zoo_esthetic = f32::from_bits(0x3e4ccccd); // 0.2
        self.high_zoo_esthetic_change = 0;
        self.low_zoo_esthetic = 0.0;
        self.low_zoo_esthetic_change = 0;
        self.research_cost = f32::from_bits(0x447a0000); // 1000.0

        self.pricing_factor = f32::from_bits(0x3f800000); // 1.0
        self.donation_factor = f32::from_bits(0x2a9c5fff);
        self.building_use_cost_default = f32::from_bits(0x41e80000); // 29.0
        self.building_use_cost_max = f32::from_bits(0x42c80000); // 100.0
        self.zoo_doo_recycling_amount = f32::from_bits(0x42480000); // 50.0
        self.admission_income_multiplier = f32::from_bits(0x3f800000); // 1.0

        let old_date = unsafe { GET_OLD_DATE.original()() } as u64;
        self.last_animal_escape_timestamp_low = old_date as u32;
        self.last_animal_escape_timestamp_high = (old_date >> 32) as u32;

        self.admission_price_min = 0.0;
        self.admission_price_max = f32::from_bits(0x42c80000); // 100.0

        // admission_price (+0x1150) is deliberately never written above - see this method's doc
        // comment. Whatever the buffer already held gets clamped in place by the call below.
        let seed_price = self.admission_price;
        self.set_adult_admission_price(seed_price);

        self.override_config(config);
    }

    /// Reimplementation of `ZooStatus::override` (`0x004bbef5`), Stage 6 of the implementation plan. Per
    /// `ZooStatus_override.c` (read in full, now fully - see below). The sidecar `.asm` still only covers
    /// the leading three `[checks]` reads plus the `getFloatList` size check up to its `JMP 0x00591c0f`
    /// tail-call (the raw-byte dump tool that produced it stops at that boundary), but the decompiler's
    /// own reconstructed control flow graph - the `.c` - now includes a real, regenerated body for that
    /// tail-called continuation (previously an opaque `FUN_00591c0f` with no decompile at all - see below)
    /// and is what this port follows for the whole method.
    ///
    /// Real vanilla `this` argument is `config: *const BFConfigFile` from the caller (`ZooStatus::init`'s
    /// own tail, or `ZTGameMgr::overrideNewGameDefaults`) - unlike `ztshowmgr.rs`'s `init_show_params`,
    /// this method never constructs or tears down that `BFConfigFile` itself, it only reads one the
    /// caller already owns.
    ///
    /// Null-checks `config` first, exactly like vanilla - passing null (e.g. from a caller that has no
    /// config loaded) is a real, intentional early-return, not a bug.
    ///
    /// Every `getInt`/`getFloat` call below is a direct transcription of `ZooStatus_override.c`'s own
    /// section/key/destination triples, in vanilla's exact call order (see [`override_config_keys`] for
    /// the string-literal addresses, each one copied straight from that file's own Ghidra symbol names).
    /// `[checks]` is real vanilla's own section name for the first three; every other key lives in
    /// `[characteristics]`.
    ///
    /// **The `cAdultAdmission` `getFloatList` call, resolved.** A regenerated decompile of this method
    /// (the `FUN_00591c0f` tail-call site now has a real body instead of an opaque, undecompiled function)
    /// shows the size-checked branch is a plain copy loop, not an early return:
    /// ```c
    /// BFConfigFile::getFloatList(param_1,(char *)&local_c,s_characteristics_0063f5c4);
    /// if ((local_8 - (int)local_c & 0xfffffffcU) == 0x14) {
    ///   puVar4 = (undefined4 *)((int)&UNK_006392a4 + 8);
    ///   do {
    ///     *puVar4 = *(undefined4 *)((int)puVar4 + (int)(local_c + -0x18e4ab));
    ///     puVar4 = puVar4 + 1;
    ///   } while (puVar4 < &DAT_006392c0);
    /// }
    /// ```
    /// `&UNK_006392a4 + 8 = 0x6392ac`, and the loop's own bound (`&DAT_006392c0`) makes the write range
    /// `0x6392ac..0x6392c0` - five contiguous `f32` slots, copied straight from the `cAdultAdmission`
    /// vector (confirmed 5 elements exactly, per the size check). This is a **global**, not a `this`-relative
    /// write, matching what an earlier pass of this method's own doc comment already deduced from
    /// process-of-elimination (every `this`-relative offset `override` touches was already fully
    /// accounted for elsewhere with zero bytes spare) and from the macOS decompile's structurally
    /// equivalent, non-early-returning version of the same branch: `0x6392ac..0x6392c0` is exactly
    /// [`raw_globals::PRICE_TIER_BOUNDARY_0_RVA`]`..=`[`raw_globals::PRICE_TIER_BOUNDARY_4_RVA`] - the
    /// same four boundaries [`Self::price_tier`] (already ported, Stage 4) already reads, plus one more
    /// slot that method doesn't consume. So `cAdultAdmission`'s 5 config values
    /// (real `economy.cfg`: `49`/`29`/`19`/`9`/`0`) are genuinely the live source of `newguestChecks`'
    /// price-tier bucketing - not hardcoded constants, as this plan originally assumed when those globals
    /// were first named. Implemented here via [`GET_FLOAT_LIST_FIXED`]/[`VanillaFloatVector`] (see their
    /// own doc comments for the `generated.rs`-gap workaround and the deliberate-leak teardown choice) -
    /// live-verified byte-identical against real vanilla by `ZOOSTATUS_OVERRIDE`
    /// (`reimplementation_tests/mod.rs`), which runs both poles against the same real, loaded
    /// `economy.cfg` and confirms this write lands in the same place real vanilla's does.
    pub fn override_config(&mut self, config: *const c_void) {
        if config.is_null() {
            return;
        }

        use override_config_keys::*;

        let config_ptr = config as *const u32;
        let base = get_module_base("zoo.exe") as u32;

        let get_i = |section: u32, key: u32, out: *mut i32| unsafe {
            GET_INT.original()(config_ptr, base + section, base + key, out as *const u32);
        };
        let get_f = |section: u32, key: u32, out: *mut f32| unsafe {
            GET_FLOAT.original()(config_ptr, base + section, base + key, out as *const f32);
        };

        get_i(CHECKS_SECTION_RVA, RATING_KEY_RVA, &raw mut self.rating_check_interval);
        get_i(CHECKS_SECTION_RVA, MESSAGE_KEY_RVA, &raw mut self.message_check_interval);
        get_i(CHECKS_SECTION_RVA, NEWGUEST_KEY_RVA, &raw mut self.newguest_check_interval);

        // cAdultAdmission getFloatList + copy loop (formerly an opaque `FUN_00591c0f` tail-call - see
        // this method's own doc comment for how that got resolved). Writes into a global, not `this` -
        // see raw_globals::PRICE_TIER_BOUNDARY_0_RVA..`_4_RVA`'s own doc comments.
        let mut admission_tiers = VanillaFloatVector::rvo_target();
        unsafe {
            GET_FLOAT_LIST_FIXED.original()(
                config_ptr,
                admission_tiers.as_ptr(),
                base + CHARACTERISTICS_SECTION_RVA,
                base + C_ADULT_ADMISSION_KEY_RVA,
            );
        }
        if admission_tiers.as_slice().len() == 5 {
            let boundary_rvas = [
                raw_globals::PRICE_TIER_BOUNDARY_0_RVA,
                raw_globals::PRICE_TIER_BOUNDARY_1_RVA,
                raw_globals::PRICE_TIER_BOUNDARY_2_RVA,
                raw_globals::PRICE_TIER_BOUNDARY_3_RVA,
                raw_globals::PRICE_TIER_BOUNDARY_4_RVA,
            ];
            for (&rva, &value) in boundary_rvas.iter().zip(admission_tiers.as_slice()) {
                save_to_memory(base + rva, value);
            }
        }

        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_ANIMALS_SICK_CHANGE_KEY_RVA, &raw mut self.angry_animals_sick_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_SICK_KEY_RVA, &raw mut self.message_threshold_0x70);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_PROTESTORS_KEY_RVA, &raw mut self.message_threshold_0x74);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_HUNGRY_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_hungry_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_HUNGRY_KEY_RVA, &raw mut self.message_threshold_0x7c);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_THIRSTY_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_thirsty_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_THIRSTY_KEY_RVA, &raw mut self.message_threshold_0x84);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_BATHROOM_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_bathroom_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_BATHROOM_KEY_RVA, &raw mut self.message_threshold_0x8c);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_SOUVENIR_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_souvenir_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_SOUVENIR_KEY_RVA, &raw mut self.message_threshold_0x94);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_REMOVE_ANIMAL_CHANGE_KEY_RVA, &raw mut self.angry_remove_animal_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_TIRED_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_tired_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_TIRED_KEY_RVA, &raw mut self.message_threshold_0xa0);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_TRASH_GUESTS_CHANGE_KEY_RVA, &raw mut self.angry_trash_guests_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_PCT_TRASH_KEY_RVA, &raw mut self.message_threshold_0xa8);

        get_i(CHARACTERISTICS_SECTION_RVA, C_CREATE_GUEST_CHANCE_VERY_LOW_KEY_RVA, &raw mut self.guest_type_arrival_multiplier[0]);
        get_i(CHARACTERISTICS_SECTION_RVA, C_CREATE_GUEST_CHANCE_LOW_KEY_RVA, &raw mut self.guest_type_arrival_multiplier[1]);
        get_i(CHARACTERISTICS_SECTION_RVA, C_CREATE_GUEST_CHANCE_MED_KEY_RVA, &raw mut self.guest_type_arrival_multiplier[2]);
        get_i(CHARACTERISTICS_SECTION_RVA, C_CREATE_GUEST_CHANCE_HIGH_KEY_RVA, &raw mut self.guest_type_arrival_multiplier[3]);
        get_i(CHARACTERISTICS_SECTION_RVA, C_CREATE_GUEST_CHANCE_VERY_HIGH_KEY_RVA, &raw mut self.guest_type_arrival_multiplier[4]);

        get_i(CHARACTERISTICS_SECTION_RVA, C_LOAN_AVAILABLE_KEY_RVA, &raw mut self.loan_available);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HIGH_ZOO_VALUE_CHANGE_KEY_RVA, &raw mut self.high_zoo_value_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_LOW_ZOO_VALUE_CHANGE_KEY_RVA, &raw mut self.low_zoo_value_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_HIGH_ZOO_VALUE_KEY_RVA, &raw mut self.high_zoo_value);
        get_f(CHARACTERISTICS_SECTION_RVA, C_LOW_ZOO_VALUE_KEY_RVA, &raw mut self.low_zoo_value);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HIGH_SPECIES_THRESHOLD_KEY_RVA, &raw mut self.high_species_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HAPPY_DIVERSE_ANIMALS_CHANGE_KEY_RVA, &raw mut self.happy_diverse_animals_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_LOW_SPECIES_THRESHOLD_KEY_RVA, &raw mut self.low_species_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_DIVERSE_ANIMALS_CHANGE_KEY_RVA, &raw mut self.angry_diverse_animals_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HIGH_AVG_ANIMAL_HAPPY_THRESHOLD_KEY_RVA, &raw mut self.high_avg_animal_happy_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HAPPY_ANIMALS_CHANGE_KEY_RVA, &raw mut self.happy_animals_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_LOW_AVG_ANIMAL_HAPPY_THRESHOLD_KEY_RVA, &raw mut self.low_avg_animal_happy_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_ANIMALS_CHANGE_KEY_RVA, &raw mut self.angry_animals_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HIGH_AVG_GUEST_HAPPY_THRESHOLD_KEY_RVA, &raw mut self.high_avg_guest_happy_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HAPPY_GUEST_CHANGE_KEY_RVA, &raw mut self.happy_guest_change);
        get_i(CHARACTERISTICS_SECTION_RVA, C_LOW_AVG_GUEST_HAPPY_THRESHOLD_KEY_RVA, &raw mut self.low_avg_guest_happy_threshold);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ANGRY_GUEST_CHANGE_KEY_RVA, &raw mut self.angry_guest_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_ITEM_CHEAP_KEY_RVA, &raw mut self.item_cheap);
        get_f(CHARACTERISTICS_SECTION_RVA, C_ITEM_EXPENSIVE_KEY_RVA, &raw mut self.item_expensive);
        get_f(CHARACTERISTICS_SECTION_RVA, C_HIGH_ZOO_ESTHETIC_KEY_RVA, &raw mut self.high_zoo_esthetic);
        get_i(CHARACTERISTICS_SECTION_RVA, C_HIGH_ZOO_ESTHETIC_CHANGE_KEY_RVA, &raw mut self.high_zoo_esthetic_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_LOW_ZOO_ESTHETIC_KEY_RVA, &raw mut self.low_zoo_esthetic);
        get_i(CHARACTERISTICS_SECTION_RVA, C_LOW_ZOO_ESTHETIC_CHANGE_KEY_RVA, &raw mut self.low_zoo_esthetic_change);
        get_f(CHARACTERISTICS_SECTION_RVA, C_RESEARCH_COST_KEY_RVA, &raw mut self.research_cost);
        get_f(CHARACTERISTICS_SECTION_RVA, C_ADMISSION_MULTIPLE_KEY_RVA, &raw mut self.admission_income_multiplier);

        get_i(CHARACTERISTICS_SECTION_RVA, C_DONATIONS_AVAIL_KEY_RVA, &raw mut self.donation_count_bound);
        get_i(CHARACTERISTICS_SECTION_RVA, C_DONATION_LOW_KEY_RVA, &raw mut self.donation_amount_min);
        get_i(CHARACTERISTICS_SECTION_RVA, C_DONATION_HIGH_KEY_RVA, &raw mut self.donation_amount_max);
        get_i(CHARACTERISTICS_SECTION_RVA, C_DONATION_CHANCE_KEY_RVA, &raw mut self.donation_chance_percent);
        get_i(CHARACTERISTICS_SECTION_RVA, C_SPECIES_AVAILABLE_KEY_RVA, &raw mut self.species_rating_cap);
        get_i(CHARACTERISTICS_SECTION_RVA, C_MEMBERSHIP_JOIN_FACTOR_KEY_RVA, &raw mut self.membership_join_factor);
        get_i(CHARACTERISTICS_SECTION_RVA, C_MEMBERSHIP_JOIN_HAPPINESS_KEY_RVA, &raw mut self.membership_join_happiness);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ENDOWMENT_GIFT_LOW_KEY_RVA, &raw mut self.endowment_gift_low);
        get_i(CHARACTERISTICS_SECTION_RVA, C_ENDOWMENT_GIFT_HIGH_KEY_RVA, &raw mut self.endowment_gift_high);
        get_i(CHARACTERISTICS_SECTION_RVA, C_MEMBERSHIP_JOIN_CHANCE_KEY_RVA, &raw mut self.membership_join_chance);

        get_f(CHARACTERISTICS_SECTION_RVA, C_PRICING_FACTOR_KEY_RVA, &raw mut self.pricing_factor);
        get_f(CHARACTERISTICS_SECTION_RVA, C_DONATION_FACTOR_KEY_RVA, &raw mut self.donation_factor);
        get_f(CHARACTERISTICS_SECTION_RVA, C_BUILDING_USE_COST_DEFAULT_KEY_RVA, &raw mut self.building_use_cost_default);
        get_f(CHARACTERISTICS_SECTION_RVA, C_BUILDING_USE_COST_MAX_KEY_RVA, &raw mut self.building_use_cost_max);
        get_f(CHARACTERISTICS_SECTION_RVA, C_ZOO_DOO_RECYCLING_AMOUNT_KEY_RVA, &raw mut self.zoo_doo_recycling_amount);

        get_f(CHARACTERISTICS_SECTION_RVA, C_MIN_ADULT_ADMISSION_PRICE_KEY_RVA, &raw mut self.admission_price_min);
        get_f(CHARACTERISTICS_SECTION_RVA, C_MAX_ADULT_ADMISSION_PRICE_KEY_RVA, &raw mut self.admission_price_max);
        if self.admission_price_max < self.admission_price_min {
            mem::swap(&mut self.admission_price_min, &mut self.admission_price_max);
        }

        let seed_price = self.admission_price;
        self.set_adult_admission_price(seed_price);
    }

    /// Reimplementation of `ZooStatus::resetFinanceInfo` (`0x004c9f13`). Per
    /// `ZooStatus_resetFinanceInfo.c`/`.asm` (both read in full): exactly [`Self::init`]'s three
    /// zero-loops plus the same two index resets, then [`Self::calculate_sums`] (native as of Stage 5) -
    /// no config-driven tail, unlike `init`. No dedicated live test: the zero-loop logic is the same as
    /// [`Self::init`]'s (which does have one), and [`Self::calculate_sums`] has its own dedicated live
    /// test.
    pub fn reset_finance_info(&mut self) {
        self.zero_history_regions();
        self.current_month_index = 1;
        self.current_year_index = 0;
        self.calculate_sums();
    }

    /// The three history-array zero-loops shared verbatim by [`Self::init`] and
    /// [`Self::reset_finance_info`] (`ZooStatus_init.asm:105-135` / `ZooStatus_resetFinanceInfo.asm`
    /// are byte-identical over this span).
    fn zero_history_regions(&mut self) {
        self.monthly_history = [[0.0; 12]; 31];
        self.yearly_history = [[0.0; 20]; 31];
        self.flat_totals = [0.0; 31];
    }

    /// Shared write pattern for the 14 "simple accumulator" methods below (Stage 3 of the
    /// implementation plan) - each one adds `amount` to its own monthly/yearly/flat slot and applies
    /// `shared_sign * amount` to a slot shared by every other method in its group (`spend*`: `-1.0`,
    /// draining a shared expense total; `refund*`/`increase*`/`buyPeopleFood`: `+1.0`, feeding a shared
    /// income total - see `ZooStatus_spendConstruction.asm`, read in full, for the six-write shape this
    /// mirrors exactly, confirmed byte-for-byte identical across all 14 `.asm` files bar the six literal
    /// offsets).
    ///
    /// Offsets are the *raw* struct-relative byte addresses `.asm` gives per method (the plan's
    /// per-method offset table), not decomposed into [`Self::monthly_history`]/[`Self::yearly_history`]'s
    /// `[category][column]` indexing - real vanilla addresses each slot as `LEA
    /// [ECX+EAX*4+<literal offset>]` with `EAX` = [`Self::current_month_index`]/
    /// [`Self::current_year_index`], and the per-method literal offsets don't cleanly resolve to a
    /// `category*0x30`/`category*0x50` row base under any grid model checked so far (the plan's own "Open
    /// questions" section) - so this mirrors the real addressing mode directly instead of guessing a row
    /// index. [`Self::flat_totals`]' slots, by contrast, are never index-scaled (`.asm` addresses them as
    /// a bare `[ECX+<offset>]`), so `flat_own`/`flat_shared` are used as-is.
    fn accumulate(&mut self, monthly_own: u32, monthly_shared: u32, yearly_own: u32, yearly_shared: u32, flat_own: u32, flat_shared: u32, amount: f32, shared_sign: f32) {
        let base = self as *mut Self as u32;
        let month_offset = self.current_month_index as u32 * 4;
        let year_offset = self.current_year_index as u32 * 4;

        for (addr, sign) in [
            (base + monthly_own + month_offset, 1.0),
            (base + monthly_shared + month_offset, shared_sign),
            (base + yearly_own + year_offset, 1.0),
            (base + yearly_shared + year_offset, shared_sign),
            (base + flat_own, 1.0),
            (base + flat_shared, shared_sign),
        ] {
            save_to_memory(addr, get_from_memory::<f32>(addr) + sign * amount);
        }
    }

    /// `ZooStatus::spendConstruction` (`0x004d9250`). Per `ZooStatus_spendConstruction.asm` (read in
    /// full, quoted in the plan): monthly `0x1e0`(own)/`0x3f0`(shared), yearly `0x814`/`0xb84`, flat
    /// `0x10e0`/`0x110c`.
    pub fn spend_construction(&mut self, amount: f32) {
        self.accumulate(0x1e0, 0x3f0, 0x814, 0xb84, 0x10e0, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::spendBuildingUpkeep` (`0x0049bd80`). Monthly `0x690`/`0x3f0`, yearly `0xfe4`/`0xb84`,
    /// flat `0x1144`/`0x110c`.
    pub fn spend_building_upkeep(&mut self, amount: f32) {
        self.accumulate(0x690, 0x3f0, 0xfe4, 0xb84, 0x1144, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::spendGuideWages` (`0x0048bd8b`). Monthly `0x390`/`0x3f0`, yearly `0xae4`/`0xb84`, flat
    /// `0x1104`/`0x110c`.
    pub fn spend_guide_wages(&mut self, amount: f32) {
        self.accumulate(0x390, 0x3f0, 0xae4, 0xb84, 0x1104, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::buyAnimal` (`0x004e1fde`). Monthly `0x1b0`/`0x3f0`, yearly `0x7c4`/`0xb84`, flat
    /// `0x10dc`/`0x110c`.
    ///
    /// **Renamed (Stage 10)**, same bytes/offsets/logic: `generated.rs` originally mislabeled this
    /// address `SPEND_KEEPER_WAGES_0` (an OOAnalyzer artifact) - Stage 3 ported these exact bytes under
    /// that wrong name. A fresh Windows Ghidra pass plus a macOS-caller cross-check (see the plan's "The
    /// macOS-only methods" section) confirmed the real method is `buyAnimal`, byte-identical to the macOS
    /// `_ZooStatus__buyAnimal.c` export, and the regen landed the correction (`generated.rs`'s
    /// `SPEND_KEEPER_WAGES_0` constant is gone, replaced by `BUY_ANIMAL` at the same address). No other
    /// call site needed touching - Stage 3's own accumulator group has no `ZTGameMgr` call-through site
    /// for this method.
    pub fn buy_animal(&mut self, amount: f32) {
        self.accumulate(0x1b0, 0x3f0, 0x7c4, 0xb84, 0x10dc, 0x110c, amount, -1.0);
    }

    /// Reimplementation of `ZooStatus::healAnimal` (`0x0047039e`), Stage 10 - one of the five real
    /// Windows methods a fresh Ghidra pass recovered from the macOS-only corpus (see the plan's "The
    /// macOS-only methods - resolved" section). Per `ZooStatus_healAnimal.asm` (read in full): the same
    /// `spend*`-shape [`Self::accumulate`] call as Stage 3's group - own slot `+= amount` (monthly `0x180`,
    /// yearly `0x774`, flat `0x10d8`), shared slot `-= amount` (the same `spend*`-family shared triple,
    /// monthly/yearly/flat `0x3f0`/`0xb84`/`0x110c`). Called from `ZTGoalHealAnimal::complete`
    /// (`0x00470261`, still a vanilla call-through - `ZTGoal*` isn't reimplemented).
    pub fn heal_animal(&mut self, amount: f32) {
        self.accumulate(0x180, 0x3f0, 0x774, 0xb84, 0x10d8, 0x110c, amount, -1.0);
    }

    /// Reimplementation of `ZooStatus::purchaseFood` (`0x0048f7e9`), Stage 10 - see [`Self::heal_animal`]'s
    /// doc comment for the macOS-recovery context. Per `ZooStatus_purchaseFood.asm` (read in full): own
    /// slot `+= amount` (monthly `0x150`, yearly `0x724`, flat `0x10d4`), shared slot `-= amount` (the
    /// same `spend*`-family shared triple). Called from `ZTGoalPuttingFood::complete` (`0x0048f624`, a
    /// vanilla call-through).
    ///
    /// **Monthly own base is `0x150`, not `0x154`** - a real, confirmed-in-`.asm` 4-byte offset from this
    /// struct's own [`Self::monthly_history`] field (whose declared base comes from `init`'s zero-loop
    /// start). Not a bug in this port: [`Self::get_status`]'s own `.asm`-derived addressing computes the
    /// exact same `0x150` row-0 base independently (see its doc comment for the full evidence and why the
    /// aliased slot - which overlaps [`Self::current_year_index`]'s own 4 bytes at `index == 0` - is dead
    /// in real play, since [`Self::current_month_index`] defaults to `1` and this codebase has no evidence
    /// it's ever `0`). This port reproduces vanilla's raw addressing exactly, matching every other
    /// accumulator method in this file (`Self::accumulate`'s own raw byte offsets, not the named array
    /// fields), rather than "fixing" what isn't a divergence from real vanilla.
    pub fn purchase_food(&mut self, amount: f32) {
        self.accumulate(0x150, 0x3f0, 0x724, 0xb84, 0x10d4, 0x110c, amount, -1.0);
    }

    /// Reimplementation of `ZooStatus::increaseAdmissionsIncome` (`0x004f7da4`), Stage 10 - see
    /// [`Self::heal_animal`]'s doc comment for the macOS-recovery context. Per
    /// `ZooStatus_increaseAdmissionsIncome.asm` (read in full): the "income" sign pattern (own **and**
    /// shared slots both `+= amount`, like [`Self::increase_donations`]/[`Self::increase_endowment`]) -
    /// own monthly `0x240`, yearly `0x8b4`, flat `0x10e8`; shared monthly/yearly/flat
    /// `0x3f0`/`0xb84`/`0x110c`. Called from `fCreateGuest`, which stays a vanilla call-through (see
    /// [`Self::update`]'s doc comment) - this method itself has no native caller yet, only its own detour.
    pub fn increase_admissions_income(&mut self, amount: f32) {
        self.accumulate(0x240, 0x3f0, 0x8b4, 0xb84, 0x10e8, 0x110c, amount, 1.0);
    }

    /// Reimplementation of `ZooStatus::increaseAdmissions` (`0x004f7e2f`), Stage 10 - see
    /// [`Self::heal_animal`]'s doc comment for the macOS-recovery context. Per
    /// `ZooStatus_increaseAdmissions.asm` (read in full, `i32` guest count, not a pre-converted `f32`):
    /// writes `count` (int-to-float converted) into **four** slots, not [`Self::accumulate`]'s usual six -
    /// monthly `0x210`, yearly `0x864`, flat `0x10e4`, plus a **second** monthly write at `0x5d0` - no
    /// shared-slot writes at all, the one accumulator method in this file that fits neither the
    /// `spend*`/`refund*`-family sign pattern nor the plain "income" pattern. Same shape as
    /// [`Self::change_endowment_members`]'s own hand-rolled write loop, for the same reason (doesn't fit
    /// [`Self::accumulate`]'s six-slot signature).
    pub fn increase_admissions(&mut self, count: i32) {
        let amount = count as f32;
        let base = self as *mut Self as u32;
        let month_offset = self.current_month_index as u32 * 4;
        let year_offset = self.current_year_index as u32 * 4;

        for addr in [base + 0x210 + month_offset, base + 0x864 + year_offset, base + 0x10e4, base + 0x5d0 + month_offset] {
            save_to_memory(addr, get_from_memory::<f32>(addr) + amount);
        }
    }

    /// Reimplementation of `ZooStatus::getStatus` (`0x0041dd64`), Stage 10 - the last of the six real
    /// Windows methods a fresh Ghidra pass recovered from the macOS-only corpus (see the plan's "The
    /// macOS-only methods - resolved" section). A generic history-region reader the zoo-status UI graph
    /// renderers (`_updateGraphs`/`_updateAttendanceGraph`/etc., all now decompiled) pull every series
    /// through - `when` selects the region (`0` = monthly, row stride `0x30`, row-0 base `0x150`; `1` =
    /// yearly, row stride `0x50`, row-0 base `0x724`; `2` = flat, row base `0x10d4`, `index` ignored;
    /// anything else returns [`raw_globals::ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA`]'s live value
    /// unconditionally - real vanilla's own fallback, not an error path this port invented), `index == -1`
    /// defaults to the region's own rolling write cursor ([`Self::current_month_index`]/
    /// [`Self::current_year_index`]) for the monthly/yearly cases. Derived directly from
    /// `ZooStatus_getStatus.asm` (read in full - the `.c` decompile is unusable, pure pointer-arithmetic
    /// noise on a `float10*` return type, see [`GET_STATUS_FIXED`]'s own doc comment for that artifact).
    ///
    /// **A genuine 4-byte discrepancy with this struct's own [`Self::monthly_history`] field is real, not
    /// a bug in this port**: `monthly_history` declares base `0x154` (from `init`'s zero-loop start), but
    /// `getStatus`'s own monthly row-0 base is `0x150` - one slot before. [`Self::current_month_index`]
    /// defaults to `1` and this codebase has no evidence it's ever `0` in real play, so the aliased slot
    /// (`0x150`, which overlaps [`Self::current_year_index`]'s own 4 bytes when `index == 0`) is dead in
    /// practice; this port reproduces the raw `.asm` addressing exactly rather than "fixing" it, the same
    /// way [`Self::purchase_food`]'s own row-0 base does (see its doc comment - both were derived
    /// independently and agree). No such discrepancy exists for the yearly/flat regions (`0x724`/`0x10d4`
    /// match [`Self::yearly_history`]/[`Self::flat_totals`]'s own declared bases exactly).
    pub fn get_status(&self, category: i32, when: i32, index: i32) -> f32 {
        let base = self as *const Self as u32;
        let addr = match when {
            0 => {
                let idx = if index == -1 { self.current_month_index } else { index };
                (base as i32 + 0x150 + category * 0x30 + idx * 4) as u32
            }
            1 => {
                let idx = if index == -1 { self.current_year_index } else { index };
                (base as i32 + 0x724 + category * 0x50 + idx * 4) as u32
            }
            2 => (base as i32 + 0x10d4 + category * 4) as u32,
            _ => return get_from_memory(get_module_base("zoo.exe") as u32 + raw_globals::ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA),
        };
        get_from_memory(addr)
    }

    /// `ZooStatus::spendKeeperWages_1` (`0x005ad038`). Monthly `0x360`/`0x3f0`, yearly `0xa94`/`0xb84`,
    /// flat `0x1100`/`0x110c`.
    pub fn spend_keeper_wages_1(&mut self, amount: f32) {
        self.accumulate(0x360, 0x3f0, 0xa94, 0xb84, 0x1100, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::spendMaintWages` (`0x00483d34`). Monthly `0x3c0`/`0x3f0`, yearly `0xb34`/`0xb84`, flat
    /// `0x1108`/`0x110c`.
    pub fn spend_maint_wages(&mut self, amount: f32) {
        self.accumulate(0x3c0, 0x3f0, 0xb34, 0xb84, 0x1108, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::spendMarketing` (`0x0041f368`). Monthly `0x6c0`/`0x3f0`, yearly `0x1034`/`0xb84`, flat
    /// `0x1148`/`0x110c`.
    pub fn spend_marketing(&mut self, amount: f32) {
        self.accumulate(0x6c0, 0x3f0, 0x1034, 0xb84, 0x1148, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::spendResearch` (`0x0041f3f3`). Monthly `0x4e0`/`0x3f0`, yearly `0xd14`/`0xb84`, flat
    /// `0x1120`/`0x110c`.
    pub fn spend_research(&mut self, amount: f32) {
        self.accumulate(0x4e0, 0x3f0, 0xd14, 0xb84, 0x1120, 0x110c, amount, -1.0);
    }

    /// `ZooStatus::refundAnimalCost` (`0x0048d2da`). Monthly `0x330`/`0x3f0`, yearly `0xa44`/`0xb84`, flat
    /// `0x10fc`/`0x110c` - unlike the `spend*` group, the shared slot is also *added to* here (a refund
    /// reverses a prior spend, but vanilla adds rather than double-subtracting - see the plan's own note
    /// on this).
    pub fn refund_animal_cost(&mut self, amount: f32) {
        self.accumulate(0x330, 0x3f0, 0xa44, 0xb84, 0x10fc, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::refundConstruction` (`0x004f9329`). Monthly `0x300`/`0x3f0`, yearly `0x9f4`/`0xb84`,
    /// flat `0x10f8`/`0x110c`.
    pub fn refund_construction(&mut self, amount: f32) {
        self.accumulate(0x300, 0x3f0, 0x9f4, 0xb84, 0x10f8, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::increaseDonations` (`0x0042ebbe`). Monthly `0x2d0`/`0x3f0`, yearly `0x9a4`/`0xb84`,
    /// flat `0x10f4`/`0x110c`.
    pub fn increase_donations(&mut self, amount: f32) {
        self.accumulate(0x2d0, 0x3f0, 0x9a4, 0xb84, 0x10f4, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::increaseEndowment` (`0x0048442b`). Monthly `0x510`/`0x3f0`, yearly `0xd64`/`0xb84`,
    /// flat `0x1124`/`0x110c`.
    pub fn increase_endowment(&mut self, amount: f32) {
        self.accumulate(0x510, 0x3f0, 0xd64, 0xb84, 0x1124, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::increaseShowAdmission` (`0x005a9718`). Monthly `0x6f0`/`0x3f0`, yearly `0x1084`/`0xb84`,
    /// flat `0x114c`/`0x110c`.
    pub fn increase_show_admission(&mut self, amount: f32) {
        self.accumulate(0x6f0, 0x3f0, 0x1084, 0xb84, 0x114c, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::buyPeopleFood` (`0x0042df22`). Monthly `0x270`/`0x3f0`, yearly `0x904`/`0xb84`, flat
    /// `0x10ec`/`0x110c`.
    pub fn buy_people_food(&mut self, amount: f32) {
        self.accumulate(0x270, 0x3f0, 0x904, 0xb84, 0x10ec, 0x110c, amount, 1.0);
    }

    /// `ZooStatus::changeEndowmentMembers` (`0x005ad160`), the one accumulator outlier - takes an `i32`
    /// member-count delta (not a pre-converted `f32`), and its three-way `if`/`else if` shape (per
    /// `ZooStatus_changeEndowmentMembers.c`/`.asm`, both read in full) doesn't fit [`Self::accumulate`]:
    ///
    /// - An **unconditional** base write always happens, every call: monthly `0x540`, yearly `0xdb4`,
    ///   flat `0x1128`, each `+= delta as f32`.
    /// - If `delta > 0`, a *second* triple is also added: monthly `0x570`, yearly `0xe04`, flat `0x112c`.
    /// - If `delta < 0`, a *different* triple is *subtracted* by the (negative) `delta as f32` - i.e.
    ///   `field -= delta as f32`, which **adds** `abs(delta)` since `delta as f32` is itself negative:
    ///   monthly `0x5a0`, yearly `0xe54`, flat `0x1130`.
    /// - `delta == 0` touches only the unconditional base triple, matching neither `if`.
    ///
    /// The base/positive/negative offsets for each region are exactly one row apart (`0x540`/`0x570`/
    /// `0x5a0` differ by `0x30`, the monthly row stride; `0xdb4`/`0xe04`/`0xe54` differ by `0x50`, the
    /// yearly row stride; `0x1128`/`0x112c`/`0x1130` differ by `4`, adjacent flat slots) - internally
    /// consistent confirmation that these are three distinct rows/slots, not a typo in either source.
    pub fn change_endowment_members(&mut self, delta: i32) {
        let amount = delta as f32;
        let base = self as *mut Self as u32;
        let month_offset = self.current_month_index as u32 * 4;
        let year_offset = self.current_year_index as u32 * 4;

        for addr in [base + 0x540 + month_offset, base + 0xdb4 + year_offset, base + 0x1128] {
            save_to_memory(addr, get_from_memory::<f32>(addr) + amount);
        }

        if delta > 0 {
            for addr in [base + 0x570 + month_offset, base + 0xe04 + year_offset, base + 0x112c] {
                save_to_memory(addr, get_from_memory::<f32>(addr) + amount);
            }
        } else if delta < 0 {
            for addr in [base + 0x5a0 + month_offset, base + 0xe54 + year_offset, base + 0x1130] {
                save_to_memory(addr, get_from_memory::<f32>(addr) - amount);
            }
        }
    }

    /// Counts the live `std::list<ZTAnimal*>` of currently-escaped animals
    /// ([`raw_globals::ESCAPED_ANIMAL_LIST_SENTINEL_RVA`]) - shared by [`Self::rating_checks`] (and by
    /// vanilla's own `newguestChecks`, which independently performs the identical walk to gate new-guest
    /// arrivals while any animal is loose - `newguestChecks` itself stays a vanilla call-through this
    /// stage, see [`Self::update`]'s doc comment, so this helper is only exercised via `rating_checks`
    /// here). Read-only pointer-chasing over vanilla's own live container - no allocation/freeing of our
    /// own, so none of `CLAUDE.md`'s cross-allocator hazards apply (same shape as `calculateSums`' tile
    /// walk).
    fn count_escaped_animals() -> i32 {
        // `DAT_00638fb0` is a global *pointer variable* whose stored value is the sentinel's real
        // address (confirmed by `ratingChecks.asm`: `MOV ECX, dword ptr DAT_00638fb0` loads the global's
        // *value*, not `LEA ECX, DAT_00638fb0` - a genuine extra level of indirection versus this
        // helper's first draft, which wrongly treated the global's own address as the sentinel and could
        // walk garbage pointers forever).
        let global_addr = get_module_base("zoo.exe") as u32 + raw_globals::ESCAPED_ANIMAL_LIST_SENTINEL_RVA;
        let sentinel: u32 = get_from_memory(global_addr);
        let mut node: u32 = get_from_memory(sentinel);
        let mut count = 0;
        while node != sentinel {
            node = get_from_memory(node);
            count += 1;
        }
        count
    }

    /// Reimplementation of `ZooStatus::animalEscaped` (`0x0050cde4`), Stage 4. Per
    /// `ZooStatus_animalEscaped.c`/`.asm` (both read in full, `fastcall`/single-`this`-register, no other
    /// params): stamps the last-escape timestamp with the live game date, read through the already-ported
    /// [`crate::ztgamemgr::ZTGameMgr::get_date`] on the real `GLOBAL_ZTGameMgr` singleton (matching
    /// vanilla's own real body, which reads `GLOBAL_ZTGameMgr` directly rather than deriving it from
    /// `this` - `ZooStatus` has no back-pointer to its enclosing `ZTGameMgr`, and vanilla doesn't need one
    /// here either).
    pub fn animal_escaped(&mut self) {
        let date = globals().ztgamemgr().get_date();
        self.last_animal_escape_timestamp_low = date as u32;
        self.last_animal_escape_timestamp_high = (date >> 32) as u32;
    }

    /// Reimplementation of `ZooStatus::admissionMessage` (`0x00429d68`), Stage 4. Per
    /// `ZooStatus_admissionMessage.c`/`.asm` (both read in full): a one-line guard in front of a call to
    /// native [`f_zoo_message`] (`GLOBAL_BFUIMgr` is now resolved - see
    /// [`raw_globals::GLOBAL_BFUIMGR_RVA`] - so this no longer needs to stay a call-through to real
    /// vanilla `fZooMessage`).
    pub fn admission_message(&self, message_id: *const u32, param: u32) {
        if self.guest_tile_count > 10 {
            f_zoo_message(message_id, param, 0, 0);
        }
    }

    /// Reimplementation of `ZooStatus::newguestChecks` (per `generated.rs`'s `NEWGUEST_CHECKS` entry),
    /// Stage 4. Per `ZooStatus_newguestChecks.c`/`.asm`, re-read after a Ghidra re-pass cleaned up a
    /// genuinely-confusing earlier export (the prior `.c` had an impossible nested condition -
    /// `if (0x3c < iVar6) { ... }` inside a branch already guarded by `iVar6 < 0x3c` - that made the real
    /// low/mid attendance-tier dispatch impossible to confidently re-derive; the current export's own
    /// `WARNING: Removing unreachable block`/`Possible PIC construction... Changing call to branch` notes
    /// confirm Ghidra corrected a real analysis mistake, not just re-formatted the same logic). This
    /// resolves the ambiguity this plan previously flagged and lets `newguestChecks` join the fully-native
    /// tier alongside `message_checks`/`rating_checks` - only [`F_CHANCE`]/[`F_CREATE_GUEST`] stay
    /// call-throughs, for the reasons given in [`Self::update`]'s doc comment (shared RNG stream /
    /// cross-allocator freelist hazard). `fZooMessage` itself (reached via [`Self::admission_message`])
    /// is now native too - see [`f_zoo_message`].
    ///
    /// 1. Resets [`Self::newguest_check_elapsed`] (this check's own interval accumulator, matching
    ///    [`Self::message_checks`]/[`Self::rating_checks`]'s analogous resets).
    /// 2. Counts occupied habitats (`ZTHabitat::getNumAnimals(habitat, false) != 0`, real vanilla
    ///    call-through per-habitat over the already-reimplemented `ZTHabitatMgr::exhibit_array` - a
    ///    read-only walk of a container we already own the shape of, not a fresh dependency) and
    ///    currently-escaped animals ([`Self::count_escaped_animals`], shared with `rating_checks`).
    ///    Returns immediately if any animal is loose, or if no habitat has any animals at all - no new
    ///    guests arrive either way.
    /// 3. Buckets [`Self::admission_price`] into one of five tiers ([`Self::field_0x48`], `0..=4`) against
    ///    four boundary constants - unchanged from this plan's earlier analysis, already correctly derived
    ///    from the always-clean part of the old export.
    /// 4. Computes an attendance factor from [`Self::zoo_rating_current`] plus the live
    ///    `ZTMarketingMgr`'s current funding level's `benefit` (doubled if
    ///    [`raw_globals::DOUBLE_MARKETING_BENEFIT_FLAG_RVA`] is set) plus a flat `+30` if
    ///    [`raw_globals::FLAT_ATTENDANCE_BONUS_FLAG_RVA`] is set.
    /// 5. Buckets that attendance factor into four bands (`>=0x51`, `0x3c..=0x50`, `0x1e..=0x3b`,
    ///    `<=0x1d`) and, per `(band, price tier)`, either returns with no guest or selects one of
    ///    [`Self::guest_type_arrival_multiplier`]'s five entries as the [`F_CHANCE`] roll parameter - see
    ///    the `(band, tier) -> (multiplier index, shows admissionMessage)` table below, hand-verified
    ///    against every branch/jump target in `ZooStatus_newguestChecks.asm` (the price-tier/attendance
    ///    arithmetic and the `GLOBAL_ZTMarketingMgr`/`DAT_00638fb0` reads were already confirmed against
    ///    the `.asm` in this plan's Stage-0 pass; this pass re-confirmed every dispatch target/shared
    ///    label the cleaned-up `.c` introduces):
    ///
    ///    | Price tier | attendance `<=0x1d` | `0x1e..=0x3b` | `0x3c..=0x50` | attendance `>=0x51` |
    ///    |---|---|---|---|---|
    ///    | 0 | no guest | no guest | no guest | `mult[0]` |
    ///    | 1 | no guest | `mult[0]` | `mult[1]` | `mult[2]` |
    ///    | 2 | `mult[0]` | `mult[2]` | `mult[2]` | `mult[3]` |
    ///    | 3 | `mult[1]` | `mult[2]` | `mult[3]` | `mult[3]`, **shows message** |
    ///    | 4 | `mult[1]` | `mult[3]`, **shows message** | `mult[4]`, **shows message** | `mult[4]`, **shows message** |
    ///
    /// 6. Rolls [`F_CHANCE`] with the selected multiplier; on success, calls real vanilla
    ///    [`F_CREATE_GUEST`] (a genuine cross-allocator/freelist hazard - see [`Self::update`]'s doc
    ///    comment for why that one specific real dependency stays a call-through even though the
    ///    surrounding dispatch logic is now native), then - only for the table's marked cells -
    ///    [`Self::admission_message`] with string id `0x2721`.
    ///
    /// The tier-bucketing and band/tier dispatch are pulled out into the two pure functions below
    /// ([`Self::price_tier`]/[`Self::newguest_dispatch`]) purely so they're unit-testable without live
    /// game state - `newguest_checks` itself can't get a `#[cfg(test)]` unit test (it reads
    /// `GLOBAL_ZTHabitatMgr`/`GLOBAL_ZTMarketingMgr`/the live escaped-animal list and calls real vanilla
    /// [`F_CHANCE`]/[`F_CREATE_GUEST`]), but the actual decision logic this stage's own re-derivation
    /// pass exists to get right can be, and is, covered directly.
    pub fn newguest_checks(&mut self) {
        self.newguest_check_elapsed = 0;

        let habitat_mgr = globals().zthabitatmgr();
        let mut occupied_habitats = 0i32;
        for i in 0..habitat_mgr.exhibit_array().len() {
            let habitat_ptr = habitat_mgr.exhibit_array().get_ptr(i);
            if unsafe { GET_NUM_ANIMALS.original()(habitat_ptr as *const u32, false) } != 0 {
                occupied_habitats += 1;
            }
        }

        if Self::count_escaped_animals() != 0 {
            return;
        }
        if occupied_habitats == 0 {
            return;
        }

        let base = get_module_base("zoo.exe") as u32;
        let price = self.admission_price;
        let boundary_0: f32 = get_from_memory(base + raw_globals::PRICE_TIER_BOUNDARY_0_RVA);
        let boundary_1: f32 = get_from_memory(base + raw_globals::PRICE_TIER_BOUNDARY_1_RVA);
        let boundary_2: f32 = get_from_memory(base + raw_globals::PRICE_TIER_BOUNDARY_2_RVA);
        let boundary_3: f32 = get_from_memory(base + raw_globals::PRICE_TIER_BOUNDARY_3_RVA);

        self.field_0x48 = Self::price_tier(price, [boundary_0, boundary_1, boundary_2, boundary_3]);

        let mut attendance = self.zoo_rating_current;
        if let Some(marketing) = globals().ztmarketingmgr().marketing()
            && let Some(level) = marketing.funding_levels().get(marketing.current_funding_level() as usize)
        {
            let benefit = level.benefit();
            attendance += benefit;
            let double_benefit: u8 = get_from_memory(base + raw_globals::DOUBLE_MARKETING_BENEFIT_FLAG_RVA);
            if double_benefit != 0 {
                attendance += benefit;
            }
        }
        let flat_bonus: u8 = get_from_memory(base + raw_globals::FLAT_ATTENDANCE_BONUS_FLAG_RVA);
        if flat_bonus != 0 {
            attendance += 30;
        }

        let Some((chance_param, show_message)) = Self::newguest_dispatch(attendance, self.field_0x48, self.guest_type_arrival_multiplier) else {
            return;
        };

        let chance = unsafe { F_CHANCE.original()(chance_param) };
        // Only the low byte is defined when `chance_param == 0` (real vanilla's own `fChance` leaves the
        // upper 3 bytes as leftover EAX garbage in that case) - `ZooStatus_newguestChecks.asm`'s own real
        // caller tests `TEST %AL, %AL`, never the full `EAX`. See [`Self::update`]'s own doc comment for
        // the fuller evidence trail (a live crash from the same untruncated-comparison bug elsewhere).
        if chance & 0xff == 0 {
            return;
        }
        unsafe { F_CREATE_GUEST.original()(self as *mut Self as *const u32) };
        if show_message {
            self.admission_message(0x2721 as *const u32, 0);
        }
    }

    /// Pure admission-price-tier bucketing for [`Self::newguest_checks`] - the `< , < , <=(<=), else`
    /// chain confirmed against `ZooStatus_newguestChecks.asm`'s `FLD`/`FCOMP`/`FNSTSW`/`TEST AH,0x41`
    /// sequence over `[ESI+0x1150]` (i.e. [`Self::admission_price`]). Pulled out standalone so it's
    /// unit-testable without live game state.
    fn price_tier(price: f32, boundaries: [f32; 4]) -> i32 {
        let [boundary_0, boundary_1, boundary_2, boundary_3] = boundaries;
        if boundary_0 < price {
            0
        } else if boundary_1 < price {
            1
        } else if price <= boundary_2 {
            if price <= boundary_3 {
                4
            } else {
                3
            }
        } else {
            2
        }
    }

    /// Pure `(attendance band, price tier) -> (F_CHANCE multiplier, shows admissionMessage)` dispatch for
    /// [`Self::newguest_checks`] - the table documented on that method's own doc comment, hand-verified
    /// against every branch/jump target in `ZooStatus_newguestChecks.c`/`.asm`. `None` means "no new guest
    /// this tick." Pulled out standalone so it's unit-testable without live game state.
    fn newguest_dispatch(attendance: i32, price_tier: i32, multipliers: [i32; 5]) -> Option<(i32, bool)> {
        let m = multipliers;
        if attendance >= 0x51 {
            match price_tier {
                1 => Some((m[2], false)),
                2 => Some((m[3], false)),
                3 => Some((m[3], true)),
                4 => Some((m[4], true)),
                _ => Some((m[0], false)),
            }
        } else if attendance > 0x3b {
            match price_tier {
                1 => Some((m[1], false)),
                2 => Some((m[2], false)),
                3 => Some((m[3], false)),
                4 => Some((m[4], true)),
                _ => None,
            }
        } else if attendance > 0x1d {
            match price_tier {
                1 => Some((m[0], false)),
                2 => Some((m[2], false)),
                3 => Some((m[2], false)),
                4 => Some((m[3], true)),
                _ => None,
            }
        } else {
            match price_tier {
                2 => Some((m[0], false)),
                3 => Some((m[1], false)),
                4 => Some((m[1], false)),
                _ => None,
            }
        }
    }

    /// Reimplementation of `ZooStatus::messageChecks` (`0x0041ffed`-family, per `generated.rs`'s
    /// `MESSAGE_CHECKS` entry), Stage 4. Per `ZooStatus_messageChecks.c`/`.asm` (both read in full, and
    /// cross-checked instruction-by-instruction - no `this[N]` ambiguity anywhere in this method, every
    /// field is a direct, already-named offset): resets [`Self::message_check_elapsed`] (this check's own "ticks
    /// since last run" accumulator, mirroring [`Self::rating_checks`]/vanilla's `financeChecks`'s
    /// analogous resets), then eight `count * threshold < count` frequency checks (mixed
    /// [`Self::num_animals`]/[`Self::guest_tile_count`]-scaled, matching the real `.asm`'s
    /// `FILD`/`FMUL`/`FCOMPP` int-to-float promotion before each compare) gated behind
    /// `guest_tile_count > 10` for all but the first, plus two direct
    /// [`Self::guest_rating_metric`] threshold checks, then an unconditional three-way budget check
    /// against the live `ZTGameMgr::cash` (`round()` implemented as `as i32`, matching the real body's
    /// `FISTP`-with-truncating-control-word idiom exactly - Rust's `f32 as i32` cast already truncates
    /// toward zero). Every message dispatch goes through native [`f_zoo_message`] (see
    /// [`Self::admission_message`]'s doc comment for why this no longer needs to call through to real
    /// vanilla `fZooMessage`).
    pub fn message_checks(&mut self) {
        self.message_check_elapsed = 0;

        let zoo_message = |id: u32, priority: u32| f_zoo_message(id as *const u32, priority, 0, 0);

        if (self.num_animals as f32) * self.message_threshold_0x74 < self.animal_condition_counter_1 as f32 {
            zoo_message(0x2719, 2);
        }

        if self.guest_tile_count > 10 {
            if (self.num_animals as f32) * self.message_threshold_0x70 < self.animal_condition_counter_1 as f32 {
                zoo_message(0x2718, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0x7c < self.num_hungry_guests as f32 {
                zoo_message(0x271b, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0x84 < self.num_thirst_guests as f32 {
                zoo_message(0x271c, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0x8c < self.num_guests_restroom_need as f32 {
                zoo_message(0x271d, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0x94 < self.guest_condition_counter_1 as f32 {
                zoo_message(0x271e, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0xa0 < self.num_tired_guests as f32 {
                zoo_message(0x271f, 2);
            }
            if (self.guest_tile_count as f32) * self.message_threshold_0xa8 < self.guest_condition_counter_2 as f32 {
                zoo_message(0x2720, 2);
            }
            if self.high_avg_guest_happy_threshold <= self.guest_rating_metric {
                zoo_message(0x2725, 1);
            }
            if self.guest_rating_metric <= self.low_avg_guest_happy_threshold {
                zoo_message(0x2726, 2);
            }
        }

        let cash = globals().ztgamemgr().cash() as i32;
        if cash > 0 {
            if cash < 1000 {
                zoo_message(0x2722, 2);
            }
        } else {
            zoo_message(0x2723, 2);
            if cash < 0 {
                zoo_message(0x2724, 2);
            }
        }
    }

    /// Reimplementation of `ZooStatus::ratingChecks` (per `generated.rs`'s `RATING_CHECKS` entry), Stage
    /// 4. Per `ZooStatus_ratingChecks.c`/`.asm` (both read in full - the `.c`'s `this[N]` indexing was
    /// cross-checked against the `.asm`'s literal displacements throughout, using the confirmed `0x154`
    /// stride; see this method's inline comments for the resolved offset of each `this[N].field_0xM`):
    ///
    /// 1. Resets [`Self::rating_check_elapsed`] (this check's own interval accumulator), then calls
    ///    [`Self::calculate_sums`] (native as of Stage 5) to refresh the live counters this formula
    ///    reads.
    /// 2. Computes [`Self::zoo_rating_current`] from an animal-health fraction, a species-count bonus
    ///    (capped at [`Self::species_rating_cap`]), the animal/guest rating metrics, a clamped
    ///    `field_0x4c`-derived bonus, and a research-completion-vs-map-coverage penalty/bonus term - see
    ///    the inline comments for the exact formula, which was hand-verified against the `.asm`'s
    ///    `IDIV`/`IMUL`-by-magic-constant integer-division sequences (Rust's plain `/` on the same
    ///    integer types produces identical results; the `.asm`'s reciprocal-multiply tricks are just
    ///    compiler codegen for the same division, not extra logic).
    /// 3. Counts currently-escaped animals ([`Self::count_escaped_animals`]); if any, calls
    ///    [`Self::animal_escaped`] to refresh the escape timestamp (this does **not** touch the rating
    ///    computed in step 2 - only the decay term in step 4 depends on the timestamp, and only on
    ///    *subsequent* calls, since `hoursAgo` is computed after this step using whatever timestamp is
    ///    current at that point).
    /// 4. Applies an escape-recency decay penalty (via the real, already-ported
    ///    [`crate::ztgamemgr::ZTGameMgr::hours_ago`]) and clamps the final rating into `0..=100` - per the
    ///    `.asm`, this final clamp is a genuine integer `clamp`, not a float round-trip: the decompile's
    ///    "float" literals here (`4.2039e-41`, `1.4013e-43`) are decompiler type-unification artifacts
    ///    rendering raw reused-stack-slot integer bit patterns (`30000`/`100`) as if they were float
    ///    literals - confirmed by the `.asm` only ever using plain `MOV`/`CMP` (never `FLD`) against these
    ///    slots at the points that matter, matching this method's own `research_completion_percent`
    ///    type-correction reasoning above.
    /// 5. Writes the final clamped rating into its monthly/yearly/flat history slots (`0x450`/`0xc24`/
    ///    `0x1114` - matching the plan's own pre-recorded offsets for this write).
    pub fn rating_checks(&mut self) {
        self.rating_check_elapsed = 0;
        self.calculate_sums();

        let mut rating: i32 = 0;
        if self.num_animals > 0 {
            rating = ((self.num_animals as i32 - self.animal_condition_counter_1 as i32) * 15) / self.num_animals as i32;
        }
        if self.num_species > 0 {
            let capped = (self.num_species as i32).min(self.species_rating_cap);
            rating += (capped * 10) / self.species_rating_cap;
        }

        rating += scaled_rating_metric(self.animal_rating_metric, 25);
        rating += scaled_rating_metric(self.guest_rating_metric, 25);

        let clamped_config = self.config_budget_0x0_4c_clamped();
        rating += (clamped_config * 10) / 30000;

        let base = get_module_base("zoo.exe") as u32;
        let fraction_scale: f32 = get_from_memory(base + raw_globals::ATTENDANCE_FRACTION_SCALE_RVA);
        let fraction_floor: f32 = get_from_memory(base + raw_globals::ATTENDANCE_FRACTION_FLOOR_RVA);
        let vs_research_threshold: f32 = get_from_memory(base + raw_globals::ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA);
        let decay_scale: f32 = get_from_memory(base + raw_globals::RATING_DECAY_SCALE_RVA);

        let fraction_scaled = self.non_blank_tile_fraction * fraction_scale;
        let attendance_cap = if fraction_scaled <= fraction_floor { fraction_scaled } else { 100.0 };
        let research_decay = (self.research_completion_percent * 5) / 100;
        let final_attendance = if attendance_cap < vs_research_threshold { research_decay as f32 } else { attendance_cap };
        let rounded_penalty = (final_attendance * decay_scale) as i32;
        rating = rating + research_decay - rounded_penalty;

        if Self::count_escaped_animals() != 0 {
            self.animal_escaped();
        }

        let escape_timestamp = ((self.last_animal_escape_timestamp_high as u64) << 32) | self.last_animal_escape_timestamp_low as u64;
        let hours_since_escape = globals().ztgamemgr().hours_ago(escape_timestamp);
        let escape_decay_baseline: i32 = get_from_memory(base + raw_globals::ESCAPE_DECAY_BASELINE_RVA);
        let escape_decay_per_day: i32 = get_from_memory(base + raw_globals::ESCAPE_DECAY_PER_DAY_RVA);
        let escape_decay = escape_decay_baseline - escape_decay_per_day * ((hours_since_escape as u32 as u64 / 24) as i32);
        let escape_decay_clamped = if escape_decay < 1 { 0 } else { escape_decay };
        rating -= escape_decay_clamped;

        self.zoo_rating_current = rating.clamp(0, 100);

        let month_offset = self.current_month_index as u32 * 4;
        let year_offset = self.current_year_index as u32 * 4;
        let base_ptr = self as *mut Self as u32;
        save_to_memory(base_ptr + 0x450 + month_offset, self.zoo_rating_current as f32);
        save_to_memory(base_ptr + 0xc24 + year_offset, self.zoo_rating_current as f32);
        save_to_memory(base_ptr + 0x1114, self.zoo_rating_current as f32);
    }

    /// `field_0x4c` clamped into `0..=30000` - `ratingChecks.asm`'s own pointer-swap clamp idiom (compare
    /// against `30000`, then against `0`), pulled out as its own function purely for readability at the
    /// call site above.
    fn config_budget_0x0_4c_clamped(&self) -> i32 {
        self.field_0x4c.clamp(0, 30000)
    }

    /// Reimplementation of `ZooStatus::fGrantDonation` (`0x00613e4a`), Stage 4. Per
    /// `ZooStatus_fGrantDonation.c`/`.asm` (both read in full - the `.c`'s own header warns "Type
    /// propagation algorithm not settling", and ground truth for the `+0x120`/`+0x124` pair came from the
    /// `.asm` directly, not the `.c`'s mistyped `local_628`/`local_62c` locals - see
    /// [`Self::donation_count_this_period`]'s own doc comment for the field-type correction this pass
    /// applied).
    ///
    /// 1. Increments [`Self::donation_count_this_period`] by `1.0` each call.
    /// 2. If the incremented counter now exactly equals `donation_count_bound + 1` (i.e. this call is the
    ///    one that pushed the counter *past* the period's bound): shows a "no more donations this period"
    ///    message (string id `0x3a9b`) and returns - a one-time edge trigger, not a per-call gate.
    /// 3. Otherwise, if the counter is still `<= donation_count_bound`: rolls a donation amount uniformly
    ///    in `[donation_amount_min, donation_amount_max)` (real vanilla `msvc_std::RAND` - the Ghidra
    ///    regen that resolved `FUN_0040f103` confirms it's exactly the CRT `rand()`
    ///    `ZooStatus::fChance`'s own decompile separately inlines), grants it via **the live
    ///    `GLOBAL_ZTGameMgr`'s own embedded `ZooStatus`, not `self`** - matching vanilla's own `LEA
    ///    ECX,[ESI+0x10]` off `GLOBAL_ZTGameMgr` rather than reusing `this`, a real quirk preserved
    ///    verbatim (only observable when this method is ever called against a standalone, non-global test
    ///    instance) - then shows the formatted "donated $X" message (string id `0x3a9a`, real vanilla
    ///    [`format_money_text`] for the localized amount text).
    /// 4. Any other count (already past the bound this period) is a silent no-op, matching vanilla
    ///    exactly.
    ///
    /// Both messages build their `std::string` via [`VanillaString`] (real vanilla `BASIC_STRING_2`/
    /// `BASIC_STRING_0` constructor/destructor) rather than replicating vanilla's own
    /// `tree::cls_0x4012a6`/`compare` two-call construction sequence for the id-`5` branch - same net
    /// result (a populated, vanilla-allocator-owned string), fewer unresolved intermediate symbols. The
    /// `%s` substitution vanilla does via `BFLog::FormatLogMessage` for the id-`4` branch (whose own real
    /// arg count is ambiguous - the `.c`/its own `.meta` say 2 args, but the `.asm`'s push/cleanup math
    /// implies 3 once combined with the preceding `getMoneyText` call) is done in Rust instead via
    /// [`str::replace`], sidestepping that ambiguity entirely - same displayed text either way, since the
    /// only thing being substituted is [`format_money_text`]'s own already-vanilla-formatted output.
    ///
    /// The three-way branch above is dispatched through [`donation_outcome`], a pure function pulled out
    /// purely so its branch selection has direct unit-test coverage (see its own doc comment) - **no live
    /// comparison test exercises the message-firing branches (`BoundJustCrossed`/`Grant`) at all**: both
    /// call through to real vanilla `BFUIMgr::displayMessage`, and an earlier `message_checks` live test
    /// that accidentally reached that same call live hung the whole reimplementation-test battery (see
    /// `ZOOSTATUS_CHECKS`'s own doc comment) - the same "keep a real UI-dialog condition false" discipline
    /// applies here. `ZOOSTATUS_F_GRANT_DONATION_NO_OP` (`reimplementation_tests/mod.rs`) does cover the
    /// silent `AlreadyPastBound` branch live, since it never reaches a message call.
    pub fn f_grant_donation(&mut self) {
        self.donation_count_this_period += 1.0;
        let bound = self.donation_count_bound;

        match donation_outcome(self.donation_count_this_period, bound) {
            DonationOutcome::BoundJustCrossed => {
                let message = load_localized_string(0x3a9b);
                let vanilla_str = VanillaString::new(&message);
                display_message_string(vanilla_str.as_ptr(), 5);
            }
            DonationOutcome::AlreadyPastBound => {}
            DonationOutcome::Grant => {
                let range = self.donation_amount_max - self.donation_amount_min;
                let amount = if range > 0 {
                    self.donation_amount_min + (unsafe { RAND.original()() } as i32 % range)
                } else {
                    self.donation_amount_min
                };
                let amount_f = amount as f32;

                let ztgamemgr_ptr = globals().ztgamemgr_ptr();
                let global_zoostatus_ptr = (ztgamemgr_ptr as u32 + 0x10) as *mut ZooStatus;
                unsafe { (*global_zoostatus_ptr).increase_donations(amount_f) };
                unsafe { (*ztgamemgr_ptr).add_cash(amount_f) };

                let template = load_localized_string(0x3a9a);
                let money_text = format_money_text(amount as u32);
                let message = template.replace("%s", &money_text);
                let vanilla_str = VanillaString::new(&message);
                display_message_string(vanilla_str.as_ptr(), 4);
            }
        }
    }

    /// Reimplementation of `ZooStatus::setAdultAdmissionPrice` (per `generated.rs`'s
    /// `SET_ADULT_ADMISSION_PRICE` entry), Stage 5. Per `ZooStatus_setAdultAdmissionPrice.c`/`.asm`
    /// (both read in full): clamps `price` into
    /// `[`[`Self::admission_price_min`]`, `[`Self::admission_price_max`]`]` and stores the result into
    /// [`Self::admission_price`]. `.asm`'s own two-`FCOMP` sequence is exactly "clamp `price` between
    /// the two bounds" - written here as the explicit min-then-max form (not Rust's `.clamp()`, which
    /// panics if `min > max`) since this method's own callers can't guarantee that invariant the way
    /// `.clamp()`'s contract requires.
    pub fn set_adult_admission_price(&mut self, price: f32) {
        let clamped_high = if price < self.admission_price_max { price } else { self.admission_price_max };
        self.admission_price = if clamped_high <= self.admission_price_min { self.admission_price_min } else { clamped_high };
    }

    /// Reimplementation of `ZooStatus::showPrices` (per `generated.rs`'s `SHOW_PRICES` entry), Stage 5.
    /// Per `ZooStatus_showPrices.c`/`.asm` (both read in full): a pure UI-refresh method - reads
    /// [`Self::admission_price`]/`_min`/`_max` but writes nothing back into `self`, so (unlike every
    /// other method in this file) there is no struct state for a live comparison test to diff against;
    /// see `ZOOSTATUS_SHOW_PRICES_SMOKE`'s own doc comment for how this is verified instead.
    ///
    /// 1. If the `0x105e` UI element exists and its own state flag bit `9` (raw `+0x7c` read - `ztui.rs`'s
    ///    own `UIElement` struct doesn't model this particular bit) is clear, sets its money text to the
    ///    current admission price via real vanilla `bfinternat::setMoneyText`.
    /// 2. Unconditionally sets the `0x1061` element's money text to the same price.
    /// 3. Sets the `0x1062` element's money text to
    ///    `admission_price * `[`raw_globals::CHILD_ADMISSION_PRICE_SCALE_RVA`] (the "child admission
    ///    price" display).
    /// 4. Enables the `0x1063` element if `admission_price < `[`Self::admission_price_max`], disables it
    ///    otherwise.
    /// 5. Disables the `0x105f` element if `admission_price <= `[`Self::admission_price_min`], enables
    ///    it otherwise.
    ///
    /// **Vanilla quirk, deliberately not reproduced**: `.asm` shows both the `0x1063`/`0x105f`
    /// enable/disable dispatches are unconditional vtable calls through `[element_vtable+0x68]`/`[+0x6c]`
    /// even on the path where `BFUIMgr::getElement` returned null (a genuine null-pointer vtable
    /// dispatch in real vanilla code) - unreachable in practice (both elements are always registered by
    /// the time `showPrices` can run live), so this port skips the call entirely when the element is
    /// null rather than replicate a crash.
    pub fn show_prices(&self) {
        let base = get_module_base("zoo.exe") as u32;
        let bfuimgr_ptr = base + raw_globals::GLOBAL_BFUIMGR_RVA;

        let element_105e = unsafe { GET_ELEMENT_0.original()(bfuimgr_ptr as *const u32, 0x105e) };
        if !element_105e.is_null() {
            let flags: u32 = get_from_memory(element_105e as u32 + 0x7c);
            if (flags >> 9) & 1 == 0 {
                unsafe { SET_MONEY_TEXT_0.original()(0x105e, self.admission_price, 0) };
            }
        }

        unsafe { SET_MONEY_TEXT_0.original()(0x1061, self.admission_price, 0) };

        let child_scale: f32 = get_from_memory(base + raw_globals::CHILD_ADMISSION_PRICE_SCALE_RVA);
        unsafe { SET_MONEY_TEXT_0.original()(0x1062, self.admission_price * child_scale, 0) };

        let element_1063 = unsafe { GET_ELEMENT_0.original()(bfuimgr_ptr as *const u32, 0x1063) };
        if !element_1063.is_null() {
            if self.admission_price < self.admission_price_max {
                unsafe { ENABLE.original()(element_1063) };
            } else {
                unsafe { DISABLE.original()(element_1063) };
            }
        }

        let element_105f = unsafe { GET_ELEMENT_0.original()(bfuimgr_ptr as *const u32, 0x105f) };
        if !element_105f.is_null() {
            if self.admission_price <= self.admission_price_min {
                unsafe { DISABLE.original()(element_105f) };
            } else {
                unsafe { ENABLE.original()(element_105f) };
            }
        }
    }

    /// Reimplementation of `ZooStatus::calculateSums` (per `generated.rs`'s `CALCULATE_SUMS` entry),
    /// Stage 5 - the most cross-class-dependent method in this file, per the plan's own staging
    /// rationale. Per `ZooStatus_calculateSums.c`/`.asm` (both read in full, ground truth taken from the
    /// `.asm` throughout since the `.c`'s `this[N].field_0xM` pseudo-array notation needed the confirmed
    /// `0x154` stride to resolve unambiguously - every monthly/yearly/flat offset below is independently
    /// cross-checked against the plan's own pre-recorded offset table).
    ///
    /// **This pass overturned this struct's own `escaped_animal_tile_count`/`animal_condition_counter_2`/
    /// `_3` naming** - see [`Self::guest_tile_count`]/[`Self::guest_condition_counter_1`]/
    /// [`Self::guest_condition_counter_2`]'s doc comments for the full evidence trail; this method's own
    /// walk below is exactly the evidence that surfaced it.
    ///
    /// 1. Zeroes [`Self::num_animals`]/[`Self::animal_condition_counter_1`]/[`Self::num_tired_guests`]/
    ///    [`Self::num_hungry_guests`]/[`Self::num_thirst_guests`]/[`Self::num_guests_restroom_need`]/
    ///    [`Self::guest_condition_counter_1`]/[`Self::guest_condition_counter_2`]/
    ///    [`Self::guest_tile_count`], plus the current month's [`Self::monthly_history`] slot at row
    ///    offset `0x600` (a guest-tile-count history row, its own row - not shared with any of the
    ///    "simple accumulator" methods).
    /// 2. Seeds [`Self::field_0x4c`] from `round(ZTGameMgr::cash())` (real, already-ported
    ///    [`crate::ztgamemgr::ZTGameMgr::cash`] - confirmed `ZTGameMgr+0xc`; `round()` implemented as
    ///    `as i32`, matching every other `FISTP`-with-truncating-control-word site in this file).
    /// 3. Zeroes [`Self::field_0x58`]/[`Self::non_blank_tile_fraction`].
    /// 4. Walks every live entity in `GLOBAL_ZTWorldMgr`'s entity array
    ///    ([`crate::ztworldmgr::ZTWorldMgr::entity_array_start`]/`entity_array_end`, already-modeled -
    ///    exactly `BFWorldMgr`'s own `+0x80`/`+0x84` vector this method's `.asm` walks), skipping null
    ///    slots (the `.asm`'s own null check runs *after* a defensive, and for a real null entry wild,
    ///    `+0x128` read; every real entry in this array is non-null in practice, so this port checks
    ///    first instead of replicating that instruction ordering). For each entity, dispatches its
    ///    type's vtable slot `0x1c` (via [`entity_type_matches`]) against three type-check arguments in
    ///    turn:
    ///    - [`raw_globals::GUEST_TYPE_CHECK_RVA`] ("is this a guest"): increments
    ///      [`Self::guest_tile_count`], adds `1.0` to the current month's `+0x600` history slot, then for
    ///      each of the guest's own hunger/thirst/restroom-need/tiredness fields (raw offsets `+0x2b0`/
    ///      `+0x2b8`/`+0x2c8`/`+0x2c0` - no `ZTGuest` struct modeled in this codebase yet, so read raw)
    ///      exceeding [`raw_globals::GUEST_NEED_THRESHOLD_RVA`], increments the matching
    ///      [`Self::num_hungry_guests`]/[`Self::num_thirst_guests`]/[`Self::num_guests_restroom_need`]/
    ///      [`Self::num_tired_guests`]; also increments [`Self::guest_condition_counter_1`] if the
    ///      guest's own `+0x33c` byte flag is set, and [`Self::guest_condition_counter_2`] if the
    ///      pointer at the guest's own `+0x26c` points to a struct whose own `+0x10` field is non-zero
    ///      (an unguarded double dereference in vanilla, matching real guest entities' always-valid
    ///      sub-object); finally accumulates the guest's own `+0x2a8` "score" field into a local sum
    ///      later divided by [`Self::guest_tile_count`] for [`Self::guest_rating_metric`]'s own average.
    ///    - [`raw_globals::ANIMAL_TYPE_CHECK_RVA`] ("is this an animal"): increments
    ///      [`Self::num_animals`], accumulates the animal's own `+0x2a8` field (same offset as the
    ///      guest score field above - a shared base-class layout) into a local sum for
    ///      [`Self::animal_rating_metric`]'s own average, increments [`Self::animal_condition_counter_1`] if the
    ///      animal's own `+0x3a7` byte flag is set, then calls the animal's own *type* object's vtable
    ///      slot `0xbc` (unresolved semantic role - `private/docs/vtables/ZTAnimalType.md`/
    ///      `ZTUnitType.md` both list this slot `unknown`) and adds `round()` of its float return into
    ///      [`Self::field_0x4c`].
    ///    - [`raw_globals::BUILDING_TYPE_CHECK_RVA`] ("is this a building" - confirmed via
    ///      `ZTBuildingType.md`'s `+0x1c` override/`+0xa4` = `getPurchaseCost`, see that constant's own
    ///      doc comment): calls the building's own type object's vtable slot `0xa4` (`getPurchaseCost`)
    ///      and adds `round()` of its float return into [`Self::field_0x4c`], then calls the building
    ///      entity's **own** (not its type's) vtable slot `0x11c` four times, with category ids
    ///      `0x251f..=0x2522`, accumulating each `i32` result as a float into
    ///      [`Self::non_blank_tile_fraction`] (a running sum at this point - only divided into a true
    ///      fraction after the map walk in step 6 below; the field is reused as both an accumulator and
    ///      the final ratio, matching vanilla's own single-field reuse).
    ///
    ///    **Vanilla quirk, deliberately not reproduced**: `.asm` shows every one of the three branches
    ///    re-dispatches the *same* vtable-`0x1c` check a second time immediately after the first (a
    ///    defensive "confirm and possibly null out" idiom whose failure path would null-deref the very
    ///    next vtable call) - since both calls use identical arguments against the identical,
    ///    already-matched object, the second call is always true in every reachable case, so this port
    ///    skips it.
    /// 5. [`Self::animal_rating_metric`] is `animal_score_sum / num_animals` (or `0` if no animals), written into
    ///    [`Self::monthly_history`]/[`Self::yearly_history`]/[`Self::flat_totals`] at `0x480`/`0xc74`/
    ///    `0x1118`. [`Self::guest_rating_metric`] is `guest_score_sum / guest_tile_count` (or `0` if no guest
    ///    tiles), written at `0x4b0`/`0xcc4`/`0x111c`.
    /// 6. Walks every map tile (`GLOBAL_ZTWorldMgr::map_x_size` × `map_y_size`, via the already-modeled
    ///    [`crate::ztworldmgr::ZTWorldMgr::get_tile_from_pos`]/`get_ptr_from_bftile`) counting "blank"
    ///    tiles - a tile whose four raw dwords at `+0x4`/`+0x8`/`+0xc`/`+0x10` (the last being
    ///    [`crate::ztmapview::BFTile::entity_ptr`] itself) are all zero. If
    ///    `map_x_size * map_y_size - blank_count` is nonzero, divides [`Self::non_blank_tile_fraction`]
    ///    (the building-category sum from step 4) by it; otherwise leaves it as the raw sum, matching
    ///    vanilla's own `if (nonBlank != 0) field_0x64 /= nonBlank;` guard.
    /// 7. Refreshes [`Self::num_species`] via real vanilla [`GET_NUM_SPECIES`] against the live
    ///    `GLOBAL_ZTHabitatMgr` (already resolved elsewhere in this codebase), and writes
    ///    [`Self::field_0x4c`] into [`Self::monthly_history`]/[`Self::yearly_history`]/
    ///    [`Self::flat_totals`] at `0x420`/`0xbd4`/`0x1110`.
    /// 8. Computes [`Self::research_completion_percent`] as `completed * 100 / total` over every program
    ///    in every category in every branch of the live `GLOBAL_ZTResearchMgr` (already-modeled
    ///    [`crate::ztresearch::ZTResearchMgr::branches`]/`ZTResearchBranch::categories`/
    ///    `ZTResearchCategory::programs`, [`crate::ztresearch::ZTResearchProgram::is_complete`] matching
    ///    the `.asm`'s `target_cost <= current_progress` check exactly) - `100` if there are no programs
    ///    at all, matching vanilla's own fallback.
    ///
    /// **Not reproduced**: the `.asm`'s own `ExitApplicationGracefully` bound-check branches on the
    /// research walk (calls `FUN_005fd10e(10)`/exits the process) - genuine defensive array-bounds
    /// assertions that can never be reached by a well-formed `ZTResearchMgr` (the vector-length reads and
    /// the loop bounds they guard are derived from the exact same vector, so they can never actually
    /// disagree); Rust's own iterator-based walk here has no equivalent failure mode to begin with.
    pub fn calculate_sums(&mut self) {
        self.num_animals = 0;
        self.animal_condition_counter_1 = 0;
        self.num_tired_guests = 0;
        self.num_hungry_guests = 0;
        self.num_thirst_guests = 0;
        self.num_guests_restroom_need = 0;
        self.guest_condition_counter_1 = 0;
        self.guest_condition_counter_2 = 0;
        self.guest_tile_count = 0;

        let base_ptr = self as *mut Self as u32;
        let month_offset = self.current_month_index as u32 * 4;
        let year_offset = self.current_year_index as u32 * 4;
        save_to_memory(base_ptr + 0x600 + month_offset, 0.0f32);

        self.field_0x4c = globals().ztgamemgr().cash() as i32;
        self.field_0x58 = 0;
        self.non_blank_tile_fraction = 0.0;

        let base = get_module_base("zoo.exe") as u32;
        let need_threshold: i32 = get_from_memory(base + raw_globals::GUEST_NEED_THRESHOLD_RVA);

        let world = globals().ztworldmgr();
        let mut animal_score_sum: i32 = 0;
        let mut guest_score_sum: i32 = 0;

        let mut entity_addr = world.entity_array_start();
        while entity_addr != world.entity_array_end() {
            let entity_ptr: u32 = get_from_memory(entity_addr);
            entity_addr += 4;
            if entity_ptr == 0 {
                continue;
            }

            if entity_type_matches(entity_ptr, raw_globals::GUEST_TYPE_CHECK_RVA) {
                self.guest_tile_count += 1;
                let slot = base_ptr + 0x600 + month_offset;
                save_to_memory(slot, get_from_memory::<f32>(slot) + 1.0);

                let hunger: i32 = get_from_memory(entity_ptr + 0x2b0);
                if hunger > need_threshold {
                    self.num_hungry_guests += 1;
                }
                let thirst: i32 = get_from_memory(entity_ptr + 0x2b8);
                if thirst > need_threshold {
                    self.num_thirst_guests += 1;
                }
                let restroom: i32 = get_from_memory(entity_ptr + 0x2c8);
                if restroom > need_threshold {
                    self.num_guests_restroom_need += 1;
                }
                let tired: i32 = get_from_memory(entity_ptr + 0x2c0);
                if tired > need_threshold {
                    self.num_tired_guests += 1;
                }

                let flag: u8 = get_from_memory(entity_ptr + 0x33c);
                if flag != 0 {
                    self.guest_condition_counter_1 += 1;
                }
                let sub_object: u32 = get_from_memory(entity_ptr + 0x26c);
                let sub_flag: i32 = get_from_memory(sub_object + 0x10);
                if sub_flag != 0 {
                    self.guest_condition_counter_2 += 1;
                }

                let score: i32 = get_from_memory(entity_ptr + 0x2a8);
                guest_score_sum += score;
            } else if entity_type_matches(entity_ptr, raw_globals::ANIMAL_TYPE_CHECK_RVA) {
                self.num_animals += 1;
                let score: i32 = get_from_memory(entity_ptr + 0x2a8);
                animal_score_sum += score;
                let flag: u8 = get_from_memory(entity_ptr + 0x3a7);
                if flag != 0 {
                    self.animal_condition_counter_1 += 1;
                }

                let type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
                if type_ptr != 0 {
                    let vtable: u32 = get_from_memory(type_ptr);
                    let get_avg = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> f32>(get_from_memory::<u32>(vtable + 0xbc)) };
                    self.field_0x4c += get_avg(type_ptr) as i32;
                }
            } else if entity_type_matches(entity_ptr, raw_globals::BUILDING_TYPE_CHECK_RVA) {
                let type_ptr: u32 = get_from_memory(entity_ptr + 0x128);
                if type_ptr != 0 {
                    let vtable: u32 = get_from_memory(type_ptr);
                    let get_purchase_cost = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> f32>(get_from_memory::<u32>(vtable + 0xa4)) };
                    self.field_0x4c += get_purchase_cost(type_ptr) as i32;
                }

                let entity_vtable: u32 = get_from_memory(entity_ptr);
                let get_category_value = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32, i32) -> i32>(get_from_memory::<u32>(entity_vtable + 0x11c)) };
                for category_id in 0x251fi32..=0x2522 {
                    self.non_blank_tile_fraction += get_category_value(entity_ptr, category_id) as f32;
                }
            }
        }

        self.animal_rating_metric = if self.num_animals > 0 { animal_score_sum / self.num_animals as i32 } else { 0 };
        save_to_memory(base_ptr + 0x480 + month_offset, self.animal_rating_metric as f32);
        save_to_memory(base_ptr + 0xc74 + year_offset, self.animal_rating_metric as f32);
        save_to_memory(base_ptr + 0x1118, self.animal_rating_metric as f32);

        self.guest_rating_metric = if self.guest_tile_count > 0 { guest_score_sum / self.guest_tile_count as i32 } else { 0 };
        save_to_memory(base_ptr + 0x4b0 + month_offset, self.guest_rating_metric as f32);
        save_to_memory(base_ptr + 0xcc4 + year_offset, self.guest_rating_metric as f32);
        save_to_memory(base_ptr + 0x111c, self.guest_rating_metric as f32);

        let mut blank_tiles: u32 = 0;
        for y in 0..world.map_y_size {
            for x in 0..world.map_x_size {
                let Some(tile) = world.get_tile_from_pos(IVec3::new(x as i32, y as i32, 0)) else { continue };
                let tile_addr = world.get_ptr_from_bftile(&tile);
                let a: u32 = get_from_memory(tile_addr + 4);
                let b: u32 = get_from_memory(tile_addr + 8);
                let c: u32 = get_from_memory(tile_addr + 0xc);
                let entity_ptr: u32 = get_from_memory(tile_addr + 0x10);
                if a == 0 && b == 0 && c == 0 && entity_ptr == 0 {
                    blank_tiles += 1;
                }
            }
        }
        let total_tiles = world.map_x_size * world.map_y_size;
        let non_blank = total_tiles.saturating_sub(blank_tiles);
        if non_blank != 0 {
            self.non_blank_tile_fraction /= non_blank as f32;
        }

        self.num_species = unsafe { GET_NUM_SPECIES.original()(globals().zthabitatmgr_ptr() as *const u32) } as u16;
        save_to_memory(base_ptr + 0x420 + month_offset, self.field_0x4c as f32);
        save_to_memory(base_ptr + 0xbd4 + year_offset, self.field_0x4c as f32);
        save_to_memory(base_ptr + 0x1110, self.field_0x4c as f32);

        let mut completed = 0i32;
        let mut total = 0i32;
        for branch in globals().ztresearchmgr().branches() {
            for category in branch.categories() {
                for program in category.programs() {
                    if program.is_complete() {
                        completed += 1;
                    }
                    total += 1;
                }
            }
        }
        self.research_completion_percent = if total != 0 { completed * 100 / total } else { 100 };
    }

    /// Reimplementation of `ZooStatus::update` (per `generated.rs`'s `UPDATE` entry), Stage 4. Per
    /// `zoostatus_update.c`/`.asm` (both read in full and cross-checked instruction-by-instruction - no
    /// ambiguity here, every field is a direct, already-named offset):
    ///
    /// 1. Advances three independent "ticks since last check" accumulators
    ///    ([`Self::rating_check_elapsed`]/[`Self::message_check_elapsed`]/[`Self::newguest_check_elapsed`])
    ///    by `delta`, comparing each against its own configured interval
    ///    ([`Self::rating_check_interval`]/[`Self::message_check_interval`]/
    ///    [`Self::newguest_check_interval`]) to decide
    ///    whether [`Self::rating_checks`]/[`Self::message_checks`]/vanilla `newguestChecks` should run
    ///    this tick - [`Self::rating_checks`]'s own trigger is additionally OR'd with
    ///    [`Self::finance_check_pending`] (read *before* `financeChecks` would clear it later in this same
    ///    call, matching vanilla's own read-before-call ordering).
    /// 2. Calls the already-reimplemented `ZTMegatileMgr::update` on the live `GLOBAL_ZTMegatileMgr`
    ///    singleton (direct Rust call, not an address call-through - same no-address-call-through
    ///    rationale `ztgamemgr.rs`'s own `update`/`update_sim` use for their embedded reimplemented
    ///    sub-objects).
    /// 3. Dispatches the three interval checks (native [`Self::rating_checks`]/[`Self::message_checks`],
    ///    real vanilla `newguestChecks` call-through - see below), then real vanilla `financeChecks` if
    ///    [`Self::finance_check_pending`] is set, then a donation roll (`fChance` real vanilla
    ///    call-through, native [`Self::f_grant_donation`]) gated on the live budget being below
    ///    [`raw_globals::DONATION_CASH_THRESHOLD_RVA`].
    ///
    /// **Deliberate scope reduction from the implementation plan, applied here**: `financeChecks` stays a
    /// real vanilla call-through this stage, not reimplemented, despite the plan originally scoping it
    /// into Stage 4 (`newguestChecks`/`fZooMessage`/`fGrantDonation` were three more deferred methods in
    /// earlier passes - `newguestChecks` unblocked by a Ghidra re-decompile cleaning up its control-flow
    /// ambiguity, see [`Self::newguest_checks`]'s own doc comment; `fZooMessage` and `fGrantDonation`
    /// unblocked once `GLOBAL_BFUIMgr`'s address was resolved and, for `fGrantDonation`, once a further
    /// regen resolved `FUN_0040f103` as `msvc_std::RAND` - see [`Self::f_grant_donation`]'s own doc
    /// comment for both). `financeChecks` and [`Self::newguest_checks`]'s own `fCreateGuest` call share the
    /// exact risk profile `ztgamemgr.rs`'s own `removedZooDoo` attempt already hit and explicitly backed
    /// out from (see that module's doc comment) - **a real, tracked follow-up, not a dead end**: once
    /// `ZTWorldMgr`'s building list/`ZTBuilding` (for `financeChecks`) or `ZTWorldMgr`'s entity-creation
    /// path (for `fCreateGuest`) are themselves reimplemented and this codebase genuinely owns those
    /// structures/freelists, both become safe to port - see `zoostatus-implementation-plan.md`'s "Open
    /// risks" section for the tracked entry. `financeChecks` walks `ZTWorldMgr::getBuildingList`'s
    /// freelist-backed building list via a real vanilla `std::string` tag construction, exactly the
    /// pattern that made `removedZooDoo` "too much unverified surface for a single pass"; `fCreateGuest`
    /// builds a scratch tile-search buffer through the same small-object freelist
    /// (`&DAT_00638000`-indexed) `removedZooDoo` got stuck on, then live-spawns a new guest entity via an
    /// un-ported vtable "create instance" call into `ZTWorldMgr`. Calling `financeChecks` through its
    /// real, un-detoured address against this live, vanilla-layout-compatible struct is always safe (same
    /// call-through convention as `override`/`save`/`load`'s migration paths elsewhere in this module) -
    /// this is a documented, deliberate deferral, not an oversight.
    pub fn update(&mut self, delta: i32) {
        let this_ptr = self as *mut Self as *const u32;

        self.rating_check_elapsed += delta;
        let rating_pending = self.rating_check_elapsed > self.rating_check_interval || self.finance_check_pending;

        self.message_check_elapsed += delta;
        let message_pending = self.message_check_elapsed > self.message_check_interval;

        self.newguest_check_elapsed += delta;
        let newguest_pending = self.newguest_check_elapsed > self.newguest_check_interval;

        unsafe { &mut *globals().ztmegatilemgr_ptr() }.update(delta as u32);

        if rating_pending {
            self.rating_checks();
        }
        if message_pending {
            self.message_checks();
        }
        if newguest_pending {
            self.newguest_checks();
        }
        if self.finance_check_pending {
            unsafe { FINANCE_CHECKS.original()(this_ptr) };
        }

        let cash = globals().ztgamemgr().cash();
        let donation_threshold: f32 = get_from_memory(get_module_base("zoo.exe") as u32 + raw_globals::DONATION_CASH_THRESHOLD_RVA);
        if cash < donation_threshold {
            let chance = unsafe { F_CHANCE.original()(self.donation_chance_percent) };
            // Only the low byte is a defined result - real vanilla's own `fChance` (`ZooStatus_fChance.c`)
            // returns `in_EAX & 0xffffff00` (upper 3 bytes untouched leftover garbage, only ever cleared
            // to 0 in the low byte) when its `param_1` (== `donation_chance_percent`) is `0`, and its own
            // real caller here (`zoostatus_update.asm`) tests the result with `TEST %AL, %AL`, never the
            // full `EAX`. Comparing the untruncated `u32` (this port's original Stage-4 code) let stale
            // upper-byte garbage make `donation_chance_percent == 0` spuriously "roll true" and fire
            // `f_grant_donation` - crashed a live standalone-`ZTGameMgr` test, since `f_grant_donation`
            // grants through the *live* `GLOBAL_ZTGameMgr` (not `self`) and calls real vanilla
            // `BFApp::loadString`, both unsafe this early/against a non-live instance. Fixed to match the
            // real caller's own `TEST AL, AL` - see `ztgamemgr.rs`'s `update_sim`/`zoostatus_result & 0xff`
            // for the same established masking convention elsewhere in this codebase.
            if chance & 0xff != 0 {
                self.f_grant_donation();
            }
        }
    }

    /// Stage 7 port of `ZooStatus::save` (`0x0047ad4e`, per `ZooStatus_save.c`/`.asm`, read in full):
    /// a straight linear write, no version branching at all - vanilla's `save` always emits the
    /// current format. Field order matches the decompile exactly: the eleven scalar header fields
    /// `save.c` names explicitly, then two literal `i32` markers (`31`/`20` - this struct's own
    /// [`Self::monthly_history`] category count and [`Self::yearly_history`] year count, matching the
    /// confirmed array geometry, not re-derived from `self`) that [`Self::load`]'s current-version path
    /// reads back to size its own loops, then the three history/total array regions, then the
    /// admission price and the escape timestamp.
    ///
    /// Vanilla writes each of the 744 array elements (`31*12 + 31*20 + 31`) with its own individual
    /// `WriteBytesToFile(ptr, 4, 1, file)` call; this writes each whole region in one bulk call
    /// instead. Both produce byte-identical output (a real `fwrite`-shaped primitive given `size=4,
    /// count=N` writes the same `4*N` contiguous bytes a loop of `N` `size=4, count=1` calls would),
    /// and this struct's `#[repr(C)]` array layout is already confirmed byte-identical to vanilla's own
    /// per-element write order (see the module doc comment's struct-layout derivation) - so nothing is
    /// lost by not replicating the loop itself.
    ///
    /// The escape timestamp is one 8-byte call (`&self.last_animal_escape_timestamp_low`, size `8`),
    /// matching vanilla's own single `WriteBytesToFile(&this[0xd].field_0x34, 8, 1, file)` exactly -
    /// the two `u32` fields are laid out contiguously with no padding between them (see their own doc
    /// comment).
    ///
    /// Returns `0`/`1` in the low byte, like every other `*::save` in this codebase - vanilla's own
    /// `CONCAT31` return construction leaves the upper 3 bytes as unrelated register garbage (the same
    /// wart `standalone::WRITE_BYTES_TO_FILE` itself carries, per the plan's Dependencies section), so
    /// only the low byte is a defined contract.
    pub fn save(&self, file: *const i8) -> u32 {
        let mut ok = write_bytes_to_file(&self.rating_check_elapsed, file);
        ok &= write_bytes_to_file(&self.message_check_elapsed, file);
        ok &= write_bytes_to_file(&self.newguest_check_elapsed, file);
        ok &= write_bytes_to_file(&self.finance_check_pending, file);
        ok &= write_bytes_to_file(&self.zoo_rating_current, file);
        ok &= write_bytes_to_file(&self.field_0x48, file);
        ok &= write_bytes_to_file(&self.field_0x50, file);
        ok &= write_bytes_to_file(&self.field_0x54, file);
        ok &= write_bytes_to_file(&self.donation_count_this_period, file);
        ok &= write_bytes_to_file(&self.current_month_index, file);
        ok &= write_bytes_to_file(&self.current_year_index, file);

        let monthly_category_count: i32 = self.monthly_history.len() as i32;
        let yearly_year_count: i32 = self.yearly_history[0].len() as i32;
        ok &= write_bytes_to_file(&monthly_category_count, file);
        ok &= write_bytes_to_file(&yearly_year_count, file);

        ok &= write_bytes_to_file(&self.monthly_history, file);
        ok &= write_bytes_to_file(&self.yearly_history, file);
        ok &= write_bytes_to_file(&self.flat_totals, file);

        ok &= write_bytes_to_file(&self.admission_price, file);
        ok &= unsafe { WRITE_BYTES_TO_FILE.hooked()(&self.last_animal_escape_timestamp_low as *const u32, 8, 1, file) == 1 };

        ok as u32
    }

    /// Stage 9 port of `ZooStatus::load` (`0x0059497f`, per `ZooStatus_load.c`/`.asm`, both read in
    /// full for this stage) - the full version range, down to the `0xc`-byte minimum, per the plan's
    /// own "flagged as hardest" scoping (`zoostatus-implementation-plan.md`'s `save`/`load` section).
    /// Six thresholds gate vanilla's own control flow (`0xc`/`0x17`/`0x18`/`0x19`/`0x26`/`0x27`,
    /// `0x47` staying the boundary Stage 7 already covered):
    ///
    /// - `version <= 0xc`: nothing at all is read - every field this method could touch keeps whatever
    ///   [`Self::init`] already put there. Real vanilla control flow skips straight past the entire
    ///   scalar/array region to the tail (`ZooStatus_load.asm`'s very first `CMP EAX,0xd; JC .13d5bc`).
    /// - `0xc < version < 0x17`: the header scalars only ([`Self::rating_check_elapsed`] through
    ///   [`Self::donation_count_this_period`]) - the array regions and their own cursor fields
    ///   ([`Self::current_month_index`]/[`Self::current_year_index`]) are never read at all, so this
    ///   port's default/`init`-seeded values pass through unchanged for a save this old. Two of those
    ///   header reads are themselves conditional: [`Self::finance_check_pending`] is read as a raw
    ///   `i32` and derived (`value > 360_000`) rather than read as a `bool` byte for `version < 0x17`,
    ///   and one extra `i32` is read-and-discarded for `version < 0x18` (a field the current format no
    ///   longer stores at that position).
    /// - `0x17 <= version < 0x26`: the real migration path. Every `(category, month)`/`(category,
    ///   year)`/`category` slot is read as a raw `i32` (`FILD`, a genuine int->float *conversion* -
    ///   `ZooStatus_load.asm`'s `.184eba`/`.184f84`/`.18505c` blocks - unlike every other version range,
    ///   which reads the stored bytes as an already-IEEE754 `f32` via a plain reinterpreting copy). For
    ///   `version < 0x19` specifically, every one of those same raw values *also* feeds a second,
    ///   independent adjustment against a single shared accumulator - real, `.asm`-confirmed category
    ///   row **14** of each region (`monthly_history[14]`/`yearly_history[14]`/`flat_totals[14]`,
    ///   resolved directly from literal displacements `this+0x3f4`/`this+0xb84`/`this+0x110c`, *not*
    ///   from the decompile's own `local_14[2].mbr_0x14c`-style pseudo-array indexing - that guessed
    ///   stride is wrong here the same way the plan's "history-array region" section already found it
    ///   wrong elsewhere). Which of add/subtract/skip applies is decided per outer-loop position (the
    ///   region's own flat float-index for monthly/yearly, the bare category index for flat) against a
    ///   five-threshold band lifted verbatim from `.asm`'s literal `CMP`/`JLE`/`JL`/`JGE` chain (see
    ///   [`Self::legacy_band`]) - not re-derived algebraically, to avoid an off-by-one against real
    ///   vanilla's own branch structure. Because this accumulator is a real struct field rather than
    ///   scratch memory, a save whose `category_count` reaches row 14 (`>= 15`) has that row's migrated
    ///   value overwritten by its own straight read on that category's own turn - real vanilla behavior
    ///   this port reproduces exactly rather than "fixes", confirmed intentional-shaped: row 14's own
    ///   band position (linear index `0x55+14*12=0xfd` monthly, category `14` flat) always falls outside
    ///   every add/subtract band, so it never re-differences itself either way. No zero-fill tail here -
    ///   unlike every other reachable branch, an unreachable `[15..31)` region for a genuinely old save
    ///   simply keeps whatever [`Self::init`]/[`Self::reset_finance_info`] already zeroed it to.
    /// - `0x26 <= version < 0x47`: the same array shape [`Self::load`]'s Stage 7 fast path already
    ///   covers (raw reinterpreting `f32` reads, no int conversion, no migration adjustment - i.e. every
    ///   nested `if (param_2 < 0x19)` inside `ZooStatus_load.c`'s own `else` branch is unreachable dead
    ///   code once `version >= 0x26`, since `0x26 > 0x19`), but *without* this struct's own `>= 0x47`
    ///   fast path's hard `category_count`/`year_count` validation - vanilla's own bound checks here are
    ///   purely per-slot (`iVar17 < 0x1c9`/`iVar13 < 0x435`/`iVar17 < 0x1f`), silently discarding
    ///   whatever doesn't fit rather than failing the whole load, which matters for real backward
    ///   compatibility with saves whose category/year counts may not be this port's own `31`/`20`.
    /// - `version < 0x27`: [`Self::admission_price`] defaults to `49.0` (`0x42440000`) instead of being
    ///   read.
    /// - `version < 0x47`: [`Self::last_animal_escape_timestamp_low`]/`_high` are re-seeded from
    ///   [`GET_OLD_DATE`] instead of being read, and `load` returns immediately afterward without
    ///   attempting any further read - matching `ZooStatus_load.c`'s own early `return` in that branch.
    ///
    /// Two `i32` markers, written by [`Self::save`] as the fixed `31`/`20` constants matching this
    /// struct's own array geometry for a save *this port* produces, are read back for `version >= 0x17`
    /// as `category_count`/`year_count` and used as the array-region read's own loop bounds - matching
    /// vanilla's `local_8`/`local_4`. Vanilla bails out immediately, without touching the array regions
    /// or the tail fields at all, if any read up to and including these two markers fails
    /// (`if (local_19 == 0) return ...`) - reproduced here as an early `return 0`, reachable only once
    /// `version >= 0x17` (older saves never read these markers, so never hit this check).
    ///
    /// This struct's own `>= 0x47` fast path keeps its Stage 7 hard-failure behavior for a
    /// `category_count`/`year_count` that doesn't fit `31`/`20` unchanged (every save this port's own
    /// [`Self::save`] produces is exactly `31`/`20` by construction, and no other producer of a
    /// `version >= 0x47` save is expected to differ - see that branch's own history, kept verbatim). The
    /// `0x26 <= version < 0x47` and `0x17 <= version < 0x26` branches added this stage instead follow
    /// vanilla's own genuinely more permissive discard/zero-fill behavior throughout, since backward
    /// compatibility with a real foreign/historical save's differently-shaped region is the entire point
    /// of porting them. One deliberate, narrow divergence from a literal transcription: the `version <
    /// 0x19` migration's yearly-region accumulator write is guarded to `year < 20` (this struct's own
    /// row width) rather than replicated unguarded - a `year_count > 20` old save would have vanilla's
    /// own accumulator pointer walk *past* `yearly_history[14]`'s real bounds into adjacent struct
    /// memory, which this port cannot safely reproduce without genuine unsafe out-of-bounds writes for
    /// an edge case no real save is expected to hit (year counts only ever grew *to* 20, never past it).
    pub fn load(&mut self, file: *const u32, version: u32) -> u32 {
        if version <= 0xc {
            return self.load_tail(version, file, true);
        }

        let mut ok = read_bytes(&mut self.rating_check_elapsed, file);
        ok &= read_bytes(&mut self.message_check_elapsed, file);
        ok &= read_bytes(&mut self.newguest_check_elapsed, file);

        if version < 0x17 {
            let mut raw: i32 = 0;
            ok &= read_bytes(&mut raw, file);
            self.finance_check_pending = raw > 360_000;
        } else {
            ok &= read_bytes(&mut self.finance_check_pending, file);
        }

        ok &= read_bytes(&mut self.zoo_rating_current, file);
        ok &= read_bytes(&mut self.field_0x48, file);
        ok &= read_bytes(&mut self.field_0x50, file);
        ok &= read_bytes(&mut self.field_0x54, file);

        if version < 0x18 {
            let mut discard: i32 = 0;
            ok &= read_bytes(&mut discard, file);
        }

        ok &= read_bytes(&mut self.donation_count_this_period, file);

        if version < 0x17 {
            return self.load_tail(version, file, ok);
        }

        ok &= read_bytes(&mut self.current_month_index, file);
        ok &= read_bytes(&mut self.current_year_index, file);

        let mut category_count: i32 = 0;
        let mut year_count: i32 = 0;
        ok &= read_bytes(&mut category_count, file);
        ok &= read_bytes(&mut year_count, file);

        if !ok {
            return 0;
        }

        if version < 0x26 {
            ok &= self.load_history_legacy_migration(file, version, category_count, year_count);
        } else if version < 0x47 {
            ok &= self.load_history_compat(file, category_count, year_count);
        } else {
            if category_count < 0
                || year_count < 0
                || category_count as usize > self.monthly_history.len()
                || year_count as usize > self.yearly_history[0].len()
            {
                error!(
                    "ZooStatus::load: category_count={category_count}/year_count={year_count} out of range for this port's {}-category/{}-year history regions",
                    self.monthly_history.len(),
                    self.yearly_history[0].len()
                );
                return 0;
            }
            let category_count = category_count as usize;
            let year_count = year_count as usize;

            for row in &mut self.monthly_history[..category_count] {
                for month in row.iter_mut() {
                    ok &= read_bytes(month, file);
                }
            }
            for row in &mut self.yearly_history[..category_count] {
                for year in &mut row[..year_count] {
                    ok &= read_bytes(year, file);
                }
            }
            for slot in &mut self.flat_totals[..category_count] {
                ok &= read_bytes(slot, file);
            }

            // Vanilla zero-fills every remaining category/year row when an older/foreign save wrote
            // fewer than this struct's own 31 categories (`ZooStatus_load.c`'s tail `do`/`while`) -
            // real behavior, not a migration guess. Never exercised by this port's own round-trip
            // (`save` always writes `31`/`20`), kept for a save written by some other build with a
            // shorter region.
            for row in &mut self.monthly_history[category_count..] {
                *row = [0.0; 12];
            }
            for row in &mut self.yearly_history[category_count..] {
                *row = [0.0; 20];
            }
            for slot in &mut self.flat_totals[category_count..] {
                *slot = 0.0;
            }
        }

        self.load_tail(version, file, ok)
    }

    /// `0x26 <= version < 0x47` array-region read for [`Self::load`]: the same shape as `load`'s own
    /// `>= 0x47` fast path (raw reinterpreting `f32` reads, no migration adjustment - real vanilla's own
    /// nested `if (param_2 < 0x19)` is dead code once `version >= 0x26`), but bounds-checked per slot
    /// rather than hard-failing on an out-of-range `category_count`/`year_count`, matching
    /// `ZooStatus_load.c`'s own `else` branch (`iVar17 < 0x1c9`/`iVar13 < 0x435`/`iVar17 < 0x1f` inline
    /// discards) - real backward-compatibility behavior for a foreign/historical save whose region
    /// doesn't match this port's own `31`-category/`20`-year geometry. Every slot is still read
    /// (discarded if out of bounds) to keep the file cursor aligned with vanilla's own read count, and
    /// every row from `category_count` up to `31` is zero-filled afterward exactly like the `>= 0x47`
    /// path's own tail loop (`ZooStatus_load.c`'s shared tail `do`/`while`, `.asm` label `.1599a3`).
    fn load_history_compat(&mut self, file: *const u32, category_count: i32, year_count: i32) -> bool {
        let mut ok = true;

        for category in 0..category_count.max(0) {
            for month in 0..12 {
                let mut raw: f32 = 0.0;
                ok &= read_bytes(&mut raw, file);
                if (category as usize) < 31 {
                    self.monthly_history[category as usize][month] = raw;
                }
            }
        }
        for category in 0..category_count.max(0) {
            for year in 0..year_count.max(0) {
                let mut raw: f32 = 0.0;
                ok &= read_bytes(&mut raw, file);
                if (category as usize) < 31 && (year as usize) < 20 {
                    self.yearly_history[category as usize][year as usize] = raw;
                }
            }
        }
        for category in 0..category_count.max(0) {
            let mut raw: f32 = 0.0;
            ok &= read_bytes(&mut raw, file);
            if (category as usize) < 31 {
                self.flat_totals[category as usize] = raw;
            }
        }

        let filled = category_count.clamp(0, 31) as usize;
        for row in &mut self.monthly_history[filled..] {
            *row = [0.0; 12];
        }
        for row in &mut self.yearly_history[filled..] {
            *row = [0.0; 20];
        }
        for slot in &mut self.flat_totals[filled..] {
            *slot = 0.0;
        }

        ok
    }

    /// `0x17 <= version < 0x26` array-region read for [`Self::load`] - the real pre-migration path, see
    /// [`Self::load`]'s own doc comment for the full evidence trail (`.asm` labels `.184eba`
    /// monthly/`.184f84` yearly/`.18505c` flat). Every slot is a genuine `i32`->`f32` *conversion*
    /// (`FILD`, not a reinterpreting copy), written into row/column `category`/`month`|`year` if it
    /// fits this struct's own `31`-category/`20`-year geometry (discarded, not hard-failed, otherwise -
    /// real vanilla saves from this era could exceed either), and, for `version < 0x19` only, also folds
    /// into the shared row-14 accumulator per [`Self::legacy_band`]'s verdict for that slot's position.
    /// No zero-fill tail - see [`Self::load`]'s own doc comment for why none is needed here.
    fn load_history_legacy_migration(&mut self, file: *const u32, version: u32, category_count: i32, year_count: i32) -> bool {
        let mut ok = true;

        for category in 0..category_count.max(0) {
            let row_base = 0x55 + category * 0xc;
            for month in 0..12usize {
                let mut raw: i32 = 0;
                ok &= read_bytes(&mut raw, file);
                if (category as usize) < 31 {
                    self.monthly_history[category as usize][month] = raw as f32;
                }
                if version < 0x19 && let Some(add) = Self::legacy_band(row_base, 0x79, 0xd9, 0xf1, 0x91, 0xcd) {
                    let target = &mut self.monthly_history[14][month];
                    *target = if add { *target + raw as f32 } else { *target - raw as f32 };
                }
            }
        }
        for category in 0..category_count.max(0) {
            let row_base = 0x1c9 + category * 0x14;
            for year in 0..year_count.max(0) as usize {
                let mut raw: i32 = 0;
                ok &= read_bytes(&mut raw, file);
                if (category as usize) < 31 && year < 20 {
                    self.yearly_history[category as usize][year] = raw as f32;
                }
                if version < 0x19 && year < 20 && let Some(add) = Self::legacy_band(row_base, 0x205, 0x2a5, 0x2cd, 0x22d, 0x291) {
                    let target = &mut self.yearly_history[14][year];
                    *target = if add { *target + raw as f32 } else { *target - raw as f32 };
                }
            }
        }
        for category in 0..category_count.max(0) {
            let mut raw: i32 = 0;
            ok &= read_bytes(&mut raw, file);
            if (category as usize) < 31 {
                self.flat_totals[category as usize] = raw as f32;
            }
            if version < 0x19 && let Some(add) = Self::legacy_band(category, 3, 0xb, 0xd, 5, 0xa) {
                let target = &mut self.flat_totals[14];
                *target = if add { *target + raw as f32 } else { *target - raw as f32 };
            }
        }

        ok
    }

    /// Mirrors `ZooStatus_load.asm`'s literal band-check control flow used by the pre-`0x19` history
    /// migration (`.184efe`-`.184f3a` for monthly, `.184fd5`-`.185014` for yearly, `.185089`-`.1850cb`
    /// for flat) - same five-threshold shape in all three, different literal thresholds per region.
    /// `x` is the region's own outer-loop position (a flat float-index for monthly/yearly, the bare
    /// category index for flat). Returns `Some(true)` to add, `Some(false)` to subtract, `None` to
    /// leave the target untouched - matching the real `CMP`/`JLE`/`JL`/`JGE` chain exactly rather than
    /// an algebraically-simplified re-derivation, to avoid an off-by-one against real vanilla.
    fn legacy_band(x: i32, subtract_le: i32, subtract_ge: i32, subtract_le2: i32, add_ge: i32, add_le: i32) -> Option<bool> {
        if x <= subtract_le {
            return Some(false);
        }
        if x >= subtract_ge && x <= subtract_le2 {
            return Some(false);
        }
        if x < add_ge || x > add_le {
            return None;
        }
        Some(true)
    }

    /// Shared tail for every [`Self::load`] version range (`ZooStatus_load.asm`'s `.13d5bc`/`.13d5b8`
    /// labels, reached by every branch above): [`Self::admission_price`] for `version >= 0x27`, else a
    /// hardcoded `49.0` default; then, for `version >= 0x47`, [`Self::last_animal_escape_timestamp_low`]/
    /// `_high` read from file and the accumulated `ok` returned, or for `version < 0x47`, those two
    /// fields re-seeded from [`GET_OLD_DATE`] and an **immediate** return - vanilla's own `version < 0x47`
    /// branch returns right after the seed with no further read, which this mirrors exactly.
    fn load_tail(&mut self, version: u32, file: *const u32, mut ok: bool) -> u32 {
        if version < 0x27 {
            self.admission_price = 49.0;
        } else {
            ok &= read_bytes(&mut self.admission_price, file);
        }

        if version < 0x47 {
            let old_date = unsafe { GET_OLD_DATE.original()() } as u64;
            self.last_animal_escape_timestamp_low = old_date as u32;
            self.last_animal_escape_timestamp_high = (old_date >> 32) as u32;
            return ok as u32;
        }

        ok &= unsafe {
            DEALLOCATE.hooked()(&mut self.last_animal_escape_timestamp_low as *mut u32 as *const u32, 8, 1, file as *const u8) == 1
        };

        ok as u32
    }
}

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
/// for the mislabeling this corrects. `GET_STATUS`'s detour uses the locally-corrected
/// [`GET_STATUS_FIXED`] `FunctionDef`, not `generated.rs`'s own entry (wrong return type) - see its doc
/// comment.
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

    #[detour(SHOW_PRICES)]
    unsafe extern "thiscall" fn show_prices(this: *const u32) {
        unsafe { ref_from_memory::<ZooStatus>(this) }.show_prices();
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
    unsafe extern "thiscall" fn save(this: *const u32, file: *const i8) -> u32 {
        unsafe { ref_from_memory::<ZooStatus>(this) }.save(file)
    }

    /// `file` is declared `*const u8` in `generated.rs`; [`ZooStatus::load`] takes `*const u32` (matching
    /// `ztgamemgr.rs`'s own `load`'s file-handle type) - cast only, same handle either way.
    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u8, version: u32) -> u32 {
        unsafe { mut_from_memory::<ZooStatus>(this) }.load(file as *const u32, version)
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

    /// Uses [`GET_STATUS_FIXED`] (the locally-corrected `FunctionDef`), not `generated.rs`'s own
    /// `GET_STATUS` entry - see its doc comment for why.
    #[detour(GET_STATUS_FIXED)]
    unsafe extern "thiscall" fn get_status(this: *const u32, category: i32, when: i32, index: i32) -> f32 {
        unsafe { ref_from_memory::<ZooStatus>(this) }.get_status(category, when, index)
    }

    /// Live-test access to each detour's installation state. Once `init_detours()` has patched these
    /// 36 addresses, `.original()` on them re-enters the Rust detours above instead of reaching vanilla
    /// in release builds (a raw address cast there); debug builds route `.original()` through the hook
    /// registry's trampolines instead, unaffected by hook state - see
    /// `ztgamemgr_menumusichandler.rs`'s `menu_music_handler_detours::test_real` doc comment for the
    /// full per-profile rationale this mirrors. Lives inside the detour module because the
    /// macro-generated `*_DETOUR` statics are module-private.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        /// `(name, is_enabled)` per detour - the battery asserts all 36 to catch a silently-failed
        /// `init_detours()` (error logged, game continues on vanilla).
        pub(crate) fn status() -> [(&'static str, bool); 36] {
            [
                ("INIT", super::INIT_DETOUR.is_enabled()),
                ("OVERRIDE", super::OVERRIDE_DETOUR.is_enabled()),
                ("RESET_FINANCE_INFO", super::RESET_FINANCE_INFO_DETOUR.is_enabled()),
                ("SPEND_CONSTRUCTION", super::SPEND_CONSTRUCTION_DETOUR.is_enabled()),
                ("SPEND_BUILDING_UPKEEP", super::SPEND_BUILDING_UPKEEP_DETOUR.is_enabled()),
                ("SPEND_GUIDE_WAGES", super::SPEND_GUIDE_WAGES_DETOUR.is_enabled()),
                ("BUY_ANIMAL", super::BUY_ANIMAL_DETOUR.is_enabled()),
                ("SPEND_KEEPER_WAGES", super::SPEND_KEEPER_WAGES_DETOUR.is_enabled()),
                ("SPEND_MAINT_WAGES", super::SPEND_MAINT_WAGES_DETOUR.is_enabled()),
                ("SPEND_MARKETING", super::SPEND_MARKETING_DETOUR.is_enabled()),
                ("SPEND_RESEARCH", super::SPEND_RESEARCH_DETOUR.is_enabled()),
                ("REFUND_ANIMAL_COST", super::REFUND_ANIMAL_COST_DETOUR.is_enabled()),
                ("REFUND_CONSTRUCTION", super::REFUND_CONSTRUCTION_DETOUR.is_enabled()),
                ("INCREASE_DONATIONS", super::INCREASE_DONATIONS_DETOUR.is_enabled()),
                ("INCREASE_ENDOWMENT", super::INCREASE_ENDOWMENT_DETOUR.is_enabled()),
                ("INCREASE_SHOW_ADMISSION", super::INCREASE_SHOW_ADMISSION_DETOUR.is_enabled()),
                ("BUY_PEOPLE_FOOD", super::BUY_PEOPLE_FOOD_DETOUR.is_enabled()),
                ("CHANGE_ENDOWMENT_MEMBERS", super::CHANGE_ENDOWMENT_MEMBERS_DETOUR.is_enabled()),
                ("ANIMAL_ESCAPED", super::ANIMAL_ESCAPED_DETOUR.is_enabled()),
                ("ADMISSION_MESSAGE", super::ADMISSION_MESSAGE_DETOUR.is_enabled()),
                ("NEWGUEST_CHECKS", super::NEWGUEST_CHECKS_DETOUR.is_enabled()),
                ("MESSAGE_CHECKS", super::MESSAGE_CHECKS_DETOUR.is_enabled()),
                ("RATING_CHECKS", super::RATING_CHECKS_DETOUR.is_enabled()),
                ("F_GRANT_DONATION", super::F_GRANT_DONATION_DETOUR.is_enabled()),
                ("F_ZOO_MESSAGE", super::F_ZOO_MESSAGE_DETOUR.is_enabled()),
                ("SET_ADULT_ADMISSION_PRICE", super::SET_ADULT_ADMISSION_PRICE_DETOUR.is_enabled()),
                ("SHOW_PRICES", super::SHOW_PRICES_DETOUR.is_enabled()),
                ("CALCULATE_SUMS", super::CALCULATE_SUMS_DETOUR.is_enabled()),
                ("UPDATE", super::UPDATE_DETOUR.is_enabled()),
                ("SAVE", super::SAVE_DETOUR.is_enabled()),
                ("LOAD", super::LOAD_DETOUR.is_enabled()),
                ("HEAL_ANIMAL", super::HEAL_ANIMAL_DETOUR.is_enabled()),
                ("PURCHASE_FOOD", super::PURCHASE_FOOD_DETOUR.is_enabled()),
                ("INCREASE_ADMISSIONS_INCOME", super::INCREASE_ADMISSIONS_INCOME_DETOUR.is_enabled()),
                ("INCREASE_ADMISSIONS", super::INCREASE_ADMISSIONS_DETOUR.is_enabled()),
                ("GET_STATUS_FIXED", super::GET_STATUS_FIXED_DETOUR.is_enabled()),
            ]
        }
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

    /// `(name, is_enabled)` per detour - see `zoostatus_detours::test_real::status`.
    pub(crate) fn detour_status() -> [(&'static str, bool); 36] {
        zoostatus_detours::test_real::status()
    }
}

#[cfg(test)]
mod tests {
    use std::mem::offset_of;

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
