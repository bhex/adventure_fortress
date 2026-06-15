//! Real-time presentation of the turn-based core: a continuous clock drives
//! each game day. Dawn rolls the day's event (held as PendingEvent, fired at a
//! UI-chosen hour), midnight runs the shared finish_day pipeline. The core
//! stays deterministic — presentation timing never touches GameState.rng.

use bevy::prelude::*;
use rand::Rng;

use fortress_core::{auto_pick, resolve, roll};

use crate::bridge::{finish_day, ActiveEvent, AutoMode, EngineCtl, EventDeck, Game, GameLog};
use crate::AppState;

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameClock>()
            .add_systems(OnEnter(AppState::FortressView), resume_clock)
            .add_systems(OnExit(AppState::FortressView), pause_clock)
            .add_systems(
                Update,
                (tick_clock, speed_hotkeys, auto_toggle).run_if(in_state(AppState::FortressView)),
            );
    }
}

pub const DAWN: f32 = 6.0;
pub const DUSK_START: f32 = 19.0;
pub const NIGHT_START: f32 = 21.0;
/// Real seconds for one full game day at Normal speed. Long enough that the
/// dusk window gives wandering actors time to actually reach their beds.
const DAY_SECONDS_NORMAL: f32 = 100.0;
const FAST_MULTIPLIER: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockSpeed {
    Paused,
    Normal,
    Fast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayPhase {
    Dawn,
    Day,
    Dusk,
    Night,
}

#[derive(Resource)]
pub struct GameClock {
    pub hour: f32,
    pub speed: ClockSpeed,
    /// Speed to restore when returning from a modal pause.
    resume_speed: ClockSpeed,
    /// Hour at which today's pending event fires, if one was rolled.
    pub event_hour: Option<f32>,
    /// Whether dawn duties (event roll) ran for the current day.
    dawn_done: bool,
    /// Fast-forwarding to the next dawn (QoL skip); clears at dawn.
    pub skipping: bool,
}

impl Default for GameClock {
    fn default() -> GameClock {
        GameClock {
            hour: DAWN,
            speed: ClockSpeed::Normal,
            resume_speed: ClockSpeed::Normal,
            event_hour: None,
            dawn_done: false,
            skipping: false,
        }
    }
}

impl GameClock {
    pub fn phase(&self) -> DayPhase {
        if self.hour < DAWN || self.hour >= NIGHT_START {
            DayPhase::Night
        } else if self.hour < 8.0 {
            DayPhase::Dawn
        } else if self.hour < DUSK_START {
            DayPhase::Day
        } else {
            DayPhase::Dusk
        }
    }

    /// How fast the wandering actors should move, tracking clock speed so the
    /// fortress looks busier when time runs faster. Skip-to-dawn is capped so
    /// the crowd stays readable rather than blinking across the map.
    pub fn actor_speed_mult(&self) -> f32 {
        if self.skipping {
            return 4.0;
        }
        match self.speed {
            ClockSpeed::Paused => 0.0,
            ClockSpeed::Normal => 1.0,
            ClockSpeed::Fast => FAST_MULTIPLIER,
        }
    }

    pub fn readout(&self) -> String {
        let h = self.hour as u32;
        let m = ((self.hour - h as f32) * 60.0) as u32;
        format!("{h:02}:{m:02}")
    }

    /// Fast-forward to the next dawn. The day still flows through tick_clock,
    /// so pending events fire (pausing the skip) and midnight runs finish_day.
    pub fn skip_to_dawn(&mut self) {
        self.skipping = true;
    }
}

// Modals freeze time transparently: exact speed (even an explicit user pause)
// is saved on exit and restored on return.
fn resume_clock(mut clock: ResMut<GameClock>) {
    clock.speed = clock.resume_speed;
}

fn pause_clock(mut clock: ResMut<GameClock>) {
    clock.resume_speed = clock.speed;
    clock.speed = ClockSpeed::Paused;
}

#[allow(clippy::too_many_arguments)]
fn tick_clock(
    time: Res<Time>,
    mut commands: Commands,
    mut clock: ResMut<GameClock>,
    deck: Res<EventDeck>,
    mut game: ResMut<Game>,
    mut ctl: ResMut<EngineCtl>,
    mut log: ResMut<GameLog>,
    auto: Res<AutoMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let rate = if clock.skipping {
        24.0 * 8.0 / DAY_SECONDS_NORMAL
    } else {
        match clock.speed {
            ClockSpeed::Paused => return,
            ClockSpeed::Normal => 24.0 / DAY_SECONDS_NORMAL,
            ClockSpeed::Fast => 24.0 * FAST_MULTIPLIER / DAY_SECONDS_NORMAL,
        }
    };
    clock.hour += time.delta_secs() * rate;

    // Dawn duties: roll today's event once, schedule its fire hour.
    if !clock.dawn_done && clock.hour >= DAWN {
        clock.dawn_done = true;
        clock.skipping = false; // skip-to-dawn arrived
        let day = game.0.fortress.day;
        // Under auto-mode the hold builds itself, raising the next thing it needs.
        if auto.0 {
            if let Some(upgrade) = game.0.auto_build_pick() {
                if let Ok(line) = game.0.construct(upgrade) {
                    log.push(format!("Day {day}: {line} (auto)"));
                }
            }
        }
        let rolled = roll(&deck.0, day, &mut game.0, ctl.last_event_name.as_deref()).cloned();
        match rolled {
            Some(event) => {
                ctl.last_event_name = Some(event.name.clone());
                ctl.pending_event = Some(event);
                // presentation-only randomness: UI rng, never gs.rng
                clock.event_hour = Some(rand::rng().random_range(9.0..18.0));
            }
            None => {
                clock.event_hour = None;
                log.push(format!("Day {day}: a quiet day passes."));
            }
        }
    }

    // Fire the pending event at its hour.
    if let Some(fire_at) = clock.event_hour {
        if clock.hour >= fire_at {
            clock.event_hour = None;
            if let Some(event) = ctl.pending_event.take() {
                // Three ways an event can settle without a modal: it's an auto
                // event (one foregone choice), or auto-mode is on (the engine
                // picks for us), or no choice is available. Otherwise, pause for
                // the player's decision.
                let auto_choice = if event.auto {
                    Some(0)
                } else if auto.0 {
                    auto_pick(&event, &game.0).or(Some(0))
                } else {
                    None
                };
                if let Some(idx) = auto_choice {
                    let result = resolve(&event, idx, &mut game.0);
                    if !event.auto {
                        log.push(format!(
                            "Day {}: {} — {} (auto)",
                            game.0.fortress.day, result.event_name, result.choice_label
                        ));
                    }
                    for line in result.lines {
                        log.push(format!("Day {}: {}", game.0.fortress.day, line));
                    }
                } else {
                    let availability = event
                        .choices
                        .iter()
                        .map(|c| fortress_core::choice_availability(c, &event, &game.0))
                        .collect();
                    commands.insert_resource(ActiveEvent { event, availability });
                    next_state.set(AppState::EventModal);
                    return;
                }
            }
        }
    }

    // Midnight: run the day-end pipeline and wrap.
    if clock.hour >= 24.0 {
        clock.hour -= 24.0;
        clock.dawn_done = false;
        let next = finish_day(&mut game.0, &mut log);
        if next != AppState::FortressView {
            next_state.set(next);
        }
    }
}

fn speed_hotkeys(keys: Res<ButtonInput<KeyCode>>, mut clock: ResMut<GameClock>) {
    if keys.just_pressed(KeyCode::Space) {
        clock.speed = if clock.speed == ClockSpeed::Paused {
            ClockSpeed::Normal
        } else {
            ClockSpeed::Paused
        };
    }
    if keys.just_pressed(KeyCode::Digit1) {
        clock.speed = ClockSpeed::Normal;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        clock.speed = ClockSpeed::Fast;
    }
}

/// Press A to hand the reins to the engine (Progress-Quest auto-play), or take
/// them back. Auto-resolved events skip the modal; the clock keeps running.
fn auto_toggle(keys: Res<ButtonInput<KeyCode>>, mut auto: ResMut<AutoMode>, mut log: ResMut<GameLog>) {
    if keys.just_pressed(KeyCode::KeyA) {
        auto.0 = !auto.0;
        log.push(if auto.0 {
            "Auto-mode ON — the fortress runs itself.".to_string()
        } else {
            "Auto-mode OFF — you have the reins again.".to_string()
        });
    }
}
