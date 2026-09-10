//! Compares real vanilla `ZTGuest` megatile-query methods against their reimplementations
//! (production file `openzt/src/ztguest.rs`) for every live guest the loaded save contains.

use openzt_detour::generated::ztguest as gen_ztguest;
use std::io::Write;
use tracing::error;

use crate::reimplementation_tests::harness::write_success_line;
use crate::ztguest::{self, live_support as guest_live_support};
use crate::ztworldmgr::BFEntity;

/// ZTGUEST_MEGATILE_METHODS_LIVE: compares the real `ZTGuest::fCrowdDensityMegatile`/
/// `fStinkyMegatile`/`fEstheticBonusMegatile` (see `ztguest.rs`'s module doc comment for how these
/// three addresses were confirmed) against their Rust reimplementations, for *every* live guest
/// `guest_live_support::find_live_guests` finds on the loaded save - not just one, so the comparison
/// samples whatever spread of tiles/megatiles/entity-type category ids the live population actually
/// has, rather than a single arbitrary data point. Runs after the megatile-grid tests above so the
/// live singleton's grid is already in a real, recalculated state.
pub(crate) fn run_ztguest_megatile_methods_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_MEGATILE_METHODS_LIVE";
    let guests = guest_live_support::find_live_guests();
    if guests.is_empty() {
        write_success_line(failure_log, &format!("{} (skipped: no live guest found)", test_name));
        return false;
    }

    let mut fail_flag = false;
    let mut compared = 0usize;
    for (guest_ptr, tile) in guests {
        let this = guest_ptr as *const u32;
        let entity = unsafe { crate::util::ref_from_memory::<BFEntity>(guest_ptr) };
        compared += 1;

        let real_crowd = unsafe { gen_ztguest::F_CROWD_DENSITY_MEGATILE.original()(this) };
        let reimpl_crowd = ztguest::crowd_density_megatile(&tile);
        if real_crowd != reimpl_crowd {
            error!("{}: crowd density mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_crowd, reimpl_crowd);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: crowd density mismatch for guest {:#010x} at {}: real={}, reimpl={}\n", test_name, guest_ptr, tile, real_crowd, reimpl_crowd).as_bytes(),
                );
            }
            fail_flag = true;
        }

        let real_stink = unsafe { gen_ztguest::F_STINKY_MEGATILE.original()(this) };
        let reimpl_stink = ztguest::stinky_megatile(&tile);
        if (real_stink - reimpl_stink).abs() >= 0.01 {
            error!("{}: stink mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_stink, reimpl_stink);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!("Test Failed {}: stink mismatch for guest {:#010x} at {}: real={}, reimpl={}\n", test_name, guest_ptr, tile, real_stink, reimpl_stink).as_bytes(),
                );
            }
            fail_flag = true;
        }

        let real_esthetic = unsafe { gen_ztguest::F_ESTHETIC_BONUS_MEGATILE.original()(this) };
        let reimpl_esthetic = ztguest::esthetic_bonus_megatile(entity, &tile);
        if (real_esthetic - reimpl_esthetic).abs() >= 0.01 {
            error!("{}: esthetic bonus mismatch for guest {:#010x} at {}: real={}, reimpl={}", test_name, guest_ptr, tile, real_esthetic, reimpl_esthetic);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(
                    format!(
                        "Test Failed {}: esthetic bonus mismatch for guest {:#010x} at {}: real={}, reimpl={}\n",
                        test_name, guest_ptr, tile, real_esthetic, reimpl_esthetic
                    )
                    .as_bytes(),
                );
            }
            fail_flag = true;
        }
    }

    if !fail_flag {
        write_success_line(failure_log, &format!("{} ({} guests compared)", test_name, compared));
    }
    fail_flag
}
