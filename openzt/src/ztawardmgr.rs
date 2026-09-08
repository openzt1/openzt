//! Structs and methods for the vanilla `ZTAwardMgr` class, which tracks which zoo-achievement awards the
//! player has earned (persisted) and the catalogue of awards the `award*.cfg` resource files define (rebuilt from resources
//! every load, never persisted).
//!
//! Unlike this codebase's other `ZT*Mgr` reimplementations (`ZTMarketingMgr`/`ZTResearchMgr`/
//! `ZTThoughtMgr`/`ZTMegatileMgr`), `ZTAwardMgr` has no vtable and its global instance is a
//! **directly-embedded** struct at a fixed address (`0x006390e8`), not a pointer to a heap allocation - see
//! `private/docs/vtables/ZTAwardMgr.md`. A full decompile-corpus grep found exactly two external callers
//! that read its fields directly (bypassing any method call): `ZTScenarioSimpleGoal::eval`'s case `0xb`
//! arm (see [`eval_award_count_override`]) and `_showAwards` (see [`show_awards_detour`]) - both of those
//! are reimplemented/detoured here too, closing the surface. Every other caller goes through a plain
//! method call and is layout-agnostic.
//!
//! That closed surface makes this class a candidate for CLAUDE.md's "fully independent Rust store" style:
//! the constructor/destructor are deliberately left un-detoured (vanilla's own copy of the tree/vector
//! becomes inert dead weight, never read or written by any of the code below), which sidesteps the
//! cross-allocator hazard class entirely. [`live_support::read_vanilla_award_tree`] is the sole exception -
//! a read-only, never-mutating/freeing walk of vanilla's real tree, used only by the live-comparison test
//! suite to check the two representations agree.

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};

use openzt_configparser::ini::Ini;
use openzt_detour::generated::standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE};
use tracing::error;

use crate::{
    encoding_utils::decode_game_text,
    globals::{get_module_base, globals},
    resource_manager::lazyresourcemap::get_file,
    string_registry::load_string_by_id,
    util::{get_from_memory, ref_from_memory, ZTBufferString},
    zoostatus::ZooStatus,
};

/// One entry in the `award*.cfg` catalogue - rebuilt from resources by [`start`] every load, never
/// persisted.
#[derive(Debug, Clone, Default)]
pub struct AwardData {
    name_id: i32,
    tooltip_id: i32,
    icon: String,
}

impl AwardData {
    pub fn name_id(&self) -> i32 {
        self.name_id
    }

    pub fn tooltip_id(&self) -> i32 {
        self.tooltip_id
    }

    pub fn icon(&self) -> &str {
        &self.icon
    }
}

/// Process-global store backing the one real `ZTAwardMgr` singleton. Unlike `ztthoughtmgr.rs`'s
/// `HashMap<u32, ..>`-keyed registry, there's only ever one real instance and its address is a
/// compile-time constant baked into vanilla's own constructor (never passed as a parameter anywhere), so
/// no per-instance key is needed here.
#[derive(Debug, Default)]
struct ZTAwardMgrState {
    /// Vanilla's `+0xc` `vector<int>` - the earned/awarded award-id set, kept sorted and deduped.
    /// Persisted by [`save`]/[`load`].
    earned_ids: Vec<i32>,
    /// Vanilla's `+0x0` red-black tree - the `award*.cfg` catalogue, keyed by award id. Rebuilt by
    /// [`start`] every load; never persisted.
    awards: BTreeMap<i32, AwardData>,
}

static AWARD_MGR: LazyLock<Mutex<ZTAwardMgrState>> = LazyLock::new(|| Mutex::new(ZTAwardMgrState::default()));

/// Reimplementation of `OOAnalyzer::ZTAwardMgr::addAward`. Vanilla's duplicate check is a genuine linear
/// scan (`for (piVar7 = begin; piVar7 != end && *piVar7 != param_1; ...)`), independent of any
/// ordering. The insertion-sort-shaped code that follows a successful append (confirmed live via
/// `ZTAWARDMGR_ADD_AWARD_SAVE_LOAD` - `add_award([0, -1])` produces `[0, -1]`, not the ascending `[-1,
/// 0]` an initial ascending-binary-search implementation of this function produced) keeps the vector
/// sorted **descending** (largest first), not ascending - confirmed by hand-tracing
/// `ZTAwardMgr_addAward.c`'s inner comparisons (`*piVar4 < iVar11` / `iVar8 < iVar11`), which shift
/// smaller existing elements rightward to make room for a larger new one at the front.
fn insert_sorted_unique(ids: &mut Vec<i32>, id: i32) {
    if ids.contains(&id) {
        return;
    }
    let pos = ids.partition_point(|&x| x > id);
    ids.insert(pos, id);
}

pub fn add_award(id: i32) {
    let mut state = AWARD_MGR.lock().unwrap();
    insert_sorted_unique(&mut state.earned_ids, id);
}

/// Reimplementation of `OOAnalyzer::ZTAwardMgr::getAward`'s tree lookup, minus the raw
/// tree-node-pointer return value (see [`award_mgr_detours`]'s doc comment on why that can't be
/// reproduced, and why nothing needs it to be).
pub fn get_award(id: i32) -> Option<AwardData> {
    AWARD_MGR.lock().unwrap().awards.get(&id).cloned()
}

pub fn earned_ids() -> Vec<i32> {
    AWARD_MGR.lock().unwrap().earned_ids.clone()
}

pub fn earned_count() -> i32 {
    AWARD_MGR.lock().unwrap().earned_ids.len() as i32
}

/// Writes `value` as a single little-endian dword via whatever is installed at the vanilla
/// `WriteBytesToFile` address (`.hooked()` - the real CRT write normally, `io_redirect`'s in-memory
/// buffer inside a live-battery capture window), shared by [`save`]. Duplicated per-file rather than
/// shared, matching `ztthoughtmgr.rs`'s own precedent.
fn write_dword(file: *const u32, value: u32) -> bool {
    let bytes = value.to_le_bytes();
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, 4, 1, file as *const i8) }) == 1
}

/// Reads a single little-endian dword via whatever is installed at the vanilla read-primitive address
/// (`.hooked()` - see [`write_dword`]), shared by [`load`]. `None` on a short/failed read.
fn read_dword(file: *const u32) -> Option<u32> {
    let mut buf = 0u32;
    let ok = unsafe { DEALLOCATE.hooked()(&mut buf as *mut u32 as *const u32, 4, 1, file as *const u8) };
    (ok == 1).then_some(buf)
}

/// Reimplementation of `ZTAwardMgr::save`: writes the earned-id count, then every id, in that order.
/// Every element is attempted regardless of an earlier write failing (matching
/// `ZTAwardMgr_save.c`'s loop shape); the return value only reflects whether everything wrote
/// successfully.
pub fn save(file: *const u32) -> bool {
    let ids = AWARD_MGR.lock().unwrap().earned_ids.clone();
    let mut ok = write_dword(file, ids.len() as u32);
    for id in ids {
        ok &= write_dword(file, id as u32);
    }
    ok
}

/// Reimplementation of `ZTAwardMgr::load`: resets the earned-id vector, reads a count, then calls
/// [`add_award`] per entry - re-running the sorted-unique insert for every loaded id (not a raw push), so
/// a malformed/duplicate save can't reintroduce duplicates. Reproduces the exact on-disk wire format
/// (`i32` count + `i32[count]`) byte-for-byte.
pub fn load(file: *const u32) -> bool {
    AWARD_MGR.lock().unwrap().earned_ids.clear();
    let Some(count) = read_dword(file) else {
        return false;
    };
    let mut ok = true;
    for _ in 0..count {
        match read_dword(file) {
            Some(id) => add_award(id as i32),
            None => {
                // A corrupted/truncated stream fails every subsequent read identically - stop instead
                // of spinning through the rest of `count` (which can be a huge garbage value read off
                // a corrupted save).
                ok = false;
                break;
            }
        }
    }
    ok
}

/// Loads and parses a resource-relative `.cfg` path, matching `ztresearch.rs`'s
/// `research_config_reimplementation::read_cfg` precedent exactly (vanilla's real `;`-only comment
/// convention, not this codebase's own more lenient mod-loader parsing).
fn read_cfg(path: &str) -> Option<Ini> {
    let Some((_, data)) = get_file(path) else {
        error!("ztawardmgr: resource '{path}' not found");
        return None;
    };
    let text = decode_game_text(&data);
    let mut ini = Ini::new_cs();
    ini.set_comment_symbols(&[';']);
    match ini.read(text) {
        Ok(_) => Some(ini),
        Err(e) => {
            error!("ztawardmgr: failed to parse '{path}': {e}");
            None
        }
    }
}

/// All values for a repeated key, dropping any that trim to empty - see
/// `ztresearch.rs`'s `research_config_reimplementation::values` for why this (not `Ini::get_vec` directly)
/// matches `BFConfigFile::addKeyVal`'s real behavior.
fn values(ini: &Ini, section: &str, key: &str) -> Vec<String> {
    ini.get_vec(section, key).unwrap_or_default().into_iter().filter(|v| !v.trim().is_empty()).collect()
}

/// `BFConfigFile::getString`/`getInt` return the *first* value for a repeated key, unlike `Ini::get`'s
/// last-wins semantics - see `ztresearch.rs`'s `research_config_reimplementation::first`.
fn first(ini: &Ini, section: &str, key: &str) -> Option<String> {
    values(ini, section, key).into_iter().next()
}

fn first_parse<T: std::str::FromStr>(ini: &Ini, section: &str, key: &str) -> Option<T> {
    first(ini, section, key)?.parse().ok()
}

/// Reimplementation of `ZTAwardMgr::start`'s resource parse. `BFResource::find(this, "award", ".cfg")`
/// (per `ZTAwardMgr_start.c`) is a **multi-file** lookup, not a single fixed path - confirmed live: this
/// codebase's real `awards/` resource directory holds several distinct `award*.cfg` files
/// (`awards/awards.cfg`, `awards/award001.cfg`, `awards/award002.cfg`, `awards/award003.cfg`), and only
/// the three `award0NN.cfg` files carry the `id`/`nameID`/`tooltipID`/`icon`-bearing catalogue data,
/// gated behind a `[Version]` section - `awards/awards.cfg` itself is a different, unrelated small file
/// (an `[Awards]` section with a bare `award=`/`maxCurrent=` pair, no `[Version]` section at all), which
/// `ZTAwardMgr_start.c`'s own version-gate (`getInt(this, Version, version, &local_78)`, only proceeding
/// if `local_78 > 0`) correctly skips. Since `resource_manager::lazyresourcemap`'s `LAZY_RESOURCE_MAP` is
/// keyed by exact lowercased filename (collapsing same-named files across archives to one, but not
/// collapsing *distinct* filenames), every matching name is enumerated here directly via
/// `get_file_names()` and filtered by basename prefix `"award"`/suffix `".cfg"`, reproducing vanilla's
/// multi-file search without needing a dedicated prefix-search primitive in the resource layer.
///
/// For each matching file (in enumeration order - unlike a single unique-key resource lookup, there's no
/// natural "first/last wins" ordering across separate files, and none is needed: every real award id is
/// only ever defined once, in exactly one qualifying file), checks a `[Version]` section's `version` key
/// is present and positive (skipping the file otherwise - this is what correctly excludes
/// `awards/awards.cfg`, which has no `[Version]` section at all), then reads the repeated `award` key from
/// the `[Awards]` section. **Confirmed live against the real files** (not assumed from the decompile's own
/// symbolic-looking `Version`/`Awards` argument names, which turned out to name *sections*, not
/// `[default]`-section keys, and whose actual per-file key is the lowercase singular `award`/`version` -
/// see `ZTAWARDMGR_START` in `reimplementation_tests/mod.rs`). Each `award` value is itself a section name
/// (in practice just the id's own decimal digits, e.g. `13`) whose `id`/`nameID`/`tooltipID`/`icon` keys
/// are read via the 4-arg form. Only inserted when `id != 0 && nameID != 0` (matching
/// `ZTAwardMgr_start.c:65`); a later section with the same `id` overwrites an earlier one (the tree-insert
/// itself is unconditional overwrite-by-id, not first-wins).
pub fn start() -> bool {
    let candidates: Vec<String> = crate::resource_manager::lazyresourcemap::get_file_names()
        .into_iter()
        .filter(|name| {
            let base = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
            base.starts_with("award") && base.ends_with(".cfg")
        })
        .collect();

    let mut state = AWARD_MGR.lock().unwrap();
    let mut any = false;
    for path in candidates {
        let Some(ini) = read_cfg(&path) else { continue };
        let version: i32 = first_parse(&ini, "Version", "version").unwrap_or_default();
        if version <= 0 {
            continue;
        }
        for section in values(&ini, "Awards", "award") {
            let id: i32 = first_parse(&ini, &section, "id").unwrap_or_default();
            let name_id: i32 = first_parse(&ini, &section, "nameID").unwrap_or_default();
            if id != 0 && name_id != 0 {
                let tooltip_id = first_parse(&ini, &section, "tooltipID").unwrap_or_default();
                let icon = first(&ini, &section, "icon").unwrap_or_default();
                state.awards.insert(id, AwardData { name_id, tooltip_id, icon });
                any = true;
            }
        }
    }
    any
}

/// The `ZTGameMgr` field offsets `ZTScenarioSimpleGoal::eval`'s case `0xb` reads directly, per
/// `ZTScenarioSimpleGoal_eval.c:52-53` - both fall inside `ztgamemgr.rs`'s existing `pad9` gap, so no
/// struct changes are needed; their true meaning is unconfirmed and out of scope here.
const GOAL_KIND_OFFSET: u32 = 0xc;
const GOAL_SUBMETRIC_OFFSET: u32 = 0x10;
const GOAL_THRESHOLD_OFFSET: u32 = 0x1c;
const AWARD_COUNT_SUBMETRIC: i32 = 0xb;
const NUMERIC_STAT_GOAL_KIND: i32 = 1;

fn elapsed_metric(game_mgr_ptr: u32) -> i32 {
    let zoo_status = unsafe { ref_from_memory::<ZooStatus>((game_mgr_ptr + 0x10) as *const u32) };
    zoo_status.current_month_index + zoo_status.current_year_index * 12
}

/// Pure gate for `ZTScenarioSimpleGoal::eval`'s case `0xb` arm, isolated for testing without touching
/// live memory. Matches `ZTScenarioSimpleGoal_eval.c` lines 44-94: goal kind `1` (`mbr_0xc`), submetric
/// `0xb` (`mbr_0x10`), a live `GLOBAL_ZTGameMgr`, and the goal's threshold (`mbr_0x1c`) already reached.
fn should_report_award_count(kind: i32, submetric: i32, threshold: i32, game_mgr_ptr: u32, elapsed: i32) -> bool {
    kind == NUMERIC_STAT_GOAL_KIND && submetric == AWARD_COUNT_SUBMETRIC && game_mgr_ptr != 0 && threshold <= elapsed
}

/// Registers this module's live detours: `ZTAwardMgr`'s own methods, the `ZTScenarioSimpleGoal::eval`
/// partial override, and `_showAwards`.
pub fn init() {
    award_mgr_detours::init();
    eval_award_count_override::init();
    show_awards_detour::init();
}

/// Detours `ZTAwardMgr`'s five own methods onto the free functions above.
///
/// **`GET_AWARD`'s return value**: vanilla's real return is a raw pointer into its own tree-node memory,
/// which nothing in the Rust store has an equivalent for. The only confirmed external consumers are
/// `_showAwards` (reimplemented in [`show_awards_detour`], which calls [`get_award`] directly rather than
/// going through this detour's return value) and `ZTScenarioSimpleGoal::trigger06` (only checks
/// `!= 0`/`== 0`, per `ZTScenarioSimpleGoal_trigger06.c:13-14`, never dereferences it). So this detour
/// returns a fixed non-null sentinel (`1`) when found and `0` when not - **never dereference this return
/// value**.
mod award_mgr_detours {
    use openzt_detour::generated::ztawardmgr::{ADD_AWARD, GET_AWARD, LOAD, SAVE, START};
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(ADD_AWARD)]
        unsafe extern "thiscall" fn add_award(_this: *const u32, id: i32) {
            crate::ztawardmgr::add_award(id);
        }

        #[detour(GET_AWARD)]
        unsafe extern "thiscall" fn get_award(_this: *const u32, id: i32) -> i32 {
            if crate::ztawardmgr::get_award(id).is_some() {
                1
            } else {
                0
            }
        }

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(_this: *const u32, file: *const i8) -> u32 {
            crate::ztawardmgr::save(file as *const u32) as u32
        }

        /// `_version` is unused - `ZTAwardMgr_load.c`'s own body never reads its 3rd formal parameter -
        /// but `ZTWorldMgr_load.c`'s real call site genuinely pushes 3 arguments and `generated.rs`
        /// declares 3, so this must be kept in the signature purely to keep the thiscall stack-cleanup
        /// arithmetic correct.
        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(_this: *const u32, file: *const u32, _version: u32) -> u32 {
            crate::ztawardmgr::load(file) as u32
        }

        #[detour(START)]
        unsafe extern "thiscall" fn start(_this: *const u32) -> u32 {
            crate::ztawardmgr::start() as u32
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztawardmgr own-method detours: {e:?}");
        }
    }
}

/// Partial-override detour for `ZTScenarioSimpleGoal::eval`'s case `0xb` arm - the only other place
/// (besides `_showAwards`) that reads `ZTAwardMgr`'s raw fields directly. Everything else about this
/// large, mostly-unrelated switch is left as vanilla via `EVAL_DETOUR.call(this)` - see
/// `resource_manager/hooks.rs`'s `zoo_ui_general_get_info_image_name` for the same
/// match-one-condition/call-through-otherwise shape.
pub(crate) mod eval_award_count_override {
    use openzt_detour::generated::ztscenariosimplegoal::EVAL;
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(EVAL)]
        unsafe extern "thiscall" fn eval(this: *const u32) -> i32 {
            let kind = get_from_memory::<i32>(this as u32 + GOAL_KIND_OFFSET);
            let submetric = get_from_memory::<i32>(this as u32 + GOAL_SUBMETRIC_OFFSET);
            let threshold = get_from_memory::<i32>(this as u32 + GOAL_THRESHOLD_OFFSET);
            let game_mgr_ptr = globals().ztgamemgr_ptr() as u32;
            let elapsed = if game_mgr_ptr != 0 { elapsed_metric(game_mgr_ptr) } else { 0 };
            if should_report_award_count(kind, submetric, threshold, game_mgr_ptr, elapsed) {
                return crate::ztawardmgr::earned_count();
            }
            unsafe { EVAL_DETOUR.call(this) }
        }

        /// Exposes the real vanilla trampoline for the live comparison test. `EVAL.original()` can't be
        /// used for this once this module's own detour has patched `EVAL`'s address - `FunctionDef::
        /// original()` is a raw address cast in release (debug builds route it through openzt-detour's
        /// hook registry, see `openzt-detour/src/lib.rs`), so once hooked it silently re-enters this same
        /// detour instead of reaching real vanilla there. `EVAL_DETOUR.call(this)` (the `retour`
        /// trampoline this macro generates) is the way back to real vanilla behavior in every build.
        pub(super) fn call_real(this: *const u32) -> i32 {
            unsafe { EVAL_DETOUR.call(this) }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztscenariosimplegoal eval award-count override detour: {e:?}");
        }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn call_real(this: *const u32) -> i32 {
        detours::call_real(this)
    }
}

/// Reimplementation of the free function `_showAwards`, which iterates `ZTAwardMgr`'s earned-id vector
/// directly (bypassing any method call) to populate a `UIListBox`. `GLOBAL_BFUIMgr`'s RVA (`0x0023_8de0`)
/// is the same one `ztthoughtmgr.rs`'s `thought_ui_detours::global_bfuimgr` already uses.
pub(crate) mod show_awards_detour {
    use openzt_detour::generated::{
        bfuimgr::GET_ELEMENT_0,
        standalone::SHOW_AWARDS,
        uilistbox::{ADD_STRING_0, CLEAR},
    };
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;
    use crate::encoding_utils::encode_to_ansi;

    const AWARDS_LIST_ELEMENT_ID: i32 = 0x101c;
    const AWARD_LIST_ITEM_COLOR: u32 = 0x00ff_00ff;

    fn global_bfuimgr() -> *const u32 {
        (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32
    }

    /// Builds temporary buffers for `text`/`icon` and hands them to the real `UIListBox::addString`,
    /// mirroring `_showAwards.c`'s exact argument mapping: `icon` is passed as the raw `p2` slot (the
    /// award's value-payload `+0x1c` dword directly - **not** a `ZTBufferString` wrapper, unlike `text`),
    /// `p5` is `0xffffffff` (not null), and `color` is
    /// `((tooltip_id as u32) & 0xff00_0000) | AWARD_LIST_ITEM_COLOR` - implemented exactly this way
    /// (not pre-simplified to the bare literal) so a live comparison can confirm the top byte of a real
    /// `tooltip_id` is always `0` in practice, per this module's own doc comment on that open question.
    #[allow(clippy::manual_dangling_ptr)] // literal sentinel value `1`, not a real pointer
    fn add_award_to_list_box(list_box: *const u32, text: &str, icon: &str, tooltip_id: i32) {
        let mut encoded_text = encode_to_ansi(text);
        let text_len = encoded_text.len() as u32;
        encoded_text.push(0);
        let text_start = encoded_text.as_ptr() as u32;
        let text_buffer = ZTBufferString::from_raw_parts(text_start, text_start + text_len, text_start + encoded_text.len() as u32);

        let mut encoded_icon = encode_to_ansi(icon);
        encoded_icon.push(0);
        let icon_ptr = encoded_icon.as_ptr();

        let color = ((tooltip_id as u32) & 0xff00_0000) | AWARD_LIST_ITEM_COLOR;

        unsafe {
            ADD_STRING_0.original()(
                list_box,
                &text_buffer as *const ZTBufferString as *const u32,
                icon_ptr as *const i32,
                std::ptr::null(),
                std::ptr::null(),
                0xffffffffu32 as *const i32,
                0,
                1 as *const i32,
                color,
                tooltip_id as u32 as *const i32,
            );
        }
    }

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(SHOW_AWARDS)]
        unsafe extern "stdcall" fn show_awards() {
            let element = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), AWARDS_LIST_ELEMENT_ID) };
            if element.is_null() {
                return;
            }
            unsafe { CLEAR.original()(element) };
            for id in crate::ztawardmgr::earned_ids() {
                let Some(award) = crate::ztawardmgr::get_award(id) else { continue };
                let Some(text) = load_string_by_id(award.name_id() as u32) else { continue };
                add_award_to_list_box(element, &text, award.icon(), award.tooltip_id());
            }
        }

        /// Exposes the real vanilla trampoline for the live comparison test - see
        /// `eval_award_count_override::detours::call_real`'s doc comment for why `SHOW_AWARDS.original()`
        /// can't be used for this once this detour has patched the address.
        pub(super) fn call_real() {
            unsafe { SHOW_AWARDS_DETOUR.call() }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztawardmgr show_awards detour: {e:?}");
        }
    }

    #[cfg(feature = "reimplementation-tests")]
    pub(crate) fn call_real() {
        detours::call_real()
    }
}

/// Live-comparison test support for `reimplementation_tests`. Unlike `ZTThoughtMgr`, there's no
/// standalone-instance capability here - the constructor hardcodes the global address `0x006390e8` inside
/// its own body - so every test drives the one real live singleton via [`real_ptr`], resetting it between
/// cases rather than building/freeing a fresh instance.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// RVA of the real singleton `GLOBAL_ZTAwardMgr` embedded object (`0x006390e8` raw VA minus the
    /// `0x400000` preferred base).
    const GLOBAL_ZTAWARDMGR_RVA: u32 = 0x0023_90e8;

    pub(crate) fn real_ptr() -> *const u32 {
        (get_module_base("zoo.exe") as u32 + GLOBAL_ZTAWARDMGR_RVA) as *const u32
    }

    /// Resets the Rust-side reimplemented store to empty - test-only, since production code has no
    /// dedicated "clear" entry point (there's only ever one real singleton, never destroyed).
    pub(crate) fn reset_reimplemented_store() {
        let mut state = AWARD_MGR.lock().unwrap();
        state.earned_ids.clear();
        state.awards.clear();
    }

    /// The Rust-side catalogue as `(id, name_id, tooltip_id)` triples, in ascending-id order (`BTreeMap`
    /// iteration order) - matching [`read_vanilla_award_tree`]'s in-order walk, for direct comparison.
    pub(crate) fn reimplemented_award_triples() -> Vec<(i32, i32, i32)> {
        AWARD_MGR.lock().unwrap().awards.iter().map(|(&id, a)| (id, a.name_id, a.tooltip_id)).collect()
    }

    /// Read-only in-order walk of the real vanilla rb-tree rooted at the live singleton's `+0x0` header
    /// pointer, using the node layout confirmed in `private/docs/vtables/ZTAwardMgr.md` (`_Left`/`_Right`
    /// at `+0x8`/`+0xc`, key at `+0x10`, `nameID`/`tooltipID` at `+0x14`/`+0x18`). Never mutates or frees
    /// anything - safe regardless of which allocator built the nodes.
    pub(crate) fn read_vanilla_award_tree() -> Vec<(i32, i32, i32)> {
        let header = get_from_memory::<u32>(real_ptr() as u32);
        let root = get_from_memory::<u32>(header + 4);
        let mut result = Vec::new();
        walk_tree(root, &mut result);
        result
    }

    fn walk_tree(node: u32, out: &mut Vec<(i32, i32, i32)>) {
        if node == 0 {
            return;
        }
        walk_tree(get_from_memory::<u32>(node + 8), out);
        let key = get_from_memory::<i32>(node + 0x10);
        let name_id = get_from_memory::<i32>(node + 0x14);
        let tooltip_id = get_from_memory::<i32>(node + 0x18);
        out.push((key, name_id, tooltip_id));
        walk_tree(get_from_memory::<u32>(node + 0xc), out);
    }

    /// Read-only, direct-memory read of the real vanilla `vector<int>` at `+0xc`/`+0x10` (begin/end) -
    /// the earned-award-id set `addAward`/`save`/`load` maintain directly, per
    /// `private/docs/vtables/ZTAwardMgr.md`. Used by the real-zoo save/load round-trip test to see what
    /// real, undetoured vanilla `ZTAwardMgr::load` actually populated from the loaded zoo file - nothing
    /// in `AWARD_MGR` mirrors it, since this test build never installs `award_mgr_detours` (see
    /// `reimplementation_tests/mod.rs`'s own `init()` doc comment on why). Never mutates anything.
    pub(crate) fn read_vanilla_earned_ids() -> Vec<i32> {
        let this = real_ptr() as u32;
        let begin = get_from_memory::<u32>(this + 0xc);
        let end = get_from_memory::<u32>(this + 0x10);
        if begin == 0 || end <= begin {
            return Vec::new();
        }
        let count = ((end - begin) / 4) as usize;
        (0..count).map(|i| get_from_memory::<i32>(begin + (i * 4) as u32)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_sorted_unique_keeps_vector_descending_and_deduped() {
        let mut ids = Vec::new();
        for id in [5, 1, 3, 1, 5, 2] {
            insert_sorted_unique(&mut ids, id);
        }
        assert_eq!(ids, vec![5, 3, 2, 1]);
    }

    #[test]
    fn insert_sorted_unique_matches_live_observed_ordering() {
        // Confirmed live via ZTAWARDMGR_ADD_AWARD_SAVE_LOAD: adding [0, -1] in that order leaves the
        // real vanilla vector as [0, -1], not the ascending [-1, 0].
        let mut ids = Vec::new();
        insert_sorted_unique(&mut ids, 0);
        insert_sorted_unique(&mut ids, -1);
        assert_eq!(ids, vec![0, -1]);
    }

    #[test]
    fn should_report_award_count_requires_every_condition() {
        assert!(should_report_award_count(1, 0xb, 10, 0x1000, 10));
        assert!(should_report_award_count(1, 0xb, 10, 0x1000, 20));
        assert!(!should_report_award_count(0, 0xb, 10, 0x1000, 10), "wrong goal kind");
        assert!(!should_report_award_count(1, 0xa, 10, 0x1000, 10), "wrong submetric");
        assert!(!should_report_award_count(1, 0xb, 10, 0, 10), "null game mgr");
        assert!(!should_report_award_count(1, 0xb, 11, 0x1000, 10), "threshold not yet reached");
    }
}
