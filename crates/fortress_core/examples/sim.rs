//! Headless simulation: plays a run with the same auto-mode picker the game
//! uses, so this doubles as a Progress-Quest-style auto-play demo and a survival
//! harness for balance work.
//!
//! Usage:
//!   cargo run -p fortress_core --example sim [seed] [max_days]   # one verbose run
//!   cargo run -p fortress_core --example sim band [max_days]     # survival sweep

use fortress_core::{
    auto_pick, content, resolve, roll, ClassKind, Event, GameState, PlayerCharacter, Stats,
};

/// How a run ended, for the survival harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Survived,
    Starved,
    Despair,
    CommanderFell,
}

impl Cause {
    fn label(&self) -> &'static str {
        match self {
            Cause::Survived => "survived",
            Cause::Starved => "starved",
            Cause::Despair => "morale-0",
            Cause::CommanderFell => "commander fell",
        }
    }
}

struct Outcome {
    days: u32,
    cause: Cause,
    alive: usize,
    morale: i32,
    darkness: i32,
}

/// Drive one full run under auto-mode (events *and* construction), quietly unless
/// `verbose`. Returns how it ended.
fn run(deck: &[Event], seed: u64, max_days: u32, verbose: bool) -> Outcome {
    let player = PlayerCharacter::new("Simbot", ClassKind::Steward, Stats { might: 5, wit: 5, heart: 4 });
    let mut gs = GameState::new_game(seed, "Simhold", player);
    let mut last_event: Option<String> = None;
    // remember whether food ever hit zero, to tell starvation from plain despair
    let mut ever_starved = false;

    while !gs.is_game_over() && gs.fortress.day <= max_days {
        let day = gs.fortress.day;

        // Dawn: a hold left to itself raises the next building it needs.
        if let Some(upgrade) = gs.auto_build_pick() {
            if let Ok(line) = gs.queue_build(upgrade) {
                if verbose {
                    println!("Day {day}: {line}");
                }
            }
        }

        if let Some(event) = roll(deck, day, &mut gs, last_event.as_deref()).cloned() {
            last_event = Some(event.name.clone());
            match auto_pick(&event, &gs) {
                Some(i) => {
                    let result = resolve(&event, i, &mut gs);
                    if verbose {
                        println!("Day {day}: {} -> {}", event.name, result.choice_label);
                        for line in &result.lines {
                            println!("    {line}");
                        }
                    }
                }
                None if verbose => {
                    println!("Day {day}: {} -> no available choice, the day passes", event.name)
                }
                None => {}
            }
        } else if verbose {
            println!("Day {day}: a quiet day");
        }

        for line in gs.apply_daily_effects() {
            if line.starts_with("Not enough food") {
                ever_starved = true;
            }
            if verbose {
                println!("    » {line}");
            }
        }
        gs.fortress.advance_day();
    }

    let cause = if !gs.is_game_over() {
        Cause::Survived
    } else if gs.commander_has_fallen() {
        Cause::CommanderFell
    } else if ever_starved {
        Cause::Starved
    } else {
        Cause::Despair
    };
    Outcome {
        days: gs.fortress.day - 1,
        cause,
        alive: gs.inhabitants.count_alive(),
        morale: gs.fortress.morale,
        darkness: gs.region.darkness,
    }
}

fn main() {
    let dir = content::default_content_dir().expect("content dir");
    let deck = content::load_events(&dir).expect("load events");

    let mut args = std::env::args().skip(1);
    let first = args.next();

    // Survival sweep: `sim band [max_days]` runs a band of seeds and tallies how
    // they ended — the data behind balance tuning.
    if first.as_deref() == Some("band") {
        let max_days: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(150);
        println!("Survival sweep over seeds 1..=20, up to {max_days} days:\n");
        let mut survived = 0;
        let mut total_days = 0u32;
        for seed in 1..=20u64 {
            let o = run(&deck, seed, max_days, false);
            if o.cause == Cause::Survived {
                survived += 1;
            }
            total_days += o.days;
            println!(
                "  seed {seed:>2}: {:>8} | day {:>3} | alive {:>2} | morale {:>3} | darkness {:>3}",
                o.cause.label(),
                o.days,
                o.alive,
                o.morale,
                o.darkness,
            );
        }
        println!(
            "\n{survived}/20 survived to day {max_days}; mean run length {} days.",
            total_days / 20
        );
        return;
    }

    let seed: u64 = first.and_then(|s| s.parse().ok()).unwrap_or(42);
    let max_days: u32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);
    println!("Loaded {} events. Seed {seed}. Simulating up to {max_days} days.\n", deck.len());
    let o = run(&deck, seed, max_days, true);
    println!();
    println!(
        "{} | days: {} | cause: {} | alive: {} | morale: {} | darkness: {}",
        if o.cause == Cause::Survived { "STANDING" } else { "FALLEN" },
        o.days,
        o.cause.label(),
        o.alive,
        o.morale,
        o.darkness,
    );
}
