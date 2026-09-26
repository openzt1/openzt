//! `Ambients`/`AmbientsGroup` reimplementation, per
//! `openzt/plans/ztsoundscape-ambients-full-port-plan.md`. `Ambients` is a config-driven collection of
//! `AmbientsGroup` level bands plus a world position that `ztsoundscape.rs`'s `update` jitters each tick
//! before [`Ambients::play`] forwards to whichever band contains the caller's level - owned by
//! `ZTSoundscape` (`crowd_ambients`/`world_ambients`, `ztsoundscape.rs`) and,
//! independently, by `ZTViewingArea` and `ZTHabitat` (per the plan's "consumer-sharing" analysis,
//! neither needs touching: both only ever hold an opaque `Ambients*` built elsewhere and call the fixed
//! `ambients::PLAY` address on it).
//!
//! `AmbientsGroup` (struct, constructor, and `play`) is reimplemented below too - one level band
//! (e.g. "quiet"/"medium"/"large"), config-driven, forwarding its actual audio to a `SoundGroup*`
//! borrowed from `BFSndMgr`'s own pool. [`Ambients::destruct`] frees each group via plain vanilla
//! `operator_delete` (no `~AmbientsGroup` exists anywhere in the corpus - nothing else to free, see the
//! borrowed-`SoundGroup*` note on [`AmbientsGroup`] itself). [`Ambients::play`] walks the group vector
//! and forwards to the first band that contains the caller's level.
//!
//! **Two `AmbientsGroup` construction paths, deliberately kept side by side**: [`AmbientsGroup::construct`]
//! reads its config through a real vanilla `BFConfigFile*` (`GET_INT`/`GET_STRING_LIST`/`GET_INT_LIST`) -
//! this is what stays hooked onto the raw `ambientsgroup::CONSTRUCTOR` address, since any caller
//! reaching that address (the live battery's own real-vanilla-pole trampoline included) hands it an
//! already-open real config object with no recoverable filename to re-derive anything else from.
//! [`Ambients::construct`]'s own top-level parse, by contrast, is the one case where nothing else needs
//! that particular `BFConfigFile` instance to keep existing afterward - it opens one file, reads it, and
//! throws it away - so it loads through `bfconfigfile::ini_compat` instead (see that module's doc
//! comment) and calls [`AmbientsGroup::construct_from_ini`] per group, retiring the real
//! `BFConfigFile` construct/attempt/release/freelist-node-return sequence from `Ambients`' own common
//! path entirely. `AMBIENTS_LIVE_GROUP_COMPARE` validates the two paths agree: real vanilla
//! `Ambients::Ambients`'s own internal call to `AmbientsGroup::AmbientsGroup` lands on the hooked
//! address (running [`AmbientsGroup::construct`]), while the Rust pole's [`Ambients::construct`] runs
//! [`AmbientsGroup::construct_from_ini`] - so a `sound_group` pointer match between the two poles is a
//! live cross-check that both paths parse the same file into the same pool entry.
//!
//! The [`ambients_detours`] block hooks both classes' fixed addresses
//! (`ambients::{CONSTRUCTOR, PLAY, DESTRUCTOR}`, `ambientsgroup::{CONSTRUCTOR, PLAY}`) onto the Rust
//! methods above, so any caller reaching those addresses - including `ztsoundscape.rs` (which calls the
//! Rust methods directly, reimplemented-to-reimplemented, rather than through the addresses) and the
//! still-un-ported `ZTViewingArea`/`ZTHabitat` consumers (which only ever hold an opaque `Ambients*`
//! and call the fixed `ambients::PLAY` address on it) - runs this port automatically, without either
//! consumer needing its own changes (see the plan's "consumer-sharing problem dissolves" section).
//!
//! Vanilla-layout-compatible (style 1, per `CLAUDE.md`), forced by the `ZTViewingArea`/`ZTHabitat`
//! sharing above: every byte of [`Ambients`] must stay exactly vanilla-shaped so those still-un-ported
//! callers keep working now that this class's constructor/destructor are detoured.
//!
//! Cross-allocator contract (per `CLAUDE.md`): every allocation this file makes or frees - each
//! `AmbientsGroup` block (`0x20c` bytes, vanilla `operator_new`/`operator_delete`) and the group-pointer
//! array at `this+0x0..+0x8` (vanilla `operator_new`, freed through the shared small-object freelist or
//! `operator_delete` depending on size, mirroring `Ambients::~Ambients`'s own tail exactly) - is real
//! vanilla memory, never Rust's `Box`. [`Ambients::construct`] stages the constructed group pointers in
//! a plain Rust `Vec` only as scratch (a Rust-owned, Rust-dropped buffer whose contents are vanilla
//! pointers but whose own backing allocation never becomes part of any vanilla-read struct) - the
//! pointer array the struct's own `0x0/0x4/0x8` fields end up pointing at is always a separate,
//! freshly vanilla-`operator_new`'d, exact-fit copy built once the final group count is known. This
//! deliberately departs from vanilla's own incremental-growth-and-copy dance
//! (`ZTScenarioTimer::meth_0x402119`), per the plan's "de-risked, not open" conclusion: the resulting
//! `cap == end` state is a legitimate one vanilla's own growth can also land on exactly, and no code
//! anywhere distinguishes "grown to fit" from "allocated to fit".
//!
//! `generated.rs`'s `bfconfigfile::GET_STRING_LIST`/`GET_INT_LIST` take a caller-owned 3-dword
//! `begin`/`end`/`cap` output-vector buffer as their second argument, per `Ambients_Ambients.asm`'s own
//! call site.

use std::ffi::CStr;

use openzt_configparser::ini::Ini;
use openzt_detour::generated::bfconfigfile::RELEASE as CONFIG_RELEASE;
use openzt_detour::generated::bfconfigfile::{GET_INT, GET_INT_LIST, GET_STRING_LIST};
use openzt_detour::generated::bfsndmgr::NEW_SOUND_GROUP;
use openzt_detour::generated::msvc_std_vectorint::{VECTORINT_0, VECTORINT_1};
/// Vanilla's actual `AmbientsGroup::AmbientsGroup`-site free for its own `sound`/`prob`
/// `getStringList`/`getIntList` result buffers - a 1-arg thiscall taking the whole 3-dword vector
/// struct's own address, confirmed by all four call sites in `AmbientsGroup_AmbientsGroup.asm`
/// loading the struct's address into ECX with zero pushed stack args before the `CALL`. Distinct
/// from [`DEALLOCATE_VECTOR`] (the unrelated free path for `Ambients::Ambients`'s own
/// `"ambientlevels"` list buffers - see that constant's doc comment).
use openzt_detour::generated::msvc_std_vector_t_4::VECTOR_T_4;
/// Frees the `vector<T>` result buffers a real `GET_STRING_LIST`/`GET_INT_LIST` call over the
/// `"ambientlevels"` name/range lists allocates - confirmed against `Ambients_Ambients.c`'s own
/// `FUN_004012f3(apuStack_c[0], (dStack_4-apuStack_c[0])>>2)` call sites. Production code no longer
/// makes that real call (see [`Ambients::build_groups_from_ini`]); only
/// [`live_support::first_ambient_level`]'s own real-config test helper still does, for the same two
/// keys. **Not** what real vanilla uses to free [`AmbientsGroup::construct`]'s own `sound`/`prob`
/// list buffers - see [`VECTOR_T_4`]'s doc comment for that one.
#[cfg(feature = "reimplementation-tests")]
use openzt_detour::generated::poolalloc::DEALLOCATE_N_4 as DEALLOCATE_VECTOR;
use openzt_detour::generated::soundgroup::PLAY as SOUNDGROUP_PLAY;
use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
use openzt_detour_macro::detour_mod;
use tracing::{error, warn};

use crate::bfconfigfile;
use crate::globals::get_module_base;
#[cfg(feature = "reimplementation-tests")]
use crate::util::ZTArray;
use crate::util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory};

/// `.rdata` addresses of the three config string literals `Ambients::Ambients` reads (per
/// `Ambients_Ambients.asm`'s `s_ambientlevels_0063b740`/`s_group_0063b750`/`s_value_0063914c` operand
/// labels) - RVA'd like every other data address in this codebase (zoo.exe has no ASLR, but these are
/// **data**, not code, so `base + RVA` still applies; see `ztsoundscape.rs`'s own data-address doc
/// comment for the code/data distinction).
const AMBIENTLEVELS_KEY_RVA: u32 = 0x0063_b740 - 0x400000;
const GROUP_SECTION_RVA: u32 = 0x0063_b750 - 0x400000; // "group" - getStringList's section
const VALUE_SECTION_RVA: u32 = 0x0063_914c - 0x400000; // "value" - getIntList's section

/// Base of vanilla's shared small-object freelist bucket array, bucketed by `(byte_capacity - 1) >> 3` -
/// the same `DAT_00638000` family `ztshowinfo.rs`'s `RVA_UNIT_ARRAY_FREELIST_BUCKETS`/`zoostatus.rs`
/// already document, confirmed independently here by `Ambients_~Ambients.asm`'s own tail (`SUB EAX,1;
/// SAR EAX,3; MOV EAX,[EAX*4+0x638000]; MOV [ECX],EAX; MOV [EAX*4... ]` - the same per-bucket
/// singly-linked push). RVA = `0x00638000 - 0x400000`.
const RVA_GROUP_ARRAY_FREELIST_BUCKETS: u32 = 0x0023_8000;

/// The shared small-object freelist head (`DAT_0063800c`) that a local scratch `BFConfigFile`'s
/// inlined `~BFConfigFile` tail returns its tree-root node to (see [`Ambients::construct`]'s doc
/// comment) - the same freelist `ztshowmgr.rs`'s `CONFIG_FREELIST_HEAD_RVA` already documents and uses
/// for the identical pattern. Note this is a *different* head from [`RVA_GROUP_ARRAY_FREELIST_BUCKETS`]
/// above - matching free idiom, different size class.
const CONFIG_FREELIST_HEAD_RVA: u32 = 0x0063800c - 0x400000;

/// The group-pointer array's shared-freelist bucket index for a given byte capacity, per
/// `Ambients_~Ambients.asm`'s `(byte_capacity - 1) >> 3` - split out as a pure function so the bucketing
/// math itself is unit-testable without a live process (see `tests` below). Callers must not pass `0`
/// (would underflow); [`free_group_array_buffer`] only calls this once it has already confirmed a
/// non-null buffer, whose byte capacity is always `>= 4`.
fn freelist_bucket_index(byte_capacity: u32) -> u32 {
    (byte_capacity - 1) >> 3
}

/// Whether a group-pointer array buffer of this byte capacity is freed via plain `operator_delete`
/// rather than the shared small-object freelist, per `Ambients_~Ambients.asm`'s own `byte_capacity >
/// 0x80` branch - split out as a pure predicate so the boundary itself is unit-testable without a live
/// process (see `tests` below).
fn should_use_operator_delete(byte_capacity: u32) -> bool {
    byte_capacity > 0x80
}

/// Frees a group-pointer array buffer built by [`Ambients::construct`] back to wherever real vanilla's
/// own `~Ambients` would - see [`RVA_GROUP_ARRAY_FREELIST_BUCKETS`]'s doc comment. No-op for a null
/// buffer (mirrors `~Ambients`'s own `if (puVar2 != 0)` guard).
fn free_group_array_buffer(buf: u32, byte_capacity: u32) {
    if buf == 0 {
        return;
    }
    if should_use_operator_delete(byte_capacity) {
        unsafe { OPERATOR_DELETE.original()(buf) };
        return;
    }
    let bucket_head_addr = get_module_base("zoo.exe") as u32 + RVA_GROUP_ARRAY_FREELIST_BUCKETS + freelist_bucket_index(byte_capacity) * 4;
    let old_head = get_from_memory::<u32>(bucket_head_addr);
    save_to_memory(buf, old_head);
    save_to_memory(bucket_head_addr, buf);
}

/// `.rdata` addresses of the three config key literals `AmbientsGroup::AmbientsGroup` reads (per
/// `AmbientsGroup_AmbientsGroup.asm`'s `s_chance_0063b768`/`s_sound_0063b760`/`s_prob_0063b758` operand
/// labels), looked up within the section named by the group's own level name (confirmed via
/// `BFConfigFile::getStringListPtr`'s own body: its first arg is searched at the config's root/section
/// level, its second arg searched within that section's children for the key - and `getStringList`'s own
/// hidden third stack argument, traced through `AmbientsGroup_AmbientsGroup.asm`'s push order, lands
/// exactly in that second/key slot). RVA'd like every other data address in this codebase.
const CHANCE_KEY_RVA: u32 = 0x0063_b768 - 0x400000;
const SOUND_KEY_RVA: u32 = 0x0063_b760 - 0x400000;
const PROB_KEY_RVA: u32 = 0x0063_b758 - 0x400000;

/// Sound-device singleton, same address `ztsoundscape.rs`'s own `GLOBAL_DX8SNDMGR_RVA` already
/// documents and reads (`MOV %ECX, dword ptr GLOBAL_DX8SndMgr` in both `AmbientsGroup::AmbientsGroup`
/// and that class's own detours) - duplicated locally rather than importing across modules, matching
/// this codebase's existing per-file convention for shared data RVAs.
const GLOBAL_DX8SNDMGR_RVA: u32 = 0x0063_80a8 - 0x400000;

/// The vanilla `Ambients` class (`0x18` bytes, confirmed allocation size - `ZTSoundscape::init` calls
/// `operator_new(0x18)` before constructing, see `ztsoundscape.rs`). Owns a vanilla-allocated array of
/// `AmbientsGroup*` (level bands, e.g. "quiet"/"medium"/"large") plus a world position `play` (plan
/// stage 3) jitters around each tick - already read/written raw by `ztsoundscape.rs`'s own `update` at
/// these same `x`/`y`/`z` offsets, independently corroborating this part of the layout.
#[repr(C)]
pub struct Ambients {
    groups_begin: u32, // 0x00 - *const *mut c_void, vanilla-allocated group-pointer array start
    groups_end: u32,   // 0x04 - one past the last valid entry
    groups_cap: u32,   // 0x08 - one past the buffer's actual allocated capacity (== groups_end here; see the module doc comment)
    x: i32,            // 0x0c - world position, jittered in place by ZTSoundscape::update today
    y: i32,            // 0x10
    z: i32,            // 0x14
}

const _: () = assert!(std::mem::size_of::<Ambients>() == 0x18);

/// Pure range-match check underlying [`Ambients::play`]'s per-group test, split out for unit testing
/// without a live process. `Ambients_play.c`'s miss condition is `param_2 < range_lo || range_hi <
/// param_2`; this is the negation - `level` inside `[range_lo, range_hi]`, inclusive both ends.
fn group_matches_level(range_lo: i32, range_hi: i32, level: i32) -> bool {
    level >= range_lo && level <= range_hi
}

/// Releases a local scratch `BFConfigFile` (already `CONFIG_RELEASE`-releasable, whether or not it was
/// ever successfully `attempt`-ed) via the real vanilla `RELEASE` call followed by vanilla's own
/// inlined `~BFConfigFile` tail - handing the tree-root node back to the shared small-object freelist
/// it was popped from (`DAT_0063800c`, plain `*node = head; head = node` - the same freelist
/// `ztshowmgr.rs`'s `CONFIG_FREELIST_HEAD_RVA` already documents and uses for this identical
/// local-scratch-config pattern). Skipping this would leak one node per call. Factored out of
/// [`Ambients::construct`] so [`live_support`]'s own standalone-`AmbientsGroup` test helper can build
/// and tear down a scratch config the identical way.
fn release_scratch_config(config_ptr: *const u32) {
    unsafe { CONFIG_RELEASE.original()(config_ptr) };

    let tree_root: u32 = get_from_memory(config_ptr as u32);
    if tree_root != 0 {
        let base = get_module_base("zoo.exe") as u32;
        let freelist_head = (base + CONFIG_FREELIST_HEAD_RVA) as *mut u32;
        unsafe {
            let head = *freelist_head;
            *(tree_root as *mut u32) = head;
            *freelist_head = tree_root;
        }
    }
}

impl Ambients {
    /// Reimplementation of `Ambients::Ambients` (`0x004495a3`), per `Ambients_Ambients.c`/`.asm`.
    /// `name` is the ambients config filename (`ZTSoundscape::init`'s own `crowd_ambients_name`/
    /// `world_ambients_name` arguments); `position` is the incoming `BFPos`-shaped 3-int world position
    /// (`ZTSoundscape::init`'s zeroed `ctor_data` local).
    ///
    /// Loads `name` through `bfconfigfile::ini_compat` rather than a real vanilla `BFConfigFile` - see
    /// the module doc comment for why this particular config instance (opened, read, and discarded
    /// entirely within this one call) doesn't need to be real vanilla memory the way
    /// [`AmbientsGroup::construct`]'s own `config` parameter does.
    pub fn construct(&mut self, name: *const u8, position: *const i32) {
        self.groups_begin = 0;
        self.groups_end = 0;
        self.groups_cap = 0;
        self.x = get_from_memory(position as u32);
        self.y = get_from_memory(position as u32 + 4);
        self.z = get_from_memory(position as u32 + 8);

        let name_present = !name.is_null() && get_from_memory::<u8>(name as u32) != 0;
        if !name_present {
            return;
        }

        let name_str = unsafe { CStr::from_ptr(name as *const i8) }.to_string_lossy().into_owned();
        match bfconfigfile::ini_compat::read_cfg(&name_str) {
            Some(ini) => self.build_groups_from_ini(&ini),
            None => warn!("Unable to open ambients file"),
        }
    }

    /// The config-driven group-building loop, `Ini`-backed counterpart of real vanilla
    /// `Ambients::Ambients`'s own `"ambientlevels"` walk. Real config layout (confirmed against a live
    /// `crowdsnd.cfg`, e.g.):
    /// ```text
    /// [ambientlevels]
    /// group = none
    /// value = -100
    /// value = 0
    /// group = small
    /// value = 1
    /// value = 20
    /// ```
    /// i.e. `ambientlevels` is the **section**, and `group`/`value` are repeated **keys** within it -
    /// the reverse of what this port originally assumed (and what `GET_STRING_LIST`/`GET_INT_LIST`'s
    /// own `(key, section)` argument-order doc comments elsewhere in this file describe; a live
    /// `AMBIENTS_LIVE_GROUP_COMPARE` run caught the swap - group counts were 0 on the `Ini` pole before
    /// this fix). `group` gives each level's name, one repeated-key line each; `value` gives the
    /// matching guest-count band as a flat, unsegmented sequence of ints (one int per line, not a
    /// space-separated pair - unlike [`AmbientsGroup::construct_from_ini`]'s own `sound`/`prob` lookups,
    /// which tolerate either convention via `word_list`), read in `(lo, hi)` pairs exactly like the
    /// pre-`Ini` version of this function read its raw `ZTArray`. Builds one [`AmbientsGroup`] per entry
    /// into a fresh vanilla allocation via [`AmbientsGroup::construct_from_ini`], then installs the
    /// resulting pointers into a freshly vanilla-allocated, exact-fit group array (see the module doc
    /// comment for why this departs from vanilla's own incremental-growth loop). A name with no
    /// matching range pair (a malformed config) gets `(0, 0)` rather than reading past the end of the
    /// flat list, unlike vanilla's own unchecked parallel-array indexing.
    fn build_groups_from_ini(&mut self, ini: &Ini) {
        let names = bfconfigfile::ini_compat::values(ini, "ambientlevels", "group");
        let flat_ranges: Vec<i32> = bfconfigfile::ini_compat::values(ini, "ambientlevels", "value").iter().filter_map(|v| v.parse().ok()).collect();
        let ranges: Vec<(i32, i32)> =
            (0..names.len()).map(|i| (flat_ranges.get(2 * i).copied().unwrap_or(0), flat_ranges.get(2 * i + 1).copied().unwrap_or(0))).collect();

        let mut groups: Vec<u32> = Vec::with_capacity(names.len());
        for (i, name) in names.iter().enumerate() {
            let (range_lo, range_hi) = ranges.get(i).copied().unwrap_or((0, 0));
            let group_block = unsafe { OPERATOR_NEW.original()(0x20c) };
            let group = if group_block.is_null() {
                0
            } else {
                unsafe { &mut *(group_block as *mut AmbientsGroup) }.construct_from_ini(ini, name, range_lo, range_hi);
                group_block as u32
            };
            groups.push(group);
        }

        if !groups.is_empty() {
            let byte_size = (groups.len() * 4) as u32;
            let buf = unsafe { OPERATOR_NEW.original()(byte_size) } as u32;
            if buf != 0 {
                for (i, &group) in groups.iter().enumerate() {
                    save_to_memory(buf + (i as u32) * 4, group);
                }
                self.groups_begin = buf;
                self.groups_end = buf + byte_size;
                self.groups_cap = buf + byte_size;
            }
        }
    }

    /// Reimplementation of `Ambients::~Ambients` (`0x0041e930`, `generated.rs`'s `ambients::DESTRUCTOR`,
    /// confirmed via `Ambients_~Ambients.meta`'s matching address and its sole caller being
    /// `operator_delete`), per `Ambients_~Ambients.c`/`.asm`. Frees each owned `AmbientsGroup*` via
    /// plain vanilla `operator_delete` (no `~AmbientsGroup` call, see the module doc comment), then
    /// returns the group-pointer array itself to vanilla's shared small-object freelist or
    /// `operator_delete`, by **capacity** (`groups_cap - groups_begin`), not the used range, matching
    /// vanilla's own `param_1[2] - puVar2` exactly (both are equal in this port's exact-fit allocation,
    /// but the real field is preserved for fidelity).
    pub fn destruct(&mut self) {
        let mut p = self.groups_begin;
        while p != self.groups_end {
            let group = get_from_memory::<u32>(p);
            if group != 0 {
                unsafe { OPERATOR_DELETE.original()(group) };
            }
            p += 4;
        }
        if self.groups_begin != 0 {
            free_group_array_buffer(self.groups_begin, self.groups_cap - self.groups_begin);
        }
    }

    /// Reimplementation of `Ambients::play` (`0x0043f445`, `generated.rs`'s `ambients::PLAY`), per
    /// `Ambients_play.c`/`.asm`: walks the group vector from `groups_begin` to `groups_end` and
    /// forwards to the first group whose `[range_lo, range_hi]` band contains `level`
    /// ([`group_matches_level`]), passing `delta` through untouched and `&self.x` (the `x`/`y`/`z`
    /// position triple at `this+0xc`, already jittered in place by `ztsoundscape.rs`'s `update` before
    /// this is called) as the position pointer - matching the `.asm`'s `ADD %EDI, 0xc` before the call.
    /// A miss (empty vector, or no band contains `level`) is a silent no-op, vanilla-faithfully; no null
    /// guard on the group pointer either, matching the `.asm`'s unconditional `[ESI]` deref.
    pub fn play(&self, delta: i32, level: i32) {
        let mut p = self.groups_begin;
        while p != self.groups_end {
            let group = unsafe { &*(get_from_memory::<u32>(p) as *const AmbientsGroup) };
            if group_matches_level(group.range_lo, group.range_hi, level) {
                group.play(delta, &self.x as *const i32);
                return;
            }
            p += 4;
        }
    }
}

/// The vanilla `AmbientsGroup` class (`0x20c` bytes, confirmed allocation size - `Ambients::Ambients`
/// calls `operator_new(0x20c)` per group, see [`Ambients::build_groups_from_ini`]). One config-driven level band
/// (e.g. "quiet"/"medium"/"large"): a `(range_lo, range_hi)` guest-count band plus a `SoundGroup*`
/// **borrowed** from `BFSndMgr`'s own get-or-create pool (keyed by `(section, chance, sound-list,
/// probability-list)` - see [`Self::construct`]), never owned - no `~AmbientsGroup` exists anywhere in
/// the corpus, and there is nothing else in the class for a destructor to free once the name buffer and
/// the two range ints are accounted for. [`Ambients::destruct`] frees each block directly via vanilla
/// `operator_delete`, leaving the borrowed `SoundGroup*` untouched.
#[repr(C)]
pub struct AmbientsGroup {
    /// Embedded null-terminated level-name string (e.g. `"quiet"`), copied byte-for-byte
    /// (`strlen(name) + 1` bytes, matching vanilla's `REPNE SCASB` + `REP MOVSD`/`MOVSB` copy exactly -
    /// see `AmbientsGroup_AmbientsGroup.asm:12-27`) into a `0x200`-byte fixed buffer whose tail past the
    /// copied prefix stays whatever `operator_new` left there - not zeroed, matching every other
    /// un-zeroed `operator_new` allocation elsewhere in this codebase. No decompiled method reads this
    /// buffer back (write-only from what's observable); reproduced anyway in case an un-decompiled
    /// reader exists.
    name: [u8; 0x200], // 0x000
    range_lo: i32, // 0x200 - guest-count band lower bound
    range_hi: i32, // 0x204 - guest-count band upper bound
    /// Borrowed, not owned - see the struct doc comment.
    sound_group: u32, // 0x208 - *const SoundGroup
}

const _: () = assert!(std::mem::size_of::<AmbientsGroup>() == 0x20c);

impl AmbientsGroup {
    /// Reimplementation of `AmbientsGroup::AmbientsGroup` (`0x0045067e`), per
    /// `AmbientsGroup_AmbientsGroup.c`/`.asm` (the `.asm` is ground truth for the real argument order -
    /// the `.c` mistypes `range_hi` as `int**` and hides the `getStringList`/`getIntList` calls' third
    /// stack argument entirely, see [`CHANCE_KEY_RVA`]'s doc comment). `config` is an already-`attempt`ed
    /// real vanilla `BFConfigFile`; `name` is this group's level name (e.g. `"quiet"`), read as this
    /// group's own config **section**; `range_lo`/`range_hi` are the guest-count band from the caller's
    /// `"ambientlevels"`/`"value"` int list. This is the real-`BFConfigFile`-based path kept for the
    /// hooked `ambientsgroup::CONSTRUCTOR` address and the live battery's own real-vanilla-pole
    /// trampoline - see the module doc comment's "two construction paths" section for
    /// [`Self::construct_from_ini`], the path `Ambients::build_groups_from_ini` actually calls.
    ///
    /// `sound_buf`/`prob_buf` (the `getStringList`/`getIntList` result vectors) are freed at the tail
    /// via [`VECTOR_T_4`], not [`DEALLOCATE_VECTOR`] - real vanilla frees these two specifically through
    /// a different function than it uses for `Ambients::Ambients`'s own `"ambientlevels"` list buffers
    /// (see both constants' doc comments); the `VECTORINT_0`/`VECTORINT_1` copies built from
    /// them are deliberately never freed here, matching vanilla (absorbed by `NEW_SOUND_GROUP` by value).
    pub fn construct(&mut self, config: *const u32, name: *const u8, range_lo: i32, range_hi: i32) {
        self.range_lo = range_lo;
        self.range_hi = range_hi;

        let name_len = unsafe { CStr::from_ptr(name as *const i8) }.to_bytes().len();
        let copy_len = name_len.min(self.name.len() - 1);
        unsafe { std::ptr::copy_nonoverlapping(name, self.name.as_mut_ptr(), copy_len + 1) };

        let base = get_module_base("zoo.exe") as u32;
        let name_section = name as *const i8;

        // `chance`: vanilla reuses a dead incoming-argument stack slot as scratch here, so a missing
        // key would leave that slot's leftover bits behind rather than a clean default - not
        // reproduced; `0` is this port's default instead, matching the all-absent config case the
        // plan document's live capture actually observed (chance == 0 alongside every other field null).
        let mut chance: i32 = 0;
        unsafe { GET_INT.original()(config, name_section as u32, base + CHANCE_KEY_RVA, &raw mut chance as *const u32) };

        let mut sound_buf = [0u32; 3];
        let mut prob_buf = [0u32; 3];
        unsafe {
            GET_STRING_LIST.original()(config, sound_buf.as_mut_ptr(), name_section, (base + SOUND_KEY_RVA) as *const i8);
            GET_INT_LIST.original()(config, prob_buf.as_mut_ptr(), name_section, (base + PROB_KEY_RVA) as *const i8);
        }
        // `NEW_SOUND_GROUP` takes fresh copies of both lists - vanilla's own transient `sound_buf`/
        // `prob_buf` above get freed right after this call, unconditionally (below) - built via the
        // same `vector<int>`-copy-constructor calls vanilla makes, into scratch this function owns.
        let mut sound_copy = [0u32; 3];
        let mut prob_copy = [0u32; 3];
        unsafe {
            VECTORINT_0.original()(sound_copy.as_mut_ptr() as *const i32, sound_buf.as_ptr() as *const i32);
            VECTORINT_1.original()(prob_copy.as_mut_ptr() as *const i32, prob_buf.as_ptr() as *const i32);
        }

        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        self.sound_group = unsafe {
            NEW_SOUND_GROUP.original()(
                sndmgr as *const u32,
                chance as u32,
                sound_copy[0] as *const u32,
                sound_copy[1] as *const u32,
                sound_copy[2] as *const u32,
                prob_copy[0] as *const u32,
                prob_copy[1] as *const u32,
                prob_copy[2] as *const u32,
            ) as u32
        };

        // Real vanilla frees `sound_buf`/`prob_buf` unconditionally here, prob first then sound
        // (`AmbientsGroup_AmbientsGroup.asm`'s own tail) - not `DEALLOCATE_VECTOR`, and not guarded by
        // a non-null check: `VECTOR_T_4` is a self-checking destructor-shaped free that no-ops safely
        // on an all-zero (never-populated) struct.
        unsafe {
            VECTOR_T_4.original()(prob_buf.as_ptr());
            VECTOR_T_4.original()(sound_buf.as_ptr());
        }
    }

    /// `Ini`-backed counterpart to [`Self::construct`], used only by
    /// [`Ambients::build_groups_from_ini`]'s own Rust-to-Rust call path - see the module doc comment's
    /// "two construction paths" section for why this exists alongside, rather than instead of,
    /// [`Self::construct`]. `chance`/`sound`/`prob` are read from `ini`'s `name`-named section, matching
    /// real vanilla's own per-level-section convention (`ini_compat::first_parse`/`word_list`, the
    /// `Ini`-backed equivalents of `getInt`/`getStringList`/`getIntList`).
    ///
    /// `NEW_SOUND_GROUP` is itself real, un-ported `BFSndMgr` pool code, so it still needs real
    /// vanilla-shaped `vector<char*>`/`vector<int>` arguments - [`build_vanilla_string_array`]/
    /// [`build_vanilla_int_array`] build fresh ones from the `Ini`-parsed data rather than reusing real
    /// vanilla's own `getStringList`/`getIntList` output the way [`Self::construct`] does.
    /// `BFSndMgr_newSoundGroup.c`'s own body only ever reads the string array's contents (copying them
    /// into a pool cache key) and never retains the raw pointers past the call: real vanilla's own
    /// equivalent strings live inside the caller's local scratch `BFConfigFile`'s tree, released only
    /// once every group in the file has been built, which would be a use-after-free if `newSoundGroup`
    /// kept them - so it is equally safe to free ours once the call returns. The array buffers
    /// themselves need no explicit free on the success path: `NEW_SOUND_GROUP`'s own internal
    /// `vector_t_4` destructors consume them by value, exactly as they do the `VECTORINT_0`/
    /// `VECTORINT_1` copies [`Self::construct`] builds.
    pub fn construct_from_ini(&mut self, ini: &Ini, name: &str, range_lo: i32, range_hi: i32) {
        self.range_lo = range_lo;
        self.range_hi = range_hi;

        let name_bytes = name.as_bytes();
        let copy_len = name_bytes.len().min(self.name.len() - 1);
        unsafe {
            std::ptr::copy_nonoverlapping(name_bytes.as_ptr(), self.name.as_mut_ptr(), copy_len);
            self.name[copy_len] = 0;
        }

        let chance: i32 = bfconfigfile::ini_compat::first_parse(ini, name, "chance").unwrap_or(0);
        let sound_names = bfconfigfile::ini_compat::word_list(ini, name, "sound");
        let probs: Vec<i32> = bfconfigfile::ini_compat::word_list(ini, name, "prob").iter().filter_map(|v| v.parse().ok()).collect();

        self.sound_group = 0;
        let Some((sound_array, sound_ptrs)) = build_vanilla_string_array(&sound_names) else {
            return;
        };
        let Some(prob_array) = build_vanilla_int_array(&probs) else {
            free_vanilla_strings(&sound_ptrs);
            if sound_array[0] != 0 {
                unsafe { OPERATOR_DELETE.original()(sound_array[0]) };
            }
            return;
        };

        let base = get_module_base("zoo.exe") as u32;
        let sndmgr: u32 = get_from_memory(base + GLOBAL_DX8SNDMGR_RVA);
        self.sound_group = unsafe {
            NEW_SOUND_GROUP.original()(
                sndmgr as *const u32,
                chance as u32,
                sound_array[0] as *const u32,
                sound_array[1] as *const u32,
                sound_array[2] as *const u32,
                prob_array[0] as *const u32,
                prob_array[1] as *const u32,
                prob_array[2] as *const u32,
            ) as u32
        };

        // `NEW_SOUND_GROUP`'s own internal `vector_t_4` destructors already freed both arrays'
        // backing buffers (see this method's doc comment); the per-string buffers are still ours.
        free_vanilla_strings(&sound_ptrs);
    }

    /// Reimplementation of `AmbientsGroup::play` (`0x0043f4d5`), per `AmbientsGroup_play.c` - a
    /// one-line forward through the borrowed `SoundGroup*` at `+0x208`. `delta` is the tick-delta
    /// [`Ambients::play`] received as its own first argument (forwarded through untouched, not the
    /// level it range-matched on). Not yet detoured; real vanilla callers still reach vanilla through
    /// `ambientsgroup::PLAY.original()` today.
    pub fn play(&self, delta: i32, position: *const i32) {
        unsafe { SOUNDGROUP_PLAY.original()(self.sound_group as *const u32, delta, position as u32) };
    }
}

/// Builds a fresh, exact-fit, real-vanilla `vector<char*>`-shaped `(begin, end, cap)` triple from a
/// list of Rust strings for [`AmbientsGroup::construct_from_ini`] to hand to `NEW_SOUND_GROUP` - one
/// freshly `operator_new`'d, null-terminated `char*` buffer per string, plus a freshly `operator_new`'d
/// pointer array holding them (mirroring [`Ambients::build_groups_from_ini`]'s own exact-fit
/// group-pointer array). An empty `strings` returns the all-null triple matching a default-constructed,
/// never-grown vanilla `vector<T>` - no allocation needed, and `NEW_SOUND_GROUP`'s own begin/end walk
/// treats it identically to any other zero-length vector.
///
/// Returns `None` (having already freed anything it did allocate) if any `operator_new` call fails.
/// The returned `Vec<u32>` is the caller's own record of the individual string-buffer pointers - see
/// [`AmbientsGroup::construct_from_ini`]'s doc comment for why the caller, not this function, frees
/// them, and only after `NEW_SOUND_GROUP` has read them.
fn build_vanilla_string_array(strings: &[String]) -> Option<([u32; 3], Vec<u32>)> {
    let mut ptrs: Vec<u32> = Vec::with_capacity(strings.len());
    for s in strings {
        let bytes = s.as_bytes();
        let buf = unsafe { OPERATOR_NEW.original()(bytes.len() as u32 + 1) } as u32;
        if buf == 0 {
            free_vanilla_strings(&ptrs);
            return None;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
            *((buf + bytes.len() as u32) as *mut u8) = 0;
        }
        ptrs.push(buf);
    }
    if ptrs.is_empty() {
        return Some(([0, 0, 0], ptrs));
    }
    let byte_size = (ptrs.len() * 4) as u32;
    let array = unsafe { OPERATOR_NEW.original()(byte_size) } as u32;
    if array == 0 {
        free_vanilla_strings(&ptrs);
        return None;
    }
    for (i, &p) in ptrs.iter().enumerate() {
        save_to_memory(array + (i as u32) * 4, p);
    }
    Some(([array, array + byte_size, array + byte_size], ptrs))
}

/// Frees every per-string buffer [`build_vanilla_string_array`] allocated - not the array itself (see
/// that function's and [`AmbientsGroup::construct_from_ini`]'s doc comments for who owns which part).
fn free_vanilla_strings(ptrs: &[u32]) {
    for &p in ptrs {
        unsafe { OPERATOR_DELETE.original()(p) };
    }
}

/// Builds a fresh, exact-fit, real-vanilla `vector<int>`-shaped `(begin, end, cap)` triple from a list
/// of ints for [`AmbientsGroup::construct_from_ini`] - the `vector<int>` counterpart to
/// [`build_vanilla_string_array`]. Unlike that function, callers never need to free anything themselves
/// on the success path: `NEW_SOUND_GROUP`'s own internal `vector_t_4` destructor frees the array (see
/// [`AmbientsGroup::construct_from_ini`]'s doc comment).
fn build_vanilla_int_array(ints: &[i32]) -> Option<[u32; 3]> {
    if ints.is_empty() {
        return Some([0, 0, 0]);
    }
    let byte_size = (ints.len() * 4) as u32;
    let array = unsafe { OPERATOR_NEW.original()(byte_size) } as u32;
    if array == 0 {
        return None;
    }
    for (i, &v) in ints.iter().enumerate() {
        save_to_memory(array + (i as u32) * 4, v as u32);
    }
    Some([array, array + byte_size, array + byte_size])
}

/// Hooks both classes' fixed addresses onto the Rust methods above, so any caller reaching those
/// addresses - `ztsoundscape.rs`'s own direct Rust calls included, since it no longer routes through
/// these addresses at all - runs this port. `ambients::DESTRUCTOR` is included (unlike
/// `ztsoundscape.rs`'s own destructor, which stays un-detoured): it is a plain, decision-free teardown
/// (see the module doc comment's `Ambients::~Ambients` note), the same category of destructor the
/// `MenuMusicHandler`/`ZTMegatileMgr`/`ZTAdvTerrainMgr` precedent hooks rather than leaves un-detoured.
/// `AmbientsGroup` has no destructor to hook (none exists anywhere in the corpus, see [`AmbientsGroup`]'s
/// struct doc comment).
#[detour_mod]
mod ambients_detours {
    use std::ffi::c_void;

    use openzt_detour::generated::ambients::{
        CONSTRUCTOR as AMBIENTS_CONSTRUCTOR, DESTRUCTOR as AMBIENTS_DESTRUCTOR, PLAY as AMBIENTS_PLAY,
    };
    use openzt_detour::generated::ambientsgroup::{
        CONSTRUCTOR as AMBIENTSGROUP_CONSTRUCTOR, PLAY as AMBIENTSGROUP_PLAY,
    };

    use super::*;

    #[detour(AMBIENTS_CONSTRUCTOR)]
    unsafe extern "thiscall" fn ambients_constructor(this: *const u32, name: *const u32, position: *const u32) -> *const u32 {
        unsafe { mut_from_memory::<Ambients>(this) }.construct(name as *const u8, position as *const i32);
        this
    }

    #[detour(AMBIENTS_PLAY)]
    unsafe extern "thiscall" fn ambients_play(this: *const u32, delta: i32, level: i32) {
        unsafe { ref_from_memory::<Ambients>(this) }.play(delta, level);
    }

    #[detour(AMBIENTS_DESTRUCTOR)]
    unsafe extern "fastcall" fn ambients_destructor(this: *const c_void) {
        unsafe { mut_from_memory::<Ambients>(this) }.destruct();
    }

    /// Signature per `generated.rs`'s `ambientsgroup::CONSTRUCTOR` (see [`AmbientsGroup::construct`]'s
    /// doc comment for why `config`/`name` are untyped `c_void` pointers and `range_hi` is mistyped
    /// `*const i32` rather than a plain scalar - both regenerated wart, casts happen here at the call
    /// site into the correctly-typed Rust method).
    #[detour(AMBIENTSGROUP_CONSTRUCTOR)]
    unsafe extern "thiscall" fn ambientsgroup_constructor(
        this: *const u32,
        config: *const c_void,
        name: *const c_void,
        range_lo: u32,
        range_hi: *const i32,
    ) -> *const u32 {
        unsafe { mut_from_memory::<AmbientsGroup>(this) }.construct(config as *const u32, name as *const u8, range_lo as i32, range_hi as i32);
        this
    }

    #[detour(AMBIENTSGROUP_PLAY)]
    unsafe extern "thiscall" fn ambientsgroup_play(this: *const u32, delta: i32, position: *const i32) {
        unsafe { ref_from_memory::<AmbientsGroup>(this) }.play(delta, position);
    }

    /// Live-test access to the real vanilla bodies - same
    /// per-profile rationale as `ztsoundscape.rs`'s own `soundscape_detours::test_real` (`.original()`
    /// on a hooked address is a raw cast in release, so it would re-enter these detours instead of
    /// reaching vanilla; `*_DETOUR.call` stays correct in every profile). Lives inside the detour
    /// module because the generated `*_DETOUR` statics are module-private.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn ambients_constructor(this: *const u32, name: *const u32, position: *const u32) -> *const u32 {
            unsafe { super::AMBIENTS_CONSTRUCTOR_DETOUR.call(this, name, position) }
        }

        pub(crate) fn ambients_play(this: *const u32, delta: i32, level: i32) {
            unsafe { super::AMBIENTS_PLAY_DETOUR.call(this, delta, level) }
        }

        pub(crate) fn ambients_destructor(this: *const c_void) {
            unsafe { super::AMBIENTS_DESTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn ambientsgroup_constructor(
            this: *const u32,
            config: *const c_void,
            name: *const c_void,
            range_lo: u32,
            range_hi: *const i32,
        ) -> *const u32 {
            unsafe { super::AMBIENTSGROUP_CONSTRUCTOR_DETOUR.call(this, config, name, range_lo, range_hi) }
        }

        pub(crate) fn ambientsgroup_play(this: *const u32, delta: i32, position: *const i32) {
            unsafe { super::AMBIENTSGROUP_PLAY_DETOUR.call(this, delta, position) }
        }
    }
}

/// Registers this module's live detours - see [`ambients_detours`]'s doc comment for which entries and
/// why.
pub fn init() {
    if let Err(e) = unsafe { ambients_detours::init_detours() } {
        error!("Failed to initialise ambients detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use std::ffi::c_void;

    use super::*;

    /// `(name, is_enabled)` per detour - see `ambients_detours::status`.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        ambients_detours::status()
    }

    /// Allocates a fresh, uninitialized `0x18`-byte block via the real vanilla allocator - mirrors
    /// `ztsoundscape.rs`'s own `allocate_uninitialized`. Callers must run either
    /// [`real_construct`]/[`real_ambientsgroup_construct`] or [`Ambients::construct`]/
    /// [`AmbientsGroup::construct`] before reading any field.
    pub(crate) fn allocate_uninitialized_ambients() -> *mut Ambients {
        unsafe { OPERATOR_NEW.original()(0x18) as *mut Ambients }
    }

    /// Frees a standalone `Ambients` block built via [`allocate_uninitialized_ambients`] and
    /// constructed on the **real vanilla** pole (via [`real_construct`]), through the real
    /// destructor trampoline followed by vanilla `operator delete` on the outer block - vanilla
    /// allocator both sides, per `CLAUDE.md`'s cross-allocator rule.
    pub(crate) fn destroy_real_ambients(ptr: *mut Ambients) {
        if ptr.is_null() {
            return;
        }
        ambients_detours::test_real::ambients_destructor(ptr as *const c_void);
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Frees a standalone `Ambients` block constructed on the **Rust** pole (via
    /// [`Ambients::construct`]), through [`Ambients::destruct`] followed by vanilla
    /// `operator delete` - same cross-allocator contract as [`destroy_real_ambients`].
    pub(crate) fn destroy_reimpl_ambients(ptr: *mut Ambients) {
        if ptr.is_null() {
            return;
        }
        unsafe { (*ptr).destruct() };
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Trampoline to the real vanilla `Ambients::Ambients` body for the battery's "real vanilla"
    /// pole once `reimplementation_tests::init()` has installed this module's detours -
    /// `.original()` on `ambients::CONSTRUCTOR` is a raw cast in release, so it would re-enter the
    /// Rust detour there (see `ambients_detours::test_real`'s doc comment).
    pub(crate) fn real_construct(this: *mut Ambients, name: *const u8, position: *const i32) {
        ambients_detours::test_real::ambients_constructor(this as *const u32, name as *const u32, position as *const u32);
    }

    /// Trampoline to the real vanilla `Ambients::play` body - same per-profile rationale as
    /// [`real_construct`].
    pub(crate) fn real_play(this: *const Ambients, delta: i32, level: i32) {
        ambients_detours::test_real::ambients_play(this as *const u32, delta, level);
    }

    /// Allocates a fresh, uninitialized `0x20c`-byte block via the real vanilla allocator - the
    /// `AmbientsGroup`-level counterpart to [`allocate_uninitialized_ambients`]. Callers must run
    /// either [`real_ambientsgroup_construct`] or [`AmbientsGroup::construct`] before reading any
    /// field.
    pub(crate) fn allocate_uninitialized_ambientsgroup() -> *mut AmbientsGroup {
        unsafe { OPERATOR_NEW.original()(0x20c) as *mut AmbientsGroup }
    }

    /// Frees a standalone `AmbientsGroup` block built via [`allocate_uninitialized_ambientsgroup`] -
    /// plain vanilla `operator_delete`, no per-class destructor on either pole (see [`AmbientsGroup`]'s
    /// struct doc comment: no `~AmbientsGroup` exists anywhere in the corpus).
    pub(crate) fn destroy_ambientsgroup(ptr: *mut AmbientsGroup) {
        if ptr.is_null() {
            return;
        }
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Trampoline to the real vanilla `AmbientsGroup::AmbientsGroup` body for the battery's "real
    /// vanilla" pole - same per-profile rationale as [`real_construct`]. `range_hi` is taken as a plain
    /// `i32` here and re-cast to the detour's own mistyped `*const i32` parameter at the call site (see
    /// `ambients_detours::ambientsgroup_constructor`'s doc comment for why that parameter is mistyped).
    pub(crate) fn real_ambientsgroup_construct(this: *mut AmbientsGroup, config: *const u32, name: *const u8, range_lo: i32, range_hi: i32) {
        ambients_detours::test_real::ambientsgroup_constructor(
            this as *const u32,
            config as *const c_void,
            name as *const c_void,
            range_lo as u32,
            range_hi as *const i32,
        );
    }

    /// Releases a scratch `BFConfigFile` built the same way [`Ambients::construct`] builds its own
    /// (real `CONFIG_CONSTRUCTOR` + `ATTEMPT_0`) - thin wrapper exposing the private top-level
    /// [`release_scratch_config`] to `reimplementation_tests`.
    pub(crate) fn release_scratch_config(config_ptr: *const u32) {
        super::release_scratch_config(config_ptr)
    }

    /// Reads the first `"ambientlevels"` entry (name pointer, `range_lo`, `range_hi`) off an
    /// already-successfully-`attempt`-ed `BFConfigFile` via the same real `GET_STRING_LIST`/
    /// `GET_INT_LIST` calls real vanilla `Ambients::Ambients` makes over the `"group"`/`"value"`
    /// sections (production code reads this same data through `bfconfigfile::ini_compat` instead - see
    /// [`Ambients::build_groups_from_ini`] - this test helper stays real-`BFConfigFile`-based since its
    /// caller in `reimplementation_tests` needs a real config to feed [`AmbientsGroup::construct`]'s own
    /// real-vanilla-pole comparison), reading only the first entry rather than building every group.
    /// `None` if the config has no `"ambientlevels"` entries at all.
    /// The returned name pointer aliases the config's own live tree-node memory (not owned by the
    /// caller); the `GET_STRING_LIST`/`GET_INT_LIST` output-vector buffers themselves are freed here via
    /// [`DEALLOCATE_VECTOR`], which does not free the strings/ints the name pointer and range values
    /// point at.
    pub(crate) fn first_ambient_level(config_ptr: *const u32) -> Option<(*const u8, i32, i32)> {
        let mut string_buf = [0u32; 3];
        let mut int_buf = [0u32; 3];
        unsafe {
            let base = get_module_base("zoo.exe") as u32;
            let key = (base + AMBIENTLEVELS_KEY_RVA) as *const i8;
            GET_STRING_LIST.original()(config_ptr, string_buf.as_mut_ptr(), key, (base + GROUP_SECTION_RVA) as *const i8);
            GET_INT_LIST.original()(config_ptr, int_buf.as_mut_ptr(), key, (base + VALUE_SECTION_RVA) as *const i8);
        }
        let names = ZTArray::<u32>::from_raw_parts(string_buf[0], string_buf[1], string_buf[2]);
        let ranges = ZTArray::<u32>::from_raw_parts(int_buf[0], int_buf[1], int_buf[2]);
        let count = names.len();
        let result = if count == 0 { None } else { Some((names.get_ptr(0) as *const u8, ranges.get_ptr(0) as i32, ranges.get_ptr(1) as i32)) };

        unsafe {
            if string_buf[0] != 0 {
                DEALLOCATE_VECTOR.original()(string_buf[0] as *const u32, count as i32);
            }
            if int_buf[0] != 0 {
                DEALLOCATE_VECTOR.original()(int_buf[0] as *const u32, ranges.len() as i32);
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`freelist_bucket_index`] against `Ambients_~Ambients.asm`'s own `(byte_capacity - 1) >> 3`,
    /// including every bucket-boundary byte capacity below the `0x80` `operator_delete` cutoff
    /// [`free_group_array_buffer`] guards separately.
    #[test]
    fn freelist_bucket_index_matches_the_vanilla_shift() {
        assert_eq!(freelist_bucket_index(4), 0);
        assert_eq!(freelist_bucket_index(8), 0);
        assert_eq!(freelist_bucket_index(9), 1);
        assert_eq!(freelist_bucket_index(16), 1);
        assert_eq!(freelist_bucket_index(17), 2);
        assert_eq!(freelist_bucket_index(0x80), 0xf);
    }

    /// [`should_use_operator_delete`] at the exact `0x80` boundary `Ambients_~Ambients.asm`'s own
    /// `byte_capacity > 0x80` branch tests.
    #[test]
    fn should_use_operator_delete_matches_the_vanilla_boundary() {
        assert!(!should_use_operator_delete(0x80), "at the boundary: freelist");
        assert!(should_use_operator_delete(0x81), "just past the boundary: operator_delete");
    }

    /// [`AmbientsGroup`]'s field offsets against `AmbientsGroup_AmbientsGroup.asm`'s own writes
    /// (`this+0x200`/`+0x204`/`+0x208`), independent of the `0x20c` size assert alone.
    #[test]
    fn ambientsgroup_field_offsets_match_the_vanilla_layout() {
        use std::mem::offset_of;
        assert_eq!(offset_of!(AmbientsGroup, name), 0x000);
        assert_eq!(offset_of!(AmbientsGroup, range_lo), 0x200);
        assert_eq!(offset_of!(AmbientsGroup, range_hi), 0x204);
        assert_eq!(offset_of!(AmbientsGroup, sound_group), 0x208);
    }

    /// [`group_matches_level`] against `Ambients_play.c`'s miss condition (`level < range_lo ||
    /// range_hi < level`), at both band boundaries, just outside them, and over an inverted
    /// (`range_lo > range_hi`, always-miss) band.
    #[test]
    fn group_matches_level_matches_the_vanilla_band_check() {
        assert!(group_matches_level(10, 20, 10), "inclusive lower bound");
        assert!(group_matches_level(10, 20, 20), "inclusive upper bound");
        assert!(group_matches_level(10, 20, 15), "mid-band");
        assert!(!group_matches_level(10, 20, 9), "just below");
        assert!(!group_matches_level(10, 20, 21), "just above");
        assert!(group_matches_level(5, 5, 5), "single-value band");
        assert!(!group_matches_level(20, 10, 15), "inverted band never matches");
    }
}
