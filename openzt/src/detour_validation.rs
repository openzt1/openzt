//! Detour validation: logs when annotated game functions are called.
//!
//! To add a new function: annotate its FunctionDef const in
//! openzt-detour/src/generated.rs with:
//!   #[cfg_attr(feature = "detour-validation", validate_detour("module/fn"))]
//!
//! Then rebuild and run:
//!   ./openzt.bat validate-detours module/fn
//!   ./openzt.bat validate-detours all

use openzt_detour_macro::detour_mod;

#[detour_mod]
mod detours_validation {

    use openzt_detour::generated::standalone::EXIT_3;

    #[detour(EXIT_3)]
    unsafe extern "stdcall" fn exit_3() {
        // write_shutdown_marker();
        tracing::info!("OPENZT_CLEAN_SHUTDOWN");
        // tracing::info!("EXIT_3 called, writing shutdown marker");
        unsafe { EXIT_3_DETOUR.call() };
    }
}

fn write_shutdown_marker() {
    use std::io::Write;
    let path = crate::util::get_base_path().join("openzt.log");
    if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path) {
        let _ = writeln!(file, "OPENZT_CLEAN_SHUTDOWN");
    }
}

pub fn init(enabled: &[&str]) {
    use std::panic::AssertUnwindSafe;

    match std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { detours_validation::init_detours() })) {
        Ok(Ok(())) => tracing::info!("Clean shutdown marker detour enabled"),
        Ok(Err(e)) => tracing::error!("Failed to enable clean shutdown marker detour: {}", e),
        Err(_) => tracing::error!("Panic enabling clean shutdown marker detour; address may be wrong"),
    }

    let enable_all = enabled.iter().any(|&s| s == "all");

    for entry in inventory::iter::<openzt_detour::ValidationEntry>() {
        if !enable_all && !enabled.contains(&entry.name) {
            continue;
        }
        match std::panic::catch_unwind(AssertUnwindSafe(entry.enable)) {
            Ok(Ok(())) =>
                tracing::info!("Validation detour enabled: {}", entry.name),
            Ok(Err(e)) =>
                tracing::error!(
                    "Failed to enable validation detour '{}': {}. \
                     Possible: already hooked by main codebase.", entry.name, e
                ),
            Err(_) =>
                tracing::error!(
                    "Panic enabling validation detour '{}'. \
                     Address in generated.rs is likely wrong.", entry.name
                ),
        }
    }
}
