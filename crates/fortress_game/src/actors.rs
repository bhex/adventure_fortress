//! Inhabitants as wandering glyph entities, synced with the core game state.
//! By day they work their preferred spots; at dusk they head to beds
//! (Barracks for guards, the Keep for the rest, stables for the overflow).

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use rand::Rng;

use fortress_core::{Role, Upgrade};

use crate::bridge::Game;
use crate::clock::{DayPhase, GameClock};
use crate::map::{role_glyph, AnchorKind, MapLayout};
use crate::AppState;

pub struct ActorsPlugin;

impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (sync_actors, wander).run_if(in_state(AppState::FortressView)),
        );
    }
}

/// Map actors are the inhabitants and the commander; heroes live in the
/// roster, not on the grid.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActorKind {
    Inhabitant(Role),
    Commander,
}

#[derive(Component)]
pub struct Actor {
    pub name: String,
    pub kind: ActorKind,
}

#[derive(Component)]
pub struct GridPos(pub IVec2);

#[derive(Component)]
pub struct Glyph {
    pub ch: char,
    pub color: Color,
}

#[derive(Component, Default)]
pub struct Wander {
    pub target: Option<IVec2>,
    pub idle_ticks: u8,
    pub timer: Timer,
    /// True only once an actor has actually reached its bed — drives the
    /// sleeping glyph, so folk still walking at night aren't shown as asleep.
    pub asleep: bool,
}

/// Spawn entities for new inhabitants, despawn entities for the dead/departed.
fn sync_actors(
    mut commands: Commands,
    game: Res<Game>,
    layout: Res<MapLayout>,
    actors: Query<(Entity, &Actor)>,
) {
    let mut wanted: Vec<(String, ActorKind)> = game
        .0
        .inhabitants
        .get_alive()
        .iter()
        .map(|i| (i.name.clone(), ActorKind::Inhabitant(i.role)))
        .collect();
    // The commander walks their own keep — an '@' among the people.
    if let Some(p) = &game.0.player {
        if p.is_alive() {
            wanted.push((p.name.clone(), ActorKind::Commander));
        }
    }

    for (entity, actor) in actors.iter() {
        if !wanted.iter().any(|(n, _)| n == &actor.name) {
            commands.entity(entity).despawn();
        }
    }

    let mut rng = rand::rng();
    for (name, kind) in wanted {
        if actors.iter().any(|(_, a)| a.name == name) {
            continue;
        }
        let spawn = layout
            .anchors
            .get(&AnchorKind::Gate)
            .and_then(|v| v.first().copied())
            .unwrap_or(IVec2::new(20, 5));
        let (ch, color) = match kind {
            ActorKind::Inhabitant(role) => role_glyph(role),
            ActorKind::Commander => ('@', crate::ui::ACCENT),
        };
        commands.spawn((
            Actor { name, kind },
            GridPos(spawn),
            Glyph { ch, color },
            Wander {
                target: None,
                idle_ticks: 0,
                timer: Timer::from_seconds(rng.random_range(0.18..0.32), TimerMode::Repeating),
                asleep: false,
            },
        ));
    }
}

/// Where the overflow sleeps rough: a stables row by the west wall.
const STABLES_ROW: [IVec2; 6] = [
    IVec2::new(4, 2),
    IVec2::new(5, 2),
    IVec2::new(6, 2),
    IVec2::new(7, 2),
    IVec2::new(8, 2),
    IVec2::new(9, 2),
];

/// Deterministic bed assignment, mirroring core's sleeping_capacity:
/// guards take Barracks bunks if built, everyone else the Keep's 6 beds
/// (front row), and whoever is left beds down in the stables.
fn assign_beds(actors: &[(String, ActorKind)], layout: &MapLayout) -> HashMap<String, IVec2> {
    let has_barracks = layout.built.iter().any(|b| b.kind == Upgrade::Barracks);
    let mut barracks: Vec<IVec2> = if has_barracks {
        (31..=35).map(|x| IVec2::new(x, 16)).collect()
    } else {
        Vec::new()
    };
    let mut keep_beds: Vec<IVec2> = (17..=22).map(|x| IVec2::new(x, 17)).collect();
    // beds in front of each built housing plot, then rough stables overflow
    let mut housing_beds = layout.housing_beds.clone();
    let mut stables = STABLES_ROW.to_vec();

    let mut sorted: Vec<&(String, ActorKind)> = actors.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut beds = HashMap::new();
    // the commander always takes the first Keep bed — it is their keep
    for (name, _) in sorted.iter().filter(|(_, k)| *k == ActorKind::Commander) {
        if let Some(spot) = keep_beds.pop() {
            beds.insert(name.clone(), spot);
        }
    }
    // guards claim barracks bunks next
    for (name, _) in
        sorted.iter().filter(|(_, k)| matches!(k, ActorKind::Inhabitant(Role::Guard)))
    {
        if let Some(spot) = barracks.pop() {
            beds.insert(name.clone(), spot);
        }
    }
    for (name, _) in sorted {
        if beds.contains_key(name) {
            continue;
        }
        let spot = keep_beds
            .pop()
            .or_else(|| housing_beds.pop())
            .or_else(|| stables.pop())
            .unwrap_or(IVec2::new(20, 5));
        beds.insert(name.clone(), spot);
    }
    beds
}

fn preferred_anchors(kind: ActorKind) -> &'static [AnchorKind] {
    match kind {
        ActorKind::Commander => {
            &[AnchorKind::Courtyard, AnchorKind::Walls, AnchorKind::Keep]
        }
        ActorKind::Inhabitant(Role::Guard) => {
            &[AnchorKind::Gate, AnchorKind::Walls, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Farmer) => {
            &[AnchorKind::Farm, AnchorKind::Gate, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Blacksmith) => &[
            AnchorKind::Building(fortress_core::Upgrade::Blacksmith),
            AnchorKind::Keep,
            AnchorKind::Courtyard,
        ],
        ActorKind::Inhabitant(Role::Healer) => &[
            AnchorKind::Building(fortress_core::Upgrade::Infirmary),
            AnchorKind::Keep,
            AnchorKind::Courtyard,
        ],
        ActorKind::Inhabitant(Role::Miner) => &[
            AnchorKind::Building(fortress_core::Upgrade::Mine),
            AnchorKind::Courtyard,
            AnchorKind::Keep,
        ],
        ActorKind::Inhabitant(Role::Peasant) => {
            &[AnchorKind::Courtyard, AnchorKind::Gate, AnchorKind::Walls]
        }
    }
}

fn wander(
    time: Res<Time>,
    clock: Res<GameClock>,
    layout: Res<MapLayout>,
    mut actors: Query<(&Actor, &mut GridPos, &mut Wander)>,
) {
    let mut rng = rand::rng();
    let occupied: Vec<IVec2> = actors.iter().map(|(_, p, _)| p.0).collect();

    // Actors move in step with the clock: faster speeds make the fortress
    // visibly busier; a pause freezes everyone mid-stride.
    let speed_mult = clock.actor_speed_mult();
    let scaled_delta = time.delta().mul_f32(speed_mult);

    let phase = clock.phase();
    let bedtime = matches!(phase, DayPhase::Dusk | DayPhase::Night);
    let beds = if bedtime {
        let roster: Vec<(String, ActorKind)> =
            actors.iter().map(|(a, _, _)| (a.name.clone(), a.kind)).collect();
        assign_beds(&roster, &layout)
    } else {
        HashMap::new()
    };

    for (actor, mut pos, mut wander) in actors.iter_mut() {
        wander.timer.tick(scaled_delta);
        if !wander.timer.just_finished() {
            continue;
        }

        // Dusk/Night: head to bed; asleep only once actually there.
        if bedtime {
            let Some(&bed) = beds.get(&actor.name) else { continue };
            if pos.0 == bed {
                wander.asleep = true;
                continue; // zzz
            }
            wander.asleep = false;
            wander.target = Some(bed);
            wander.idle_ticks = 0;
        } else if wander.asleep {
            wander.asleep = false;
        } else if wander.idle_ticks > 0 {
            wander.idle_ticks -= 1;
            continue;
        }

        let target = match wander.target {
            Some(t) if t != pos.0 => t,
            _ => {
                if bedtime {
                    continue;
                }
                if wander.target.is_some() {
                    // arrived
                    wander.target = None;
                    wander.idle_ticks = rng.random_range(2..=5);
                    continue;
                }
                let mut picked = None;
                for kind in preferred_anchors(actor.kind) {
                    if let Some(spots) = layout.anchors.get(kind) {
                        if !spots.is_empty() {
                            picked = Some(spots[rng.random_range(0..spots.len())]);
                            break;
                        }
                    }
                }
                match picked {
                    Some(t) => {
                        wander.target = Some(t);
                        t
                    }
                    None => continue,
                }
            }
        };

        // greedy 4-dir step: larger-delta axis first, fallback other axis, fallback random neighbor
        let delta = target - pos.0;
        let primary = if delta.x.abs() >= delta.y.abs() {
            IVec2::new(delta.x.signum(), 0)
        } else {
            IVec2::new(0, delta.y.signum())
        };
        let secondary = if primary.x != 0 {
            IVec2::new(0, delta.y.signum())
        } else {
            IVec2::new(delta.x.signum(), 0)
        };

        let mut stepped = false;
        for dir in [primary, secondary] {
            if dir == IVec2::ZERO {
                continue;
            }
            let next = pos.0 + dir;
            if layout.walkable.contains(&next) && !occupied.contains(&next) {
                pos.0 = next;
                stepped = true;
                break;
            }
        }
        if !stepped {
            let neighbors = [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y];
            let dir = neighbors[rng.random_range(0..4)];
            let next = pos.0 + dir;
            if layout.walkable.contains(&next) && !occupied.contains(&next) {
                pos.0 = next;
            }
        }
    }
}
