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
                (close_panel, tint_buttons, expedition_click).run_if(in_state(AppState::RegionView)),
            )
            .add_systems(Update, open_key.run_if(in_state(AppState::FortressView)));
    }
}

#[derive(Component)]
struct CloseButton;

#[derive(Component)]
struct ExpeditionButton(String);

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
    let mut targets = Vec::new();
    for site in &gs.region.sites {
        targets.push(site.name.clone());
    }
    if gs.flags.contains("artifact_hint_1") && !gs.flags.contains("artifact_retrieved") {
        targets.push("The Forgotten Vault".to_string());
    }

    if targets.is_empty() {
        panel.spawn(text("Nothing stands. The dark has the field.", 14.0, band_color("besieged")));
        return;
    }

    let heroes_avail = !gs.adventurers.is_empty();

    for site_name in targets {
        let (kind_name, strength_band, glyph, bar) = if let Some(site) = gs.region.sites.iter().find(|s| s.name == site_name) {
            let filled = (site.strength.clamp(0, 14) / 2) as usize;
            let b = format!("{}{}", "█".repeat(filled), "░".repeat(7 - filled.min(7)));
            (site.kind.name(), site.strength_band(), site_glyph(site.kind), b)
        } else {
            ("ancient ruin", "unknown", '*', "███████".to_string())
        };

        let label = format!(
            "{} {:<22} {} {}",
            glyph,
            format!("{} ({})", site_name, kind_name),
            bar,
            strength_band
        );

        if heroes_avail {
            let mut button = panel.spawn((
                ExpeditionButton(site_name.clone()),
                Button,
                Node {
                    padding: UiRect::all(Val::Px(6.0)),
                    margin: UiRect::vertical(Val::Px(2.0)),
                    ..Default::default()
                },
                BackgroundColor(BTN_BG),
            ));
            button.with_children(|b| {
                b.spawn(text(label, 14.0, band_color(strength_band)));
                b.spawn(text("  [Send Expedition]", 13.0, ACCENT));
            });
        } else {
            panel.spawn(text(label, 14.0, band_color(strength_band)));
        }
    }
}

fn expedition_click(
    interactions: Query<(&Interaction, &ExpeditionButton), Changed<Interaction>>,
    mut game: ResMut<Game>,
    mut log: ResMut<crate::bridge::GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, btn) in interactions.iter() {
        if *interaction != Interaction::Pressed { continue; }
        
        if game.0.adventurers.is_empty() {
            log.push("No heroes available to send.".to_string());
            return;
        }
        
        // Take all adventurers
        let heroes = std::mem::take(&mut game.0.adventurers);
        let count = heroes.len();
        
        game.0.expeditions.push(fortress_core::Expedition {
            target_site_name: btn.0.clone(),
            days_remaining: 5,
            heroes,
        });
        
        log.push(format!("Day {}: Dispatched {} heroes on an expedition to {}.", game.0.fortress.day, count, btn.0));
        next_state.set(AppState::FortressView);
        return;
    }
}
