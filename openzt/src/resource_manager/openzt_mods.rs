pub(crate) mod habitats_locations;
pub(crate) mod extensions;
pub(crate) mod legacy_attributes;
pub(crate) mod loading;
pub mod patches;
pub(crate) mod ztd_registry;
pub(crate) mod entity_lookup;

pub use crate::resource_manager::openzt_mods::{
    habitats_locations::{get_location_habitat_ids, get_location_or_habitat_by_id},
    loading::{discover_mods, get_mod_ids, get_num_mod_ids, load_open_zt_mod},
};

// Re-export items needed for integration tests
#[cfg(feature = "integration-tests")]
pub use crate::resource_manager::openzt_mods::{
    habitats_locations::{get_habitat_id, get_location_id},
    loading::{
        clear_load_tracker,
        get_load_events,
        load_open_zt_mod_from_memory,
        DefFileCategory,
        LoadEvent,
    },
};
