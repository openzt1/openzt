use mlua::Lua;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::Mutex;
use tracing::info;

use crate::resource_manager::openzt_mods::extensions;
use crate::resource_manager::openzt_mods::legacy_attributes::{self, LegacyEntityType};

/// Macro to simplify registering Lua functions
///
/// # Usage
/// ```rust
/// use openztlib::lua_fn;
///
/// // No arguments
/// lua_fn!("my_func", "Does something", "my_func()", || {
///     Ok("result")
/// });
///
/// // Single argument
/// lua_fn!("my_func", "Does something", "my_func(arg)", |arg: String| {
///     Ok(format!("Got: {}", arg))
/// });
///
/// // Multiple arguments
/// lua_fn!("my_func", "Does something", "my_func(a, b)", |a: u32, b: String| {
///     Ok(format!("{}: {}", a, b))
/// });
///
/// // Optional argument
/// lua_fn!("my_func", "Does something", "my_func([opt])", |opt: Option<String>| {
///     Ok(opt.unwrap_or_else(|| "default".to_string()))
/// });
/// ```
#[macro_export]
macro_rules! lua_fn {
    // No arguments
    ($name:expr, $desc:expr, $sig:expr, || $body:block) => {
        $crate::scripting::add_lua_function(
            $name,
            $desc,
            $sig,
            |lua| lua.create_function(|_, ()| $body).unwrap()
        ).unwrap()
    };

    // Single argument
    ($name:expr, $desc:expr, $sig:expr, |$arg:ident : $arg_ty:ty| $body:block) => {
        $crate::scripting::add_lua_function(
            $name,
            $desc,
            $sig,
            |lua| lua.create_function(|_, $arg: $arg_ty| $body).unwrap()
        ).unwrap()
    };

    // Multiple arguments (2+)
    ($name:expr, $desc:expr, $sig:expr, |$($arg:ident : $arg_ty:ty),+ $(,)?| $body:block) => {
        $crate::scripting::add_lua_function(
            $name,
            $desc,
            $sig,
            |lua| lua.create_function(|_, ($($arg),+): ($($arg_ty),+)| $body).unwrap()
        ).unwrap()
    };
}

static LUA_CONTEXT: LazyLock<Mutex<Lua>> = LazyLock::new(|| Mutex::new(Lua::new()));

// Metadata for help() function
static LUA_FUNCTION_METADATA: LazyLock<Mutex<Vec<LuaFunctionMeta>>> = LazyLock::new(|| Mutex::new(Vec::new()));

struct LuaFunctionMeta {
    name: String,
    description: String,
    signature: String,
}

/// Registers a Lua function with metadata for help()
pub fn add_lua_function(name: &str, description: &str, signature: &str, func_closure: fn(&Lua) -> mlua::Function) -> Result<(), mlua::Error> {
    // Add metadata
    LUA_FUNCTION_METADATA.lock().unwrap().push(LuaFunctionMeta {
        name: name.to_string(),
        description: description.to_string(),
        signature: signature.to_string(),
    });

    // Register function in Lua global scope
    let lua = LUA_CONTEXT.lock().unwrap();
    let func = func_closure(&lua);
    let globals = lua.globals();
    globals.set(name, func)
}

/// Executes Lua code and returns the result as a string
pub fn execute_lua(code: &str) -> Result<String, String> {
    let lua = LUA_CONTEXT.lock().unwrap();
    match lua.load(code).eval::<mlua::Value>() {
        Ok(value) => Ok(lua_value_to_string(&value)),
        Err(e) => Err(format!("Lua error: {}", e)),
    }
}

/// Converts a Lua value to a string representation
fn lua_value_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_else(|_| "<invalid utf8>".to_string()),
        mlua::Value::Table(t) => {
            // Simple table representation
            let mut result = String::from("{");
            let mut first = true;
            for (k, v) in t.pairs::<mlua::Value, mlua::Value>().flatten() {
                if !first {
                    result.push_str(", ");
                }
                first = false;
                result.push_str(&format!("{} = {}", lua_value_to_string(&k), lua_value_to_string(&v)));
            }
            result.push('}');
            result
        }
        mlua::Value::Function(_) => "<function>".to_string(),
        mlua::Value::Thread(_) => "<thread>".to_string(),
        mlua::Value::UserData(_) => "<userdata>".to_string(),
        mlua::Value::LightUserData(_) => "<lightuserdata>".to_string(),
        mlua::Value::Error(e) => format!("<error: {}>", e),
        _ => "<unknown>".to_string(),
    }
}

pub fn init() {
    info!("Initializing Lua scripting");

    // Register the continue() function
    lua_fn!("click_continue", "Clicks the continue button", "continue()", || {
        unsafe {
            openzt_detour::gen::ztui::CLICK_CONTINUE.original()();
        }
        Ok(())
    });

    // Register the help() function
    lua_fn!(
        "help",
        "Lists available Lua functions or searches by keyword",
        "help([search_term])",
        |search: Option<String>| {
            let metadata = LUA_FUNCTION_METADATA.lock().unwrap();
            let filtered: Vec<&LuaFunctionMeta> = match &search {
                Some(term) => metadata
                    .iter()
                    .filter(|m| m.name.contains(term.as_str()) || m.description.to_lowercase().contains(&term.to_lowercase()))
                    .collect(),
                None => metadata.iter().collect(),
            };

            if filtered.is_empty() {
                if let Some(term) = search {
                    return Ok(format!("No functions found matching '{}'", term));
                } else {
                    return Ok("No functions registered".to_string());
                }
            }

            let mut result = String::new();
            for meta in filtered {
                result.push_str(&format!("{} - {}\n  Usage: {}\n\n", meta.name, meta.description, meta.signature));
            }
            Ok(result)
        }
    );

    // Register the get_legacy_attribute() function
    lua_fn!(
        "get_legacy_attribute",
        "Get a legacy entity attribute (name_id currently supported)",
        "get_legacy_attribute(entity_type, entity_name, [subtype], attribute)",
        |entity_type: String, entity_name: String, args: mlua::Variadic<String>| {
            // Parse variadic args: either (subtype, attribute) or just (attribute)
            let (subtype, attribute) = if args.len() == 2 {
                (Some(args[0].as_str()), args[1].as_str())
            } else if args.len() == 1 {
                (None, args[0].as_str())
            } else {
                return Ok((String::new(), "Expected 2 or 3 arguments".to_string()));
            };

            let type_result: Result<LegacyEntityType, _> = entity_type.parse();
            let entity_type = match type_result {
                Ok(t) => t,
                Err(e) => return Ok((String::new(), format!("Invalid entity type: {}", e))),
            };

            match legacy_attributes::get_legacy_attribute_with_subtype(entity_type, &entity_name, subtype, attribute) {
                Ok(value) => Ok((value, String::new())),
                Err(e) => Ok((String::new(), e.to_string())),
            }
        }
    );

    // Register the list_legacy_entities() function
    lua_fn!(
        "list_legacy_entities",
        "List all legacy entities (optionally filtered by type)",
        "list_legacy_entities([entity_type])",
        |entity_type: Option<String>| {
            use crate::resource_manager::openzt_mods::legacy_attributes::LEGACY_ATTRIBUTES_MAP;

            let map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();

            if let Some(type_str) = entity_type {
                // List specific type
                let entity_type: Result<LegacyEntityType, _> = type_str.parse();
                let entity_type = match entity_type {
                    Ok(t) => t,
                    Err(e) => return Ok(format!("Invalid entity type: {}", e)),
                };

                if let Some(entities) = map.get(&entity_type) {
                    let mut result = format!("{} ({} entities):\n", type_str, entities.len());
                    let mut entity_names: Vec<_> = entities.keys().collect();
                    entity_names.sort();
                    for name in entity_names {
                        if let Some(attrs) = entities.get(name) {
                            let subtype_list = attrs.subtype_list();

                            // Check if all subtypes share the same name_id
                            let name_ids: Vec<_> = attrs.subtype_attributes.values().filter_map(|v| v.name_id).collect();
                            let all_same = name_ids.len() == 1 || name_ids.windows(2).all(|w| w[0] == w[1]);

                            // Build the display string
                            if subtype_list.is_empty() {
                                // No subtypes - show single name_id if available
                                if let Some(name_id) = attrs.get_name_id(None) {
                                    result.push_str(&format!("  {} -> name_id={}\n", name, name_id));
                                } else {
                                    result.push_str(&format!("  {} -> (no name_id)\n", name));
                                }
                            } else if !name_ids.is_empty() && all_same {
                                // Has subtypes and they all share the same name_id - show compact format
                                result.push_str(&format!("  {} -> [{}] name_id={}\n", name, subtype_list, name_ids[0]));
                            } else {
                                // Has subtypes with different name_ids - show each subtype
                                result.push_str(&format!("  {}:\n", name));
                                let mut subtype_name_ids: Vec<(String, Option<u32>)> = attrs
                                    .subtype_attributes
                                    .iter()
                                    .filter(|(k, _)| !k.is_empty())
                                    .map(|(k, v)| (k.clone(), v.name_id))
                                    .collect();
                                subtype_name_ids.sort();
                                for (subtype, name_id) in subtype_name_ids {
                                    if let Some(nid) = name_id {
                                        result.push_str(&format!("    [{}] name_id={}\n", subtype, nid));
                                    } else {
                                        result.push_str(&format!("    [{}] (no name_id)\n", subtype));
                                    }
                                }
                            }
                        }
                    }
                    Ok(result)
                } else {
                    Ok(format!("No entities found for type '{}'", type_str))
                }
            } else {
                // List ALL entities with their attributes (for debugging)
                let mut result = String::from("All legacy entities:\n");
                let mut types: Vec<_> = map.keys().collect();
                types.sort_by_key(|t| t.as_str());
                for entity_type in types {
                    if let Some(entities) = map.get(entity_type) {
                        result.push_str(&format!("\n[{}] ({} entities):\n", entity_type.as_str(), entities.len()));
                        let mut entity_names: Vec<_> = entities.keys().collect();
                        entity_names.sort();
                        for name in entity_names {
                            if let Some(attrs) = entities.get(name) {
                                let subtype_list = attrs.subtype_list();

                                // Check if all subtypes share the same name_id
                                let name_ids: Vec<_> = attrs.subtype_attributes.values().filter_map(|v| v.name_id).collect();
                                let all_same = name_ids.len() == 1 || name_ids.windows(2).all(|w| w[0] == w[1]);

                                // Build the display string
                                if subtype_list.is_empty() {
                                    // No subtypes - show single name_id if available
                                    if let Some(name_id) = attrs.get_name_id(None) {
                                        result.push_str(&format!("  {} -> name_id={}\n", name, name_id));
                                    } else {
                                        result.push_str(&format!("  {} -> (no name_id)\n", name));
                                    }
                                } else if !name_ids.is_empty() && all_same {
                                    // Has subtypes and they all share the same name_id - show compact format
                                    result.push_str(&format!("  {} -> [{}] name_id={}\n", name, subtype_list, name_ids[0]));
                                } else {
                                    // Has subtypes with different name_ids - show each subtype
                                    result.push_str(&format!("  {}:\n", name));
                                    let mut subtype_name_ids: Vec<(String, Option<u32>)> = attrs
                                        .subtype_attributes
                                        .iter()
                                        .filter(|(k, _)| !k.is_empty())
                                        .map(|(k, v)| (k.clone(), v.name_id))
                                        .collect();
                                    subtype_name_ids.sort();
                                    for (subtype, name_id) in subtype_name_ids {
                                        if let Some(nid) = name_id {
                                            result.push_str(&format!("    [{}] name_id={}\n", subtype, nid));
                                        } else {
                                            result.push_str(&format!("    [{}] (no name_id)\n", subtype));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(result)
            }
        }
    );

    // Register the list_legacy_types() function
    lua_fn!(
        "list_legacy_types",
        "List all available legacy entity types with counts",
        "list_legacy_types()",
        || {
            use crate::resource_manager::openzt_mods::legacy_attributes::LEGACY_ATTRIBUTES_MAP;

            let map = LEGACY_ATTRIBUTES_MAP.lock().unwrap();
            let mut result = String::from("Available legacy entity types:\n");

            // Sort types alphabetically
            let mut types: Vec<_> = map.keys().collect();
            types.sort_by_key(|t| t.as_str());

            for entity_type in types {
                if let Some(entities) = map.get(entity_type) {
                    result.push_str(&format!("  {} ({} entities)\n", entity_type.as_str(), entities.len()));
                }
            }

            // Also list types that have no entities
            let all_types = &[
                ("animals", LegacyEntityType::Animal),
                ("buildings", LegacyEntityType::Building),
                ("fences", LegacyEntityType::Fence),
                ("food", LegacyEntityType::Food),
                ("guests", LegacyEntityType::Guest),
                ("items", LegacyEntityType::Item),
                ("paths", LegacyEntityType::Path),
                ("scenery", LegacyEntityType::Scenery),
                ("staff", LegacyEntityType::Staff),
                ("walls", LegacyEntityType::Wall),
            ];

            for (name, ty) in all_types {
                if !map.contains_key(ty) {
                    result.push_str(&format!("  {} (0 entities)\n", name));
                }
            }

            Ok(result)
        }
    );

    // Register the get_extension() function
    lua_fn!(
        "get_extension",
        "Get extension data by extension key (e.g., 'animals.elephant')",
        "get_extension(extension_key)",
        |extension_key: String| {
            match extensions::get_extension(&extension_key) {
                Some(record) => {
                    let tags_str = if record.extension.tags().is_empty() {
                        "(none)".to_string()
                    } else {
                        record.extension.tags().join(", ")
                    };
                    let attrs_str = if record.extension.attributes().is_empty() {
                        "(none)".to_string()
                    } else {
                        record
                            .extension
                            .attributes()
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    Ok((
                        format!("Mod: {}\nBase: {}\nTags: {}\nAttributes: {}", record.mod_id, record.base, tags_str, attrs_str),
                        String::new(),
                    ))
                }
                None => Ok(("(no extension data)".to_string(), String::new())),
            }
        }
    );

    // Register the get_extension_by_base() function
    lua_fn!(
        "get_extension_by_base",
        "Get extension data by base entity (e.g., 'legacy.animals.elephant')",
        "get_extension_by_base(base)",
        |base: String| {
            match extensions::get_extension_by_base(&base) {
                Some(record) => {
                    let tags_str = if record.extension.tags().is_empty() {
                        "(none)".to_string()
                    } else {
                        record.extension.tags().join(", ")
                    };
                    let attrs_str = if record.extension.attributes().is_empty() {
                        "(none)".to_string()
                    } else {
                        record
                            .extension
                            .attributes()
                            .iter()
                            .map(|(k, v)| format!("{}: {}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    Ok((
                        format!("Mod: {}\nKey: {}\nTags: {}\nAttributes: {}", record.mod_id, record.extension_key, tags_str, attrs_str),
                        String::new(),
                    ))
                }
                None => Ok(("(no extension data)".to_string(), String::new())),
            }
        }
    );

    // Register the get_extension_tags() function
    lua_fn!(
        "get_extension_tags",
        "Get tags for an extension by key",
        "get_extension_tags(extension_key)",
        |extension_key: String| {
            match extensions::get_entity_tags(&extension_key) {
                Ok(tags) => Ok(if tags.is_empty() { "(no tags)".to_string() } else { tags.join(", ") }),
                Err(e) => Ok(format!("Error: {}", e)),
            }
        }
    );

    // Register the get_extension_attribute() function
    lua_fn!(
        "get_extension_attribute",
        "Get a specific attribute for an extension",
        "get_extension_attribute(extension_key, attribute_key)",
        |extension_key: String, attribute_key: String| {
            match extensions::get_entity_attribute(&extension_key, &attribute_key) {
                Ok(Some(value)) => Ok(value),
                Ok(None) => Ok("(attribute not found)".to_string()),
                Err(e) => Ok(format!("Error: {}", e)),
            }
        }
    );

    // Register the extension_has_tag() function
    lua_fn!(
        "extension_has_tag",
        "Check if an extension has a specific tag",
        "extension_has_tag(extension_key, tag)",
        |extension_key: String, tag: String| {
            match extensions::entity_has_tag(&extension_key, &tag) {
                Ok(has_tag) => Ok(if has_tag { "true" } else { "false" }.to_string()),
                Err(e) => Ok(format!("Error: {}", e)),
            }
        }
    );

    // Register the list_extensions_with_tag() function
    lua_fn!(
        "list_extensions_with_tag",
        "List all extensions that have a specific tag",
        "list_extensions_with_tag(tag)",
        |tag: String| {
            let exts = extensions::list_extensions_with_tag(&tag);
            Ok(if exts.is_empty() { "(no extensions found)".to_string() } else { exts.join(", ") })
        }
    );

    // Register the list_registered_tags() function
    lua_fn!(
        "list_registered_tags",
        "List all registered tags (optionally filtered by entity type)",
        "list_registered_tags([entity_type])",
        |entity_type: Option<String>| {
            use crate::resource_manager::openzt_mods::extensions::EXTENSION_REGISTRY;

            let registry = EXTENSION_REGISTRY.lock().unwrap();
            let tags = registry.list_tags();

            let filtered = if let Some(type_str) = entity_type {
                match LegacyEntityType::from_str(&type_str) {
                    Ok(et) => tags.iter().filter(|def| def.scope.includes(et)).cloned().collect::<Vec<_>>(),
                    Err(_) => vec![],
                }
            } else {
                tags.iter().cloned().collect::<Vec<_>>()
            };

            if filtered.is_empty() {
                Ok("(no registered tags)".to_string())
            } else {
                let result = filtered
                    .iter()
                    .map(|def| format!("{} ({}) - {}", def.name, def.module, def.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(result)
            }
        }
    );

    // Register the list_registered_attributes() function
    lua_fn!(
        "list_registered_attributes",
        "List all registered attributes (optionally filtered by entity type)",
        "list_registered_attributes([entity_type])",
        |entity_type: Option<String>| {
            use crate::resource_manager::openzt_mods::extensions::EXTENSION_REGISTRY;

            let registry = EXTENSION_REGISTRY.lock().unwrap();
            let attrs = registry.list_attributes();

            let filtered = if let Some(type_str) = entity_type {
                match LegacyEntityType::from_str(&type_str) {
                    Ok(et) => attrs.iter().filter(|def| def.scope.includes(et)).cloned().collect::<Vec<_>>(),
                    Err(_) => vec![],
                }
            } else {
                attrs.iter().cloned().collect::<Vec<_>>()
            };

            if filtered.is_empty() {
                Ok("(no registered attributes)".to_string())
            } else {
                let result = filtered
                    .iter()
                    .map(|def| format!("{} ({}) - {}", def.name, def.module, def.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(result)
            }
        }
    );

    // Register the hide_roofs() function
    lua_fn!("hide_roofs", "Hide all entities tagged with 'roof'", "hide_roofs()", || {
        crate::roofs::hide_roofs();
        Ok(("Roofs hidden".to_string(), None::<String>))
    });
}
