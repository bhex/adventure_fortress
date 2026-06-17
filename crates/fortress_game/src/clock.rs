//! Day director. The core is turn-based: one event per day. Instead of a
//! real-time clock, each day *arrives* with a short dawn gradient sweep (the
//! map lightens from night to full day), then the day's event fires — as a
//! modal for a decision, or auto-resolved. Once resolved the day is settled and
//! waits for the player to advance (Space / the HUD button), or auto-advances
//! under auto-mode. The core stays deterministic; the sweep is pure
//! presentation and never touches GameState.rng.

use bevy::prelude::*;

use fortress_core::{auto_pick, resolve, roll};

use crate::bridge::{finish_day, ActiveEvent, AutoMode, EngineCtl, EventDeck, Game, GameLog};
use crate::AppState;

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayCycle>()
            .add_systems(OnEnter(AppState::FortressView), settle_after_modal)
            .add_systems(
                Update,
                (run_day, advance_hotkey, auto_toggle).run_if(in_state(AppState::FortressView)),
            );
    }
}

/// How long the dawn gradient sweep takes, in real seconds.
pub const SWEEP_SECONDS: f32 = 2.2;
/// How long a settled day lingers before auto-mode moves on.
const AUTO_SETTLE_PAUSE: f32 = 1.2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DayState {
    /// The dawn sweep is playing; the event has been rolled but not yet fired.
    Arriving,
    /// An interactive event modal is up; the daily tick runs when it closes.
    AwaitingModal,
    /// Event resolved and the daily tick applied — awaiting the next day.
    Settled,
}

#[derive(Resource)]
pub struct DayCycle {
    /// Seconds into the dawn sweep; clamped at `SWEEP_SECONDS` once full day.
    sweep: f32,
    /// Real seconds the current settled day has lingered (for auto-advance).
    settle_timer: f32,
    /// Whether the day's event has been rolled yet (once per Arriving).
    rolled: bool,
    state: DayState,
}

impl Default for DayCycle {
    fn default() -> DayCycle {
        DayCycle { sweep: 0.0, settle_timer: 0.0, rolled: false, state: DayState::Arriving }
    }
}

impl DayCycle {
    /// Dawn-sweep progress, 0 (night) → 1 (full day). Drives the map gradient.
    pub fn sweep_progress(&self) -> f32 {
        (self.sweep / SWEEP_SECONDS).clamp(0.0, 1.0)
    }

    /// Roll over to the next day — only from a settled day. Restarts the sweep
    /// (the screen drops to night, then morning rises) and re-arms the roll.
    pub fn request_next_day(&mut self) {
        if self.state == DayState::Settled {
            self.sweep = 0.0;
            self.settle_timer = 0.0;
            self.rolled = false;
            self.state = DayState::Arriving;
        }
    }
}

/// Park a freshly resolved day, or hand control to the game-over screen.
fn settle(cycle: &mut DayCycle, next: AppState, next_state: &mut NextState<AppState>) {
    if next != AppState::FortressView {
        next_state.set(next);
    } else {
        cycle.state = DayState::Settled;
        cycle.settle_timer = 0.0;
    }
}

#[allow(clippy::too_many_arguments)]
fn run_day(
    time: Res<Time>,
    mut commands: Commands,
    mut cycle: ResMut<DayCycle>,
    deck: Res<EventDeck>,
    mut game: ResMut<Game>,
    mut ctl: ResMut<EngineCtl>,
    mut log: ResMut<GameLog>,
    auto: Res<AutoMode>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    match cycle.state {
        DayState::Arriving => {
            // Roll the day's event once, as the sweep begins.
            if !cycle.rolled {
                cycle.rolled = true;
                let day = game.0.fortress.day;
                // Under auto-mode the hold builds itself, raising what it needs.
                if auto.0 {
                    if let Some(upgrade) = game.0.auto_build_pick() {
                        if let Ok(line) = game.0.construct(upgrade) {
                            log.push(format!("Day {day}: {line} (auto)"));
                        }
                    }
                }
                let rolled = roll(&deck.0, day, &mut game.0, ctl.last_event_name.as_deref()).cloned();
                if let Some(event) = &rolled {
                    ctl.last_event_name = Some(event.name.clone());
                }
                ctl.pending_event = rolled;
            }

            cycle.sweep += time.delta_secs();
            if cycle.sweep < SWEEP_SECONDS {
                return;
            }
            cycle.sweep = SWEEP_SECONDS;

            // Dawn has fully broken — the day's trouble arrives.
            let day = game.0.fortress.day;
            match ctl.pending_event.take() {
                None => {
                    log.push(format!("Day {day}: a quiet day passes."));
                    let next = finish_day(&mut game.0, &mut log);
                    settle(&mut cycle, next, &mut next_state);
                }
                Some(event) => {
                    // Three ways an event settles without a modal: it's an auto
                    // event (one foregone choice), auto-mode is on (the engine
                    // picks), or no choice is available. Otherwise, a decision.
                    let auto_choice = if event.auto {
                        Some(0)
                    } else if auto.0 {
                        auto_pick(&event, &game.0).or(Some(0))
                    } else {
                        None
                    };
                    match auto_choice {
                        Some(idx) => {
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
                            let next = finish_day(&mut game.0, &mut log);
                            settle(&mut cycle, next, &mut next_state);
                        }
                        None => {
                            let availability = event
                                .choices
                                .iter()
                                .map(|c| fortress_core::choice_availability(c, &event, &game.0))
                                .collect();
                            commands.insert_resource(ActiveEvent { event, availability });
                            cycle.state = DayState::AwaitingModal;
                            next_state.set(AppState::EventModal);
                        }
                    }
                }
            }
        }
        DayState::AwaitingModal => {
            // The modal owns the day; the daily tick runs in settle_after_modal.
        }
        DayState::Settled => {
            if auto.0 {
                cycle.settle_timer += time.delta_secs();
                if cycle.settle_timer >= AUTO_SETTLE_PAUSE {
                    cycle.request_next_day();
                }
            }
        }
    }
}

/// When an interactive event modal closes we return to the fortress; this runs
/// the day-end pipeline that the auto paths run inline. No-ops on every other
/// return to the fortress (build menu, region view, the first day's entry).
fn settle_after_modal(
    mut cycle: ResMut<DayCycle>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if cycle.state != DayState::AwaitingModal {
        return;
    }
    let next = finish_day(&mut game.0, &mut log);
    settle(&mut cycle, next, &mut next_state);
}

fn advance_hotkey(keys: Res<ButtonInput<KeyCode>>, mut cycle: ResMut<DayCycle>) {
    if keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::KeyN) {
        cycle.request_next_day();
    }
}

/// Press A to hand the reins to the engine (Progress-Quest auto-play): events
/// auto-resolve and days advance on their own. Press again to take them back.
fn auto_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut auto: ResMut<AutoMode>,
    mut log: ResMut<GameLog>,
) {
    if keys.just_pressed(KeyCode::KeyA) {
        auto.0 = !auto.0;
        log.push(if auto.0 {
            "Auto-mode ON — the fortress runs itself.".to_string()
        } else {
            "Auto-mode OFF — you have the reins again.".to_string()
        });
    }
}
