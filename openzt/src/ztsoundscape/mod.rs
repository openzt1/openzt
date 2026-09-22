//! `ZTSoundscape` reimplementation - the game's crowd/world ambient-audio crossfade state machine, one
//! of `ZTGameMgr`'s pointed-to sub-object classes: `ztgamemgr.rs` holds it as `soundscape_ptr`
//! (`this+0x1190`, explicitly zeroed by `CreateZTGameMgr`) and drives it through this port (`start`
//! allocates + constructs + `init`s it, `update_sim` calls `update`, `stop` runs the destructor + free).

use std::ffi::c_void;
use openzt_detour::generated::ztsoundscape::{CONSTRUCTOR, INIT, UPDATE};
use openzt_detour_macro::detour_mod;
use tracing::error;
use crate::util::mut_from_memory;

pub mod snd_slot;
#[allow(clippy::module_inception)]
pub mod ztsoundscape;

#[allow(unused_imports)]
pub use snd_slot::*;
pub use ztsoundscape::*;

/// Hooks `ZTSoundscape`'s three hooked entries (`CONSTRUCTOR`/`INIT`/`UPDATE`) so any caller reaching
/// those addresses runs the Rust code above. Every detour fully replaces its entry with the
/// corresponding Rust method (each verified equivalent live by the `ZTSOUNDSCAPE_*` battery tests)
/// and never calls vanilla. The class's fourth entry - the destructor, `generated.rs`'s
/// `ztsoundscape::DESTRUCTOR` - is deliberately **not** hooked.
#[detour_mod]
mod soundscape_detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const c_void) -> *const u32 {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.construct();
        this as *const u32
    }

    #[detour(INIT)]
    unsafe extern "thiscall" fn init(
        this: *const c_void,
        crowd_ambients_name: *const u32,
        world_ambients_name: *const u32,
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.init(
            crowd_ambients_name as *const u8,
            world_ambients_name as *const u8,
            crowd_config_name,
            world_config_name,
        );
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const c_void, delta: i32) {
        unsafe { mut_from_memory::<ZTSoundscape>(this) }.update(delta);
    }

    /// Live-test access to the real vanilla bodies and to the detours' installation state.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn constructor(this: *const c_void) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn init(
            this: *const c_void,
            crowd_ambients_name: *const u32,
            world_ambients_name: *const u32,
            crowd_config_name: *const u8,
            world_config_name: *const u8,
        ) {
            unsafe { super::INIT_DETOUR.call(this, crowd_ambients_name, world_ambients_name, crowd_config_name, world_config_name) }
        }

        pub(crate) fn update(this: *const c_void, delta: i32) {
            unsafe { super::UPDATE_DETOUR.call(this, delta) }
        }

        pub(crate) fn status() -> [(&'static str, bool); 3] {
            [
                ("CONSTRUCTOR", super::CONSTRUCTOR_DETOUR.is_enabled()),
                ("INIT", super::INIT_DETOUR.is_enabled()),
                ("UPDATE", super::UPDATE_DETOUR.is_enabled()),
            ]
        }
    }
}

/// Installs `ZTSoundscape`'s three address detours. Safe to call once at DLL load (via `lib.rs`),
/// or before the `ZTSOUNDSCAPE_*` tests in `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { soundscape_detours::init_detours() } {
        error!("Failed to initialise ztsoundscape detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub mod live_support {
    use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
    use openzt_detour::generated::ztsoundscape::DESTRUCTOR as ZTSOUNDSCAPE_DESTRUCTOR;

    use super::*;

    /// Allocates a fresh, uninitialized `0x54`-byte block via the real vanilla allocator.
    pub fn allocate_uninitialized() -> *mut ZTSoundscape {
        unsafe { OPERATOR_NEW.original()(0x54) as *mut ZTSoundscape }
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`].
    pub fn destroy_standalone(ptr: *mut ZTSoundscape) {
        if ptr.is_null() {
            return;
        }
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    /// Frees a standalone instance built via [`allocate_uninitialized`] plus construction and init.
    pub fn destroy_standalone_after_init(ptr: *mut ZTSoundscape) {
        if ptr.is_null() {
            return;
        }
        unsafe { ZTSOUNDSCAPE_DESTRUCTOR.original()(ptr as *const std::ffi::c_void) };
        unsafe { OPERATOR_DELETE.original()(ptr as u32) };
    }

    pub fn real_constructor(this: *const c_void) -> *const u32 {
        soundscape_detours::test_real::constructor(this)
    }

    pub fn real_init(
        this: *const c_void,
        crowd_ambients_name: *const u32,
        world_ambients_name: *const u32,
        crowd_config_name: *const u8,
        world_config_name: *const u8,
    ) {
        soundscape_detours::test_real::init(this, crowd_ambients_name, world_ambients_name, crowd_config_name, world_config_name)
    }

    pub fn real_update(this: *const c_void, delta: i32) {
        soundscape_detours::test_real::update(this, delta)
    }

    pub fn detour_status() -> [(&'static str, bool); 3] {
        soundscape_detours::test_real::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_writes_exactly_the_vanilla_set() {
        #[repr(align(4))]
        struct Scratch([u8; 0x54]);

        let mut scratch = Scratch([0xAA; 0x54]);
        let soundscape = unsafe { &mut *(scratch.0.as_mut_ptr() as *mut ZTSoundscape) };
        soundscape.construct();

        let bytes = &scratch.0;
        let dword = |off: usize| u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());

        for off in [0x0c, 0x14, 0x3c] {
            assert_eq!(dword(off), SNDSOUND_VTABLE, "vtable dword at +{off:#x}");
            assert_eq!(dword(off + 4), 0, "inner dword at +{:#x}", off + 4);
        }
        assert_eq!(dword(0x4c), 0, "crowd_ambients at +0x4c");
        assert_eq!(dword(0x50), 0, "world_ambients at +0x50");

        for (i, &b) in bytes.iter().enumerate() {
            let written = matches!(i, 0x0c..=0x1b | 0x3c..=0x43 | 0x4c..=0x53);
            if written {
                assert_ne!(b, 0xAA, "byte at +{i:#x} inside a written region still 0xAA");
            } else {
                assert_eq!(b, 0xAA, "ctor wrote outside the vanilla set at +{i:#x}");
            }
        }
    }

    #[test]
    fn ambient_level_matches_the_vanilla_band_table() {
        let expected = |g: i32| -> i32 {
            if g < 0 || g >= 100 {
                90
            } else if g <= 9 {
                -100
            } else if g <= 49 {
                10
            } else {
                40
            }
        };
        for g in [-2, -1, 0, 1, 8, 9, 10, 11, 48, 49, 50, 51, 98, 99, 100, 101, i32::MIN, i32::MAX] {
            assert_eq!(ambient_level(g), expected(g), "guests = {g}");
        }
        assert_eq!(ambient_level(-1), 90);
        assert_eq!(ambient_level(0), -100);
        assert_eq!(ambient_level(9), -100);
        assert_eq!(ambient_level(10), 10);
        assert_eq!(ambient_level(49), 10);
        assert_eq!(ambient_level(50), 40);
        assert_eq!(ambient_level(99), 40);
        assert_eq!(ambient_level(100), 90);
    }

    #[test]
    fn select_target_track_matches_the_vanilla_grid() {
        let bands: [(i32, i32); 7] = [(-5, 14), (15, 20), (21, 74), (75, 85), (86, 149), (150, 160), (161, 500_000)];
        let grid: [[Option<u32>; 7]; 5] = [
            [Some(0), Some(1), Some(1), Some(1), Some(1), Some(1), Some(1)],
            [None, None, Some(1), Some(1), Some(1), Some(1), Some(1)],
            [Some(0), None, None, None, Some(2), Some(2), Some(2)],
            [Some(0), Some(1), Some(1), None, None, None, Some(3)],
            [Some(0), Some(2), Some(2), Some(2), Some(2), None, None],
        ];
        let tracks = [-1i32, 0, 1, 2, 3];
        for (band, &(lo, hi)) in bands.iter().enumerate() {
            for (track_row, &t) in tracks.iter().enumerate() {
                for g in [lo, hi] {
                    assert_eq!(
                        select_target_track(t, g),
                        grid[track_row][band],
                        "current_track = {t}, guests = {g}"
                    );
                }
            }
        }
    }

    #[test]
    fn lcg_and_jitter_match_the_vanilla_sequence() {
        assert_eq!(lcg_next(1), 2_745_024);
        assert_eq!(lcg_next(2_745_024), 3_357_800_067);
        assert_eq!(lcg_next(3_357_800_067), 415_139_642);
        assert_eq!(lcg_next(415_139_642), 3_884_216_597);
        assert_eq!(lcg_next(u32::MAX), 0xFFFF_FFFFu32.wrapping_mul(0x343FD).wrapping_add(0x269EC3));

        let (position, state) = ambients_jitter(1, 1000, 2000, 7);
        assert_eq!(position, [1000 + 234, 2000 - 126, 7], "x from r3/r4 %400, y from r1/r2 %300, z passthrough");
        assert_eq!(state, 3_884_216_597, "final state must be r4, not an earlier advance");

        assert_eq!(lcg_next(0), 2_531_011);
        assert_eq!(lcg_next(3), 3_173_050);
        assert_eq!(lcg_next(3_173_050), 471_647_893);
        assert_eq!(lcg_next(471_647_893), 2_756_632_324);
        assert_eq!(lcg_next(2_756_632_324), 2_743_275_959);
        let (position, state) = ambients_jitter(3, 0, 0, -1);
        assert_eq!(position, [-197, -248, -1]);
        assert_eq!(state, 2_743_275_959);
    }

    #[test]
    fn advance_fade_matches_the_vanilla_clamps() {
        assert_eq!(advance_fade(5000, 1, 1000), 6000);
        assert_eq!(advance_fade(5000, 0, 1000), 4000);
        assert_eq!(advance_fade(0, 1, 7), 7);
        assert_eq!(advance_fade(7, 0, 7), 0);
        assert_eq!(advance_fade(500, 0, 1000), 0);
        assert_eq!(advance_fade(0, 0, 1), 0);
        assert_eq!(advance_fade(9500, 1, 1000), 10000);
        assert_eq!(advance_fade(10000, 1, 1), 10000);
        assert_eq!(advance_fade(9999, 1, i32::MAX), 0);
        assert_eq!(advance_fade(1, 0, i32::MAX), 0);
        assert_eq!(advance_fade(1, 0, i32::MIN), 0);
        assert_eq!(advance_fade(5000, 1, 0), 5001);
        assert_eq!(advance_fade(5000, 0, 0), 4999);
        assert_eq!(advance_fade(10000, 0, 0), 9999);
    }

    #[test]
    fn fade_attenuators_match_the_x87_sequence() {
        let c1 = f32::from_bits(0x38D1_B717);
        let c2 = 4500.0_f32;
        let c3 = 1.0_f32;

        assert_eq!(fade_atten_a(0, c1, c2), 0);
        assert_eq!(fade_atten_b(0, c1, c2, c3), 4500);
        assert_eq!(fade_atten_a(10000, c1, c2), 4500);
        assert_eq!(fade_atten_b(10000, c1, c2, c3), 0);

        assert_eq!(fade_atten_a(100, c1, c2), 44);
        assert_eq!(fade_atten_a(9000, c1, c2), 4049);
        assert_eq!(fade_atten_b(60, c1, c2, c3), 4472);
        assert_eq!(fade_atten_a(3333, c1, c2), 1499);
        assert_eq!(fade_atten_b(3333, c1, c2, c3), 3000);

        let (mut prev_a, mut prev_b) = (0, 4500);
        for fade in 0..=10000 {
            let (a, b) = (fade_atten_a(fade, c1, c2), fade_atten_b(fade, c1, c2, c3));
            assert!(a >= prev_a, "fade_atten_a decreased at fade = {fade}");
            assert!(b <= prev_b, "fade_atten_b increased at fade = {fade}");
            assert!((4500 - a - b) == 0 || (4500 - a - b) == 1, "a+b = {} at fade = {fade}", a + b);
            prev_a = a;
            prev_b = b;
        }
        assert_eq!(prev_a, 4500);
        assert_eq!(prev_b, 0);
        assert_eq!(fade_atten_b(9999, c1, c2, c3), 0);
    }
}
