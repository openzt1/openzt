//! Integration tests for patch source file resolution from .ztd archives
//!
//! Tests that patch source files can be read from within the .ztd archive
//! under the `resources/` subdirectory.

use super::TestResult;

const DEFS_TOML: &str = include_str!("../../resources/test/patch_source_resolution/defs/01-patches.toml");
const MERGE_SOURCE: &str = include_str!("../../resources/test/patch_source_resolution/resources/test_merge_source.ai");
const REPLACE_SOURCE: &str = include_str!("../../resources/test/patch_source_resolution/resources/test_replace_source.ai");
const MULTI_SOURCE: &str = include_str!("../../resources/test/patch_source_resolution/resources/test_multi_source.ai");

crate::integration_tests![
    test_source_file_from_archive,
    test_merge_from_archive,
    test_replace_from_archive,
    test_missing_source_file_error,
    test_source_file_wrong_path_error,
];

/// Helper function to create meta.toml content with a given mod_id
fn create_meta_toml(mod_id: &str) -> String {
    format!(r#"name = "Patch Source Resolution Test"
mod_id = "{}"
version = "0.1.0"
description = "Tests that patch source files can be read from .ztd archive"
authors = ["OpenZT"]
ztd_type = "openzt"
"#, mod_id)
}

/// Helper function to load a test mod with the given mod_id
fn load_test_mod(mod_id: &str) -> Result<(), String> {
    // Create test mod file map
    let mut file_map = std::collections::HashMap::new();
    file_map.insert("meta.toml".to_string(), create_meta_toml(mod_id).as_bytes().to_vec().into_boxed_slice());
    file_map.insert("defs/01-patches.toml".to_string(), DEFS_TOML.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("resources/test_merge_source.ai".to_string(), MERGE_SOURCE.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("resources/test_replace_source.ai".to_string(), REPLACE_SOURCE.as_bytes().to_vec().into_boxed_slice());
    file_map.insert("resources/test_multi_source.ai".to_string(), MULTI_SOURCE.as_bytes().to_vec().into_boxed_slice());

    // Load the test mod
    crate::resource_manager::openzt_mods::loading::load_open_zt_mod_from_memory(
        file_map,
        mod_id,
        std::path::Path::new("dummy"),
    ).map(|_| ()).map_err(|e| format!("Failed to load test mod: {}", e))
}

fn test_source_file_from_archive() -> TestResult {
    let test_name = "test_source_file_from_archive";

    if let Err(e) = load_test_mod("patch_source_test") {
        return TestResult::fail(test_name, e);
    }

    TestResult::pass(test_name)
}

fn test_merge_from_archive() -> TestResult {
    let test_name = "test_merge_from_archive";

    // First create the target file to merge into
    use crate::resource_manager::lazyresourcemap::add_ztfile_from_memory;
    use crate::resource_manager::ztfile::{ZTFile, ZTFileType};
    add_ztfile_from_memory(
        "test_setup",
        "animals/testsource.ai".to_string(),
        ZTFile::RawBytes(b"[base]\nkey=value".to_vec().into_boxed_slice(), ZTFileType::Ai, 0)
    ).expect("Failed to create target file");

    // Load the test mod with unique mod_id so patches are applied
    if let Err(e) = load_test_mod("patch_source_merge_test") {
        return TestResult::fail(test_name, format!("Failed to load mod: {}", e));
    }

    // Check that the merge was applied
    use crate::resource_manager::lazyresourcemap::get_file;
    let Some((_, data)) = get_file("animals/testsource.ai") else {
        return TestResult::fail(test_name, "Target file not created".to_string());
    };

    let content = String::from_utf8_lossy(&data);
    if !content.contains("merged_from_archive") {
        return TestResult::fail(test_name, format!("Merge content not found. Got: {}", content));
    }
    if !content.contains("also_merged") {
        return TestResult::fail(test_name, "Second merge key not found".to_string());
    }

    TestResult::pass(test_name)
}

fn test_replace_from_archive() -> TestResult {
    let test_name = "test_replace_from_archive";

    // First create a target file to replace
    use crate::resource_manager::lazyresourcemap::add_ztfile_from_memory;
    use crate::resource_manager::ztfile::{ZTFile, ZTFileType};
    add_ztfile_from_memory(
        "test_setup",
        "animals/testtarget.ai".to_string(),
        ZTFile::RawBytes(b"[old]\noldkey=value".to_vec().into_boxed_slice(), ZTFileType::Ai, 0)
    ).expect("Failed to create target file");

    // Load the test mod with unique mod_id so patches are applied
    if let Err(e) = load_test_mod("patch_source_replace_test") {
        return TestResult::fail(test_name, format!("Failed to load mod: {}", e));
    }

    // Check that the replace worked
    use crate::resource_manager::lazyresourcemap::get_file;
    let Some((_, data)) = get_file("animals/testtarget.ai") else {
        return TestResult::fail(test_name, "Target file not found".to_string());
    };

    let content = String::from_utf8_lossy(&data);
    if !content.contains("[replaced]") {
        return TestResult::fail(test_name, format!("Replace content not found. Got: {}", content));
    }
    if !content.contains("key1") && !content.contains("value1") {
        return TestResult::fail(test_name, "Replace keys not found".to_string());
    }

    TestResult::pass(test_name)
}

fn test_missing_source_file_error() -> TestResult {
    let test_name = "test_missing_source_file_error";

    // First create the target file so merge can proceed to source file check
    use crate::resource_manager::lazyresourcemap::add_ztfile_from_memory;
    use crate::resource_manager::ztfile::{ZTFile, ZTFileType};
    add_ztfile_from_memory(
        "test_setup",
        "animals/target.ai".to_string(),
        ZTFile::RawBytes(b"[base]\nkey=value".to_vec().into_boxed_slice(), ZTFileType::Ai, 0)
    ).expect("Failed to create target file");

    // Create a mod with a patch that references a non-existent source file
    let mut file_map = std::collections::HashMap::new();
    file_map.insert("meta.toml".to_string(), r#"name = "Missing Source Test"
mod_id = "missing_source_test"
version = "0.1.0"
description = "Test for missing source files"
authors = ["OpenZT"]
ztd_type = "openzt"
"#.as_bytes().to_vec().into_boxed_slice());

    file_map.insert("defs/01-patches.toml".to_string(), r#"
        [patches]
        [patches.bad_patch]
        operation = "merge"
        target = "animals/target.ai"
        source = "nonexistent/file.ai"

        [patch_meta]
        on_error = "abort"
    "#.as_bytes().to_vec().into_boxed_slice());

    let result = crate::resource_manager::openzt_mods::loading::load_open_zt_mod_from_memory(
        file_map,
        "missing_source_test",
        std::path::Path::new("dummy"),
    );

    // Should fail with appropriate error message
    match result {
        Err(e) if e.to_string().contains("not found in archive") => {
            TestResult::pass(test_name)
        }
        Err(e) => {
            TestResult::fail(test_name, format!("Wrong error type: {}", e))
        }
        Ok(_) => {
            TestResult::fail(test_name, "Expected error but got success".to_string())
        }
    }
}

fn test_source_file_wrong_path_error() -> TestResult {
    let test_name = "test_source_file_wrong_path_error";

    // First create the target file so merge can proceed to source file check
    use crate::resource_manager::lazyresourcemap::add_ztfile_from_memory;
    use crate::resource_manager::ztfile::{ZTFile, ZTFileType};
    add_ztfile_from_memory(
        "test_setup",
        "animals/target.ai".to_string(),
        ZTFile::RawBytes(b"[base]\nkey=value".to_vec().into_boxed_slice(), ZTFileType::Ai, 0)
    ).expect("Failed to create target file");

    // Create a mod where source file exists but not under resources/
    let mut file_map = std::collections::HashMap::new();
    file_map.insert("meta.toml".to_string(), r#"name = "Wrong Path Test"
mod_id = "wrong_path_test"
version = "0.1.0"
description = "Test for wrong path to source files"
authors = ["OpenZT"]
ztd_type = "openzt"
"#.as_bytes().to_vec().into_boxed_slice());

    file_map.insert("defs/01-patches.toml".to_string(), r#"
        [patches]
        [patches.wrong_path]
        operation = "merge"
        target = "animals/target.ai"
        source = "patches/file.ai"  # Should be resources/patches/file.ai

        [patch_meta]
        on_error = "abort"
    "#.as_bytes().to_vec().into_boxed_slice());

    // File exists but in wrong location (no resources/ prefix)
    file_map.insert("patches/file.ai".to_string(), b"[wrong]\nkey=value".to_vec().into_boxed_slice());

    let result = crate::resource_manager::openzt_mods::loading::load_open_zt_mod_from_memory(
        file_map,
        "wrong_path_test",
        std::path::Path::new("dummy"),
    );

    // Should fail because file is not under resources/
    match result {
        Err(e) if e.to_string().contains("not found in archive") => {
            TestResult::pass(test_name)
        }
        Err(e) => {
            TestResult::fail(test_name, format!("Wrong error type: {}", e))
        }
        Ok(_) => {
            TestResult::fail(test_name, "Expected error but got success".to_string())
        }
    }
}
