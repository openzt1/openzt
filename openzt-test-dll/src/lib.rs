#![allow(dead_code)]

#[cfg(target_os = "windows")]
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH};

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn DllMain(_module: u8, reason: u32, _reserved: u8) -> i32 {
    match reason {
        DLL_PROCESS_ATTACH => {
            openztlib::reimplementation_tests::init();
        }
        DLL_PROCESS_DETACH => {}
        DLL_THREAD_ATTACH => {}
        DLL_THREAD_DETACH => {}
        _ => {}
    }
    1
}
