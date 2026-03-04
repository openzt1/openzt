#![allow(dead_code)]
/// Centralized registry for accessing global C++ manager instances
mod globals;

/// Reimplementation of the BFRegistry, a vanilla system used to store pointers to the ZT*Mgr classes. In theory this
/// allowed customization via zoo.ini, but in practice it appears unused.
mod bfregistry;

/// Hooks into the vanilla game's logging system to re-log messages with the default OpenZT logger.
mod capture_ztlog;

/// Basic development console, includes a server that listens for a client connection to recieve commands from,
/// functions for registering commands with a function callback and hooks so that a command is run every game update
mod command_console;

/// Commands and functions for reading entities and entity types from the ZTWorldMgr class
pub mod ztworldmgr;

mod resource_manager;

/// Centralized logging initialization
pub mod logging;

/// Terminal User Interface console
#[cfg(feature = "tui")]
mod tui_console;

/// Reading and changing the state of the UI, contains hooks for UI elements and some basic UI manipulation functions.
mod ztui;

/// Assembly patches and functions to fix bugs in the vanilla game.
///
/// Currently fixes a crash when a maintenance worker tries to fix a
/// fence 1 tile away from the edge of the map, and a bug where the
/// game crashes if a zoo wall that is one tile away from the edge
/// of the map is deleted.
#[cfg(target_os = "windows")]
mod bugfix;

/// Methods for reading the vanilla ZTAdvTerrainMgr class, which contains information about terrain types.
mod ztadvterrainmgr;

/// Reimplementation of vanilla handling of Expansion Packs, including the ability to define custom expansions.
///
/// Default behaviour adds in an expansion called "Custom Content" which includes all non-vanilla entities.
/// Expanding the Expansion dropdown is also handled here.
mod expansions;

/// Reimplementation of the vanilla BFApp::loadString, has functions to add a string to the OpenZT string registry,
/// will fallback to the vanilla BFApp::loadString if the string is not found in the registry.
mod string_registry;

/// Helper methods for parsing binary data, including reading and writing binary data to and from buffers.
mod binary_parsing;

/// Encoding utilities for handling text from game files with various encodings (UTF-8, Windows ANSI code pages).
mod encoding_utils;

/// ZTAF Animation file format parsing, writing and some modification methods.
///
/// Based on documentation at <https://github.com/jbostoen/ZTStudio/wiki/ZT1-Graphics-Explained>
mod animation;

/// Structs that mirror ZT Entity types and their properties. Currently there are many missing fields.
mod bfentitytype;

/// ztgamemgr module has commands to interact with the live zoo stats such as cash, num animals, species, guests, etc. via the vanilla ZTGameMgr class.
mod ztgamemgr;

/// ztmapview is the main view in zoo tycoon, all map interaction is done through this class.
pub mod ztmapview;

/// zthabitatmgr module has commands to interact with habitats/exhibits/tanks via the vanilla ZTHabitatMgr class.
mod zthabitatmgr;

mod experimental;

/// Roof tag extension for scenery entities
mod roofs;

/// DLL dependency validation for Zoo Tycoon game DLLs
mod dll_dependencies;

/// Global runtime state store for cross-module state sharing
mod runtime_state;

/// Keyboard shortcut registration system for game thread callbacks
mod shortcuts;

/// Patches in the current OpenZT build version into the game's version string.
mod version;

// TODO: Move this to resource_manager/openzt_mods
/// OpenZT mod structs
mod mods;

/// Utility functions for working with the game's memory, including reading and writing memory, and patching the game's assembly.
/// Common structs like ZTString are also defined here
mod util;

/// Loads settings from the zoo.ini file and commands/functions for reading and writing settings during runtime
mod settings;

/// Scripting module for OpenZT using the mlua library. Contains functions for loading and running Lua scripts, and registering Rust functions to be called from Lua.
pub mod scripting;

/// RPC server for testing OpenZT functionality
#[cfg(feature = "reimplementation-tests")]
pub mod reimplementation_tests;

/// Integration tests that run via detours in live game (for CI)
#[cfg(feature = "integration-tests")]
pub mod integration_tests;

/// Pass-through logging stubs for validating addresses in generated.rs.
/// Annotate consts with #[validate_detour("name")] and run:
///   ./openzt.bat validate-detours [names...] (or "all")
#[cfg(all(feature = "detour-validation", target_os = "windows"))]
mod detour_validation;

#[cfg(target_os = "windows")]
use openzt_detour_macro::detour_mod;

#[cfg(target_os = "windows")]
use tracing::info;

#[cfg(target_os = "windows")]
#[detour_mod]
mod zoo_init {
    use super::*;
    use openzt_detour::generated::bfapp::LOAD_LANG_DLLS;

    // Note(finn): We hook the LoadLangDLLs function to perform some later initialization steps. Starting
    //  the console starts a new thead which is not recommended in the DllMain function.
    #[detour(LOAD_LANG_DLLS)]
    unsafe extern "thiscall" fn load_lang_dlls(this: u32) -> u32 {
        // Load config to determine logging settings
        let config = resource_manager::mod_config::get_openzt_config();

        // Initialize console and logging with config settings
        #[cfg(feature = "tui")]
        let tui_config = Some(&config.tui);
        #[cfg(not(feature = "tui"))]
        let tui_config = None;

        if let Err(e) = logging::init_with_console(&config.logging, tui_config) {
            eprintln!("Failed to initialize logging: {}", e);
            return 0; // Return 0 to indicate failure
        }

        info!("OpenZT initialization starting");

        #[cfg(feature = "detour-validation")]
        {
            let names_env = std::env::var("OPENZT_VALIDATE_DETOURS").unwrap_or_default();
            let names: Vec<&str> = names_env
                .split([',', ' '])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            detour_validation::init(&names);
        }

        // Initialize TUI if enabled
        #[cfg(feature = "tui")]
        if config.tui.enabled {
            if let Err(e) = tui_console::init(&config.tui) {
                info!("Failed to initialize TUI: {}", e);
            }
        }

        // Command console is broken on latest stable Rust so we disable it by default.
        if cfg!(feature = "command-console") {
            command_console::init();
        }
        resource_manager::init();
        dll_dependencies::init();
        expansions::init();
        string_registry::init();
        bugfix::init();
        version::init();
        ztui::init();
        ztworldmgr::init();
        bfentitytype::init();
        settings::init();
        scripting::init();
        shortcuts::init();
        roofs::init();

        if cfg!(feature = "capture_ztlog") {
            use crate::capture_ztlog;
            info!("Feature 'capture_ztlog' enabled");
            capture_ztlog::init();
        }

        if cfg!(feature = "experimental") {
            info!("Feature 'experimental' enabled");
            ztadvterrainmgr::init();
            ztgamemgr::init();
            experimental::init();
            ztmapview::init();
            zthabitatmgr::init();
        }
        unsafe { LOAD_LANG_DLLS_DETOUR.call(this) }
    }
}

#[cfg(target_os = "windows")]
pub fn init() {
    // If integration tests are enabled, run those instead of the main game
    #[cfg(feature = "integration-tests")]
    {
        integration_tests::init();
        return;
    }

    // Initialize the detours
    unsafe {
        zoo_init::init_detours().expect("Failed to initialize detours");
    }
}
