use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::mods::{AddSectionPatch, DeletePatch, ErrorHandling, OnExists, Patch, PatchMeta, SetKeyPatch};
use crate::resource_manager::{
    lazyresourcemap::{add_ztfile, check_file, get_file, remove_resource},
    openzt_mods::patches::apply_patches,
    ztfile::{ZTFile, ZTFileType},
};

use super::TestResult;

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
    // Convert bytes to string
    Ok(String::from_utf8_lossy(&data).to_string())
}

/// Helper function to clean up a test file
fn cleanup_test_file(path: &str) {
    remove_resource(path);
}

/// Run all patch rollback tests
crate::integration_tests![
    test_continue_mode_applies_directly,
    test_continue_mode_skips_failed_patches,
    test_abort_mode_rolls_back_on_failure,
    test_abort_mode_commits_on_success,
    test_shadow_multiple_patches_same_file,
    test_shadow_file_deletion,
    test_shadow_create_and_delete_in_same_batch,
    test_shadow_resources_get_file_fallback,
    test_shadow_resources_delete_file,
];

fn test_continue_mode_applies_directly() -> TestResult {
    let test_name = "test_continue_mode_applies_directly";
    let test_file = "test_continue.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    // Create patches with Continue error handling
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
            condition: None,
        }),
    );

    // Apply patches
    let file_map = HashMap::new();
    if let Err(e) = apply_patches(&patch_meta, &patches, &file_map, "test_mod") {
        cleanup_test_file(test_file);
        return TestResult::fail(test_name, format!("Patches failed to apply: {}", e));
    }

    // Verify
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            // INI library writes without spaces: Key=Modified
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, format!("File not modified. Content: {}", content))
            }
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_continue_mode_skips_failed_patches() -> TestResult {
    let test_name = "test_continue_mode_skips_failed_patches";
    let test_file = "test_continue_skip.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Continue,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify_existing".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: None,
        }),
    );
    patches.insert(
        "modify_nonexistent".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: "nonexistent.ini".to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "ShouldFail".to_string(),
            condition: None,
        }),
    );
    patches.insert(
        "modify_after_failure".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "NewKey".to_string(),
            value: "AfterFailure".to_string(),
            condition: None,
        }),
    );

    // Apply patches (should continue on error)
    let file_map = HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "test_mod");

    // Verify
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            // INI library writes without spaces: Key=Modified
            if !content.contains("Key=Modified") && !content.contains("Key = Modified") {
                return TestResult::fail(test_name, "First patch should have applied".to_string());
            }
            if !content.contains("NewKey=AfterFailure") && !content.contains("NewKey = AfterFailure") {
                return TestResult::fail(test_name, "Patch after failure should have applied".to_string());
            }
            TestResult::pass(test_name)
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_abort_mode_rolls_back_on_failure() -> TestResult {
    let test_name = "test_abort_mode_rolls_back_on_failure";
    let test_file = "test_abort.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Abort,
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
            condition: None,
        }),
    );
    patches.insert(
        "fail".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: "nonexistent.ini".to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "ShouldNotApply".to_string(),
            condition: None,
        }),
    );

    // Apply patches (should fail and rollback)
    let file_map = HashMap::new();
    match apply_patches(&patch_meta, &patches, &file_map, "test_mod") {
        Ok(_) => {
            cleanup_test_file(test_file);
            return TestResult::fail(test_name, "Patches should have failed".to_string());
        }
        Err(_) => {} // Expected
    }

    // Verify rollback
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            // INI library writes without spaces: Key=Original
            if !content.contains("Key=Original") && !content.contains("Key = Original") {
                return TestResult::fail(test_name, "File should be unchanged after rollback".to_string());
            }
            if content.contains("Key=Modified") || content.contains("Key = Modified") {
                return TestResult::fail(test_name, "Modification should have been rolled back".to_string());
            }
            TestResult::pass(test_name)
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_abort_mode_commits_on_success() -> TestResult {
    let test_name = "test_abort_mode_commits_on_success";
    let test_file = "test_abort_success.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Abort,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify1".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Modified".to_string(),
            condition: None,
        }),
    );
    patches.insert(
        "modify2".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "NewKey".to_string(),
            value: "NewValue".to_string(),
            condition: None,
        }),
    );

    // Apply patches
    let file_map = HashMap::new();
    if let Err(e) = apply_patches(&patch_meta, &patches, &file_map, "test_mod") {
        cleanup_test_file(test_file);
        return TestResult::fail(test_name, format!("Patches failed to apply: {}", e));
    }

    // Verify
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            // INI library writes without spaces: Key=Modified
            if !content.contains("Key=Modified") && !content.contains("Key = Modified") {
                return TestResult::fail(test_name, "First modification should be committed".to_string());
            }
            if !content.contains("NewKey=NewValue") && !content.contains("NewKey = NewValue") {
                return TestResult::fail(test_name, "Second modification should be committed".to_string());
            }
            TestResult::pass(test_name)
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_shadow_multiple_patches_same_file() -> TestResult {
    let test_name = "test_shadow_multiple_patches_same_file";
    let test_file = "test_shadow_multiple.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section1]\nKey1 = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Abort,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "modify1".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section1".to_string(),
            key: "Key1".to_string(),
            value: "Modified1".to_string(),
            condition: None,
        }),
    );
    patches.insert(
        "add_section".to_string(),
        Patch::AddSection(AddSectionPatch {
            target: test_file.to_string(),
            section: "Section2".to_string(),
            keys: HashMap::new(),
            on_exists: OnExists::Skip,
            condition: None,
        }),
    );
    patches.insert(
        "modify2".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section2".to_string(),
            key: "Key2".to_string(),
            value: "Value2".to_string(),
            condition: None,
        }),
    );

    // Apply patches
    let file_map = HashMap::new();
    if let Err(e) = apply_patches(&patch_meta, &patches, &file_map, "test_mod") {
        cleanup_test_file(test_file);
        return TestResult::fail(test_name, format!("Patches failed to apply: {}", e));
    }

    // Verify
    match read_test_file(test_file) {
        Ok(content) => {
            cleanup_test_file(test_file);
            // INI library writes without spaces: Key1=Modified1
            if !content.contains("Key1=Modified1") && !content.contains("Key1 = Modified1") {
                return TestResult::fail(test_name, "First modification should be applied".to_string());
            }
            if !content.contains("[Section2]") {
                return TestResult::fail(test_name, "Section should be added".to_string());
            }
            if !content.contains("Key2=Value2") && !content.contains("Key2 = Value2") {
                return TestResult::fail(test_name, "Second modification should be applied".to_string());
            }
            TestResult::pass(test_name)
        }
        Err(e) => {
            cleanup_test_file(test_file);
            TestResult::fail(test_name, format!("Failed to read file: {}", e))
        }
    }
}

fn test_shadow_file_deletion() -> TestResult {
    let test_name = "test_shadow_file_deletion";
    let test_file = "test_shadow_delete.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Abort,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "delete".to_string(),
        Patch::Delete(DeletePatch {
            target: test_file.to_string(),
            condition: None,
        }),
    );

    // Apply patches
    let file_map = HashMap::new();
    if let Err(e) = apply_patches(&patch_meta, &patches, &file_map, "test_mod") {
        cleanup_test_file(test_file);
        return TestResult::fail(test_name, format!("Delete patch failed: {}", e));
    }

    // Verify
    let deleted = !check_file(test_file);
    if deleted {
        TestResult::pass(test_name)
    } else {
        cleanup_test_file(test_file);
        TestResult::fail(test_name, "File should be deleted after commit".to_string())
    }
}

fn test_shadow_create_and_delete_in_same_batch() -> TestResult {
    let test_name = "test_shadow_create_and_delete_in_same_batch";
    let test_file = "test_create_delete.ini";

    // Ensure file doesn't exist
    cleanup_test_file(test_file);

    let patch_meta = PatchMeta {
        on_error: ErrorHandling::Abort,
        condition: None,
    };

    let mut patches = indexmap::IndexMap::new();
    patches.insert(
        "create".to_string(),
        Patch::SetKey(SetKeyPatch {
            target: test_file.to_string(),
            section: "Section".to_string(),
            key: "Key".to_string(),
            value: "Value".to_string(),
            condition: None,
        }),
    );
    patches.insert(
        "delete".to_string(),
        Patch::Delete(DeletePatch {
            target: test_file.to_string(),
            condition: None,
        }),
    );

    // Apply patches
    let file_map = HashMap::new();
    let _ = apply_patches(&patch_meta, &patches, &file_map, "test_mod");

    // Verify
    let deleted = !check_file(test_file);
    cleanup_test_file(test_file);
    if deleted {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, "File should not exist after create+delete".to_string())
    }
}

fn test_shadow_resources_get_file_fallback() -> TestResult {
    let test_name = "test_shadow_resources_get_file_fallback";
    let test_file = "test_fallback.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    use crate::resource_manager::openzt_mods::patches::{ShadowResources, ShadowScope};

    // Create shadow with no files
    let shadow = match ShadowResources::new(&HashSet::new(), ShadowScope::PatchFile) {
        Ok(s) => s,
        Err(e) => {
            cleanup_test_file(test_file);
            return TestResult::fail(test_name, format!("Failed to create shadow: {}", e));
        }
    };

    // Get file should fall back to main resources
    let file = shadow.get_file(test_file);
    cleanup_test_file(test_file);

    if file.is_some() {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, "Should get file from main resources".to_string())
    }
}

fn test_shadow_resources_delete_file() -> TestResult {
    let test_name = "test_shadow_resources_delete_file";
    let test_file = "test_shadow_delete_method.ini";

    // Setup
    if let Err(e) = create_test_ini_file(test_file, "[Section]\nKey = Original\n") {
        return TestResult::fail(test_name, format!("Setup failed: {}", e));
    }

    use crate::resource_manager::openzt_mods::patches::{ShadowResources, ShadowScope};

    // Create shadow with this file
    let mut affected = HashSet::new();
    affected.insert(test_file.to_string());
    let mut shadow = match ShadowResources::new(&affected, ShadowScope::PatchFile) {
        Ok(s) => s,
        Err(e) => {
            cleanup_test_file(test_file);
            return TestResult::fail(test_name, format!("Failed to create shadow: {}", e));
        }
    };

    // Delete from shadow
    shadow.delete_file(test_file);

    // Verify file is no longer in shadow map
    let file_in_shadow = shadow.files.contains_key(test_file);
    cleanup_test_file(test_file);

    if !file_in_shadow {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, "File should not be in shadow map".to_string())
    }
}
