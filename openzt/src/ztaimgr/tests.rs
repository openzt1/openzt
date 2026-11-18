use super::*;

#[test]
fn special_move_delta_rotates_by_direction_and_recenters() {
    assert_eq!(special_move_delta([3, 5, 9], 1, 0, 0), [3 + 0x20, 5 + 0x20, 9]);
    assert_eq!(special_move_delta([3, 5, 9], 2, 0, 0), [5 + 0x20, -3 + 0x20, 9]);
    assert_eq!(special_move_delta([3, 5, 9], 6, 0, 0), [-5 + 0x20, 3 + 0x20, 9]);
    assert_eq!(special_move_delta([3, 5, 9], 0, 0, 0), [-3 + 0x20, -5 + 0x20, 9]);
    // C remainder: negative positions keep their sign, and only the low six bits of magnitude count.
    assert_eq!(special_move_delta([0, 0, 0], 1, 70, -70), [-6 + 0x20, 6 + 0x20, 0]);
    assert_eq!(special_move_delta([0, 0, 0], 1, -64, 64), [0x20, 0x20, 0]);
}

#[test]
fn ai_params_use_defaults_keys_and_clamps() {
    let mut keys = Vec::new();
    let defaults = compute_ai_params(|key, default| {
        keys.push((key.to_string(), default));
        default
    });
    assert_eq!(defaults, [9, 9, 9, 9, 0, 0]);
    let expected: Vec<(String, i32)> = [("mRandomXGuest", 9), ("mRandomYGuest", 9), ("mRandomXStaff", 9), ("mRandomYStaff", 9), ("AnimalResetInterval", 0), ("GuestResetInterval", 0)]
        .iter()
        .map(|(key, default)| (key.to_string(), *default))
        .collect();
    assert_eq!(keys, expected);
    // Extents clamp up to 1, intervals up to 0, positive values pass through.
    assert_eq!(compute_ai_params(|_, _| -5), [1, 1, 1, 1, 0, 0]);
    assert_eq!(compute_ai_params(|_, _| 0), [1, 1, 1, 1, 0, 0]);
    assert_eq!(compute_ai_params(|_, _| 25), [25; 6]);
    let by_key = compute_ai_params(|key, _| match key {
        "mRandomXGuest" => 3,
        "mRandomYStaff" => -1,
        "GuestResetInterval" => 40,
        _ => 7,
    });
    assert_eq!(by_key, [3, 7, 7, 1, 7, 40]);
}

fn string_of(bytes: &'static [u8]) -> RawCallString {
    let range = bytes.as_ptr_range();
    RawCallString { begin: range.start, end: range.end, capacity: range.end, _live: LiveMemory::default() }
}

fn call_with(name: &'static [u8], params: &[RawCallString]) -> RawFunctionCall {
    let range = params.as_ptr_range();
    RawFunctionCall { name: string_of(name), params_begin: range.start, params_end: range.end, params_capacity: range.end, _live: LiveMemory::default() }
}

fn screaming_snake(camel: &str) -> String {
    let mut out = String::new();
    for (index, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

fn leading_ident(text: &str) -> &str {
    let end = text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(text.len());
    &text[..end]
}

/// The source between two markers, so the dispatch tables can be checked as written.
fn source_between(start: &str, end: &str) -> &'static str {
    let source: &'static str = include_str!("dispatch.rs");
    let from = source.find(start).unwrap_or_else(|| panic!("marker {start:?} not found"));
    let to = from + source[from..].find(end).unwrap_or_else(|| panic!("marker {end:?} not found"));
    &source[from..to]
}

#[test]
fn vanilla_handlers_invoke_the_entry_named_after_each_variant() {
    let region = source_between("impl Handlers for VanillaHandlers", "/// The handler for a `ZTAIMgr` call name.");
    let arms: Vec<&str> = region.split("Handler::").skip(1).collect();
    assert_eq!(arms.len(), 44, "one arm per Handler variant");
    for arm in arms {
        let variant = leading_ident(arm);
        let expected = format!("F_{}", screaming_snake(variant));
        let first_entry = arm
            .match_indices("F_")
            .map(|(at, _)| leading_ident(&arm[at..]))
            .find(|token| token.chars().all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit()))
            .unwrap_or_else(|| panic!("Handler::{variant} calls no F_ entry"));
        assert_eq!(first_entry, expected, "Handler::{variant} must invoke {expected}");
    }
}

#[test]
fn zt_handler_maps_each_call_to_its_own_handler() {
    let region = source_between("pub fn zt_handler(", "/// `ZTAIMgr::executeCall`: dispatches");
    let arms: Vec<&str> = region.split("ZtCall::").skip(1).collect();
    assert_eq!(arms.len(), 23);
    for arm in arms {
        let call = leading_ident(arm);
        let target = arm.split("Handler::").nth(1).map(leading_ident).unwrap_or_else(|| panic!("ZtCall::{call} has no handler"));
        assert_eq!(target, format!("{call}F"), "ZtCall::{call} must reach Handler::{call}F");
    }
}

#[test]
fn zt_call_lookup_covers_every_name_and_nothing_near() {
    let names: [(&[u8], ZtCall); 23] = [
        (b"fWalk", ZtCall::MoveSlow),
        (b"fSwim", ZtCall::MoveSlow),
        (b"fTrot", ZtCall::MoveMedium),
        (b"fRun", ZtCall::MoveFast),
        (b"fSpray", ZtCall::Spray),
        (b"fKeeperRoutine", ZtCall::KeeperRoutine),
        (b"fGuestRoutine", ZtCall::GuestRoutine),
        (b"fGuideRoutine", ZtCall::GuideRoutine),
        (b"fMaintRoutine", ZtCall::MaintRoutine),
        (b"fFaceTowardFood", ZtCall::FaceTowardFood),
        (b"fFaceTowardTarget", ZtCall::FaceTowardTarget),
        (b"fShowPredator", ZtCall::ShowPredator),
        (b"fHidePredator", ZtCall::HidePredator),
        (b"fEnterBuilding", ZtCall::EnterBuilding),
        (b"fExitBuilding", ZtCall::ExitBuilding),
        (b"fDustBall", ZtCall::DustBall),
        (b"fKillPrey", ZtCall::KillPrey),
        (b"fCaughtPrey", ZtCall::CaughtPrey),
        (b"fDie", ZtCall::Die),
        (b"fTargetBuilding", ZtCall::TargetBuilding),
        (b"fHatch", ZtCall::Hatch),
        (b"fEndTrick", ZtCall::EndTrick),
        (b"fReturnToBuilding", ZtCall::ReturnToBuilding),
    ];
    for (name, expected) in names {
        assert_eq!(lookup_zt_call(name), Some(expected), "{}", String::from_utf8_lossy(name));
        let mut longer = name.to_vec();
        longer.push(b'x');
        assert_eq!(lookup_zt_call(&longer), None);
        assert_eq!(lookup_zt_call(&name[..name.len() - 1]), None);
        assert_eq!(lookup_zt_call(&name.to_ascii_lowercase()), None);
    }
    assert_eq!(lookup_zt_call(b"fPlaySetTank"), Some(ZtCall::PlaySetTank));
    for name in [&b""[..], b"fMove", b"fPlay", b"fHalt", b"fMoveSlow", b"fWalkx"] {
        assert_eq!(lookup_zt_call(name), None);
    }
}

#[test]
fn base_call_lookup_covers_every_name_and_nothing_near() {
    let names: [(&[u8], BaseCall); 28] = [
        (b"fPlay", BaseCall::Play),
        (b"fSetBase", BaseCall::SetBase),
        (b"fPlayWithBase", BaseCall::PlayWithBase),
        (b"fMoveEntityDelta", BaseCall::MoveEntityDelta),
        (b"fMoveEntitySpecial", BaseCall::MoveEntitySpecial),
        (b"fPlayTime", BaseCall::PlayTime),
        (b"fPlaySound", BaseCall::PlaySound),
        (b"fPlaySpecial", BaseCall::PlaySpecial),
        (b"fPlaySp", BaseCall::PlaySpecial),
        (b"fPlaySetProb", BaseCall::PlaySetProb),
        (b"fPlaySetTerrain", BaseCall::PlaySetTerrain),
        (b"fPlayReverse", BaseCall::PlayReverse),
        (b"fPlayPingPong", BaseCall::PlayPingPong),
        (b"fPingPong", BaseCall::PlayPingPong),
        (b"fPlayWithSound", BaseCall::PlayWithSound),
        (b"fPlaySFX", BaseCall::PlayWithSound),
        (b"fPlayStopAtEnd", BaseCall::PlayStopAtEnd),
        (b"fPlaySet", BaseCall::PlaySet),
        (b"fMoveSlow", BaseCall::MoveSlow),
        (b"fMoveMedium", BaseCall::MoveMedium),
        (b"fMove", BaseCall::Move),
        (b"fMoveReverse", BaseCall::MoveReverse),
        (b"fMoveFast", BaseCall::MoveFast),
        (b"fMoveWithSound", BaseCall::MoveWithSound),
        (b"fFaceDirection", BaseCall::FaceDirection),
        (b"fFly", BaseCall::Fly),
        (b"fKeepControl", BaseCall::KeepControl),
        (b"fHalt", BaseCall::Halt),
    ];
    for (name, expected) in names {
        assert_eq!(lookup_base_call(name), Some(expected), "{}", String::from_utf8_lossy(name));
        assert_eq!(lookup_base_call(&name.to_ascii_lowercase()), None);
        let mut longer = name.to_vec();
        longer.push(b'x');
        assert!(lookup_base_call(&longer).is_none(), "{}", String::from_utf8_lossy(&longer));
    }
    assert_eq!(lookup_base_call(b"fFlyReverse"), Some(BaseCall::FlyReverse));
    // Neighbouring names that are prefixes of a handled name, or ZT-only names, are not base calls.
    for name in [&b""[..], b"fPla", b"fPlayS", b"fPlaySe", b"fMov", b"fWalk", b"fRun", b"fDie", b"fHal"] {
        assert_eq!(lookup_base_call(name), None, "{}", String::from_utf8_lossy(name));
    }
}

#[test]
fn move_function_names() {
    let none: [RawCallString; 0] = [];
    let call = |name: &'static [u8]| call_with(name, &none);
    assert!(call(b"fMoveSlow").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
    assert!(call(b"fWalk").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
    assert!(!call(b"fRun").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
    assert!(!call(b"fMove").is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk"));
}

#[test]
fn plain_move_matches_on_third_parameter() {
    let params = [string_of(b"x"), string_of(b"y"), string_of(b"trot")];
    let call = call_with(b"fMove", &params);
    assert!(call.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot"));
    assert!(!call.is_move_function(b"fMoveFast", &[b"fRun"], b"run"));
    let short = call_with(b"fMove", &params[..2]);
    assert!(!short.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot"));
}

/// The game's `rand()`: an LCG over one dword (`state * 0x343fd + 0x269ec3`, result bits 16..30).
struct GameRand(u32);

impl GameRand {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        (self.0 >> 16) & 0x7fff
    }
}

#[test]
fn game_rand_matches_the_documented_sequence() {
    let mut rand = GameRand(1);
    assert_eq!(rand.next(), 41);
    assert_eq!(rand.next(), 18467);
    assert_eq!(rand.next(), 6334);
}

use LinkKind::{Main, Tail};

fn timed(total: i32, duration: i32, ping_pong: bool, stop: bool) -> Vec<LinkKind> {
    timed_repeat(total, duration, ping_pong, stop).take(1000).collect()
}

#[test]
fn timed_repeat_fills_the_duration() {
    // An exact multiple: 300 ms of 100 ms links is the first link plus two more.
    assert_eq!(timed(300, 100, false, true), vec![Main, Main]);
    assert_eq!(timed(301, 100, false, true), vec![Main, Main, Main]);
    assert_eq!(timed(299, 100, false, true), vec![Main, Main]);
    // The duration is covered by the first link alone, or not at all.
    assert_eq!(timed(100, 100, false, true), vec![]);
    assert_eq!(timed(50, 100, false, true), vec![]);
    assert_eq!(timed(0, 100, false, true), vec![]);
    assert_eq!(timed(-5, 100, false, true), vec![]);
}

#[test]
fn timed_repeat_with_zero_duration_differs_between_callers() {
    // `fMove` and `fPlayTime` stop after one extra link; `fPlaySpecial` never ends (bounded here).
    assert_eq!(timed(500, 0, false, true), vec![Main]);
    assert_eq!(timed(0, 0, false, true), vec![]);
    assert_eq!(timed(-1, 0, false, true), vec![]);
    assert_eq!(timed(500, 0, false, false).len(), 1000);
    assert_eq!(timed(0, 0, false, false), vec![]);
    assert_eq!(timed(500, 0, true, false).len(), 1000);
}

#[test]
fn timed_repeat_ping_pong_subtracts_twice() {
    // 500 ms of 100 ms: the first main and tail are paid up front (500 - 200 = 300), then each pair costs 200.
    assert_eq!(timed(500, 100, true, false), vec![Main, Tail, Main, Tail]);
    assert_eq!(timed(300, 100, true, false), vec![Main, Tail]);
    assert_eq!(timed(200, 100, true, false), vec![]);
    assert_eq!(timed(150, 100, true, false), vec![]);
    // A pair is always appended whole, even when the remainder covers only its first half.
    assert_eq!(timed(301, 100, true, false), vec![Main, Tail]);
}

#[test]
fn timed_repeat_wraps_like_the_32_bit_arithmetic() {
    // `total - duration` wraps to a positive remainder for the most negative total, so it keeps appending.
    assert_eq!(timed(i32::MIN, 100, false, true).len(), 1000);
    // A negative duration only grows the remainder; the sequence is unbounded (bounded here).
    assert_eq!(timed(5, -1, false, true).len(), 1000);
}

#[test]
fn counted_repeat_appends_count_minus_one() {
    let counted = |count, ping_pong| counted_repeat(count, ping_pong).collect::<Vec<_>>();
    // `count - 1` wraps for the most negative count, which asks for 2^31 - 1 links (bounded here).
    assert_eq!(counted_repeat(i32::MIN, false).take(5).count(), 5);
    assert_eq!(counted(-3, false), vec![]);
    assert_eq!(counted(0, false), vec![]);
    assert_eq!(counted(1, false), vec![]);
    assert_eq!(counted(2, false), vec![Main]);
    assert_eq!(counted(4, false), vec![Main, Main, Main]);
    assert_eq!(counted(1, true), vec![]);
    assert_eq!(counted(3, true), vec![Main, Tail, Main, Tail]);
}

#[test]
fn set_prob_picker_follows_the_scripted_rolls() {
    let pick = |weights: &[i32], rolls: &[u32]| {
        let mut rolls = rolls.iter().copied();
        let picked = pick_set_prob(weights, || rolls.next().expect("rand called more often than scripted"));
        (picked, rolls.count())
    };
    // A roll is `rand % 100 + 1`; 0 -> 1, 29 -> 30.
    assert_eq!(pick(&[30, 70], &[0, 9]), (Some(0), 1));
    assert_eq!(pick(&[30, 70], &[29, 9]), (Some(0), 1));
    // 40 -> roll 41 misses entry 0 (weight 30); the remainder 70 rerolls `5 % 70 + 1 = 6`.
    assert_eq!(pick(&[30, 70], &[40, 5]), (Some(1), 0));
    assert_eq!(pick(&[10, 10, 80], &[95, 89, 79]), (Some(2), 0));
    // An entry with weight 0 is passed over, and a single entry always hits.
    assert_eq!(pick(&[0, 100], &[0, 0]), (Some(1), 0));
    assert_eq!(pick(&[100], &[99]), (Some(0), 0));
    // Weights that do not sum to 100, and no entries, roll nothing.
    assert_eq!(pick(&[30, 60], &[]), (None, 0));
    assert_eq!(pick(&[], &[]), (None, 0));
    assert_eq!(pick(&[60, 60], &[]), (None, 0));
    // A negative weight can make the sum 100; the first entry then covers every roll.
    assert_eq!(pick(&[110, -10], &[50]), (Some(0), 0));
}

#[test]
fn set_prob_picker_matches_the_weights_over_the_game_rand() {
    let weights = [20, 30, 50];
    let mut rand = GameRand(0x1234_5678);
    let mut counts = [0u32; 3];
    let draws = 20_000u32;
    for _ in 0..draws {
        let picked = pick_set_prob(&weights, || rand.next()).expect("weights sum to 100, so something is always picked");
        counts[picked] += 1;
    }
    for (count, weight) in counts.iter().zip(weights) {
        let expected = draws * weight as u32 / 100;
        assert!(count.abs_diff(expected) < draws / 25, "{counts:?} against {weights:?}");
    }
}

fn facts(f: impl FnOnce(&mut PlaySpecialFFacts)) -> PlaySpecialFFacts {
    let mut facts = PlaySpecialFFacts::default();
    f(&mut facts);
    facts
}

#[test]
fn play_special_f_decisions() {
    use NoSoundPlay as N;
    use PlaySpecialFAction as A;
    let decide = decide_play_special_f;
    assert_eq!(decide(facts(|f| f.seconds = true)), A::PlaySpecial { use_seconds: true, use_repeat: false });
    assert_eq!(decide(facts(|f| (f.seconds, f.repeat) = (true, true))), A::Fail);
    assert_eq!(decide(facts(|f| f.repeat = true)), A::PlaySpecial { use_seconds: false, use_repeat: true });
    // A repeat count wins over every string predicate.
    assert_eq!(decide(facts(|f| (f.repeat, f.sound_none, f.special_reverse) = (true, true, true))), A::PlaySpecial { use_seconds: false, use_repeat: true });
    // A duration with the "none" sound and a plain special becomes `fPlayTime`; the second flag follows mode B.
    assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_mode_a) = (true, true, true))), A::PlayTime { second_flag: false });
    assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_mode_b) = (true, true, true))), A::PlayTime { second_flag: true });
    assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_none) = (true, true, true))), A::PlayTime { second_flag: false });
    // Not plain, or a named sound: `fPlaySpecial` with the duration.
    assert_eq!(decide(facts(|f| (f.seconds, f.sound_none, f.special_reverse) = (true, true, true))), A::PlaySpecial { use_seconds: true, use_repeat: false });
    assert_eq!(decide(facts(|f| (f.seconds, f.special_mode_a) = (true, true))), A::PlaySpecial { use_seconds: true, use_repeat: false });
    // Neither number, the "none" sound.
    assert_eq!(decide(facts(|f| (f.sound_none, f.special_mode_a) = (true, true))), A::NoSound(N::Plain));
    assert_eq!(decide(facts(|f| (f.sound_none, f.special_none) = (true, true))), A::NoSound(N::Plain));
    assert_eq!(decide(facts(|f| (f.sound_none, f.special_reverse) = (true, true))), A::NoSound(N::Reverse));
    assert_eq!(decide(facts(|f| (f.sound_none, f.special_ping_pong) = (true, true))), A::NoSound(N::PingPong));
    assert_eq!(decide(facts(|f| f.sound_none = true)), A::NoSound(N::Other));
    // A plain mode wins over reverse and ping-pong when a string matches several.
    assert_eq!(decide(facts(|f| (f.sound_none, f.special_mode_a, f.special_reverse) = (true, true, true))), A::NoSound(N::Plain));
    // A named sound: with a named animation `fPlayWithSound`, with the "none" animation `fPlaySpecial`.
    assert_eq!(decide(facts(|_| {})), A::PlayWithSound);
    assert_eq!(decide(facts(|f| f.anim_none = true)), A::PlaySpecialNoAnim { second_flag: false });
    assert_eq!(decide(facts(|f| (f.anim_none, f.special_mode_b) = (true, true))), A::PlaySpecialNoAnim { second_flag: true });
}

#[test]
fn play_special_f_decision_invariants_over_every_combination() {
    for bits in 0u32..512 {
        let f = PlaySpecialFFacts {
            seconds: bits & 1 != 0,
            repeat: bits & 2 != 0,
            sound_none: bits & 4 != 0,
            anim_none: bits & 8 != 0,
            special_mode_a: bits & 16 != 0,
            special_mode_b: bits & 32 != 0,
            special_none: bits & 64 != 0,
            special_reverse: bits & 128 != 0,
            special_ping_pong: bits & 256 != 0,
        };
        let action = decide_play_special_f(f);
        assert_eq!(action == PlaySpecialFAction::Fail, f.seconds && f.repeat, "{f:?}");
        if f.repeat && !f.seconds {
            assert_eq!(action, PlaySpecialFAction::PlaySpecial { use_seconds: false, use_repeat: true }, "{f:?}");
        }
        if !f.seconds && !f.repeat && f.sound_none {
            assert!(matches!(action, PlaySpecialFAction::NoSound(_)), "{f:?}");
        }
        if !f.seconds && !f.repeat && !f.sound_none {
            assert_eq!(matches!(action, PlaySpecialFAction::PlayWithSound), !f.anim_none, "{f:?}");
        }
        if let PlaySpecialFAction::PlayTime { second_flag } = action {
            assert!(f.seconds && !f.repeat && f.sound_none && (f.special_mode_a || f.special_mode_b || f.special_none), "{f:?}");
            assert_eq!(second_flag, f.special_mode_b);
        }
    }
}

#[test]
fn random_walk_border_is_walked_in_column_order() {
    let border = |hx, hy| border_offsets(hx, hy).collect::<Vec<_>>();
    assert_eq!(border(1, 1), vec![(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]);
    assert_eq!(border(0, 0), vec![(0, 0)]);
    assert_eq!(border(2, 1), vec![(-2, -1), (-2, 0), (-2, 1), (-1, -1), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 1), (2, -1), (2, 0), (2, 1)]);
    for (hx, hy) in [(1, 1), (2, 3), (5, 2), (4, 4), (1, 6)] {
        let offsets = border(hx, hy);
        let interior = ((2 * hx - 1).max(0) * (2 * hy - 1).max(0)) as usize;
        assert_eq!(offsets.len(), ((2 * hx + 1) * (2 * hy + 1)) as usize - interior, "({hx},{hy})");
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]), "({hx},{hy}) is dx-major, dy ascending");
        assert!(offsets.iter().all(|&(dx, dy)| dx.abs() == hx || dy.abs() == hy));
    }
}

#[test]
fn random_walk_neighbour_scan_starts_at_the_roll_and_does_not_wrap() {
    assert_eq!(neighbor_scan(0, 4).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert_eq!(neighbor_scan(2, 4).collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(neighbor_scan(3, 4).collect::<Vec<_>>(), vec![3]);
    assert_eq!(neighbor_scan(0, 1).collect::<Vec<_>>(), vec![0]);
}

#[test]
fn route_tables_are_total_and_distinct() {
    let zt_names: [&[u8]; 24] = [
        b"fWalk", b"fSwim", b"fTrot", b"fRun", b"fSpray", b"fKeeperRoutine", b"fGuestRoutine", b"fGuideRoutine", b"fMaintRoutine", b"fFaceTowardFood", b"fFaceTowardTarget", b"fShowPredator", b"fHidePredator", b"fEnterBuilding", b"fExitBuilding", b"fDustBall",
        b"fKillPrey", b"fCaughtPrey", b"fDie", b"fTargetBuilding", b"fHatch", b"fEndTrick", b"fReturnToBuilding", b"fPlaySetTank",
    ];
    let mut zt_handlers: Vec<Handler> = zt_names.iter().map(|name| zt_handler(lookup_zt_call(name).unwrap())).collect();
    zt_handlers.sort_by_key(|h| *h as u32);
    zt_handlers.dedup();
    // `fWalk`/`fSwim` share a handler; every other name has its own.
    assert_eq!(zt_handlers.len(), 23);

    let base_names: [&[u8]; 28] = [
        b"fPlay", b"fSetBase", b"fPlayWithBase", b"fMoveEntityDelta", b"fMoveEntitySpecial", b"fPlayTime", b"fPlaySound", b"fPlaySpecial", b"fPlaySp", b"fPlaySetProb", b"fPlaySetTerrain", b"fPlayReverse", b"fPlayPingPong", b"fPingPong", b"fPlayWithSound", b"fPlaySFX",
        b"fPlayStopAtEnd", b"fPlaySet", b"fMoveSlow", b"fMoveMedium", b"fMove", b"fMoveReverse", b"fMoveFast", b"fMoveWithSound", b"fFaceDirection", b"fFly", b"fFlyReverse", b"fKeepControl",
    ];
    let mut routes = Vec::new();
    let mut unrouted = Vec::new();
    for name in base_names.iter().chain([&b"fHalt"[..]].iter()) {
        let which = lookup_base_call(name).unwrap();
        match base_route(which) {
            Some(route) => routes.push(route.handler),
            None => unrouted.push(which),
        }
    }
    assert_eq!(unrouted, vec![BaseCall::PlaySetTerrain, BaseCall::KeepControl]);
    routes.sort_by_key(|h| *h as u32);
    routes.dedup();
    // Three aliases share handlers (`fPlaySp`, `fPingPong`, `fPlaySFX`).
    assert_eq!(routes.len(), 29 - 2 - 3);
    // The trailing constants the vanilla chain passes (`fPlayF` `0, 0`, `fMoveF` `false`, `fFlyF` `0`) are all zero.
    assert!(base_names.iter().all(|name| base_route(lookup_base_call(name).unwrap()).map_or(true, |r| r.trailing == [0, 0])));
}

#[test]
fn bf_ai_params_use_defaults_keys_and_clamps() {
    let mut keys = Vec::new();
    let defaults = compute_bf_ai_params(|key, default| {
        keys.push((key.to_string(), default));
        default
    });
    let expected: Vec<(&str, i32)> = vec![
        ("BaseX", 1), ("BaseY", 1), ("RandomX", 5), ("RandomY", 5), ("mAIHowOften", 1), ("mAITick", 1000), ("mStatusPeriod", 1000), ("mGoalPeriod", 0), ("ShowAIInfo", 0), ("ShowName", 1), ("ShowPosition", 1), ("ShowStatusVars", 1), ("ShowFunctionCall", 1),
        ("ShowEvents", 1), ("AIInfoNth", 100), ("ShowSelected", 0), ("ShowFrame", 0), ("ShowGoal", 0), ("VariableUnitSpeeds", 1), ("AllowDiagonalMoves", 1), ("ClearUnreachableFreq", 5), ("MinUnreachableCount", 5),
    ];
    assert_eq!(keys, expected.iter().map(|(key, default)| (key.to_string(), *default)).collect::<Vec<_>>());
    assert_eq!(
        defaults,
        BfAiParams {
            base_x: 1, base_y: 1, random_x: 5, random_y: 5, how_often: 1, tick: 1000, status_period: 1000, goal_period: 0, show_ai_info: 0, show_name: 1, show_position: 1, show_status_vars: 1, show_function_call: 1, show_events: 1, ai_info_nth: 100,
            show_selected: 0, show_frame: 0, show_goal: 0, variable_unit_speeds: 1, allow_diagonal_moves: true, clear_unreachable_freq: 5, min_unreachable_count: 5,
        }
    );
    // The base extents clamp up to 0, the random extents up to 1; positive values pass through.
    let clamped = compute_bf_ai_params(|_, _| -7);
    assert_eq!((clamped.base_x, clamped.base_y, clamped.random_x, clamped.random_y), (0, 0, 1, 1));
    let zero = compute_bf_ai_params(|_, _| 0);
    assert_eq!((zero.base_x, zero.random_x, zero.allow_diagonal_moves), (0, 1, false));
    let big = compute_bf_ai_params(|_, _| 300);
    assert_eq!((big.base_x, big.random_y, big.tick, big.clear_unreachable_freq), (300, 300, 300, 300));
    // Unclamped fields keep negative values, `AllowDiagonalMoves` is true only for exactly 1, and the
    // unreachable count keeps its low byte.
    assert_eq!((clamped.tick, clamped.ai_info_nth, clamped.clear_unreachable_freq), (-7, -7, -7));
    assert_eq!(compute_bf_ai_params(|key, d| if key == "AllowDiagonalMoves" { 2 } else { d }).allow_diagonal_moves, false);
    assert_eq!(compute_bf_ai_params(|key, d| if key == "MinUnreachableCount" { 0x1ff } else { d }).min_unreachable_count, 0xff);
    let by_key = compute_bf_ai_params(|key, d| match key {
        "RandomY" => 8,
        "mGoalPeriod" => 42,
        _ => d,
    });
    assert_eq!((by_key.random_x, by_key.random_y, by_key.goal_period, by_key.status_period), (5, 8, 42, 1000));
}
