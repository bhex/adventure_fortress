//! Ability draft screen shown when the commander reaches a new level.

use bevy::prelude::*;

use fortress_core::PlayerAbility;

use crate::bridge::{Game, GameLog, LevelUpOffers};
use crate::ui::{text, tint_buttons, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct LevelUpPlugin;

impl Plugin for LevelUpPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::LevelUp), spawn_screen)
            .add_systems(
                Update,
                (ability_click, tint_buttons).run_if(in_state(AppState::LevelUp)),
            );
    }
}

#[derive(Component, Clone, Copy)]
struct AbilityButton(PlayerAbility);

fn spawn_screen(mut commands: Commands, game: Res<Game>, offers: Res<LevelUpOffers>) {
    let next_level = game.0.player.as_ref().map(|p| p.level + 1).unwrap_or(2);
    let commander = game
        .0
        .player
        .as_ref()
        .map(|p| format!("{} the {}", p.name, p.class.name()))
        .unwrap_or_default();

    commands
        .spawn((
            DespawnOnExit(AppState::LevelUp),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.82)),
        ))
        .with_children(|overlay| {
            overlay
                .spawn((
                    Node {
                        width: Val::Px(600.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(10.0),
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|panel| {
                    panel.spawn(text(
                        format!("COMMANDER GROWS IN LEGEND"),
                        24.0,
                        Color::srgb(0.95, 0.8, 0.3),
                    ));
                    panel.spawn(text(
                        format!("{} — Level {}", commander, next_level),
                        16.0,
                        Color::WHITE,
                    ));
                    panel.spawn(text("Choose one ability:", 15.0, TEXT_DIM));
                    panel.spawn(Node { height: Val::Px(6.0), ..Default::default() });

                    for ability in offers.0.iter() {
                        panel
                            .spawn((
                                AbilityButton(*ability),
                                Button,
                                Node {
                                    width: Val::Percent(100.0),
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::FlexStart,
                                    padding: UiRect::all(Val::Px(14.0)),
                                    margin: UiRect::vertical(Val::Px(3.0)),
                                    ..Default::default()
                                },
                                BackgroundColor(BTN_BG),
                            ))
                            .with_children(|b| {
                                b.spawn(text(ability.name(), 19.0, Color::WHITE));
                                b.spawn(text(ability.description(), 13.0, TEXT_DIM));
                            });
                    }
                });
        });
}

fn ability_click(
    interactions: Query<(&Interaction, &AbilityButton), Changed<Interaction>>,
    mut commands: Commands,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, button) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let ability = button.0;
        game.0.apply_level_up(ability);
        log.push(format!(
            "» Commander reaches level {} — {} gained.",
            game.0.player.as_ref().map(|p| p.level).unwrap_or(0),
            ability.name()
        ));
        commands.remove_resource::<LevelUpOffers>();
        // Back to the running day — the clock owns midnight.
        next_state.set(AppState::FortressView);
        return;
    }
}
