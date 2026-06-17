//! Inhabitants as glyphs standing at their stations, synced with the core game
//! state. People no longer wander in real time — each soul is placed on a free
//! tile near the building they work (guards by the gate/walls, smiths at the
//! forge, …), so the fortress reads as populated without per-frame motion.
//! Positions are recomputed only when the roster or the layout changes.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use fortress_core::Role;

use crate::bridge::Game;
use crate::map::{role_glyph, AnchorKind, MapLayout};
use crate::AppState;

pub struct ActorsPlugin;

impl Plugin for ActorsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, sync_actors.run_if(in_state(AppState::FortressView)));
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

/// Which stations a role gravitates to, in priority order. The first anchor
/// that exists in the layout becomes the soul's standing post.
fn preferred_anchors(kind: ActorKind) -> &'static [AnchorKind] {
    use fortress_core::Upgrade;
    match kind {
        ActorKind::Commander => &[AnchorKind::Keep, AnchorKind::Courtyard, AnchorKind::Walls],
        ActorKind::Inhabitant(Role::Guard) => {
            &[AnchorKind::Gate, AnchorKind::Walls, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Farmer) => {
            &[AnchorKind::Farm, AnchorKind::Courtyard, AnchorKind::Gate]
        }
        ActorKind::Inhabitant(Role::Blacksmith) => {
            &[AnchorKind::Building(Upgrade::Blacksmith), AnchorKind::Keep, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Healer) => {
            &[AnchorKind::Building(Upgrade::Infirmary), AnchorKind::Keep, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Miner) => {
            &[AnchorKind::Building(Upgrade::Mine), AnchorKind::Courtyard, AnchorKind::Keep]
        }
        ActorKind::Inhabitant(Role::Peasant) => {
            &[AnchorKind::Courtyard, AnchorKind::Gate, AnchorKind::Walls]
        }
        ActorKind::Inhabitant(Role::Scholar) => {
            &[AnchorKind::Building(Upgrade::Library), AnchorKind::Keep, AnchorKind::Courtyard]
        }
        ActorKind::Inhabitant(Role::Herbalist) => {
            &[AnchorKind::Building(Upgrade::Alchemist), AnchorKind::Courtyard, AnchorKind::Keep]
        }
    }
}

/// The tile a role anchors to, falling back toward the courtyard centre.
fn station(kind: ActorKind, layout: &MapLayout) -> IVec2 {
    for anchor in preferred_anchors(kind) {
        if let Some(spot) = layout.anchors.get(anchor).and_then(|v| v.first()) {
            return *spot;
        }
    }
    IVec2::new(20, 12)
}

/// Deterministic placement: each soul (sorted by name) takes the nearest free
/// walkable tile to its station, so the same roster always lays out the same.
fn place_all(wanted: &[(String, ActorKind)], layout: &MapLayout) -> HashMap<String, IVec2> {
    let walkable: Vec<IVec2> = layout.walkable.iter().copied().collect();
    let mut used: bevy::platform::collections::HashSet<IVec2> = Default::default();
    let mut out = HashMap::new();

    let mut sorted: Vec<&(String, ActorKind)> = wanted.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, kind) in sorted {
        let post = station(*kind, layout);
        let spot = walkable
            .iter()
            .filter(|t| !used.contains(*t))
            .min_by_key(|t| (**t - post).length_squared())
            .copied()
            .unwrap_or(post);
        used.insert(spot);
        out.insert(name.clone(), spot);
    }
    out
}

/// Spawn entities for new souls, despawn the departed, and (re)place everyone at
/// their stations. Runs only when the roster or the layout actually changes.
fn sync_actors(
    mut commands: Commands,
    game: Res<Game>,
    layout: Res<MapLayout>,
    mut actors: Query<(Entity, &Actor, &mut GridPos)>,
) {
    // The roster changes only with births/deaths/the commander falling; the
    // layout changes as buildings go up (shifting stations). `actors.is_empty`
    // bootstraps the first frame on entry.
    if !game.is_changed() && !layout.is_changed() && !actors.is_empty() {
        return;
    }

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

    let placement = place_all(&wanted, &layout);

    // despawn the departed, reposition the living
    for (entity, actor, mut pos) in actors.iter_mut() {
        match placement.get(&actor.name) {
            Some(spot) => {
                if pos.0 != *spot {
                    pos.0 = *spot;
                }
            }
            None => commands.entity(entity).despawn(),
        }
    }

    // spawn any newcomers
    let existing: bevy::platform::collections::HashSet<&str> =
        actors.iter().map(|(_, a, _)| a.name.as_str()).collect();
    for (name, kind) in &wanted {
        if existing.contains(name.as_str()) {
            continue;
        }
        let spot = placement.get(name).copied().unwrap_or(IVec2::new(20, 12));
        let (ch, color) = match kind {
            ActorKind::Inhabitant(role) => role_glyph(*role),
            ActorKind::Commander => ('@', crate::ui::ACCENT),
        };
        commands.spawn((Actor { name: name.clone(), kind: *kind }, GridPos(spot), Glyph { ch, color }));
    }
}
