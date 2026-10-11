//! Redirects `ZTHabitat::generateFaces` (`0x004d9953`, via [`intercept`] from the port's own detour in
//! `hooks_zthabitatmgr`, since two detours cannot share an address), `ZTAnimal::FUN_004d9a2f` and the two
//! `ZTWorldMgr::playSmileSound`/`playFrownSound` stubs into one ordered in-memory record, so
//! `ZTHABITAT_AFTER_ENTITY_CHANGE_MATCHES_REAL_LIVE` can compare which faces and sounds real vanilla
//! `afterEntityChange` and the port each request - without spawning faces or playing audio whose state
//! would couple the two runs. `generateFaces` returns a scripted value derived from its arguments. Same
//! thread-local capture-window shape as `send_event_recorder.rs`; the port reaches these detours through
//! `.hooked()`.

use std::cell::{Cell, RefCell};

use openzt_detour::{
    generated::{
        ztworldmgr::{PLAY_FROWN_SOUND, PLAY_SMILE_SOUND},
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;

/// Vanilla `afterEntityChange` reaches the world manager's sounds through its own inline thunks
/// (`MOV EAX,[ECX+0x1e0]; ADD ECX,0x1e0; JMP [EAX+0x2c]` and the `+0x1e8` twin) rather than through
/// `ZTWorldMgr::playSmileSound`/`playFrownSound` (`0x004dbf90`/`0x005acf8a`), which have the same bodies at
/// other addresses. Hooking these two lets the recorder see the real function's sounds.
const ANIMAL_SHOW_FACE_FN: FunctionDef<unsafe extern "thiscall" fn(*const u32, bool)> = crate::zthabitat::habitat::ANIMAL_SHOW_FACE;
const AFTER_ENTITY_CHANGE_SMILE_THUNK: FunctionDef<unsafe extern "thiscall" fn(*const u32)> = FunctionDef::new(0x004d954a);
const AFTER_ENTITY_CHANGE_FROWN_THUNK: FunctionDef<unsafe extern "thiscall" fn(*const u32)> = FunctionDef::new(0x004d9573);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    GenerateFaces { habitat: u32, species_type: u32, smile: bool },
    AnimalFace { animal: u32, smile: bool },
    SmileSound { world: u32 },
    FrownSound { world: u32 },
}

thread_local! {
    static REDIRECT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ANIMAL_FACE_CAPTURE_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static RECORDED: RefCell<Vec<Recorded>> = const { RefCell::new(Vec::new()) };
    static SCRIPTED_TRUE_RESULTS: Cell<bool> = const { Cell::new(false) };
}

/// The value a captured `generateFaces` call returns when scripted-true results are on (otherwise always
/// `false`): a pure function of its arguments, so both sides of a comparison see the same flags.
fn scripted_result(species_type: u32, smile: bool) -> bool {
    ((species_type >> 4) as u8 ^ smile as u8) & 1 == 1
}

pub fn begin_capture(scripted_true_results: bool) {
    SCRIPTED_TRUE_RESULTS.with(|s| s.set(scripted_true_results));
    RECORDED.with(|r| r.borrow_mut().clear());
    REDIRECT_ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<Recorded> {
    REDIRECT_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Records (instead of executing) every per-animal face request `generateFaces` makes, leaving
/// `generateFaces` itself un-intercepted - for comparing real vs port bodies.
pub fn begin_animal_face_capture() {
    RECORDED.with(|r| r.borrow_mut().clear());
    ANIMAL_FACE_CAPTURE_ACTIVE.with(|a| a.set(true));
}

pub fn end_animal_face_capture() -> Vec<Recorded> {
    ANIMAL_FACE_CAPTURE_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Called first by the `GENERATE_FACES` detour: while a capture window is open, records the request
/// and returns the scripted value instead of running `generateFaces`.
pub fn intercept(habitat: u32, species_type: u32, smile: bool) -> Option<bool> {
    if !REDIRECT_ACTIVE.with(|a| a.get()) {
        return None;
    }
    record(Recorded::GenerateFaces { habitat, species_type, smile });
    Some(SCRIPTED_TRUE_RESULTS.with(|s| s.get()) && scripted_result(species_type, smile))
}

fn record(event: Recorded) {
    RECORDED.with(|r| r.borrow_mut().push(event));
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(ANIMAL_SHOW_FACE_FN)]
    unsafe extern "thiscall" fn animal_show_face(this: *const u32, smile: bool) {
        if ANIMAL_FACE_CAPTURE_ACTIVE.with(|a| a.get()) {
            record(Recorded::AnimalFace { animal: this as u32, smile });
            return;
        }
        unsafe { ANIMAL_SHOW_FACE_FN_DETOUR.call(this, smile) }
    }

    #[detour(PLAY_SMILE_SOUND)]
    unsafe extern "thiscall" fn play_smile_sound(this: *const u32) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            record(Recorded::SmileSound { world: this as u32 });
            return;
        }
        unsafe { PLAY_SMILE_SOUND_DETOUR.call(this) }
    }

    #[detour(PLAY_FROWN_SOUND)]
    unsafe extern "thiscall" fn play_frown_sound(this: *const u32) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            record(Recorded::FrownSound { world: this as u32 });
            return;
        }
        unsafe { PLAY_FROWN_SOUND_DETOUR.call(this) }
    }

    #[detour(AFTER_ENTITY_CHANGE_SMILE_THUNK)]
    unsafe extern "thiscall" fn after_entity_change_smile_thunk(this: *const u32) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            record(Recorded::SmileSound { world: this as u32 });
            return;
        }
        unsafe { AFTER_ENTITY_CHANGE_SMILE_THUNK_DETOUR.call(this) }
    }

    #[detour(AFTER_ENTITY_CHANGE_FROWN_THUNK)]
    unsafe extern "thiscall" fn after_entity_change_frown_thunk(this: *const u32) {
        if REDIRECT_ACTIVE.with(|a| a.get()) {
            record(Recorded::FrownSound { world: this as u32 });
            return;
        }
        unsafe { AFTER_ENTITY_CHANGE_FROWN_THUNK_DETOUR.call(this) }
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise generate_faces_recorder detours: {e:?}");
    }
}
