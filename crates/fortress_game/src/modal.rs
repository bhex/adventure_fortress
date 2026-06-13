//! Event modal: the day's event with clickable choices, then the result.

use bevy::prelude::*;

use fortress_core::{describe_effects, resolve, stat_check_odds, ChoiceAvailability};

use crate::bridge::{ActiveEvent, Game, GameLog, LevelUpOffers};
use crate::ui::{button_node, text, tint_buttons, Disabled, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::EventModal), spawn_modal)
            .add_systems(
                Update,
                (choice_click, continue_click, tint_buttons).run_if(in_state(AppState::EventModal)),
            );
    }
}

#[derive(Component, Clone, Copy)]
struct ChoiceButton(usize);

#[derive(Component)]
struct ContinueButton;

#[derive(Component)]
struct ModalRoot;

fn spawn_modal(mut commands: Commands, active: Res<ActiveEvent>, game: Res<Game>) {
    let player = game.0.player.as_ref();
    commands
        .spawn((
            ModalRoot,
            DespawnOnExit(AppState::EventModal),
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
                        width: Val::Px(620.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        ..Default::default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|panel| {
                    panel.spawn(text(format!("»  {}", active.event.name), 22.0, ACCENT));
                    panel.spawn(text(active.event.description.clone(), 16.0, Color::WHITE));
                    panel.spawn(Node {
                        height: Val::Px(8.0),
                        ..Default::default()
                    });

                    for (idx, choice) in active.event.choices.iter().enumerate() {
                        let availability = &active.availability[idx];
                        let enabled = *availability == ChoiceAvailability::Ok;

                        let suffix = match availability {
                            ChoiceAvailability::Ok => {
                                if choice.cost.is_zero() {
                                    String::new()
                                } else {
                                    format!("  (costs {})", choice.cost.describe_cost())
                                }
                            }
                            ChoiceAvailability::CantAfford => {
                                format!("  (can't afford: {})", choice.cost.describe_cost())
                            }
                            ChoiceAvailability::StatLocked(stat, min) => {
                                format!("  [{} {} required]", stat.name(), min)
                            }
                        };

                        let mut button = panel.spawn((
                            ChoiceButton(idx),
                            Button,
                            Node {
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::FlexStart,
                                padding: UiRect::all(Val::Px(10.0)),
                                margin: UiRect::vertical(Val::Px(2.0)),
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ));
                        if !enabled {
                            button.insert(Disabled);
                        }
                        let label_color = if enabled { Color::WHITE } else { TEXT_DIM };

                        // upfront effects + the gamble, if any
                        let mut preview = describe_effects(&choice.effects);
                        if let (Some(check), Some(p)) = (&choice.stat_check, player) {
                            let odds = stat_check_odds(check, p);
                            let gamble = format!("{} check — {} to succeed", check.stat.name(), odds);
                            preview = if preview.is_empty() {
                                gamble
                            } else {
                                format!("{preview}  ·  {gamble}")
                            };
                        }

                        button.with_children(|b| {
                            b.spawn(text(
                                format!("{}. {}{}", idx + 1, choice.label, suffix),
                                17.0,
                                label_color,
                            ));
                            if !choice.description.is_empty() {
                                b.spawn(text(choice.description.clone(), 13.0, TEXT_DIM));
                            }
                            if !preview.is_empty() {
                                b.spawn(text(format!("→ {preview}"), 12.0, ACCENT));
                            }
                        });
                    }
                });
        });
}

fn choice_click(
    mut commands: Commands,
    interactions: Query<(&Interaction, &ChoiceButton, Option<&Disabled>), Changed<Interaction>>,
    active: Option<Res<ActiveEvent>>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    roots: Query<Entity, With<ModalRoot>>,
) {
    let Some(active) = active else { return };
    for (interaction, button, disabled) in interactions.iter() {
        if *interaction != Interaction::Pressed || disabled.is_some() {
            continue;
        }
        let result = resolve(&active.event, button.0, &mut game.0);
        log.push(format!(
            "Day {}: {} — {}",
            game.0.fortress.day, result.event_name, result.choice_label
        ));

        // swap modal content for the result panel
        for root in roots.iter() {
            commands.entity(root).despawn();
        }
        spawn_result_panel(&mut commands, &result);
        commands.remove_resource::<ActiveEvent>();
        return;
    }
}

fn spawn_result_panel(commands: &mut Commands, result: &fortress_core::EventResult) {
    commands
        .spawn((
            ModalRoot,
            DespawnOnExit(AppState::EventModal),
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
                        width: Val::Px(620.0),
                        padding: UiRect::all(Val::Px(18.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..Default::default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|panel| {
                    panel.spawn(text(result.choice_label.clone(), 20.0, ACCENT));
                    let body = if result.lines.is_empty() {
                        "Nothing comes of it.".to_string()
                    } else {
                        result.lines.join("\n")
                    };
                    panel.spawn(text(body, 16.0, Color::WHITE));
                    panel
                        .spawn((
                            ContinueButton,
                            Button,
                            button_node(),
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text("Continue »", 18.0, Color::WHITE));
                        });
                });
        });
}

fn continue_click(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<ContinueButton>)>,
    mut game: ResMut<Game>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let clicked = interactions.iter().any(|i| *i == Interaction::Pressed);
    if !clicked {
        return;
    }
    // Level-up check: if the threshold was just crossed, branch to the ability draft.
    if game.0.should_level_up() {
        let offers = game.0.ability_offers();
        if !offers.is_empty() {
            commands.insert_resource(LevelUpOffers(offers));
            next_state.set(AppState::LevelUp);
            return;
        }
    }
    // The day continues — the clock runs finish_day at midnight.
    next_state.set(AppState::FortressView);
}
