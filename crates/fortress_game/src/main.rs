mod actors;
mod bridge;
mod build;
mod charcreate;
mod clock;
mod gameover;
mod levelup;
mod map;
mod modal;
mod picking;
mod region_panel;
mod roster;
mod title;
mod ui;

use bevy::prelude::*;
use bevy_ascii_terminal::*;

use crate::bridge::{EngineCtl, EventDeck, GameLog, Selected};

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Title,
    CharCreation,
    FortressView,
    EventModal,
    BuildMenu,
    RegionView,
    LevelUp,
    GameOver,
}

pub const MAP_W: usize = 40;
pub const MAP_H: usize = 25;

fn main() {
    let dir = fortress_core::content::default_content_dir()
        .expect("could not locate content/events — run from the repo root or set FORTRESS_CONTENT");
    let deck = fortress_core::content::load_events(&dir)
        .unwrap_or_else(|e| panic!("failed to load events: {e}"));

    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Adventure Fortress".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        )
        .add_plugins(TerminalPlugins)
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(EventDeck(deck))
        .init_resource::<EngineCtl>()
        .init_resource::<GameLog>()
        .init_resource::<Selected>()
        .init_state::<AppState>()
        .add_systems(PreStartup, override_default_font)
        .add_systems(Startup, setup_terminal)
        .add_plugins((
            title::TitlePlugin,
            charcreate::CharCreatePlugin,
            map::MapPlugin,
            actors::ActorsPlugin,
            picking::PickingPlugin,
            ui::HudPlugin,
            roster::RosterPlugin,
            clock::ClockPlugin,
            modal::ModalPlugin,
            build::BuildMenuPlugin,
            region_panel::RegionPanelPlugin,
            levelup::LevelUpPlugin,
            gameover::GameOverPlugin,
        ))
        .run();
}

fn setup_terminal(mut commands: Commands) {
    commands.spawn(Terminal::new([MAP_W, MAP_H]));
    commands.spawn(TerminalCamera::new());
}

/// Bevy's bundled default font only covers a small glyph subset, which renders
/// tofu boxes and corrupts the glyph atlas. Replace it with a full font so all
/// UI text (em dashes, box-drawing banner, bullets) renders correctly.
fn override_default_font(mut fonts: ResMut<Assets<Font>>) {
    let font = Font::try_from_bytes(
        include_bytes!("../assets/fonts/DejaVuSansMono.ttf").to_vec(),
    )
    .expect("bundled DejaVuSansMono.ttf is a valid font");
    let _ = fonts.insert(&Handle::<Font>::default(), font);
}
