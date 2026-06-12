use fortress_core::*;
use std::path::Path;

fn content_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/events")
}

fn deck() -> Vec<Event> {
    content::load_events(&content_dir()).expect("content must parse")
}

#[test]
fn all_content_parses_with_enough_events() {
    let events = deck();
    assert!(events.len() >= 35, "expected >=35 events, got {}", events.len());
    for e in &events {
        assert!(!e.choices.is_empty(), "{} has no choices", e.name);
    }
}

#[test]
fn every_content_choice_resolves() {
    for event in deck() {
        for idx in 0..event.choices.len() {
            let mut gs = GameState::new(1);
            gs.fortress.name = "T".to_string();
            gs.resources.apply_delta(&ResourceDelta { food: 999, valuables: 999, stone: 999, wood: 999, gear: 999, tools: 999 });
            for role in Role::ALL {
                gs.inhabitants.add(Inhabitant::new(&format!("T-{}", role.name()), role));
            }
            resolve(&event, idx, &mut gs); // must not panic
        }
    }
}

#[test]
fn golden_deterministic_run() {
    let outcome_a = run_bot(42);
    let outcome_b = run_bot(42);
    assert_eq!(outcome_a, outcome_b, "same seed must produce identical runs");
    let outcome_c = run_bot(43);
    assert_ne!(outcome_a, outcome_c, "different seeds should diverge");
}

#[test]
fn save_load_continues_identically() {
    let deck = deck();
    let player = PlayerCharacter::new("Hero", ClassKind::Mystic, Stats::default());
    let mut gs = GameState::new_game(7, "Hold", player);
    let mut last: Option<String> = None;

    // play 5 days
    for _ in 0..5 {
        step_day(&deck, &mut gs, &mut last);
    }

    let tmp = std::env::temp_dir().join(format!("fortress_test_{}.json", std::process::id()));
    gs.save(&tmp).unwrap();
    let mut restored = GameState::load(&tmp).unwrap();
    std::fs::remove_file(&tmp).ok();

    // last_event_name is transient (not saved) — equalize before comparing futures
    let mut last_a = None;
    let mut last_b = None;
    for _ in 0..10 {
        step_day(&deck, &mut gs, &mut last_a);
        step_day(&deck, &mut restored, &mut last_b);
    }
    assert_eq!(
        serde_json::to_string(&gs).unwrap(),
        serde_json::to_string(&restored).unwrap()
    );
}

fn step_day(deck: &[Event], gs: &mut GameState, last: &mut Option<String>) {
    if gs.is_game_over() {
        return;
    }
    let day = gs.fortress.day;
    if let Some(event) = roll(deck, day, gs, last.as_deref()).cloned() {
        *last = Some(event.name.clone());
        if let Some(i) = (0..event.choices.len())
            .find(|&i| choice_availability(&event.choices[i], &event, gs) == ChoiceAvailability::Ok)
        {
            resolve(&event, i, gs);
        }
    }
    gs.apply_daily_effects();
    gs.fortress.advance_day();
}

fn run_bot(seed: u64) -> String {
    let deck = deck();
    let player = PlayerCharacter::new("Bot", ClassKind::Warlord, Stats { might: 8, wit: 3, heart: 3 });
    let mut gs = GameState::new_game(seed, "Hold", player);
    let mut last = None;
    while !gs.is_game_over() {
        step_day(&deck, &mut gs, &mut last);
        if gs.fortress.day > 50 { break; } // cap sim length for tests
    }
    serde_json::to_string(&gs).unwrap()
}

#[test]
fn serde_round_trip_byte_equal() {
    let player = PlayerCharacter::new("Hero", ClassKind::Steward, Stats::default());
    let gs = GameState::new_game(99, "Hold", player);
    let json_a = serde_json::to_string(&gs).unwrap();
    let restored: GameState = serde_json::from_str(&json_a).unwrap();
    let json_b = serde_json::to_string(&restored).unwrap();
    assert_eq!(json_a, json_b);
}

#[test]
fn old_schema_choices_parse_without_new_keys() {
    let json = r#"{
        "name": "Legacy",
        "description": "old event",
        "choices": [
            {"label": "Go", "description": "", "effects": [{"kind": "morale", "params": {"amount": 1}}]}
        ]
    }"#;
    let e: Event = serde_json::from_str(json).unwrap();
    assert!(e.choices[0].requires_stat.is_empty());
    assert!(e.choices[0].stat_check.is_none());
}
