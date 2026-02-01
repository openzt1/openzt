use super::TestResult;
use crate::resource_manager::{
    dependency_resolver::{DependencyResolver, ResolutionWarning},
    validation::validate_load_order,
};
use std::collections::HashMap;
use std::path::PathBuf;

// ============================================================================
// Embedded Test Resources for Dependency Tests
// ============================================================================

#[cfg(feature = "integration-tests")]
mod embedded_resources {
    // Test Mod A - No dependencies (base mod)
    pub const META_MOD_A: &str = r#"
name = "Test Mod A"
description = "Base test mod with no dependencies"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_a"
version = "1.0.0"
ztd_type = "openzt"
"#;

    // Test Mod B - Depends on A (after)
    pub const META_MOD_B: &str = r#"
name = "Test Mod B"
description = "Test mod that depends on mod A"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_b"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_a", name = "Test Mod A", ordering = "after" }
]
"#;

    // Test Mod C - Depends on B (after), creating chain A -> B -> C
    pub const META_MOD_C: &str = r#"
name = "Test Mod C"
description = "Test mod that depends on mod B"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_c"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_b", name = "Test Mod B", ordering = "after" }
]
"#;

    // Test Mod D - Circular dependency with E
    pub const META_MOD_D: &str = r#"
name = "Test Mod D"
description = "Test mod with circular dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_d"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_e", name = "Test Mod E", ordering = "after" }
]
"#;

    // Test Mod E - Circular dependency with D
    pub const META_MOD_E: &str = r#"
name = "Test Mod E"
description = "Test mod with circular dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_e"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_d", name = "Test Mod D", ordering = "after" }
]
"#;

    // Test Mod F - Optional dependency
    pub const META_MOD_F: &str = r#"
name = "Test Mod F"
description = "Test mod with optional dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_f"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.nonexistent", name = "Nonexistent Mod", optional = true, ordering = "after" }
]
"#;

    // Test Mod G - Before dependency
    pub const META_MOD_G: &str = r#"
name = "Test Mod G"
description = "Test mod that must load before H"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_g"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_h", name = "Test Mod H", ordering = "before" }
]
"#;

    // Test Mod H - No dependencies but G loads before it
    pub const META_MOD_H: &str = r#"
name = "Test Mod H"
description = "Test mod with no dependencies"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_h"
version = "1.0.0"
ztd_type = "openzt"
"#;

    // Two-Stage Cycle Detection Test Mods

    // Mod I - Optional cycle with J (I -(optional)→ J)
    pub const META_MOD_I: &str = r#"
name = "Test Mod I"
description = "Optional cycle test mod I"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_i"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_j", name = "Test Mod J", optional = true, ordering = "after" }
]
"#;

    // Mod J - Optional cycle with I (J -(optional)→ I)
    pub const META_MOD_J: &str = r#"
name = "Test Mod J"
description = "Optional cycle test mod J"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_j"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_i", name = "Test Mod I", optional = true, ordering = "after" }
]
"#;

    // Mod K - Mixed cycle: K -(required)→ L
    pub const META_MOD_K: &str = r#"
name = "Test Mod K"
description = "Mixed cycle test mod K"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_k"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_l", name = "Test Mod L", ordering = "after" }
]
"#;

    // Mod L - Mixed cycle: L -(optional)→ M
    pub const META_MOD_L: &str = r#"
name = "Test Mod L"
description = "Mixed cycle test mod L"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_l"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_m", name = "Test Mod M", optional = true, ordering = "after" }
]
"#;

    // Mod M - Mixed cycle: M -(required)→ K (completes cycle)
    pub const META_MOD_M: &str = r#"
name = "Test Mod M"
description = "Mixed cycle test mod M"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_m"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_k", name = "Test Mod K", ordering = "after" }
]
"#;

    // Mod N - Triangle with optional: N -(required)→ O
    pub const META_MOD_N: &str = r#"
name = "Test Mod N"
description = "Triangle test mod N"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_n"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_o", name = "Test Mod O", ordering = "after" }
]
"#;

    // Mod O - Triangle with optional: O -(required)→ P
    pub const META_MOD_O: &str = r#"
name = "Test Mod O"
description = "Triangle test mod O"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_o"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_p", name = "Test Mod P", ordering = "after" }
]
"#;

    // Mod P - Triangle with optional: P -(optional)→ N (completes optional cycle)
    pub const META_MOD_P: &str = r#"
name = "Test Mod P"
description = "Triangle test mod P"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_p"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_n", name = "Test Mod N", optional = true, ordering = "after" }
]
"#;

    // Mod Q - Chain with optional: Q -(required)→ R
    pub const META_MOD_Q: &str = r#"
name = "Test Mod Q"
description = "Chain test mod Q"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_q"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_r", name = "Test Mod R", ordering = "after" }
]
"#;

    // Mod R - Chain with optional: R -(required)→ S
    pub const META_MOD_R: &str = r#"
name = "Test Mod R"
description = "Chain test mod R"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_r"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_s", name = "Test Mod S", ordering = "after" }
]
"#;

    // Mod S - Chain with optional: S -(required)→ T
    pub const META_MOD_S: &str = r#"
name = "Test Mod S"
description = "Chain test mod S"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_s"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_t", name = "Test Mod T", ordering = "after" }
]
"#;

    // Mod T - Chain with optional: T -(optional)→ Q (completes optional cycle)
    pub const META_MOD_T: &str = r#"
name = "Test Mod T"
description = "Chain test mod T"
authors = ["OpenZT Test Suite"]
mod_id = "test.dependency.mod_t"
version = "1.0.0"
ztd_type = "openzt"

dependencies = [
    { mod_id = "test.dependency.mod_q", name = "Test Mod Q", optional = true, ordering = "after" }
]
"#;
}

/// Create test mod metadata map for dependency tests
#[cfg(feature = "integration-tests")]
fn create_test_mods() -> HashMap<String, &'static str> {
    use embedded_resources::*;
    let mut mods = HashMap::new();

    mods.insert("test.dependency.mod_a".to_string(), META_MOD_A);
    mods.insert("test.dependency.mod_b".to_string(), META_MOD_B);
    mods.insert("test.dependency.mod_c".to_string(), META_MOD_C);
    mods.insert("test.dependency.mod_d".to_string(), META_MOD_D);
    mods.insert("test.dependency.mod_e".to_string(), META_MOD_E);
    mods.insert("test.dependency.mod_f".to_string(), META_MOD_F);
    mods.insert("test.dependency.mod_g".to_string(), META_MOD_G);
    mods.insert("test.dependency.mod_h".to_string(), META_MOD_H);
    mods.insert("test.dependency.mod_i".to_string(), META_MOD_I);
    mods.insert("test.dependency.mod_j".to_string(), META_MOD_J);
    mods.insert("test.dependency.mod_k".to_string(), META_MOD_K);
    mods.insert("test.dependency.mod_l".to_string(), META_MOD_L);
    mods.insert("test.dependency.mod_m".to_string(), META_MOD_M);
    mods.insert("test.dependency.mod_n".to_string(), META_MOD_N);
    mods.insert("test.dependency.mod_o".to_string(), META_MOD_O);
    mods.insert("test.dependency.mod_p".to_string(), META_MOD_P);
    mods.insert("test.dependency.mod_q".to_string(), META_MOD_Q);
    mods.insert("test.dependency.mod_r".to_string(), META_MOD_R);
    mods.insert("test.dependency.mod_s".to_string(), META_MOD_S);
    mods.insert("test.dependency.mod_t".to_string(), META_MOD_T);

    mods
}

/// Parse test mods into Meta objects
#[cfg(feature = "integration-tests")]
fn parse_test_mods() -> HashMap<String, crate::mods::Meta> {
    let test_mods = create_test_mods();
    let mut parsed = HashMap::new();

    for (mod_id, toml_str) in test_mods {
        match toml::from_str(toml_str) {
            Ok(meta) => {
                parsed.insert(mod_id, meta);
            }
            Err(e) => {
                panic!("Failed to parse test mod {}: {}", mod_id, e);
            }
        }
    }

    parsed
}

// ============================================================================
// Tests
// ============================================================================

crate::integration_tests![
    test_simple_dependency_chain,
    test_circular_dependency_handling,
    test_optional_dependency_warning,
    test_before_dependency,
    test_new_mod_insertion,
    test_disabled_mods_excluded,
    test_validation_detects_violations,
    // Two-stage cycle detection tests
    test_optional_dependency_cycle,
    test_mixed_cycle_resolution,
    test_triangle_with_optional,
    test_large_cycle_one_optional,
    // New identifier type tests
    test_ztd_name_dependency,
    test_dll_name_dependency,
    test_mixed_identifier_types,
    test_unresolved_ztd_name_warning,
    test_backwards_compatibility_mod_id,
];

/// Test A -> B -> C dependency chain resolves correctly
fn test_simple_dependency_chain() -> TestResult {
    let test_name = "test_simple_dependency_chain";

    let all_mods = parse_test_mods();

    // Only use mods A, B, C for this test
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_a".to_string(), all_mods["test.dependency.mod_a"].clone());
    mods.insert("test.dependency.mod_b".to_string(), all_mods["test.dependency.mod_b"].clone());
    mods.insert("test.dependency.mod_c".to_string(), all_mods["test.dependency.mod_c"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Expected order: A, B, C
    let expected = vec![
        "test.dependency.mod_a".to_string(),
        "test.dependency.mod_b".to_string(),
        "test.dependency.mod_c".to_string(),
    ];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    if !result.warnings.is_empty() {
        return TestResult::fail(test_name, format!("Expected no warnings, got: {:?}", result.warnings));
    }

    TestResult::pass(test_name)
}

/// Test circular dependency D <-> E is detected and handled
fn test_circular_dependency_handling() -> TestResult {
    let test_name = "test_circular_dependency_handling";

    let all_mods = parse_test_mods();

    // Only use mods D and E (circular dependency)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_d".to_string(), all_mods["test.dependency.mod_d"].clone());
    mods.insert("test.dependency.mod_e".to_string(), all_mods["test.dependency.mod_e"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have both mods in the order
    if result.order.len() != 2 {
        return TestResult::fail(test_name, format!("Expected 2 mods in order, got {}", result.order.len()));
    }

    // Should have circular dependency warning
    let has_cycle_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::CircularDependency { .. }));

    if !has_cycle_warning {
        return TestResult::fail(test_name, "Expected CircularDependency warning".to_string());
    }

    TestResult::pass(test_name)
}

/// Test optional dependency generates warning but doesn't fail
fn test_optional_dependency_warning() -> TestResult {
    let test_name = "test_optional_dependency_warning";

    let all_mods = parse_test_mods();

    // Only use mod F (has optional dependency on nonexistent mod)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_f".to_string(), all_mods["test.dependency.mod_f"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have mod F in the order
    if result.order != vec!["test.dependency.mod_f"] {
        return TestResult::fail(test_name, format!("Expected only mod_f in order, got {:?}", result.order));
    }

    // Should have missing optional dependency warning
    let has_optional_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::MissingOptionalDependency { .. }));

    if !has_optional_warning {
        return TestResult::fail(test_name, "Expected MissingOptionalDependency warning".to_string());
    }

    TestResult::pass(test_name)
}

/// Test "before" dependency works correctly (G before H)
fn test_before_dependency() -> TestResult {
    let test_name = "test_before_dependency";

    let all_mods = parse_test_mods();

    // Use mods G and H
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_g".to_string(), all_mods["test.dependency.mod_g"].clone());
    mods.insert("test.dependency.mod_h".to_string(), all_mods["test.dependency.mod_h"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Expected order: G, H (G must be before H)
    let expected = vec!["test.dependency.mod_g".to_string(), "test.dependency.mod_h".to_string()];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test inserting a new mod into existing order
fn test_new_mod_insertion() -> TestResult {
    let test_name = "test_new_mod_insertion";

    let all_mods = parse_test_mods();

    // Start with A and C in existing order
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_a".to_string(), all_mods["test.dependency.mod_a"].clone());
    mods.insert("test.dependency.mod_b".to_string(), all_mods["test.dependency.mod_b"].clone());
    mods.insert("test.dependency.mod_c".to_string(), all_mods["test.dependency.mod_c"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);

    // Existing order has A and C, missing B
    let existing = vec!["test.dependency.mod_a".to_string(), "test.dependency.mod_c".to_string()];

    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&existing, &[], pure_legacy);

    // B should be inserted between A and C
    let expected = vec![
        "test.dependency.mod_a".to_string(),
        "test.dependency.mod_b".to_string(),
        "test.dependency.mod_c".to_string(),
    ];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected B to be inserted between A and C, got {:?}", result.order));
    }

    TestResult::pass(test_name)
}

/// Test disabled mods stay in order but new disabled mods are not added
fn test_disabled_mods_excluded() -> TestResult {
    let test_name = "test_disabled_mods_excluded";

    let all_mods = parse_test_mods();

    // Use mods A, B, C
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_a".to_string(), all_mods["test.dependency.mod_a"].clone());
    mods.insert("test.dependency.mod_b".to_string(), all_mods["test.dependency.mod_b"].clone());
    mods.insert("test.dependency.mod_c".to_string(), all_mods["test.dependency.mod_c"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);

    // B is in existing order but disabled - should stay in order
    // C is new and disabled - should NOT be added
    let existing = vec!["test.dependency.mod_a".to_string(), "test.dependency.mod_b".to_string()];
    let disabled = vec!["test.dependency.mod_b".to_string(), "test.dependency.mod_c".to_string()];
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&existing, &disabled, pure_legacy);

    // Should have A and B (B is disabled but stays in order)
    // C is new and disabled, so should not be added
    if !result.order.contains(&"test.dependency.mod_b".to_string()) {
        return TestResult::fail(test_name, "Disabled mod_b should stay in order (already exists)".to_string());
    }

    if result.order.contains(&"test.dependency.mod_c".to_string()) {
        return TestResult::fail(test_name, "New disabled mod_c should not be added to order".to_string());
    }

    if !result.order.contains(&"test.dependency.mod_a".to_string()) {
        return TestResult::fail(test_name, "Enabled mod_a should be in order".to_string());
    }

    // Expected order: A, B (C not added because it's new and disabled)
    let expected = vec!["test.dependency.mod_a".to_string(), "test.dependency.mod_b".to_string()];
    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test validation detects ordering violations
fn test_validation_detects_violations() -> TestResult {
    let test_name = "test_validation_detects_violations";

    let all_mods = parse_test_mods();

    // Use mods A, B (B depends on A)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_a".to_string(), all_mods["test.dependency.mod_a"].clone());
    mods.insert("test.dependency.mod_b".to_string(), all_mods["test.dependency.mod_b"].clone());

    // Create WRONG order: B before A (violates dependency)
    let wrong_order = vec!["test.dependency.mod_b".to_string(), "test.dependency.mod_a".to_string()];

    let validation = validate_load_order(&wrong_order, &mods, &[]);

    // Should have warnings about ordering violation
    if validation.warnings.is_empty() {
        return TestResult::fail(test_name, "Expected warnings for ordering violation".to_string());
    }

    TestResult::pass(test_name)
}

// ============================================================================
// Two-Stage Cycle Detection Tests
// ============================================================================

/// Test I <-(optional)-> J: Both mods have optional deps on each other
/// Expected: Stage 1 cycle detected, Stage 2 no cycle (formerly cyclic)
fn test_optional_dependency_cycle() -> TestResult {
    let test_name = "test_optional_dependency_cycle";

    let all_mods = parse_test_mods();

    // Use mods I and J (optional cycle)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_i".to_string(), all_mods["test.dependency.mod_i"].clone());
    mods.insert("test.dependency.mod_j".to_string(), all_mods["test.dependency.mod_j"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have both mods in alphabetical order
    if result.order.len() != 2 {
        return TestResult::fail(test_name, format!("Expected 2 mods in order, got {}", result.order.len()));
    }

    // Both should be formerly cyclic (not truly cyclic)
    // Should have Stage 1 CircularDependency warning but NO TrulyCyclicDependency warning
    let has_stage1_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::CircularDependency { .. }));
    let has_stage2_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::TrulyCyclicDependency { .. }));
    let has_formerly_cyclic = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::FormerlyCyclicDependency { .. }));

    if !has_stage1_warning {
        return TestResult::fail(test_name, "Expected CircularDependency warning from Stage 1".to_string());
    }

    if has_stage2_warning {
        return TestResult::fail(test_name, "Should NOT have TrulyCyclicDependency warning (optional deps only)".to_string());
    }

    if !has_formerly_cyclic {
        return TestResult::fail(test_name, "Expected FormerlyCyclicDependency warnings".to_string());
    }

    // Mods should NOT be at the end (they're formerly cyclic, inserted normally)
    // They should be at the beginning in alphabetical order
    let expected = vec!["test.dependency.mod_i".to_string(), "test.dependency.mod_j".to_string()];

    if result.order != expected {
        return TestResult::fail(
            test_name,
            format!("Expected order {:?} (formerly cyclic, alphabetical), got {:?}", expected, result.order),
        );
    }

    TestResult::pass(test_name)
}

/// Test K -> L -(optional)-> M -> K: Mixed cycle with one optional edge
/// Expected: All formerly cyclic, proper topological order without the optional edge
fn test_mixed_cycle_resolution() -> TestResult {
    let test_name = "test_mixed_cycle_resolution";

    let all_mods = parse_test_mods();

    // Use mods K, L, M (mixed cycle: K -(required)-> L -(optional)-> M -(required)-> K)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_k".to_string(), all_mods["test.dependency.mod_k"].clone());
    mods.insert("test.dependency.mod_l".to_string(), all_mods["test.dependency.mod_l"].clone());
    mods.insert("test.dependency.mod_m".to_string(), all_mods["test.dependency.mod_m"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have all 3 mods
    if result.order.len() != 3 {
        return TestResult::fail(test_name, format!("Expected 3 mods in order, got {}", result.order.len()));
    }

    // Should have Stage 1 warning but NO Stage 2 warning
    let has_stage1_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::CircularDependency { .. }));
    let has_stage2_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::TrulyCyclicDependency { .. }));

    if !has_stage1_warning {
        return TestResult::fail(test_name, "Expected CircularDependency warning from Stage 1".to_string());
    }

    if has_stage2_warning {
        return TestResult::fail(test_name, "Should NOT have TrulyCyclicDependency warning (cycle broken by optional)".to_string());
    }

    // Without the optional edge L->M, the required-only graph is:
    // K -> L (K depends on L)
    // M -> K (M depends on K)
    // No edge from L to M
    // So: K requires L to be before it, M requires K to be before it
    // Valid order: L, K, M
    let expected = vec![
        "test.dependency.mod_l".to_string(),
        "test.dependency.mod_k".to_string(),
        "test.dependency.mod_m".to_string(),
    ];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test N -> O -> P -(optional)-> N: Triangle with optional back-edge
/// Expected: All formerly cyclic, order N, O, P
fn test_triangle_with_optional() -> TestResult {
    let test_name = "test_triangle_with_optional";

    let all_mods = parse_test_mods();

    // Use mods N, O, P (triangle: N -(required)-> O -(required)-> P -(optional)-> N)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_n".to_string(), all_mods["test.dependency.mod_n"].clone());
    mods.insert("test.dependency.mod_o".to_string(), all_mods["test.dependency.mod_o"].clone());
    mods.insert("test.dependency.mod_p".to_string(), all_mods["test.dependency.mod_p"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have all 3 mods
    if result.order.len() != 3 {
        return TestResult::fail(test_name, format!("Expected 3 mods in order, got {}", result.order.len()));
    }

    // Should have Stage 1 warning but NO Stage 2 warning
    let has_stage1_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::CircularDependency { .. }));
    let has_stage2_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::TrulyCyclicDependency { .. }));

    if !has_stage1_warning {
        return TestResult::fail(test_name, "Expected CircularDependency warning from Stage 1".to_string());
    }

    if has_stage2_warning {
        return TestResult::fail(test_name, "Should NOT have TrulyCyclicDependency warning (cycle broken by optional)".to_string());
    }

    // Without the optional edge P->N, the order should be: P, O, N
    // (P has no deps, O depends on P, N depends on O)
    let expected = vec![
        "test.dependency.mod_p".to_string(),
        "test.dependency.mod_o".to_string(),
        "test.dependency.mod_n".to_string(),
    ];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test Q -> R -> S -> T -(optional)-> Q: Long chain with one optional back-edge
/// Expected: All formerly cyclic, order Q, R, S, T
fn test_large_cycle_one_optional() -> TestResult {
    let test_name = "test_large_cycle_one_optional";

    let all_mods = parse_test_mods();

    // Use mods Q, R, S, T (chain: Q -> R -> S -> T -(optional)-> Q)
    let mut mods = HashMap::new();
    mods.insert("test.dependency.mod_q".to_string(), all_mods["test.dependency.mod_q"].clone());
    mods.insert("test.dependency.mod_r".to_string(), all_mods["test.dependency.mod_r"].clone());
    mods.insert("test.dependency.mod_s".to_string(), all_mods["test.dependency.mod_s"].clone());
    mods.insert("test.dependency.mod_t".to_string(), all_mods["test.dependency.mod_t"].clone());

    // Create discovered map for the resolver
    let discovered: HashMap<String, (String, crate::mods::Meta)> = mods.iter().map(|(id, meta)| (id.clone(), (format!("{}.ztd", id), meta.clone()))).collect();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    // Should have all 4 mods
    if result.order.len() != 4 {
        return TestResult::fail(test_name, format!("Expected 4 mods in order, got {}", result.order.len()));
    }

    // Should have Stage 1 warning but NO Stage 2 warning
    let has_stage1_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::CircularDependency { .. }));
    let has_stage2_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::TrulyCyclicDependency { .. }));

    if !has_stage1_warning {
        return TestResult::fail(test_name, "Expected CircularDependency warning from Stage 1".to_string());
    }

    if has_stage2_warning {
        return TestResult::fail(test_name, "Should NOT have TrulyCyclicDependency warning (cycle broken by optional)".to_string());
    }

    // Without the optional edge T->Q, the order should be: T, S, R, Q
    // (T has no deps, S depends on T, R depends on S, Q depends on R)
    let expected = vec![
        "test.dependency.mod_t".to_string(),
        "test.dependency.mod_s".to_string(),
        "test.dependency.mod_r".to_string(),
        "test.dependency.mod_q".to_string(),
    ];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }

    TestResult::pass(test_name)
}

/// Test dependency using ztd_name identifier
fn test_ztd_name_dependency() -> TestResult {
    use crate::mods;
    let test_name = "test_ztd_name_dependency";

    let meta_a: mods::Meta = toml::from_str(
        r#"name = "Test Mod A"
description = "Base test mod"
authors = ["OpenZT Test Suite"]
mod_id = "test.identifier.mod_a"
version = "1.0.0"
ztd_type = "openzt"
"#,
    )
    .unwrap();

    let meta_b: mods::Meta = toml::from_str(
        r#"name = "Test Mod B"
description = "Test mod with ztd_name dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.identifier.mod_b"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { ztd_name = "test.identifier.mod_a.ztd", name = "Test Mod A", ordering = "after" }
]
"#,
    )
    .unwrap();

    let mut mods = HashMap::new();
    mods.insert("test.identifier.mod_a".to_string(), meta_a);
    mods.insert("test.identifier.mod_b".to_string(), meta_b);

    let mut discovered = HashMap::new();
    discovered.insert(
        "test.identifier.mod_a".to_string(),
        ("test.identifier.mod_a.ztd".to_string(), mods["test.identifier.mod_a"].clone()),
    );
    discovered.insert(
        "test.identifier.mod_b".to_string(),
        ("test.identifier.mod_b.ztd".to_string(), mods["test.identifier.mod_b"].clone()),
    );

    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    let expected = vec!["test.identifier.mod_a".to_string(), "test.identifier.mod_b".to_string()];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }
    TestResult::pass(test_name)
}

/// Test dependency using dll_name identifier
fn test_dll_name_dependency() -> TestResult {
    use crate::mods;
    let test_name = "test_dll_name_dependency";

    let meta: mods::Meta = toml::from_str(
        r#"name = "Test Mod"
description = "Test mod with dll_name dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.identifier.mod_dll"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { dll_name = "langusa.dll", name = "English Language", optional = false }
]
"#,
    )
    .unwrap();

    let mut mods = HashMap::new();
    mods.insert("test.identifier.mod_dll".to_string(), meta);

    let discovered: HashMap<String, (String, mods::Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    if !result.order.contains(&"test.identifier.mod_dll".to_string()) {
        return TestResult::fail(test_name, "Mod should be in order even with DLL dependency".to_string());
    }
    TestResult::pass(test_name)
}

/// Test mixed identifier types in dependencies
fn test_mixed_identifier_types() -> TestResult {
    use crate::mods;
    let test_name = "test_mixed_identifier_types";

    let meta_a: mods::Meta = toml::from_str(
        r#"name = "Test Mod A"
description = "Base test mod"
authors = ["OpenZT Test Suite"]
mod_id = "test.mixed.mod_a"
version = "1.0.0"
ztd_type = "openzt"
"#,
    )
    .unwrap();

    let meta_b: mods::Meta = toml::from_str(
        r#"name = "Test Mod B"
description = "Test mod with mod_id dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.mixed.mod_b"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { mod_id = "test.mixed.mod_a", name = "Test Mod A", ordering = "after" }
]
"#,
    )
    .unwrap();

    let meta_c: mods::Meta = toml::from_str(
        r#"name = "Test Mod C"
description = "Test mod with ztd_name and dll_name dependencies"
authors = ["OpenZT Test Suite"]
mod_id = "test.mixed.mod_c"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { ztd_name = "test.mixed.mod_b.ztd", name = "Test Mod B", ordering = "after" },
    { dll_name = "langusa.dll", name = "English Language", optional = true }
]
"#,
    )
    .unwrap();

    let mut mods = HashMap::new();
    mods.insert("test.mixed.mod_a".to_string(), meta_a);
    mods.insert("test.mixed.mod_b".to_string(), meta_b);
    mods.insert("test.mixed.mod_c".to_string(), meta_c);

    let mut discovered = HashMap::new();
    discovered.insert("test.mixed.mod_a".to_string(), ("test.mixed.mod_a.ztd".to_string(), mods["test.mixed.mod_a"].clone()));
    discovered.insert("test.mixed.mod_b".to_string(), ("test.mixed.mod_b.ztd".to_string(), mods["test.mixed.mod_b"].clone()));
    discovered.insert("test.mixed.mod_c".to_string(), ("test.mixed.mod_c.ztd".to_string(), mods["test.mixed.mod_c"].clone()));

    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    let expected = vec!["test.mixed.mod_a".to_string(), "test.mixed.mod_b".to_string(), "test.mixed.mod_c".to_string()];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }
    TestResult::pass(test_name)
}

/// Test unresolved ztd_name dependency generates warning
fn test_unresolved_ztd_name_warning() -> TestResult {
    use crate::mods;
    let test_name = "test_unresolved_ztd_name_warning";

    let meta: mods::Meta = toml::from_str(
        r#"name = "Test Mod"
description = "Test mod with unresolved ztd_name dependency"
authors = ["OpenZT Test Suite"]
mod_id = "test.unresolved.mod"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { ztd_name = "nonexistent.ztd", name = "Nonexistent Mod", optional = true, ordering = "after" }
]
"#,
    )
    .unwrap();

    let mut mods = HashMap::new();
    mods.insert("test.unresolved.mod".to_string(), meta);

    let discovered: HashMap<String, (String, mods::Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    let has_missing_warning = result.warnings.iter().any(|w| matches!(w, ResolutionWarning::MissingOptionalDependency { .. }));

    if !has_missing_warning {
        return TestResult::fail(test_name, "Expected warning for missing optional ztd_name dependency".to_string());
    }
    TestResult::pass(test_name)
}

/// Test backwards compatibility with mod_id dependencies
fn test_backwards_compatibility_mod_id() -> TestResult {
    use crate::mods;
    let test_name = "test_backwards_compatibility_mod_id";

    let meta_a: mods::Meta = toml::from_str(
        r#"name = "Test Mod A"
description = "Base test mod"
authors = ["OpenZT Test Suite"]
mod_id = "test.compat.mod_a"
version = "1.0.0"
ztd_type = "openzt"
"#,
    )
    .unwrap();

    let meta_b: mods::Meta = toml::from_str(
        r#"name = "Test Mod B"
description = "Test mod with mod_id dependency (old style)"
authors = ["OpenZT Test Suite"]
mod_id = "test.compat.mod_b"
version = "1.0.0"
ztd_type = "openzt"
dependencies = [
    { mod_id = "test.compat.mod_a", name = "Test Mod A", ordering = "after" }
]
"#,
    )
    .unwrap();

    let mut mods = HashMap::new();
    mods.insert("test.compat.mod_a".to_string(), meta_a);
    mods.insert("test.compat.mod_b".to_string(), meta_b);

    let discovered: HashMap<String, (String, mods::Meta)> = HashMap::new();
    let resolver = DependencyResolver::new(mods, &discovered);
    let pure_legacy: &[(String, PathBuf)] = &[];
    let result = resolver.resolve_order(&[], &[], pure_legacy);

    let expected = vec!["test.compat.mod_a".to_string(), "test.compat.mod_b".to_string()];

    if result.order != expected {
        return TestResult::fail(test_name, format!("Expected order {:?}, got {:?}", expected, result.order));
    }
    TestResult::pass(test_name)
}
