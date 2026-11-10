//! Raw RVAs for global variables and constants read by ZooStatus methods.

pub mod raw_globals {
    /// `ratingChecks`' "non-blank-tile-fraction cap" scale factor (`f32`,
    /// `non_blank_tile_fraction * DAT_00630d60`).
    pub const ATTENDANCE_FRACTION_SCALE_RVA: u32 = 0x00630d60 - 0x400000;
    /// `ratingChecks`' cap-vs-floor comparison threshold (`f32`) for the attendance-fraction term.
    pub const ATTENDANCE_FRACTION_FLOOR_RVA: u32 = 0x00630d64 - 0x400000;
    /// `ratingChecks`' comparison threshold (`f32`) selecting between the attendance-fraction term and the
    /// research-decay fallback.
    pub const ATTENDANCE_VS_RESEARCH_THRESHOLD_RVA: u32 = 0x00630d5c - 0x400000;
    /// `ratingChecks`' final decay-penalty scale factor (`f32`).
    pub const RATING_DECAY_SCALE_RVA: u32 = 0x00630d74 - 0x400000;
    /// `ratingChecks`' escape-recency decay baseline (`i32`, `DAT_006392a0 - DAT_00639294 *
    /// (hoursSinceEscape / 24)`).
    pub const ESCAPE_DECAY_BASELINE_RVA: u32 = 0x006392a0 - 0x400000;
    /// `ratingChecks`' escape-recency decay-per-day rate (`i32`), paired with
    /// [`ESCAPE_DECAY_BASELINE_RVA`].
    pub const ESCAPE_DECAY_PER_DAY_RVA: u32 = 0x00639294 - 0x400000;
    /// The escaped-animal `std::list<ZTAnimal*>`'s head-pointer *global variable* - **not** the sentinel
    /// node's own address.
    pub const ESCAPED_ANIMAL_LIST_SENTINEL_RVA: u32 = 0x00638fb0 - 0x400000;
    /// `ZooStatus::update`'s donation-roll gate: only rolls [`super::super::zoostatus::F_CHANCE`] when the live budget
    /// (`ZTGameMgr::cash`) is below this threshold (`f32`).
    pub const DONATION_CASH_THRESHOLD_RVA: u32 = 0x00635128 - 0x400000;

    /// `newguestChecks`' four admission-price tier boundaries (`f32`, compared against
    /// [`super::super::zoostatus::ZooStatus::admission_price`] in a `< , < , <=(<=), else` chain).
    pub const PRICE_TIER_BOUNDARY_0_RVA: u32 = 0x006392ac - 0x400000;
    pub const PRICE_TIER_BOUNDARY_1_RVA: u32 = 0x006392b0 - 0x400000;
    pub const PRICE_TIER_BOUNDARY_2_RVA: u32 = 0x006392b4 - 0x400000;
    pub const PRICE_TIER_BOUNDARY_3_RVA: u32 = 0x006392b8 - 0x400000;
    /// The `cAdultAdmission` list's fifth and last value (real `economy.cfg`: `0`).
    pub const PRICE_TIER_BOUNDARY_4_RVA: u32 = 0x006392bc - 0x400000;
    /// `newguestChecks`' "double the marketing benefit" event flag (`bool`, stored as a byte).
    pub const DOUBLE_MARKETING_BENEFIT_FLAG_RVA: u32 = 0x006392c1 - 0x400000;
    /// `newguestChecks`' flat `+30` attendance-bonus event flag (`bool`, stored as a byte).
    pub const FLAT_ATTENDANCE_BONUS_FLAG_RVA: u32 = 0x006392c0 - 0x400000;
    /// `GLOBAL_BFUIMgr`'s RVA.
    pub const GLOBAL_BFUIMGR_RVA: u32 = 0x00638de0 - 0x400000;

    /// `calculateSums`' entity-type-check argument for "is this a guest" (`&DAT_00638700` in the decompile).
    pub const GUEST_TYPE_CHECK_RVA: u32 = 0x00638700 - 0x400000;
    /// `calculateSums`' entity-type-check argument for "is this an animal" (`&DAT_00638690`).
    pub const ANIMAL_TYPE_CHECK_RVA: u32 = 0x00638690 - 0x400000;
    /// `calculateSums`' entity-type-check argument for the third tile-content category (`&DAT_00638670`).
    pub const BUILDING_TYPE_CHECK_RVA: u32 = 0x00638670 - 0x400000;
    /// `calculateSums`' guest-need threshold (`i32`).
    pub const GUEST_NEED_THRESHOLD_RVA: u32 = 0x00639024 - 0x400000;
    /// `showPrices`' child-admission-price scale factor (`f32`).
    pub const CHILD_ADMISSION_PRICE_SCALE_RVA: u32 = 0x00630d54 - 0x400000;
}
