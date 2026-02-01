use crate::resource_manager::lazyresourcemap::get_file;
use crate::resource_manager::openzt_mods::{get_habitat_id, get_load_events, DefFileCategory, LoadEvent};
use openzt_configparser::ini::Ini;

use super::TestResult;

// ============================================================================
// Embedded Test Resources
// ============================================================================

#[cfg(feature = "integration-tests")]
mod embedded_resources {
    // Meta file
    pub const META_TOML: &str = include_str!("../../resources/test/loading-order-test/meta.toml");

    // Definition files
    pub const DEF_00_HABITAT: &str = include_str!("../../resources/test/loading-order-test/defs/00-habitat-only.toml");
    pub const DEF_01_LOCATION: &str = include_str!("../../resources/test/loading-order-test/defs/01-location-only.toml");
    pub const DEF_02_HABITAT: &str = include_str!("../../resources/test/loading-order-test/defs/02-another-habitat.toml");
    pub const DEF_CAPITALS: &str = include_str!("../../resources/test/loading-order-test/defs/Capitals-Test.toml");
    pub const DEF_50_MIXED: &str = include_str!("../../resources/test/loading-order-test/defs/50-mixed-content.toml");
    pub const DEF_51_MIXED: &str = include_str!("../../resources/test/loading-order-test/defs/51-second-mixed.toml");
    pub const DEF_98_PATCH: &str = include_str!("../../resources/test/loading-order-test/defs/98-early-patch.toml");
    pub const DEF_99_PATCH: &str = include_str!("../../resources/test/loading-order-test/defs/99-patches-only.toml");

    // Icon resources (reusing existing test icons)
    pub const ICON_DATA: &[u8] = include_bytes!("../../resources/test/N");
    pub const ICON_PALETTE: &[u8] = include_bytes!("../../resources/test/ltb.pal");
}

/// Create an in-memory file map for the test mod (mimics ZIP structure)
#[cfg(feature = "integration-tests")]
pub fn create_test_mod_file_map() -> std::collections::HashMap<String, Box<[u8]>> {
    use embedded_resources::*;

    let mut file_map = std::collections::HashMap::new();

    // Add meta.toml
    file_map.insert("meta.toml".to_string(), META_TOML.as_bytes().to_vec().into_boxed_slice());

    // Add definition files
    file_map.insert("defs/00-habitat-only.toml".to_string(), DEF_00_HABITAT.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/01-location-only.toml".to_string(), DEF_01_LOCATION.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/02-another-habitat.toml".to_string(), DEF_02_HABITAT.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/Capitals-Test.toml".to_string(), DEF_CAPITALS.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/50-mixed-content.toml".to_string(), DEF_50_MIXED.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/51-second-mixed.toml".to_string(), DEF_51_MIXED.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/98-early-patch.toml".to_string(), DEF_98_PATCH.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/99-patches-only.toml".to_string(), DEF_99_PATCH.as_bytes().to_vec().into_boxed_slice());

    // Add icon resources
    file_map.insert("resources/test/icon".to_string(), ICON_DATA.to_vec().into_boxed_slice());
    file_map.insert("resources/test/icon.pal".to_string(), ICON_PALETTE.to_vec().into_boxed_slice());

    file_map
}

/// Run all loading order tests
crate::integration_tests![
    test_category_ordering,
    test_alphabetical_within_nopatch,
    test_alphabetical_within_mixed,
    test_alphabetical_within_patchonly,
    test_case_insensitive_sorting,
    test_cross_file_habitat_reference,
    test_mixed_file_self_reference,
    test_patch_execution_order,
];

// ============================================================================
// Helper Functions
// ============================================================================

/// Helper to verify habitat is registered
fn check_habitat_registered(mod_id: &str, habitat_name: &str) -> Result<u32, String> {
    get_habitat_id(mod_id, habitat_name).ok_or_else(|| format!("Habitat '{}' not registered for mod '{}'", habitat_name, mod_id))
}

/// Helper to read INI key value from resource system
fn read_ini_key(file_path: &str, section: &str, key: &str) -> Result<String, String> {
    let (_filename, data) = get_file(file_path).ok_or_else(|| format!("File '{}' not found", file_path))?;

    let content = String::from_utf8_lossy(&data);
    let mut ini = Ini::new();
    ini.read(content.to_string()).map_err(|e| format!("Failed to parse INI: {}", e))?;

    ini.get(section, key).ok_or_else(|| format!("Key '{}.{}' not found in '{}'", section, key, file_path))
}

/// Helper to extract filenames from load events for a specific category and mod
fn get_filenames_for_category(events: &[LoadEvent], mod_id: &str, category: DefFileCategory) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.category == category && e.mod_id == mod_id)
        .map(|e| e.filename.clone())
        .collect()
}

/// Helper to extract just the filename (without path) for easier comparison
fn basename(path: &str) -> &str {
    path.split('/').last().unwrap_or(path)
}

// ============================================================================
// Test Implementations
// ============================================================================

fn test_category_ordering() -> TestResult {
    let test_name = "test_category_ordering";
    let mod_id = "loading_order_test";

    let events = get_load_events();
    let mod_events: Vec<&LoadEvent> = events.iter().filter(|e| e.mod_id == mod_id).collect();

    if mod_events.is_empty() {
        return TestResult::fail(test_name, "No load events found for test mod".to_string());
    }

    // Track the category we're currently in
    let mut current_category_order = -1i32;

    for event in mod_events {
        let category_order = match event.category {
            DefFileCategory::NoPatch => 0,
            DefFileCategory::Mixed => 1,
            DefFileCategory::PatchOnly => 2,
        };

        if category_order < current_category_order {
            return TestResult::fail(
                test_name,
                format!(
                    "Category ordering violated: {:?} (order {}) loaded after category order {}. File: {}",
                    event.category, category_order, current_category_order, event.filename
                ),
            );
        }

        current_category_order = category_order;
    }

    TestResult::pass(test_name)
}

fn test_alphabetical_within_nopatch() -> TestResult {
    let test_name = "test_alphabetical_within_nopatch";
    let mod_id = "loading_order_test";

    let events = get_load_events();
    let filenames = get_filenames_for_category(&events, mod_id, DefFileCategory::NoPatch);

    if filenames.is_empty() {
        return TestResult::fail(test_name, "No NoPatch files found".to_string());
    }

    // Expected order (case-insensitive alphabetical):
    // 00-habitat-only.toml, 01-location-only.toml, 02-another-habitat.toml, Capitals-Test.toml
    let expected = vec![
        "defs/00-habitat-only.toml",
        "defs/01-location-only.toml",
        "defs/02-another-habitat.toml",
        "defs/Capitals-Test.toml",
    ];

    if filenames.len() != expected.len() {
        return TestResult::fail(
            test_name,
            format!("Expected {} NoPatch files, found {}. Files: {:?}", expected.len(), filenames.len(), filenames),
        );
    }

    for (i, (actual, expected)) in filenames.iter().zip(expected.iter()).enumerate() {
        if actual != expected {
            return TestResult::fail(test_name, format!("File at position {} should be '{}', got '{}'", i, expected, actual));
        }
    }

    TestResult::pass(test_name)
}

fn test_alphabetical_within_mixed() -> TestResult {
    let test_name = "test_alphabetical_within_mixed";
    let mod_id = "loading_order_test";

    let events = get_load_events();
    let filenames = get_filenames_for_category(&events, mod_id, DefFileCategory::Mixed);

    if filenames.is_empty() {
        return TestResult::fail(test_name, "No Mixed files found".to_string());
    }

    // Expected order: 50-mixed-content.toml, 51-second-mixed.toml
    let expected = vec!["defs/50-mixed-content.toml", "defs/51-second-mixed.toml"];

    if filenames.len() != expected.len() {
        return TestResult::fail(
            test_name,
            format!("Expected {} Mixed files, found {}. Files: {:?}", expected.len(), filenames.len(), filenames),
        );
    }

    for (i, (actual, expected)) in filenames.iter().zip(expected.iter()).enumerate() {
        if actual != expected {
            return TestResult::fail(test_name, format!("File at position {} should be '{}', got '{}'", i, expected, actual));
        }
    }

    TestResult::pass(test_name)
}

fn test_alphabetical_within_patchonly() -> TestResult {
    let test_name = "test_alphabetical_within_patchonly";
    let mod_id = "loading_order_test";

    let events = get_load_events();
    let filenames = get_filenames_for_category(&events, mod_id, DefFileCategory::PatchOnly);

    if filenames.is_empty() {
        return TestResult::fail(test_name, "No PatchOnly files found".to_string());
    }

    // Expected order: 98-early-patch.toml, 99-patches-only.toml
    let expected = vec!["defs/98-early-patch.toml", "defs/99-patches-only.toml"];

    if filenames.len() != expected.len() {
        return TestResult::fail(
            test_name,
            format!("Expected {} PatchOnly files, found {}. Files: {:?}", expected.len(), filenames.len(), filenames),
        );
    }

    for (i, (actual, expected)) in filenames.iter().zip(expected.iter()).enumerate() {
        if actual != expected {
            return TestResult::fail(test_name, format!("File at position {} should be '{}', got '{}'", i, expected, actual));
        }
    }

    TestResult::pass(test_name)
}

fn test_case_insensitive_sorting() -> TestResult {
    let test_name = "test_case_insensitive_sorting";
    let mod_id = "loading_order_test";

    let events = get_load_events();
    let filenames = get_filenames_for_category(&events, mod_id, DefFileCategory::NoPatch);

    // Find position of Capitals-Test.toml
    let capitals_pos = filenames
        .iter()
        .position(|f| basename(f) == "Capitals-Test.toml")
        .ok_or_else(|| TestResult::fail(test_name, "Capitals-Test.toml not found".to_string()))
        .unwrap();

    // It should be the last NoPatch file (after 02-another-habitat.toml)
    // Because 'C' comes after '0', '1', '2' when sorting case-insensitively
    if capitals_pos != filenames.len() - 1 {
        return TestResult::fail(
            test_name,
            format!(
                "Capitals-Test.toml should be last NoPatch file (pos {}), but was at pos {}. Order: {:?}",
                filenames.len() - 1,
                capitals_pos,
                filenames
            ),
        );
    }

    TestResult::pass(test_name)
}

fn test_cross_file_habitat_reference() -> TestResult {
    let test_name = "test_cross_file_habitat_reference";
    let mod_id = "loading_order_test";

    // Verify test_habitat_a is registered (from 00-habitat-only.toml)
    let habitat_id = match check_habitat_registered(mod_id, "test_habitat_a") {
        Ok(id) => id,
        Err(e) => return TestResult::fail(test_name, e),
    };

    // Read animals/test.ai and verify cAlternateHabitat contains the habitat string ID
    // The patch in 99-patches-only.toml uses {habitats.test_habitat_a}
    match read_ini_key("animals/test.ai", "Habitat", "cAlternateHabitat") {
        Ok(value) => {
            let value_as_id = value.parse::<u32>().unwrap_or(0);
            if value_as_id != habitat_id {
                TestResult::fail(test_name, format!("cAlternateHabitat should be {} (habitat ID), got {}", habitat_id, value))
            } else {
                TestResult::pass(test_name)
            }
        }
        Err(e) => TestResult::fail(test_name, format!("Failed to read patch result: {}", e)),
    }
}

fn test_mixed_file_self_reference() -> TestResult {
    let test_name = "test_mixed_file_self_reference";
    let mod_id = "loading_order_test";

    // Verify test_habitat_b is registered (from 50-mixed-content.toml)
    let habitat_id = match check_habitat_registered(mod_id, "test_habitat_b") {
        Ok(id) => id,
        Err(e) => return TestResult::fail(test_name, e),
    };

    // Read animals/test.ai and verify cHabitat contains the habitat string ID
    // The patch in 50-mixed-content.toml uses {habitats.test_habitat_b}
    match read_ini_key("animals/test.ai", "Habitat", "cHabitat") {
        Ok(value) => {
            let value_as_id = value.parse::<u32>().unwrap_or(0);
            if value_as_id != habitat_id {
                TestResult::fail(test_name, format!("cHabitat should be {} (habitat ID), got {}", habitat_id, value))
            } else {
                TestResult::pass(test_name)
            }
        }
        Err(e) => TestResult::fail(test_name, format!("Failed to read patch result: {}", e)),
    }
}

fn test_patch_execution_order() -> TestResult {
    let test_name = "test_patch_execution_order";

    // Read animals/test_order.ai and verify LoadOrder key
    // 98-early-patch.toml sets it to "First"
    // 99-patches-only.toml sets it to "Second"
    // Since 99 loads after 98, final value should be "Second"
    match read_ini_key("animals/test_order.ai", "Test", "LoadOrder") {
        Ok(value) => {
            if value == "Second" {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("LoadOrder should be 'Second' (from 99-patches-only.toml), got '{}'", value))
            }
        }
        Err(e) => TestResult::fail(test_name, format!("Failed to read patch result: {}", e)),
    }
}
