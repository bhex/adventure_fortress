//! Build menu modal: raise buildings for materials, gated by specialists.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;

use fortress_core::{BuildAvailability, GameState, Upgrade};

use crate::bridge::{Game, GameLog};
use crate::ui::{button_node, text, tint_buttons, Disabled, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct BuildMenuPlugin;

impl Plugin for BuildMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::BuildMenu), spawn_menu)
            .add_systems(
                Update,
                (build_click, queue_click, scroll_menu, close_menu, tint_buttons)
                    .run_if(in_state(AppState::BuildMenu)),
            )
            .add_systems(
                Update,
                (open_key, forge_focus_key).run_if(in_state(AppState::FortressView)),
            );
    }
}

#[derive(Component, Clone, Copy)]
struct BuildButton(Upgrade);

/// Build-queue row controls, carrying the row's queue index.
#[derive(Component, Clone, Copy)]
struct QueueUp(usize);
#[derive(Component, Clone, Copy)]
struct QueueDown(usize);
#[derive(Component, Clone, Copy)]
struct QueueCancel(usize);

/// A small square button for the queue's ▲ ▼ ✕ controls.
fn queue_btn() -> Node {
    Node {
        width: Val::Px(28.0),
        height: Val::Px(24.0),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

#[derive(Component)]
struct CloseButton;

#[derive(Component)]
struct MenuRoot;

/// The scrollable build-menu panel (the box holding the building grid + queue).
#[derive(Component)]
struct MenuPanel;

fn open_key(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::KeyB) {
        next_state.set(AppState::BuildMenu);
    }
}

/// Press F to cycle what the forge concentrates on (weapons → armor → tools).
fn forge_focus_key(
    keys: Res<ButtonInput<KeyCode>>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
) {
    use fortress_core::ItemKind;
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let next = match game.0.fortress.craft_focus {
        ItemKind::Weapon => ItemKind::Armor,
        ItemKind::Armor => ItemKind::Tool,
        ItemKind::Tool => ItemKind::Weapon,
    };
    game.0.fortress.craft_focus = next;
    log.push(format!("The forge will now work toward {}s.", next.name()));
}

fn spawn_menu(mut commands: Commands, game: Res<Game>) {
    spawn_menu_ui(&mut commands, &game.0);
}

fn spawn_menu_ui(commands: &mut Commands, gs: &GameState) {
    commands
        .spawn((
            MenuRoot,
            DespawnOnExit(AppState::BuildMenu),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    MenuPanel,
                    ScrollPosition::default(),
                    Node {
                        width: Val::Px(900.0),
                        // Stay within the window even on a short display, and
                        // scroll the overflow (mouse wheel) rather than clipping
                        // the queue and Close button off the bottom.
                        max_height: Val::Percent(92.0),
                        overflow: Overflow::scroll_y(),
                        padding: UiRect::all(Val::Px(18.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..Default::default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|panel| {
                    panel.spawn(text("»  Raise a Building", 22.0, ACCENT));
                    panel.spawn(text(
                        "The steward tallies timber and stone. Choose what goes up next.",
                        14.0,
                        TEXT_DIM,
                    ));

                    panel.spawn(Node {
                        width: Val::Percent(100.0),
                        flex_wrap: FlexWrap::Wrap,
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::SpaceBetween,
                        ..Default::default()
                    }).with_children(|grid| {
                        for upgrade in Upgrade::ALL {
                            let availability = gs.build_availability(upgrade);
                            // CantAfford is still clickable — it queues to build later.
                            let clickable = matches!(
                                availability,
                                BuildAvailability::Ok | BuildAvailability::CantAfford
                            );
                            let next = gs.fortress.next_build_level(upgrade).unwrap_or(0);

                            // current standing: tier numeral, or plots for housing
                            let standing = if upgrade == Upgrade::Housing {
                                format!(" ({}/{} plots)", gs.fortress.housing_units(), fortress_core::HOUSING_PLOTS)
                            } else {
                                match gs.fortress.building_level(upgrade) {
                                    0 => String::new(),
                                    l => format!(" {}", fortress_core::level_numeral(l)),
                                }
                            };
                            let verb = if gs.fortress.has_upgrade(upgrade) && upgrade != Upgrade::Housing {
                                format!("upgrade to {}", fortress_core::level_numeral(next))
                            } else {
                                "build".to_string()
                            };
                            let suffix = match availability {
                                BuildAvailability::Ok => {
                                    format!("  — queue {} (costs {})", verb, upgrade.build_cost(next).describe_cost())
                                }
                                BuildAvailability::MaxLevel => "  — at its height".to_string(),
                                BuildAvailability::MissingRole(role) => {
                                    format!("  [needs a {}]", role.name())
                                }
                                BuildAvailability::CantAfford => {
                                    format!("  — queue {} (costs {} — click to queue)", verb, upgrade.build_cost(next).describe_cost())
                                }
                                BuildAvailability::InProgress => {
                                    let project = gs
                                        .fortress
                                        .projects
                                        .iter()
                                        .find(|p| p.upgrade == upgrade);
                                    match project {
                                        Some(p) if p.funded => {
                                            let wf = gs.build_workforce().max(1);
                                            let eta = (p.worker_days_remaining + wf - 1) / wf;
                                            format!("  — underway (~{eta} days left)")
                                        }
                                        _ => "  — in the build queue".to_string(),
                                    }
                                }
                            };

                            let mut button = grid.spawn((
                                BuildButton(upgrade),
                                Button,
                                Node {
                                    width: Val::Percent(49.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::FlexStart,
                                    padding: UiRect::all(Val::Px(8.0)),
                                    margin: UiRect::vertical(Val::Px(2.0)),
                                    ..Default::default()
                                },
                            BackgroundColor(BTN_BG),
                        ));
                        if !clickable {
                            button.insert(Disabled);
                        }
                        let label_color = match availability {
                            BuildAvailability::Ok => Color::WHITE,
                            // Not yet affordable, but still queueable — reads in amber.
                            BuildAvailability::CantAfford => Color::srgb(0.9, 0.75, 0.35),
                            _ => TEXT_DIM,
                        };

                        // effect line: current → next; plus what's missing if poor
                        let cur_level = gs.fortress.building_level(upgrade);
                        let effect_line = if availability == BuildAvailability::MaxLevel {
                            format!("now: {}", upgrade.effect_summary(cur_level))
                        } else if cur_level == 0 {
                            format!("gives: {}", upgrade.effect_summary(next))
                        } else {
                            format!(
                                "now: {}  →  next: {}",
                                upgrade.effect_summary(cur_level),
                                upgrade.effect_summary(next)
                            )
                        };
                        let detail = if availability == BuildAvailability::CantAfford {
                            let cost = upgrade.build_cost(next);
                            let r = &gs.resources;
                            let mut miss = Vec::new();
                            if cost.wood > r.wood {
                                miss.push(format!("{} wood", cost.wood - r.wood));
                            }
                            if cost.stone > r.stone {
                                miss.push(format!("{} stone", cost.stone - r.stone));
                            }
                            if cost.food > r.food {
                                miss.push(format!("{} food", cost.food - r.food));
                            }
                            format!("{}  ·  missing {}", effect_line, miss.join(", "))
                        } else {
                            effect_line
                        };

                            button.with_children(|b| {
                                b.spawn(text(format!("{}{}{}", upgrade.name(), standing, suffix), 16.0, label_color));
                                b.spawn(text(detail, 12.0, TEXT_DIM));
                            });
                        }
                    });

                    // ── The build queue: worked strictly front to back. ──
                    if !gs.fortress.projects.is_empty() {
                        let last = gs.fortress.projects.len() - 1;
                        let wf = gs.build_workforce().max(1);
                        panel.spawn(text("»  Build Queue", 18.0, ACCENT));
                        panel.spawn(text(
                            "Worked top to bottom; the top order is paid for and raised first.",
                            12.0,
                            TEXT_DIM,
                        ));
                        for (i, p) in gs.fortress.projects.iter().enumerate() {
                            let name = format!(
                                "{} {}",
                                p.upgrade.name(),
                                fortress_core::level_numeral(p.target_level)
                            );
                            let status = if p.funded {
                                let eta = (p.worker_days_remaining + wf - 1) / wf;
                                format!("underway — ~{eta}d left")
                            } else if i == 0 && gs.resources.can_afford(&p.materials_owed) {
                                "ready — funds next day".to_string()
                            } else {
                                format!("waiting · costs {}", p.materials_owed.describe_cost())
                            };
                            panel
                                .spawn(Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(6.0),
                                    margin: UiRect::vertical(Val::Px(2.0)),
                                    ..Default::default()
                                })
                                .with_children(|row| {
                                    row.spawn((
                                        Node { width: Val::Px(520.0), ..Default::default() },
                                        text(format!("{}.  {}  ·  {}", i + 1, name, status), 14.0, Color::WHITE),
                                    ));
                                    // ▲ move toward the front
                                    let mut up = row.spawn((QueueUp(i), Button, queue_btn(), BackgroundColor(BTN_BG)));
                                    if i == 0 {
                                        up.insert(Disabled);
                                    }
                                    up.with_children(|b| { b.spawn(text("▲", 14.0, Color::WHITE)); });
                                    // ▼ move toward the back
                                    let mut down = row.spawn((QueueDown(i), Button, queue_btn(), BackgroundColor(BTN_BG)));
                                    if i == last {
                                        down.insert(Disabled);
                                    }
                                    down.with_children(|b| { b.spawn(text("▼", 14.0, Color::WHITE)); });
                                    // ✕ cancel (funded orders refund their materials)
                                    row.spawn((QueueCancel(i), Button, queue_btn(), BackgroundColor(BTN_BG)))
                                        .with_children(|b| { b.spawn(text("✕", 14.0, Color::srgb(0.9, 0.55, 0.5))); });
                                });
                        }
                    }

                    panel
                        .spawn((CloseButton, Button, button_node(), BackgroundColor(BTN_BG)))
                        .with_children(|b| {
                            b.spawn(text("Close (Esc)", 16.0, Color::WHITE));
                        });
                });
        });
}

fn build_click(
    mut commands: Commands,
    interactions: Query<(&Interaction, &BuildButton, Option<&Disabled>), Changed<Interaction>>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    roots: Query<Entity, With<MenuRoot>>,
) {
    for (interaction, button, disabled) in interactions.iter() {
        if *interaction != Interaction::Pressed || disabled.is_some() {
            continue;
        }
        // Every build joins the queue — affordable or not. Materials are paid
        // when the order reaches the front (see GameState::fund_front_project).
        if let Ok(line) = game.0.queue_build(button.0) {
            log.push(format!("Day {}: {}", game.0.fortress.day, line));
            // respawn so costs and availability reflect the new state
            for root in roots.iter() {
                commands.entity(root).despawn();
            }
            spawn_menu_ui(&mut commands, &game.0);
        }
        return;
    }
}

/// Reorder (▲/▼) or cancel (✕) a build-queue row, then respawn the menu so the
/// queue and resources reflect the change.
fn queue_click(
    mut commands: Commands,
    up: Query<(&Interaction, &QueueUp, Option<&Disabled>), Changed<Interaction>>,
    down: Query<(&Interaction, &QueueDown, Option<&Disabled>), Changed<Interaction>>,
    cancel: Query<(&Interaction, &QueueCancel), Changed<Interaction>>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    roots: Query<Entity, With<MenuRoot>>,
) {
    let mut changed = false;
    let mut line: Option<String> = None;
    for (interaction, btn, disabled) in up.iter() {
        if *interaction == Interaction::Pressed && disabled.is_none() {
            changed |= game.0.fortress.move_project(btn.0, true);
        }
    }
    for (interaction, btn, disabled) in down.iter() {
        if *interaction == Interaction::Pressed && disabled.is_none() {
            changed |= game.0.fortress.move_project(btn.0, false);
        }
    }
    for (interaction, btn) in cancel.iter() {
        if *interaction == Interaction::Pressed {
            if let Some(l) = game.0.cancel_build(btn.0) {
                line = Some(l);
                changed = true;
            }
        }
    }
    if !changed {
        return;
    }
    if let Some(l) = line {
        log.push(format!("Day {}: {}", game.0.fortress.day, l));
    }
    for root in roots.iter() {
        commands.entity(root).despawn();
    }
    spawn_menu_ui(&mut commands, &game.0);
}

/// Scroll the build-menu panel with the mouse wheel, so a tall menu (long
/// queue) stays fully reachable on any window size.
fn scroll_menu(
    mut wheel: MessageReader<MouseWheel>,
    mut panels: Query<&mut ScrollPosition, With<MenuPanel>>,
) {
    let mut dy = 0.0;
    for ev in wheel.read() {
        dy += match ev.unit {
            MouseScrollUnit::Line => ev.y * 24.0,
            MouseScrollUnit::Pixel => ev.y,
        };
    }
    if dy == 0.0 {
        return;
    }
    for mut pos in panels.iter_mut() {
        pos.0.y = (pos.0.y - dy).max(0.0);
    }
}

fn close_menu(
    keys: Res<ButtonInput<KeyCode>>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<CloseButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let clicked = interactions.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyB) {
        next_state.set(AppState::FortressView);
    }
}
