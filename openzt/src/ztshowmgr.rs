//! Stage 1 (struct + constructor + store) of the vanilla `ZTShowMgr` reimplementation - see
//! `openzt/plans/ztshowmgr-implementation-plan.md`. `ZTShowMgr` is the show-scheduling manager: it maps
//! show ids to the `ZTShowInfo` objects habitats register with it (`std::map<unsigned short,
//! ZTShowInfo*>` at `+0x28`), owns the config-loaded trick/show satisfaction thresholds
//! `ZTShow::doTrickEvent`/`calculatePercentAdjustment` read off the live global, and embeds a real
//! `ZTShowScriptMgr` inline at `+0x34` (four of its own methods are pure one-line delegations to that
//! sub-object; `ztshowscriptmgr.rs` already detours the sub-object's own addresses, so those wrappers
//! stay real vanilla code - see the plan's "Composition" section).
//!
//! **Style 2 - fully independent Rust store** (CLAUDE.md's decision criteria): a full decompile-corpus
//! grep (58 files referencing `GLOBAL_ZTShowMgr` outside the class's own, plan appendix) found every
//! external caller reaching the registered-shows map through named accessor methods
//! (`getShowInfo`/`getScript`/`getScriptID`/`registerShow`/`unregisterShow`) and never reading
//! `+0x28`/`+0x2c`/`+0x30` raw - so once those accessors are ported (stages 3+), vanilla's own map
//! becomes safely-inert dead weight that no Rust code below ever reads or writes. That choice also
//! sidesteps this class's one genuine allocator hazard outright: the map's `0x18`-byte nodes come from
//! the shared small-object freelist at `DAT_00638008` (`FUN_00402f85`/`FUN_00405002`/`FUN_00401b16`,
//! none of which has a `generated.rs` entry), a size-class freelist plausibly shared with unrelated
//! 24-byte allocations elsewhere in the game - allocating into it from Rust without the matching free
//! helpers would be exactly CLAUDE.md's cross-allocator corruption class.
//!
//! **The constructor is deliberately never detoured.** Vanilla's own ctor must keep running for the live
//! global (nothing else constructs `GLOBAL_ZTShowMgr`), and under style 2 its real work - taking a
//! freelist node for the map header and running the real config-driven `initShowParams` - is exactly
//! what the live global should keep doing. [`ZTShowMgr::construct`] below is therefore not a detour
//! target: it reproduces the constructor's config-independent writes so standalone instances can be
//! built and compared against the real ctor in the live battery (`ZTSHOWMGR_STANDALONE_ROUNDTRIP`),
//! mirroring `ztsoundscape.rs`'s own `construct`/roundtrip precedent. The one part it deliberately does
//! not reproduce is the freelist allocation itself: a standalone instance's `tree_header` stays null
//! (nothing reimplemented ever reads it - see the style-2 note above), so the test compares the real
//! side's node *shape* but excludes the pointer field itself.
//!
//! **Stage 2** adds the constructor's config-driven half: [`ZTShowMgr::init_show_params`] (the eight
//! `shows.cfg` threshold lookups, gated on expansion pack 2 through real
//! `BFApp::getInstalledExpansion`) detoured via `INIT_SHOW_PARAMS`, so the real, un-detoured
//! constructor's tail-call into it now runs the Rust port. Since the port writes the same defaults
//! vanilla writes and overrides from the same config, the live constructor's end state is unchanged.
//!
//! **Stage 3** makes [`SHOW_STORE`] a *shadow/mirror* of vanilla's tree rather than a replacement
//! (`ztshowmgr-implementation-plan.md`'s "Staging decision" - the read path and write path have to
//! agree on where the data lives, so the first-detoured side must mirror until the read cutover):
//! `registerShow`/`unregisterShow` are detoured, but each detour calls the real vanilla body through
//! its `<NAME>_DETOUR.call` trampoline first - vanilla keeps owning the real tree at `+0x28`, the
//! `DAT_0063e480` id counter, the `field_0x70` id writes, and `clearShowScriptStates` - and only on
//! the real body's success byte mirrors the outcome into [`SHOW_STORE`]. Live-game mutation behavior
//! is therefore exactly vanilla, while the store is a live-verified copy of the real tree. (Stage
//! 9 below has since retired the mirroring itself - the shadow phase's job was to prove the store
//! in sync before anything read it, which five live-green stages on top of the read cutover did.)
//!
//! **Stage 4** flips the read path onto the store: `getShowInfo`/`getScriptID`
//! ([`ZTShowMgr::get_show_info`]/[`ZTShowMgr::get_script_id`]) are detoured and answer every caller -
//! Rust and un-decompiled vanilla alike - out of [`SHOW_STORE`]. Vanilla's own tree is thereby on its
//! way to inert: every external caller the corpus grep found goes through these two addresses
//! (`unregisterShow`'s own internal clear-target lookup included), so the readers always see the
//! in-sync mirror. From here on `GET_SHOW_INFO.original()`/`GET_SCRIPT_ID.original()` are
//! release-profile re-entry hazards like every other hooked address: in-repo call sites that want the
//! live behavior use `.hooked()` (`ztshow.rs`'s `start`, `ztshowui.rs`'s `show_info_for_habitat`), and
//! the battery's real-side poles go through the [`detours`] trampolines. Lock ordering stays safe: the
//! writers take the store's mutex only *after* their trampoline returns, and the readers'
//! mutex critical sections copy-and-unlock without calling out - even though vanilla bodies reached
//! through those trampolines now *do* re-enter the hooked `GET_SHOW_INFO` address internally. The
//! plan's open item - when (and whether) the writers drop their call-through to become full style-2
//! ports - is resolved by stage 9 below; until then the dual-write state was behavior-safe
//! indefinitely while the mirror stayed in sync, which the battery's cross-store oracles asserted
//! after every mutation.
//!
//! **Stage 5** ports the two whole-map walks, `enterNewMonth`/`update`
//! ([`ZTShowMgr::enter_new_month`]/[`ZTShowMgr::update`]), onto the store - the first consumers
//! that iterate *everything* rather than probing one id. They stayed real vanilla through the
//! shadow phase precisely because they walked vanilla's still-authoritative tree; post-cutover the
//! `BTreeMap`'s ascending-`u16` order *is* vanilla's in-order sequence, so both walks become plain
//! iterations calling the same real, untouched `ZTShowInfo` callees vanilla calls (directly for
//! `enterNewMonth`, through the show's own vtable slot `+0x20` for `update`). Neither walk holds
//! [`SHOW_STORE`]'s lock across a callee call: the callees can re-enter the detoured
//! reader/writer addresses (a show's own update can start/stop shows), and the lock is only ever
//! taken to copy the value list out.
//!
//! **Stage 6** ports the save/load pair ([`ZTShowMgr::save`]/[`ZTShowMgr::load`]) - vanilla's thin
//! wrapper around two pieces: the embedded `ZTShowScriptMgr`'s own save/load (already
//! `ztshowscriptmgr.rs`'s Rust store, reached through the same direct `CALL` vanilla makes) and the
//! 2-byte show-id-counter persistence. Those ports landed reading/writing the `DAT_0063e480`
//! global in place because the stage-3 writers still kept their vanilla call-through (the counter
//! was then vanilla-owned); stage 9 has since moved the counter into [`SHOW_STORE`] and repointed
//! both ports at it, making the persisted counter single-owned end to end.
//!
//! **Stage 7** ports `isDoingShow` ([`ZTShowMgr::is_doing_show`]) - the "is this unit performing in
//! that show" probe `ZTUnit::isDoingShow` (`0x00437402`, the corpus's one caller) reaches through.
//! It introduces no new state: it composes the two halves earlier stages put in place, the
//! store-backed `getShowInfo` lookup (stage 4) and a lookup over the found show's embedded
//! `ZTShow`'s script-state map - the same callee `ztshow.rs`'s `do_current_item` already drives.
//! A later review pass ([`crate::ztshow::get_show_script_state`]) replaced that lookup's real,
//! `.hooked()`-routed vanilla call with a pure Rust reader of the same read-only tree - see that
//! function's own doc comment.
//!
//! **Stage 8** ports `isShowScriptDone` ([`ZTShowMgr::is_show_script_done`]) - the last method in
//! the plan's inventory (`ztshowmgr-implementation-plan.md` stage 8), unblocked once a Ghidra regen
//! added its `generated.rs` entry and Windows decompile. Structurally a sibling of stage 7 - same
//! store-backed lookup, same script-state map lookup - differing only in what it does with the
//! found script state: the state pointer is dereferenced for the "done" byte at `+0x13`, which is
//! returned raw. That completed the plan's method inventory.
//!
//! **Stage 9** finishes the style-2 end state: [`ZTShowMgr::register_show`]/[`ZTShowMgr::
//! unregister_show`] drop their `<NAME>_DETOUR.call` call-throughs and become full ports of the
//! real bodies (`ZTShowMgr_registerShow.asm`/`ZTShowMgr_unregisterShow.asm`), and the
//! `DAT_0063e480` show-id counter moves into [`SHOW_STORE`] as a `u16` field (the real increment
//! is a *word* `INC`; the assigned id is `(u16)counter % 0xffff`, so `0xffff` is never assigned
//! and the counter's `0xffff` and wrapped-to-`0` states both yield id `0`). The counter is seeded
//! once from the global at detour-install time ([`init`]); after that the global is inert exactly
//! like vanilla's `+0x28` tree - its three consumers (verified against the full decompile corpus:
//! `registerShow`'s increment, `save`'s write, `load`'s read) are all this module's detoured
//! addresses now, and stage 6's save/load were repointed at the store's field in the same change
//! so the persisted counter never has two owners at any buildable midpoint. The fresh-id write
//! goes through a Rust reimplementation of `ZTShowInfo::setShowInfoID` - not a plain `field_0x70`
//! store, because the real setter also keeps the embedded `ZTShow`'s `+0x10` back-pointer and
//! `+0x6` id copy in sync. Behavior in-game is unchanged by any of this; the stage exists to
//! retire the permanent store-tree sync obligation of the shadow phase.
//!
//! **Stage 10** (`ztshowinfo-implementation-plan.md`) moves that reimplementation into
//! [`crate::ztshowinfo::set_show_info_id`] and detours the real `setShowInfoID` address too, so
//! [`ZTShowMgr::register_show`] now calls the shared function directly rather than keeping its
//! own private copy.

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};

use crate::bfconfigfile;
use crate::globals::get_module_base;
use crate::util::{get_from_memory, mut_from_memory};
use openzt_detour::generated::bfapp::GET_INSTALLED_EXPANSION;
use openzt_detour::generated::standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE};
use openzt_detour::generated::ztshow::CLEAR_SHOW_SCRIPT_STATES;
use openzt_detour::generated::ztshowmgr::{
    ENTER_NEW_MONTH, GET_SCRIPT_ID, GET_SHOW_INFO, INIT_SHOW_PARAMS, IS_DOING_SHOW, IS_SHOW_SCRIPT_DONE, LOAD, REGISTER_SHOW, SAVE,
    UNREGISTER_SHOW, UPDATE,
};
use openzt_detour::generated::ztshowscriptmgr::{LOAD as ZTSHOWSCRIPTMGR_LOAD, SAVE as ZTSHOWSCRIPTMGR_SAVE};
use openzt_detour_macro::detour_mod;
use tracing::error;

/// `GLOBAL_ZTApp`'s RVA - a raw pointer-typed global (one dereference gives the live `ZTApp*`
/// singleton), read by [`ZTShowMgr::init_show_params`] as the `this` for the expansion-2 gate call.
/// Re-declared per-file after `ztgamemgr.rs`'s own private `GLOBAL_ZTAPP_RVA` (same value, same
/// one-level-of-indirection shape) per the repo's no-shared-consts convention.
const GLOBAL_ZTAPP_RVA: u32 = 0x00638154 - 0x400000;

/// `DAT_0063e480` - the vanilla show-id counter global behind `registerShow`'s inlined `makeID`
/// logic (16-bit `INC`, assigned id = `(u16)counter % 0xffff`, so `0xffff` is never assigned).
/// Stage 9 moved the counter into [`SHOW_STORE`], so this global is inert exactly like vanilla's
/// `+0x28` tree: it is read exactly once, by [`init`]'s one-time seed, and afterwards only by the
/// battery's real-pole save/load seeding ([`live_support::show_id_counter_addr`]) - the vanilla
/// save/load bodies reached through those trampolines still read and write this address in place.
const SHOW_ID_COUNTER_RVA: u32 = 0x0063e480 - 0x400000;

/// `ZTShowMgr`'s real vtable VA (`private/docs/vtables/ZTShowMgr.md` - 2 slots: destructor, `update`),
/// written at `+0x0` by both real constructor paths (the transient `&BFMgr_vftable` placeholder the ctor
/// writes first is always overwritten before it returns, so only the final value is reproduced here).
/// Stored as `get_module_base + RVA` per CLAUDE.md's data-address rule.
const ZTSHOWMGR_VTABLE_RVA: u32 = 0x00635120 - 0x400000;

/// The embedded `ZTShowScriptMgr`'s own vtable VA (`ZTShowScriptMgr_ZTShowScriptMgr.c` names it
/// `ZTShowScriptMgr__vtable_00630d28`; the `.asm` names the same address `vf_returnTrue` - a 1-slot
/// vtable whose only entry returns true). Same data-address handling as [`ZTSHOWMGR_VTABLE_RVA`].
const ZTSHOWSCRIPTMGR_VTABLE_RVA: u32 = 0x00630d28 - 0x400000;

/// The `0x10`-byte `ZTShowScriptMgr` sub-object embedded at `ZTShowMgr+0x34` (its complete real size -
/// `CreateZTShowMgr` allocates the whole `ZTShowMgr` as `operator_new(0x44)`, leaving exactly `0x10`
/// bytes past `+0x34`). This mirrors its vanilla layout for structural fidelity only: the live
/// reimplementation of this sub-object is `ztshowscriptmgr.rs`'s fully independent Rust store, which
/// never reads or writes these bytes - real vanilla code owns them.
#[repr(C)]
pub struct ZTShowScriptMgrSlot {
    /// `+0x34` - `ZTShowScriptMgr`'s vtable (`ZTSHOWSCRIPTMGR_VTABLE_RVA`).
    pub vtable: u32,
    /// `+0x38` - the sub-object's own `std::map` header/nil-node pointer, a `0x18`-byte node taken from
    /// the same shared `DAT_00638008` freelist (confirmed via `ZTShowScriptMgr_ZTShowScriptMgr.asm`).
    /// Standalone constructions leave this null - see [`ZTShowMgr::construct`]'s doc comment.
    pub tree_header: u32,
    /// `+0x3c` - the sub-object's map size (zeroed by the real ctor).
    pub tree_size: u32,
    /// `+0x40` - see [`ZTShowMgr::tag_byte`] for the (odd) value the real ctor writes here.
    pub tag_byte: u8,
    _pad_0x41: [u8; 3],
}

const _: () = assert!(std::mem::size_of::<ZTShowScriptMgrSlot>() == 0x10);

/// Full, `#[repr(C)]` mirror of vanilla `ZTShowMgr` - real size `0x44` (`CreateZTShowMgr.c`'s own
/// `operator_new(0x44)`), size-asserted. Only ever *read* by the reimplementations that need the live
/// global's threshold fields (`ztshow.rs`'s `do_trick_event`); the Rust store below never reads or
/// writes any of it. The two `tag_byte` fields reproduce, byte-faithfully but unexplained, what both
/// real constructors write there: the **high byte of the instance's own address** (the `.asm` reads it
/// off the stack as `[ESP+0xb]`/`[ESP+0x7]` - the saved `ECX`, i.e. `this` - in both
/// `ZTShowMgr_ZTShowMgr.asm` and `ZTShowScriptMgr_ZTShowScriptMgr.asm`; semantics unknown).
#[repr(C)]
pub struct ZTShowMgr {
    /// `+0x0` - `ZTShowMgr`'s vtable.
    pub vtable: u32,
    /// `+0x4` - zeroed by the real ctor; semantics unknown.
    pub field_0x4: u8,
    _pad_0x5: [u8; 3],
    /// `+0x8` - bad-trick satisfaction threshold (`initShowParams`: `[trickSatisfactionThresholds]
    /// badTrick`, default `0`). Read as `threshold_a` by `ztshow.rs`'s `do_trick_event`.
    pub threshold_a: u32,
    /// `+0xc` - good-trick satisfaction threshold (`goodTrick`, default `3`). Read as `threshold_b` by
    /// `do_trick_event`.
    pub threshold_b: u32,
    /// `+0x10` - great-trick satisfaction threshold (`greatTrick`, default `6`). Read as `threshold_c`
    /// by `do_trick_event`.
    pub threshold_c: u32,
    /// `+0x14` - bad-show satisfaction threshold (`[showSatisfactionThresholds] badShow`, default `0x19`).
    pub bad_show: u32,
    /// `+0x18` - good-show satisfaction threshold (`goodShow`, default `0x32`).
    pub good_show: u32,
    /// `+0x1c` - great-show satisfaction threshold (`greatShow`, default `0x4b`).
    pub great_show: u32,
    /// `+0x20` - minimum ideal show length (`[show] minIdealLength`, default `6`). Both ideal-length
    /// fields bound `ZTShow::calculatePercentAdjustment`'s trick-count adjustment (read raw at these
    /// offsets by `ztshow.rs`).
    pub min_ideal_length: u32,
    /// `+0x24` - maximum ideal show length (`maxIdealLength`, default `6`).
    pub max_ideal_length: u32,
    /// `+0x28` - the registered-shows `std::map`'s header/nil-node pointer (a `0x18`-byte node from the
    /// shared `DAT_00638008` freelist; self-referential when empty). The Rust store below replaces every
    /// consumer of this tree - standalone constructions leave the field null.
    pub tree_header: u32,
    /// `+0x2c` - registered-shows map size (the destructor's and `update`'s own empty-check field).
    pub tree_size: u32,
    /// `+0x30` - high byte of this instance's own address, per the real ctor's stack read. Semantics
    /// unknown; reproduced faithfully, never read by anything reimplemented.
    pub tag_byte: u8,
    _pad_0x31: [u8; 3],
    /// `+0x34` - the embedded, real-vanilla-owned `ZTShowScriptMgr` sub-object.
    pub show_script_mgr: ZTShowScriptMgrSlot,
}

const _: () = assert!(std::mem::size_of::<ZTShowMgr>() == 0x44);

impl ZTShowMgr {
    /// Reimplementation of `ZTShowMgr::ZTShowMgr`'s config-independent parts, per
    /// `ZTShowMgr_ZTShowMgr.c`/`.asm` (field writes verified against the `.asm`, not just the decompiled
    /// `.c`): zeroing `+0x4`, the eight `initShowParams` defaults (which the real `initShowParams`
    /// writes *before* its expansion-gated `BFConfigFile` override - stage 2 ports that override), both
    /// map sizes, both vtable writes, and both `tag_byte` self-address writes.
    ///
    /// Deliberately **not** reproduced: the freelist allocations backing `+0x28`/`+0x38` (this instance's
    /// map headers stay null - no reimplemented code ever reads them, and allocating a lookalike node
    /// without the matching freelist free helpers would set up the cross-allocator hazard the style-2
    /// decision exists to avoid). Callers get a fully zeroed struct first (the real ctor runs over
    /// un-zeroed `operator_new` memory and simply leaves its own padding untouched; zeroing the padding
    /// here is the deterministic equivalent for a test instance, matching the roundtrip test's
    /// pre-zeroed-buffer precedent). Not a detour target - see the module doc comment.
    pub fn construct(&mut self) -> &mut Self {
        let base = get_module_base("zoo.exe") as u32;
        let this_addr = self as *const ZTShowMgr as u32;

        self.vtable = base + ZTSHOWMGR_VTABLE_RVA;
        self.field_0x4 = 0;
        self.threshold_a = 0;
        self.threshold_b = 3;
        self.threshold_c = 6;
        self.bad_show = 0x19;
        self.good_show = 0x32;
        self.great_show = 0x4b;
        self.min_ideal_length = 6;
        self.max_ideal_length = 6;
        self.tree_header = 0;
        self.tree_size = 0;
        self.tag_byte = (this_addr >> 24) as u8;

        self.show_script_mgr.vtable = base + ZTSHOWSCRIPTMGR_VTABLE_RVA;
        self.show_script_mgr.tree_header = 0;
        self.show_script_mgr.tree_size = 0;
        self.show_script_mgr.tag_byte = (this_addr >> 24) as u8;

        self
    }

    /// Reimplementation of `ZTShowMgr::initShowParams` (`0x0051f59b`), per
    /// `ZTShowMgr_initShowParams.asm`:
    ///
    /// 1. Write the eight config defaults (same values [`ZTShowMgr::construct`] writes - vanilla
    ///    writes them here too, so the defaults survive untouched whenever the config half is
    ///    skipped below).
    /// 2. Gate on "expansion pack 2 installed" - ported as a call-through to the real
    ///    `BFApp::getInstalledExpansion` (`GET_INSTALLED_EXPANSION`, `0x004ab32c`) on the live
    ///    `GLOBAL_ZTApp` singleton, per the plan's finding that the Windows `.asm`'s inlined
    ///    `word ptr [app + 0x444] & 2` read is exactly that method's own body. Real vanilla reads
    ///    `GLOBAL_ZTApp` with a "if null, lazily assign it a bogus `ZTApp::handleMessages`
    ///    function-pointer sentinel before re-reading" defensive branch first - deliberately not
    ///    reproduced, same precedent as `ztgamemgr.rs`'s `stop` (a null app global here means
    ///    "app not ready"; this port treats that as "skip the config override" rather than writing
    ///    a code address into live global state to match a never-taken vanilla path).
    /// 3. If gated in: load `shows.cfg` through `bfconfigfile::ini_compat` (see that module's doc
    ///    comment) instead of a real vanilla `BFConfigFile` - this scratch config is never shared
    ///    with any other code (no other reimplemented or un-ported caller reads it), so nothing
    ///    requires it to be real vanilla memory. Each of the eight keys, read in vanilla's exact
    ///    section/key naming (`ZTShowMgr_initShowParams.asm`'s own `PUSH s_*` literals -
    ///    `[trickSatisfactionThresholds] badTrick`/`goodTrick`/`greatTrick`,
    ///    `[show] minIdealLength`/`maxIdealLength`,
    ///    `[showSatisfactionThresholds] badShow`/`goodShow`/`greatShow`), only overwrites its field
    ///    when present and parseable - matching real `BFConfigFile::getInt`'s "leave the caller's
    ///    existing value untouched on a missing/bad key" behavior - and a missing/unparseable
    ///    `shows.cfg` itself leaves every field at its step-1 default, matching vanilla's own
    ///    "skip every lookup when the file didn't load" gate.
    ///
    /// This also retires the local-scratch-`BFConfigFile` freelist-node-return dance the real
    /// vanilla-call version of this method needed (see `git blame` for that prior shape, or
    /// `ambients.rs`'s `Ambients::construct` for the still-live version of the same pattern) - there
    /// is no vanilla allocation here to hand back.
    ///
    /// Returns `1` - the real body's only return write is `MOV AL, 0x1` (upper EAX bits are leftover
    /// register garbage there; this port returns a clean `1`, which no caller distinguishes - its one
    /// caller, the constructor, ignores the return).
    pub fn init_show_params(&mut self) -> u32 {
        self.threshold_a = 0;
        self.threshold_b = 3;
        self.threshold_c = 6;
        self.bad_show = 0x19;
        self.good_show = 0x32;
        self.great_show = 0x4b;
        self.min_ideal_length = 6;
        self.max_ideal_length = 6;

        let base = get_module_base("zoo.exe") as u32;
        let ztapp_ptr: u32 = get_from_memory(base + GLOBAL_ZTAPP_RVA);
        let expansion_2_installed = ztapp_ptr != 0 && unsafe { GET_INSTALLED_EXPANSION.original()(ztapp_ptr as *const u32, 2) };

        if expansion_2_installed
            && let Some(ini) = bfconfigfile::ini_compat::read_cfg("shows.cfg")
        {
            use bfconfigfile::ini_compat::first_parse;
            self.threshold_a = first_parse(&ini, "trickSatisfactionThresholds", "badTrick").unwrap_or(self.threshold_a);
            self.threshold_b = first_parse(&ini, "trickSatisfactionThresholds", "goodTrick").unwrap_or(self.threshold_b);
            self.threshold_c = first_parse(&ini, "trickSatisfactionThresholds", "greatTrick").unwrap_or(self.threshold_c);
            self.min_ideal_length = first_parse(&ini, "show", "minIdealLength").unwrap_or(self.min_ideal_length);
            self.max_ideal_length = first_parse(&ini, "show", "maxIdealLength").unwrap_or(self.max_ideal_length);
            self.bad_show = first_parse(&ini, "showSatisfactionThresholds", "badShow").unwrap_or(self.bad_show);
            self.good_show = first_parse(&ini, "showSatisfactionThresholds", "goodShow").unwrap_or(self.good_show);
            self.great_show = first_parse(&ini, "showSatisfactionThresholds", "greatShow").unwrap_or(self.great_show);
        }

        1
    }

    /// Stage 9 full port of `ZTShowMgr::registerShow` (`0x005abb26`, per
    /// `ZTShowMgr_registerShow.asm`; `ztshowmgr-implementation-plan.md` stage 9): the style-2 end
    /// state of the stage-3 shadow/mirror - the `<NAME>_DETOUR.call` call-through is gone, and
    /// everything the real body owned (the tree insert, the `DAT_0063e480` counter, the
    /// `field_0x70` write) is Rust-owned now.
    ///
    /// Vanilla shape, `.asm`-verified: a null `show` returns `AL=0` before touching anything;
    /// otherwise a find on the show's *current* `field_0x70` runs first (already present ->
    /// `AL=0`, nothing written, even with the force flag set - and the probe uses the current
    /// value *even when it is 0*, since id 0 can legitimately sit in the map after counter wrap);
    /// only then does the force flag **or** `field_0x70 == 0` assign a fresh id (`INC word ptr
    /// DAT_0063e480`, id = `(u16)counter % 0xffff` - so `0xffff` is never assigned, and the
    /// counter's `0xffff` and wrapped-to-`0` states both yield id `0`) written through the real
    /// `ZTShowInfo::setShowInfoID`, reimplemented as [`crate::ztshowinfo::set_show_info_id`]; the
    /// final tree write is insert-or-assign, so a force-assigned id colliding with a registered id
    /// steals that slot in place, which `BTreeMap::insert` reproduces exactly. The real body
    /// reports success in `AL` only (upper EAX bits are leftover register garbage there, its
    /// `MOV %AL,%BL` exit); this returns a clean `1`, which every caller observes through the
    /// low byte alone. The real body's setter-return insert gate (`TEST %BL,%BL; JZ`) is not
    /// reproduced separately: the guard that can return `0` in `ZTShowInfo_setShowInfoID.asm`
    /// compares the field the setter just wrote, so it is unreachable through this call (the
    /// plan's implementation-time verification) and the setter helper has no failure path.
    pub fn register_show(&mut self, show: *const u32, force: bool) -> u32 {
        if show.is_null() {
            return 0;
        }
        let show_addr = show as u32;
        let current_id = get_from_memory::<u16>(show_addr + 0x70);
        let fresh_id = {
            let mut store = SHOW_STORE.lock().unwrap();
            if store.registered_shows.contains_key(&current_id) {
                return 0;
            }
            if force || current_id == 0 {
                store.show_id_counter = store.show_id_counter.wrapping_add(1);
                Some(store.show_id_counter % 0xffff)
            } else {
                None
            }
        };
        let id = match fresh_id {
            Some(id) => {
                crate::ztshowinfo::set_show_info_id(show_addr, id);
                id
            }
            None => current_id,
        };
        SHOW_STORE.lock().unwrap().registered_shows.insert(id, show_addr);
        1
    }

    /// Stage 9 full port of `ZTShowMgr::unregisterShow` (`0x005aaa95`, per
    /// `ZTShowMgr_unregisterShow.asm`/`.c`): the style-2 end state of the stage-3 shadow/mirror -
    /// vanilla's tree erase is now [`BTreeMap::remove`], and the clear-target lookup the real body
    /// made through `GLOBAL_ZTShowMgr` comes straight off [`SHOW_STORE`] (coherent: that internal
    /// `getShowInfo` call has resolved through the stage-4 detoured reader since the cutover).
    ///
    /// Vanilla's path matrix, all verified against the `.c`/`.asm`: null `show` + null `id`
    /// returns `AL=0`; `id == 0` with a show pointer derives the id from the show's own
    /// `field_0x70` - deliberately *stale* after a prior unregister, since vanilla never zeroes
    /// that field - and, with the clear flag set, runs the real `clearShowScriptStates` on that
    /// show's embedded `ZTShow` directly (no lookup); both `id != 0` paths (with or without a
    /// show pointer) pick the clear target up from the store and only clear on a hit. The real
    /// body's discarded-result wart in the show-pointer + id path (it calls
    /// `getShowInfo(this, id)` and throws the answer away) simply disappears in the port - it was
    /// unobservable; noted so the omission isn't mistaken for infidelity. Absent-key remove is a
    /// silent success, like vanilla's erase-by-key; the return is a clean `1` whenever it
    /// proceeds (the real body's `MOV %AL, 0x1`).
    pub fn unregister_show(&mut self, id: u16, show: *const u32, clear: bool) -> u32 {
        if show.is_null() && id == 0 {
            return 0;
        }
        let effective_id = if id != 0 {
            id
        } else {
            // `id == 0` excludes the null-show early return above, so `show` is non-null here and
            // vanilla's own derivation applies.
            get_from_memory::<u16>(show as u32 + 0x70)
        };
        if clear {
            // The id-0 path clears the passed show itself; the id paths look the target up - the
            // real body through `GLOBAL_ZTShowMgr`'s `getShowInfo` (which has answered from this
            // store since the stage-4 cutover), the port straight off the map.
            let target = if id != 0 { Self::get_show_info(id) } else { show as u32 };
            if target != 0 {
                unsafe { CLEAR_SHOW_SCRIPT_STATES.hooked()((target + 0x4) as *const u32) };
            }
        }
        SHOW_STORE.lock().unwrap().registered_shows.remove(&effective_id);
        1
    }

    /// Stage 4 read cutover of `ZTShowMgr::getShowInfo` (`0x0041ebfd`, per
    /// `ZTShowMgr_getShowInfo.asm`): the registered-shows lookup, answered out of [`SHOW_STORE`]
    /// instead of vanilla's `+0x28` tree. Vanilla shape, `.asm`-verified: a standard MSVC lower-bound
    /// walk over the u16-keyed tree returning the found node's `+0x14` value, clean `0` on a miss.
    /// A registered-but-null value is indistinguishable from a miss in vanilla; the store can never
    /// hold a null value (only a real body's success byte inserts), so
    /// `get().copied().unwrap_or(0)` reproduces the mapping exactly - unsigned `u16` key order
    /// matches vanilla's unsigned compare, which is what makes `BTreeMap<u16>` a drop-in.
    ///
    /// `this` is deliberately unread - one real instance, one process-global store (see
    /// [`SHOW_STORE`]). Where vanilla's own body faults on a null `this` (unguarded `[ECX+0x28]`
    /// read), this reader has no `this` to fault on, which is also what keeps `unregisterShow`'s
    /// global-targeted internal lookups safe against the battery's standalone instances.
    pub fn get_show_info(id: u16) -> u32 {
        SHOW_STORE.lock().unwrap().registered_shows.get(&id).copied().unwrap_or(0)
    }

    /// Stage 4 read cutover of `ZTShowMgr::getScriptID` (`0x005a2665`, per
    /// `ZTShowMgr_getScriptID.asm`): [`ZTShowMgr::get_show_info`] plus the found `ZTShowInfo`'s own
    /// assigned-script-id field at `+0x8` (the same field `ztshow.rs`'s `check_unit_type` reads).
    /// The real body's found path (`MOV %AX, word ptr [EAX+0x8]`, no `movzx`) leaves EAX's upper
    /// half holding the upper half of the `getShowInfo` return - live-verified in
    /// `ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID` - which is unobservable by every caller (its only
    /// chain, `ZTUnit::getShowItem` -> `ZTUnitType::getShowItem` -> `ZTShowMgr::getScript`, ends in
    /// `ZTShowScriptMgr::getScript`, which masks the id back to 16 bits before its own tree walk),
    /// so this returns the u16 cleanly zero-extended. `0` when the show isn't registered -
    /// indistinguishable there from a found show whose `+0x8` is `0`, exactly like vanilla.
    pub fn get_script_id(id: u16) -> u32 {
        let show_info = Self::get_show_info(id);
        if show_info != 0 {
            get_from_memory::<u16>(show_info + 0x8) as u32
        } else {
            0
        }
    }

    /// Stage 5 port of `ZTShowMgr::enterNewMonth` (`0x004842a2`, per `ZTShowMgr_enterNewMonth.asm`/
    /// `.c`): the monthly tick (`ZooStatus::financeChecks` is the corpus's one caller) running
    /// `ZTShowInfo::enterNewMonth` (`0x0048b57e`) on every registered, non-null show. Vanilla
    /// traverses its `+0x28` tree with the standard `_Tree::_Inc` successor walk starting at the
    /// leftmost node; iterating [`SHOW_STORE`]'s `BTreeMap` yields that same in-order sequence
    /// (ascending unsigned `u16` key - the property that made it a stage-4 drop-in), with vanilla's
    /// `left == header` empty case covered by the empty vector.
    ///
    /// **Stage 9 of `ztshowinfo-implementation-plan.md` ported `ZTShowInfo::enterNewMonth`
    /// ([`crate::ztshowinfo::enter_new_month`]) - this now calls it directly** rather than through
    /// `ztshowinfo::ENTER_NEW_MONTH.hooked()`: a same-module Rust call is both simpler and, unlike a
    /// raw address cast, immune to the release-profile re-entry hazard `.hooked()`/`.original()`
    /// have to route around for an address whose behavior lives in this crate now.
    pub fn enter_new_month() {
        for show in Self::registered_show_values() {
            if show != 0 {
                crate::ztshowinfo::enter_new_month(show);
            }
        }
    }

    /// Stage 5 port of `ZTShowMgr::update` (`0x00434e1e`, per `ZTShowMgr_update.asm`/`.c` - the
    /// decompile shows the full body; the `.asm` is just the guard tail-jumping to the shared walk
    /// code at `0x0059e6e3`): `update`'s own `0 < (int)mbr_0x2c` size guard (map sizes are never
    /// negative, so `!is_empty()` is the exact equivalent, and also subsumes vanilla's second
    /// `left == header` empty check), then the same in-order walk as
    /// [`ZTShowMgr::enter_new_month`] - reached through `ZTShowMgr`'s own 2-slot vtable
    /// (`private/docs/vtables/ZTShowMgr.md`) by the AI manager's periodic update loop.
    ///
    /// The per-show call is reproduced as the same **virtual dispatch** vanilla does
    /// (`(**(code **)(**value + 0x20))()`): the callee comes out of the show object's *own* vtable
    /// pointer at slot `+0x20` (`ztshowinfo::UPDATE`, `0x0059e725`, confirmed against
    /// `private/docs/vtables/ZTShowInfo.md`), not from a hardcoded address - so a subclass slot
    /// override or a future detour of `ztshowinfo::UPDATE` is picked up exactly as a vanilla
    /// caller would pick it up.
    pub fn update() {
        let shows = Self::registered_show_values();
        if shows.is_empty() {
            return;
        }
        for show in shows {
            if show != 0 {
                let vtable: u32 = get_from_memory(show);
                let slot: u32 = get_from_memory(vtable + 0x20);
                let update_fn: unsafe extern "thiscall" fn(*const u32) = unsafe { std::mem::transmute(slot) };
                unsafe { update_fn(show as *const u32) };
            }
        }
    }

    /// Stage 6 port of `ZTShowMgr::save` (`0x00479fa4`, per `ZTShowMgr_save.asm`/`.c`; macOS's
    /// `ZTShowMgr_save.c` agrees): delegates to the embedded `ZTShowScriptMgr`'s own save, then
    /// persists the 2-byte show-id counter through real `WriteBytesToFile`. The registered-shows
    /// map itself is deliberately *not* serialized - every `ZTShowInfo` re-registers through its
    /// own separate load path - so the counter bytes are this method's only own payload. Vanilla
    /// runs the counter write even when the delegation fails and combines the two with plain `&`
    /// (no short-circuit), which is reproduced. The delegation goes through a direct
    /// `CALL ZTShowScriptMgr::save`, so it is routed via `.hooked()` (same direct-address rule as
    /// [`ZTShowMgr::enter_new_month`]): a direct CALL executes whatever sits at the raw address,
    /// which is `ztshowscriptmgr.rs`'s Rust detour once installed. Returns `0`/`1` - the real
    /// body only guarantees its `SETZ`-produced `AL`; upper EAX is leftover register garbage
    /// there.
    ///
    /// Stage 9 repointed the counter read at [`SHOW_STORE`]'s field (the counter's single owner
    /// since the writers' cutover); the bytes are copied out under the lock and written from a
    /// local, so the store mutex is never held across the `WriteBytesToFile` call-out.
    pub fn save(&mut self, file: *const i8) -> u32 {
        let script_ok = unsafe { ZTSHOWSCRIPTMGR_SAVE.hooked()(&raw const self.show_script_mgr as *const u32, file) };
        let counter: u16 = SHOW_STORE.lock().unwrap().show_id_counter;
        let write_ok = unsafe { WRITE_BYTES_TO_FILE.hooked()(&raw const counter as *const u32, 2, 1, file) } == 1;
        (script_ok & write_ok) as u32
    }

    /// Stage 6 port of `ZTShowMgr::load` (`0x004c6f54`, per `ZTShowMgr_load.asm`/`.c`; macOS's
    /// `ZTShowMgr_load.c` agrees): the mirror of [`ZTShowMgr::save`] - same `ZTShowScriptMgr`
    /// delegation through `.hooked()` (same direct-CALL routing) - then, only for
    /// `version > 0x60` (the `.asm` gates on unsigned `CMP %ESI, 0x61` / `JC`), reads the 2-byte
    /// counter back through real `deallocate` and ANDs the read's success into the result. The
    /// read is attempted whenever the version gate passes - even after a failed delegation -
    /// exactly like vanilla. Returns `0`/`1` cleaned like [`ZTShowMgr::save`].
    ///
    /// Stage 9 repointed the counter destination at [`SHOW_STORE`]'s field: the read lands in a
    /// local (so the store mutex is never held across the `deallocate` call-out) and is copied
    /// into the store only on a successful read - the same only-on-success visibility the real
    /// body's in-place global write gave the battery's short-read pins.
    pub fn load(&mut self, file: *const u32, version: u32) -> u32 {
        let mut ok = unsafe { ZTSHOWSCRIPTMGR_LOAD.hooked()(&raw const self.show_script_mgr as *const u32, file, version) };
        if version > 0x60 {
            let mut counter: u16 = 0;
            let read_ok =
                unsafe { DEALLOCATE.hooked()(&raw mut counter as *const u32, 2, 1, file as *const u8) } == 1;
            if read_ok {
                SHOW_STORE.lock().unwrap().show_id_counter = counter;
            }
            ok &= read_ok;
        }
        ok as u32
    }

    /// Stage 7 port of `ZTShowMgr::isDoingShow` (`0x0059eb6e`, per `ZTShowMgr_isDoingShow.asm`/
    /// `.c`): the "is `unit_id` performing in show `show_id`" probe. Vanilla shape, `.asm`-verified
    /// (`RET 0x8` - a u32 and a u16 stack slot): `getShowInfo(this, show_id)`, answered from
    /// [`SHOW_STORE`] exactly like [`ZTShowMgr::get_show_info`], which this calls directly the way
    /// [`ZTShowMgr::get_script_id`] does (the real body only ever forwards its `this` to that
    /// lookup, which the stage-4 detour answers from the store); on a hit, the pure Rust
    /// [`crate::ztshow::get_show_script_state`] reader on the found show's embedded `ZTShow` at
    /// `+0x4` - the same read-only script-state map lookup `ztshow.rs`'s `do_current_item` already
    /// drives. Any script state recorded for that unit (a non-null `+0x14` value out of the
    /// embedded `ZTShow`'s script-state map walk) means the unit is doing the show.
    ///
    /// The real body reports its answer in `AL` only (`SETNZ %AL`, with EAX's upper bits left
    /// holding the state pointer's high bits); macOS's `ZTShowMgr_isDoingShow.c` normalizes the
    /// same predicate to a full-width 0/1 (`(-v | v) >> 0x1f`), which this returns. That is the
    /// observable contract too: vanilla's own `ZTUnit::isDoingShow` propagates the full EAX but has
    /// a sibling path returning garbage-upper-bits-with-`AL=1`, so its callers necessarily test the
    /// low byte alone.
    pub fn is_doing_show(unit_id: u32, show_id: u16) -> u32 {
        let show_info = Self::get_show_info(show_id);
        if show_info != 0 {
            let state = crate::ztshow::get_show_script_state(show_info + 0x4, unit_id);
            (state != 0) as u32
        } else {
            0
        }
    }

    /// Stage 8 port of `ZTShowMgr::isShowScriptDone` (`0x0059fab2`, per
    /// `ZTShowMgr_isShowScriptDone.asm`/`.c`; macOS's `ZTShowMgr_isShowScriptDone.c` agrees): the
    /// "has `script_id`'s script in show `show_id` reached its done state" probe
    /// `ZTUnit::isShowScriptDone` reaches through (vtable slot `+0x230`, `0x0059fa79` - not to be
    /// confused with that caller's own address, which the regen attributed to `ztanimal`;
    /// `generated.rs`'s `ztshowmgr::IS_SHOW_SCRIPT_DONE` is this method). Vanilla shape,
    /// `.asm`-verified (`RET 0x8` - a u32 and a u16 stack slot; `this` is forwarded to nothing but
    /// the `getShowInfo` lookup): `getShowInfo(this, show_id)`, answered from [`SHOW_STORE`]
    /// exactly like [`ZTShowMgr::get_show_info`], which this calls directly the way
    /// [`ZTShowMgr::is_doing_show`] does; on a hit, the same pure Rust
    /// [`crate::ztshow::get_show_script_state`] reader on the found show's embedded `ZTShow` at
    /// `+0x4` [`ZTShowMgr::is_doing_show`] uses - and, where that sibling null-tests the returned
    /// state pointer, this reads the found `ZTShowScriptState`'s done byte at `+0x13` (`MOV %AL,
    /// byte ptr [EAX + 0x13]`). Both miss paths (unregistered show, stateless unit) return `0`,
    /// like the real body's `MOV %AL, %BL` exits.
    ///
    /// The hit path returns the raw done byte zero-extended, not a normalized 0/1 - macOS's
    /// version returns the same byte (`undefined1`), so the byte is the contract. Upper EAX bits
    /// are garbage in the real body (left holding the state pointer's high bits), but that is
    /// unobservable for the same reason it is in [`ZTShowMgr::is_doing_show`]: the only corpus
    /// caller chain (`ZTUnit::isShowScriptDone`) propagates the full EAX yet has a sibling early
    /// path exiting `MOV %AL, %BL` - garbage upper bits with `AL=0` - so no caller can compare the
    /// full width against anything stable; the low byte is all any caller can meaningfully read.
    pub fn is_show_script_done(script_id: u32, show_id: u16) -> u32 {
        let show_info = Self::get_show_info(show_id);
        if show_info != 0 {
            let state = crate::ztshow::get_show_script_state(show_info + 0x4, script_id);
            if state != 0 {
                get_from_memory::<u8>(state + 0x13) as u32
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Copy of the store's registered values, in the `BTreeMap`'s ascending-`u16`-key order. The
    /// lock is held only for the copy - never across the walks' callee calls, which can re-enter
    /// the detoured `GET_SHOW_INFO`/`REGISTER_SHOW`/`UNREGISTER_SHOW` addresses (event processing
    /// inside a show's own `update` can start/stop shows) and would deadlock against the writers'
    /// post-trampoline mirroring. A store mutation that happens mid-walk (from a callee) is
    /// therefore picked up on the *next* walk, exactly as vanilla's own tree walk would have been
    /// at worst similarly stale - no vanilla caller depends on same-walk visibility of a
    /// callee-triggered registration change.
    fn registered_show_values() -> Vec<u32> {
        SHOW_STORE.lock().unwrap().registered_shows.values().copied().collect()
    }
}

/// Stage 2's detour: `INIT_SHOW_PARAMS` (`0x0051f59b`). The real constructor
/// (`0x0051f73a`, never detoured - see the module doc) tail-calls this address
/// (`CALL 0x0051f59b` at `0x0051f793`, binary-confirmed), so hooking it routes the live global's
/// config-driven construction through the Rust port; it writes the same defaults vanilla writes
/// and overrides from the same `shows.cfg`, so the live constructor's end state is unchanged.
/// The decompile corpus has no other caller (`ZTShowMgr_ZTShowMgr.c` is the only reference).
///
/// Stage 3 adds the registered-shows write path to the same block: `REGISTER_SHOW`/`UNREGISTER_SHOW`
/// (`0x005abb26`/`0x005aaa95`, no other `openzt/src` callers of either address) - detoured first as
/// shadow/mirrors over the call-through vanilla bodies, then completed by stage 9 into full ports
/// (call-through dropped) - see [`ZTShowMgr::register_show`]/[`ZTShowMgr::unregister_show`].
///
/// Stage 4 adds the read cutover: `GET_SHOW_INFO`/`GET_SCRIPT_ID` (`0x0041ebfd`/`0x005a2665`)
/// route every caller - including un-decompiled vanilla code and the real `unregisterShow` body's
/// own internal lookups, which reach `getShowInfo` by raw address - onto the store-backed readers.
///
/// Stage 5 adds the two whole-map walks: `ENTER_NEW_MONTH`/`UPDATE` (`0x004842a2`/`0x00434e1e`)
/// iterate [`SHOW_STORE`] and call the same real `ZTShowInfo` callees vanilla's own walks call -
/// see [`ZTShowMgr::enter_new_month`]/[`ZTShowMgr::update`]. The live callers this puts on the
/// Rust path are `ZooStatus::financeChecks` (monthly, direct call) and the AI manager's periodic
/// vtable dispatch of `update`.
///
/// Stage 6 adds the save/load pair: `SAVE`/`LOAD` (`0x00479fa4`/`0x004c6f54`) - the real
/// save-game flow (`ZTWorldMgr::save`/`load`, the corpus's only callers) now runs the Rust script
/// store's save/load through the same direct `CALL` vanilla makes and persists the vanilla-owned
/// show-id counter around it - see [`ZTShowMgr::save`]/[`ZTShowMgr::load`].
///
/// Stage 7 adds a read-side probe: `IS_DOING_SHOW` (`0x0059eb6e`), the address
/// `ZTUnit::isDoingShow` (the corpus's one caller) reaches through with a unit id and the unit's
/// current show id - see [`ZTShowMgr::is_doing_show`].
///
/// Stage 8 adds the last probe: `IS_SHOW_SCRIPT_DONE` (`0x0059fab2`), the address
/// `ZTUnit::isShowScriptDone` (vtable slot `+0x230`) reaches through with the same two ids -
/// see [`ZTShowMgr::is_show_script_done`].
#[detour_mod]
mod detours {
    use super::*;

    #[detour(INIT_SHOW_PARAMS)]
    unsafe extern "thiscall" fn init_show_params_detour(this: *const u32) -> u32 {
        unsafe { mut_from_memory::<ZTShowMgr>(this).init_show_params() }
    }

    #[detour(REGISTER_SHOW)]
    unsafe extern "thiscall" fn register_show_detour(this: *const u32, show: *const u32, force: bool) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).register_show(show, force) != 0 }
    }

    #[detour(UNREGISTER_SHOW)]
    unsafe extern "thiscall" fn unregister_show_detour(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).unregister_show(id, show, clear) != 0 }
    }

    /// Stage 4 read cutover - see [`ZTShowMgr::get_show_info`]. The instance pointer is deliberately
    /// unread (single process-global store), so unlike the other detours here it is dropped rather
    /// than mapped onto a `ZTShowMgr` - a null `this`, which vanilla's own body would fault on,
    /// resolves through the same store as any other.
    #[detour(GET_SHOW_INFO)]
    unsafe extern "thiscall" fn get_show_info_detour(_this: *const u32, id: u16) -> *const u32 {
        ZTShowMgr::get_show_info(id) as *const u32
    }

    /// Stage 4 read cutover - see [`ZTShowMgr::get_script_id`]. Same dropped-`this` reasoning as
    /// [`get_show_info_detour`].
    #[detour(GET_SCRIPT_ID)]
    unsafe extern "thiscall" fn get_script_id_detour(_this: *const u32, id: u16) -> u16 {
        ZTShowMgr::get_script_id(id) as u16
    }

    /// Stage 5 walk - see [`ZTShowMgr::enter_new_month`]. Dropped `this`, same single-store
    /// reasoning as [`get_show_info_detour`].
    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month_detour(_this: *const u32) {
        ZTShowMgr::enter_new_month()
    }

    /// Stage 5 walk - see [`ZTShowMgr::update`]. Dropped `this`, same single-store reasoning as
    /// [`get_show_info_detour`].
    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update_detour(_this: *const u32) {
        ZTShowMgr::update()
    }

    /// Stage 6 save - see [`ZTShowMgr::save`]. The instance is used only to derive the embedded
    /// `ZTShowScriptMgr` sub-object address the delegation receives (its own detour ignores it -
    /// the Rust store is process-global), matching vanilla's `ADD %ECX, 0x34` hand-off.
    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).save(file) != 0 }
    }

    /// Stage 6 load - see [`ZTShowMgr::load`]. Same sub-object hand-off as [`save_detour`].
    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { mut_from_memory::<ZTShowMgr>(this).load(file, version) != 0 }
    }

    /// Stage 7 probe - see [`ZTShowMgr::is_doing_show`]. Dropped `this`, same single-store
    /// reasoning as [`get_show_info_detour`]: the real body only ever forwards `this` to
    /// `getShowInfo`, whose stage-4 detour already answers from the store.
    #[detour(IS_DOING_SHOW)]
    unsafe extern "thiscall" fn is_doing_show_detour(_this: *const u32, unit_id: u32, show_id: u16) -> bool {
        ZTShowMgr::is_doing_show(unit_id, show_id) != 0
    }

    /// Stage 8 probe - see [`ZTShowMgr::is_show_script_done`]. Dropped `this`, same single-store
    /// reasoning as [`get_show_info_detour`]: the real body only ever forwards `this` to
    /// `getShowInfo`, whose stage-4 detour already answers from the store.
    #[detour(IS_SHOW_SCRIPT_DONE)]
    unsafe extern "thiscall" fn is_show_script_done_detour(_this: *const u32, script_id: u32, show_id: u16) -> u8 {
        ZTShowMgr::is_show_script_done(script_id, show_id) as u8
    }

    /// The real vanilla body through the detour's trampoline - the only release-safe way back to
    /// vanilla once this detour has patched the address (`FunctionDef::original()` is a raw address
    /// cast in release and would silently re-enter [`init_show_params_detour`]). Used by the live
    /// battery's `ZTSHOWMGR_INIT_SHOW_PARAMS` real-side pole.
    pub(super) fn call_real(this: *const u32) -> u32 {
        unsafe { INIT_SHOW_PARAMS_DETOUR.call(this) }
    }

    /// The real vanilla `registerShow` through the detour's trampoline - same release-safety
    /// reasoning as [`call_real`]. This is [`ZTShowMgr::register_show`]'s shadow/mirror call-through.
    pub(super) fn call_real_register_show(this: *const u32, show: *const u32, force: bool) -> bool {
        unsafe { REGISTER_SHOW_DETOUR.call(this, show, force) }
    }

    /// Same mechanism for the real `unregisterShow` - see [`call_real_register_show`].
    pub(super) fn call_real_unregister_show(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        unsafe { UNREGISTER_SHOW_DETOUR.call(this, id, show, clear) }
    }

    /// The real vanilla `getShowInfo` through the detour's trampoline - the only route back to the
    /// genuine tree walk now that stage 4 hooks the address. The battery's real-side pole for it:
    /// since stage 9 stopped the writers maintaining the tree, this walk answers only what was
    /// planted there through the raw trampolines (see [`call_real_register_show`]), which is exactly
    /// the tree-only differential the stage-5 walk tests pin.
    pub(super) fn call_real_get_show_info(this: *const u32, id: u16) -> *const u32 {
        unsafe { GET_SHOW_INFO_DETOUR.call(this, id) }
    }

    /// Same mechanism for the real `getScriptID`. Note its real body reaches `getShowInfo` by raw
    /// address (`CALL ZTShowMgr::getShowInfo` in `ZTShowMgr_getScriptID.asm`), so post-cutover this
    /// pole is vanilla glue around the *detoured* reader - it verifies the real ABI glue and the
    /// real `+0x8` read on top of [`SHOW_STORE`]'s answer, not vanilla's tree.
    pub(super) fn call_real_get_script_id(this: *const u32, id: u16) -> u16 {
        unsafe { GET_SCRIPT_ID_DETOUR.call(this, id) }
    }

    /// The real vanilla `enterNewMonth` walk through the detour's trampoline - the battery's
    /// `ZTSHOWMGR_ENTER_NEW_MONTH` real-side pole: it still traverses the dual-written standalone
    /// vanilla tree, so its visits prove the vanilla walk sees exactly the registrations the store
    /// does (plus any made through the raw trampoline that the store deliberately never mirrored).
    pub(super) fn call_real_enter_new_month(this: *const u32) {
        unsafe { ENTER_NEW_MONTH_DETOUR.call(this) }
    }

    /// Same mechanism for the real `update` walk - the `ZTSHOWMGR_UPDATE` real-side pole. Its
    /// per-show virtual dispatch is vanilla's own, so routing a sentinel vtable slot through it
    /// pins the dispatch convention (slot `+0x20`, `this` = the show pointer) the port reproduces.
    pub(super) fn call_real_update(this: *const u32) {
        unsafe { UPDATE_DETOUR.call(this) }
    }

    /// The real vanilla `save` through the detour's trampoline - the `ZTSHOWMGR_SAVE_LOAD`
    /// real-side pole. Its delegation still reaches `ZTShowScriptMgr::save` through a direct
    /// `CALL`, i.e. the same Rust script-store save the port's `.hooked()` call reaches, so what
    /// the pole isolates is vanilla's own tail: reading [`SHOW_ID_COUNTER_RVA`] in place and the
    /// exact write shape (address, size 2, count 1) it hands `WriteBytesToFile` - the capture
    /// must come out byte-identical to the port's.
    pub(super) fn call_real_save(this: *const u32, file: *const i8) -> bool {
        unsafe { SAVE_DETOUR.call(this, file) }
    }

    /// Same mechanism for the real `load` - the `ZTSHOWMGR_SAVE_LOAD` real-side pole, pinning
    /// vanilla's unsigned `version > 0x60` gate and the 2-byte read target against the port's.
    pub(super) fn call_real_load(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { LOAD_DETOUR.call(this, file, version) }
    }

    /// Same mechanism for the real `isDoingShow` - the `ZTSHOWMGR_IS_DOING_SHOW` real-side pole.
    /// Its `getShowInfo` leg resolves through the stage-4 detoured reader (a raw `CALL`), so the
    /// pole isolates vanilla's own tail: the embedded-`ZTShow` hand-off (`LEA %ECX, [EAX + 0x4]`)
    /// and the `SETNZ %AL` combine on the real `getShowScriptState` walk.
    pub(super) fn call_real_is_doing_show(this: *const u32, unit_id: u32, show_id: u16) -> bool {
        unsafe { IS_DOING_SHOW_DETOUR.call(this, unit_id, show_id) }
    }

    /// Same mechanism for the real `isShowScriptDone` - the `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE`
    /// real-side pole. Its `getShowInfo` leg resolves through the stage-4 detoured reader (a raw
    /// `CALL`), so the pole isolates vanilla's own tail: the embedded-`ZTShow` hand-off
    /// (`LEA %ECX, [EAX + 0x4]`), the real `getShowScriptState` walk, and the `+0x13` byte read.
    pub(super) fn call_real_is_show_script_done(this: *const u32, script_id: u32, show_id: u16) -> u8 {
        unsafe { IS_SHOW_SCRIPT_DONE_DETOUR.call(this, script_id, show_id) }
    }
}

/// Enables the stage-2 through stage-9 detours (`INIT_SHOW_PARAMS`, the `registerShow`/
/// `unregisterShow` write ports, the `getShowInfo`/`getScriptID` read cutover, the
/// `enterNewMonth`/`update` walk ports, the `save`/`load` pair, and the `isDoingShow`/
/// `isShowScriptDone` probes), and seeds the Rust-owned show-id counter ([`ZTShowMgrState::
/// show_id_counter`]) from the vanilla `DAT_0063e480` global it replaces. The seed runs before the
/// hooks go live: nothing vanilla can reach the counter through except this module's (about to be
/// detoured) addresses, so from the instant they are armed the store's copy is the only live one -
/// the global is inert exactly like vanilla's `+0x28` tree. Called from `lib.rs`'s `experimental`
/// block and from `reimplementation_tests::init` (the test harness never runs `openztlib::init()`).
pub fn init() {
    let counter_addr = (get_module_base("zoo.exe") as u32 + SHOW_ID_COUNTER_RVA) as *const u16;
    SHOW_STORE.lock().unwrap().show_id_counter = unsafe { *counter_addr };
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowmgr detours: {e:?}");
    }
}

/// Process-global store backing the one real `ZTShowMgr` singleton, keyed by nothing (one real
/// instance, same shape as `ztawardmgr.rs`'s store). Since stage 9 this is the *only* live copy of
/// the registered-shows map *and* of the show-id counter: stage 3 landed it as a live mirror of
/// vanilla's tree, stage 4's read cutover flipped `getShowInfo`/`getScriptID` onto it, and stage 9
/// dropped the writers' call-throughs, moving the tree insert/erase (`register_show`/
/// `unregister_show`) and the `DAT_0063e480` counter ([`ZTShowMgrState::show_id_counter`], seeded
/// once from the global by [`init`]) in with them. Vanilla's own `+0x28` tree and counter global
/// are now safely-inert dead weight, maintained by nothing.
///
/// Values are real, vanilla-owned `ZTShowInfo*` addresses - **never owned, never freed, never
/// dereferenced-freed** by this module (`ZTShowInfo` objects are constructed/owned by their real
/// callers, e.g. `ZTHabitat::setIsShowExhibit`'s own `new(0xb0)`; same un-owned-pointer rule as
/// everything `ztshow.rs` already does).
#[derive(Debug, Default)]
struct ZTShowMgrState {
    registered_shows: BTreeMap<u16, u32>,
    /// The show-id counter vanilla kept at `DAT_0063e480` - a u16, since every one of the global's
    /// three consumers accesses it 16-bit (`registerShow`'s word `INC` and both of save/load's
    /// 2-byte transfers). Incremented by [`ZTShowMgr::register_show`]'s fresh-id branch and
    /// persisted by stage 6's save/load; never `0xffff` as an assigned id (`% 0xffff`).
    show_id_counter: u16,
}

static SHOW_STORE: LazyLock<Mutex<ZTShowMgrState>> = LazyLock::new(|| Mutex::new(ZTShowMgrState::default()));

/// Number of shows currently registered in the Rust store. Test-support: `ZTSHOWMGR_STANDALONE_
/// ROUNDTRIP` asserts it stays empty across standalone construction, and the stage-3 mirror test
/// (`ZTSHOWMGR_REGISTER_UNREGISTER_SHOW`) asserts its own ops drain it back to zero.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn registered_show_count() -> usize {
    SHOW_STORE.lock().unwrap().registered_shows.len()
}

/// The store's answer for one show id - the battery's store-content probe
/// (`ZTSHOWMGR_REGISTER_UNREGISTER_SHOW` and `ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID`). Since stage
/// 4's read cutover this is also the live implementation: [`ZTShowMgr::get_show_info`] and the
/// `GET_SHOW_INFO` detour answer every in-game caller from the same map this reads.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn registered_show_for_id(id: u16) -> Option<u32> {
    SHOW_STORE.lock().unwrap().registered_shows.get(&id).copied()
}

/// Every `(id, show_addr)` pair currently in the store - diagnostic for a live test to check for
/// duplicate `show_addr` values under different ids, the signature of a stale entry left behind by
/// `ZTShowInfo::updateFromLoad`'s real register-then-unregister-old-id dance (see
/// `private/resources/decompiles/ZTShowInfo_updateFromLoad.c`) if [`ZTShowMgr::register_show`]/
/// [`ZTShowMgr::unregister_show`] ever mishandle it.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn all_registered_shows() -> Vec<(u16, u32)> {
    SHOW_STORE.lock().unwrap().registered_shows.iter().map(|(&id, &addr)| (id, addr)).collect()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use openzt_detour::generated::standalone::OPERATOR_NEW;

    /// Allocates a standalone, zero-initialized `0x44`-byte `ZTShowMgr` via the real vanilla allocator
    /// (`standalone::OPERATOR_NEW`) for the live battery's `ZTSHOWMGR_STANDALONE_ROUNDTRIP` test.
    ///
    /// Deliberately never freed: instances built through the *real* constructor (the test runs both
    /// constructors side by side) own `DAT_00638008`-freelist nodes at `+0x28`/`+0x38` that have no safe
    /// Rust-side return path (the matching free helpers have no `generated.rs` entries), so per
    /// CLAUDE.md's leak-only-teardown precedent (`ztthoughtmgr.rs`'s
    /// `destroy_standalone_mgr_leaking_nodes`) both buffers are just left allocated for the rest of the
    /// one-shot test process's lifetime.
    pub(crate) fn allocate_uninitialized() -> *mut ZTShowMgr {
        unsafe { OPERATOR_NEW.original()(0x44) as *mut ZTShowMgr }
    }

    /// Calls the real vanilla `ZTShowMgr::initShowParams` through the detour's trampoline, for the
    /// live battery's `ZTSHOWMGR_INIT_SHOW_PARAMS` real-side pole - see
    /// [`detours::call_real`]'s doc comment for why `INIT_SHOW_PARAMS.original()` can't be used for
    /// this now that stage 2 hooks the address (a release build's raw-cast `.original()` would
    /// silently re-enter the Rust detour and degenerate the test into Rust-vs-Rust).
    pub(crate) fn call_real_init_show_params(this: *const u32) -> u32 {
        detours::call_real(this)
    }

    /// Calls the real vanilla `ZTShowMgr::getShowInfo` through the detour's trampoline - the
    /// battery's genuine-vanilla-tree pole (`ZTSHOWMGR_REGISTER_UNREGISTER_SHOW`'s diff oracle and
    /// `ZTSHOWMGR_GET_SHOW_INFO_GET_SCRIPT_ID`'s real side) - see
    /// [`detours::call_real_get_show_info`]; `.original()` has been a release-profile re-entry
    /// hazard on this address since stage 4 hooked it.
    pub(crate) fn call_real_get_show_info(this: *const u32, id: u16) -> *const u32 {
        detours::call_real_get_show_info(this, id)
    }

    /// Calls the real vanilla `ZTShowMgr::getScriptID` through the detour's trampoline - see
    /// [`detours::call_real_get_script_id`] for why this pole is only half-real post-cutover.
    pub(crate) fn call_real_get_script_id(this: *const u32, id: u16) -> u16 {
        detours::call_real_get_script_id(this, id)
    }

    /// Calls the real vanilla `ZTShowMgr::enterNewMonth` through the detour's trampoline - the
    /// `ZTSHOWMGR_ENTER_NEW_MONTH` real-side pole; `.original()` has been a release-profile
    /// re-entry hazard on this address since stage 5 hooked it.
    pub(crate) fn call_real_enter_new_month(this: *const u32) {
        detours::call_real_enter_new_month(this)
    }

    /// Calls the real vanilla `ZTShowMgr::update` through the detour's trampoline - the
    /// `ZTSHOWMGR_UPDATE` real-side pole, same `.original()` hazard as
    /// [`call_real_enter_new_month`].
    pub(crate) fn call_real_update(this: *const u32) {
        detours::call_real_update(this)
    }

    /// Runtime address of the vanilla show-id counter global ([`SHOW_ID_COUNTER_RVA`]) - inert to
    /// the implementation since stage 9, but still read/written **in place** by the real
    /// save/load bodies the battery reaches through their trampolines, so `ZTSHOWMGR_SAVE_LOAD`
    /// seeds/clobbers/restores it around those poles (alongside the store's own copy via
    /// [`show_id_counter`]/[`set_show_id_counter`]).
    pub(crate) fn show_id_counter_addr() -> u32 {
        get_module_base("zoo.exe") as u32 + SHOW_ID_COUNTER_RVA
    }

    /// The store's show-id counter - the single live copy since stage 9. Test-support for
    /// `ZTSHOWMGR_REGISTER_UNREGISTER_SHOW`'s counter pins (fresh-id step, wrap semantics) and
    /// `ZTSHOWMGR_SAVE_LOAD`'s pole-specific seeding.
    pub(crate) fn show_id_counter() -> u16 {
        SHOW_STORE.lock().unwrap().show_id_counter
    }

    /// Overwrites the store's show-id counter - the stage-9 test rework seeds exact values to make
    /// fresh-id assignment deterministic and to reach the wrap boundary (`0xfffe`/`0xffff`)
    /// without needing 65k registrations.
    pub(crate) fn set_show_id_counter(value: u16) {
        SHOW_STORE.lock().unwrap().show_id_counter = value;
    }

    /// Calls the real vanilla `ZTShowMgr::save` through the detour's trampoline - the
    /// `ZTSHOWMGR_SAVE_LOAD` real-side pole; `.original()` has been a release-profile re-entry
    /// hazard on this address since stage 6 hooked it.
    pub(crate) fn call_real_save(this: *const u32, file: *const i8) -> bool {
        detours::call_real_save(this, file)
    }

    /// Same for the real `ZTShowMgr::load` - see [`call_real_save`].
    pub(crate) fn call_real_load(this: *const u32, file: *const u32, version: u32) -> bool {
        detours::call_real_load(this, file, version)
    }

    /// Calls the real vanilla `ZTShowMgr::isDoingShow` through the detour's trampoline - the
    /// `ZTSHOWMGR_IS_DOING_SHOW` real-side pole; `.original()` has been a release-profile re-entry
    /// hazard on this address since stage 7 hooked it.
    pub(crate) fn call_real_is_doing_show(this: *const u32, unit_id: u32, show_id: u16) -> bool {
        detours::call_real_is_doing_show(this, unit_id, show_id)
    }

    /// Calls the real vanilla `ZTShowMgr::isShowScriptDone` through the detour's trampoline - the
    /// `ZTSHOWMGR_IS_SHOW_SCRIPT_DONE` real-side pole; `.original()` has been a release-profile
    /// re-entry hazard on this address since stage 8 hooked it.
    pub(crate) fn call_real_is_show_script_done(this: *const u32, script_id: u32, show_id: u16) -> u8 {
        detours::call_real_is_show_script_done(this, script_id, show_id)
    }

    /// Calls the real vanilla `ZTShowMgr::registerShow` through the detour's trampoline,
    /// **bypassing the stage-9 port** - the stage-5 walk tests use it to plant a registration
    /// that exists only in the standalone vanilla tree, proving the Rust walks read [`SHOW_STORE`]
    /// and not the tree (the hooked register writes only the store, so the raw body is the only
    /// way to make the two stores disagree on purpose). Note the raw body still increments the
    /// now-inert vanilla counter global and writes `field_0x70` through the real setter - both
    /// harmless to the store, which never reads either.
    pub(crate) fn call_real_register_show(this: *const u32, show: *const u32, force: bool) -> bool {
        detours::call_real_register_show(this, show, force)
    }

    /// The matching raw-body `unregisterShow` - drains a [`call_real_register_show`]-planted
    /// registration from the standalone vanilla tree without touching the store (a hooked
    /// unregister's mirror step would be a no-op remove for an id the store never held, but going
    /// through the raw body keeps the store-tree asymmetry explicit).
    pub(crate) fn call_real_unregister_show(this: *const u32, id: u16, show: *const u32, clear: bool) -> bool {
        detours::call_real_unregister_show(this, id, show, clear)
    }
}
