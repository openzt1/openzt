//! Integration tests for permitted archive pattern filtering
//!
//! Tests the regex-based pattern system that controls which files
//! can be loaded from specific archives.

use crate::integration_tests::TestResult;

#[cfg(feature = "integration-tests")]
use crate::resource_manager::legacy_loading::{
    is_archive_permitted_for_file, PERMITTED_ARCHIVE_PATTERNS,
};

/// Test that regex patterns correctly match filenames
#[cfg(feature = "integration-tests")]
pub fn test_regex_pattern_matching() -> TestResult {
    let test_name = "test_regex_pattern_matching";

    // Verify the xpac pattern exists and matches expected files
    let has_xpac_pattern = PERMITTED_ARCHIVE_PATTERNS.iter().any(|(pattern, _): &(_, _)| {
        pattern.is_match("xpac03.cfg")
    });

    if !has_xpac_pattern {
        return TestResult::fail(test_name, "xpac pattern not found in PERMITTED_ARCHIVE_PATTERNS".to_string());
    }

    // Verify the permitted list contains expected archives
    let permitted_list: Vec<&str> = PERMITTED_ARCHIVE_PATTERNS.iter()
        .filter(|(pattern, _)| pattern.is_match("xpac03.cfg"))
        .flat_map(|(_, permitted)| permitted.iter().copied())
        .collect();

    let expected_archives = vec!["zupdate/config2.ztd", "xpack1/config2.ztd", "xpack2/config3.ztd"];
    for expected in expected_archives {
        if !permitted_list.contains(&expected) {
            return TestResult::fail(
                test_name,
                format!("Expected archive '{}' not in permitted list. Got: {:?}", expected, permitted_list)
            );
        }
    }

    TestResult::pass(test_name)
}

/// Test that permitted archive checking works correctly
#[cfg(feature = "integration-tests")]
pub fn test_permitted_archive_checking() -> TestResult {
    let test_name = "test_permitted_archive_checking";

    // First, do a direct test of the basic case
    let basic_result = is_archive_permitted_for_file("zupdate/config2.ztd", "xpac03.cfg");
    if !basic_result {
        return TestResult::fail(
            test_name,
            format!("Basic test failed: is_archive_permitted_for_file(\"zupdate/config2.ztd\", \"xpac03.cfg\") returned false, expected true")
        );
    }

    // Test xpac files from permitted archives
    let permitted_archives = vec![
        ("zupdate/config2.ztd", "xpac03.cfg", true),
        ("zupdate/config2.ztd", "xpac99.cfg", true),
        ("xpack1/config2.ztd", "xpac03.cfg", true),
        ("xpack2/config3.ztd", "xpac03.cfg", true),
        ("ZUPDATE/CONFIG2.ZTD", "xpac03.cfg", true),  // Case insensitive
        ("XPACK1/CONFIG2.ZTD", "xpac03.cfg", true),  // Case insensitive
    ];

    for (archive, file, expected) in permitted_archives {
        let result = is_archive_permitted_for_file(archive, file);
        if result != expected {
            return TestResult::fail(
                test_name,
                format!("Expected is_archive_permitted_for_file({}, {}) = {}, got {}", archive, file, expected, result)
            );
        }
    }

    // Test xpac files from non-permitted archives
    let non_permitted_archives = vec![
        ("other.ztd", "xpac03.cfg", false),
        ("custom.ztd", "xpac99.cfg", false),
        ("xpack1/config3.ztd", "xpac03.cfg", false),  // Different config file
        ("random.ztd", "xpacwhatever.cfg", false),
        ("config.ztd", "xpac03.cfg", false),  // Wrong path for base config
    ];

    for (archive, file, expected) in non_permitted_archives {
        let result = is_archive_permitted_for_file(archive, file);
        if result != expected {
            return TestResult::fail(
                test_name,
                format!("Expected is_archive_permitted_for_file({}, {}) = {}, got {}", archive, file, expected, result)
            );
        }
    }

    // Test non-xpac files (should be unrestricted)
    let unrestricted_files = vec![
        ("zupdate/config2.ztd", "animal.cfg", true),
        ("other.ztd", "ui/main.lyt", true),
        ("custom.ztd", "buildings/bldg.cfg", true),
    ];

    for (archive, file, expected) in unrestricted_files {
        let result = is_archive_permitted_for_file(archive, file);
        if result != expected {
            return TestResult::fail(
                test_name,
                format!("Expected is_archive_permitted_for_file({}, {}) = {}, got {}", archive, file, expected, result)
            );
        }
    }

    TestResult::pass(test_name)
}

/// Test that xpac files can load from permitted archives
#[cfg(feature = "integration-tests")]
pub fn test_xpac_from_permitted_archive() -> TestResult {
    let test_name = "test_xpac_from_permitted_archive";

    // This test verifies that the pattern matching logic is correctly set up
    // The actual loading test would require creating test ZTD files
    // Here we verify the logic path is correct

    let permitted_cases = vec![
        ("zupdate/config2.ztd", "xpac03.cfg"),
        ("xpack1/config2.ztd", "xpac03.cfg"),
        ("xpack2/config3.ztd", "xpac99.cfg"),
    ];

    for (archive, file) in permitted_cases {
        if !is_archive_permitted_for_file(archive, file) {
            return TestResult::fail(
                test_name,
                format!("Permitted archive/file pair ({}, {}) was rejected", archive, file)
            );
        }
    }

    TestResult::pass(test_name)
}

/// Test that xpac files are blocked from non-permitted archives
#[cfg(feature = "integration-tests")]
pub fn test_xpac_from_non_permitted_archive() -> TestResult {
    let test_name = "test_xpac_from_non_permitted_archive";

    let blocked_cases = vec![
        ("custom.ztd", "xpac03.cfg"),
        ("extra.ztd", "xpac99.cfg"),
        ("randomarchive.ztd", "xpac.cfg"),
        ("config.ztd", "xpac03.cfg"),  // Wrong path (should be zupdate/config2.ztd)
    ];

    for (archive, file) in blocked_cases {
        let result = is_archive_permitted_for_file(archive, file);
        if result {
            return TestResult::fail(
                test_name,
                format!("Non-permitted archive/file pair ({}, {}) was allowed", archive, file)
            );
        }
    }

    TestResult::pass(test_name)
}

/// Test that non-cfg files are not affected by xpac pattern
#[cfg(feature = "integration-tests")]
pub fn test_non_cfg_files_skipped() -> TestResult {
    let test_name = "test_non_cfg_files_skipped";

    // Non-cfg files should be unrestricted even from non-permitted archives
    let test_cases = vec![
        ("custom.ztd", "xpac03.uca"),
        ("custom.ztd", "xpac99.ucb"),
        ("custom.ztd", "xpac03.ucs"),
        ("custom.ztd", "xpac.ai"),
        ("custom.ztd", "xpac.bmp"),
    ];

    for (archive, file) in test_cases {
        if !is_archive_permitted_for_file(archive, file) {
            return TestResult::fail(
                test_name,
                format!("Non-cfg file {} from {} was incorrectly restricted", file, archive)
            );
        }
    }

    TestResult::pass(test_name)
}

/// Test that path normalization works correctly (leading './' and backslash handling)
#[cfg(feature = "integration-tests")]
pub fn test_path_normalization() -> TestResult {
    let test_name = "test_path_normalization";

    // Test with leading './' and backslashes - should be normalized correctly
    let normalization_cases = vec![
        ("./zupdate/config2.ztd", "xpac03.cfg", true),          // Leading ./ stripped
        (".\\zupdate/config2.ztd", "xpac03.cfg", true),         // Leading .\ stripped and \ converted to /
        ("./xpack1\\config2.ztd", "xpac03.cfg", true),          // Mixed path separators
        ("xpack1\\config2.ztd", "xpac03.cfg", true),            // Backslashes converted
        ("./xpack2\\CONFIG3.ZTD", "xpac03.cfg", true),          // Case insensitive with normalization
        ("OTHER.ZTD", "xpac03.cfg", false),                     // Non-permitted still blocked
        (".\\other.ztd", "xpac03.cfg", false),                  // Non-permitted with normalization still blocked
        ("./config.ztd", "xpac03.cfg", false),                  // Wrong path (should be zupdate/config2.ztd)
    ];

    for (archive, file, expected) in normalization_cases {
        let result = is_archive_permitted_for_file(archive, file);
        if result != expected {
            return TestResult::fail(
                test_name,
                format!("Expected is_archive_permitted_for_file({}, {}) = {}, got {}", archive, file, expected, result)
            );
        }
    }

    TestResult::pass(test_name)
}

crate::integration_tests![
    test_regex_pattern_matching,
    test_permitted_archive_checking,
    test_xpac_from_permitted_archive,
    test_xpac_from_non_permitted_archive,
    test_non_cfg_files_skipped,
    test_path_normalization,
];
