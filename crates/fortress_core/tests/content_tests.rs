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
    assert!(events.len() >= 49, "expected >=49 events, got {}", events.len());
    for e in &events {
        assert!(!e.choices.is_empty(), "{} has no choices", e.name);
    }
}

#[test]
fn auto_events_have_exactly_one_choice() {
    for e in deck() {
        if e.auto {
            assert_eq!(
                e.choices.len(),
                1,
                "auto event {} must have exactly one (foregone) choice",
                e.name
            );
        }
    }
}

#[test]
fn flag_gates_are_reachable() {
    use std::collections::HashSet;
    let deck = deck();

    // every flag any choice/branch can raise
    let mut settable: HashSet<String> = HashSet::new();
    let mut collect = |effects: &[Effect], set: &mut HashSet<String>| {
        for e in effects {
            if let Effect::SetFlag { flag } = e {
                set.insert(flag.clone());
            }
        }
    };
    for ev in &deck {
        for c in &ev.choices {
            collect(&c.effects, &mut settable);
            if let Some(sc) = &c.stat_check {
                collect(&sc.success_effects, &mut settable);
                collect(&sc.failure_effects, &mut settable);
            }
        }
    }

    // every flag a gate depends on must be raisable somewhere — no dead arcs
    for ev in &deck {
        for f in ev.requires_flags.iter().chain(ev.forbids_flags.iter()) {
            assert!(
                settable.contains(f),
                "event {:?} gates on flag {:?} that no choice ever sets",
                ev.name,
                f
            );
        }
    }
}

#[test]
fn merchant_arc_plays_through() {
    let deck = deck();
    let find = |name: &str| deck.iter().find(|e| e.name == name).expect("arc event present").clone();

    let mut gs = GameState::new(1);
    gs.fortress.name = "T".to_string();
    gs.resources.apply_delta(&ResourceDelta { valuables: 50, ..Default::default() });

    // step 1: the plea is eligible up front; lending sets the debt flag
    let plea = find("The Merchant's Plea");
    assert!(eligible_events(std::slice::from_ref(&plea), 5, &gs, None).len() == 1);
    resolve(&plea, 0, &mut gs); // choice 0 = "Lend him the coin"
    assert!(gs.flags.contains("merchant_debt"));

    // step 2: the payoff was NOT eligible before the flag, IS now (day 12+)
    let returns = find("The Merchant Returns");
    assert_eq!(eligible_events(std::slice::from_ref(&returns), 12, &gs, None).len(), 1);
    let before = gs.resources.valuables;
    resolve(&returns, 0, &mut gs);
    assert!(gs.resources.valuables > before); // repaid with interest
    assert!(gs.flags.contains("merchant_repaid") && !gs.flags.contains("merchant_debt"));

    // step 3: with the debt closed, neither the plea nor the payoff recurs
    assert!(eligible_events(std::slice::from_ref(&plea), 13, &gs, None).is_empty());
    assert!(eligible_events(std::slice::from_ref(&returns), 13, &gs, None).is_empty());
}

#[test]
fn every_content_choice_resolves() {
    for event in deck() {
        for idx in 0..event.choices.len() {
            let mut gs = GameState::new(1);
            gs.fortress.name = "T".to_string();
            gs.resources.apply_delta(&ResourceDelta { food: 999, valuables: 999, stone: 999, wood: 999, ore: 999, ..Default::default() });
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

#[test]
fn auto_mode_plays_a_full_deterministic_run() {
    // The auto-picker drives a whole run with no panic, identically per seed.
    let auto_run = |seed: u64| {
        let deck = deck();
        let player = PlayerCharacter::new("Auto", ClassKind::Warlord, Stats { might: 7, wit: 4, heart: 3 });
        let mut gs = GameState::new_game(seed, "Hold", player);
        let mut last: Option<String> = None;
        while !gs.is_game_over() && gs.fortress.day <= 60 {
            let day = gs.fortress.day;
            if let Some(event) = roll(&deck, day, &mut gs, last.as_deref()).cloned() {
                last = Some(event.name.clone());
                if let Some(idx) = auto_pick(&event, &gs) {
                    resolve(&event, idx, &mut gs);
                }
            }
            gs.apply_daily_effects();
            gs.fortress.advance_day();
        }
        serde_json::to_string(&gs).unwrap()
    };
    assert_eq!(auto_run(7), auto_run(7), "auto-mode must replay identically");
    assert_ne!(auto_run(7), auto_run(8));
}

#[test]
fn an_auto_run_survives_well_past_the_turning_seasons() {
    // A hold left entirely to the engine — events *and* construction on auto —
    // should keep its feet well past the seasons turning, on a representative
    // seed. This guards the food / winter / morale balance against regression.
    let deck = deck();
    let player =
        PlayerCharacter::new("Auto", ClassKind::Steward, Stats { might: 5, wit: 5, heart: 4 });
    let mut gs = GameState::new_game(4, "Hold", player);
    let mut last: Option<String> = None;
    let target = 120;
    while !gs.is_game_over() && gs.fortress.day <= target {
        let day = gs.fortress.day;
        // a hold left to itself raises the next building it needs
        if let Some(upgrade) = gs.auto_build_pick() {
            let _ = gs.queue_build(upgrade);
        }
        if let Some(event) = roll(&deck, day, &mut gs, last.as_deref()).cloned() {
            last = Some(event.name.clone());
            if let Some(idx) = auto_pick(&event, &gs) {
                resolve(&event, idx, &mut gs);
            }
        }
        gs.apply_daily_effects();
        gs.fortress.advance_day();
    }
    assert!(
        !gs.is_game_over(),
        "a competently-run hold should survive to day {target} (it fell on day {} at morale {})",
        gs.fortress.day,
        gs.fortress.morale
    );
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
fn no_gear_or_tools_keys_linger_in_content() {
    // The Gear/Tools resources are retired (Stage 3). Because ResourceDelta
    // silently ignores unknown keys, a stray "gear"/"tools" param would parse to
    // nothing rather than fail — so guard against it at the raw-text level.
    for entry in std::fs::read_dir(content_dir()).expect("content dir") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"gear\""), "{:?} still references a gear resource", path);
        assert!(!text.contains("\"tools\""), "{:?} still references a tools resource", path);
    }
}

#[test]
fn migrated_grants_hand_over_named_items() {
    // The retired gear/tools grants became GrantItem — resolving the choice that
    // arms the guards should drop a real, named weapon in the armory.
    let deck = deck();
    let arm = deck
        .iter()
        .find(|e| e.choices.iter().any(|c| c.label == "Arm the guards"))
        .expect("the arm-the-guards event is present")
        .clone();
    let idx = arm.choices.iter().position(|c| c.label == "Arm the guards").unwrap();
    let mut gs = GameState::new(1);
    gs.fortress.name = "T".to_string();
    let before = gs.items.count();
    resolve(&arm, idx, &mut gs);
    assert_eq!(gs.items.count(), before + 1, "a weapon joins the armory");
    assert_eq!(gs.items.items.last().unwrap().kind, ItemKind::Weapon);
}

#[test]
fn equipped_loadouts_survive_a_round_trip() {
    let mut gs = GameState::new(5);
    gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats::default()));
    gs.inhabitants.add(Inhabitant::new("G", Role::Guard));
    gs.items.add(Item::new(ItemKind::Weapon, Quality::Fine));
    gs.items.add(Item::new(ItemKind::Armor, Quality::Plain));
    gs.redistribute_equipment();
    assert!(gs.player.as_ref().unwrap().loadout.weapon.is_some(), "the commander is armed");
    let json = serde_json::to_string(&gs).unwrap();
    let restored: GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(serde_json::to_string(&restored).unwrap(), json);
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
