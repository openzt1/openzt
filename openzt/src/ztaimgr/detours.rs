//! The `ZTAIMgr`/`BFAIMgr` detours and the real-body wrappers the live battery calls.

use super::*;

#[detour_mod]
pub(super) mod ztaimgr_detours {
    use super::*;

    // Behavior halves: `(unit, done)`.
    #[detour(F_END_TRICK)]
    unsafe extern "stdcall" fn f_end_trick_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_end_trick(unit as u32, done) }
    }

    #[detour(F_ENTER_BUILDING)]
    unsafe extern "stdcall" fn f_enter_building_detour(unit: *const u32, done: *const bool) {
        if !unsafe { f_enter_building(unit as u32, done) } {
            unsafe { F_ENTER_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_RETURN_TO_BUILDING)]
    unsafe extern "stdcall" fn f_return_to_building_detour(unit: *const u32, done: *const bool) {
        if !unsafe { f_return_to_building(unit as u32, done) } {
            unsafe { F_RETURN_TO_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_HIDE_PREDATOR)]
    unsafe extern "stdcall" fn f_hide_predator_detour(unit: *const u32, done: *const bool) -> bool {
        if unsafe { f_hide_predator(unit as u32, done) } {
            true
        } else {
            unsafe { F_HIDE_PREDATOR_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_GUEST_ROUTINE)]
    unsafe extern "stdcall" fn f_guest_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_guest_routine(unit as u32, done) }
    }

    #[detour(F_MAINT_ROUTINE)]
    unsafe extern "stdcall" fn f_maint_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_maint_routine(unit as u32, done) }
    }

    #[detour(F_GUIDE_ROUTINE)]
    unsafe extern "stdcall" fn f_guide_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_guide_routine(unit as u32, done) }
    }

    #[detour(F_KEEPER_ROUTINE)]
    unsafe extern "stdcall" fn f_keeper_routine_detour(unit: *const u32, done: *const bool) -> bool {
        unsafe { f_keeper_routine(unit as u32, done) }
    }

    // `F` wrappers: `(call, unit)`, with the `done` flag at `call + 0x18`.
    #[detour(F_END_TRICK_F)]
    unsafe extern "stdcall" fn f_end_trick_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_end_trick(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_ENTER_BUILDING_F)]
    unsafe extern "stdcall" fn f_enter_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_enter_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_ENTER_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_RETURN_TO_BUILDING_F)]
    unsafe extern "stdcall" fn f_return_to_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_return_to_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_RETURN_TO_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_HIDE_PREDATOR_F)]
    unsafe extern "stdcall" fn f_hide_predator_f_detour(call: i32, unit: *const u8) -> bool {
        if unsafe { f_hide_predator(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_HIDE_PREDATOR_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(F_GUEST_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guest_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_guest_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_MAINT_ROUTINE_F)]
    unsafe extern "stdcall" fn f_maint_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_maint_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_GUIDE_ROUTINE_F)]
    unsafe extern "stdcall" fn f_guide_routine_f_detour(call: u32, unit: u32) -> bool {
        unsafe { f_guide_routine(unit, call_done_flag(call)) }
    }

    #[detour(F_KEEPER_ROUTINE_F)]
    unsafe extern "stdcall" fn f_keeper_routine_f_detour(call: u32, unit: *const i32) -> bool {
        unsafe { f_keeper_routine(unit as u32, call_done_flag(call)) }
    }

    // Prey/predator/death/hatch/exit handlers. Halves are `(unit, done)`; `F` wrappers are `(call, unit)`.
    #[detour(F_SHOW_PREDATOR)]
    unsafe extern "stdcall" fn f_show_predator_detour(unit: *const i32, done: *const u8) -> u32 {
        match unsafe { f_show_predator(unit as u32, done as *const bool) } {
            Some(result) => result as u32,
            None => unsafe { F_SHOW_PREDATOR_DETOUR.call(unit, done) },
        }
    }

    #[detour(F_SHOW_PREDATOR_F)]
    unsafe extern "stdcall" fn f_show_predator_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_show_predator(unit as u32, call_done_flag(call as u32)) } {
            Some(result) => result,
            None => unsafe { F_SHOW_PREDATOR_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_CAUGHT_PREY)]
    unsafe extern "stdcall" fn f_caught_prey_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_caught_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_CAUGHT_PREY_F)]
    unsafe extern "stdcall" fn f_caught_prey_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_caught_prey(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_KILL_PREY)]
    unsafe extern "stdcall" fn f_kill_prey_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_kill_prey(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_KILL_PREY_F)]
    unsafe extern "stdcall" fn f_kill_prey_f_detour(call: i32, unit: *const u8) -> bool {
        unsafe { f_kill_prey(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_DIE)]
    unsafe extern "stdcall" fn f_die_detour(unit: *const i32, done: *const u8) -> u32 {
        unsafe { f_die(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_DIE_F)]
    unsafe extern "stdcall" fn f_die_f_detour(call: i32, unit: *const i32) -> bool {
        unsafe { f_die(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_HATCH)]
    unsafe extern "stdcall" fn f_hatch_detour(unit: *const i32, done: *const u8) -> u32 {
        if unsafe { f_hatch(unit as u32, done as *const bool) } {
            1
        } else {
            unsafe { F_HATCH_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_HATCH_F)]
    unsafe extern "stdcall" fn f_hatch_f_detour(call: i32, unit: *const i32) -> bool {
        if unsafe { f_hatch(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_HATCH_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(F_EXIT_BUILDING)]
    unsafe extern "stdcall" fn f_exit_building_detour(unit: *const i32, done: *const u8) {
        if !unsafe { f_exit_building(unit as u32, done as *const bool) } {
            unsafe { F_EXIT_BUILDING_DETOUR.call(unit, done) };
        }
    }

    #[detour(F_EXIT_BUILDING_F)]
    unsafe extern "stdcall" fn f_exit_building_f_detour(call: i32, unit: *const i32) -> bool {
        match unsafe { f_exit_building_al(unit as u32, call_done_flag(call as u32)) } {
            Some(al) => al != 0,
            None => unsafe { F_EXIT_BUILDING_F_DETOUR.call(call, unit) },
        }
    }

    #[detour(F_FACE_TOWARD_FOOD)]
    unsafe extern "stdcall" fn f_face_toward_food_detour(unit: *const u8, done: *const u8) -> u32 {
        unsafe { f_face_toward_food(unit as u32, done as *const bool) as u32 }
    }

    #[detour(F_FACE_TOWARD_FOOD_F)]
    unsafe extern "stdcall" fn f_face_toward_food_f_detour(call: i32, unit: *const u8) -> bool {
        unsafe { f_face_toward_food(unit as u32, call_done_flag(call as u32)) }
    }

    #[detour(F_FACE_TOWARD_TARGET)]
    unsafe extern "stdcall" fn f_face_toward_target_detour(unit: *const c_void, done: *const u8) -> u32 {
        if unsafe { f_face_toward_target(unit as u32, done as *const bool) } {
            1
        } else {
            unsafe { F_FACE_TOWARD_TARGET_DETOUR.call(unit, done) }
        }
    }

    #[detour(F_FACE_TOWARD_TARGET_F)]
    unsafe extern "stdcall" fn f_face_toward_target_f_detour(call: i32, unit: *const c_void) -> bool {
        if unsafe { f_face_toward_target(unit as u32, call_done_flag(call as u32)) } {
            true
        } else {
            unsafe { F_FACE_TOWARD_TARGET_F_DETOUR.call(call, unit) }
        }
    }

    #[detour(ZTAIMGR_F_PLAY_SET_TERRAIN_F)]
    unsafe extern "thiscall" fn f_play_set_terrain_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        match unsafe { f_play_set_terrain_f(call as u32, unit as u32, unit_tile_flags_vanilla, play_set_vanilla) } {
            Some(result) => result,
            None => unsafe { ZTAIMGR_F_PLAY_SET_TERRAIN_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_DUST_BALL)]
    unsafe extern "thiscall" fn f_dust_ball_detour(this: *const u32, unit: *const i32, done: *const i8) -> bool {
        match unsafe { f_dust_ball(this as u32, unit as u32, done as *const bool, &DUST_BALL_VANILLA) } {
            Some(result) => result,
            None => unsafe { F_DUST_BALL_DETOUR.call(this, unit, done) },
        }
    }

    #[detour(F_DUST_BALL_F)]
    unsafe extern "thiscall" fn f_dust_ball_f_detour(this: *const u32, call: i32, unit: *const i32) -> bool {
        match unsafe { f_dust_ball(this as u32, unit as u32, call_done_flag(call as u32), &DUST_BALL_VANILLA) } {
            Some(result) => result,
            None => unsafe { F_DUST_BALL_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_RANDOM_WALK)]
    unsafe extern "thiscall" fn f_random_walk_detour(this: *const u32, unit: *const i32) -> u32 {
        match unsafe { f_random_walk(this as u32, unit as u32, &RANDOM_WALK_VANILLA) } {
            Some(result) => u32::from(result),
            None => unsafe { F_RANDOM_WALK_DETOUR.call(this, unit) },
        }
    }

    #[detour(F_FLY_F)]
    unsafe extern "thiscall" fn f_fly_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8) -> bool {
        unsafe { f_fly_f(this as u32, call as u32, unit as u32, u32::from(reverse as u8), &FLY_VANILLA) }
    }

    #[detour(F_PLAY_SET_PROB_F)]
    unsafe extern "stdcall" fn f_play_set_prob_f_detour(call: *const u32, unit: *const u32) -> bool {
        unsafe { f_play_set_prob_f(call as u32, unit as u32, &SET_PROB_VANILLA) }
    }

    #[detour(F_PLAY_F)]
    unsafe extern "thiscall" fn f_play_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
        unsafe { f_play_f(this as u32, call as u32, unit as u32, reverse != 0, ping_pong != 0, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_PLAY_WITH_BASE_F)]
    unsafe extern "thiscall" fn f_play_with_base_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
        unsafe { f_play_with_base_f(this as u32, call as u32, unit as u32, reverse != 0, ping_pong != 0, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_SET_BASE_F)]
    unsafe extern "thiscall" fn f_set_base_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { f_set_base_f(this as u32, call as u32, unit as u32, &PLAY_VANILLA_OPS) }
    }

    #[detour(F_PLAY_SPECIAL)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_play_special_detour(this: *const u32, anim: *const i8, sound: *const i8, seconds: i32, count: i32, special: *const i8, unit: *const i32, done: *const u8) -> bool {
        let args = FPlaySpecialArgs { this: this as u32, anim_name: anim as u32, sound_name: sound as u32, seconds: seconds as u32, count, special: special as u32, unit: unit as u32, done };
        unsafe { f_play_special(&args, &F_PLAY_SPECIAL_VANILLA_OPS) }
    }

    #[detour(F_MOVE)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_move_detour(this: *const u32, first_param: i32, seconds: i32, unit: *const i32, done: *const u8, name: i32, speed: u32, extra: *const i8, reverse: u32, limit: u32) -> bool {
        let args = FMoveArgs { this: this as u32, first_param: first_param as u32, seconds, unit: unit as u32, done, name: name as u32, speed, extra: extra as u32, reverse, limit };
        unsafe { f_move(&args, &F_MOVE_VANILLA_OPS) }
    }

    #[detour(F_MOVE_F)]
    unsafe extern "thiscall" fn f_move_f_detour(this: *const u32, call: *const u32, unit: *const u32, reverse: bool) -> u32 {
        u32::from(unsafe { f_move_f(this as u32, call as u32, unit as u32, reverse, &MOVE_VANILLA) })
    }

    #[detour(F_MOVE_REVERSE_F)]
    unsafe extern "thiscall" fn f_move_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { F_MOVE_F.hooked()(this, call, unit, true) })
    }

    #[detour(F_MOVE_WITH_SOUND_F)]
    unsafe extern "thiscall" fn f_move_with_sound_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        u32::from(unsafe { f_move_with_sound_f(this as u32, call as u32, unit as u32, &MOVE_VANILLA) })
    }

    #[detour(F_MOVE_SLOW_F)]
    unsafe extern "thiscall" fn f_move_slow_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Slow, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_MOVE_MEDIUM_F)]
    unsafe extern "thiscall" fn f_move_medium_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Medium, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_MOVE_FAST_F)]
    unsafe extern "thiscall" fn f_move_fast_f_detour(this: *const u32, call: i32, unit: *const i32) -> u32 {
        u32::from(unsafe { f_move_speed_f(this as u32, call as u32, unit as u32, MoveSpeed::Fast, &MOVE_SPEED_VANILLA) })
    }

    #[detour(F_TARGET_BUILDING)]
    unsafe extern "thiscall" fn f_target_building_detour(this: *const u32, unit: *const u32, done: *const u8) -> bool {
        unsafe { f_target_building(this as u32, unit as u32, done as *const bool, &TARGET_BUILDING_VANILLA) }
    }

    #[detour(F_TARGET_BUILDING_F)]
    unsafe extern "thiscall" fn f_target_building_f_detour(this: *const u32, call: i32, unit: *const i32) -> bool {
        unsafe { f_target_building_f(this as u32, call as u32, unit as u32, &TARGET_BUILDING_VANILLA) }
    }

    #[detour(F_FACE_DIRECTION_F)]
    unsafe extern "stdcall" fn f_face_direction_f_detour(call: *const u32, _unit: *const u32) -> bool {
        unsafe { f_face_direction_f(call as u32) }
    }

    #[detour(F_MOVE_ENTITY_DELTA_F)]
    unsafe extern "thiscall" fn f_move_entity_delta_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        match unsafe { f_move_entity_delta_f(this as u32, call as u32, unit as u32, clear_mixer_vanilla, move_entity_delta_vanilla) } {
            Some(result) => result as u32,
            None => unsafe { F_MOVE_ENTITY_DELTA_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_MOVE_ENTITY_SPECIAL_F)]
    unsafe extern "thiscall" fn f_move_entity_special_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
        match unsafe { f_move_entity_special_f(this as u32, call as u32, unit as u32, move_entity_delta_vanilla) } {
            Some(result) => result as u32,
            None => unsafe { F_MOVE_ENTITY_SPECIAL_F_DETOUR.call(this, call, unit) },
        }
    }

    #[detour(F_PLAY_SET_F)]
    unsafe extern "stdcall" fn f_play_set_f_detour(call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_set_f(call as u32, unit as u32, play_set_vanilla) })
    }

    #[detour(F_PLAY_REVERSE_F)]
    unsafe extern "thiscall" fn f_play_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.hooked()(this, call, unit, 1, 0) }
    }

    #[detour(F_PLAY_PING_PONG_F)]
    unsafe extern "thiscall" fn f_play_ping_pong_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 1) }
    }

    #[detour(F_PLAY_STOP_AT_END_F)]
    unsafe extern "thiscall" fn f_play_stop_at_end_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        if !unsafe { f_play_stop_at_end_prepare(unit as u32) } {
            return unsafe { F_PLAY_STOP_AT_END_F_DETOUR.call(this, call, unit) };
        }
        unsafe { F_PLAY_F.hooked()(this, call, unit, 0, 0) }
    }

    #[detour(F_FLY_REVERSE_F)]
    unsafe extern "thiscall" fn f_fly_reverse_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        unsafe { F_FLY_F.hooked()(this, call, unit, 1) }
    }

    #[detour(F_PLAY_WITH_SOUND_F)]
    unsafe extern "stdcall" fn f_play_with_sound_f_detour(call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        unsafe {
            F_PLAY_WITH_SOUND.hooked()(
                call_params_begin(call) as *const c_void,
                call_params_end(call).wrapping_sub(CALL_PARAM_SIZE) as *const c_void,
                unit,
                (call + CALL_DONE_FLAG_OFFSET) as *const i8,
            )
        }
    }

    #[detour(F_PLAY_SOUND_F)]
    unsafe extern "thiscall" fn f_play_sound_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        let call = call as u32;
        if unsafe { call_param_count(call) } != 1 {
            return false;
        }
        let result = unsafe { F_PLAY_SOUND.hooked()(this, call_params_begin(call) as *const i8, unit as *const i32, (call + CALL_DONE_FLAG_OFFSET) as *const u8) };
        low_byte_bool(result)
    }

    #[detour(F_PLAY_TIME_F)]
    unsafe extern "thiscall" fn f_play_time_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        low_byte_bool(unsafe { f_play_time_f(this as u32, call as u32, unit as u32, play_time_vanilla) })
    }

    #[detour(F_FLY)]
    #[allow(clippy::too_many_arguments)]
    unsafe extern "thiscall" fn f_fly_detour(
        _this: *const u32, unit: *const u32, name: *const c_void, value: i32, extra: *const c_void, done: *const i8, reverse: i32, limit: i32,
    ) -> bool {
        let args = FFlyArgs { unit: unit as u32, name: name as u32, value: value as u32, extra: extra as u32, done: done as *const u8, reverse: reverse as u32, limit: limit as u32 };
        unsafe { f_fly(&args, &FLY_CORE_VANILLA) }
    }

    #[detour(F_PLAY_WITH_SOUND)]
    unsafe extern "stdcall" fn f_play_with_sound_detour(anim_name: *const c_void, sound_name: *const c_void, unit: *const u32, done: *const i8) -> bool {
        unsafe { f_play_with_sound(anim_name as u32, sound_name as u32, unit as u32, done as *const u8, &PLAY_WITH_SOUND_VANILLA) }
    }

    #[detour(F_PLAY_TIME)]
    unsafe extern "thiscall" fn f_play_time_detour(_this: *const u32, name: *const c_void, millis: i32, unit: *const u32, done: *const i8, _flag_b: i8) -> bool {
        unsafe { f_play_time(name as u32, millis as u32, unit as u32, done as *const u8, &PLAY_TIME_VANILLA_OPS) }
    }

    #[detour(F_PLAY_SET)]
    unsafe extern "stdcall" fn f_play_set_detour(name: *const i32, unit: *const c_void, child: i8, flag: u32) -> bool {
        unsafe { f_play_set(name as u32, unit as u32, child != 0, flag, &PLAY_SET_VANILLA) }
    }

    #[detour(F_PLAY_SET_PROB)]
    unsafe extern "stdcall" fn f_play_set_prob_detour(entries: *const i32, unit: *const std::ffi::c_void) -> u32 {
        u32::from(unsafe { f_play_set_prob(entries as u32, unit as u32, &SET_PROB_PICK_VANILLA) })
    }

    #[detour(F_PLAY_SOUND)]
    unsafe extern "thiscall" fn f_play_sound_detour(this: *const u32, sound: *const i8, unit: *const i32, done: *const u8) -> u32 {
        let mode_a = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_PLAY_MODE_A_STRING);
        u32::from(unsafe { f_play_sound(this as u32, sound as u32, unit as u32, done, mode_a, play_special_f_forward) })
    }

    #[detour(F_PLAY_SPECIAL_F)]
    unsafe extern "thiscall" fn f_play_special_f_detour(this: *const u32, call: *const u32, unit: *const u32) -> bool {
        match unsafe { f_play_special_f(this as u32, call as u32, unit as u32, &PLAY_SPECIAL_F_VANILLA) } {
            Some(result) => result,
            None => low_byte_bool(unsafe { F_PLAY_SPECIAL_F_DETOUR.call(this, call, unit) } as u32),
        }
    }

    #[detour(EXECUTE_CALL)]
    unsafe extern "thiscall" fn execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> bool {
        unsafe { execute_call(this as u32, call as u32, unit as u32, done as *mut u8) != 0 }
    }

    #[detour(BFAIMGR_EXECUTE_CALL)]
    unsafe extern "thiscall" fn bfaimgr_execute_call_detour(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> u32 {
        unsafe { execute_base_call(this as u32, call as u32, unit as u32, done as *mut u8) as u32 }
    }

    /// `fSprayF` only forwards to the `return 1` stub `BFAIMgr::fFaceDirection`, so it returns 1.
    #[detour(F_SPRAY_F)]
    unsafe extern "stdcall" fn f_spray_f_detour(_call: i32, _unit: *const u32) -> bool {
        true
    }

    #[detour(GET_RANDOM_X)]
    unsafe extern "thiscall" fn get_random_x_detour(this: *const u32, entity: *const u32) -> i32 {
        unsafe { get_random_x(this as u32, entity as u32) }
    }

    #[detour(GET_RANDOM_Y)]
    unsafe extern "thiscall" fn get_random_y_detour(this: *const u32, entity: *const u32) -> i32 {
        unsafe { get_random_y(this as u32, entity as u32) }
    }

    #[detour(IS_SLOW_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_slow_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_slow_move_function(call) }
    }

    #[detour(IS_MEDIUM_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_medium_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_medium_move_function(call) }
    }

    #[detour(IS_FAST_MOVE_FUNCTION)]
    unsafe extern "thiscall" fn is_fast_move_function_detour(_this: *const u32, call: *const u32) -> bool {
        unsafe { is_fast_move_function(call) }
    }

    #[detour(CONVERT_CONSTANT_PARAM)]
    unsafe extern "thiscall" fn convert_constant_param_detour(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
        match unsafe { convert_constant_param(name, entity as u32) } {
            Some(value) => value,
            None => unsafe { CONVERT_CONSTANT_PARAM_DETOUR.call(this, name, entity) },
        }
    }

    #[detour(ZTAIMGR_UPDATE)]
    unsafe extern "thiscall" fn ztaimgr_update_detour(this: *const u32, dt: u32) {
        unsafe { ztaimgr_update(this as u32, dt) }
    }

    #[detour(BFAIMGR_UPDATE)]
    unsafe extern "thiscall" fn bfaimgr_update_detour(this: *const u32, dt: u32) {
        unsafe { bfaimgr_update(this as u32, dt) }
    }

    #[detour(BFAIMGR_SAVE)]
    unsafe extern "thiscall" fn bfaimgr_save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { bfaimgr_save(this as u32, file) }
    }

    #[detour(BFAIMGR_LOAD)]
    unsafe extern "thiscall" fn bfaimgr_load_detour(this: *const u32, file: *const u8, version: u32) -> u32 {
        unsafe { bfaimgr_load(this as u32, file, version) }
    }

    #[detour(BFAIMGR_SEND_EVENT)]
    unsafe extern "thiscall" fn bfaimgr_send_event_detour(
        this: *const u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16,
    ) {
        unsafe { bfaimgr_send_event(this as u32, kind, target, flag_b, value_a, flag_a, value_b, extra, count) }
    }

    #[detour(BFAIMGR_GET_EVENTS)]
    unsafe extern "thiscall" fn bfaimgr_get_events_detour(this: *const u32, events: *const c_void, target: u32, kind: i8) {
        unsafe { bfaimgr_get_events(this as u32, events, target, kind) }
    }

    #[detour(BFAIMGR_CALL_DONE)]
    unsafe extern "thiscall" fn bfaimgr_call_done_detour(_this: *const u32, entity: *const u32) -> bool {
        unsafe { bfaimgr_call_done(entity as u32) }
    }

    #[detour(BFAIMGR_REMOVE_ENTITIES)]
    unsafe extern "thiscall" fn bfaimgr_remove_entities_detour(this: *const u32) {
        unsafe { bfaimgr_remove_entities(this as u32) }
    }

    #[detour(BFAIMGR_GET_PATH_1)]
    unsafe extern "thiscall" fn bfaimgr_get_path_tiles_detour(this: *const u32, from: *const u32, to: *const u32, unit: *const u32, flag: bool) -> bool {
        unsafe { bfaimgr_get_path_tiles(this as u32, from as u32, to as u32, unit as u32, flag) }
    }

    #[detour(ZTAIMGR_INIT_AIPARAMS)]
    unsafe extern "thiscall" fn ztaimgr_init_ai_params_detour(this: *const u32) -> u32 {
        unsafe { ztaimgr_init_ai_params(this as u32) }
    }

    #[detour(BFAIMGR_CHECK_PATH)]
    unsafe extern "thiscall" fn bfaimgr_check_path_detour(this: *const u32, from: *const u32, to: *const u32, unit: *const u32) -> u32 {
        unsafe { bfaimgr_check_path(this as u32, from as u32, to as u32, unit as u32) }
    }

    #[detour(BFAIMGR_LOAD_DATA)]
    unsafe extern "thiscall" fn bfaimgr_load_data_detour(_this: *const u32, full: bool) -> bool {
        unsafe { bfaimgr_load_data(full) }
    }

    #[detour(BFAIMGR_INIT)]
    unsafe extern "thiscall" fn bfaimgr_init_detour(this: *const u32) -> bool {
        unsafe { bfaimgr_init(this as u32) }
    }

    #[detour(BFAIMGR_INIT_AIPARAMS)]
    unsafe extern "thiscall" fn bfaimgr_init_ai_params_detour(this: *const u32) -> u32 {
        unsafe { bfaimgr_init_ai_params(this as u32) }
    }

    #[detour(BFAIMGR_CONSTRUCTOR)]
    unsafe extern "thiscall" fn bfaimgr_constructor_detour(this: *const u32) -> *const u32 {
        unsafe { bfaimgr_construct(this as u32) as *const u32 }
    }

    #[detour(BFAIMGR_DESTRUCTOR_0)]
    unsafe extern "thiscall" fn bfaimgr_destructor_0_detour(this: *const u32) -> *const u32 {
        unsafe { bfaimgr_destruct(this as u32) as *const u32 }
    }

    #[detour(BFAIMGR_DESTRUCTOR_1)]
    unsafe extern "thiscall" fn bfaimgr_destructor_1_detour(this: *const u32, flags: u8) -> *const u32 {
        unsafe { bfaimgr_deleting_destruct(this as u32, flags) as *const u32 }
    }

    #[detour(ZTAIMGR_CREATE)]
    unsafe extern "stdcall" fn ztaimgr_create_detour() -> u32 {
        unsafe { ztaimgr_create() }
    }

    #[detour(REMOVE_ENTITY)]
    unsafe extern "thiscall" fn remove_entity_detour(_this: *const u32, entity: *const u32) -> bool {
        unsafe { remove_entity(entity as u32) }
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn get_random_x(this: *const u32, entity: *const u32) -> i32 {
            unsafe { super::GET_RANDOM_X_DETOUR.call(this, entity) }
        }

        pub(crate) fn get_random_y(this: *const u32, entity: *const u32) -> i32 {
            unsafe { super::GET_RANDOM_Y_DETOUR.call(this, entity) }
        }

        pub(crate) fn is_slow_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_SLOW_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn is_medium_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_MEDIUM_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn is_fast_move_function(this: *const u32, call: *const u32) -> bool {
            unsafe { super::IS_FAST_MOVE_FUNCTION_DETOUR.call(this, call) }
        }

        pub(crate) fn convert_constant_param(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
            unsafe { super::CONVERT_CONSTANT_PARAM_DETOUR.call(this, name, entity) }
        }

        pub(crate) fn f_end_trick(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_END_TRICK_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_enter_building(unit: *const u32, done: *const bool) {
            unsafe { super::F_ENTER_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_return_to_building(unit: *const u32, done: *const bool) {
            unsafe { super::F_RETURN_TO_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn execute_call(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> bool {
            unsafe { super::EXECUTE_CALL_DETOUR.call(this, call, unit, done) }
        }

        pub(crate) fn base_execute_call(this: *const u32, call: *const i8, unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::BFAIMGR_EXECUTE_CALL_DETOUR.call(this, call, unit, done) }
        }

        pub(crate) fn bfaimgr_update(this: *const u32, dt: u32) {
            unsafe { super::BFAIMGR_UPDATE_DETOUR.call(this, dt) }
        }

        pub(crate) fn ztaimgr_update(this: *const u32, dt: u32) {
            unsafe { super::ZTAIMGR_UPDATE_DETOUR.call(this, dt) }
        }

        pub(crate) fn bfaimgr_save(this: *const u32, file: *const i8) -> bool {
            unsafe { super::BFAIMGR_SAVE_DETOUR.call(this, file) }
        }

        pub(crate) fn bfaimgr_load_data(this: *const u32, full: bool) -> bool {
            unsafe { super::BFAIMGR_LOAD_DATA_DETOUR.call(this, full) }
        }

        pub(crate) fn bfaimgr_load(this: *const u32, file: *const u8, version: u32) -> u32 {
            unsafe { super::BFAIMGR_LOAD_DETOUR.call(this, file, version) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn bfaimgr_send_event(this: *const u32, kind: u16, target: u32, flag_b: u8, value_a: u32, flag_a: u8, value_b: u32, extra: u16, count: u16) {
            unsafe { super::BFAIMGR_SEND_EVENT_DETOUR.call(this, kind, target, flag_b, value_a, flag_a, value_b, extra, count) }
        }

        pub(crate) fn bfaimgr_get_events(this: *const u32, events: *const c_void, target: u32, kind: i8) {
            unsafe { super::BFAIMGR_GET_EVENTS_DETOUR.call(this, events, target, kind) }
        }

        pub(crate) fn bfaimgr_call_done(this: *const u32, entity: *const u32) -> bool {
            unsafe { super::BFAIMGR_CALL_DONE_DETOUR.call(this, entity) }
        }

        pub(crate) fn bfaimgr_remove_entities(this: *const u32) {
            unsafe { super::BFAIMGR_REMOVE_ENTITIES_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_get_path_tiles(this: *const u32, from: *const u32, to: *const u32, unit: *const u32, flag: bool) -> bool {
            unsafe { super::BFAIMGR_GET_PATH_1_DETOUR.call(this, from, to, unit, flag) }
        }

        pub(crate) fn ztaimgr_init_ai_params(this: *const u32) -> u32 {
            unsafe { super::ZTAIMGR_INIT_AIPARAMS_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_check_path(this: *const u32, from: *const u32, to: *const u32, unit: *const u32) -> u32 {
            unsafe { super::BFAIMGR_CHECK_PATH_DETOUR.call(this, from, to, unit) }
        }

        pub(crate) fn bfaimgr_init(this: *const u32) -> bool {
            unsafe { super::BFAIMGR_INIT_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_init_ai_params(this: *const u32) -> u32 {
            unsafe { super::BFAIMGR_INIT_AIPARAMS_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_construct(this: *const u32) -> *const u32 {
            unsafe { super::BFAIMGR_CONSTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) fn bfaimgr_destruct(this: *const u32) -> *const u32 {
            unsafe { super::BFAIMGR_DESTRUCTOR_0_DETOUR.call(this) }
        }

        pub(crate) fn ztaimgr_create() -> u32 {
            unsafe { super::ZTAIMGR_CREATE_DETOUR.call() }
        }

        pub(crate) fn f_exit_building(unit: *const i32, done: *const u8) {
            unsafe { super::F_EXIT_BUILDING_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_kill_prey(unit: *const u8, done: *const u8) -> u32 {
            unsafe { super::F_KILL_PREY_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_caught_prey(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_CAUGHT_PREY_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_die(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_DIE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_show_predator(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_SHOW_PREDATOR_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_hide_predator(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_HIDE_PREDATOR_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_hatch(unit: *const i32, done: *const u8) -> u32 {
            unsafe { super::F_HATCH_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_face_toward_food(unit: *const u8, done: *const u8) -> u32 {
            unsafe { super::F_FACE_TOWARD_FOOD_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_face_toward_target(unit: *const std::ffi::c_void, done: *const u8) -> u32 {
            unsafe { super::F_FACE_TOWARD_TARGET_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_guest_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_GUEST_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_maint_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_MAINT_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_guide_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_GUIDE_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_face_direction_f(call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_FACE_DIRECTION_F_DETOUR.call(call, unit) }
        }

        pub(crate) fn f_play_set_f(call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SET_F_DETOUR.call(call, unit) }
        }

        pub(crate) fn f_play_reverse_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_REVERSE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_ping_pong_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_PING_PONG_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_stop_at_end_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_STOP_AT_END_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_fly_reverse_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_FLY_REVERSE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_sound_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SOUND_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_time_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_TIME_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_entity_special_f(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
            unsafe { super::F_MOVE_ENTITY_SPECIAL_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_keeper_routine(unit: *const u32, done: *const bool) -> bool {
            unsafe { super::F_KEEPER_ROUTINE_DETOUR.call(unit, done) }
        }

        pub(crate) fn f_random_walk(this: *const u32, unit: *const i32) -> u32 {
            unsafe { super::F_RANDOM_WALK_DETOUR.call(this, unit) }
        }

        pub(crate) fn f_play_set_prob(entries: *const i32, unit: *const c_void) -> u32 {
            unsafe { super::F_PLAY_SET_PROB_DETOUR.call(entries, unit) }
        }

        pub(crate) fn f_move_f(this: *const u32, call: *const u32, unit: *const u32, reverse: bool) -> u32 {
            unsafe { super::F_MOVE_F_DETOUR.call(this, call, unit, reverse) }
        }

        pub(crate) fn f_move_with_sound_f(this: *const u32, call: *const u32, unit: *const u32) -> u32 {
            unsafe { super::F_MOVE_WITH_SOUND_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_slow_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_SLOW_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_medium_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_MEDIUM_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_move_fast_f(this: *const u32, call: i32, unit: *const i32) -> u32 {
            unsafe { super::F_MOVE_FAST_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_target_building(this: *const u32, unit: *const u32, done: *const u8) -> bool {
            unsafe { super::F_TARGET_BUILDING_DETOUR.call(this, unit, done) }
        }

        pub(crate) fn f_target_building_f(this: *const u32, call: i32, unit: *const i32) -> bool {
            unsafe { super::F_TARGET_BUILDING_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_set_base_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_SET_BASE_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_dust_ball(this: *const u32, unit: *const i32, done: *const i8) -> bool {
            unsafe { super::F_DUST_BALL_DETOUR.call(this, unit, done) }
        }

        pub(crate) fn f_dust_ball_f(this: *const u32, call: i32, unit: *const i32) -> bool {
            unsafe { super::F_DUST_BALL_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_set(name: *const i32, unit: *const c_void, child: i8, flag: u32) -> bool {
            unsafe { super::F_PLAY_SET_DETOUR.call(name, unit, child, flag) }
        }

        pub(crate) fn f_play_special_f(this: *const u32, call: *const u32, unit: *const u32) -> bool {
            unsafe { super::F_PLAY_SPECIAL_F_DETOUR.call(this, call, unit) }
        }

        pub(crate) fn f_play_special(this: *const u32, anim: *const i8, sound: *const i8, seconds: i32, count: i32, special: *const i8, unit: *const i32, done: *const u8) -> bool {
            unsafe { super::F_PLAY_SPECIAL_DETOUR.call(this, anim, sound, seconds, count, special, unit, done) }
        }

        pub(crate) fn f_play_f(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
            unsafe { super::F_PLAY_F_DETOUR.call(this, call, unit, reverse, ping_pong) }
        }

        pub(crate) fn f_play_with_base_f(this: *const u32, call: *const u32, unit: *const u32, reverse: i8, ping_pong: i8) -> bool {
            unsafe { super::F_PLAY_WITH_BASE_F_DETOUR.call(this, call, unit, reverse, ping_pong) }
        }

        pub(crate) fn f_play_time(this: *const u32, name: *const c_void, millis: i32, unit: *const u32, done: *const i8, flag_b: i8) -> bool {
            unsafe { super::F_PLAY_TIME_DETOUR.call(this, name, millis, unit, done, flag_b) }
        }

        pub(crate) fn f_play_with_sound(anim_name: *const c_void, sound_name: *const c_void, unit: *const u32, done: *const i8) -> bool {
            unsafe { super::F_PLAY_WITH_SOUND_DETOUR.call(anim_name, sound_name, unit, done) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn f_fly(this: *const u32, unit: *const u32, name: *const c_void, value: i32, extra: *const c_void, done: *const i8, reverse: i32, limit: i32) -> bool {
            unsafe { super::F_FLY_DETOUR.call(this, unit, name, value, extra, done, reverse, limit) }
        }

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn f_move(this: *const u32, first_param: i32, seconds: i32, unit: *const i32, done: *const u8, name: i32, speed: u32, extra: *const i8, reverse: u32, limit: u32) -> bool {
            unsafe { super::F_MOVE_DETOUR.call(this, first_param, seconds, unit, done, name, speed, extra, reverse, limit) }
        }
    }
}
