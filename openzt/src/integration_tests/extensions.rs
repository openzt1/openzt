use crate::mods::EntityExtension;
use crate::resource_manager::openzt_mods::extensions::*;
use crate::resource_manager::openzt_mods::legacy_attributes::LegacyEntityType;
use std::collections::HashMap;

use super::TestResult;

crate::integration_tests![
    test_registry_tag_validation,
    test_register_extension,
    test_get_extension,
    test_get_extension_by_base,
    test_get_entity_tags,
    test_get_entity_attribute,
    test_extension_has_tag,
    test_list_extensions_with_tag,
    test_extension_validation_valid,
    test_extension_validation_invalid_tag,
    test_extension_validation_invalid_attribute,
];

fn test_registry_tag_validation() -> TestResult {
    let test_name = "test_registry_tag_validation";
    clear_extensions();

    // Register a tag for scenery only
    register_tag("test", "roof", "Test roof tag", EntityScope::single(LegacyEntityType::Scenery)).unwrap();

    // Try to add roof tag to scenery extension - should succeed
    let ext1 = EntityExtension::new_test("legacy.scenery.statue".to_string(), vec!["roof".to_string()], HashMap::new());
    match add_extension("test_mod".to_string(), "scenery.statue".to_string(), ext1) {
        Ok(_) => {}
        Err(e) => return TestResult::fail(test_name, format!("Scenery with roof tag should succeed: {}", e)),
    }
    clear_extensions();

    // Try to add roof tag to animal extension - should fail
    let ext2 = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["roof".to_string()], HashMap::new());
    match add_extension("test_mod".to_string(), "animals.elephant".to_string(), ext2) {
        Ok(_) => return TestResult::fail(test_name, "Animal with roof tag should fail".to_string()),
        Err(_) => TestResult::pass(test_name),
    }
}

fn test_register_extension() -> TestResult {
    let test_name = "test_register_extension";
    clear_extensions();

    // Register test tag and attribute
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();
    register_attribute("test", "example_attribute", "Test attribute for unit tests", EntityScope::all(), None).unwrap();

    let mut attributes = HashMap::new();
    attributes.insert("example_attribute".to_string(), "test_value".to_string());

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], attributes);

    match add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension) {
        Ok(_) => TestResult::pass(test_name),
        Err(e) => TestResult::fail(test_name, format!("Failed: {}", e)),
    }
}

fn test_get_extension() -> TestResult {
    let test_name = "test_get_extension";
    clear_extensions();

    // Register test tag and attribute
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();
    register_attribute("test", "example_attribute", "Test attribute for unit tests", EntityScope::all(), None).unwrap();

    let mut attributes = HashMap::new();
    attributes.insert("example_attribute".to_string(), "test_value".to_string());

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], attributes);

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension.clone());

    match get_extension("animals.elephant") {
        Some(record) => {
            if record.mod_id == "test_mod"
                && record.base == "legacy.animals.elephant"
                && record.extension.tags().contains(&"example_tag".to_string())
                && record.extension.attributes().get("example_attribute") == Some(&"test_value".to_string())
            {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, "Data mismatch".to_string())
            }
        }
        None => TestResult::fail(test_name, "Not found".to_string()),
    }
}

fn test_get_extension_by_base() -> TestResult {
    let test_name = "test_get_extension_by_base";
    clear_extensions();

    // Register test tag
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], HashMap::new());

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension);

    match get_extension_by_base("legacy.animals.elephant") {
        Some(record) => {
            if record.extension_key == "animals.elephant" && record.mod_id == "test_mod" {
                TestResult::pass(test_name)
            } else {
                TestResult::fail(test_name, "Data mismatch".to_string())
            }
        }
        None => TestResult::fail(test_name, "Not found by base".to_string()),
    }
}

fn test_get_entity_tags() -> TestResult {
    let test_name = "test_get_entity_tags";
    clear_extensions();

    // Register test tag
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], HashMap::new());

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension);

    match get_entity_tags("animals.elephant") {
        Ok(tags) if tags == vec!["example_tag".to_string()] => TestResult::pass(test_name),
        Ok(tags) => TestResult::fail(test_name, format!("Wrong tags: {:?}", tags)),
        Err(e) => TestResult::fail(test_name, format!("Failed: {}", e)),
    }
}

fn test_get_entity_attribute() -> TestResult {
    let test_name = "test_get_entity_attribute";
    clear_extensions();

    // Register test attribute
    register_attribute("test", "example_attribute", "Test attribute for unit tests", EntityScope::all(), None).unwrap();

    let mut attributes = HashMap::new();
    attributes.insert("example_attribute".to_string(), "test_value".to_string());

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec![], attributes);

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension);

    match get_entity_attribute("animals.elephant", "example_attribute") {
        Ok(Some(value)) if value == "test_value" => TestResult::pass(test_name),
        Ok(Some(value)) => TestResult::fail(test_name, format!("Wrong value: {}", value)),
        Ok(None) => TestResult::fail(test_name, "Attribute not found".to_string()),
        Err(e) => TestResult::fail(test_name, format!("Failed: {}", e)),
    }
}

fn test_extension_has_tag() -> TestResult {
    let test_name = "test_extension_has_tag";
    clear_extensions();

    // Register test tag
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], HashMap::new());

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension);

    match entity_has_tag("animals.elephant", "example_tag") {
        Ok(true) => TestResult::pass(test_name),
        Ok(false) => TestResult::fail(test_name, "Tag not found".to_string()),
        Err(e) => TestResult::fail(test_name, format!("Failed: {}", e)),
    }
}

fn test_list_extensions_with_tag() -> TestResult {
    let test_name = "test_list_extensions_with_tag";
    clear_extensions();

    // Register test tag
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();

    let ext1 = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], HashMap::new());

    let ext2 = EntityExtension::new_test("legacy.animals.lion".to_string(), vec!["example_tag".to_string()], HashMap::new());

    let _ = add_extension("test_mod".to_string(), "animals.elephant".to_string(), ext1);
    let _ = add_extension("test_mod".to_string(), "animals.lion".to_string(), ext2);

    let extensions = list_extensions_with_tag("example_tag");
    if extensions.len() == 2 && extensions.contains(&"animals.elephant".to_string()) && extensions.contains(&"animals.lion".to_string()) {
        TestResult::pass(test_name)
    } else {
        TestResult::fail(test_name, format!("Wrong extensions: {:?}", extensions))
    }
}

fn test_extension_validation_valid() -> TestResult {
    let test_name = "test_extension_validation_valid";
    clear_extensions();

    // Register test tag and attribute
    register_tag("test", "example_tag", "Test tag for unit tests", EntityScope::all()).unwrap();
    register_attribute("test", "example_attribute", "Test attribute for unit tests", EntityScope::all(), None).unwrap();

    let mut attributes = HashMap::new();
    attributes.insert("example_attribute".to_string(), "value".to_string());

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["example_tag".to_string()], attributes);

    match add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension) {
        Ok(_) => TestResult::pass(test_name),
        Err(e) => TestResult::fail(test_name, format!("Valid extension rejected: {}", e)),
    }
}

fn test_extension_validation_invalid_tag() -> TestResult {
    let test_name = "test_extension_validation_invalid_tag";
    clear_extensions();

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec!["invalid_tag".to_string()], HashMap::new());

    match add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension) {
        Ok(_) => TestResult::fail(test_name, "Should reject invalid tag".to_string()),
        Err(_) => TestResult::pass(test_name),
    }
}

fn test_extension_validation_invalid_attribute() -> TestResult {
    let test_name = "test_extension_validation_invalid_attribute";
    clear_extensions();

    let mut attributes = HashMap::new();
    attributes.insert("invalid_attribute".to_string(), "value".to_string());

    let extension = EntityExtension::new_test("legacy.animals.elephant".to_string(), vec![], attributes);

    match add_extension("test_mod".to_string(), "animals.elephant".to_string(), extension) {
        Ok(_) => TestResult::fail(test_name, "Should reject invalid attribute".to_string()),
        Err(_) => TestResult::pass(test_name),
    }
}
