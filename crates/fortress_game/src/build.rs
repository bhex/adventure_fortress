//! Build menu modal: raise buildings for materials, gated by specialists.

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
                (build_click, close_menu, tint_buttons).run_if(in_state(AppState::BuildMenu)),
            )
            .add_systems(Update, open_key.run_if(in_state(AppState::FortressView)));
    }
}

#[derive(Component, Clone, Copy)]
struct BuildButton(Upgrade);

#[derive(Component)]
struct CloseButton;

#[derive(Component)]
struct MenuRoot;

fn open_key(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::KeyB) {
        next_state.set(AppState::BuildMenu);
    }
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
                    Node {
                        width: Val::Px(560.0),
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

                    for upgrade in Upgrade::ALL {
                        let availability = gs.build_availability(upgrade);
                        let enabled = availability == BuildAvailability::Ok;
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
                                format!("  — {} (costs {})", verb, upgrade.build_cost(next).describe_cost())
                            }
                            BuildAvailability::MaxLevel => "  — at its height".to_string(),
                            BuildAvailability::MissingRole(role) => {
                                format!("  [needs a {}]", role.name())
                            }
                            BuildAvailability::CantAfford => {
                                format!("  — {} (can't afford: {})", verb, upgrade.build_cost(next).describe_cost())
                            }
                        };

                        let mut button = panel.spawn((
                            BuildButton(upgrade),
                            Button,
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::FlexStart,
                                padding: UiRect::all(Val::Px(8.0)),
                                margin: UiRect::vertical(Val::Px(1.0)),
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ));
                        if !enabled {
                            button.insert(Disabled);
                        }
                        let label_color = if enabled { Color::WHITE } else { TEXT_DIM };

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
        if let Ok(line) = game.0.construct(button.0) {
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
