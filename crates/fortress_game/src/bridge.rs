//! Bevy resources wrapping the pure fortress_core state, plus the shared
//! end-of-day pipeline used by both the quiet-day path and the modal path.

use bevy::prelude::*;
use std::collections::VecDeque;
use std::path::PathBuf;

use fortress_core::{Event, GameState};

pub const SAVE_FILE: &str = "save.json";

pub fn save_path() -> PathBuf {
    PathBuf::from(SAVE_FILE)
}

#[derive(Resource)]
pub struct Game(pub GameState);

#[derive(Resource)]
pub struct EventDeck(pub Vec<Event>);

#[derive(Resource, Default)]
pub struct EngineCtl {
    pub last_event_name: Option<String>,
    /// Rolled at dawn, fired later in the day by the clock.
    pub pending_event: Option<Event>,
}

/// Progress-Quest-style auto-play: when on, the clock keeps running and events
/// resolve themselves by `auto_pick` — no modal, no pausing for a decision.
#[derive(Resource, Default)]
pub struct AutoMode(pub bool);

#[derive(Resource, Default)]
pub struct GameLog(pub VecDeque<String>);

impl GameLog {
    pub fn push(&mut self, line: impl Into<String>) {
        self.0.push_back(line.into());
        while self.0.len() > 60 {
            self.0.pop_front();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Selection {
    Inhabitant(String),
    Commander,
    Hero(String),
    Building(fortress_core::Upgrade),
    Keep,
    Gate,
}

#[derive(Resource, Default)]
pub struct Selected(pub Option<Selection>);

#[derive(Resource)]
pub struct ActiveEvent {
    pub event: Event,
    pub availability: Vec<fortress_core::ChoiceAvailability>,
}

/// Day-end pipeline: daily tick → advance → autosave.
/// Returns the next AppState. The fortress falls only at morale 0 — there is no victory.
pub fn finish_day(game: &mut GameState, log: &mut GameLog) -> crate::AppState {
    for line in game.apply_daily_effects() {
        log.push(format!("» {line}"));
    }
    game.fortress.advance_day();

    if game.is_game_over() {
        let _ = std::fs::remove_file(save_path());
        return crate::AppState::GameOver;
    }
    if let Err(e) = game.save(&save_path()) {
        log.push(format!("(autosave failed: {e})"));
    }
    crate::AppState::FortressView
}
