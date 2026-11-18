//! `BFAIMgr::init` and `initAIParams`.

use super::*;

/// `ZTAIMGR_BFAIMGR_INIT_AI_PARAMS` (live zoo, so the configuration is loaded): the real and ported
/// `BFAIMgr::initAIParams` over the same fake manager blocks must leave every dword identical, write the two
/// globals the same way and return the same low byte.
pub(crate) fn run_ztaimgr_bfaimgr_init_ai_params_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::INIT_AIPARAMS;
    let test_name = "ZTAIMGR_BFAIMGR_INIT_AI_PARAMS";
    let mut failures: Vec<String> = Vec::new();
    let freq_cell = tag(0x0023_9028) as *mut u32;
    let count_cell = tag(0x0023_9532) as *mut u8;
    let (saved_freq, saved_count) = unsafe { (*freq_cell, *count_cell) };

    let mut blocks = [Box::new([0u32; 0x60]), Box::new([0u32; 0x60])];
    for block in blocks.iter_mut() {
        for (index, word) in block.iter_mut().enumerate() {
            *word = 0x7000_0000 + index as u32;
        }
    }
    let seed_globals = || unsafe {
        *freq_cell = 0x5555_aaaa;
        *count_cell = 0x77;
    };
    seed_globals();
    let real_ret = live_support::real_bfaimgr_init_ai_params(blocks[0].as_ptr());
    let real_globals = unsafe { (*freq_cell, *count_cell) };
    seed_globals();
    let port_ret = unsafe { INIT_AIPARAMS.hooked()(blocks[1].as_ptr()) };
    let port_globals = unsafe { (*freq_cell, *count_cell) };
    unsafe {
        *freq_cell = saved_freq;
        *count_cell = saved_count;
    }
    if real_ret & 0xff != 1 || port_ret & 0xff != 1 {
        failures.push(format!("return low byte real {:#x} port {:#x}, expected 1", real_ret & 0xff, port_ret & 0xff));
    }
    if real_globals != port_globals {
        failures.push(format!("globals real {real_globals:x?} port {port_globals:x?}"));
    }
    if real_globals == (0x5555_aaaa, 0x77) {
        failures.push("the real initAIParams did not write the globals".to_string());
    }
    for index in 0..0x60 {
        if blocks[0][index] != blocks[1][index] {
            failures.push(format!("+{:#x}: real {:#x} != port {:#x}", index * 4, blocks[0][index], blocks[1][index]));
        }
    }
    for offset in [0xd4usize, 0xd8, 0xdc, 0xe0, 0xe4, 0xf0, 0xf4, 0xf8, 0xfc, 0x100, 0x104, 0x108, 0x10c, 0x110, 0x114, 0x118, 0x11c, 0x120, 0x124, 0x128] {
        if blocks[0][offset / 4] == 0x7000_0000 + (offset / 4) as u32 {
            failures.push(format!("+{offset:#x}: not written by the real initAIParams"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_INIT`: the real and ported `BFAIMgr::init` over fake manager blocks whose vtable `+0x38` is a
/// recorder. Compares the tick remainder and counter (zeroed), which target ran (it must be reached through the
/// manager's vtable), the result's low byte and the `rand` state, which both seed from the clock: each must land
/// inside the window of host clock readings around the call.
pub(crate) fn run_ztaimgr_bfaimgr_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use openzt_detour::generated::bfaimgr::INIT;
    let test_name = "ZTAIMGR_BFAIMGR_INIT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static RESULT: AtomicU32 = AtomicU32::new(0);
    extern "thiscall" fn fake_init_ai_params(this: u32) -> u32 {
        LOG.lock().unwrap().push(this);
        RESULT.load(Ordering::Relaxed)
    }
    let now = || SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
    let mut vtable = Box::new([0u32; 0x40]);
    vtable[0x38 / 4] = fake_init_ai_params as *const () as usize as u32;
    let saved = rand_state();

    for result in [0x1234_5601u32, 0xabcd_ef00, 0x1, 0x0] {
        let mut outcomes = Vec::new();
        for port in [false, true] {
            let mut block = Box::new([0x5a5a_5a5au32; 0x60]);
            block[0] = vtable.as_ptr() as u32;
            let ptr = block.as_ptr();
            RESULT.store(result, Ordering::Relaxed);
            LOG.lock().unwrap().clear();
            set_rand_state(0xdead_beef);
            let before = now();
            let ret = if port { unsafe { INIT.hooked()(ptr) } } else { live_support::real_bfaimgr_init(ptr) };
            let after = now();
            let seed = rand_state();
            if seed < before || seed > after {
                failures.push(format!("{} seed {seed} outside [{before}, {after}] (result {result:#x})", if port { "port" } else { "real" }));
            }
            outcomes.push((ret, block[0xe8 / 4], block[0xec / 4], LOG.lock().unwrap().clone() == vec![ptr as u32], block[0x20 / 4]));
        }
        let expected = (result & 0xff != 0, 0, 0, true, 0x5a5a_5a5a);
        if outcomes[0] != expected || outcomes[1] != expected {
            failures.push(format!("result {result:#x}: real {:?}, port {:?}, expected {expected:?}", outcomes[0], outcomes[1]));
        }
    }
    set_rand_state(saved);
    finish_test(test_name, failures, failure_log)
}
