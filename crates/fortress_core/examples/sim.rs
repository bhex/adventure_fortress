//! Headless simulation: plays a run picking the first available choice.
//! Usage: cargo run -p fortress_core --example sim [seed] [max_days]

use fortress_core::{
    choice_availability, content, resolve, roll, ChoiceAvailability, ClassKind, GameState,
    PlayerCharacter, Stats,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(42);
    let max_days: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);

    let dir = content::default_content_dir().expect("content dir");
    let deck = content::load_events(&dir).expect("load events");
    println!("Loaded {} events. Seed {seed}. Simulating up to {max_days} days.", deck.len());

    let player = PlayerCharacter::new(
        "Simbot",
        ClassKind::Steward,
        Stats { might: 5, wit: 5, heart: 4 },
    );
    let mut gs = GameState::new_game(seed, "Simhold", player);
    let mut last_event: Option<String> = None;

    while !gs.is_game_over() && gs.fortress.day <= max_days {
        let day = gs.fortress.day;
        if let Some(event) = roll(&deck, day, &mut gs, last_event.as_deref()).cloned() {
            last_event = Some(event.name.clone());
            let pick = (0..event.choices.len()).find(|&i| {
                choice_availability(&event.choices[i], &event, &gs) == ChoiceAvailability::Ok
            });
            match pick {
                Some(i) => {
                    let result = resolve(&event, i, &mut gs);
                    println!("Day {day}: {} -> {}", event.name, result.choice_label);
                    for line in &result.lines {
                        println!("    {line}");
                    }
                }
                None => println!("Day {day}: {} -> no available choice, the day passes", event.name),
            }
        } else {
            println!("Day {day}: a quiet day");
        }

        for line in gs.apply_daily_effects() {
            println!("    » {line}");
        }
        gs.fortress.advance_day();
    }

    println!();
    let outcome = if gs.is_game_over() { "FALLEN" } else { "STANDING" };
    println!(
        "{outcome} | days: {} | events: {} | alive: {} | dead: {} | morale: {}",
        gs.fortress.day - 1,
        gs.events_resolved,
        gs.inhabitants.count_alive(),
        gs.inhabitants.count_dead(),
        gs.fortress.morale,
    );
}
