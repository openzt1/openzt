use regex::Regex;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;
use tracing::{info, warn};

/// Set of available language DLL files in the game directory
static AVAILABLE_DLLS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Initialize DLL dependency checking by scanning the game directory
///
/// This function should be called from load_lang_dlls() during early initialization.
/// It scans the base exe directory for language DLL files matching `lang.*\.dll`
/// and stores them for validation.
pub fn init() {
    use std::fs;

    // Get the directory containing the running executable (zoo.exe/zt.exe)
    let exe_path = std::env::current_exe().expect("Failed to get current executable path");

    let exe_dir = exe_path.parent().expect("Failed to get executable directory");

    info!("Scanning directory for DLL dependencies: {}", exe_dir.display());

    let Ok(entries) = fs::read_dir(exe_dir) else {
        warn!("Failed to read executable directory for DLL scanning");
        return;
    };

    // Regex to match language DLLs (e.g., langusa.dll, languk.dll)
    let lang_dll_regex = Regex::new(r"^lang.*\.dll$").unwrap();

    let mut found_dlls = HashSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(file_name) = path.file_name()
            && let Some(name_str) = file_name.to_str() {
                let name_lower = name_str.to_lowercase();
                // Only include DLLs matching the lang*.dll pattern
                if lang_dll_regex.is_match(&name_lower) {
                    found_dlls.insert(name_lower);
                }
            }
    }

    // Initialize the OnceLock with our found DLLs
    let _ = AVAILABLE_DLLS.set(Mutex::new(found_dlls));

    let available = AVAILABLE_DLLS.get().unwrap();
    info!("Found {} language DLL files for dependency validation", available.lock().unwrap().len());
}

/// Check if a DLL dependency is satisfied
///
/// Returns true if the DLL file exists in the game directory, false otherwise.
pub fn check_dll_dependency(dll_name: &str) -> bool {
    // Validate that dll_name ends with .dll
    if !dll_name.to_lowercase().ends_with(".dll") {
        warn!("Invalid dll_name '{}': must end with .dll extension", dll_name);
        return false;
    }

    if let Some(available) = AVAILABLE_DLLS.get() {
        let available = available.lock().unwrap();
        let dll_name_lower = dll_name.to_lowercase();
        available.contains(&dll_name_lower)
    } else {
        // init() hasn't been called yet - this shouldn't happen in normal flow
        warn!("DLL dependency check attempted before initialization");
        false
    }
}

/// Get the set of all available DLL names (for testing/debugging)
#[cfg(test)]
pub fn get_available_dlls() -> HashSet<String> {
    if let Some(available) = AVAILABLE_DLLS.get() {
        available.lock().unwrap().clone()
    } else {
        HashSet::new()
    }
}
