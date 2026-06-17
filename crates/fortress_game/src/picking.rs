//! Mouse hover and click resolution on the glyph grid.

use bevy::prelude::*;
use bevy_ascii_terminal::*;

use crate::actors::{Actor, ActorKind, GridPos};
use crate::bridge::{Selected, Selection};
use crate::map::{MapLayout, TileKind};
use crate::AppState;

pub struct PickingPlugin;

impl Plugin for PickingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Hovered>().add_systems(
            Update,
            (update_hover, click_select).run_if(in_state(AppState::FortressView)),
        );
    }
}

#[derive(Resource, Default, PartialEq)]
pub struct Hovered(pub Option<IVec2>);

pub fn cursor_tile(
    windows: &Query<&Window>,
    camera: &Query<(&Camera, &GlobalTransform), With<TerminalCamera>>,
    terminal: &Query<&TerminalTransform>,
) -> Option<IVec2> {
    let window = windows.single().ok()?;
    let cursor = window.cursor_position()?;
    let (cam, cam_transform) = camera.single().ok()?;
    let world = cam.viewport_to_world_2d(cam_transform, cursor).ok()?;
    let term_transform = terminal.single().ok()?;
    term_transform.world_to_tile(world)
}

fn update_hover(
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<TerminalCamera>>,
    terminal: Query<&TerminalTransform>,
    mut hovered: ResMut<Hovered>,
) {
    // Only write when the tile under the cursor actually changes, so the
    // resource's change-detection flag stays quiet on idle frames (the map
    // redraw watches it).
    hovered.set_if_neq(Hovered(cursor_tile(&windows, &camera, &terminal)));
}

fn click_select(
    buttons: Res<ButtonInput<MouseButton>>,
    hovered: Res<Hovered>,
    layout: Res<MapLayout>,
    actors: Query<(&Actor, &GridPos)>,
    mut selected: ResMut<Selected>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(tile) = hovered.0 else {
        return;
    };

    // priority: actor > building > keep/gate > ground (clears)
    if let Some((actor, _)) = actors.iter().find(|(_, p)| p.0 == tile) {
        selected.0 = Some(match actor.kind {
            ActorKind::Commander => Selection::Commander,
            ActorKind::Inhabitant(_) => Selection::Inhabitant(actor.name.clone()),
        });
        return;
    }
    selected.0 = match layout.tiles.get(&tile) {
        Some(TileKind::Building(u)) => Some(Selection::Building(*u)),
        Some(TileKind::Keep) => Some(Selection::Keep),
        Some(TileKind::Gate) => Some(Selection::Gate),
        _ => None,
    };
}
