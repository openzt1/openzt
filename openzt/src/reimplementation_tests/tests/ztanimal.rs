//! Live checks for `ztanimal.rs`. Each comparison runs the real vanilla body (through the detour's trampoline) and the
//! Rust port over the same animal and compares the results. Animals come from two sources: fixtures built by the
//! vanilla constructor over a live animal type, with their state fields set to chosen values, and the animals the live
//! zoo already holds (untouched). Checks that follow the unit's habitat or tile (`isUnhappyWithHabitat`,
//! `isHungryAndFoodless`, `withinPreattackRange`, ...) run on live animals only, because an unplaced fixture has no
//! habitat or tile for the vanilla body to resolve.

use openzt_detour::generated::{
    standalone::OPERATOR_NEW,
    ztanimal::{CONSTRUCTOR as ZTANIMAL_CONSTRUCTOR, DESTRUCTOR_1 as ZTANIMAL_DESTRUCTOR_1},
};

use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::harness::{finish_test, write_success_line},
    util::{get_from_memory, low_byte_bool, save_to_memory},
    ztanimal::{animal_offsets, live_support, live_support::real, ZTAnimal, ANIMAL_SIZE, ANIMAL_TYPE_TAG_RVA, ANIMAL_VTABLE_RVA},
    ztkeeper::unit_offsets,
};

/// `ZTUnit`'s movement mode word (`+0x258`), read by `needsBreath` and `noShadowPart`.
const TERRAIN_MODE_OFFSET: u32 = 0x258;
/// `ZTAnimalType`'s prey-id list (begin and end pointers) and the hunger/happiness/health ranges.
const TYPE_PREY_BEGIN: u32 = 0x2e4;
const TYPE_PREY_END: u32 = 0x2e8;

pub(super) fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// `ZTANIMAL_DETOURS_ENABLED`: every `ZTAnimal` detour reports enabled, so the comparisons below exercise the hooked
/// path rather than silently running vanilla.
pub(crate) fn run_ztanimal_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

fn is_animal_type(entity_type: u32) -> bool {
    let vtable: u32 = get_from_memory(entity_type);
    let check = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
    check(entity_type, base() + ANIMAL_TYPE_TAG_RVA)
}

/// Up to `limit` distinct animal types the live world holds.
pub(super) fn live_animal_types(limit: usize) -> Vec<u32> {
    let world = globals().ztworldmgr();
    let mut types = Vec::new();
    let mut slot = world.entity_type_array_start;
    while slot != world.entity_type_array_end && types.len() < limit {
        let ty: u32 = get_from_memory(slot);
        slot += 4;
        if ty != 0 && is_animal_type(ty) {
            types.push(ty);
        }
    }
    types
}

/// Every animal entity the live world holds.
pub(super) fn live_animals() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let mut animals = Vec::new();
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() {
        let entity: u32 = get_from_memory(slot);
        slot += 4;
        if entity == 0 {
            continue;
        }
        let ty: u32 = get_from_memory(entity + unit_offsets::ENTITY_TYPE);
        if ty != 0 && is_animal_type(ty) {
            animals.push(entity);
        }
    }
    animals
}

pub(super) fn build_animal(entity_type: u32) -> ZTAnimal {
    let memory = unsafe { OPERATOR_NEW.original()(ANIMAL_SIZE) } as u32;
    unsafe { std::ptr::write_bytes(memory as *mut u8, 0, ANIMAL_SIZE as usize) };
    unsafe { ZTANIMAL_CONSTRUCTOR.original()(memory as *const u32, entity_type) };
    ZTAnimal(memory)
}

pub(super) fn destroy_animal(animal: ZTAnimal) {
    unsafe { ZTANIMAL_DESTRUCTOR_1.original()(animal.0 as *const u32, 1) };
}

/// Whether the percentage getters can be compared for `entity_type`: vanilla divides by the range width and faults on
/// a zero width, which the port turns into `0`.
fn ranges_are_nonzero(entity_type: u32) -> bool {
    [(0x22c, 0x230), (0x244, 0x240), (0x238, 0x234)].iter().all(|&(low, high)| get_from_memory::<i32>(entity_type + low) != get_from_memory::<i32>(entity_type + high))
}

/// A small deterministic generator so a failure reproduces.
pub(super) struct Lcg(pub(super) u32);

impl Lcg {
    pub(super) fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }

    fn pick(&mut self, values: &[u32]) -> u32 {
        values[(self.next() as usize) % values.len()]
    }

    fn signed(&mut self, range: i32) -> i32 {
        (self.next() % (2 * range as u32 + 1)) as i32 - range
    }
}

/// Compares the field-only methods on `animal`, recording a failure for each mismatch.
fn compare_field_only(label: &str, animal: ZTAnimal, failures: &mut Vec<String>) {
    let this = animal.0;
    let mut check = |name: &str, real: String, port: String| {
        if real != port {
            failures.push(format!("{label} {name}: real {real}, port {port}"));
        }
    };
    check("isHungry", format!("{}", low_byte_bool(real::is_hungry(this))), format!("{}", animal.is_hungry()));
    check("isUnhappyForReproduction", format!("{}", real::is_unhappy_for_reproduction(this)), format!("{}", animal.is_unhappy_for_reproduction()));
    check("isSickly", format!("{}", low_byte_bool(real::is_sickly(this))), format!("{}", animal.is_sickly()));
    check("isEgg", format!("{}", real::is_egg(this)), format!("{}", animal.is_egg()));
    check("isSelectable", format!("{}", real::is_selectable(this)), format!("{}", animal.is_selectable()));
    check("preysOnMan", format!("{}", real::preys_on_man(this)), format!("{}", animal.preys_on_man()));
    check("getAmbientKey", format!("{}", real::get_ambient_key(this)), format!("{}", animal.get_ambient_key()));
    check("diagonalOK", format!("{}", real::diagonal_ok(this)), format!("{}", animal.diagonal_ok()));
    check("isWalkable", format!("{}", low_byte_bool(real::is_walkable(this))), format!("{}", animal.is_walkable()));
    check("noShadowPart", format!("{}", low_byte_bool(real::no_shadow_part(this))), format!("{}", animal.no_shadow_part()));
    check("needsBreath", format!("{}", low_byte_bool(real::needs_breath(this))), format!("{}", animal.needs_breath()));
    check("getKeepMoving", format!("{}", low_byte_bool(real::get_keep_moving(this))), format!("{}", animal.get_keep_moving()));

    check("getIconZoom", format!("{}", real::get_icon_zoom(this)), format!("{}", animal.get_icon_zoom()));
    let egg_or_boxed = animal.is_egg() != 0 || get_from_memory::<u8>(this + animal_offsets::BOXED) != 0;
    if egg_or_boxed {
        check("getIdleAnim(false)", format!("{}", real::get_idle_anim(this, false)), format!("{}", animal.get_idle_anim(false)));
    }

    let ty: u32 = get_from_memory(this + unit_offsets::ENTITY_TYPE);
    if ranges_are_nonzero(ty) {
        check("getHappyPct", format!("{}", real::get_happy_pct(this)), format!("{}", animal.get_happy_pct()));
        check("getHungerPct", format!("{}", real::get_hunger_pct(this)), format!("{}", animal.get_hunger_pct()));
        check("getHealthPct", format!("{}", real::get_health_pct(this)), format!("{}", animal.get_health_pct()));
    }

    let begin: u32 = get_from_memory(ty + TYPE_PREY_BEGIN);
    let end: u32 = get_from_memory(ty + TYPE_PREY_END);
    let mut ids = vec![0, 1, 0x251f, -1];
    ids.extend((begin..end).step_by(4).take(4).map(get_from_memory::<i32>));
    for id in ids {
        check(&format!("isInPreyList({id:#x})"), format!("{}", low_byte_bool(real::is_in_prey_list(this, id))), format!("{}", animal.is_in_prey_list(id)));
    }
}

/// `ZTANIMAL_LEAF_FIELDS_LIVE`: the field-only predicates and accessors over fixtures with chosen state (every flag
/// and mode combination the bodies test, hunger/happiness/health either side of each threshold) and over the live
/// zoo's own animals. `clearPrey` is compared by the fixture's memory after each body.
pub(crate) fn run_ztanimal_leaf_fields_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_LEAF_FIELDS_LIVE";
    let mut failures: Vec<String> = Vec::new();

    let types = live_animal_types(6);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }

    let vtable_expected = base() + ANIMAL_VTABLE_RVA;
    let mut rng = Lcg(0x5eed_1234);
    let mut cases = 0usize;
    for &ty in &types {
        let animal = build_animal(ty);
        let vtable: u32 = get_from_memory(animal.0);
        if vtable != vtable_expected {
            failures.push(format!("type {ty:#x}: fixture vtable {vtable:#x}, expected {vtable_expected:#x}"));
        }
        let hunger_threshold: i32 = get_from_memory(ty + 0x20c);
        let repro_threshold: i32 = get_from_memory(ty + 0x378);
        let breath_threshold: i32 = get_from_memory(ty + 0x1fc);
        for _ in 0..200 {
            save_to_memory(animal.0 + animal_offsets::HAPPINESS, repro_threshold.wrapping_add(rng.signed(2)));
            save_to_memory(animal.0 + animal_offsets::HUNGER, hunger_threshold.wrapping_add(rng.signed(2)));
            save_to_memory(animal.0 + animal_offsets::HEALTH, rng.signed(400));
            save_to_memory(animal.0 + animal_offsets::BREATH, breath_threshold.wrapping_add(rng.signed(2)));
            save_to_memory(animal.0 + TERRAIN_MODE_OFFSET, rng.pick(&[0, 3, 4, 5]));
            save_to_memory(animal.0 + unit_offsets::GOAL_ID, rng.pick(&[0, 1, 0x72, 5]));
            for offset in [animal_offsets::BOXED, animal_offsets::EGG, animal_offsets::HAS_ESCAPE_INFO, animal_offsets::NOT_SELECTABLE] {
                save_to_memory(animal.0 + offset, rng.pick(&[0, 1]) as u8);
            }
            for offset in [0x39fu32, 0x3a0, 0x3a1, 0x3a5, 0x3a2] {
                save_to_memory(animal.0 + offset, (rng.next() % 8 == 0) as u8);
            }
            save_to_memory(animal.0 + unit_offsets::GOAL_ID, rng.pick(&[0, 1, 0x7a, 0x72, 0x73, 0x7c, 0x85, 0x86, 5]));
            let (real_busy, port_busy) = (low_byte_bool(real::is_doing_something(animal.0)), animal.is_doing_something());
            if real_busy != port_busy {
                failures.push(format!("type {ty:#x} fixture isDoingSomething: real {real_busy}, port {port_busy}"));
            }
            compare_field_only(&format!("type {ty:#x} fixture"), animal, &mut failures);
            cases += 1;
            if failures.len() > 40 {
                break;
            }
        }

        save_to_memory(animal.0 + animal_offsets::PREY_ID, 0x1234u32);
        real::clear_prey(animal.0);
        let after_real: u32 = get_from_memory(animal.0 + animal_offsets::PREY_ID);
        save_to_memory(animal.0 + animal_offsets::PREY_ID, 0x1234u32);
        animal.clear_prey();
        let after_port: u32 = get_from_memory(animal.0 + animal_offsets::PREY_ID);
        if after_real != 0 || after_port != 0 {
            failures.push(format!("type {ty:#x}: clearPrey left real {after_real:#x}, port {after_port:#x}"));
        }

        for _ in 0..20 {
            let words = |a: ZTAnimal| (0..ANIMAL_SIZE / 4).map(|i| get_from_memory::<u32>(a.0 + i * 4)).collect::<Vec<_>>();
            let flags = [rng.next(), rng.next(), rng.next(), 0];
            let seed = |a: ZTAnimal| {
                for (i, offset) in [0x298u32, 0x29c, 0x2a0, 0x2a4].into_iter().enumerate() {
                    save_to_memory(a.0 + offset, flags[i]);
                }
                save_to_memory(a.0 + animal_offsets::HEALTH, 7i32);
                save_to_memory(a.0 + 0x3a7, 1u8);
            };
            seed(animal);
            real::set_max_health(animal.0);
            let real_words = words(animal);
            seed(animal);
            animal.set_max_health();
            let port_words = words(animal);
            if real_words != port_words {
                failures.push(format!("type {ty:#x}: setMaxHealth memory differs between real and port"));
                break;
            }
        }
        destroy_animal(animal);
    }

    let live = live_animals();
    for &entity in &live {
        compare_field_only("live animal", ZTAnimal(entity), &mut failures);
        cases += 1;
    }

    finish_test(&format!("{test_name} ({} type(s), {} live animal(s), {cases} cases)", types.len(), live.len()), failures, failure_log)
}

/// `ZTANIMAL_LEAF_HABITAT_LIVE`: the checks that follow the habitat or the tile, over the live zoo's animals (and
/// every ordered pair of the first few for the preattack checks).
pub(crate) fn run_ztanimal_leaf_habitat_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_LEAF_HABITAT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let live = live_animals();
    if live.is_empty() {
        return finish_test(&format!("{test_name} (no live animals; nothing to compare)"), failures, failure_log);
    }

    for &entity in &live {
        let animal = ZTAnimal(entity);
        let pairs = [
            ("isUnhappyWithHabitat", low_byte_bool(real::is_unhappy_with_habitat(entity)), animal.is_unhappy_with_habitat()),
            ("isVeryUnhappyWithHabitat", low_byte_bool(real::is_very_unhappy_with_habitat(entity)), animal.is_very_unhappy_with_habitat()),
            ("isHungryAndFoodless", low_byte_bool(real::is_hungry_and_foodless(entity)), animal.is_hungry_and_foodless()),
        ];
        for (name, real_value, port_value) in pairs {
            if real_value != port_value {
                failures.push(format!("animal {entity:#x} {name}: real {real_value}, port {port_value}"));
            }
        }
        let numbers = [
            ("fNumAnimals", real::f_num_animals(entity), animal.f_num_animals()),
            ("fNumSameAnimals", real::f_num_same_animals(entity), animal.f_num_same_animals()),
            ("fNumSameAdultAnimals", real::f_num_same_adult_animals(entity), animal.f_num_same_adult_animals()),
            ("fTilesPerAdultAnimal", real::f_tiles_per_adult_animal(entity), animal.f_tiles_per_adult_animal()),
            ("fCompatAnimalsRating", real::f_compat_animals_rating(entity), animal.f_compat_animals_rating()),
            ("getIdleAnim(false)", real::get_idle_anim(entity, false), animal.get_idle_anim(false)),
            ("getIdleAnim(true)", real::get_idle_anim(entity, true), animal.get_idle_anim(true)),
        ];
        for (name, real_value, port_value) in numbers {
            if real_value != port_value {
                failures.push(format!("animal {entity:#x} {name}: real {real_value}, port {port_value}"));
            }
        }
    }

    let sample: Vec<u32> = live.iter().copied().take(24).collect();
    let mut compared = 0usize;
    for &first in &sample {
        for &second in &sample {
            let animal = ZTAnimal(first);
            let preattack = low_byte_bool(real::is_preattack(first, second));
            let range = low_byte_bool(real::within_preattack_range(first, second));
            if preattack != animal.is_preattack(second) {
                failures.push(format!("{first:#x} -> {second:#x} isPreattack: real {preattack}, port {}", animal.is_preattack(second)));
            }
            if range != animal.within_preattack_range(second) {
                failures.push(format!("{first:#x} -> {second:#x} withinPreattackRange: real {range}, port {}", animal.within_preattack_range(second)));
            }
            compared += 1;
        }
        // A null partner: vanilla answers false without touching it.
        if low_byte_bool(real::is_preattack(first, 0)) != ZTAnimal(first).is_preattack(0) || low_byte_bool(real::within_preattack_range(first, 0)) != ZTAnimal(first).within_preattack_range(0) {
            failures.push(format!("{first:#x} null partner disagrees"));
        }
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({} animal(s), {compared} pair(s))", live.len()), failures, failure_log)
}

fn habitat_of(animal: u32) -> u32 {
    crate::ztkeeper::call_slot_0(animal, 0x24c)
}

/// Runs `real_body`, restores the listed words, runs `port_body`, and records a failure when the results or the final
/// words differ. Used for bodies that write animal state.
fn compare_stateful<T: PartialEq + std::fmt::Debug>(
    name: &str,
    words: &[u32],
    real_body: impl FnOnce() -> T,
    port_body: impl FnOnce() -> T,
    failures: &mut Vec<String>,
) {
    let before: Vec<u32> = words.iter().map(|&a| get_from_memory(a)).collect();
    let real_result = real_body();
    let after_real: Vec<u32> = words.iter().map(|&a| get_from_memory(a)).collect();
    for (&a, &v) in words.iter().zip(&before) {
        save_to_memory(a, v);
    }
    let port_result = port_body();
    let after_port: Vec<u32> = words.iter().map(|&a| get_from_memory(a)).collect();
    if real_result != port_result {
        failures.push(format!("{name}: real {real_result:?}, port {port_result:?}"));
    }
    if after_real != after_port {
        failures.push(format!("{name}: state real {after_real:x?}, port {after_port:x?}"));
    }
    for (&a, &v) in words.iter().zip(&before) {
        save_to_memory(a, v);
    }
}

fn vector_contents(vector: u32) -> Vec<u32> {
    let begin: u32 = get_from_memory(vector);
    let end: u32 = get_from_memory(vector + 4);
    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
}

/// `ZTANIMAL_ENTITY_STATE_LIVE`: the methods that resolve other entities, the habitat or the tile (`getSwims`,
/// `getPreyUnit`, `fMateGone`, the census and keeper-food checks, `canCapture`, `canService`, `cliffsOK`, the home
/// habitat family, `getSellPrice`, `fAnimalSpaceRatio`, `getHabitatRating`, ...) over the live zoo's animals. Methods
/// that write state are compared by running the real body, restoring the touched words and running the port.
pub(crate) fn run_ztanimal_entity_state_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_ENTITY_STATE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let live = live_animals();
    if live.is_empty() {
        return finish_test(&format!("{test_name} (no live animals; nothing to compare)"), failures, failure_log);
    }

    for &entity in &live {
        let animal = ZTAnimal(entity);
        let habitat = habitat_of(entity);
        let tile = unsafe { openzt_detour::generated::bfentity::GET_TILE.original()(entity as *const u32) } as u32;
        let mut check = |name: &str, real_value: String, port_value: String| {
            if real_value != port_value {
                failures.push(format!("animal {entity:#x} {name}: real {real_value}, port {port_value}"));
            }
        };
        check("isDoingSomething", format!("{}", low_byte_bool(real::is_doing_something(entity))), format!("{}", animal.is_doing_something()));
        check("getSwims", format!("{}", low_byte_bool(real::get_swims(entity))), format!("{}", low_byte_bool(animal.get_swims())));
        check("getUnderwater", format!("{}", low_byte_bool(real::get_underwater(entity))), format!("{}", low_byte_bool(animal.get_underwater())));
        check("getPreyUnit", format!("{:#x}", real::get_prey_unit(entity)), format!("{:#x}", animal.get_prey_unit() as i32));
        check("fOtherAnimalAngry", format!("{}", low_byte_bool(real::f_other_animal_angry(entity))), format!("{}", animal.f_other_animal_angry()));
        check("fOtherAnimalSick", format!("{}", low_byte_bool(real::f_other_animal_sick(entity))), format!("{}", animal.f_other_animal_sick()));
        check(
            "fAnimalSpaceRatio",
            format!("{:#x}", real::f_animal_space_ratio(entity).to_bits()),
            format!("{:#x}", animal.f_animal_space_ratio().to_bits()),
        );
        check("fFoodUnitsActuallyEaten", format!("{}", real::f_food_units_actually_eaten(entity)), format!("{}", animal.f_food_units_actually_eaten()));
        check("fIsThereKeeperFood", format!("{}", low_byte_bool(real::f_is_there_keeper_food(entity))), format!("{}", animal.f_is_there_keeper_food()));
        check("getIconAnim", format!("{}", real::get_icon_anim(entity)), format!("{}", animal.get_icon_anim()));
        check("getOnlySwims", format!("{}", real::get_only_swims(entity)), format!("{}", animal.get_only_swims()));
        check("getIdleAnimSymbol", format!("{:#x}", real::get_idle_anim_symbol(entity)), format!("{:#x}", animal.get_idle_anim_symbol()));
        let walk = b"bWalk ";
        let name = [walk.as_ptr() as u32, walk.as_ptr() as u32 + 5, walk.as_ptr() as u32 + 6];
        check("getMoveAnim", format!("{:#x}", real::get_move_anim(entity, name.as_ptr() as u32)), format!("{:#x}", animal.get_move_anim(name.as_ptr() as *const i8) as u32));
        check("getSellPrice", format!("{:#x}", real::get_sell_price(entity).to_bits()), format!("{:#x}", animal.get_sell_price().to_bits()));
        check("getHabitatRating", format!("{}", real::get_habitat_rating(entity, habitat)), format!("{}", animal.habitat_rating_in(habitat)));
        check("getHabitatRating(null)", format!("{}", real::get_habitat_rating(entity, 0)), format!("{}", animal.habitat_rating_in(0)));
        check("canSleepBeSickEat", format!("{}", real::can_sleep_be_sick_eat(entity)), format!("{}", animal.can_sleep_be_sick_eat()));
        if tile != 0 {
            for (x, y) in [(0, 0), (33, 5), (5, 33), (63, 63), (64, 0), (-1, 3), (3, 64)] {
                let pos = [x as i32, y as i32, 0];
                check(
                    &format!("isNeedSatisfier({x},{y})"),
                    format!("{}", low_byte_bool(real::is_need_satisfier(entity, tile, pos.as_ptr() as u32))),
                    format!("{}", ZTAnimal::is_need_satisfier(tile, pos.as_ptr() as u32)),
                );
            }
            for direction in 0..8u32 {
                check(&format!("cliffsOK({direction})"), format!("{}", real::cliffs_ok(entity, tile, direction)), format!("{}", animal.cliffs_ok(tile, direction)));
            }
        }
        check("isNeedSatisfier(null)", format!("{}", real::is_need_satisfier(entity, 0, 0) & 0xff), format!("{}", ZTAnimal::is_need_satisfier(0, 0) as u8));

        for flag in [0u32, 1] {
            let real_out = [0u32; 3];
            let port_out = [0u32; 3];
            real::get_undesirable_scenery_in_habitat(entity, real_out.as_ptr() as u32, flag);
            animal.get_undesirable_scenery_in_habitat(port_out.as_ptr() as u32, flag);
            let (real_items, port_items) = (vector_contents(real_out.as_ptr() as u32), vector_contents(port_out.as_ptr() as u32));
            if real_items != port_items || (real_out[0] == 0) != (port_out[0] == 0) || real_out[2].wrapping_sub(real_out[0]) != port_out[2].wrapping_sub(port_out[0]) {
                failures.push(format!("animal {entity:#x} getUndesirableSceneryInHabitat({flag}): real {real_items:x?}, port {port_items:x?}"));
            }
            for out in [real_out, port_out] {
                if out[0] != 0 {
                    unsafe { openzt_detour::generated::poolalloc::DEALLOCATE.original()(out[0] as *const u32, out[2] - out[0]) };
                }
            }
        }

        let mate = get_from_memory::<u32>(entity + animal_offsets::MATE_ID);
        let mut mate_words = vec![entity + animal_offsets::MATE_ID];
        if mate != 0 {
            for &other in &live {
                if get_from_memory::<u32>(other + unit_offsets::ENTITY_ID) == mate {
                    mate_words.push(other + animal_offsets::MATE_ID);
                }
            }
        }
        compare_stateful(&format!("animal {entity:#x} fMateGone"), &mate_words, || low_byte_bool(real::f_mate_gone(entity)), || animal.f_mate_gone(), &mut failures);
        compare_stateful(
            &format!("animal {entity:#x} calcHabitat"),
            &[entity + animal_offsets::CACHED_HABITAT],
            || {
                real::calc_habitat(entity);
                real::get_habitat(entity) as u32
            },
            || {
                animal.calc_habitat();
                animal.get_habitat() as u32
            },
            &mut failures,
        );
        let home = [entity + animal_offsets::HOME_GATE_TILE];
        compare_stateful(&format!("animal {entity:#x} setHomeHabitat"), &home, || real::set_home_habitat(entity, habitat), || animal.set_home_habitat(habitat), &mut failures);
        compare_stateful(&format!("animal {entity:#x} setHomeHabitat(null)"), &home, || real::set_home_habitat(entity, 0), || animal.set_home_habitat(0), &mut failures);
        compare_stateful(&format!("animal {entity:#x} recalcHomeHabitat"), &home, || real::recalc_home_habitat(entity), || animal.recalc_home_habitat(), &mut failures);
        compare_stateful(&format!("animal {entity:#x} getHomeHabitat"), &home, || real::get_home_habitat(entity), || animal.get_home_habitat(), &mut failures);
        if failures.len() > 40 {
            break;
        }
    }

    let sample: Vec<u32> = live.iter().copied().take(24).collect();
    for &first in &sample {
        let animal = ZTAnimal(first);
        for &second in &sample {
            let pairs = [
                ("canCapture", low_byte_bool(real::can_capture(first, second)), animal.can_capture(second)),
                ("canService", low_byte_bool(real::can_service(first, second)), animal.can_service(second)),
                ("isWalkableBy", low_byte_bool(real::is_walkable_by(first, second)), animal.is_walkable_by(second)),
                ("compareLT", real::compare_lt(first, second), animal.compare_lt(second)),
            ];
            for (name, real_value, port_value) in pairs {
                if real_value != port_value {
                    failures.push(format!("{first:#x} -> {second:#x} {name}: real {real_value}, port {port_value}"));
                }
            }
        }
        for (name, real_value, port_value) in [
            ("canCapture(null)", low_byte_bool(real::can_capture(first, 0)), animal.can_capture(0)),
            ("canService(null)", low_byte_bool(real::can_service(first, 0)), animal.can_service(0)),
        ] {
            if real_value != port_value {
                failures.push(format!("{first:#x} {name}: real {real_value}, port {port_value}"));
            }
        }
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({} animal(s))", live.len()), failures, failure_log)
}

/// `ZTANIMAL_CHECK_DONE_LIVE`: `checkDone` on fixtures heading for an escape tile it has not reached and one it has
/// (the record is freed and the flag cleared), plus fixtures that fall through to `BFUnit::checkDone`.
pub(crate) fn run_ztanimal_check_done_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_CHECK_DONE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(1);
    let Some(&ty) = types.first() else {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let fixture_tile = |animal: ZTAnimal| unsafe { openzt_detour::generated::bfentity::GET_TILE.original()(animal.0 as *const u32) } as u32;
    for (goal, has_escape, reach) in [(1i32, 1u8, false), (1, 1, true), (1, 0, false), (5, 1, false), (5, 0, false)] {
        let mut outcomes = Vec::new();
        for use_real in [true, false] {
            let animal = build_animal(ty);
            save_to_memory(animal.0 + unit_offsets::GOAL_ID, goal);
            save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, has_escape);
            let record = unsafe { OPERATOR_NEW.original()(0x10) } as u32;
            unsafe { std::ptr::write_bytes(record as *mut u8, 0, 0x10) };
            save_to_memory(record + 8, if reach { fixture_tile(animal) } else { 0xdead_0000u32 });
            if has_escape != 0 {
                save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record);
            }
            let result = if use_real { real::check_done(animal.0) } else { animal.check_done() };
            outcomes.push((
                result & 0xff,
                get_from_memory::<u8>(animal.0 + animal_offsets::HAS_ESCAPE_INFO),
                get_from_memory::<u32>(animal.0 + animal_offsets::ESCAPE_RECORD) == 0,
            ));
            destroy_animal(animal);
        }
        if outcomes[0] != outcomes[1] {
            failures.push(format!("goal {goal} escape {has_escape} reach {reach}: real {:?}, port {:?}", outcomes[0], outcomes[1]));
        }
    }
    finish_test(test_name, failures, failure_log)
}
