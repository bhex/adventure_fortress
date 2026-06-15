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
            .init_resource::<RosterControls>()
            .add_systems(OnEnter(AppState::FortressView), spawn_panel)
            .add_systems(
                Update,
                (cycle_controls, assign_role_keys, refresh_rows, row_click, highlight_selected_row)
                    .chain()
                    .run_if(in_state(AppState::FortressView)),
            );
    }
}

/// How the inhabitant rows are ordered and which role is shown.
#[derive(Resource, Default, Clone, PartialEq, Eq)]
struct RosterControls {
    sort: SortMode,
    filter: Option<fortress_core::Role>,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum SortMode {
    #[default]
    Default,
    Health,
    Skill,
}

impl SortMode {
    fn label(&self) -> &'static str {
        match self {
            SortMode::Default => "roster",
            SortMode::Health => "health",
            SortMode::Skill => "skill",
        }
    }
    fn next(self) -> SortMode {
        match self {
            SortMode::Default => SortMode::Health,
            SortMode::Health => SortMode::Skill,
            SortMode::Skill => SortMode::Default,
        }
    }
}

fn next_filter(f: Option<fortress_core::Role>) -> Option<fortress_core::Role> {
    use fortress_core::Role;
    match f {
        None => Some(Role::Guard),
        Some(Role::Guard) => Some(Role::Farmer),
        Some(Role::Farmer) => Some(Role::Blacksmith),
        Some(Role::Blacksmith) => Some(Role::Healer),
        Some(Role::Healer) => Some(Role::Miner),
        Some(Role::Miner) => Some(Role::Peasant),
        Some(Role::Peasant) => Some(Role::Scholar),
        Some(Role::Scholar) => Some(Role::Herbalist),
        Some(Role::Herbalist) => None,
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
    Hero(String),
    Inhabitant(String),
}

#[derive(Component, Clone)]
struct RosterRow {
    target: RowTarget,
}

#[derive(Component)]
struct CycleSortButton;

#[derive(Component)]
struct CycleFilterButton;

/// Snapshot of what the panel currently shows; rows rebuild only when it changes.
#[derive(Resource, Default, PartialEq, Eq)]
struct RosterCache(RosterSnap);

#[derive(Default, PartialEq, Eq, Clone)]
struct RosterSnap {
    summary: String,
    controls_label: String,
    rows: Vec<RosterEntry>,
    footer: String,
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
            RowKind::Hero => RowTarget::Hero(self.name.clone()),
            RowKind::Inhabitant(_) => RowTarget::Inhabitant(self.name.clone()),
        }
    }
}

fn snapshot(game: &Game, controls: &RosterControls) -> RosterSnap {
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

    // resident heroes next — clickable, with class + perk in the detail line
    for a in &gs.adventurers {
        rows.push(RosterEntry {
            kind: RowKind::Hero,
            glyph: '&',
            name: a.name.clone(),
            detail: format!(
                "{} · {} {} · {}",
                a.class.name(),
                a.perk_tier().name(),
                a.class.home_skill().practitioner(),
                a.class.perk_name()
            ),
        });
    }

    // then the inhabitants — counted for the header, then filtered & sorted
    let alive = gs.inhabitants.get_alive();
    let mut role_counts = [0usize; fortress_core::Role::ALL.len()];
    for i in &alive {
        role_counts[i.role as usize] += 1;
    }
    let mut shown: Vec<&&fortress_core::Inhabitant> =
        alive.iter().filter(|i| controls.filter.is_none_or(|r| i.role == r)).collect();
    match controls.sort {
        SortMode::Default => {}
        SortMode::Health => shown.sort_by_key(|i| i.health),
        SortMode::Skill => shown.sort_by_key(|i| std::cmp::Reverse(i.skills.signature().0.index())),
    }
    for i in &shown {
        let (tier, skill) = i.skills.signature();
        let visible: Vec<&str> =
            i.traits.iter().filter(|t| !t.is_hidden()).map(|t| t.name()).collect();
        let traits =
            if visible.is_empty() { String::new() } else { format!(" · {}", visible.join(", ")) };
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

    // controls + totals footer
    let filter_label = match controls.filter {
        None => "all".to_string(),
        Some(r) => format!("{}s", r.name().to_lowercase()),
    };
    let controls_label = format!("sort: {} · show: {}", controls.sort.label(), filter_label);
    let wounded = alive.iter().filter(|i| i.health < 100).count();
    let avg_morale = if alive.is_empty() {
        0
    } else {
        alive.iter().map(|i| i.morale).sum::<i32>() / alive.len() as i32
    };
    let footer = format!("avg morale {avg_morale} · {wounded} wounded");

    RosterSnap { summary, controls_label, rows, footer }
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
    controls: Res<RosterControls>,
    mut cache: ResMut<RosterCache>,
    roots: Query<Entity, With<RosterRoot>>,
    rows: Query<Entity, With<RosterRow>>,
) {
    let current = snapshot(&game, &controls);
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

        // sort / filter controls (click to cycle)
        let ctl_node = || Node {
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            margin: UiRect::all(Val::Px(1.0)),
            ..Default::default()
        };
        let filter_word = match controls.filter {
            None => "all".to_string(),
            Some(r) => format!("{}s", r.name().to_lowercase()),
        };
        panel
            .spawn((
                RosterRow { target: RowTarget::None }, // tag the bar so it's despawned on rebuild
                Node { column_gap: Val::Px(4.0), margin: UiRect::vertical(Val::Px(2.0)), ..Default::default() },
            ))
            .with_children(|bar| {
                bar.spawn((CycleSortButton, Button, ctl_node(), BackgroundColor(ROW_BG)))
                    .with_children(|b| {
                        b.spawn(text(format!("⇅ sort: {}", controls.sort.label()), 11.0, crate::ui::ACCENT));
                    });
                bar.spawn((CycleFilterButton, Button, ctl_node(), BackgroundColor(ROW_BG)))
                    .with_children(|b| {
                        b.spawn(text(format!("show: {filter_word}"), 11.0, crate::ui::ACCENT));
                    });
            });

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

        // fortress-wide totals footer
        panel.spawn((
            RosterRow { target: RowTarget::None },
            text(format!("— {} —", current.footer), 11.0, TEXT_DIM),
        ));
    });
}

/// Click the sort/filter chips to cycle them; the panel rebuilds to match.
fn cycle_controls(
    mut controls: ResMut<RosterControls>,
    sort_btn: Query<&Interaction, (Changed<Interaction>, With<CycleSortButton>)>,
    filter_btn: Query<&Interaction, (Changed<Interaction>, With<CycleFilterButton>)>,
) {
    if sort_btn.iter().any(|i| *i == Interaction::Pressed) {
        controls.sort = controls.sort.next();
    }
    if filter_btn.iter().any(|i| *i == Interaction::Pressed) {
        controls.filter = next_filter(controls.filter);
    }
}

/// With an inhabitant selected, digit keys reassign their role — the manual
/// half of "assign + drift". (Heroes/commander are unaffected.)
fn assign_role_keys(
    keys: Res<ButtonInput<KeyCode>>,
    selected: Res<Selected>,
    mut game: ResMut<Game>,
) {
    let Some(Selection::Inhabitant(name)) = &selected.0 else { return };
    let role = if keys.just_pressed(KeyCode::Digit1) {
        fortress_core::Role::Guard
    } else if keys.just_pressed(KeyCode::Digit2) {
        fortress_core::Role::Farmer
    } else if keys.just_pressed(KeyCode::Digit3) {
        fortress_core::Role::Blacksmith
    } else if keys.just_pressed(KeyCode::Digit4) {
        fortress_core::Role::Healer
    } else if keys.just_pressed(KeyCode::Digit5) {
        fortress_core::Role::Miner
    } else if keys.just_pressed(KeyCode::Digit6) {
        fortress_core::Role::Peasant
    } else {
        return;
    };
    if let Some(i) = game.0.inhabitants.inhabitants.iter_mut().find(|i| &i.name == name) {
        i.role = role;
    }
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
            RowTarget::Hero(name) => selected.0 = Some(Selection::Hero(name.clone())),
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
            (Some(Selection::Hero(n)), RowTarget::Hero(m)) => n == m,
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
