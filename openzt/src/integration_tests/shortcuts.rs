//! Integration tests for the keyboard shortcut registration system.

use super::TestResult;
use crate::shortcuts::{register_shortcut, list_shortcuts, Ctrl, Alt, Shift};
use crate::shortcuts::{F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, A, B, NUM1, NUM2, NUM3, NUM4, NUM5, NUM6};

crate::integration_tests![
    test_simple_shortcut_registration,
    test_shortcut_with_modifiers,
    test_shortcut_conflict_detection,
    test_shortcut_override_behavior,
    test_multiple_shortcuts_different_keys,
    test_list_shortcuts,
    test_all_modifiers_combination,
];

/// Test simple shortcut registration (no modifiers)
fn test_simple_shortcut_registration() -> TestResult {
    let test_name = "test_simple_shortcut_registration";

    // Register a simple shortcut (using Ctrl since no-modifier shortcuts aren't ergonomic in typestate)
    let result = register_shortcut(
        "test_module",
        "Test Ctrl+F1 shortcut",
        F1 + Ctrl,
        false,
        || { /* Test callback */ }
    );

    if result.is_err() {
        return TestResult::fail(test_name, format!("Registration failed: {:?}", result));
    }

    // Verify shortcut is in list
    let list = list_shortcuts();
    if !list.contains("Test Ctrl+F1 shortcut") {
        return TestResult::fail(test_name, format!("Shortcut not found in list. List: {}", list));
    }

    if !list.contains("test_module") {
        return TestResult::fail(test_name, format!("Module name not found in list. List: {}", list));
    }

    TestResult::pass(test_name)
}

/// Test shortcut with Ctrl+Shift+Alt modifiers
fn test_shortcut_with_modifiers() -> TestResult {
    let test_name = "test_shortcut_with_modifiers";

    // Register shortcuts with different modifier combinations
    let ctrl_f2 = register_shortcut(
        "test_module",
        "Ctrl+F2 shortcut",
        F2 + Ctrl,
        false,
        || { /* Ctrl+F2 callback */ }
    );

    if ctrl_f2.is_err() {
        return TestResult::fail(test_name, format!("Ctrl+F2 registration failed: {:?}", ctrl_f2));
    }

    let ctrl_shift_f3 = register_shortcut(
        "test_module",
        "Ctrl+Shift+F3 shortcut",
        F3 + Ctrl + Shift,
        false,
        || { /* Ctrl+Shift+F3 callback */ }
    );

    if ctrl_shift_f3.is_err() {
        return TestResult::fail(test_name, format!("Ctrl+Shift+F3 registration failed: {:?}", ctrl_shift_f3));
    }

    let ctrl_shift_alt_f4 = register_shortcut(
        "test_module",
        "Ctrl+Shift+Alt+F4 shortcut",
        F4 + Ctrl + Shift + Alt,
        false,
        || { /* Ctrl+Shift+Alt+F4 callback */ }
    );

    if ctrl_shift_alt_f4.is_err() {
        return TestResult::fail(test_name, format!("Ctrl+Shift+Alt+F4 registration failed: {:?}", ctrl_shift_alt_f4));
    }

    // Verify all shortcuts are in list
    let list = list_shortcuts();
    if !list.contains("Ctrl+F2 shortcut") {
        return TestResult::fail(test_name, "Ctrl+F2 shortcut not found in list".to_string());
    }
    if !list.contains("Ctrl+Shift+F3 shortcut") {
        return TestResult::fail(test_name, "Ctrl+Shift+F3 shortcut not found in list".to_string());
    }
    if !list.contains("Ctrl+Shift+Alt+F4 shortcut") {
        return TestResult::fail(test_name, "Ctrl+Shift+Alt+F4 shortcut not found in list".to_string());
    }

    TestResult::pass(test_name)
}

/// Test that registering duplicate shortcuts fails when override=false
fn test_shortcut_conflict_detection() -> TestResult {
    let test_name = "test_shortcut_conflict_detection";

    // Register first shortcut
    let first = register_shortcut(
        "module_a",
        "First Ctrl+F5 shortcut",
        F5 + Ctrl,
        false,
        || { /* First callback */ }
    );

    if first.is_err() {
        return TestResult::fail(test_name, format!("First registration failed: {:?}", first));
    }

    // Try to register second shortcut with same key+modifiers (should fail)
    let second = register_shortcut(
        "module_b",
        "Second Ctrl+F5 shortcut",
        F5 + Ctrl,
        false,  // override = false
        || { /* Second callback */ }
    );

    if second.is_ok() {
        return TestResult::fail(test_name, "Second registration should have failed but succeeded".to_string());
    }

    // Verify error message contains expected content
    let error_msg = second.unwrap_err();
    if !error_msg.contains("already registered") && !error_msg.contains("module_a") {
        return TestResult::fail(test_name, format!("Error message unexpected: {}", error_msg));
    }

    // Verify original shortcut is still in list
    let list = list_shortcuts();
    if !list.contains("First Ctrl+F5 shortcut") {
        return TestResult::fail(test_name, "Original shortcut was replaced".to_string());
    }
    if list.contains("Second Ctrl+F5 shortcut") {
        return TestResult::fail(test_name, "Second shortcut should not be in list".to_string());
    }

    TestResult::pass(test_name)
}

/// Test that override=true allows replacing existing shortcuts
fn test_shortcut_override_behavior() -> TestResult {
    let test_name = "test_shortcut_override_behavior";

    // Register first shortcut (Shift+Alt since Shift alone isn't allowed)
    let first = register_shortcut(
        "module_a",
        "Original Shift+Alt+F6 shortcut",
        F6 + Shift + Alt,
        false,
        || { /* Original callback */ }
    );

    if first.is_err() {
        return TestResult::fail(test_name, format!("First registration failed: {:?}", first));
    }

    // Register second shortcut with same key+modifiers but override=true
    let second = register_shortcut(
        "module_b",
        "Replacement Shift+Alt+F6 shortcut",
        F6 + Shift + Alt,
        true,  // override = true
        || { /* Replacement callback */ }
    );

    if second.is_err() {
        return TestResult::fail(test_name, format!("Override registration failed: {:?}", second));
    }

    // Verify replacement shortcut is in list
    let list = list_shortcuts();
    if !list.contains("Replacement Shift+Alt+F6 shortcut") {
        return TestResult::fail(test_name, "Replacement shortcut not found in list".to_string());
    }
    if list.contains("Original Shift+Alt+F6 shortcut") {
        return TestResult::fail(test_name, "Original shortcut should be replaced".to_string());
    }
    if !list.contains("module_b") {
        return TestResult::fail(test_name, "Module_b not found in list".to_string());
    }

    TestResult::pass(test_name)
}

/// Test that multiple shortcuts with different keys can coexist
fn test_multiple_shortcuts_different_keys() -> TestResult {
    let test_name = "test_multiple_shortcuts_different_keys";

    // Register multiple shortcuts with different keys using typestate pattern
    // Note: We register each individually to avoid type inference issues

    let result1 = register_shortcut("multi_test", "F7+Ctrl action", F7 + Ctrl, false, || {});
    if result1.is_err() {
        return TestResult::fail(test_name, format!("Failed to register F7+Ctrl: {:?}", result1));
    }

    let result2 = register_shortcut("multi_test", "F8+Ctrl action", F8 + Ctrl, false, || {});
    if result2.is_err() {
        return TestResult::fail(test_name, format!("Failed to register F8+Ctrl: {:?}", result2));
    }

    let result3 = register_shortcut("multi_test", "F9+Shift+Ctrl action", F9 + Shift + Ctrl, false, || {});
    if result3.is_err() {
        return TestResult::fail(test_name, format!("Failed to register F9+Shift+Ctrl: {:?}", result3));
    }

    let result4 = register_shortcut("multi_test", "F10+Shift+Ctrl action", F10 + Shift + Ctrl, false, || {});
    if result4.is_err() {
        return TestResult::fail(test_name, format!("Failed to register F10+Shift+Ctrl: {:?}", result4));
    }

    let result5 = register_shortcut("multi_test", "F11+Alt action", F11 + Alt, false, || {});
    if result5.is_err() {
        return TestResult::fail(test_name, format!("Failed to register F11+Alt: {:?}", result5));
    }

    let result6 = register_shortcut("multi_test", "Num1+All action", NUM1 + Ctrl + Shift + Alt, false, || {});
    if result6.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Num1+All: {:?}", result6));
    }

    let result7 = register_shortcut("multi_test", "A+Ctrl action", A + Ctrl, false, || {});
    if result7.is_err() {
        return TestResult::fail(test_name, format!("Failed to register A+Ctrl: {:?}", result7));
    }

    // Verify all shortcuts are in list
    let list = list_shortcuts();
    let descriptions = ["F7+Ctrl action", "F8+Ctrl action", "F9+Shift+Ctrl action",
                       "F10+Shift+Ctrl action", "F11+Alt action", "Num1+All action", "A+Ctrl action"];
    for desc in &descriptions {
        if !list.contains(desc) {
            return TestResult::fail(test_name, format!("Shortcut '{}' not found in list", desc));
        }
    }

    TestResult::pass(test_name)
}

/// Test the list_shortcuts function format and content
fn test_list_shortcuts() -> TestResult {
    let test_name = "test_list_shortcuts";

    // Register a few test shortcuts
    // Note: Use override=true since keys might be registered by previous tests
    let result1 = register_shortcut(
        "list_test_module",
        "Test list shortcut A+Ctrl",
        A + Ctrl,
        true,  // override in case it was registered before
        || { /* Callback A */ }
    );

    if result1.is_err() {
        return TestResult::fail(test_name, format!("Failed to register A+Ctrl: {:?}", result1));
    }

    let result2 = register_shortcut(
        "list_test_module",
        "Test list shortcut B+Ctrl",
        B + Ctrl,
        true,  // override in case it was registered before
        || { /* Callback B */ }
    );

    if result2.is_err() {
        return TestResult::fail(test_name, format!("Failed to register B+Ctrl: {:?}", result2));
    }

    let list = list_shortcuts();

    // Check for expected content
    if !list.contains("Registered shortcuts:") && !list.contains("No shortcuts registered") {
        return TestResult::fail(test_name, format!("List format unexpected: {}", list));
    }

    if !list.contains("Test list shortcut A+Ctrl") {
        return TestResult::fail(test_name, format!("Shortcut A not found. List: {}", list));
    }

    if !list.contains("Test list shortcut B+Ctrl") {
        return TestResult::fail(test_name, format!("Shortcut B not found. List: {}", list));
    }

    if !list.contains("list_test_module") {
        return TestResult::fail(test_name, format!("Module name not found. List: {}", list));
    }

    TestResult::pass(test_name)
}

/// Test all possible modifier combinations (that are valid with typestate pattern)
fn test_all_modifiers_combination() -> TestResult {
    let test_name = "test_all_modifiers_combination";

    // Test all valid modifier combinations using typestate pattern
    // Note: Shift-only is not allowed, so we skip that case
    // We register each individually to avoid type inference issues
    // Use override=true in case keys were registered by previous tests

    let result1 = register_shortcut("modifier_test", "Test with Ctrl modifiers", NUM1 + Ctrl, true, || {});
    if result1.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Ctrl: {:?}", result1));
    }

    let result2 = register_shortcut("modifier_test", "Test with Shift+Ctrl modifiers", NUM2 + Shift + Ctrl, true, || {});
    if result2.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Shift+Ctrl: {:?}", result2));
    }

    let result3 = register_shortcut("modifier_test", "Test with Alt modifiers", NUM3 + Alt, true, || {});
    if result3.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Alt: {:?}", result3));
    }

    let result4 = register_shortcut("modifier_test", "Test with Ctrl+Alt modifiers", NUM4 + Ctrl + Alt, true, || {});
    if result4.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Ctrl+Alt: {:?}", result4));
    }

    let result5 = register_shortcut("modifier_test", "Test with Shift+Alt modifiers", NUM5 + Shift + Alt, true, || {});
    if result5.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Shift+Alt: {:?}", result5));
    }

    let result6 = register_shortcut("modifier_test", "Test with Ctrl+Shift+Alt modifiers", NUM6 + Ctrl + Shift + Alt, true, || {});
    if result6.is_err() {
        return TestResult::fail(test_name, format!("Failed to register Ctrl+Shift+Alt: {:?}", result6));
    }

    // Verify all are in list
    let list = list_shortcuts();
    if !list.contains("Test with Ctrl modifiers") {
        return TestResult::fail(test_name, "Ctrl modifier shortcut not found".to_string());
    }
    if !list.contains("Test with Ctrl+Shift+Alt modifiers") {
        return TestResult::fail(test_name, "All-modifier shortcut not found".to_string());
    }

    TestResult::pass(test_name)
}
