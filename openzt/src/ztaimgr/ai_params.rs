//! `initAIParams` for both classes, `BFAIMgr::init`/`load` and the listener overrides.

use super::*;

/// The six `[AI]` parameters `ZTAIMgr::initAIParams` loads, in field order: guest x/y, staff x/y, animal reset
/// interval, guest reset interval. `read(key, default)` reads one integer. Extents are at least `1`, intervals
/// at least `0`.
pub fn compute_ai_params(mut read: impl FnMut(&str, i32) -> i32) -> [i32; 6] {
    let guest_x = read("mRandomXGuest", RANDOM_WALK_DEFAULT);
    let guest_y = read("mRandomYGuest", RANDOM_WALK_DEFAULT);
    let staff_x = read("mRandomXStaff", RANDOM_WALK_DEFAULT);
    let staff_y = read("mRandomYStaff", RANDOM_WALK_DEFAULT);
    let animal_reset = read("AnimalResetInterval", 0);
    let guest_reset = read("GuestResetInterval", 0);
    [guest_x.max(1), guest_y.max(1), staff_x.max(1), staff_y.max(1), animal_reset.max(0), guest_reset.max(0)]
}

/// `ZTAIMgr::initAIParams()`: loads the six parameters from the `[AI]` section of the game configuration.
/// Returns `0` like vanilla (the low byte is cleared).
pub unsafe fn ztaimgr_init_ai_params(this: u32) -> u32 {
    let section = VanillaString::new(AI_INI_SECTION);
    let values = compute_ai_params(|key, default| {
        let key = VanillaString::new(key);
        unsafe { BFINIFILE_READ.original()(section.as_ptr(), key.as_ptr(), default as u32) as i32 }
    });
    let offsets = [OFFSET_GUEST_RANDOM_X, OFFSET_GUEST_RANDOM_Y, OFFSET_STAFF_RANDOM_X, OFFSET_STAFF_RANDOM_Y, OFFSET_ANIMAL_RESET_INTERVAL, OFFSET_GUEST_RESET_INTERVAL];
    for (offset, value) in offsets.into_iter().zip(values) {
        save_to_memory(this + offset, value);
    }
    0
}

/// The `[AI]` parameters `BFAIMgr::initAIParams` loads. The first four fill the random-walk extents
/// (`base_*` is the extra added to the roll, `random_*` the roll's range); the `show_*`/`ai_info_nth` flags drive
/// the debug overlay; `clear_unreachable_freq` and `min_unreachable_count` go to two globals rather than the
/// manager.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BfAiParams {
    pub base_x: i32,
    pub base_y: i32,
    pub random_x: i32,
    pub random_y: i32,
    pub how_often: i32,
    pub tick: i32,
    pub status_period: i32,
    pub goal_period: i32,
    pub show_ai_info: i32,
    pub show_name: i32,
    pub show_position: i32,
    pub show_status_vars: i32,
    pub show_function_call: i32,
    pub show_events: i32,
    pub ai_info_nth: i32,
    pub show_selected: i32,
    pub show_frame: i32,
    pub show_goal: i32,
    pub variable_unit_speeds: i32,
    pub allow_diagonal_moves: bool,
    pub clear_unreachable_freq: i32,
    pub min_unreachable_count: u8,
}

/// Reads every `BFAIMgr::initAIParams` key in vanilla's order: `read(key, default)` reads one integer. The
/// two base extents are clamped to at least `0` and the two random extents to at least `1`;
/// `AllowDiagonalMoves` is true only for the value `1`, and `MinUnreachableCount` keeps its low byte.
pub fn compute_bf_ai_params(mut read: impl FnMut(&str, i32) -> i32) -> BfAiParams {
    let base_x = read("BaseX", 1);
    let base_y = read("BaseY", 1);
    let random_x = read("RandomX", 5);
    let random_y = read("RandomY", 5);
    BfAiParams {
        base_x: base_x.max(0),
        base_y: base_y.max(0),
        random_x: random_x.max(1),
        random_y: random_y.max(1),
        how_often: read("mAIHowOften", 1),
        tick: read("mAITick", 1000),
        status_period: read("mStatusPeriod", 1000),
        goal_period: read("mGoalPeriod", 0),
        show_ai_info: read("ShowAIInfo", 0),
        show_name: read("ShowName", 1),
        show_position: read("ShowPosition", 1),
        show_status_vars: read("ShowStatusVars", 1),
        show_function_call: read("ShowFunctionCall", 1),
        show_events: read("ShowEvents", 1),
        ai_info_nth: read("AIInfoNth", 100),
        show_selected: read("ShowSelected", 0),
        show_frame: read("ShowFrame", 0),
        show_goal: read("ShowGoal", 0),
        variable_unit_speeds: read("VariableUnitSpeeds", 1),
        allow_diagonal_moves: read("AllowDiagonalMoves", 1) == 1,
        clear_unreachable_freq: read("ClearUnreachableFreq", 5),
        min_unreachable_count: read("MinUnreachableCount", 5) as u8,
    }
}

/// Globals `initAIParams` writes: the clear-unreachable frequency (a dword) and the minimum unreachable count (a
/// byte).
pub(super) const RVA_CLEAR_UNREACHABLE_FREQ: u32 = 0x0023_9028;
pub(super) const RVA_MIN_UNREACHABLE_COUNT: u32 = 0x0023_9532;
/// `BFAIMgr` fields `initAIParams` fills.
type AiParamAccessor = fn(&BfAiParams) -> i32;

pub(super) const BF_AI_PARAM_OFFSETS: [(u32, AiParamAccessor); 19] = [
    (0xd4, |p| p.base_x),
    (0xd8, |p| p.base_y),
    (0xdc, |p| p.random_x),
    (0xe0, |p| p.random_y),
    (0xf0, |p| p.how_often),
    (0xe4, |p| p.tick),
    (0x124, |p| p.status_period),
    (0x128, |p| p.goal_period),
    (0xf4, |p| p.show_ai_info),
    (0xf8, |p| p.show_name),
    (0xfc, |p| p.show_position),
    (0x100, |p| p.show_status_vars),
    (0x108, |p| p.show_function_call),
    (0x10c, |p| p.show_events),
    (0x110, |p| p.ai_info_nth),
    (0x104, |p| p.show_selected),
    (0x114, |p| p.show_frame),
    (0x118, |p| p.show_goal),
    (0x11c, |p| p.variable_unit_speeds),
];
pub(super) const OFFSET_ALLOW_DIAGONAL_MOVES: u32 = 0x120;

/// `BFAIMgr::initAIParams()` (`0x005267ec`): loads the `[AI]` parameters (see [`compute_bf_ai_params`]) into the
/// manager and the two globals. Vanilla writes each field's default before reading it, so a failed read keeps
/// the default; the result is the same. Returns `1`.
pub unsafe fn bfaimgr_init_ai_params(this: u32) -> u32 {
    let section = VanillaString::new(AI_INI_SECTION);
    let params = compute_bf_ai_params(|key, default| {
        let key = VanillaString::new(key);
        unsafe { BFINIFILE_READ.original()(section.as_ptr(), key.as_ptr(), default as u32) as i32 }
    });
    for (offset, field) in BF_AI_PARAM_OFFSETS {
        save_to_memory(this + offset, field(&params));
    }
    save_to_memory(this + OFFSET_ALLOW_DIAGONAL_MOVES, u8::from(params.allow_diagonal_moves));
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(base + RVA_CLEAR_UNREACHABLE_FREQ, params.clear_unreachable_freq);
    save_to_memory(base + RVA_MIN_UNREACHABLE_COUNT, params.min_unreachable_count);
    1
}

/// The game's `rand` state: one dword.
pub(super) const RVA_RAND_STATE: u32 = 0x0023_8060;
/// `BFAIMgr` vtable slot `initAIParams`.
pub(super) const BFAIMGR_VTABLE_INIT_AI_PARAMS: u32 = 0x38;

/// `BFAIMgr::init()` (`0x005267b5`): zeroes the tick remainder and counter, calls the manager's virtual
/// `initAIParams` (during construction that is still the `BFAIMgr` one, so it must go through the vtable) and
/// seeds the game's `rand` state from the game's own `time(NULL)`. Returns the virtual's low byte.
pub unsafe fn bfaimgr_init(this: u32) -> bool {
    save_to_memory(this + OFFSET_AI_TICK_REMAINDER, 0u32);
    save_to_memory(this + OFFSET_AI_TICK_COUNT, 0u32);
    let result = unsafe { call_virtual_args(this, BFAIMGR_VTABLE_INIT_AI_PARAMS, []) };
    let seconds = unsafe { TIME.original()(std::ptr::null()) } as u32;
    save_to_memory(get_module_base("zoo.exe") as u32 + RVA_RAND_STATE, seconds);
    low_byte_bool(result)
}

/// First save versions that carry the manager's tick fields (`>= 0x23`) and its event manager (`>= 0x61`).
pub(super) const SAVE_VERSION_TICKS: u32 = 0x23;
pub(super) const SAVE_VERSION_EVENTS: u32 = 0x61;

/// `BFAIMgr::load(file, version)`: the mirror of `save`. From version `0x23` reads the tick remainder and counter
/// (both reads are always attempted) and succeeds only if both returned one item; from version `0x61` also loads the
/// embedded event manager, whose result is ignored. Earlier versions read nothing and succeed.
pub unsafe fn bfaimgr_load(this: u32, file: *const u8, version: u32) -> u32 {
    let mut ok = true;
    if version >= SAVE_VERSION_TICKS {
        let read = |offset: u32| unsafe { READ_BYTES_FROM_FILE.hooked()((this + offset) as *const u32, 4, 1, file) == 1 };
        let remainder_read = read(OFFSET_AI_TICK_REMAINDER);
        let counter_read = read(OFFSET_AI_TICK_COUNT);
        ok = remainder_read && counter_read;
    }
    if version >= SAVE_VERSION_EVENTS {
        unsafe { BFEVENTMGR_LOAD.hooked()((this + OFFSET_EVENT_MGR) as *const u32, file as *const u32, version) };
    }
    u32::from(ok)
}

/// `BFEventInfo`'s vtable (RVA) and size; the info block is the `BFEvent` record without its leading vtable word.
pub(super) const RVA_BFEVENT_INFO_VTABLE: u32 = 0x0023_5568;
pub(super) const BFEVENT_INFO_SIZE: usize = 0x18;
/// The `BFAIMgr` listener overrides receive the `BFEventListenerInterface` subobject, `8` bytes into the manager;
/// its event manager is at `+0x98` from there (`+0xa0` of the manager).
pub(super) const LISTENER_EVENT_MGR: u32 = 0x98;

/// `BFAIMgr::sendEvent(...)` (the listener override, `this` = the subobject): packs its eight arguments into a
/// `BFEventInfo` and posts it to the manager's event manager.
#[allow(clippy::too_many_arguments)]
pub unsafe fn bfaimgr_send_event(this: u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16) {
    let mut info = [0u8; BFEVENT_INFO_SIZE];
    let base = info.as_mut_ptr();
    unsafe {
        (base as *mut u32).write_unaligned(get_module_base("zoo.exe") as u32 + RVA_BFEVENT_INFO_VTABLE);
        (base.add(0x04) as *mut u32).write_unaligned(value_a);
        (base.add(0x08) as *mut u32).write_unaligned(target);
        (base.add(0x0c) as *mut u32).write_unaligned(value_b);
        (base.add(0x10) as *mut u16).write_unaligned(extra);
        (base.add(0x12) as *mut u16).write_unaligned(kind);
        (base.add(0x14) as *mut u16).write_unaligned(count);
        base.add(0x16).write(flag_a);
        base.add(0x17).write(flag_b);
        BFEVENTMGR_POST_EVENT.hooked()((this + LISTENER_EVENT_MGR) as *const u32, base as *const u32);
    }
}

/// `BFAIMgr::getEvents(events, target, kind)` (the listener override, `this` = the subobject): collects the
/// event manager's events for `target` and `kind` into `events`.
pub unsafe fn bfaimgr_get_events(this: u32, events: *const c_void, target: u32, kind: i8) {
    unsafe { BFEVENTMGR_GET_EVENTS.hooked()((this + LISTENER_EVENT_MGR) as *const u32, events, target as i32, kind) };
}
