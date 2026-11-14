//! `BFEvent`: the 0x1c-byte event record that `ZTShowInfo`'s event list (`ZTShowInfo+0x5c..+0x64`) and
//! `BFEventMgr`'s two event vectors (`+0x4`/`+0x10`) hold by value. Records are packed contiguously in
//! vanilla-allocated `vector<BFEvent>` storage, and un-ported vanilla code reads them in place by raw
//! offset (`ZTShowInfo::listen` reads `+0xc`/`+0x16`; `ZTAIMgr`'s `getEvents` writes whole records
//! straight into the vector). The struct therefore mirrors vanilla's layout exactly (style 1) and its
//! methods read and write that memory in place.
//!
//! ## Layout
//! `+0x00` vtable (`0x0063556c`, one slot: `init`), then an embedded `BFEventInfo` at `+0x04` whose own
//! vtable (`0x00635568`, one slot: `BFEventInfo::init`) sits at `+0x04` and whose payload follows:
//! `+0x08`/`+0x0c`/`+0x10` `u32`, `+0x14`/`+0x16`/`+0x18` `u16`, `+0x1a`/`+0x1b` `u8`.
//!
//! ## What is ported
//! `BFEvent`'s four `generated.rs` entries: `CONSTRUCTOR`, `INIT`, `LOAD`, `SAVE`. In vanilla, `INIT`,
//! `LOAD` and `SAVE` are one-instruction forwards to the embedded `BFEventInfo`'s own method (`ADD ECX,
//! 4` then jump/call), and the constructor stores the field defaults and then runs `INIT` through the
//! embedded vtable. The `BFEventInfo` bodies are therefore implemented here directly as the `BFEvent`
//! methods. `BFEventInfo`'s own addresses (`generated::bfeventinfo`) are left un-detoured: they operate
//! on the same bytes and nothing in this crate calls them.
//!
//! ## Allocation
//! None. `BFEvent` owns no heap memory, so there is no cross-allocator hazard and no destructor to port
//! (vanilla never runs one; the vector's storage is released as a plain block).

use openzt_detour::generated::{
    bfevent::{CONSTRUCTOR, INIT, LOAD, SAVE},
    standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{util::ref_from_memory, write_live};

/// `BFEvent`'s own vtable.
const VTABLE_BFEVENT: u32 = 0x0063_556c;
/// The embedded `BFEventInfo`'s vtable.
const VTABLE_BFEVENT_INFO: u32 = 0x0063_5568;

/// Size of one record in a `vector<BFEvent>`.
pub const BFEVENT_SIZE: u32 = 0x1c;

/// The first save-file version that carries `BFEvent` payloads.
const MIN_SAVE_VERSION_EXCLUSIVE: u32 = 0x60;

/// Default for [`BFEvent::field_18`] and the two trailing bytes, as installed by `init`.
const DEFAULT_FIELD_18: u16 = 1;
const DEFAULT_TRAILING_BYTE: u8 = 0x58;

#[derive(Debug)]
#[repr(C)]
pub struct BFEvent {
    vtable: u32,       // 0x00
    info_vtable: u32,  // 0x04 - embedded BFEventInfo's vtable
    field_08: u32,     // 0x08
    target_id: u32,    // 0x0c - `ZTShowInfo::listen` matches this against the show's own id (`ZTShowInfo+0x70`)
    field_10: u32,     // 0x10
    field_14: u16,     // 0x14
    event_id: u16,     // 0x16 - `ZTShowInfo::listen` switches on this (0x2714 start, 0x2716 abort)
    field_18: u16,     // 0x18 - defaults to 1
    field_1a: u8,      // 0x1a - defaults to 0x58
    field_1b: u8,      // 0x1b - defaults to 0x58
    pub _live: crate::util::LiveMemory,
}

const _: () = assert!(std::mem::size_of::<BFEvent>() == BFEVENT_SIZE as usize);

impl BFEvent {
    pub fn target_id(&self) -> u32 {
        self.target_id
    }

    pub fn event_id(&self) -> u16 {
        self.event_id
    }

    /// `BFEvent::BFEvent`: installs both vtables and runs [`Self::init`].
    pub fn construct(&self) {
        write_live!(self, vtable, VTABLE_BFEVENT);
        write_live!(self, info_vtable, VTABLE_BFEVENT_INFO);
        self.init();
    }

    /// `BFEvent::init` (`BFEventInfo::init` on the embedded sub-object): resets every payload field to its
    /// default. Leaves both vtable pointers alone.
    pub fn init(&self) {
        write_live!(self, field_08, 0u32);
        write_live!(self, target_id, 0u32);
        write_live!(self, field_10, 0u32);
        write_live!(self, field_14, 0u16);
        write_live!(self, event_id, 0u16);
        write_live!(self, field_18, DEFAULT_FIELD_18);
        write_live!(self, field_1a, DEFAULT_TRAILING_BYTE);
        write_live!(self, field_1b, DEFAULT_TRAILING_BYTE);
    }

    /// `BFEvent::load`: reads every payload field for save versions above `0x60`; older versions carry
    /// no payload and succeed without touching `self` or the file. Every read is attempted even after one
    /// fails, matching vanilla's evaluate-all-then-combine shape.
    pub fn load(&self, file: *const u32, version: u32) -> bool {
        if version <= MIN_SAVE_VERSION_EXCLUSIVE {
            return true;
        }
        let base = self as *const Self as u32;
        let mut ok = read_field(base + 0x08, 4, file);
        ok &= read_field(base + 0x0c, 4, file);
        ok &= read_field(base + 0x10, 4, file);
        ok &= read_field(base + 0x14, 2, file);
        ok &= read_field(base + 0x16, 2, file);
        ok &= read_field(base + 0x18, 2, file);
        ok &= read_field(base + 0x1a, 1, file);
        ok &= read_field(base + 0x1b, 1, file);
        ok
    }

    /// `BFEvent::save`: writes every payload field in [`Self::load`]'s order. Every write is attempted
    /// even after one fails.
    pub fn save(&self, file: *const i8) -> bool {
        let base = self as *const Self as u32;
        let mut ok = write_field(base + 0x08, 4, file);
        ok &= write_field(base + 0x0c, 4, file);
        ok &= write_field(base + 0x10, 4, file);
        ok &= write_field(base + 0x14, 2, file);
        ok &= write_field(base + 0x16, 2, file);
        ok &= write_field(base + 0x18, 2, file);
        ok &= write_field(base + 0x1a, 1, file);
        ok &= write_field(base + 0x1b, 1, file);
        ok
    }
}

fn write_field(addr: u32, size: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(addr as *const u32, size, 1, file) == 1 }
}

fn read_field(addr: u32, size: u32, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(addr as *const u32, size, 1, file as *const u8) == 1 }
}

/// Free-function entry points over a raw `BFEvent*`, for other ported modules that hold the record by
/// address (e.g. `ZTShowInfo`'s event vector) and must not call the hooked addresses through
/// `.original()`.
pub fn construct_at(this: u32) {
    unsafe { ref_from_memory::<BFEvent>(this) }.construct();
}

pub fn load_at(this: u32, file: *const u32, version: u32) -> bool {
    unsafe { ref_from_memory::<BFEvent>(this) }.load(file, version)
}

pub fn save_at(this: u32, file: *const i8) -> bool {
    unsafe { ref_from_memory::<BFEvent>(this) }.save(file)
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const u32) -> *const u32 {
        construct_at(this as u32);
        this
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(this: *const u32) {
        unsafe { ref_from_memory::<BFEvent>(this) }.init();
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        load_at(this as u32, file, version)
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const i8) -> bool {
        save_at(this as u32, file)
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile (see
    /// `ztgamemgr_menumusichandler`'s `test_real`).
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn constructor(this: *const u32) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn init(this: *const u32) {
            unsafe { super::INIT_DETOUR.call(this) }
        }

        pub(crate) fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this, file, version) }
        }

        pub(crate) fn save(this: *const u32, file: *const i8) -> bool {
            unsafe { super::SAVE_DETOUR.call(this, file) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()` (the test DLL never reaches `lib.rs`'s cascade).
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise bfevent detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// A zeroed, leaked, 4-aligned `0x1c`-byte buffer standing in for one `BFEvent`. `BFEvent` owns no
    /// heap memory, so nothing needs freeing and the leak is bounded by the test count.
    pub(crate) fn standalone_buffer() -> u32 {
        let buf: &'static mut [u32] = Box::leak(vec![0u32; (BFEVENT_SIZE / 4) as usize].into_boxed_slice());
        buf.as_mut_ptr() as u32
    }

    pub(crate) fn real_constructor(this: *const u32) -> *const u32 {
        detours::test_real::constructor(this)
    }

    pub(crate) fn real_init(this: *const u32) {
        detours::test_real::init(this)
    }

    pub(crate) fn real_load(this: *const u32, file: *const u32, version: u32) -> bool {
        detours::test_real::load(this, file, version)
    }

    pub(crate) fn real_save(this: *const u32, file: *const i8) -> bool {
        detours::test_real::save(this, file)
    }

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_offsets_match_vanilla() {
        assert_eq!(std::mem::offset_of!(BFEvent, info_vtable), 0x04);
        assert_eq!(std::mem::offset_of!(BFEvent, field_08), 0x08);
        assert_eq!(std::mem::offset_of!(BFEvent, target_id), 0x0c);
        assert_eq!(std::mem::offset_of!(BFEvent, field_10), 0x10);
        assert_eq!(std::mem::offset_of!(BFEvent, field_14), 0x14);
        assert_eq!(std::mem::offset_of!(BFEvent, event_id), 0x16);
        assert_eq!(std::mem::offset_of!(BFEvent, field_18), 0x18);
        assert_eq!(std::mem::offset_of!(BFEvent, field_1a), 0x1a);
        assert_eq!(std::mem::offset_of!(BFEvent, field_1b), 0x1b);
    }
}
