use std::collections::HashMap;
use tracing::{warn, info};
use crate::mods::{Meta, Ordering, DependencyIdentifier};

/// Result of load order validation
#[derive(Debug)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<ValidationWarning>,
    pub errors: Vec<ValidationError>,
}

/// Warnings for non-critical issues in load order
#[derive(Debug, Clone)]
pub enum ValidationWarning {
    OrderingViolation {
        mod_id: String,
        expected_relation: String,
        other_mod: String,
        current_position: usize,
        other_position: usize,
    },
    VersionMismatch {
        mod_id: String,
        required_mod: String,
        required_version: String,
        found_version: String,
    },
    OptionalDependencyMissing {
        mod_id: String,
        missing_dep: String,
    },
}

/// Errors for critical issues in load order
#[derive(Debug, Clone)]
pub enum ValidationError {
    RequiredDependencyMissing {
        mod_id: String,
        missing_dep: String,
    },
    CircularDependency {
        cycle: Vec<String>,
    },
}

/// Validate a mod load order against dependencies
///
/// This checks if the existing order respects dependency constraints,
/// but does NOT modify the order - only reports violations.
pub fn validate_load_order(
    order: &[String],
    mods: &HashMap<String, Meta>,
) -> ValidationResult {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Create position map for efficient lookups
    let position_map: HashMap<&String, usize> = order.iter()
        .enumerate()
        .map(|(i, id)| (id, i))
        .collect();

    // Validate each mod's dependencies
    for (idx, mod_id) in order.iter().enumerate() {
        let Some(meta) = mods.get(mod_id) else {
            warn!("Mod '{}' in load order but not found in available mods", mod_id);
            continue;
        };

        // Check each dependency
        for dep in meta.dependencies() {
            // Resolve the dependency identifier to a mod_id for validation
            // Note: We can't resolve ztd_name here since we don't have the ztd_to_mod_id mapping
            // So for validation purposes, we skip ztd_name and dll_name dependencies
            let dep_mod_id = match dep.identifier() {
                DependencyIdentifier::ModId(id) => id.clone(),
                DependencyIdentifier::ZtdName(_) => {
                    // Skip validation for ztd_name dependencies (they're validated in dependency_resolver)
                    continue;
                }
                DependencyIdentifier::DllName(_) => {
                    // Skip validation for dll_name dependencies (they're validated in dependency_resolver)
                    continue;
                }
            };

            // Check if dependency exists
            if !mods.contains_key(&dep_mod_id) {
                if *dep.optional() {
                    warnings.push(ValidationWarning::OptionalDependencyMissing {
                        mod_id: mod_id.clone(),
                        missing_dep: dep_mod_id.clone(),
                    });
                } else {
                    errors.push(ValidationError::RequiredDependencyMissing {
                        mod_id: mod_id.clone(),
                        missing_dep: dep_mod_id.clone(),
                    });
                }
                continue;
            }

            // Check version constraints
            if let Some(min_version) = dep.min_version() {
                let dep_meta = &mods[&dep_mod_id];
                if dep_meta.version() < min_version {
                    warnings.push(ValidationWarning::VersionMismatch {
                        mod_id: mod_id.clone(),
                        required_mod: dep_mod_id.clone(),
                        required_version: format!("{}", min_version),
                        found_version: format!("{}", dep_meta.version()),
                    });
                }
            }

            // Check ordering constraints
            if let Some(&dep_position) = position_map.get(&dep_mod_id) {
                let violation = match dep.ordering() {
                    Ordering::After => {
                        // Current mod should load AFTER dependency
                        if idx < dep_position {
                            Some(format!("should load after '{}'", dep_mod_id))
                        } else {
                            None
                        }
                    }
                    Ordering::Before => {
                        // Current mod should load BEFORE dependency
                        if idx > dep_position {
                            Some(format!("should load before '{}'", dep_mod_id))
                        } else {
                            None
                        }
                    }
                    Ordering::None => None,
                };

                if let Some(expected) = violation {
                    warnings.push(ValidationWarning::OrderingViolation {
                        mod_id: mod_id.clone(),
                        expected_relation: expected,
                        other_mod: dep_mod_id.clone(),
                        current_position: idx,
                        other_position: dep_position,
                    });
                }
            }
        }
    }

    let is_valid = errors.is_empty();

    ValidationResult {
        is_valid,
        warnings,
        errors,
    }
}

/// Log validation warnings and errors
pub fn log_validation_result(result: &ValidationResult) {
    if result.warnings.is_empty() && result.errors.is_empty() {
        info!("Mod load order validation passed with no issues");
        return;
    }

    // Log errors
    for error in &result.errors {
        match error {
            ValidationError::RequiredDependencyMissing { mod_id, missing_dep } => {
                warn!(
                    "ERROR: Mod '{}' requires missing dependency '{}'",
                    mod_id, missing_dep
                );
            }
            ValidationError::CircularDependency { cycle } => {
                warn!(
                    "ERROR: Circular dependency detected: {:?}",
                    cycle
                );
            }
        }
    }

    // Log warnings
    for warning in &result.warnings {
        match warning {
            ValidationWarning::OrderingViolation {
                mod_id,
                expected_relation,
                other_mod,
                current_position,
                other_position,
            } => {
                warn!(
                    "WARNING: Mod '{}' at position {} {} (currently at position {})",
                    mod_id, current_position, expected_relation, other_position
                );
                info!(
                    "  Recommendation: Check openzt.toml and adjust order of '{}' relative to '{}'",
                    mod_id, other_mod
                );
            }
            ValidationWarning::VersionMismatch {
                mod_id,
                required_mod,
                required_version,
                found_version,
            } => {
                warn!(
                    "WARNING: Mod '{}' requires '{}' >= {}, but found version {}",
                    mod_id, required_mod, required_version, found_version
                );
                info!("  Recommendation: Update '{}' to version {} or later", required_mod, required_version);
            }
            ValidationWarning::OptionalDependencyMissing { mod_id, missing_dep } => {
                info!(
                    "INFO: Mod '{}' has optional dependency '{}' which is not present",
                    mod_id, missing_dep
                );
            }
        }
    }

    if !result.is_valid {
        warn!("Mod load order validation found {} error(s)", result.errors.len());
    }
    if !result.warnings.is_empty() {
        info!("Mod load order validation found {} warning(s)", result.warnings.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_validation() {
        let result = validate_load_order(&[], &HashMap::new());
        assert!(result.is_valid);
        assert!(result.warnings.is_empty());
        assert!(result.errors.is_empty());
    }
}
