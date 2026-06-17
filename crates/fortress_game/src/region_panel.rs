//! The regional overworld: a procedurally-generated, colour terrain map with
//! roads, the fortress as its focal point, and every site/portal placed on it.
//! Drawn into the shared terminal with half-block sub-pixels (so the map renders
//! at double vertical resolution), only when something changes. A docked panel
//! on the right carries the darkness gauge, a legend, and the selected hold's
//! detail + expedition order. Toggled with `R`; the world's clock is frozen.

use bevy::prelude::*;
use bevy_ascii_terminal::*;

use fortress_core::{Coord, SiteKind, FORTRESS_POS, REGION_H, REGION_W};

use crate::bridge::{Game, GameLog};
use crate::picking::cursor_tile;
use crate::region_map::RegionMap;
use crate::ui::{button_node, text, tint_buttons, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct RegionPanelPlugin;

impl Plugin for RegionPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RegionMap>()
            .init_resource::<RegionSelection>()
            .init_resource::<RegionHover>()
            .add_systems(OnEnter(AppState::RegionView), enter_region)
            .add_systems(
                Update,
                (
                    region_hover,
                    region_click,
                    redraw_region,
                    refresh_sidebar,
                    expedition_click,
                    close_panel,
                    tint_buttons,
                )
                    .run_if(in_state(AppState::RegionView)),
            )
            .add_systems(Update, open_key.run_if(in_state(AppState::FortressView)));
    }
}

#[derive(Resource, Default)]
struct RegionSelection(Option<String>);

#[derive(Resource, Default, PartialEq)]
struct RegionHover(Option<IVec2>);

#[derive(Component)]
struct SidebarRoot;

#[derive(Component)]
struct SidebarRow;

#[derive(Component)]
struct CloseButton;

#[derive(Component)]
struct ExpeditionButton(String);

// ---------------------------------------------------------------------------
// enter / leave
// ---------------------------------------------------------------------------

fn open_key(keys: Res<ButtonInput<KeyCode>>, mut next_state: ResMut<NextState<AppState>>) {
    if keys.just_pressed(KeyCode::KeyR) {
        next_state.set(AppState::RegionView);
    }
}

fn enter_region(
    mut commands: Commands,
    mut terminals: Query<&mut Terminal>,
    game: Res<Game>,
    mut map: ResMut<RegionMap>,
    mut sel: ResMut<RegionSelection>,
    mut hover: ResMut<RegionHover>,
) {
    if let Ok(mut term) = terminals.single_mut() {
        term.resize([REGION_W as usize, REGION_H as usize]);
    }
    map.ensure(&game.0); // build terrain + roads if the region changed
    sel.0 = None;
    hover.0 = None;

    // docked info panel on the right; the map fills the rest of the screen
    commands.spawn((
        SidebarRoot,
        DespawnOnExit(AppState::RegionView),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(330.0),
            padding: UiRect::all(Val::Px(14.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..Default::default()
        },
        BackgroundColor(PANEL_BG),
    ));
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

// ---------------------------------------------------------------------------
// interaction
// ---------------------------------------------------------------------------

fn region_hover(
    windows: Query<&Window>,
    camera: Query<(&Camera, &GlobalTransform), With<TerminalCamera>>,
    terminal: Query<&TerminalTransform>,
    mut hover: ResMut<RegionHover>,
) {
    hover.set_if_neq(RegionHover(cursor_tile(&windows, &camera, &terminal)));
}

fn region_click(
    buttons: Res<ButtonInput<MouseButton>>,
    hover: Res<RegionHover>,
    game: Res<Game>,
    mut sel: ResMut<RegionSelection>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(tile) = hover.0 else { return };
    let c = Coord::new(tile.x as i16, tile.y as i16);
    // select the nearest hold within a couple of tiles of the click
    sel.0 = game
        .0
        .region
        .sites
        .iter()
        .filter(|s| s.pos.dist(c) <= 2)
        .min_by_key(|s| s.pos.dist(c))
        .map(|s| s.name.clone());
}

fn expedition_click(
    interactions: Query<(&Interaction, &ExpeditionButton), Changed<Interaction>>,
    mut game: ResMut<Game>,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (interaction, btn) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if game.0.adventurers.is_empty() {
            log.push("No heroes available to send.".to_string());
            return;
        }
        let days = game.0.region.expedition_days(&btn.0);
        let heroes = std::mem::take(&mut game.0.adventurers);
        let count = heroes.len();
        game.0.expeditions.push(fortress_core::Expedition {
            target_site_name: btn.0.clone(),
            days_remaining: days,
            heroes,
        });
        log.push(format!(
            "Day {}: Dispatched {} heroes to {} — a {}-day march.",
            game.0.fortress.day, count, btn.0, days
        ));
        next_state.set(AppState::FortressView);
        return;
    }
}

// ---------------------------------------------------------------------------
// map rendering (half-block terrain + overlays), redrawn only on change
// ---------------------------------------------------------------------------

const ROAD: Color = Color::srgb(0.62, 0.50, 0.33);
const PORTAL: Color = Color::srgb(0.72, 0.26, 0.86);
const HERO: Color = Color::srgb(0.55, 0.8, 0.95);
const REFUGEE: Color = Color::srgb(0.82, 0.78, 0.6);

fn redraw_region(
    mut terminals: Query<&mut Terminal>,
    game: Res<Game>,
    map: Res<RegionMap>,
    sel: Res<RegionSelection>,
    hover: Res<RegionHover>,
) {
    if !sel.is_changed() && !hover.is_changed() && !map.is_changed() && !game.is_changed() {
        return;
    }
    let Ok(mut term) = terminals.single_mut() else {
        return;
    };
    let gs = &game.0;
    term.clear();

    // 1) terrain, two sub-pixels per cell via the upper-half-block glyph
    for y in 0..REGION_H as i32 {
        for x in 0..REGION_W as i32 {
            let blight = gs.region.in_blight(Coord::new(x as i16, y as i16));
            let top = terrain_color(&map, x, y * 2, blight);
            let bottom = terrain_color(&map, x, y * 2 + 1, blight);
            term.put_char([x as usize, y as usize], '▀').fg(top).bg(bottom);
        }
    }

    // 2) roads
    for y in 0..REGION_H {
        for x in 0..REGION_W {
            if map.is_road(x, y) {
                let bg = blend(
                    terrain_color(&map, x as i32, y as i32 * 2, false),
                    terrain_color(&map, x as i32, y as i32 * 2 + 1, false),
                );
                term.put_char([x as usize, y as usize], '∙').fg(ROAD).bg(bg);
            }
        }
    }

    // 3) refugees trudging the roads toward the gate
    if gs.region.refugees_incoming() {
        let mut near: Vec<&(i16, i16)> = map
            .roads
            .iter()
            .filter(|(x, y)| {
                let d = Coord::new(*x, *y).dist(FORTRESS_POS);
                (4..=10).contains(&d)
            })
            .collect();
        near.sort_by_key(|(x, y)| Coord::new(*x, *y).dist(FORTRESS_POS));
        for (x, y) in near.into_iter().take(3) {
            put_marker(&mut term, &map, *x, *y, 'o', REFUGEE);
        }
    }

    // 4) expeditions on the march — a hero marker partway to the target
    for exp in &gs.expeditions {
        if let Some(site) = gs.region.sites.iter().find(|s| s.name == exp.target_site_name) {
            let mx = (FORTRESS_POS.x + site.pos.x) / 2;
            let my = (FORTRESS_POS.y + site.pos.y) / 2;
            put_marker(&mut term, &map, mx, my, '&', HERO);
        }
    }

    // 5) demon portals brooding on the edges (CP437 sun '☼' reads as a baleful star)
    for p in &gs.region.portals {
        put_marker(&mut term, &map, p.x, p.y, '☼', PORTAL);
    }

    // 6) the free holds, coloured by how they fare; selected one ringed
    for site in &gs.region.sites {
        let selected = sel.0.as_deref() == Some(site.name.as_str());
        let bg = if selected {
            Color::srgb(0.45, 0.42, 0.16)
        } else {
            cell_bg(&map, site.pos.x, site.pos.y)
        };
        let color = band_color(site.strength_band());
        term.put_char([site.pos.x as usize, site.pos.y as usize], site_glyph(site.kind))
            .fg(color)
            .bg(bg);
    }

    // 7) the fortress — the focal point
    term.put_char([FORTRESS_POS.x as usize, FORTRESS_POS.y as usize], '⌂')
        .fg(ACCENT)
        .bg(Color::srgb(0.3, 0.26, 0.12));

    // 8) hover highlight
    if let Some(t) = hover.0 {
        if (0..REGION_W as i32).contains(&t.x) && (0..REGION_H as i32).contains(&t.y) {
            term.tile_mut([t.x as usize, t.y as usize]).bg(Color::srgb(0.32, 0.32, 0.45));
        }
    }
}

/// Draw a glyph marker over the terrain, keeping the land tone behind it.
fn put_marker(term: &mut Terminal, map: &RegionMap, x: i16, y: i16, ch: char, color: Color) {
    if !(0..REGION_W).contains(&x) || !(0..REGION_H).contains(&y) {
        return;
    }
    let bg = cell_bg(map, x, y);
    term.put_char([x as usize, y as usize], ch).fg(color).bg(bg);
}

fn cell_bg(map: &RegionMap, x: i16, y: i16) -> Color {
    blend(
        terrain_color(map, x as i32, y as i32 * 2, false),
        terrain_color(map, x as i32, y as i32 * 2 + 1, false),
    )
}

/// Per-tile dithered, optionally blighted biome colour.
fn terrain_color(map: &RegionMap, x: i32, sy: i32, blight: bool) -> Color {
    let base = map.biome_at(x, sy).color();
    let c = scale(base, 0.88 + 0.24 * jitter(x, sy));
    if blight {
        blighted(c)
    } else {
        c
    }
}

fn jitter(x: i32, y: i32) -> f32 {
    let mut h = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h >> 16) & 0xFF) as f32 / 255.0
}

fn scale(c: Color, k: f32) -> Color {
    let s = c.to_srgba();
    Color::srgb((s.red * k).min(1.0), (s.green * k).min(1.0), (s.blue * k).min(1.0))
}

fn blend(a: Color, b: Color) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    Color::srgb((a.red + b.red) * 0.5, (a.green + b.green) * 0.5, (a.blue + b.blue) * 0.5)
}

/// Pull a colour toward a sickly purple gloom — the darkness made visible.
fn blighted(c: Color) -> Color {
    let s = c.to_srgba();
    let t = 0.55;
    let mix = |ch: f32, target: f32| (ch * (1.0 - t) + target * t) * 0.82;
    Color::srgb(mix(s.red, 0.30), mix(s.green, 0.10), mix(s.blue, 0.36))
}

// ---------------------------------------------------------------------------
// docked info panel
// ---------------------------------------------------------------------------

fn refresh_sidebar(
    mut commands: Commands,
    game: Res<Game>,
    sel: Res<RegionSelection>,
    root: Query<Entity, With<SidebarRoot>>,
    rows: Query<Entity, With<SidebarRow>>,
    fresh: Query<(), Added<SidebarRoot>>,
) {
    if !sel.is_changed() && !game.is_changed() && fresh.is_empty() {
        return;
    }
    let Ok(root) = root.single() else { return };
    for r in rows.iter() {
        commands.entity(r).despawn();
    }
    let gs = &game.0;
    let r = &gs.region;

    commands.entity(root).with_children(|p| {
        p.spawn((SidebarRow, text("»  The Region".to_string(), 22.0, ACCENT)));
        p.spawn((SidebarRow, text(format!("season: {}", gs.world.describe()), 13.0, TEXT_DIM)));
        p.spawn((
            SidebarRow,
            text(
                format!("darkness {} {} ({})", meter(r.darkness), r.darkness, r.band().name()),
                14.0,
                band_color(if r.darkness >= 50 { "failing" } else { "holding" }),
            ),
        ));
        p.spawn((
            SidebarRow,
            text(
                format!("portals: {}   ·   holds standing: {}", r.portals.len(), r.standing_sites()),
                12.0,
                TEXT_DIM,
            ),
        ));
        if r.refugees_incoming() {
            p.spawn((
                SidebarRow,
                text("Refugees are on the roads to your gates.".to_string(), 12.0, ACCENT),
            ));
        }
        if r.all_fallen() {
            p.spawn((
                SidebarRow,
                text(
                    "The region has fallen. Watch for survivors.".to_string(),
                    12.0,
                    band_color("besieged"),
                ),
            ));
        }

        // selected hold detail + expedition order
        p.spawn((SidebarRow, text("─ selected ─".to_string(), 12.0, TEXT_DIM)));
        match sel.0.as_deref().and_then(|n| r.sites.iter().find(|s| s.name == n)) {
            Some(site) => {
                p.spawn((
                    SidebarRow,
                    text(
                        format!("{} {} ({})", site_glyph(site.kind), site.name, site.kind.name()),
                        15.0,
                        band_color(site.strength_band()),
                    ),
                ));
                let dist = FORTRESS_POS.dist(site.pos);
                p.spawn((
                    SidebarRow,
                    text(
                        format!(
                            "{} · {} tiles off · {}-day march",
                            site.strength_band(),
                            dist,
                            r.expedition_days(&site.name)
                        ),
                        12.0,
                        TEXT_DIM,
                    ),
                ));
                if gs.adventurers.is_empty() {
                    p.spawn((SidebarRow, text("(no heroes to send)".to_string(), 12.0, TEXT_DIM)));
                } else {
                    p.spawn((
                        SidebarRow,
                        ExpeditionButton(site.name.clone()),
                        Button,
                        button_node(),
                        BackgroundColor(BTN_BG),
                    ))
                    .with_children(|b| {
                        b.spawn(text(
                            format!("Send {} heroes »", gs.adventurers.len()),
                            14.0,
                            ACCENT,
                        ));
                    });
                }
            }
            None => {
                p.spawn((SidebarRow, text("Click a hold on the map.".to_string(), 12.0, TEXT_DIM)));
            }
        }

        // legend
        p.spawn((SidebarRow, text("─ legend ─".to_string(), 12.0, TEXT_DIM)));
        p.spawn((SidebarRow, text("⌂ fortress   ☼ portal   ∙ road".to_string(), 12.0, TEXT_DIM)));
        p.spawn((
            SidebarRow,
            text("# city  @ fort  % mercs  & band  + camp".to_string(), 12.0, TEXT_DIM),
        ));

        p.spawn((SidebarRow, CloseButton, Button, button_node(), BackgroundColor(BTN_BG)))
            .with_children(|b| {
                b.spawn(text("Close (Esc / R)", 15.0, Color::WHITE));
            });
    });
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

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
