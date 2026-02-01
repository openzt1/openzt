#[cfg(target_os = "windows")]
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_THREAD_ATTACH, DLL_THREAD_DETACH};

#[cfg(target_os = "windows")]
#[no_mangle]
extern "system" fn DllMain(_module: u8, reason: u32, _reserved: u8) -> i32 {
    // DO NOT uncomment any of the logs here, they will cause crashes
    match reason {
        DLL_PROCESS_ATTACH => {
            // Initialize a hook into the LoadLangDlls function, where we can perform further initialization
            openztlib::init();
        }
        DLL_PROCESS_DETACH => {
            // info!("DllMain: DLL_PROCESS_DETACH: {}, {} {}", module, reason, _reserved);
        }
        DLL_THREAD_ATTACH => {
            // info!("DllMain: DLL_THREAD_ATTACH: {}, {} {}", module, reason, _reserved);
        }
        DLL_THREAD_DETACH => {
            // info!("DllMain: DLL_THREAD_DETACH: {}, {} {}", module, reason, _reserved);
        }
        _ => {
            // info!("DllMain: Unknown: {}, {} {}", module, reason, _reserved);
        }
    }
    1
}
