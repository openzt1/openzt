//! Compares real vanilla `GXMixer::getAnimSet`, `setSet` and `getAnim` against their Rust reimplementation
//! (production file `openzt/src/gxmixer.rs`) over standalone fake mixers, and checks the detours are enabled.
//! `getAnim` is only exercised where vanilla would not call `GXLLEAnim::acquire` (a loaded animation, or a null
//! result), since a fake animation cannot be acquired; the live real-unit test covers real animations.

use std::io::Write;

use tracing::error;

use crate::gxmixer::live_support::{build_fake_mixer, detour_status, real};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::{get_from_memory, save_to_memory};

/// `GXMIXER_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_gxmixer_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "GXMIXER_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `GXMIXER_READERS_FAKE` - `getAnimSet`, `setSet` and `getAnim` on fake mixers with zero to two set entries,
/// zero to three animations, and every combination of set index, requested animation index and flag.
pub(crate) fn run_gxmixer_readers_fake_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "GXMIXER_READERS_FAKE";
    let mut failures: Vec<String> = Vec::new();
    let animation_sets: [&[(u32, u8, bool)]; 4] = [&[], &[(3, 0, true)], &[(3, 0, true), (5, 1, true)], &[(3, 0, true), (4, 0, true), (6, 2, true)]];
    for entries in 0..3usize {
        for anims in animation_sets {
            for set_index in [0u32, 1, 2, 7] {
                for requested in [0u32, 1, 2, 3, 99] {
                    for flag in [0u8, 1, 200] {
                        let label = format!("entries={entries} anims={} set_index={set_index} requested={requested} flag={flag}", anims.len());
                        let (a, b) = (build_fake_mixer(entries, anims, 0), build_fake_mixer(entries, anims, 0));
                        for fake in [&a, &b] {
                            save_to_memory(fake.mixer + 0x58, set_index);
                            save_to_memory(fake.mixer + 0x4c, 5u32);
                        }
                        // The two mixers are separate allocations: compare positions relative to each one's own set and
                        // animation array, with null kept distinct.
                        let relative = |base: u32, value: u32| if value == 0 { u32::MAX } else { value.wrapping_sub(base) };
                        let (real_set, port_set) = (real::get_anim_set(a.mixer), crate::gxmixer::live_support::get_anim_set(b.mixer));
                        if relative(a.set, real_set) != relative(b.set, port_set) {
                            failures.push(format!("{label}: getAnimSet real={real_set:#x} port={port_set:#x}"));
                        }
                        real::set_set(a.mixer, requested, flag);
                        crate::gxmixer::live_support::set_set(b.mixer, requested, flag);
                        for offset in [0x4cu32, 0x64] {
                            let (r, p) = (get_from_memory::<u32>(a.mixer + offset) & if offset == 0x64 { 0xff } else { !0 }, get_from_memory::<u32>(b.mixer + offset) & if offset == 0x64 { 0xff } else { !0 });
                            if r != p {
                                failures.push(format!("{label}: setSet +{offset:#x} real={r} port={p}"));
                            }
                        }
                        // `getAnim` where vanilla would not acquire: the index is in range and the animation is loaded
                        // (frame count differs from its count byte), or the result is null.
                        let index = get_from_memory::<u32>(a.mixer + 0x4c) as usize;
                        let has_set = real_set != 0;
                        let acquires = has_set && anims.get(index).is_some_and(|(frames, counted, _)| *frames == u32::from(*counted));
                        if !acquires {
                            let (real_anim, port_anim) = (real::get_anim(a.mixer), unsafe { crate::gxmixer::get_anim(b.mixer) });
                            if relative(a.anim_base, real_anim) != relative(b.anim_base, port_anim) {
                                failures.push(format!("{label}: getAnim real={real_anim:#x} port={port_anim:#x}"));
                            }
                        }
                    }
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
