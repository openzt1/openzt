//! `BFEntity`'s constructor (`0x412693`) and non-deleting destructor (`0x41df2e`), the base of every entity
//! class: `BFUnit`, `BFOverlay` and `ZTScenery` call them first/last, and `BFEntityType::createEntity` builds a
//! bare `BFEntity` with the constructor. The object is `0x154`
//! bytes and is still read and written by un-ported vanilla code, so it keeps vanilla's layout and is built in
//! place (style 1). Every string, vector and container inside is built through vanilla's own helpers, except the
//! embedded behavior set's strings, vectors and parsed call object, whose buffers are Rust-allocated
//! (`owned_strings.rs`, `bfscriptmgr.rs`).
//!
//! ## Layout written by the constructor
//! `+0x00` vtable (`BFEventListenerInterface`'s while building, `BFEntity`'s at the end, since derived
//! constructors overwrite it afterwards); `+0x04..+0x10` a `vector<BFEvent>`; `+0x14` the embedded
//! `BFBehaviorSet` (`0x60` bytes, see `bfbehaviorset.rs`); `+0x74` a `GXMixer` (`0x68` bytes); `+0xdc` a
//! `BFBSCall` (a string with a Rust-owned buffer plus the flag byte at `+0xe8`; see `set_pending_bs`); `+0xf8`/`+0xf9` flag bytes (0 and 1); `+0xfc` a string left as three zero words and never
//! constructed; `+0x108` a byte vector of capacity 8 whose first byte is 0; `+0x114..+0x124` position and
//! height; `+0x128` the entity type; `+0x13c..+0x146` flag bytes.
//!
//! ## Differences from vanilla
//! - Bytes vanilla does not write (`+0x68`, `+0x141`, `+0x147..+0x154`, the tails of the helper objects) keep
//!   whatever the allocation held.
//!
//! ## Destructor
//! Sets the base vtable (so virtual calls during teardown reach `BFEntity`), frees the strings at `+0x108`,
//! `+0xfc` and the `BFBSCall`'s at `+0xdc`, tears down the `GXMixer`, then the embedded set
//! (`bfbehaviorset::destroy_set_members`) and the `vector<BFEvent>` at `+0x04`. The mixer teardown is inlined in
//! vanilla: the `deque<GXMixerChain>` is cleared between its first and last iterators, then freed; the vector at
//! `+0x80` is destroyed; the vector at `+0x74` has the reference count (`+0x20`) of each non-null entry
//! decremented before its buffer is freed. Everything is freed through vanilla's own helpers.

use openzt_detour::generated::{
    bfentity::{CLEAR_BS, CLEAR_QUEUE, CONSTRUCTOR, DESTRUCTOR_0, LOAD, SAVE, SET_BS, SET_BSNEW, SET_NAME, SET_WORLD_POS},
    gxmixer::CONSTRUCTOR as GXMIXER_CONSTRUCTOR,
    msvc_std_basic_string::BASIC_STRING_1,
    standalone::CRT_ISMBCGRAPH,
    msvc_std_dequegxmixerchain::{DESTROY_RANGE_0X411CAD, DESTRUCTOR_0X41DECC},
    msvc_std_vector_t_28::VECTOR_T_28,
    msvc_std_vectorgxmixerlink::{DESTRUCTOR_0X411C0F, FREE_BUFFER_0X402970},
    ztanimal::SET_PENDING_BS_0X4300F6,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfbehaviorset::{clear_string, construct_set_members, destroy_set_members, BfBehaviorSet},
    bfscriptmgr::{clear_call_args, read_file_bytes, reset_call, write_file_bytes, write_file_string, RawString},
    owned_strings::{free_buffer, string_construct, string_fill_new, string_release, string_assign, StringSlot},
    util::{get_from_memory, ref_from_memory},
    zthabitat::habitat::name_has_meaningful_byte,
    vanilla_string::VanillaString,
    write_live,
};

pub(crate) const BFENTITY_SIZE: usize = 0x154;
const VTABLE_BASE: u32 = 0x0063_21a4;
const VTABLE_BFENTITY: u32 = 0x0062_d4b4;

const EMBEDDED_SET: u32 = 0x14;
const MIXER: u32 = 0x74;
const BS_CALL: u32 = 0xdc;
const NAME_VECTOR: u32 = 0x108;

unsafe fn write<T>(address: u32, value: T) {
    unsafe { (address as *mut T).write_volatile(value) };
}

/// Builds a `BFEntity` in place at `this` (`0x154` bytes of uninitialised memory) for `entity_type`.
pub(crate) unsafe fn construct_entity(this: u32, entity_type: u32) {
    unsafe {
        write(this, VTABLE_BASE);
        for offset in [0x04u32, 0x08, 0x0c] {
            write(this + offset, 0u32);
        }
        write(this + 0x10, 0u8);
        write(this + 0x11, 0u8);

        construct_set_members(this + EMBEDDED_SET);

        GXMIXER_CONSTRUCTOR.original()((this + MIXER) as *const u32);

        // The pending `BFBSCall` is an empty string of capacity 8 and a flag byte (`+0xe8`) of 1. Its buffer is
        // Rust-owned: only this module and `set_pending_bs` write the string, and vanilla only truncates it.
        string_construct((this + BS_CALL) as StringSlot, 8);
        write(this + BS_CALL + 0xc, 1u8);

        write(this + 0xec, 0u32);
        write(this + 0xf4, 0u32);
        write(this + 0xf8, 0u8);
        write(this + 0xf9, 1u8);
        for offset in [0xfcu32, 0x100, 0x104] {
            write(this + offset, 0u32);
        }

        // The name is an empty byte string of capacity 8 with a Rust-owned buffer; `set_name` and `load` are its only
        // writers.
        string_construct((this + NAME_VECTOR) as StringSlot, 8);

        for offset in [0x114u32, 0x118, 0x11c, 0x120] {
            write(this + offset, 0u32);
        }
        write(this + 0x128, entity_type);
        write(this + 0x130, 0u32);
        write(this + 0x138, 0u32);
        for offset in [0x13cu32, 0x13d, 0x142, 0x143, 0x144, 0x145, 0x146] {
            write(this + offset, 0u8);
        }
        for offset in [0x13eu32, 0x13f, 0x140] {
            write(this + offset, 1u8);
        }
        write(this, VTABLE_BFENTITY);
    }
}

unsafe fn read(address: u32) -> u32 {
    unsafe { (address as *const u32).read_volatile() }
}

/// Tears down the `BFEntity` at `this` without freeing it.
pub(crate) unsafe fn destroy_entity(this: u32) {
    unsafe {
        write(this, VTABLE_BFENTITY);

        string_release((this + NAME_VECTOR) as StringSlot);
        for (begin_offset, capacity_offset) in [(0xfcu32, 0x104)] {
            let begin = read(this + begin_offset);
            BASIC_STRING_1.original()(begin as *const u32, read(this + capacity_offset).wrapping_sub(begin));
        }

        string_release((this + BS_CALL) as StringSlot);

        let mixer = this + MIXER;
        let deque = mixer + 0x18;
        // `0x411cad` clears the deque's elements between its first and last iterators, passed by value (four
        // words each).
        DESTROY_RANGE_0X411CAD.original()(
            read(deque),
            read(deque + 4),
            read(deque + 8),
            read(deque + 0xc),
            read(deque + 0x10),
            read(deque + 0x14),
            read(deque + 0x18),
            read(deque + 0x1c),
        );
        DESTRUCTOR_0X41DECC.original()(deque as *const u32);
        DESTRUCTOR_0X411C0F.original()((mixer + 0xc) as *const u32);

        let mut entry = read(mixer);
        let end = read(mixer + 4);
        while entry != end {
            let link = read(entry);
            if link != 0 {
                write(link + 0x20, read(link + 0x20).wrapping_sub(1));
            }
            entry += 12;
        }
        FREE_BUFFER_0X402970.original()(mixer as *const u32);

        destroy_set_members(this + EMBEDDED_SET);
        VECTOR_T_28.original()((this + 4) as *const u32);
    }
}

/// `BFEntity::setBS(name, callFlag, force, clearMode)`: replaces the entity's current behavior set with
/// `name`. Unless a finished, unforced set is already running (`dirty` set, `+0x6c` clear, `force` 0), it tears
/// down the child, call list, strings and parsed call, resets the flags, assigns the name (the entity's `+0x6c`
/// byte becomes `call_flag`), loads the script when the name is non-empty, clears the mixer by `clear_mode`
/// (`0` total, `1` everything, else untouched) and marks the set pending when forced. Returns whether the
/// script loaded. `name` is passed by value and its buffer is freed to the pool on return, as vanilla does.
pub(crate) unsafe fn set_bs(this: u32, name: RawString, call_flag: u8, force: u8, clear_mode: i32) -> bool {
    let mut loaded = false;
    let set = unsafe { ref_from_memory::<BfBehaviorSet>(this + EMBEDDED_SET) };
    if set.dirty == 0 || set.flag_58 != 0 || force != 0 {
        write_live!(set, dirty, 0u8);
        unsafe {
            set.kill_child();
            set.erase_list();
            set.clear();
            clear_string(std::ptr::addr_of!(set.name));
        }
        write_live!(set, flag_58, 1u8);
        unsafe {
            clear_string(std::ptr::addr_of!(set.current_call));
            reset_call(std::ptr::addr_of!(set.call) as *const u32);
            set.erase_list();
            set.kill_child();
        }
        write_live!(set, loaded, 0u8);
        write_live!(set, removed, 0u8);
        write_live!(set, dirty, 0u8);
        write_live!(set, pending, 0u8);
        unsafe {
            write(this + 0xf8, 0u8);
            write(this + 0xf9, 1u8);
            set.set_name(&name);
        }
        write_live!(set, flag_58, call_flag);
        if name.begin != name.end {
            loaded = unsafe { set.load() };
            match clear_mode {
                0 => unsafe { crate::gxmixer::total_clear(this + MIXER) },
                1 => unsafe { crate::gxmixer::clear_all(this + MIXER) },
                _ => {}
            }
        }
        if force != 0 {
            write_live!(set, pending, 1u8);
        }
    }
    unsafe { free_buffer(name.begin as *mut u8, (name.capacity as usize).wrapping_sub(name.begin as usize), 1) };
    loaded
}

/// A Rust-owned string holding `bytes`, as the by-value argument words a callee takes and releases.
unsafe fn new_owned_name(bytes: &[u8]) -> RawString {
    let mut slot: [*mut u8; 3] = [std::ptr::null_mut(); 3];
    unsafe { string_fill_new(&mut slot, bytes, 1) };
    RawString { begin: slot[0], end: slot[1], capacity: slot[2] }
}

/// `ZTAnimal::setPendingBS(name, callFlag, force)` (`0x4300f6`): when the pending-call flag (`+0xe8`) is set or
/// `force` is, stores `name` as the pending call string (`+0xdc`) and sets the flag to `call_flag`. Returns whether
/// it stored it. `name` is passed by value and its buffer is released on return, stored or not. Vanilla copies
/// over the existing text and appends the remainder, so only the buffer's capacity can differ from the port's.
pub(crate) unsafe fn set_pending_bs(this: u32, name: RawString, call_flag: u8, force: u8) -> bool {
    let stored = unsafe { read(this + BS_CALL + 0xc) } as u8 != 0 || force != 0;
    if stored {
        let text = unsafe { string_bytes(name.begin as u32, name.end as u32) };
        unsafe {
            string_assign((this + BS_CALL) as StringSlot, &text);
            write(this + BS_CALL + 0xc, call_flag);
        }
    }
    unsafe { free_buffer(name.begin as *mut u8, (name.capacity as usize).wrapping_sub(name.begin as usize), 1) };
    stored
}

unsafe fn string_bytes(begin: u32, end: u32) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(begin as *const u8, end.wrapping_sub(begin) as usize) }.to_vec()
}

/// `BFEntity::setBSNew(name, callFlag, force, clearMode)`: when `name` equals the set's current name, returns
/// true and does nothing. Otherwise runs `setBS` on a copy of `name`, and when the call queued at `+0xdc`
/// (read before `setBS`) equals `name`, queues an empty call (`0x4300f6`, call flag 1, forced). Returns `setBS`'s
/// result. `name` is passed by value and its buffer is released on return, as vanilla does.
pub(crate) unsafe fn set_bs_new(this: u32, name: RawString, call_flag: u8, force: u8, clear_mode: i32) -> bool {
    let set = unsafe { ref_from_memory::<BfBehaviorSet>(this + EMBEDDED_SET) };
    let wanted = unsafe { string_bytes(name.begin as u32, name.end as u32) };
    let current = unsafe { string_bytes(set.name.begin as u32, set.name.end as u32) };
    let queued = unsafe { string_bytes(read(this + BS_CALL), read(this + BS_CALL + 4)) };

    let result = if wanted == current {
        true
    } else {
        // `setBS` frees the string it is given, so it gets a copy of its own.
        let copy_words = unsafe { new_owned_name(&wanted) };
        let loaded = unsafe { set_bs(this, copy_words, call_flag, force, clear_mode) };
        if queued == wanted {
            unsafe { set_pending_bs(this, new_owned_name(b""), 1, 1) };
        }
        loaded
    };
    unsafe { free_buffer(name.begin as *mut u8, (name.capacity as usize).wrapping_sub(name.begin as usize), 1) };
    result
}

const POSITION: u32 = 0x114;
const ID: u32 = 0x124;
const ENTITY_TYPE: u32 = 0x128;
const ROTATION: u32 = 0x12c;
const FIELD_138: u32 = 0x138;
const FLAG_13C: u32 = 0x13c;
const FLAG_13F: u32 = 0x13f;
const ID_BUCKET: u32 = 0x148;
const ID_BUCKET_RESERVED: u32 = 0x14c;
/// The bytes at `+0xfc..+0x100`, an unconstructed `{begin, end, _}` buffer that save files carry from version
/// `0x43`.
const EXTRA_BYTES: u32 = 0xfc;
/// `GLOBAL_ZTAIMgr`, `GLOBAL_ZTApp`'s lazily set word, the app object, and the counter that numbers entities of
/// pre-`0xb` saves.
const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;
const APP_GUARD: u32 = 0x0063_8154;
const APP_GUARD_INIT: u32 = 0x0044_1880;
const APP: u32 = 0x0063_8148;
const APP_KEEP_NAME_FLAG: u32 = 0x4d0;
const ENTITY_ID_COUNTER: u32 = 0x0063_8738;
const AIMGR_ID_MODULUS: u32 = 0x124;
const AIMGR_DEFAULT_FIELD_138: u32 = 0xec;
const STRING_LIMIT: u32 = 0x1000;

/// `BFEntity::save`: position, rotation, id, name string, visible byte, `+0x138`, `+0x13c`, the behavior set,
/// then the `+0xfc` byte buffer as a count and one write per byte. Every write is attempted whatever came
/// before; the result is whether all succeeded.
pub(crate) unsafe fn save_entity(this: u32, file: *const i8) -> bool {
    let write = |address: u32, size: u32| unsafe { write_file_bytes(address as *const u8, size, file) };
    let mut ok = write(this + POSITION, 0xc);
    ok &= write(this + ROTATION, 4);
    ok &= write(this + ID, 4);
    ok &= unsafe { write_file_string(file, (this + NAME_VECTOR) as *const RawString) };
    ok &= write(this + FLAG_13F, 1);
    ok &= write(this + FIELD_138, 4);
    ok &= write(this + FLAG_13C, 1);
    ok &= unsafe { ref_from_memory::<BfBehaviorSet>(this + EMBEDDED_SET).save(file) };
    let length = unsafe { read(this + EXTRA_BYTES + 4).wrapping_sub(read(this + EXTRA_BYTES)) };
    ok &= unsafe { write_file_bytes(std::ptr::addr_of!(length) as *const u8, 4, file) };
    for index in 0..length {
        ok &= write(unsafe { read(this + EXTRA_BYTES) } + index, 1);
    }
    ok
}

/// A length-prefixed string as `load` reads it: `(all reads succeeded, bytes)`. `None` when the length cannot
/// be read or is at least `0x1000`. A short read of the bytes still yields the (zeroed) bytes and fails.
unsafe fn read_prefixed_bytes(file: *const u8) -> (bool, Option<Vec<u8>>) {
    let mut length = 0u32;
    if !unsafe { read_file_bytes(std::ptr::addr_of_mut!(length) as *mut u8, 4, file) } || length >= STRING_LIMIT {
        return (false, None);
    }
    let mut bytes = vec![0u8; length as usize];
    let ok = length == 0 || unsafe { read_file_bytes(bytes.as_mut_ptr(), length, file) };
    (ok, Some(bytes))
}

/// Which parts of a saved entity a given save `version` carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct LoadGates {
    id: bool,
    /// Saves up to `0xa` renumber a typed entity's id from the global counter.
    renumber_id: bool,
    legacy_behavior_name: bool,
    visible_and_field_138: bool,
    flag_13c: bool,
    behavior_set: bool,
    extra_bytes: bool,
}

impl LoadGates {
    const fn for_version(version: u32) -> Self {
        Self {
            id: version >= 7,
            renumber_id: version >= 7 && version <= 0xa,
            legacy_behavior_name: version < 0x3f,
            visible_and_field_138: version >= 0x23,
            flag_13c: version >= 0x3a,
            behavior_set: version >= 0x3f,
            extra_bytes: version >= 0x43,
        }
    }
}

/// The behavior name `load` hands to `setBS`: saves up to `0x16` call `bRunFromPredator` `bAvoidPredator`.
fn migrate_legacy_behavior_name(version: u32, name: Vec<u8>) -> Vec<u8> {
    if version <= 0x16 && name == b"bAvoidPredator" {
        b"bRunFromPredator".to_vec()
    } else {
        name
    }
}

/// `BFEntity::load(file, version)`. Fields appear by version: position and rotation always; the id from `7`
/// (saves older than `0xb` renumber it from a global counter); a name string, kept only when the app's
/// keep-name flag is set; from `0x23` the visible byte and `+0x138` (defaulted from `ZTAIMgr` when 0); from
/// `0x3a` `+0x13c`; from `0x3f` the behavior set; from `0x43` the `+0xfc` byte buffer. Before `0x3f` a legacy
/// behavior name follows the name string (`bAvoidPredator` is renamed in saves older than `0x17`); when it is
/// non-empty the mixer is cleared and `setBS` runs with it. Each read after the first failure that vanilla
/// still performs is still performed. The result is whether everything read.
pub(crate) unsafe fn load_entity(this: u32, file: *const u8, version: u32) -> bool {
    let read_into = |address: u32, size: u32| unsafe { read_file_bytes(address as *mut u8, size, file) };
    let gates = LoadGates::for_version(version);

    let mut position = [0i32; 3];
    let mut ok = unsafe { read_file_bytes(position.as_mut_ptr() as *mut u8, 0xc, file) };
    unsafe { SET_WORLD_POS.original()(this as *const u32, position.as_ptr() as *const glam::IVec3) };
    ok &= read_into(this + ROTATION, 4);
    if gates.id {
        ok &= read_into(this + ID, 4);
        if gates.renumber_id && unsafe { read(this + ENTITY_TYPE) } != 0 {
            let id = unsafe { read(ENTITY_ID_COUNTER) }.wrapping_add(1);
            unsafe {
                write(ENTITY_ID_COUNTER, id);
                write(this + ID, id);
            }
        }
    }
    if !ok {
        return false;
    }

    let modulus = unsafe { read(read(GLOBAL_ZTAIMGR) + AIMGR_ID_MODULUS) } as i32;
    let bucket = if modulus <= 0 { 0 } else { (unsafe { read(this + ID) } as i32).wrapping_rem(modulus) as u32 };
    unsafe {
        write(this + ID_BUCKET, bucket);
        write(this + ID_BUCKET_RESERVED, 0u32);
    }

    let (read_ok, name) = unsafe { read_prefixed_bytes(file) };
    ok &= read_ok;
    if !ok {
        return false;
    }
    let name = name.unwrap_or_default();
    unsafe {
        if read(APP_GUARD) == 0 {
            write(APP_GUARD, APP_GUARD_INIT);
        }
        if get_from_memory::<u8>(APP + APP_KEEP_NAME_FLAG) != 0 {
            string_assign((this + NAME_VECTOR) as StringSlot, &name);
        }
    }

    if gates.legacy_behavior_name {
        let (read_ok, legacy) = unsafe { read_prefixed_bytes(file) };
        ok &= read_ok;
        if !ok {
            return false;
        }
        let legacy = migrate_legacy_behavior_name(version, legacy.unwrap_or_default());
        if !legacy.is_empty() {
            let string = VanillaString::from_bytes(&legacy);
            let base = string.as_ptr() as u32;
            let name = RawString { begin: unsafe { read(base) } as *const u8, end: unsafe { read(base + 4) } as *const u8, capacity: unsafe { read(base + 8) } as *const u8 };
            // `setBS` takes the string by value and frees it.
            std::mem::forget(string);
            unsafe {
                crate::gxmixer::clear_all(this + MIXER);
                set_bs(this, name, 1, 1, 2);
            }
        }
    }

    if gates.visible_and_field_138 {
        ok &= read_into(this + FLAG_13F, 1);
        ok &= read_into(this + FIELD_138, 4);
        if unsafe { read(this + FIELD_138) } == 0 {
            unsafe { write(this + FIELD_138, read(read(GLOBAL_ZTAIMGR) + AIMGR_DEFAULT_FIELD_138)) };
        }
    }
    if gates.flag_13c {
        ok &= read_into(this + FLAG_13C, 1);
    }
    if gates.behavior_set {
        ok &= unsafe { ref_from_memory::<BfBehaviorSet>(this + EMBEDDED_SET).load_file(file, version, this as i32) };
    }
    if !ok {
        return false;
    }

    if gates.extra_bytes {
        let existing = unsafe { read(this + EXTRA_BYTES + 4).wrapping_sub(read(this + EXTRA_BYTES)) };
        let mut count = 0u32;
        ok &= unsafe { read_file_bytes(std::ptr::addr_of_mut!(count) as *mut u8, 4, file) };
        if !ok {
            return false;
        }
        let mut discarded = 0u8;
        for index in 0..count {
            ok &= if index < existing {
                read_into(unsafe { read(this + EXTRA_BYTES) } + index, 1)
            } else {
                unsafe { read_file_bytes(&mut discarded, 1, file) }
            };
            if !ok {
                return false;
            }
        }
    }
    true
}

/// `BFEntity::setName(name)` (`0x449215`, `name` a pointer to a `{begin, end, cap}` string): replaces the entity's
/// name (`+0x108`) with `name` when at least one of its bytes is meaningful (see
/// `habitat::name_has_meaningful_byte`: a DBCS lead byte, a CRT graphic character, or on code page 932 a byte in
/// `0xa1..=0xdf`). An empty, all-blank or self-assigned name changes nothing. Vanilla overwrites the existing
/// bytes and appends or truncates; only the buffer's capacity can differ from the port's.
pub(crate) unsafe fn set_name(this: u32, name: u32) {
    let text = unsafe { string_bytes(read(name), read(name + 4)) };
    if text.is_empty() {
        return;
    }
    let accepted = name_has_meaningful_byte(
        &text,
        |byte| unsafe { windows::Win32::Globalization::IsDBCSLeadByte(byte) }.is_ok(),
        |byte| unsafe { CRT_ISMBCGRAPH.original()(byte as i8 as i32 as u32) } != 0,
        || unsafe { windows::Win32::Globalization::GetACP() },
    );
    if accepted && name != this + NAME_VECTOR {
        unsafe { string_assign((this + NAME_VECTOR) as StringSlot, &text) };
    }
}

/// `BFEntity::clearBS`: empties the embedded set's name, current call text, call name, call arguments and
/// call list, drops the child, sets the set's `+0x58` flag (entity `+0x6c`), clears its loaded, removed, dirty
/// and pending bytes (entity `+0x70`, `+0x6e`, `+0x6d`, `+0x6f`) and returns 1 in `AL`. Unlike `reset_call`,
/// the call name is only emptied when it is not already empty.
pub(crate) unsafe fn clear_bs(this: u32) -> u32 {
    let set = unsafe { ref_from_memory::<BfBehaviorSet>(this + EMBEDDED_SET) };
    unsafe { clear_string(std::ptr::addr_of!(set.name)) };
    write_live!(set, flag_58, 1u8);
    unsafe { clear_string(std::ptr::addr_of!(set.current_call)) };
    unsafe { clear_string(std::ptr::addr_of!(set.call) as *const RawString) };
    let call = &set.call;
    unsafe { clear_call_args(std::ptr::addr_of!(set.call) as u32) };
    write_live!(call, result_flag, 0u8);
    write_live!(call, flag_19, 0u8);
    write_live!(call, flag_1a, 0u8);
    unsafe {
        set.erase_list();
        set.kill_child();
    }
    write_live!(set, loaded, 0u8);
    write_live!(set, removed, 0u8);
    write_live!(set, dirty, 0u8);
    write_live!(set, pending, 0u8);
    1
}

/// `BFEntity::clearQueue`: empties the `BFBSCall` string at `+0xdc` and sets that call's flag byte (`+0xe8`).
/// Returns true; `clearBSAndQueue` reads it.
pub(crate) unsafe fn clear_queue(this: u32) -> bool {
    unsafe {
        clear_string((this + BS_CALL) as *const RawString);
        write(this + BS_CALL + 0xc, 1u8);
    }
    true
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const u32) -> bool {
        unsafe { save_entity(this as u32, file as *const i8) }
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { load_entity(this as u32, file as *const u8, version) }
    }

    #[detour(CLEAR_QUEUE)]
    unsafe extern "thiscall" fn clear_queue_detour(this: *const u32) -> bool {
        unsafe { clear_queue(this as u32) }
    }

    #[detour(CLEAR_BS)]
    unsafe extern "thiscall" fn clear_bs_detour(this: *const u32) -> u32 {
        unsafe { clear_bs(this as u32) }
    }

    #[detour(SET_BS)]
    unsafe extern "thiscall" fn set_bs_detour(this: *const u32, begin: *const u32, end: *const u32, capacity: *const u32, call_flag: i8, force: i8, clear_mode: i32) -> bool {
        let name = RawString { begin: begin as *const u8, end: end as *const u8, capacity: capacity as *const u8 };
        unsafe { set_bs(this as u32, name, call_flag as u8, force as u8, clear_mode) }
    }

    #[detour(SET_BSNEW)]
    unsafe extern "thiscall" fn set_bs_new_detour(this: *const u32, begin: *const u32, end: *const u32, capacity: *const u32, call_flag: i8, force: i8, clear_mode: i32) -> bool {
        let name = RawString { begin: begin as *const u8, end: end as *const u8, capacity: capacity as *const u8 };
        unsafe { set_bs_new(this as u32, name, call_flag as u8, force as u8, clear_mode) }
    }

    #[detour(SET_PENDING_BS_0X4300F6)]
    unsafe extern "thiscall" fn set_pending_bs_detour(this: *const u32, begin: *const u32, end: *const u32, capacity: *const u32, call_flag: i8, force: i8) -> bool {
        let name = RawString { begin: begin as *const u8, end: end as *const u8, capacity: capacity as *const u8 };
        unsafe { set_pending_bs(this as u32, name, call_flag as u8, force as u8) }
    }

    #[detour(SET_NAME)]
    unsafe extern "thiscall" fn set_name_detour(this: *const u32, name: *const u32) {
        unsafe { set_name(this as u32, name as u32) }
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor_detour(this: *const u32, entity_type: u32) -> *const u32 {
        unsafe { construct_entity(this as u32, entity_type) };
        this
    }

    /// The destructor returns nothing meaningful; callers ignore `EAX`.
    #[detour(DESTRUCTOR_0)]
    unsafe extern "thiscall" fn destructor_detour(this: *const u32) -> *const u32 {
        unsafe { destroy_entity(this as u32) };
        this
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn constructor(this: u32, entity_type: u32) -> u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, entity_type) as u32 }
        }
        pub(crate) fn set_bs(this: u32, name: [u32; 3], call_flag: u8, force: u8, clear_mode: i32) -> bool {
            unsafe { super::SET_BS_DETOUR.call(this as *const u32, name[0] as *const u32, name[1] as *const u32, name[2] as *const u32, call_flag as i8, force as i8, clear_mode) }
        }
        pub(crate) fn set_bs_new(this: u32, name: [u32; 3], call_flag: u8, force: u8, clear_mode: i32) -> bool {
            unsafe { super::SET_BSNEW_DETOUR.call(this as *const u32, name[0] as *const u32, name[1] as *const u32, name[2] as *const u32, call_flag as i8, force as i8, clear_mode) }
        }
        pub(crate) fn set_pending_bs(this: u32, name: [u32; 3], call_flag: u8, force: u8) -> bool {
            unsafe { super::SET_PENDING_BS_0X4300F6_DETOUR.call(this as *const u32, name[0] as *const u32, name[1] as *const u32, name[2] as *const u32, call_flag as i8, force as i8) }
        }
        pub(crate) fn set_name(this: u32, name: u32) {
            unsafe { super::SET_NAME_DETOUR.call(this as *const u32, name as *const u32) }
        }
        pub(crate) fn clear_bs(this: u32) -> u32 {
            unsafe { super::CLEAR_BS_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }
        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }
        pub(crate) fn clear_queue(this: u32) -> bool {
            unsafe { super::CLEAR_QUEUE_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn destructor(this: u32) {
            unsafe { super::DESTRUCTOR_0_DETOUR.call(this as *const u32) };
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise bfentity detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) use super::detours::test_real as real;

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_switch_on_at_their_documented_versions() {
        let at = LoadGates::for_version;
        assert!(!at(6).id && at(7).id);
        assert!(at(7).renumber_id && at(0xa).renumber_id && !at(0xb).renumber_id && !at(6).renumber_id);
        assert!(at(0x3e).legacy_behavior_name && !at(0x3f).legacy_behavior_name);
        assert!(!at(0x22).visible_and_field_138 && at(0x23).visible_and_field_138);
        assert!(!at(0x39).flag_13c && at(0x3a).flag_13c);
        assert!(!at(0x3e).behavior_set && at(0x3f).behavior_set);
        assert!(!at(0x42).extra_bytes && at(0x43).extra_bytes);
    }

    #[test]
    fn legacy_name_is_a_behavior_set_or_a_set_never_both() {
        for version in 0..0x60 {
            let gates = LoadGates::for_version(version);
            assert_ne!(gates.legacy_behavior_name, gates.behavior_set, "version {version:#x}");
        }
    }

    #[test]
    fn avoid_predator_is_renamed_only_in_old_saves() {
        assert_eq!(migrate_legacy_behavior_name(0x16, b"bAvoidPredator".to_vec()), b"bRunFromPredator");
        assert_eq!(migrate_legacy_behavior_name(0x17, b"bAvoidPredator".to_vec()), b"bAvoidPredator");
        assert_eq!(migrate_legacy_behavior_name(0x10, b"bAvoidPredatorX".to_vec()), b"bAvoidPredatorX");
        assert_eq!(migrate_legacy_behavior_name(0x10, Vec::new()), b"");
    }
}
