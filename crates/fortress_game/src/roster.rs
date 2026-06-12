//! Clickable unit roster panel — the reliable way to select units
//! (map glyphs wander; this list doesn't).

use bevy::prelude::*;

use crate::bridge::{Game, Selected, Selection};
use crate::map::role_glyph;
use crate::ui::{text, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct RosterPlugin;

impl Plugin for RosterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RosterCache>()
            .add_systems(OnEnter(AppState::FortressView), spawn_panel)
            .add_systems(
                Update,
                (refresh_rows, row_click, highlight_selected_row)
                    .run_if(in_state(AppState::FortressView)),
            );
    }
}

const ROW_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);
const ROW_BG_HOVER: Color = Color::srgb(0.22, 0.25, 0.33);
const ROW_BG_SELECTED: Color = Color::srgb(0.16, 0.28, 0.2);

#[derive(Component)]
struct RosterRoot;

#[derive(Component, Clone)]
struct RosterRow {
    name: String,
}

/// Snapshot of what the panel currently shows; rows rebuild only when it changes.
#[derive(Resource, Default, PartialEq, Eq)]
struct RosterCache(Vec<RosterEntry>);

#[derive(PartialEq, Eq, Clone)]
struct RosterEntry {
    name: String,
    role: fortress_core::Role,
    health: i32,
    morale: i32,
    signature: String,
}

fn snapshot(game: &Game) -> Vec<RosterEntry> {
    game.0
        .inhabitants
        .get_alive()
        .iter()
        .map(|i| {
            let (tier, skill) = i.skills.signature();
            RosterEntry {
                name: i.name.clone(),
                role: i.role,
                health: i.health,
                morale: i.morale,
                signature: format!("{} {}", tier.name(), skill.practitioner()),
            }
        })
        .collect()
}

fn spawn_panel(mut commands: Commands, mut cache: ResMut<RosterCache>) {
    cache.0.clear(); // force refresh_rows to rebuild on entry
    commands.spawn((
        RosterRoot,
        DespawnOnExit(AppState::FortressView),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(56.0),
            left: Val::Px(8.0),
            width: Val::Px(230.0),
            max_height: Val::Percent(60.0),
            padding: UiRect::all(Val::Px(8.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            overflow: Overflow::scroll_y(),
            ..Default::default()
        },
        BackgroundColor(PANEL_BG),
    ));
}

fn refresh_rows(
    mut commands: Commands,
    game: Res<Game>,
    mut cache: ResMut<RosterCache>,
    roots: Query<Entity, With<RosterRoot>>,
    rows: Query<Entity, With<RosterRow>>,
) {
    let current = snapshot(&game);
    if cache.0 == current {
        return;
    }
    cache.0 = current.clone();

    let Ok(root) = roots.single() else { return };
    for row in rows.iter() {
        commands.entity(row).despawn();
    }

    commands.entity(root).with_children(|panel| {
        panel.spawn((RosterRow { name: String::new() }, text("THE PEOPLE", 13.0, TEXT_DIM)));
        for entry in &current {
            let (glyph, color) = role_glyph(entry.role);
            panel
                .spawn((
                    RosterRow { name: entry.name.clone() },
                    Button,
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        column_gap: Val::Px(6.0),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    BackgroundColor(ROW_BG),
                ))
                .with_children(|row| {
                    row.spawn(text(glyph.to_string(), 15.0, color));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..Default::default()
                    })
                    .with_children(|col| {
                        col.spawn(text(&*entry.name, 14.0, Color::WHITE));
                        col.spawn(text(
                            format!("{}  hp {}  mo {}", entry.signature, entry.health, entry.morale),
                            11.0,
                            TEXT_DIM,
                        ));
                    });
                });
        }
    });
}

fn row_click(
    interactions: Query<(&Interaction, &RosterRow), (Changed<Interaction>, With<Button>)>,
    mut selected: ResMut<Selected>,
) {
    for (interaction, row) in interactions.iter() {
        if *interaction == Interaction::Pressed && !row.name.is_empty() {
            selected.0 = Some(Selection::Inhabitant(row.name.clone()));
        }
    }
}

/// Row background reflects hover and current selection (selection can also
/// come from map clicks, so this re-evaluates every frame, not just on change).
fn highlight_selected_row(
    selected: Res<Selected>,
    mut rows: Query<(&Interaction, &RosterRow, &mut BackgroundColor), With<Button>>,
) {
    for (interaction, row, mut bg) in rows.iter_mut() {
        let is_selected =
            matches!(&selected.0, Some(Selection::Inhabitant(n)) if n == &row.name);
        *bg = if is_selected {
            ROW_BG_SELECTED.into()
        } else if *interaction == Interaction::Hovered {
            ROW_BG_HOVER.into()
        } else {
            ROW_BG.into()
        };
    }
}
