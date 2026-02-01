//! Integration tests for `ztd_loaded` and `entity_exists` patch conditions.

use super::TestResult;
use crate::mods::{ErrorHandling, Patch, PatchCondition, PatchMeta, SetKeyPatch};
use crate::resource_manager::{
    lazyresourcemap::{add_ztfile, get_file, remove_resource},
    openzt_mods::legacy_attributes::{add_legacy_entity, LegacyEntityAttributes, LegacyEntityType, SubtypeAttributes},
    openzt_mods::patches::apply_patches,
    openzt_mods::ztd_registry::{self, ZtdLoadStatus},
    ztfile::{ZTFile, ZTFileType},
};
use std::path::Path;

/// Helper function to create a test INI file in the resource system
fn create_test_ini_file(path: &str, content: &str) -> anyhow::Result<()> {
    let content_len = content.len() as u32;
    let c_string = std::ffi::CString::new(content)?;
    let file_type = ZTFileType::try_from(Path::new(path)).map_err(|e| anyhow::anyhow!("Invalid file type: {}", e))?;
    let ztfile = ZTFile::Text(c_string, file_type, content_len);
    add_ztfile(Path::new(""), path.to_string(), ztfile)?;
    Ok(())
}

/// Helper function to read a file from the resource system as string
fn read_test_file(path: &str) -> anyhow::Result<String> {
    let (_filename, data) = get_file(path).ok_or_else(|| anyhow::anyhow!("File '{}' not found", path))?;
    Ok(String::from_utf8_lossy(&data).to_string())
}

/// Helper function to clean up a test file
fn cleanup_test_file(path: &str) {
    remove_resource(path);
}

/// Helper function to add a test legacy entity
fn add_test_legacy_entity(entity_type: LegacyEntityType, entity_name: &str) {
    let mut subtype_map = std::collections::HashMap::new();
    subtype_map.insert(
        String::new(),
        SubtypeAttributes {
            subtype: String::new(),
            name_id: Some(12345),
        },
    );

    let attributes = LegacyEntityAttributes {
        entity_name: entity_name.to_string(),
        subtype_attributes: subtype_map,
    };

    let _ = add_legacy_entity(entity_type, entity_name.to_string(), attributes);
}

/// Run all patch condition tests
crate::integration_tests![
    test_ztd_loaded_passes_when_ztd_loaded_before,
    test_ztd_loaded_skips_when_ztd_loaded_after,
    test_ztd_loaded_skips_when_ztd_disabled,
    test_ztd_loaded_skips_when_ztd_not_registered,
    test_ztd_loaded_case_insensitive,
    test_entity_exists_passes_when_entity_exists,
    test_entity_exists_skips_when_entity_not_exists,
    test_entity_exists_all_entity_types,
    test_entity_exists_case_insensitive,
    test_entity_exists_invalid_format,
    test_entity_exists_invalid_entity_type,
    test_combined_ztd_loaded_and_entity_exists,
    test_combined_ztd_loaded_fails_blocks_patch,
    test_combined_entity_exists_fails_blocks_patch,
];

// ============================================================================
// Tests for ztd_loaded condition
// ============================================================================

fn test_ztd_loaded_passes_when_ztd_loaded_before() -> TestResult {
    let test_name = "test_ztd_loaded_passes_when_ztd_loaded_before";
    let test_file = "test_ztd_before.ini";

    // Setup: Clear registry and register ZTDs
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("base.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    // Create patch with ztd_loaded condition
    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),
                entity_exists: None,
            }),
        }),
    );

    // Apply patches
    let file_map = std::collections::HashMap::new();
    if let Err(e) = apply_patches(&patch_meta, &patches, &file_map, "mymod") {
        cleanup_test_file(test_file);
        return TestResult::fail(test_name, format!("Patches failed to apply: {}", e));
    }

    // Verify
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch not applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_ztd_loaded_skips_when_ztd_loaded_after() -> TestResult {
    let test_name = "test_ztd_loaded_skips_when_ztd_loaded_after";
    let test_file = "test_ztd_after.ini";

    // Setup: Register current mod first, then base ZTD
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");
    ztd_registry::register_ztd("base.ztd", ZtdLoadStatus::Enabled);

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),
                entity_exists: None,
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_ztd_loaded_skips_when_ztd_disabled() -> TestResult {
    let test_name = "test_ztd_loaded_skips_when_ztd_disabled";
    let test_file = "test_ztd_disabled.ini";

    // Setup: Register base ZTD as disabled
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("base.ztd", ZtdLoadStatus::Disabled);
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),
                entity_exists: None,
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_ztd_loaded_skips_when_ztd_not_registered() -> TestResult {
    let test_name = "test_ztd_loaded_skips_when_ztd_not_registered";
    let test_file = "test_ztd_notfound.ini";

    // Setup: Only register current mod, don't register the required ZTD
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("nonexistent.ztd".to_string()),
                entity_exists: None,
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_ztd_loaded_case_insensitive() -> TestResult {
    let test_name = "test_ztd_loaded_case_insensitive";
    let test_file = "test_ztd_case.ini";

    // Setup: Register with uppercase, check with lowercase
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("MyMod.ZTD", ZtdLoadStatus::Enabled);
    ztd_registry::register_ztd("current.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("current", "current.ztd");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("mymod.ztd".to_string()), // lowercase
                entity_exists: None,
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "current");

    // Verify patch was applied (case insensitive match)
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

// ============================================================================
// Tests for entity_exists condition
// ============================================================================

fn test_entity_exists_passes_when_entity_exists() -> TestResult {
    let test_name = "test_entity_exists_passes_when_entity_exists";
    let test_file = "test_entity_exists.ini";

    // Setup: Add a test legacy entity
    add_test_legacy_entity(LegacyEntityType::Animal, "elephant");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: None,
                entity_exists: Some("legacy.animals.elephant".to_string()),
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_entity_exists_skips_when_entity_not_exists() -> TestResult {
    let test_name = "test_entity_exists_skips_when_entity_not_exists";
    let test_file = "test_entity_notexists.ini";

    // Don't add any entity

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: None,
                entity_exists: Some("legacy.animals.dragon".to_string()), // doesn't exist
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_entity_exists_all_entity_types() -> TestResult {
    let test_name = "test_entity_exists_all_entity_types";
    let test_file = "test_all_entity_types.ini";

    // Test all entity types
    let entity_types = [
        (LegacyEntityType::Animal, "animals", "lion"),
        (LegacyEntityType::Building, "buildings", "restroom"),
        (LegacyEntityType::Fence, "fences", "fence"),
        (LegacyEntityType::Food, "food", "burger"),
        (LegacyEntityType::Guest, "guests", "guest"),
        (LegacyEntityType::Item, "items", "item"),
        (LegacyEntityType::Path, "paths", "path"),
        (LegacyEntityType::Scenery, "scenery", "rock"),
        (LegacyEntityType::Staff, "staff", "keeper"),
        (LegacyEntityType::Wall, "walls", "wall"),
    ];

    for (entity_type, _type_str, entity_name) in &entity_types {
        add_test_legacy_entity(*entity_type, entity_name);
    }

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    // Test first entity type
    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: None,
                entity_exists: Some(format!("legacy.{}.lion", entity_types[0].1)),
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_entity_exists_case_insensitive() -> TestResult {
    let test_name = "test_entity_exists_case_insensitive";
    let test_file = "test_entity_case.ini";

    // Add entity with uppercase name
    add_test_legacy_entity(LegacyEntityType::Animal, "Elephant");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: None,
                entity_exists: Some("legacy.animals.elephant".to_string()), // lowercase
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was applied (case insensitive match)
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_entity_exists_invalid_format() -> TestResult {
    let test_name = "test_entity_exists_invalid_format";
    let test_file = "test_entity_invalid_format.ini";

    // Test various invalid formats
    let invalid_formats = ["invalid", "legacy.", "legacy.animals"];

    let mut all_passed = true;
    let mut error_msg = String::new();

    for invalid_format in &invalid_formats {
        if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
            return TestResult::fail(test_name, format!("Setup failed: {}", e));
        }

        let patch_meta = PatchMeta {
            on_error: ErrorHandling::Continue,
            condition: None,
        };

        let mut patches = indexmap::IndexMap::new();
        patches.insert(
            "modify".to_string(),
            Patch::SetKey(SetKeyPatch {
                target: test_file.to_string(),
                section: "Section".to_string(),
                key: "Key".to_string(),
                value: "Modified".to_string(),
                condition: Some(PatchCondition {
                    target: None,
                    mod_loaded: None,
                    key_exists: None,
                    value_equals: None,
                    ztd_loaded: None,
                    entity_exists: Some(invalid_format.to_string()),
                }),
            }),
        );

        let file_map = std::collections::HashMap::new();
        let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

        // Verify patch was NOT applied
        match read_test_file(test_file) {
            Ok(content) => {
                cleanup_test_file(test_file);
                let has_original = content.contains("Key=Original") || content.contains("Key = Original");
                if !has_original || content.contains("Modified") {
                    all_passed = false;
                    error_msg = format!("Invalid format '{}' should have been skipped", invalid_format);
                    break;
                }
            }
            Err(e) => {
                cleanup_test_file(test_file);
                all_passed = false;
                error_msg = format!("Failed to read file: {}", e);
                break;
            }
        }
    }

    if all_passed {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, error_msg)
    }
}

fn test_entity_exists_invalid_entity_type() -> TestResult {
    let test_name = "test_entity_exists_invalid_entity_type";
    let test_file = "test_entity_invalid_type.ini";

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: None,
                entity_exists: Some("legacy.dragons.elephant".to_string()), // invalid entity type
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

// ============================================================================
// Tests for combined conditions
// ============================================================================

fn test_combined_ztd_loaded_and_entity_exists() -> TestResult {
    let test_name = "test_combined_ztd_loaded_and_entity_exists";
    let test_file = "test_combined_both.ini";

    // Setup: Register ZTD and add legacy entity
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("base.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");
    add_test_legacy_entity(LegacyEntityType::Animal, "elephant");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),
                entity_exists: Some("legacy.animals.elephant".to_string()),
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was applied
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been applied. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_combined_ztd_loaded_fails_blocks_patch() -> TestResult {
    let test_name = "test_combined_ztd_loaded_fails_blocks_patch";
    let test_file = "test_combined_ztd_fails.ini";

    // Setup: Don't register base ZTD, but add entity
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");
    add_test_legacy_entity(LegacyEntityType::Animal, "elephant");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),                   // fails
                entity_exists: Some("legacy.animals.elephant".to_string()), // passes
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied (ztd_loaded condition fails)
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_combined_entity_exists_fails_blocks_patch() -> TestResult {
    let test_name = "test_combined_entity_exists_fails_blocks_patch";
    let test_file = "test_combined_entity_fails.ini";

    // Setup: Register base ZTD, but don't add entity
    ztd_registry::clear_registry_for_tests();
    ztd_registry::register_ztd("base.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_ztd("mymod.ztd", ZtdLoadStatus::Enabled);
    ztd_registry::register_mod_ztd("mymod", "mymod.ztd");

    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: Some(PatchCondition {
                target: None,
                mod_loaded: None,
                key_exists: None,
                value_equals: None,
                ztd_loaded: Some("base.ztd".to_string()),                 // passes
                entity_exists: Some("legacy.animals.dragon".to_string()), // fails
            }),
        }),
    );

    let file_map = std::collections::HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "mymod");

    // Verify patch was NOT applied (entity_exists condition fails)
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            if (content.contains("Key=Original") || content.contains("Key = Original")) && !content.contains("Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("Patch should have been skipped. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}
