//! The `executeCall` dispatch chains: name lookup, handler tables and the `ZTAIMgr`/`BFAIMgr` entry points.

use super::*;

/// Function names `ZTAIMgr::executeCall` handles itself; every other name goes to `BFAIMgr::executeCall`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ZtCall {
    MoveSlow,
    MoveMedium,
    MoveFast,
    Spray,
    KeeperRoutine,
    GuestRoutine,
    GuideRoutine,
    MaintRoutine,
    FaceTowardFood,
    FaceTowardTarget,
    ShowPredator,
    HidePredator,
    EnterBuilding,
    ExitBuilding,
    DustBall,
    KillPrey,
    CaughtPrey,
    Die,
    TargetBuilding,
    Hatch,
    EndTrick,
    ReturnToBuilding,
    PlaySetTank,
}

/// Exact, case-sensitive match of a call name against the 24 names the ZT chain tests.
pub fn lookup_zt_call(name: &[u8]) -> Option<ZtCall> {
    Some(match name {
        b"fWalk" | b"fSwim" => ZtCall::MoveSlow,
        b"fTrot" => ZtCall::MoveMedium,
        b"fRun" => ZtCall::MoveFast,
        b"fSpray" => ZtCall::Spray,
        b"fKeeperRoutine" => ZtCall::KeeperRoutine,
        b"fGuestRoutine" => ZtCall::GuestRoutine,
        b"fGuideRoutine" => ZtCall::GuideRoutine,
        b"fMaintRoutine" => ZtCall::MaintRoutine,
        b"fFaceTowardFood" => ZtCall::FaceTowardFood,
        b"fFaceTowardTarget" => ZtCall::FaceTowardTarget,
        b"fShowPredator" => ZtCall::ShowPredator,
        b"fHidePredator" => ZtCall::HidePredator,
        b"fEnterBuilding" => ZtCall::EnterBuilding,
        b"fExitBuilding" => ZtCall::ExitBuilding,
        b"fDustBall" => ZtCall::DustBall,
        b"fKillPrey" => ZtCall::KillPrey,
        b"fCaughtPrey" => ZtCall::CaughtPrey,
        b"fDie" => ZtCall::Die,
        b"fTargetBuilding" => ZtCall::TargetBuilding,
        b"fHatch" => ZtCall::Hatch,
        b"fEndTrick" => ZtCall::EndTrick,
        b"fReturnToBuilding" => ZtCall::ReturnToBuilding,
        b"fPlaySetTank" => ZtCall::PlaySetTank,
        _ => return None,
    })
}

/// The handler a dispatched call name reaches: one variant per `BFAIMgr`/`ZTAIMgr` `...F` wrapper.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Handler {
    MoveSlowF,
    MoveMediumF,
    MoveFastF,
    SprayF,
    KeeperRoutineF,
    GuestRoutineF,
    GuideRoutineF,
    MaintRoutineF,
    FaceTowardFoodF,
    FaceTowardTargetF,
    ShowPredatorF,
    HidePredatorF,
    EnterBuildingF,
    ExitBuildingF,
    DustBallF,
    KillPreyF,
    CaughtPreyF,
    DieF,
    TargetBuildingF,
    HatchF,
    EndTrickF,
    ReturnToBuildingF,
    PlaySetTankF,
    PlayF,
    SetBaseF,
    PlayWithBaseF,
    MoveEntityDeltaF,
    MoveEntitySpecialF,
    PlayTimeF,
    PlaySoundF,
    PlaySpecialF,
    PlaySetProbF,
    PlayReverseF,
    PlayPingPongF,
    PlayWithSoundF,
    PlayStopAtEndF,
    PlaySetF,
    MoveF,
    MoveReverseF,
    MoveWithSoundF,
    FaceDirectionF,
    FlyF,
    FlyReverseF,
    HaltF,
}

/// Where the dispatchers hand a call to a handler, so a test can record what they decide. `invoke(handler,
/// this, call, unit, trailing)` runs the handler with the unit it was given and the constant trailing
/// arguments the vanilla chain passes (`fPlayF`/`fPlayWithBaseF` `0, 0`, `fMoveF` `false`, `fFlyF` `0`; unused
/// slots are `0`), and returns its result (`0` for a handler with none).
pub trait Handlers {
    unsafe fn invoke(&self, handler: Handler, this: u32, call: u32, unit: u32, trailing: [u32; 2]) -> u32;
}

/// The vanilla handlers. A handler this crate has ported is reached through its wrapper address (`hooked()`,
/// what a vanilla caller would run), so its own deferral to vanilla for faulting inputs still applies; the rest
/// call the real wrappers.
pub(super) struct VanillaHandlers;

impl Handlers for VanillaHandlers {
    unsafe fn invoke(&self, handler: Handler, this: u32, call: u32, unit: u32, trailing: [u32; 2]) -> u32 {
        let (t, c, u) = (this as *const u32, call as *const u32, unit as *const u32);
        let (ci, ui) = (call as i32, unit as *const i32);
        unsafe {
            match handler {
                Handler::MoveSlowF => F_MOVE_SLOW_F.hooked()(t, ci, ui),
                Handler::MoveMediumF => F_MOVE_MEDIUM_F.hooked()(t, ci, ui),
                Handler::MoveFastF => F_MOVE_FAST_F.hooked()(t, ci, ui),
                Handler::SprayF => F_SPRAY_F.hooked()(ci, u) as u32,
                Handler::KeeperRoutineF => F_KEEPER_ROUTINE_F.hooked()(call, ui) as u32,
                Handler::GuestRoutineF => F_GUEST_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::GuideRoutineF => F_GUIDE_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::MaintRoutineF => F_MAINT_ROUTINE_F.hooked()(call, unit) as u32,
                Handler::FaceTowardFoodF => F_FACE_TOWARD_FOOD_F.hooked()(ci, unit as *const u8) as u32,
                Handler::FaceTowardTargetF => F_FACE_TOWARD_TARGET_F.hooked()(ci, unit as *const c_void) as u32,
                Handler::ShowPredatorF => F_SHOW_PREDATOR_F.hooked()(ci, ui) as u32,
                Handler::HidePredatorF => F_HIDE_PREDATOR_F.hooked()(ci, unit as *const u8) as u32,
                Handler::EnterBuildingF => F_ENTER_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::ExitBuildingF => F_EXIT_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::DustBallF => F_DUST_BALL_F.hooked()(t, ci, ui) as u32,
                Handler::KillPreyF => F_KILL_PREY_F.hooked()(ci, unit as *const u8) as u32,
                Handler::CaughtPreyF => F_CAUGHT_PREY_F.hooked()(ci, ui) as u32,
                Handler::DieF => F_DIE_F.hooked()(ci, ui) as u32,
                Handler::TargetBuildingF => F_TARGET_BUILDING_F.hooked()(t, ci, ui) as u32,
                Handler::HatchF => F_HATCH_F.hooked()(ci, ui) as u32,
                Handler::EndTrickF => F_END_TRICK_F.hooked()(ci, ui) as u32,
                Handler::ReturnToBuildingF => F_RETURN_TO_BUILDING_F.hooked()(ci, ui) as u32,
                Handler::PlaySetTankF => match f_play_set_tank(call, unit, play_set_vanilla) {
                    Some(result) => result,
                    None => F_PLAY_SET_TANK_F.original()(ci, unit as *const c_void),
                },
                Handler::PlayF => F_PLAY_F.hooked()(t, c, u, trailing[0] as i8, trailing[1] as i8) as u32,
                Handler::SetBaseF => F_SET_BASE_F.hooked()(t, c, u) as u32,
                Handler::PlayWithBaseF => F_PLAY_WITH_BASE_F.hooked()(t, c, u, trailing[0] as i8, trailing[1] as i8) as u32,
                Handler::MoveEntityDeltaF => F_MOVE_ENTITY_DELTA_F.hooked()(t, c, u) as u32,
                Handler::MoveEntitySpecialF => F_MOVE_ENTITY_SPECIAL_F.hooked()(t, c, u) as u32,
                Handler::PlayTimeF => F_PLAY_TIME_F.hooked()(t, c, u) as u32,
                Handler::PlaySoundF => F_PLAY_SOUND_F.hooked()(t, c, u) as u32,
                Handler::PlaySpecialF => F_PLAY_SPECIAL_F.hooked()(t, c, u) as u32,
                Handler::PlaySetProbF => F_PLAY_SET_PROB_F.hooked()(c, u) as u32,
                Handler::PlayReverseF => F_PLAY_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::PlayPingPongF => F_PLAY_PING_PONG_F.hooked()(t, c, u) as u32,
                Handler::PlayWithSoundF => F_PLAY_WITH_SOUND_F.hooked()(c, u) as u32,
                Handler::PlayStopAtEndF => F_PLAY_STOP_AT_END_F.hooked()(t, c, u) as u32,
                Handler::PlaySetF => F_PLAY_SET_F.hooked()(c, u) as u32,
                Handler::MoveF => F_MOVE_F.hooked()(t, c, u, trailing[0] != 0),
                Handler::MoveReverseF => F_MOVE_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::MoveWithSoundF => F_MOVE_WITH_SOUND_F.hooked()(t, c, u),
                Handler::FaceDirectionF => F_FACE_DIRECTION_F.hooked()(c, u) as u32,
                Handler::FlyF => F_FLY_F.hooked()(t, c, u, trailing[0] as i8) as u32,
                Handler::FlyReverseF => F_FLY_REVERSE_F.hooked()(t, c, u) as u32,
                Handler::HaltF => match f_halt_f(call, unit) {
                    Some(result) => u32::from(result),
                    None => F_HALT_F.original()(c, u) as u32,
                },
            }
        }
    }
}

/// The handler for a `ZTAIMgr` call name.
pub fn zt_handler(which: ZtCall) -> Handler {
    match which {
        ZtCall::MoveSlow => Handler::MoveSlowF,
        ZtCall::MoveMedium => Handler::MoveMediumF,
        ZtCall::MoveFast => Handler::MoveFastF,
        ZtCall::Spray => Handler::SprayF,
        ZtCall::KeeperRoutine => Handler::KeeperRoutineF,
        ZtCall::GuestRoutine => Handler::GuestRoutineF,
        ZtCall::GuideRoutine => Handler::GuideRoutineF,
        ZtCall::MaintRoutine => Handler::MaintRoutineF,
        ZtCall::FaceTowardFood => Handler::FaceTowardFoodF,
        ZtCall::FaceTowardTarget => Handler::FaceTowardTargetF,
        ZtCall::ShowPredator => Handler::ShowPredatorF,
        ZtCall::HidePredator => Handler::HidePredatorF,
        ZtCall::EnterBuilding => Handler::EnterBuildingF,
        ZtCall::ExitBuilding => Handler::ExitBuildingF,
        ZtCall::DustBall => Handler::DustBallF,
        ZtCall::KillPrey => Handler::KillPreyF,
        ZtCall::CaughtPrey => Handler::CaughtPreyF,
        ZtCall::Die => Handler::DieF,
        ZtCall::TargetBuilding => Handler::TargetBuildingF,
        ZtCall::Hatch => Handler::HatchF,
        ZtCall::EndTrick => Handler::EndTrickF,
        ZtCall::ReturnToBuilding => Handler::ReturnToBuildingF,
        ZtCall::PlaySetTank => Handler::PlaySetTankF,
    }
}

/// `ZTAIMgr::executeCall`: dispatches the ZT-specific names; anything else goes to the `BFAIMgr` chain
/// ([`execute_base_call`]) with the uncast unit. Afterwards the call's result flag is copied to `done`. Returns
/// the handler's own result (vanilla leaves it in `AL`, and `BFBehaviorSet::executeCall` reads it).
pub unsafe fn execute_call(this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    unsafe { execute_call_with(&VanillaHandlers, this, call, unit, done) }
}

/// [`execute_call`] over an injectable handler table. ZT handlers receive the unit only if its type is a
/// `ZTUnitType`, else null.
pub unsafe fn execute_call_with(handlers: &dyn Handlers, this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    let result = match lookup_zt_call(name) {
        Some(which) => (unsafe { handlers.invoke(zt_handler(which), this, call, unit_cast, [0, 0]) }) as u8,
        None => unsafe { execute_base_call_with(handlers, this, call, unit, done) },
    };
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
    result
}

/// Function names `BFAIMgr::executeCall` handles. Two literals alias `fPlaySpecial` (`fPlaySp`), `fPlayPingPong`
/// (`fPingPong`) and `fPlayWithSound` (`fPlaySFX`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BaseCall {
    Play,
    SetBase,
    PlayWithBase,
    MoveEntityDelta,
    MoveEntitySpecial,
    PlayTime,
    PlaySound,
    PlaySpecial,
    PlaySetProb,
    PlaySetTerrain,
    PlayReverse,
    PlayPingPong,
    PlayWithSound,
    PlayStopAtEnd,
    PlaySet,
    MoveSlow,
    MoveMedium,
    Move,
    MoveReverse,
    MoveFast,
    MoveWithSound,
    FaceDirection,
    Fly,
    FlyReverse,
    KeepControl,
    Halt,
}

/// Exact, case-sensitive match of a call name against the names the base chain tests.
pub fn lookup_base_call(name: &[u8]) -> Option<BaseCall> {
    Some(match name {
        b"fPlay" => BaseCall::Play,
        b"fSetBase" => BaseCall::SetBase,
        b"fPlayWithBase" => BaseCall::PlayWithBase,
        b"fMoveEntityDelta" => BaseCall::MoveEntityDelta,
        b"fMoveEntitySpecial" => BaseCall::MoveEntitySpecial,
        b"fPlayTime" => BaseCall::PlayTime,
        b"fPlaySound" => BaseCall::PlaySound,
        b"fPlaySpecial" | b"fPlaySp" => BaseCall::PlaySpecial,
        b"fPlaySetProb" => BaseCall::PlaySetProb,
        b"fPlaySetTerrain" => BaseCall::PlaySetTerrain,
        b"fPlayReverse" => BaseCall::PlayReverse,
        b"fPlayPingPong" | b"fPingPong" => BaseCall::PlayPingPong,
        b"fPlayWithSound" | b"fPlaySFX" => BaseCall::PlayWithSound,
        b"fPlayStopAtEnd" => BaseCall::PlayStopAtEnd,
        b"fPlaySet" => BaseCall::PlaySet,
        b"fMoveSlow" => BaseCall::MoveSlow,
        b"fMoveMedium" => BaseCall::MoveMedium,
        b"fMove" => BaseCall::Move,
        b"fMoveReverse" => BaseCall::MoveReverse,
        b"fMoveFast" => BaseCall::MoveFast,
        b"fMoveWithSound" => BaseCall::MoveWithSound,
        b"fFaceDirection" => BaseCall::FaceDirection,
        b"fFly" => BaseCall::Fly,
        b"fFlyReverse" => BaseCall::FlyReverse,
        b"fKeepControl" => BaseCall::KeepControl,
        b"fHalt" => BaseCall::Halt,
        _ => return None,
    })
}

/// `BFAIMgr` vtable slot `fPlaySetTerrainF(call, unit)`, which `ZTAIMgr` overrides.
pub(super) const BFAIMGR_VTABLE_PLAY_SET_TERRAIN: u32 = 0x14;

/// `BFAIMgr::executeCall`: dispatches a call by name and returns the low byte of the handler's result (`0`
/// for an unknown name). The play family receives the unit as given; move/fly/halt handlers get the unit
/// only if its type is a `ZTUnitType`, else null. The call's result flag is copied to `done`.
pub unsafe fn execute_base_call(this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    unsafe { execute_base_call_with(&VanillaHandlers, this, call, unit, done) }
}

/// [`execute_base_call`] over an injectable handler table.
pub unsafe fn execute_base_call_with(handlers: &dyn Handlers, this: u32, call: u32, unit: u32, done: *mut u8) -> u8 {
    let name = unsafe { &*(call as *const RawCallString) }.bytes();
    let unit_cast = if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } { unit } else { 0 };
    let result = match lookup_base_call(name) {
        Some(which) => unsafe { dispatch_base_call(handlers, this, which, call, unit, unit_cast) },
        None => 0,
    };
    unsafe { done.write_volatile(get_from_memory::<u8>(call + CALL_DONE_FLAG_OFFSET)) };
    result
}

/// How a base call name reaches its handler: which handler, whether it gets the cast unit (move, fly and halt
/// handlers) or the unit as given (the play family), and the constant trailing arguments.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BaseRoute {
    pub handler: Handler,
    pub cast_unit: bool,
    pub trailing: [u32; 2],
}

/// The route for a base call name; `None` for the two names the dispatcher handles itself
/// (`fPlaySetTerrain`, through the manager's vtable, and `fKeepControl`).
pub fn base_route(which: BaseCall) -> Option<BaseRoute> {
    let route = |handler, cast_unit, trailing| Some(BaseRoute { handler, cast_unit, trailing });
    match which {
        BaseCall::Play => route(Handler::PlayF, false, [0, 0]),
        BaseCall::SetBase => route(Handler::SetBaseF, false, [0, 0]),
        BaseCall::PlayWithBase => route(Handler::PlayWithBaseF, false, [0, 0]),
        BaseCall::MoveEntityDelta => route(Handler::MoveEntityDeltaF, false, [0, 0]),
        BaseCall::MoveEntitySpecial => route(Handler::MoveEntitySpecialF, false, [0, 0]),
        BaseCall::PlayTime => route(Handler::PlayTimeF, false, [0, 0]),
        BaseCall::PlaySound => route(Handler::PlaySoundF, false, [0, 0]),
        BaseCall::PlaySpecial => route(Handler::PlaySpecialF, false, [0, 0]),
        BaseCall::PlaySetProb => route(Handler::PlaySetProbF, false, [0, 0]),
        BaseCall::PlaySetTerrain => None,
        BaseCall::PlayReverse => route(Handler::PlayReverseF, false, [0, 0]),
        BaseCall::PlayPingPong => route(Handler::PlayPingPongF, false, [0, 0]),
        BaseCall::PlayWithSound => route(Handler::PlayWithSoundF, false, [0, 0]),
        BaseCall::PlayStopAtEnd => route(Handler::PlayStopAtEndF, false, [0, 0]),
        BaseCall::PlaySet => route(Handler::PlaySetF, false, [0, 0]),
        BaseCall::MoveSlow => route(Handler::MoveSlowF, true, [0, 0]),
        BaseCall::MoveMedium => route(Handler::MoveMediumF, true, [0, 0]),
        BaseCall::Move => route(Handler::MoveF, true, [0, 0]),
        BaseCall::MoveReverse => route(Handler::MoveReverseF, true, [0, 0]),
        BaseCall::MoveFast => route(Handler::MoveFastF, true, [0, 0]),
        BaseCall::MoveWithSound => route(Handler::MoveWithSoundF, true, [0, 0]),
        BaseCall::FaceDirection => route(Handler::FaceDirectionF, false, [0, 0]),
        BaseCall::Fly => route(Handler::FlyF, true, [0, 0]),
        BaseCall::FlyReverse => route(Handler::FlyReverseF, true, [0, 0]),
        BaseCall::KeepControl => None,
        BaseCall::Halt => route(Handler::HaltF, true, [0, 0]),
    }
}

pub(super) unsafe fn dispatch_base_call(handlers: &dyn Handlers, this: u32, which: BaseCall, call: u32, unit: u32, unit_cast: u32) -> u8 {
    if let Some(route) = base_route(which) {
        let target_unit = if route.cast_unit { unit_cast } else { unit };
        return unsafe { handlers.invoke(route.handler, this, call, target_unit, route.trailing) } as u8;
    }
    match which {
        BaseCall::PlaySetTerrain => unsafe {
            call_virtual_args(this, BFAIMGR_VTABLE_PLAY_SET_TERRAIN, [call, unit]) as u8
        },
        _ => (unit_cast != 0) as u8,
    }
}

pub(super) fn call_done_flag(call: u32) -> *const bool {
    (call + CALL_DONE_FLAG_OFFSET) as *const bool
}

/// `BFAIMgr` tick accumulator remainder (`+0xe8`), tick counter (`+0xec`) and divisor (`+0xe4`).
pub(super) const OFFSET_AI_TICK_REMAINDER: u32 = 0xe8;
pub(super) const OFFSET_AI_TICK_COUNT: u32 = 0xec;
pub(super) const OFFSET_AI_TICK_DIVISOR: u32 = 0xe4;
/// `BFAIMgr`'s embedded `BFEventMgr` (`+0xa0`) and the path-cache tree container (`{head*, size}` at `+0x12c`).
pub(super) const OFFSET_EVENT_MGR: u32 = 0xa0;
pub(super) const OFFSET_PATH_CACHE: u32 = 0x12c;
/// Pool size class of the path-cache tree nodes.
pub(super) const PATH_CACHE_NODE_SIZE: u32 = 0x18;
/// Globals (RVAs): next tick at which the path cache is flushed, and the flush interval in ticks.
pub(super) const RVA_PATH_CACHE_NEXT_FLUSH: u32 = 0x0023_94b4;
pub(super) const RVA_PATH_CACHE_FLUSH_INTERVAL: u32 = 0x0023_9028;
/// `ZTMaintTaskPool` instance (`DAT_00638f99 + 7`) that `ZTAIMgr::update` pumps.
pub(super) const RVA_MAINT_TASK_POOL: u32 = 0x0023_8fa0;
/// `BFAIMgr` vtable slot `removeEntities`, `BFEntity` vtable slot `update(dt)`, `ZTShowMgr` vtable slot `update`.
pub(super) const AI_VTABLE_REMOVE_ENTITIES: u32 = 0x40;
pub(super) const ENTITY_VTABLE_UPDATE: u32 = 0x28;
pub(super) const SHOW_MGR_VTABLE_UPDATE: u32 = 0x4;
/// `BFWorldMgr` entity vector begin/end pointers.
pub(super) const WORLD_ENTITY_BEGIN: u32 = 0x80;
pub(super) const WORLD_ENTITY_END: u32 = 0x84;

/// RVAs of the `GLOBAL_ZTWorldMgr` and `GLOBAL_ZTShowMgr` pointer cells. Read on every call, as vanilla
/// does, so the live tests can point them at fakes for one call.
pub(super) const RVA_GLOBAL_WORLD_MGR: u32 = 0x0023_8040;
pub(super) const RVA_GLOBAL_SHOW_MGR: u32 = 0x0023_9008;
