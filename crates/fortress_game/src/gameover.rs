use bevy::prelude::*;

use crate::bridge::Game;
use crate::ui::{button_node, text, tint_buttons, ACCENT, BTN_BG, PANEL_BG};
use crate::AppState;

pub struct GameOverPlugin;

impl Plugin for GameOverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::GameOver), spawn_screen)
            .add_systems(
                Update,
                (menu_button, tint_buttons).run_if(in_state(AppState::GameOver)),
            );
    }
}

#[derive(Component)]
struct MenuButton;

fn fall_epitaph(days: u32) -> &'static str {
    match days {
        0..=5 => "A swift and brutal end. The fortress barely drew breath.",
        6..=14 => "The walls were never truly tested before they fell.",
        15..=29 => "A hard-fought stand. Few fortresses last this long.",
        30..=59 => "A seasoned commander's end — history will remember this place.",
        60..=99 => "Decades of defiance. The songs will outlast the stones.",
        _ => "A LEGEND FALLS. Bards will sing of this fortress for generations.",
    }
}

fn spawn_screen(mut commands: Commands, game: Res<Game>) {
    let gs = &game.0;
    let days = gs.fortress.day.saturating_sub(1);
    let epitaph = fall_epitaph(days);

    let player_line = gs
        .player
        .as_ref()
        .map(|p| {
            let ability_list = if p.abilities.is_empty() {
                "—".to_string()
            } else {
                p.abilities.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")
            };
            format!(
                "{} the {}  (Lv.{})  |  Might {}  Wit {}  Heart {}\nAbilities: {}",
                p.name,
                p.class.name(),
                p.level,
                p.stats.might,
                p.stats.wit,
                p.stats.heart,
                ability_list,
            )
        })
        .unwrap_or_default();

    let upgrades = if gs.fortress.upgrades.is_empty() {
        "none".to_string()
    } else {
        gs.fortress.upgrades.iter().map(|u| u.name()).collect::<Vec<_>>().join(", ")
    };

    let stats = format!(
        "{player_line}\n\nDays survived: {days}\nEvents faced: {}\nInhabitants alive: {}\nInhabitants lost: {}\nUpgrades built: {}\nRun seed: {}",
        gs.events_resolved,
        gs.inhabitants.count_alive(),
        gs.inhabitants.count_dead(),
        upgrades,
        gs.run_seed,
    );

    commands
        .spawn((
            DespawnOnExit(AppState::GameOver),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.8)),
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
                    panel.spawn(text("THE FORTRESS HAS FALLEN", 22.0, Color::srgb(0.9, 0.25, 0.25)));
                    panel.spawn(text(epitaph, 16.0, Color::srgb(0.85, 0.75, 0.55)));
                    panel.spawn(text(stats, 14.0, Color::WHITE));
                    panel
                        .spawn((MenuButton, Button, button_node(), BackgroundColor(BTN_BG)))
                        .with_children(|b| {
                            b.spawn(text("Begin a New Watch »", 18.0, ACCENT));
                        });
                });
        });
}

fn menu_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<MenuButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        next_state.set(AppState::Title);
    }
}
