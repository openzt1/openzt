//! `ZTHabitatMgr::load` (`ZTHabitatMgr_load.c`/`.asm`) - the counterpart of [`ZTHabitatMgr::save`].
//!
//! `save` writes the exhibit count and then each habitat's own record; `load` does not rebuild habitats
//! from that stream. It parses every record into a `0x128`-byte *loaded-habitat record* and appends it to
//! vanilla's global record vector (`RVA_LOADED_HABITAT_RECORDS_BEGIN`/`_END`/`_CAP`). The habitats
//! themselves are created later by the world load, which `nameLoadedHabitats` then matches against these
//! records and destroys them.
//!
//! Record layout (offsets from the record start; the first two fields are read first, in this order):
//!
//! | offset | field |
//! |---|---|
//! | `+0x00`/`+0x04` | tile `x`/`y` |
//! | `+0x08` | habitat name, a vanilla `std::string` (`begin`/`end`/`cap` at `+0x8`/`+0xc`/`+0x10`) |
//! | `+0x14`/`+0x18`/`+0x1c` | three ids, `-1` before format version `0x15` |
//! | `+0x20..=+0x40` | nine dwords, zero before version `0x22` |
//! | `+0x48` | 8-byte timestamp, the current date before version `0x22` |
//! | `+0x50` | 8-byte timestamp, the current date before version `0x25` |
//! | `+0x58` | dword, zero before version `0x53` |
//! | `+0x5c` | "has show info" byte (version `0x5a` and later) |
//! | `+0x60` | embedded `ZTShowInfo` (loaded only when `+0x5c` is set) |
//! | `+0x108` | extended-data flag byte (version `0x55` and later) |
//! | `+0x10c`/`+0x110` | dwords, present when `+0x108` is set |
//! | `+0x114` | byte, `1` before version `0x58` |
//! | `+0x118`/`+0x11c`/`+0x120` | dwords, `100` before version `100` |
//!
//! The record vector and every string buffer belong to vanilla's allocator (`PoolAlloc`), because vanilla's
//! `clear`/`nameLoadedHabitats` free them. Every allocation here goes through `PoolAlloc` as well.

use openzt_detour::generated::{
    msvc_std_basic_string::BASIC_STRING_0 as MSVC_BASIC_STRING_DTOR,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    standalone::DEALLOCATE as READ_BYTES_FROM_FILE,
    zthabitat::SET_NAME as ZTHABITAT_SET_NAME,
    zttankexhibit::{
        DRAIN as ZTTANKEXHIBIT_DRAIN, FILL as ZTTANKEXHIBIT_FILL, SET_TANK_HEIGHT as ZTTANKEXHIBIT_SET_TANK_HEIGHT,
        SET_WATER_LEVEL as ZTTANKEXHIBIT_SET_WATER_LEVEL, SET_WATER_PURITY as ZTTANKEXHIBIT_SET_WATER_PURITY,
    },
    ztshowinfo::{CONSTRUCTOR_0 as ZTSHOWINFO_COPY_CONSTRUCTOR, CONSTRUCTOR_1 as ZTSHOWINFO_CONSTRUCTOR, DESTRUCTOR_0 as ZTSHOWINFO_DESTRUCTOR_0},
    ztshowmgr::GET_SHOW_INFO as ZTSHOWMGR_GET_SHOW_INFO,
};

use super::zthabitatmgr::ZTHabitatMgr;
use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, ref_from_memory, save_to_memory},
    write_live,
    zthabitat::{
        habitat::ZTHabitat,
        support::{
            call_vtable_slot_noargs_ret_bool, call_vtable_slot_with_ptr, HABITAT_LIST_DIRTY_RVA, RVA_APP_INIT_SUCCESS_BASE,
            RVA_LOADED_HABITAT_RECORDS_BEGIN, RVA_LOADED_HABITAT_RECORDS_CAP, RVA_LOADED_HABITAT_RECORDS_END,
        },
    },
    ztshow::info::lifecycle::{show_info_load, update_from_load},
};

/// Size of one loaded-habitat record.
pub const LOADED_HABITAT_RECORD_SIZE: u32 = 0x128;

/// Longest habitat name `load` accepts; a longer length field fails the whole load.
const MAX_NAME_LEN: u32 = 0x1000;

const OFFSET_NAME: u32 = 0x08;
const OFFSET_HAS_SHOW_INFO: u32 = 0x5c;
const OFFSET_SHOW_INFO: u32 = 0x60;
const OFFSET_EXTENDED_FLAG: u32 = 0x108;

/// A loaded-habitat record built on the stack. The contents are vanilla's `0x128`-byte layout; this wrapper
/// only fixes the size and alignment.
#[repr(C, align(4))]
struct RecordBuffer([u8; LOADED_HABITAT_RECORD_SIZE as usize]);

fn read_field(addr: u32, size: u32, file: *const u32) -> bool {
    unsafe { READ_BYTES_FROM_FILE.hooked()(addr as *const u32, size, 1, file as *const u8) == 1 }
}

/// Frees a record's string buffer, mirroring vanilla's inlined `PoolAlloc` free of `[cap - begin]` bytes.
fn free_name_buffer(record: u32) {
    let begin: u32 = get_from_memory(record + OFFSET_NAME);
    if begin != 0 {
        let cap: u32 = get_from_memory(record + OFFSET_NAME + 0x8);
        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, cap - begin) };
    }
}

fn destroy_temp_record(record: u32) {
    unsafe { ZTSHOWINFO_DESTRUCTOR_0.original()((record + OFFSET_SHOW_INFO) as *const u32) };
    free_name_buffer(record);
}

/// Points the record's name at a fresh `PoolAlloc` buffer holding `name` plus a NUL terminator, sized
/// `len + 1` exactly as vanilla's record copy produces.
fn set_name(record: u32, name: &[u8]) {
    let capacity = name.len() as u32 + 1;
    let buffer = unsafe { POOLALLOC_ALLOCATE.original()(capacity) } as u32;
    unsafe {
        std::ptr::copy_nonoverlapping(name.as_ptr(), buffer as *mut u8, name.len());
        *((buffer + name.len() as u32) as *mut u8) = 0;
    }
    save_to_memory::<u32>(record + OFFSET_NAME, buffer);
    save_to_memory::<u32>(record + OFFSET_NAME + 0x4, buffer + name.len() as u32);
    save_to_memory::<u32>(record + OFFSET_NAME + 0x8, buffer + capacity);
}

/// Copy-constructs a loaded-habitat record at `dst` from `src` (vanilla's `FUN_00604bfa`): the name is
/// copied into a fresh buffer of `len + 1` bytes, the embedded `ZTShowInfo` goes through its own copy
/// constructor, and every other field is copied verbatim. Padding is left untouched, like vanilla.
fn copy_construct_record(dst: u32, src: u32) {
    for offset in [0x0u32, 0x4] {
        save_to_memory::<u32>(dst + offset, get_from_memory::<u32>(src + offset));
    }

    let name_begin: u32 = get_from_memory(src + OFFSET_NAME);
    let name_end: u32 = get_from_memory(src + OFFSET_NAME + 0x4);
    let name = unsafe { std::slice::from_raw_parts(name_begin as *const u8, (name_end - name_begin) as usize) };
    set_name(dst, name);

    for offset in (0x14u32..=0x40).step_by(4).chain([0x48, 0x4c, 0x50, 0x54, 0x58]) {
        save_to_memory::<u32>(dst + offset, get_from_memory::<u32>(src + offset));
    }
    save_to_memory::<u8>(dst + OFFSET_HAS_SHOW_INFO, get_from_memory::<u8>(src + OFFSET_HAS_SHOW_INFO));

    unsafe { ZTSHOWINFO_COPY_CONSTRUCTOR.original()((dst + OFFSET_SHOW_INFO) as *const u32, (src + OFFSET_SHOW_INFO) as i32) };

    save_to_memory::<u8>(dst + OFFSET_EXTENDED_FLAG, get_from_memory::<u8>(src + OFFSET_EXTENDED_FLAG));
    save_to_memory::<u32>(dst + 0x10c, get_from_memory::<u32>(src + 0x10c));
    save_to_memory::<u32>(dst + 0x110, get_from_memory::<u32>(src + 0x110));
    save_to_memory::<u8>(dst + 0x114, get_from_memory::<u8>(src + 0x114));
    for offset in [0x118u32, 0x11c, 0x120] {
        save_to_memory::<u32>(dst + offset, get_from_memory::<u32>(src + offset));
    }
}

/// Destroys every record in the global vector in order (embedded `ZTShowInfo` first, then the name string) and
/// sets `end = begin`, keeping the buffer. The destructors release each record's `PoolAlloc` allocations; the
/// vector's own buffer is retained for the next load.
pub fn destroy_loaded_records(base: u32) {
    let begin: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_BEGIN);
    let end: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_END);
    let mut record = begin;
    while record != end {
        unsafe {
            ZTSHOWINFO_DESTRUCTOR_0.original()((record + OFFSET_SHOW_INFO) as *const u32);
            MSVC_BASIC_STRING_DTOR.original()((record + OFFSET_NAME) as *const std::ffi::c_void);
        }
        record += LOADED_HABITAT_RECORD_SIZE;
    }
    save_to_memory::<u32>(base + RVA_LOADED_HABITAT_RECORDS_END, begin);
}

/// Appends a copy of `record` to the global record vector, growing it the way vanilla does: when full, a
/// new buffer of `size + max(size, 1)` records is allocated, the old records are copy-constructed into it
/// followed by the new one, and the old records and buffer are destroyed.
fn push_record(record: u32) {
    let base = get_module_base("zoo.exe") as u32;
    let begin: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_BEGIN);
    let end: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_END);
    let cap: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_CAP);

    if end != cap {
        copy_construct_record(end, record);
        save_to_memory::<u32>(base + RVA_LOADED_HABITAT_RECORDS_END, end + LOADED_HABITAT_RECORD_SIZE);
        return;
    }

    let count = (end - begin) / LOADED_HABITAT_RECORD_SIZE;
    let new_count = count + count.max(1);
    let new_begin = unsafe { POOLALLOC_ALLOCATE.original()(new_count * LOADED_HABITAT_RECORD_SIZE) } as u32;

    let mut dst = new_begin;
    let mut src = begin;
    while src != end {
        copy_construct_record(dst, src);
        dst += LOADED_HABITAT_RECORD_SIZE;
        src += LOADED_HABITAT_RECORD_SIZE;
    }
    copy_construct_record(dst, record);

    let mut old = begin;
    while old != end {
        unsafe {
            ZTSHOWINFO_DESTRUCTOR_0.original()((old + OFFSET_SHOW_INFO) as *const u32);
            MSVC_BASIC_STRING_DTOR.original()((old + OFFSET_NAME) as *const std::ffi::c_void);
        }
        old += LOADED_HABITAT_RECORD_SIZE;
    }
    if begin != 0 {
        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, count_capacity_bytes(begin, cap)) };
    }

    save_to_memory::<u32>(base + RVA_LOADED_HABITAT_RECORDS_BEGIN, new_begin);
    save_to_memory::<u32>(base + RVA_LOADED_HABITAT_RECORDS_END, dst + LOADED_HABITAT_RECORD_SIZE);
    save_to_memory::<u32>(base + RVA_LOADED_HABITAT_RECORDS_CAP, new_begin + new_count * LOADED_HABITAT_RECORD_SIZE);
}

/// Byte size vanilla passes to `PoolAlloc::deallocate` for the old record buffer: whole records only.
fn count_capacity_bytes(begin: u32, cap: u32) -> u32 {
    (cap - begin) / LOADED_HABITAT_RECORD_SIZE * LOADED_HABITAT_RECORD_SIZE
}

/// Reads one record from `file` into `record` (the zeroed stack buffer at that address). Returns whether
/// every read succeeded; on `false` the caller destroys `record` and fails the load.
///
/// Every read executes regardless of earlier failures within the same step, matching vanilla (it ANDs the
/// results rather than short-circuiting), but the first failure turns the whole record into a failure.
fn read_record(record: u32, file: *const u32, version: u32, current_date: u64) -> bool {
    let date_low = current_date as u32;
    let date_high = (current_date >> 32) as u32;

    let mut ok = read_field(record, 4, file);
    ok &= read_field(record + 4, 4, file);

    let mut name_len: u32 = 0;
    let mut name_ok = read_field(&mut name_len as *mut u32 as u32, 4, file);
    let mut name: Vec<u8> = Vec::new();
    if name_ok {
        if name_len < MAX_NAME_LEN {
            if name_len != 0 {
                name = vec![0u8; name_len as usize];
                name_ok = read_field(name.as_mut_ptr() as u32, name_len, file);
            }
        } else {
            name_ok = false;
        }
    }
    ok &= name_ok;
    if !ok {
        return false;
    }
    set_name(record, &name);

    if version < 0x15 {
        for offset in [0x14u32, 0x18, 0x1c] {
            save_to_memory::<u32>(record + offset, 0xffff_ffff);
        }
    } else {
        for offset in [0x14u32, 0x18, 0x1c] {
            ok &= read_field(record + offset, 4, file);
        }
    }

    if version < 0x22 || !ok {
        save_to_memory::<u32>(record + 0x48, date_low);
        save_to_memory::<u32>(record + 0x4c, date_high);
        for offset in (0x20u32..=0x40).step_by(4) {
            save_to_memory::<u32>(record + offset, 0);
        }
    } else {
        for offset in (0x20u32..=0x40).step_by(4) {
            ok &= read_field(record + offset, 4, file);
        }
        ok &= read_field(record + 0x48, 8, file);
    }

    if version < 0x25 || !ok {
        save_to_memory::<u32>(record + 0x50, date_low);
        save_to_memory::<u32>(record + 0x54, date_high);
    } else {
        ok &= read_field(record + 0x50, 8, file);
    }

    if version < 0x53 {
        save_to_memory::<u32>(record + 0x58, 0);
    } else {
        ok &= read_field(record + 0x58, 4, file);
    }

    if version >= 0x55 {
        ok &= read_field(record + OFFSET_EXTENDED_FLAG, 1, file);
        if version > 0x59 {
            ok &= read_field(record + OFFSET_HAS_SHOW_INFO, 1, file);
            if get_from_memory::<u8>(record + OFFSET_HAS_SHOW_INFO) != 0 {
                show_info_load(record + OFFSET_SHOW_INFO, file, version);
            }
        }
        if get_from_memory::<u8>(record + OFFSET_EXTENDED_FLAG) != 0 {
            ok &= read_field(record + 0x10c, 4, file);
            ok &= read_field(record + 0x110, 4, file);
            save_to_memory::<u8>(record + 0x114, 1);
            if version > 0x57 {
                ok &= read_field(record + 0x114, 1, file);
            }
            for offset in [0x118u32, 0x11c, 0x120] {
                save_to_memory::<u32>(record + offset, 100);
            }
            if version > 99 {
                for offset in [0x118u32, 0x11c, 0x120] {
                    ok &= read_field(record + offset, 4, file);
                }
            }
        }
    }

    ok
}

impl ZTHabitatMgr {
    /// Ports `ZTHabitatMgr::load`. Reads the map size and zoo entrance into this manager, then (format
    /// version `0xd` and later) a record count and that many loaded-habitat records, appending each to
    /// vanilla's global record vector, and finally (version `0x40` and later) the habitat counter, which
    /// is then overwritten with `1` on every path that reaches it.
    ///
    /// A failed header read returns `false` without touching `habitat_num`; a failed record returns
    /// `false` immediately, keeping the records already appended. Differences from vanilla: the
    /// extended-data dwords of a record whose flag byte is clear (`+0x10c..+0x120`) are zero instead of
    /// stack garbage, and `ZTShowInfo::load`'s result is ignored exactly as vanilla ignores it.
    pub fn load(&self, file: *const u32, version: u32) -> bool {
        let mut ok = read_field(std::ptr::addr_of!(self.map_size_x) as u32, 4, file);
        if ok {
            ok = read_field(std::ptr::addr_of!(self.map_size_y) as u32, 4, file);
            if ok {
                ok = read_field(std::ptr::addr_of!(self.zoo_entrance_x) as u32, 4, file);
                if ok {
                    ok = read_field(std::ptr::addr_of!(self.zoo_entrance_y) as u32, 4, file);
                }
            }
        }
        if !(version > 0xc && ok) {
            return ok;
        }

        let mut record_count: i32 = 0;
        if !read_field(&mut record_count as *mut i32 as u32, 4, file) {
            return false;
        }

        let current_date = globals().ztgamemgr().get_date();
        for _ in 0..record_count.max(0) {
            let mut buffer = RecordBuffer([0; LOADED_HABITAT_RECORD_SIZE as usize]);
            let record = buffer.0.as_mut_ptr() as u32;
            unsafe { ZTSHOWINFO_CONSTRUCTOR.original()((record + OFFSET_SHOW_INFO) as *const u32) };

            let record_ok = read_record(record, file, version, current_date);
            if record_ok {
                push_record(record);
            }
            destroy_temp_record(record);
            if !record_ok {
                return false;
            }
        }

        if version > 0x3f {
            let read_ok = read_field(std::ptr::addr_of!(self.habitat_num) as u32, 4, file);
            write_live!(self, habitat_num, 1);
            return read_ok;
        }
        write_live!(self, habitat_num, 1);
        true
    }

    /// Ports `ZTHabitatMgr::nameLoadedHabitats` (`0x5939fb`): applies every loaded-habitat record to the
    /// habitat now occupying its tile, then destroys the records and marks the habitat list dirty.
    ///
    /// Per record whose tile holds a habitat: the saved name is applied through `ZTHabitat::setName` when the
    /// app's `+0x4d0` flag is set; the entrance tile (resolved against the live map, `0` when out of bounds)
    /// and rotation are stored and, for a valid rotation, the fence entity in that tile's edge slot is told
    /// the exhibit name through its vtable slot `+0x1c`; the finance and timestamp fields are copied over; the
    /// service time goes through [`ZTHabitat::set_time_last_serviced`]; the embedded show info (when flagged)
    /// is merged into the habitat's registered `ZTShowInfo`; and a flagged tank has its height, water level,
    /// fill state, the two `+0x18c`/`+0x190` words and water purity restored.
    ///
    /// Vanilla also writes the `GLOBAL_ZTApp` slot when it is null, a side effect shared by every inlined
    /// reader of that slot and not reproduced here (see `RVA_APP_INIT_SUCCESS_BASE`).
    pub fn name_loaded_habitats(&self) {
        let base = get_module_base("zoo.exe") as u32;
        let begin: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_BEGIN);
        let end: u32 = get_from_memory(base + RVA_LOADED_HABITAT_RECORDS_END);

        let mut record = begin;
        while record != end {
            let habitat_ptr = self.get_habitat_ptr(get_from_memory::<i32>(record), get_from_memory::<i32>(record + 4));
            if habitat_ptr != 0 {
                apply_record_to_habitat(base, habitat_ptr, record);
            }
            record += LOADED_HABITAT_RECORD_SIZE;
        }

        destroy_loaded_records(base);
        save_to_memory::<u8>(base + HABITAT_LIST_DIRTY_RVA, 1);
    }
}

fn apply_record_to_habitat(base: u32, habitat_ptr: u32, record: u32) {
    if get_from_memory::<u8>(base + RVA_APP_INIT_SUCCESS_BASE + 0x4d0) != 0 {
        unsafe { ZTHABITAT_SET_NAME.original()(habitat_ptr as *const u32, (record + OFFSET_NAME) as *const u32) };
    }

    let entrance_x: i32 = get_from_memory(record + 0x14);
    if entrance_x >= 0 {
        let entrance_y: i32 = get_from_memory(record + 0x18);
        let rotation: i32 = get_from_memory(record + 0x1c);
        let world = globals().ztworldmgr();
        let in_bounds = entrance_y >= 0 && entrance_x < world.map_x_size as i32 && entrance_y < world.map_y_size as i32;
        let tile_ptr = if in_bounds { world.get_tile_ptr(entrance_x as u32, entrance_y as u32) } else { 0 };
        save_to_memory::<u32>(habitat_ptr + 0x8c, tile_ptr);
        save_to_memory::<u32>(habitat_ptr + 0x90, rotation as u32);
        if tile_ptr != 0 && rotation != -1 {
            let edge_slot = tile_ptr.wrapping_add(0x14).wrapping_add(((rotation / 2) as u32).wrapping_mul(4));
            let fence_ptr: u32 = get_from_memory(edge_slot);
            if fence_ptr != 0 {
                unsafe { call_vtable_slot_with_ptr(fence_ptr, 0x1c, habitat_ptr + 0x154) };
            }
        }
    }

    for (habitat_offset, record_offset) in [
        (0xfcu32, 0x20u32), (0x100, 0x24), (0x104, 0x28), (0x108, 0x2c), (0x10c, 0x30), (0x110, 0x34),
        (0x114, 0x38), (0x118, 0x3c), (0x11c, 0x40), (0x120, 0x48), (0x124, 0x4c),
    ] {
        save_to_memory::<u32>(habitat_ptr + habitat_offset, get_from_memory::<u32>(record + record_offset));
    }
    if unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_num_animals(false) != 0 {
        save_to_memory::<u32>(habitat_ptr + 0x128, get_from_memory::<u32>(record + 0x50));
        save_to_memory::<u32>(habitat_ptr + 0x12c, get_from_memory::<u32>(record + 0x54));
    }
    unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.set_time_last_serviced(get_from_memory::<u32>(record + 0x58), true);

    if get_from_memory::<u8>(record + OFFSET_HAS_SHOW_INFO) != 0 {
        let show_owner: u32 = get_from_memory(habitat_ptr + 0x4);
        let show_id: u16 = if show_owner == 0 { 0 } else { get_from_memory(show_owner + 0x70) };
        let show_info = unsafe { ZTSHOWMGR_GET_SHOW_INFO.hooked()(globals().ztshowmgr_ptr() as *const u32, show_id) } as u32;
        if show_info != 0 {
            update_from_load(show_info, record + OFFSET_SHOW_INFO);
        }
    }

    if get_from_memory::<u8>(record + OFFSET_EXTENDED_FLAG) != 0 && unsafe { call_vtable_slot_noargs_ret_bool(habitat_ptr, 0x20) } {
        let tank = habitat_ptr as *const u32;
        unsafe {
            ZTTANKEXHIBIT_SET_TANK_HEIGHT.hooked()(tank, get_from_memory::<i32>(record + 0x10c));
            ZTTANKEXHIBIT_SET_WATER_LEVEL.original()(tank, get_from_memory::<i32>(record + 0x110));
            if get_from_memory::<u8>(record + 0x114) == 0 {
                ZTTANKEXHIBIT_DRAIN.original()(tank);
            } else {
                ZTTANKEXHIBIT_FILL.original()(tank);
            }
        }
        save_to_memory::<u32>(habitat_ptr + 0x18c, get_from_memory::<u32>(record + 0x118));
        save_to_memory::<u32>(habitat_ptr + 0x190, get_from_memory::<u32>(record + 0x11c));
        unsafe { ZTTANKEXHIBIT_SET_WATER_PURITY.original()(tank, get_from_memory::<i32>(record + 0x120)) };
    }
}
