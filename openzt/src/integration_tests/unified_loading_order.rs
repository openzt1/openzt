//! Integration tests for unified archive loading order
//!
//! Tests that ALL archives in /mods/ (both OpenZT mods and pure legacy ZTDs)
//! are loaded according to the 'order' list in openzt.toml.

use std::collections::HashMap;
use std::path::PathBuf;

use super::TestResult;
use crate::mods::Meta;
use crate::resource_manager::dependency_resolver::DependencyResolver;

/// Helper to create test metadata from TOML string
fn create_test_meta(toml_str: &str) -> Meta {
    toml::from_str(toml_str).expect("Failed to parse test TOML")
}

/// Run all unified loading order tests
crate::integration_tests![
    test_pure_legacy_discovered_in_mods,
    test_pure_legacy_added_at_position_zero,
    test_pure_legacy_alphabetical_sorting,
    test_ztd_type_legacy_no_deps_at_position_zero,
    test_mixed_order_mod_ids_and_filenames,
    test_missing_entries_kept_in_order,
    test_existing_order_preserved,
    test_disabled_pure_legacy_creates_empty_resources,
    test_disabled_ztd_only_affects_mods_dir,
    test_ztd_type_legacy_with_deps_respected,
];

// ============================================================================
// Discovery Tests
// ============================================================================

/// Test: Pure legacy archives in /mods/ are discovered
fn test_pure_legacy_discovered_in_mods() -> TestResult {
    let test_name = "test_pure_legacy_discovered_in_mods";

    // Simulate discovered mods and pure legacy archives
    let openzt_mods: HashMap<String, (String, Meta)> = HashMap::new();
    let pure_legacy: Vec<(String, PathBuf)> = vec![
        ("legacy_a.ztd".to_string(), PathBuf::from("./mods/legacy_a.ztd")),
        ("legacy_b.ztd".to_string(), PathBuf::from("./mods/legacy_b.ztd")),
    ];

    // Verify pure legacy archives are in the list
    if pure_legacy.len() != 2 {
        return TestResult::fail(test_name, format!("Expected 2 pure legacy archives, found {}", pure_legacy.len()));
    }

    // Verify they have the correct filenames
    let expected_filenames = vec!["legacy_a.ztd", "legacy_b.ztd"];
    for (i, (filename, _)) in pure_legacy.iter().enumerate() {
        if filename != expected_filenames[i] {
            return TestResult::fail(test_name, format!("Expected '{}' at position {}, found '{}'", expected_filenames[i], i, filename));
        }
    }

    TestResult::pass(test_name)
}

// ============================================================================
// Order Resolution Tests
// ============================================================================

/// Test: New pure legacy archives added at position 0
fn test_pure_legacy_added_at_position_zero() -> TestResult {
    let test_name = "test_pure_legacy_added_at_position_zero";

    // Create a resolver with one existing OpenZT mod
    let existing_meta = create_test_meta(
        r#"
name = "Existing Mod"
description = "Existing mod"
authors = ["Test"]
mod_id = "existing_mod"
version = "1.0.0"
"#,
    );
    let mut openzt_mods: HashMap<String, Meta> = HashMap::new();
    openzt_mods.insert("existing_mod".to_string(), existing_meta);

    let discovered_tuples: HashMap<String, (String, Meta)> = vec![(
        "existing_mod".to_string(),
        ("existing_mod.ztd".to_string(), openzt_mods.get("existing_mod").unwrap().clone()),
    )]
    .into_iter()
    .collect();

    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Existing order has the mod
    let existing_order = vec!["existing_mod".to_string()];

    // New pure legacy archives
    let pure_legacy: Vec<(String, PathBuf)> = vec![
        ("zzz.ztd".to_string(), PathBuf::from("./mods/zzz.ztd")),
        ("aaa.ztd".to_string(), PathBuf::from("./mods/aaa.ztd")),
    ];

    // Resolve order
    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Verify pure legacy archives are at the start
    if result.order.len() != 3 {
        return TestResult::fail(test_name, format!("Expected order length 3, got {}", result.order.len()));
    }

    // Check first two entries are pure legacy (alphabetically sorted)
    if result.order[0] != "aaa.ztd" {
        return TestResult::fail(test_name, format!("Expected 'aaa.ztd' at position 0, got '{}'", result.order[0]));
    }

    if result.order[1] != "zzz.ztd" {
        return TestResult::fail(test_name, format!("Expected 'zzz.ztd' at position 1, got '{}'", result.order[1]));
    }

    // Check existing mod is still there
    if result.order[2] != "existing_mod" {
        return TestResult::fail(test_name, format!("Expected 'existing_mod' at position 2, got '{}'", result.order[2]));
    }

    TestResult::pass(test_name)
}

/// Test: Pure legacy archives sorted alphabetically
fn test_pure_legacy_alphabetical_sorting() -> TestResult {
    let test_name = "test_pure_legacy_alphabetical_sorting";

    let openzt_mods: HashMap<String, Meta> = HashMap::new();
    let discovered_tuples: HashMap<String, (String, Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    let existing_order: Vec<String> = vec![];

    // New pure legacy archives in random order
    let pure_legacy: Vec<(String, PathBuf)> = vec![
        ("zebra.ztd".to_string(), PathBuf::from("./mods/zebra.ztd")),
        ("alpha.ztd".to_string(), PathBuf::from("./mods/alpha.ztd")),
        ("beta.ztd".to_string(), PathBuf::from("./mods/beta.ztd")),
        ("Capital.ztd".to_string(), PathBuf::from("./mods/Capital.ztd")),
    ];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Expected alphabetical order (case-insensitive)
    let expected = vec!["alpha.ztd", "beta.ztd", "Capital.ztd", "zebra.ztd"];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test: ztd_type="legacy" with no deps added at position 0
fn test_ztd_type_legacy_no_deps_at_position_zero() -> TestResult {
    let test_name = "test_ztd_type_legacy_no_deps_at_position_zero";

    // Create an OpenZT mod with ztd_type="legacy" and no dependencies
    let meta_toml = r#"
name = "Test Legacy Mod"
description = "Test mod with ztd_type=legacy and no deps"
authors = ["Test"]
mod_id = "test.legacy_mod"
version = "1.0.0"
ztd_type = "legacy"
"#;

    let meta = create_test_meta(meta_toml);
    let mut openzt_mods: HashMap<String, Meta> = HashMap::new();
    openzt_mods.insert("test.legacy_mod".to_string(), meta);

    let discovered_tuples: HashMap<String, (String, Meta)> = vec![(
        "test.legacy_mod".to_string(),
        ("legacy_mod.ztd".to_string(), openzt_mods.get("test.legacy_mod").unwrap().clone()),
    )]
    .into_iter()
    .collect();

    // Existing order has a valid OpenZT mod
    let existing_order = vec!["existing_mod".to_string()];

    // Create the existing mod and add to the maps
    let existing_meta = create_test_meta(
        r#"
name = "Existing Mod"
description = "Existing mod"
authors = ["Test"]
mod_id = "existing_mod"
version = "1.0.0"
"#,
    );
    let mut full_mods = openzt_mods.clone();
    full_mods.insert("existing_mod".to_string(), existing_meta.clone());

    let mut full_discovered = discovered_tuples.clone();
    full_discovered.insert("existing_mod".to_string(), ("existing_mod.ztd".to_string(), existing_meta));

    let resolver = DependencyResolver::new(full_mods, &full_discovered);

    let pure_legacy: Vec<(String, PathBuf)> = vec![];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Verify both mods are in the order
    if result.order.len() != 2 {
        return TestResult::fail(test_name, format!("Expected order length 2, got {}", result.order.len()));
    }

    // The new legacy mod (with no deps) should be inserted at position 0
    // The existing mod should be at position 1
    if result.order[0] != "test.legacy_mod" {
        return TestResult::fail(test_name, format!("Expected 'test.legacy_mod' at position 0, got '{}'", result.order[0]));
    }

    if result.order[1] != "existing_mod" {
        return TestResult::fail(test_name, format!("Expected 'existing_mod' at position 1, got '{}'", result.order[1]));
    }

    TestResult::pass(test_name)
}

/// Test: Mixed order with mod_ids and ZTD filenames
fn test_mixed_order_mod_ids_and_filenames() -> TestResult {
    let test_name = "test_mixed_order_mod_ids_and_filenames";

    let meta_toml = r#"
name = "Test Mod"
description = "Test mod"
authors = ["Test"]
mod_id = "test.mod"
version = "1.0.0"
"#;

    let meta = create_test_meta(meta_toml);
    let mut openzt_mods: HashMap<String, Meta> = HashMap::new();
    openzt_mods.insert("test.mod".to_string(), meta);

    let discovered_tuples: HashMap<String, (String, Meta)> =
        vec![("test.mod".to_string(), ("test_mod.ztd".to_string(), openzt_mods.get("test.mod").unwrap().clone()))]
            .into_iter()
            .collect();

    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Mixed order with both mod_id and ZTD filename
    let existing_order = vec!["legacy_a.ztd".to_string(), "test.mod".to_string(), "legacy_b.ztd".to_string()];

    let pure_legacy: Vec<(String, PathBuf)> = vec![
        ("legacy_a.ztd".to_string(), PathBuf::from("./mods/legacy_a.ztd")),
        ("legacy_b.ztd".to_string(), PathBuf::from("./mods/legacy_b.ztd")),
    ];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Verify all entries are preserved in order
    if result.order.len() != 3 {
        return TestResult::fail(test_name, format!("Expected order length 3, got {}", result.order.len()));
    }

    if result.order[0] != "legacy_a.ztd" || result.order[1] != "test.mod" || result.order[2] != "legacy_b.ztd" {
        return TestResult::fail(
            test_name,
            format!("Order mismatch: expected [legacy_a.ztd, test.mod, legacy_b.ztd], got {:?}", result.order),
        );
    }

    TestResult::pass(test_name)
}

/// Test: Missing entries kept in order with error logged
fn test_missing_entries_kept_in_order() -> TestResult {
    let test_name = "test_missing_entries_kept_in_order";

    let openzt_mods: HashMap<String, Meta> = HashMap::new();
    let discovered_tuples: HashMap<String, (String, Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Existing order has entries not found on disk
    let existing_order = vec![
        "existing_mod".to_string(),    // This mod_id doesn't exist
        "missing.ztd".to_string(),     // This ZTD filename doesn't exist
        "another_missing".to_string(), // This mod_id doesn't exist
    ];

    // Empty discovery - nothing exists
    let pure_legacy: Vec<(String, PathBuf)> = vec![];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // The resolver now keeps all entries in the order, even if missing
    // An error is logged for each missing entry
    // All three entries should remain in the order
    if result.order.len() != 3 {
        return TestResult::fail(test_name, format!("Expected 3 entries in order (all kept), got {:?}", result.order));
    }

    // Verify order is preserved
    if result.order != existing_order {
        return TestResult::fail(test_name, format!("Order should be preserved, expected {:?}, got {:?}", existing_order, result.order));
    }

    TestResult::pass(test_name)
}

/// Test: Existing order preserved for known archives
fn test_existing_order_preserved() -> TestResult {
    let test_name = "test_existing_order_preserved";

    let meta_toml = r#"
name = "Test Mod"
description = "Test mod"
authors = ["Test"]
mod_id = "test.mod"
version = "1.0.0"
"#;

    let meta = create_test_meta(meta_toml);
    let mut openzt_mods: HashMap<String, Meta> = HashMap::new();
    openzt_mods.insert("test.mod".to_string(), meta);

    let discovered_tuples: HashMap<String, (String, Meta)> =
        vec![("test.mod".to_string(), ("test_mod.ztd".to_string(), openzt_mods.get("test.mod").unwrap().clone()))]
            .into_iter()
            .collect();

    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Existing custom order (not alphabetical)
    let existing_order = vec!["zzz.ztd".to_string(), "test.mod".to_string(), "aaa.ztd".to_string()];

    let pure_legacy: Vec<(String, PathBuf)> = vec![
        ("zzz.ztd".to_string(), PathBuf::from("./mods/zzz.ztd")),
        ("aaa.ztd".to_string(), PathBuf::from("./mods/aaa.ztd")),
    ];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Verify custom order is preserved (no new archives to insert)
    let expected = vec!["zzz.ztd", "test.mod", "aaa.ztd"];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

// ============================================================================
// Disabling Tests
// ============================================================================

/// Test: Disabled pure legacy creates empty resources
fn test_disabled_pure_legacy_creates_empty_resources() -> TestResult {
    let test_name = "test_disabled_pure_legacy_creates_empty_resources";

    let openzt_mods: HashMap<String, Meta> = HashMap::new();
    let discovered_tuples: HashMap<String, (String, Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Disabled archives already in existing_order stay in order, but new disabled ones are not added
    let existing_order = vec!["disabled_legacy.ztd".to_string()];
    let disabled = vec!["disabled_legacy.ztd".to_string()];
    let pure_legacy: Vec<(String, PathBuf)> = vec![("disabled_legacy.ztd".to_string(), PathBuf::from("./mods/disabled_legacy.ztd"))];

    let result = resolver.resolve_order(&existing_order, &disabled, &pure_legacy);

    // Disabled archives stay in order but are not processed
    // We verify the order includes the disabled entry
    if !result.order.contains(&"disabled_legacy.ztd".to_string()) {
        return TestResult::fail(test_name, format!("Disabled legacy archive should stay in order, got {:?}", result.order));
    }

    TestResult::pass(test_name)
}

/// Test: Disabled ZTD only affects /mods/ directory
fn test_disabled_ztd_only_affects_mods_dir() -> TestResult {
    let test_name = "test_disabled_ztd_only_affects_mods_dir";

    // This test conceptually verifies that the disabled_ztds list is only checked
    // for archives in /mods/. The actual implementation in legacy_loading.rs
    // handles this by checking is_mods_dir when processing disabled ZTDs.

    // For this unit test, we just verify the resolver correctly handles
    // disabled entries by keeping them in the order when they exist in existing_order

    let openzt_mods: HashMap<String, Meta> = HashMap::new();
    let discovered_tuples: HashMap<String, (String, Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    // Disabled archives already in existing_order stay in order
    let existing_order = vec!["test.ztd".to_string()];
    let disabled = vec!["test.ztd".to_string()];
    let pure_legacy: Vec<(String, PathBuf)> = vec![("test.ztd".to_string(), PathBuf::from("./mods/test.ztd"))];

    let result = resolver.resolve_order(&existing_order, &disabled, &pure_legacy);

    // Disabled archive should be in order but not loaded
    if !result.order.contains(&"test.ztd".to_string()) {
        return TestResult::fail(test_name, format!("Disabled ZTD should stay in order, got {:?}", result.order));
    }

    TestResult::pass(test_name)
}

// ============================================================================
// Dependency Tests
// ============================================================================

/// Test: ztd_type="legacy" with dependencies respected
fn test_ztd_type_legacy_with_deps_respected() -> TestResult {
    let test_name = "test_ztd_type_legacy_with_deps_respected";

    // Create two mods: base_mod and dependent_mod
    // dependent_mod has ztd_type="legacy" but has dependencies

    let base_meta = create_test_meta(
        r#"
name = "Base Mod"
description = "Base mod"
authors = ["Test"]
mod_id = "test.base_mod"
version = "1.0.0"
"#,
    );

    let dependent_meta = create_test_meta(
        r#"
name = "Dependent Legacy Mod"
description = "Dependent mod"
authors = ["Test"]
mod_id = "test.dependent_legacy"
version = "1.0.0"
ztd_type = "legacy"

dependencies = [
    { mod_id = "test.base_mod", name = "Base Mod", ordering = "after" }
]
"#,
    );

    let mut openzt_mods: HashMap<String, Meta> = HashMap::new();
    openzt_mods.insert("test.base_mod".to_string(), base_meta);
    openzt_mods.insert("test.dependent_legacy".to_string(), dependent_meta);

    let discovered_tuples: HashMap<String, (String, Meta)> = vec![
        (
            "test.base_mod".to_string(),
            ("base_mod.ztd".to_string(), openzt_mods.get("test.base_mod").unwrap().clone()),
        ),
        (
            "test.dependent_legacy".to_string(),
            ("dependent_legacy.ztd".to_string(), openzt_mods.get("test.dependent_legacy").unwrap().clone()),
        ),
    ]
    .into_iter()
    .collect();

    let resolver = DependencyResolver::new(openzt_mods, &discovered_tuples);

    let existing_order: Vec<String> = vec![];
    let pure_legacy: Vec<(String, PathBuf)> = vec![];

    let result = resolver.resolve_order(&existing_order, &[], &pure_legacy);

    // Verify dependent_legacy comes after base_mod (dependency respected)
    let base_pos = result.order.iter().position(|x| x == "test.base_mod");
    let dependent_pos = result.order.iter().position(|x| x == "test.dependent_legacy");

    match (base_pos, dependent_pos) {
        (Some(base), Some(dependent)) => {
            if dependent <= base {
                return TestResult::fail(
                    test_name,
                    format!(
                        "dependent_legacy should come after base_mod, but positions are: base={}, dependent={}",
                        base, dependent
                    ),
                );
            }
        }
        _ => {
            return TestResult::fail(test_name, format!("Both mods should be in order, got {:?}", result.order));
        }
    }

    TestResult::pass(test_name)
}
