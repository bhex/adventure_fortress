use bevy::prelude::*;

use crate::bridge::{save_path, Game, GameLog};
use crate::ui::{button_node, text, tint_buttons, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct TitlePlugin;

impl Plugin for TitlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Title), spawn_title)
            .add_systems(
                Update,
                (title_buttons, tint_buttons).run_if(in_state(AppState::Title)),
            );
    }
}

#[derive(Component, Clone, Copy)]
enum TitleAction {
    NewGame,
    Continue,
}

const BANNER: &str = r#"
 █████╗ ██████╗ ██╗   ██╗███████╗███╗   ██╗████████╗██╗   ██╗██████╗ ███████╗
██╔══██╗██╔══██╗██║   ██║██╔════╝████╗  ██║╚══██╔══╝██║   ██║██╔══██╗██╔════╝
███████║██║  ██║╚██╗ ██╔╝█████╗  ██╔██╗ ██║   ██║   ██║   ██║██████╔╝█████╗
██╔══██║██║  ██║ ╚████╔╝ ██╔══╝  ██║╚██╗██║   ██║   ██║   ██║██╔══██╗██╔══╝
██║  ██║██████╔╝  ╚██╔╝  ███████╗██║ ╚████║   ██║   ╚██████╔╝██║  ██║███████╗
╚═╝  ╚═╝╚═════╝    ╚═╝   ╚══════╝╚═╝  ╚═══╝   ╚═╝    ╚═════╝ ╚═╝  ╚═╝╚══════╝
                        F O R T R E S S
"#;

fn spawn_title(mut commands: Commands) {
    let has_save = save_path().exists();

    commands
        .spawn((
            DespawnOnExit(AppState::Title),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..Default::default()
            },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.04)),
        ))
        .with_children(|parent| {
            parent.spawn(text(BANNER, 14.0, ACCENT));
            parent.spawn(text(
                "Hold the fortress as long as you can. Every choice matters. Losing is fun.",
                16.0,
                TEXT_DIM,
            ));
            parent.spawn(Node {
                height: Val::Px(20.0),
                ..Default::default()
            });

            parent
                .spawn((
                    TitleAction::NewGame,
                    Button,
                    button_node(),
                    BackgroundColor(BTN_BG),
                ))
                .with_children(|b| {
                    b.spawn(text("New Game", 20.0, Color::WHITE));
                });

            if has_save {
                parent
                    .spawn((
                        TitleAction::Continue,
                        Button,
                        button_node(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|b| {
                        b.spawn(text("Continue", 20.0, Color::WHITE));
                    });
            }

            parent.spawn(Node {
                height: Val::Px(16.0),
                ..Default::default()
            });
            parent.spawn((
                text(
                    "a fortress roguelite — bevy / wslg edition",
                    13.0,
                    TEXT_DIM,
                ),
                BackgroundColor(PANEL_BG),
            ));
        });
}

fn title_buttons(
    mut commands: Commands,
    interactions: Query<(&Interaction, &TitleAction), Changed<Interaction>>,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, action) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            TitleAction::NewGame => next_state.set(AppState::CharCreation),
            TitleAction::Continue => match fortress_core::GameState::load(&save_path()) {
                Ok(gs) => {
                    log.push(format!("Welcome back to {}.", gs.fortress.name));
                    commands.insert_resource(Game(gs));
                    commands.insert_resource(crate::clock::DayCycle::default());
                    next_state.set(AppState::FortressView);
                }
                Err(e) => {
                    log.push(format!("Could not load save: {e}. Starting fresh."));
                    let _ = std::fs::remove_file(save_path());
                    next_state.set(AppState::CharCreation);
                }
            },
        }
    }
}
