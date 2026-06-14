//! The region overlay: the darkness war beyond the walls, made legible.
//! Toggled with `R` (or the HUD button); pauses time like the build menu.

use bevy::prelude::*;

use fortress_core::{GameState, SiteKind};

use crate::bridge::Game;
use crate::ui::{button_node, text, tint_buttons, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct RegionPanelPlugin;

impl Plugin for RegionPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::RegionView), spawn_panel)
            .add_systems(
                Update,
                (close_panel, tint_buttons).run_if(in_state(AppState::RegionView)),
            )
            .add_systems(Update, open_key.run_if(in_state(AppState::FortressView)));
    }
}

#[derive(Component)]
struct CloseButton;

fn open_key(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::RegionView);
    }
}

fn close_panel(
    keys: Res<ButtonInput<KeyCode>>,
    interactions: Query<&Interaction, (Changed<Interaction>, With<CloseButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let clicked = interactions.iter().any(|i| *i == Interaction::Pressed);
    if clicked || keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::FortressView);
    }
}

/// A 20-cell filled/empty bar for a 0..=100 value.
fn meter(value: i32) -> String {
    let filled = (value.clamp(0, 100) * 20 / 100) as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(20 - filled))
}

fn site_glyph(kind: SiteKind) -> char {
    match kind {
        SiteKind::City => '#',
        SiteKind::Fortress => '@',
        SiteKind::MercCompany => '%',
        SiteKind::AdventurerBand => '&',
        SiteKind::Survivors => '+',
    }
}

fn band_color(band: &str) -> Color {
    match band {
        "thriving" => Color::srgb(0.4, 0.85, 0.4),
        "holding" => Color::srgb(0.85, 0.85, 0.4),
        "failing" => Color::srgb(0.9, 0.6, 0.3),
        _ => Color::srgb(0.9, 0.35, 0.35),
    }
}

fn spawn_panel(mut commands: Commands, game: Res<Game>) {
    let gs = &game.0;
    commands
        .spawn((
            DespawnOnExit(AppState::RegionView),
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
                        row_gap: Val::Px(5.0),
                        ..Default::default()
                    },
                    BackgroundColor(PANEL_BG),
                ))
                .with_children(|panel| {
                    panel.spawn(text("»  The Region", 22.0, ACCENT));
                    region_summary(panel, gs);
                    panel.spawn(text("─ the free peoples ─", 13.0, TEXT_DIM));
                    site_list(panel, gs);
                    panel
                        .spawn((CloseButton, Button, button_node(), BackgroundColor(BTN_BG)))
                        .with_children(|b| {
                            b.spawn(text("Close (Esc / R)", 16.0, Color::WHITE));
                        });
                });
        });
}

fn region_summary(panel: &mut ChildSpawnerCommands, gs: &GameState) {
    let r = &gs.region;
    panel.spawn(text(format!("season: {}", gs.world.describe()), 14.0, ACCENT));
    panel.spawn(text(
        format!("darkness  {}  {}  ({})", meter(r.darkness), r.darkness, r.band().name()),
        15.0,
        band_color(if r.darkness >= 50 { "failing" } else { "holding" }),
    ));
    if r.all_fallen() {
        panel.spawn(text(
            "The region has fallen. Watch for survivors regrouping from the ruin.",
            13.0,
            band_color("besieged"),
        ));
    }
    panel.spawn(text(
        format!(
            "portal pressure: {}   ·   sites standing: {}",
            r.portal_pressure,
            r.standing_sites()
        ),
        13.0,
        TEXT_DIM,
    ));
    if r.refugees_incoming() {
        panel.spawn(text("Refugees are on the roads to your gates.", 13.0, ACCENT));
    }
}

fn site_list(panel: &mut ChildSpawnerCommands, gs: &GameState) {
    if gs.region.sites.is_empty() {
        panel.spawn(text("Nothing stands. The dark has the field.", 14.0, band_color("besieged")));
        return;
    }
    for site in &gs.region.sites {
        let band = site.strength_band();
        let bar = {
            let filled = (site.strength.clamp(0, 14) / 2) as usize;
            format!("{}{}", "█".repeat(filled), "░".repeat(7 - filled.min(7)))
        };
        panel.spawn(text(
            format!(
                "{} {:<22} {} {}",
                site_glyph(site.kind),
                format!("{} ({})", site.name, site.kind.name()),
                bar,
                band
            ),
            14.0,
            band_color(band),
        ));
    }
}
