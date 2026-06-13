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
const HERO_COLOR: Color = Color::srgb(0.7, 0.4, 0.9);

#[derive(Component)]
struct RosterRoot;

/// What clicking a row selects (heroes and the header select nothing).
#[derive(Clone, PartialEq, Eq)]
enum RowTarget {
    None,
    Commander,
    Inhabitant(String),
}

#[derive(Component, Clone)]
struct RosterRow {
    target: RowTarget,
}

/// Snapshot of what the panel currently shows; rows rebuild only when it changes.
#[derive(Resource, Default, PartialEq, Eq)]
struct RosterCache(RosterSnap);

#[derive(Default, PartialEq, Eq, Clone)]
struct RosterSnap {
    summary: String,
    rows: Vec<RosterEntry>,
}

#[derive(PartialEq, Eq, Clone)]
enum RowKind {
    Commander,
    Hero,
    Inhabitant(fortress_core::Role),
}

#[derive(PartialEq, Eq, Clone)]
struct RosterEntry {
    kind: RowKind,
    glyph: char,
    name: String,
    detail: String,
}

impl RosterEntry {
    fn color(&self) -> Color {
        match &self.kind {
            RowKind::Commander => crate::ui::ACCENT,
            RowKind::Hero => HERO_COLOR,
            RowKind::Inhabitant(role) => role_glyph(*role).1,
        }
    }

    fn target(&self) -> RowTarget {
        match &self.kind {
            RowKind::Commander => RowTarget::Commander,
            RowKind::Hero => RowTarget::None,
            RowKind::Inhabitant(_) => RowTarget::Inhabitant(self.name.clone()),
        }
    }
}

fn snapshot(game: &Game) -> RosterSnap {
    let gs = &game.0;
    let mut rows = Vec::new();

    // commander pinned at the top
    if let Some(p) = gs.player.as_ref().filter(|p| p.is_alive()) {
        rows.push(RosterEntry {
            kind: RowKind::Commander,
            glyph: '@',
            name: format!("{} the {}", p.name, p.class.name()),
            detail: format!("commander · hp {} · mo {}", p.health, p.morale),
        });
    }

    // resident heroes next
    for a in &gs.adventurers {
        rows.push(RosterEntry {
            kind: RowKind::Hero,
            glyph: '&',
            name: format!("{} the {}", a.name, a.class.name()),
            detail: format!("{} {}", a.perk_tier().name(), a.class.home_skill().practitioner()),
        });
    }

    // then the inhabitants, with traits inline
    let alive = gs.inhabitants.get_alive();
    let mut role_counts = [0usize; 4];
    for i in &alive {
        role_counts[i.role as usize] += 1;
        let (tier, skill) = i.skills.signature();
        let traits = if i.traits.is_empty() {
            String::new()
        } else {
            format!(" · {}", i.traits.iter().map(|t| t.name()).collect::<Vec<_>>().join(", "))
        };
        rows.push(RosterEntry {
            kind: RowKind::Inhabitant(i.role),
            glyph: role_glyph(i.role).0,
            name: i.name.clone(),
            detail: format!(
                "{} {} · hp {} · mo {}{}",
                tier.name(),
                skill.practitioner(),
                i.health,
                i.morale,
                traits
            ),
        });
    }

    // header: who lives in the fortress, at a glance
    let commander = usize::from(gs.player.as_ref().is_some_and(|p| p.is_alive()));
    let souls = alive.len() + commander;
    let mut parts = Vec::new();
    for role in fortress_core::Role::ALL {
        let n = role_counts[role as usize];
        if n > 0 {
            parts.push(format!("{n} {}s", role.name().to_lowercase()));
        }
    }
    let mut summary = format!("{souls} souls — {}", parts.join(" · "));
    if !gs.adventurers.is_empty() {
        summary.push_str(&format!(" | {} heroes", gs.adventurers.len()));
    }

    RosterSnap { summary, rows }
}

fn spawn_panel(mut commands: Commands, mut cache: ResMut<RosterCache>) {
    cache.0 = RosterSnap::default(); // force refresh_rows to rebuild on entry
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
        // header: a one-line census of who lives here
        panel.spawn((RosterRow { target: RowTarget::None }, text("WHO LIVES HERE", 13.0, TEXT_DIM)));
        panel.spawn((RosterRow { target: RowTarget::None }, text(&*current.summary, 11.0, TEXT_DIM)));

        for entry in &current.rows {
            let color = entry.color();
            panel
                .spawn((
                    RosterRow { target: entry.target() },
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
                    row.spawn(text(entry.glyph.to_string(), 15.0, color));
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        ..Default::default()
                    })
                    .with_children(|col| {
                        col.spawn(text(&*entry.name, 14.0, Color::WHITE));
                        col.spawn(text(&*entry.detail, 11.0, TEXT_DIM));
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
        if *interaction != Interaction::Pressed {
            continue;
        }
        match &row.target {
            RowTarget::Commander => selected.0 = Some(Selection::Commander),
            RowTarget::Inhabitant(name) => {
                selected.0 = Some(Selection::Inhabitant(name.clone()))
            }
            RowTarget::None => {}
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
        let is_selected = match (&selected.0, &row.target) {
            (Some(Selection::Commander), RowTarget::Commander) => true,
            (Some(Selection::Inhabitant(n)), RowTarget::Inhabitant(m)) => n == m,
            _ => false,
        };
        *bg = if is_selected {
            ROW_BG_SELECTED.into()
        } else if *interaction == Interaction::Hovered {
            ROW_BG_HOVER.into()
        } else {
            ROW_BG.into()
        };
    }
}
