//! Live checks of `ZTBuilding` (`ztbuilding.rs`): the layout and the type's slot records, and each ported method against
//! the vanilla body, on the buildings of the loaded zoo and on fixtures built by the real constructor.

use std::{
    cell::RefCell,
    mem::{offset_of, size_of},
};

use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType, ZTBuildingUpgrade},
    globals::globals,
    reimplementation_tests::harness::{finish_test, write_success_line},
    util::{get_from_memory, save_to_memory},
    ztbuilding::{
        building_offsets,
        live_support::{self, build_standalone_building, destroy_standalone_building, lifecycle_real, test_real},
        ZTBuilding, ZTBuildingSlot,
    },
    ztmegatilemgr::entity_type_matches,
};

const BUILDING_TAG_RVA: u32 = 0x0023_8680;

fn word(address: u32) -> u32 {
    get_from_memory(address)
}

fn vector_len(address: u32, element: u32) -> u32 {
    word(address + 4).wrapping_sub(word(address)) / element
}

/// The slot records of a building type, as their addresses.
fn slot_records(building_type: u32) -> Vec<u32> {
    let vector = building_type + offset_of!(ZTBuildingType, slots) as u32;
    let begin = word(vector);
    (0..vector_len(vector, size_of::<ZTBuildingSlotType>() as u32)).map(|index| begin + index * size_of::<ZTBuildingSlotType>() as u32).collect()
}

/// Every building slot must name itself, its owner and the draw-slot count its type record gives.
fn check_slots(label: &str, building: ZTBuilding, failures: &mut Vec<String>) {
    let records = slot_records(building.entity_type());
    if building.slot_count() != records.len() as i32 {
        failures.push(format!("{label}: {} slots, type has {} records", building.slot_count(), records.len()));
        return;
    }
    let begin = word(building.0 + building_offsets::SLOTS);
    for (index, record) in records.into_iter().enumerate() {
        let slot = begin + (index * size_of::<ZTBuildingSlot>()) as u32;
        let has_draw: u8 = get_from_memory(record + offset_of!(ZTBuildingSlotType, has_draw_slots) as u32);
        let capacity = word(record + offset_of!(ZTBuildingSlotType, capacity) as u32);
        let draw_count = if has_draw != 0 { capacity } else { 0 };
        let found = (
            word(slot + offset_of!(ZTBuildingSlot, index) as u32),
            word(slot + offset_of!(ZTBuildingSlot, building) as u32),
            word(slot + offset_of!(ZTBuildingSlot, draw_slot_count) as u32),
            get_from_memory::<u8>(slot + offset_of!(ZTBuildingSlot, has_draw_slots) as u32),
            vector_len(slot + offset_of!(ZTBuildingSlot, draw_users) as u32, 4),
        );
        let expected = (index as u32, building.0, draw_count, u8::from(draw_count != 0), draw_count);
        if found != expected {
            failures.push(format!("{label} slot {index}: (index, owner, draw count, flag, draw users) {found:x?}, expected {expected:x?}"));
        }
    }
}

/// A fixture from the real constructor starts with empty containers; `initStatusVars` then builds one slot per record and
/// takes the type's default price.
fn check_fixture(building_type: u32, failures: &mut Vec<String>) {
    let building = build_standalone_building(building_type);
    let label = format!("fixture of type {building_type:#x}");
    for (name, offset) in [("members", building_offsets::MEMBERS), ("users", building_offsets::USERS), ("slots", building_offsets::SLOTS)] {
        if word(building.0 + offset) != word(building.0 + offset + 4) {
            failures.push(format!("{label}: {name} not empty after construction"));
        }
    }
    if building.entity_type() != building_type {
        failures.push(format!("{label}: entity type {:#x}", building.entity_type()));
    }
    if word(building.0 + building_offsets::CONSTRUCTION_DATE) == 0 && word(building.0 + building_offsets::CONSTRUCTION_DATE + 4) == 0 {
        failures.push(format!("{label}: construction date not set"));
    }
    if building.show_id() != 0 || word(building.0 + building_offsets::UPGRADE_INDEX) != 0 || word(building.0 + building_offsets::WATER_CAPACITY) != 0 {
        failures.push(format!("{label}: show id, upgrade index or water capacity not zero after construction"));
    }

    lifecycle_real::init_status_vars(building.0);
    check_slots(&label, building, failures);
    let default_cost = word(building_type + offset_of!(ZTBuildingType, default_cost) as u32);
    if building.price().to_bits() != default_cost {
        failures.push(format!("{label}: price {:#x}, type default cost {default_cost:#x}", building.price().to_bits()));
    }
    if building.occupancy() != 0 || building.type_state() != 0 {
        failures.push(format!("{label}: occupancy {} and type state {} after initStatusVars", building.occupancy(), building.type_state()));
    }
    destroy_standalone_building(building);
}

pub(crate) fn run_ztbuilding_layout_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures = Vec::new();
    let world = globals().ztworldmgr();
    let mut buildings = 0;
    let mut types: Vec<u32> = Vec::new();
    let mut entry = world.entity_array_start();
    while entry != world.entity_array_end() {
        let entity = word(entry);
        entry += 4;
        if entity == 0 || !unsafe { entity_type_matches(entity, BUILDING_TAG_RVA) } {
            continue;
        }
        let building = ZTBuilding(entity);
        buildings += 1;
        if !types.contains(&building.entity_type()) {
            types.push(building.entity_type());
        }
        check_slots(&format!("building {entity:#x}"), building, &mut failures);
    }
    let with_slots = types.iter().filter(|&&building_type| !slot_records(building_type).is_empty()).count();
    if with_slots == 0 {
        failures.push(format!("no building type with slots among {buildings} buildings"));
    }
    for &building_type in &types {
        check_fixture(building_type, &mut failures);
    }
    finish_test("ZTBUILDING_LAYOUT_LIVE", failures, failure_log)
}

/// `ZTBUILDING_DETOURS_ENABLED`: every `ZTBuilding` detour reports enabled, so the comparisons below exercise the hooked
/// path rather than silently running vanilla.
pub(crate) fn run_ztbuilding_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

fn check<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: &str, real: T, port: T) {
    if real != port {
        failures.push(format!("{label}: real {real:x?}, port {port:x?}"));
    }
}

/// The buildings of the loaded zoo.
fn live_buildings() -> Vec<ZTBuilding> {
    let world = globals().ztworldmgr();
    (world.entity_array_start()..world.entity_array_end())
        .step_by(4)
        .map(word)
        .filter(|&entity| entity != 0 && unsafe { entity_type_matches(entity, BUILDING_TAG_RVA) })
        .map(ZTBuilding)
        .collect()
}

/// The distinct types of the live buildings.
fn live_building_types() -> Vec<u32> {
    let mut types: Vec<u32> = Vec::new();
    for building in live_buildings() {
        if !types.contains(&building.entity_type()) {
            types.push(building.entity_type());
        }
    }
    types
}

/// The read-only methods, real against port.
fn compare_getters(label: &str, building: ZTBuilding, failures: &mut Vec<String>) {
    let this = building.0;
    check(failures, &format!("{label} getTotalUsers"), test_real::total_users(this), building.total_users());
    check(failures, &format!("{label} getLastMonthUsers"), test_real::last_month_users(this), building.last_month_users());
    check(failures, &format!("{label} getThisMonthUsers"), test_real::this_month_users(this), building.this_month_users());
    check(failures, &format!("{label} getCurrentUpgrade"), test_real::current_upgrade(this), building.current_upgrade());
    check(failures, &format!("{label} getUpkeep"), test_real::upkeep(this).to_bits(), building.upkeep().to_bits());
    check(failures, &format!("{label} getAdultChange"), test_real::adult_change(this), building.adult_change());
    check(failures, &format!("{label} getChildChange"), test_real::child_change(this), building.child_change());
}

/// A leaked upgrade list of `count` records: the sentinel, with upkeep, adult and child change distinct per record.
fn leaked_upgrades(count: u32) -> u32 {
    let nodes: Vec<*mut [u32; 10]> = (0..=count).map(|_| Box::into_raw(Box::new([0u32; 10]))).collect();
    let address = |index: usize| nodes[index % nodes.len()] as u32;
    for index in 0..nodes.len() {
        save_to_memory(address(index), address(index + 1));
        save_to_memory(address(index) + 4, address(index + nodes.len() - 1));
    }
    for record in 1..=count {
        let data = address(record as usize) + 8;
        save_to_memory(data + offset_of!(ZTBuildingUpgrade, upkeep) as u32, 100 * record as i32 + 7);
        save_to_memory(data + offset_of!(ZTBuildingUpgrade, adult_change) as u32, 10 * record as i32 + 1);
        save_to_memory(data + offset_of!(ZTBuildingUpgrade, child_change) as u32, -(10 * record as i32 + 2));
    }
    address(0)
}

/// `ZTBUILDING_UPGRADES_LIVE`: the getters over every live building, then over fixtures of each type with the type's
/// need changes made distinct and its upgrade list replaced by a scripted one, through `useUpgrade`.
pub(crate) fn run_ztbuilding_upgrades_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures = Vec::new();
    for building in live_buildings() {
        compare_getters(&format!("building {:#x}", building.0), building, &mut failures);
    }
    let types = live_building_types();
    if types.is_empty() {
        failures.push("no live building".to_string());
    }
    let upgrades = leaked_upgrades(3);
    for &building_type in &types {
        let adult = building_type + offset_of!(ZTBuildingType, adult_change) as u32;
        let child = building_type + offset_of!(ZTBuildingType, child_change) as u32;
        let list = building_type + offset_of!(ZTBuildingType, upgrades) as u32;
        let saved = (word(adult), word(child), word(list));
        let real = build_standalone_building(building_type);
        let port = build_standalone_building(building_type);
        save_to_memory(adult, 21u32);
        save_to_memory(child, 34u32);
        for (name, sentinel) in [("own upgrades", saved.2), ("scripted upgrades", upgrades)] {
            save_to_memory(list, sentinel);
            for index in [0u32, 1, 2, 3, 4, 0x7fff_ffff, 0xffff_ffff] {
                let label = format!("type {building_type:#x} {name} useUpgrade({index:#x})");
                for building in [real, port] {
                    save_to_memory(building.0 + building_offsets::UPGRADE_INDEX, 0u32);
                }
                check(&mut failures, &label, test_real::use_upgrade(real.0, index), port.use_upgrade(index));
                check(&mut failures, &format!("{label} index"), word(real.0 + building_offsets::UPGRADE_INDEX), word(port.0 + building_offsets::UPGRADE_INDEX));
                compare_getters(&label, port, &mut failures);
                // An index past the end that `useUpgrade` would refuse, as a stale save could hold.
                save_to_memory(port.0 + building_offsets::UPGRADE_INDEX, index);
                compare_getters(&format!("{label} forced"), port, &mut failures);
            }
        }
        save_to_memory(adult, saved.0);
        save_to_memory(child, saved.1);
        save_to_memory(list, saved.2);
        destroy_standalone_building(real);
        destroy_standalone_building(port);
    }
    finish_test("ZTBUILDING_UPGRADES_LIVE", failures, failure_log)
}

fn tail(building: ZTBuilding) -> Vec<u32> {
    // The two sound pointers are each fixture's own allocations.
    let sounds = [building_offsets::USE_SOUND, building_offsets::USE_SOUND + 4];
    (building_offsets::CONSTRUCTION_DATE..crate::ztbuilding::BUILDING_SIZE)
        .step_by(4)
        .filter(|offset| !sounds.contains(offset))
        .map(|offset| word(building.0 + offset))
        .collect()
}

/// `ZTBUILDING_MONTHLY_LIVE`: `receiveIncome`, `enterNewMonth` and `resetConstructionDate` on twin fixtures, comparing
/// every word from the construction date on.
pub(crate) fn run_ztbuilding_monthly_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures = Vec::new();
    for building_type in live_building_types() {
        let real = build_standalone_building(building_type);
        let port = build_standalone_building(building_type);
        for (price, total, month) in [(0.0f32, 0.0f32, 0.0f32), (3.35, 16_777_216.0, 0.1), (-2.5, 1234.567, 9.875), (1.0e9, 0.3, 1.0e-3)] {
            for building in [real, port] {
                save_to_memory(building.0 + building_offsets::PRICE, price);
                save_to_memory(building.0 + building_offsets::TOTAL_INCOME, total);
                save_to_memory(building.0 + building_offsets::LAST_MONTH_INCOME, 5.0f32);
                save_to_memory(building.0 + building_offsets::THIS_MONTH_INCOME, month);
                save_to_memory(building.0 + building_offsets::TOTAL_USERS, 11i32);
                save_to_memory(building.0 + building_offsets::LAST_MONTH_USERS, 22i32);
                save_to_memory(building.0 + building_offsets::THIS_MONTH_USERS, 33i32);
                save_to_memory(building.0 + building_offsets::TOTAL_UPKEEP, total);
                save_to_memory(building.0 + building_offsets::CONSTRUCTION_DATE, 0u32);
                save_to_memory(building.0 + building_offsets::CONSTRUCTION_DATE + 4, 0u32);
            }
            let label = format!("type {building_type:#x} price {price} total {total}");
            test_real::receive_income(real.0);
            port.receive_income();
            check(&mut failures, &format!("{label} receiveIncome"), tail(real), tail(port));
            test_real::enter_new_month(real.0);
            port.enter_new_month();
            check(&mut failures, &format!("{label} enterNewMonth"), tail(real), tail(port));
            test_real::reset_construction_date(real.0);
            port.reset_construction_date();
            check(&mut failures, &format!("{label} resetConstructionDate"), tail(real), tail(port));
        }
        destroy_standalone_building(real);
        destroy_standalone_building(port);
    }
    finish_test("ZTBUILDING_MONTHLY_LIVE", failures, failure_log)
}

/// A stand-in unit: `words[0]` is its vtable and the entity id is at `+0x124`.
fn fake_unit(id: u32, vtable: u32) -> Vec<u32> {
    let mut words = vec![0u32; 0x130 / 4];
    words[0] = vtable;
    words[0x124 / 4] = id;
    words
}

/// The member vector as `(ids, capacity in elements)`.
fn members(building: ZTBuilding) -> (Vec<u32>, u32) {
    let vector = building.0 + building_offsets::MEMBERS;
    let (begin, end, capacity) = (word(vector), word(vector + 4), word(vector + 8));
    ((begin..end).step_by(4).map(word).collect(), (capacity - begin) / 4)
}

/// `ZTBUILDING_MEMBERS_LIVE`: `addMember` (past the pool's largest small block) and `removeMember` on twin fixtures, and
/// `checkUser` over a scripted user vector.
pub(crate) fn run_ztbuilding_members_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures = Vec::new();
    let Some(&building_type) = live_building_types().first() else {
        return finish_test("ZTBUILDING_MEMBERS_LIVE", vec!["no live building".to_string()], failure_log);
    };
    let real = build_standalone_building(building_type);
    let port = build_standalone_building(building_type);
    let units: Vec<Vec<u32>> = (0..70).map(|index| fake_unit(1000 + index % 66, 0)).collect();
    let unit = |index: usize| units[index].as_ptr() as u32;

    test_real::remove_member(real.0, unit(0));
    port.remove_member(unit(0));
    test_real::add_member(real.0, 0);
    port.add_member(0);
    check(&mut failures, "empty", members(real), members(port));
    for index in 0..units.len() {
        test_real::add_member(real.0, unit(index));
        port.add_member(unit(index));
        check(&mut failures, &format!("addMember #{index}"), members(real), members(port));
    }
    // A duplicated id, the first, a middle, the last, an absent one and none.
    let absent = fake_unit(5, 0);
    for target in [unit(2), unit(0), unit(40), unit(65), unit(65), absent.as_ptr() as u32, 0] {
        test_real::remove_member(real.0, target);
        port.remove_member(target);
        check(&mut failures, "removeMember", members(real), members(port));
    }

    let users: [u32; 6] = [1001, 0, 1002, 1, 1003, 0];
    let start = users.as_ptr() as u32;
    let mut found = 0;
    for count in 0..=3u32 {
        let vector = port.0 + building_offsets::USERS;
        save_to_memory(vector, start);
        save_to_memory(vector + 4, start + count * 8);
        for index in 0..6 {
            let result = port.check_user(unit(index));
            found += u32::from(result);
            check(&mut failures, &format!("checkUser with {count} users, unit {index}"), test_real::check_user(port.0, unit(index)), result);
        }
        // The buffer is Rust's, so the destructor must not see it.
        save_to_memory(vector, 0u32);
        save_to_memory(vector + 4, 0u32);
    }
    if found != 6 {
        failures.push(format!("checkUser found {found} users, expected 6"));
    }
    destroy_standalone_building(real);
    destroy_standalone_building(port);
    finish_test("ZTBUILDING_MEMBERS_LIVE", failures, failure_log)
}

thread_local! {
    static CALLS: RefCell<Vec<Vec<u32>>> = const { RefCell::new(Vec::new()) };
}

fn record(call: Vec<u32>) {
    CALLS.with(|calls| calls.borrow_mut().push(call));
}

fn take_calls() -> Vec<Vec<u32>> {
    CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

extern "thiscall" fn add_changes_stub(this: u32, changes: *const [i32; 6]) {
    let changes = unsafe { *changes };
    record(std::iter::once(this).chain(changes.into_iter().map(|change| change as u32)).collect());
}

extern "thiscall" fn get_events_stub(this: u32, events: u32, id: u32, tag: u32) {
    record(vec![this, events, id, tag]);
}

#[allow(clippy::too_many_arguments)]
extern "thiscall" fn listener_send_stub(this: u32, a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32) {
    // Argument slots narrower than a dword hold whatever upper bytes the caller pushed.
    record(vec![this, a & 0xffff, b, c & 0xff, d, e & 0xff, f, g & 0xffff, h & 0xffff]);
}

/// `ZTBUILDING_EVENTS_LIVE`: what `doChanges`, `getEvents` and `sendEvent` hand on, through a stand-in unit, a swapped
/// building vtable and a swapped `ZTAIMgr` listener vtable.
pub(crate) fn run_ztbuilding_events_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_EVENTS_LIVE";
    let mut failures = Vec::new();
    let ai_mgr = globals().ztaimgr_ptr() as u32;
    let types = live_building_types();
    if ai_mgr == 0 || types.is_empty() {
        return finish_test(test_name, vec!["no live building or no ZTAIMgr".to_string()], failure_log);
    }

    let mut unit_vtable = vec![0u32; 0x200 / 4];
    unit_vtable[0x1fc / 4] = add_changes_stub as *const () as u32;
    let unit = fake_unit(77, unit_vtable.as_ptr() as u32);
    let unit_ptr = unit.as_ptr() as u32;

    for &building_type in &types {
        let building = build_standalone_building(building_type);
        save_to_memory(building.0 + 0x124, 0x4242u32);
        for target in [unit_ptr, 0] {
            test_real::do_changes(building.0, target);
            let real = take_calls();
            building.do_changes(target);
            let port = take_calls();
            if real.len() != usize::from(target != 0) {
                failures.push(format!("type {building_type:#x} doChanges({target:#x}): real made {} calls", real.len()));
            }
            check(&mut failures, &format!("type {building_type:#x} doChanges({target:#x})"), real, port);
        }
        destroy_standalone_building(building);
    }

    let building = build_standalone_building(types[0]);
    save_to_memory(building.0 + 0x124, 0x4242u32);

    let real_vtable = word(building.0);
    let mut vtable: Vec<u32> = (0..78).map(|slot| word(real_vtable + slot * 4)).collect();
    vtable[0xc / 4] = get_events_stub as *const () as u32;
    save_to_memory(building.0, vtable.as_ptr() as u32);
    test_real::get_events(building.0, 0xfeed_0000);
    let real = take_calls();
    building.get_events(0xfeed_0000);
    let port = take_calls();
    save_to_memory(building.0, real_vtable);
    check(&mut failures, "getEvents recorded", vec![vec![building.0, 0xfeed_0000, 0x4242, 0x42]], real.clone());
    check(&mut failures, "getEvents", real, port);

    let listener = ai_mgr + 0x8;
    let real_listener = word(listener);
    let mut listener_vtable: Vec<u32> = (0..32).map(|slot| word(real_listener + slot * 4)).collect();
    listener_vtable[1] = listener_send_stub as *const () as u32;
    let mut sent = Vec::new();
    save_to_memory(listener, listener_vtable.as_ptr() as u32);
    for (kind, target, flag, value, extra, count) in [(0x2730u16, 0u32, 0x4du8, 0x1234_5678u32, 5u16, 1u16), (0xffff, 0xffff_ffff, 0xff, 0xdead_beef, 0xffff, 0xffff)] {
        test_real::send_event(building.0, kind, target, flag, value, extra, count);
        let real = take_calls();
        building.send_event(kind, target, flag, value, extra, count);
        sent.push((kind, real, take_calls()));
    }
    save_to_memory(listener, real_listener);
    for (kind, real, port) in sent {
        if real.len() != 1 {
            failures.push(format!("sendEvent {kind:#x}: real made {} calls", real.len()));
        }
        check(&mut failures, &format!("sendEvent {kind:#x}"), real, port);
    }
    destroy_standalone_building(building);
    finish_test(test_name, failures, failure_log)
}
