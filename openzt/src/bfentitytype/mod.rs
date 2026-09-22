mod ambient;
mod base;
mod fence;
mod items;
mod scenery;
mod staff;
mod units;
mod wrapper;

#[allow(unused_imports)]
pub use ambient::*;
pub use base::*;
#[allow(unused_imports)]
pub use fence::*;
#[allow(unused_imports)]
pub use items::*;
pub use scenery::*;
#[allow(unused_imports)]
pub use staff::*;
pub use units::*;
pub use wrapper::*;

use tracing::info;

use crate::command_console::CommandError;
use crate::lua_fn;
use crate::util::map_from_memory;
use crate::ztui::get_selected_entity_type_address;
use crate::ztworldmgr;

// ------------ Custom Command Implementation ------------ //

fn command_sel_type(args: Vec<&str>) -> Result<String, CommandError> {
    let entity_type_address = get_selected_entity_type_address();
    if entity_type_address == 0 {
        return Err(CommandError::new("No entity selected".to_string()));
    }

    let entity_type = map_bfentitytype(entity_type_address)?;

    if args.is_empty() {
        Ok(entity_type.print_config_details())
    } else if args[0] == "-v" {
        // if -v flag is used, print the entity type configuration and other details
        info!("Printing configuration for entity type at address {:#x}", entity_type_address);
        // print the entity type configuration for the selected entity type
        Ok(entity_type.print_config())
    } else if args.len() == 2 {
        // parse the subargs for the entity type
        Ok(entity_type.set_config(args[0], args[1])?)
    } else {
        Ok("Invalid argument".to_string())
    }
}

#[allow(dead_code)]
fn print_info_image_name(entity_type: &BFEntityType, config: &mut String) {
    info!("Checking for cInfoImageName...");
    // TODO: move cInfoImageName to a separate struct (probably ZTSceneryType). crashes when trying to access it from guests
    if !entity_type.get_info_image_name().is_empty() {
        info!("Entity type has cInfoImageName: {}", entity_type.get_info_image_name());
        config.push_str("\n[Characteristics/Strings]\n");
        config.push_str(&entity_type.get_info_image_name());
    }
}

pub fn command_make_sel(args: Vec<&str>) -> Result<String, CommandError> {
    if args.is_empty() {
        Err(Into::into("Usage: make_sel <id>"))
    } else {
        let id = args[0].parse::<u32>()?;
        let entity_type_ptr = ztworldmgr::get_entity_type_by_id(id);
        if entity_type_ptr == 0 {
            return Err(Into::into("Entity type not found"));
        }
        let entity_type = map_from_memory::<ZTSceneryType>(entity_type_ptr);
        if entity_type.selectable {
            return Ok(format!("Entity type {} is already selectable", entity_type.bfentitytype.get_type_name()));
        }
        entity_type.selectable = true;
        Ok(format!("Entity type {} is now selectable", entity_type.bfentitytype.get_type_name()))
    }
}

// initializes the custom command
pub fn init() {
    // sel_type([key], [value]) - optional arguments
    lua_fn!(
        "sel_type",
        "Gets selected entity type config, with optional key/value to set",
        "sel_type([key], [value]) or sel_type(\"-v\")",
        |args: mlua::Variadic<String>| {
            let args_vec: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match command_sel_type(args_vec) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // make_sel(id) - single u32 arg
    lua_fn!("make_sel", "Makes entity type selectable", "make_sel(id)", |id: u32| {
        let id_str = id.to_string();
        match command_make_sel(vec![&id_str]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    // Explicit list (not derived from the enum) so it visibly needs updating when a
    // variant is added and the match arm in `FromStr` is forgotten.
    const ZT_ENTITY_TYPE_CLASS_CASES: &[(ZTEntityTypeClass, &str)] = &[
        (ZTEntityTypeClass::Animal, "animal"),
        (ZTEntityTypeClass::Ambient, "ambient"),
        (ZTEntityTypeClass::Guest, "guest"),
        (ZTEntityTypeClass::Fence, "fence"),
        (ZTEntityTypeClass::TourGuide, "tourguide"),
        (ZTEntityTypeClass::Building, "building"),
        (ZTEntityTypeClass::Scenery, "scenery"),
        (ZTEntityTypeClass::Food, "food"),
        (ZTEntityTypeClass::TankFilter, "tankfilter"),
        (ZTEntityTypeClass::Path, "path"),
        (ZTEntityTypeClass::Rubble, "rubble"),
        (ZTEntityTypeClass::TankWall, "tankwall"),
        (ZTEntityTypeClass::Keeper, "keeper"),
        (ZTEntityTypeClass::MaintenanceWorker, "maintenanceworker"),
        (ZTEntityTypeClass::Drt, "drt"),
        (ZTEntityTypeClass::BFOverlay, "bfoverlay"),
        (ZTEntityTypeClass::BFUnit, "bfunit"),
        (ZTEntityTypeClass::ZTUnit, "ztunit"),
        (ZTEntityTypeClass::Staff, "staff"),
        (ZTEntityTypeClass::BFEntity, "bfentity"),
    ];

    #[test]
    fn test_zt_entity_type_class_from_str_round_trip() {
        for (variant, s) in ZT_ENTITY_TYPE_CLASS_CASES {
            assert_eq!(s.parse::<ZTEntityTypeClass>().unwrap(), *variant, "failed to parse '{}'", s);
        }
    }

    #[test]
    fn test_zt_entity_type_class_from_str_case_insensitive() {
        assert_eq!("Animal".parse::<ZTEntityTypeClass>().unwrap(), ZTEntityTypeClass::Animal);
        assert_eq!("ANIMAL".parse::<ZTEntityTypeClass>().unwrap(), ZTEntityTypeClass::Animal);
        assert_eq!("aNiMaL".parse::<ZTEntityTypeClass>().unwrap(), ZTEntityTypeClass::Animal);
    }

    #[test]
    fn test_zt_entity_type_class_from_str_unknown_is_err() {
        assert!("not_a_real_entity_type_class".parse::<ZTEntityTypeClass>().is_err());
    }
}
