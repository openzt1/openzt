mod bfresourcemgr;
mod commands;
mod handlers;
mod hooks;
pub(crate) mod lazyresourcemap;
#[cfg(feature = "integration-tests")]
pub mod legacy_loading;
#[cfg(not(feature = "integration-tests"))]
mod legacy_loading;
pub(crate) mod openzt_mods;
mod ztd;
pub(crate) mod ztfile;

// Export for integration tests
#[cfg(feature = "integration-tests")]
pub mod dependency_resolver;
#[cfg(feature = "integration-tests")]
pub mod validation;

// Private modules when not testing
#[cfg(not(feature = "integration-tests"))]
mod dependency_resolver;
#[cfg(not(feature = "integration-tests"))]
mod validation;

// Always available internally for config loading
pub(crate) mod mod_config;

use commands::init_commands;
pub use handlers::{add_handler, Handler, RunStage};
use hooks::init_hooks;
pub use legacy_loading::OPENZT_DIR0;

// Export for integration tests
#[cfg(feature = "integration-tests")]
pub use legacy_loading::{load_legacy_entities_for_tests, load_legacy_entities_from_test_files};

pub use ztfile::{modify_ztfile_as_animation, modify_ztfile_as_ini};

///Initializes hooks and commands for the resource manager
pub fn init() {
    init_hooks();
    init_commands();
}
