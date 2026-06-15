//! HUD (top bar), log panel, inspect panel, End Day button — plus small
//! UI helpers shared by the other screens.

use bevy::prelude::*;

use fortress_core::{Role, Upgrade};

use crate::bridge::{AutoMode, Game, GameLog, Selected, Selection};
use crate::clock::{ClockSpeed, DayPhase, GameClock};
use crate::AppState;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::FortressView), spawn_hud)
            .add_systems(
                Update,
                (
                    update_hud_text,
                    update_clock_text,
                    update_log,
                    update_inspect,
                    speed_buttons,
                    build_hud_button,
                    region_hud_button,
                    auto_hud_button,
                )
                    .run_if(in_state(AppState::FortressView)),
            );
    }
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

pub const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.1, 0.92);
pub const BTN_BG: Color = Color::srgb(0.16, 0.18, 0.24);
pub const BTN_BG_HOVER: Color = Color::srgb(0.24, 0.28, 0.38);
pub const BTN_BG_DISABLED: Color = Color::srgb(0.1, 0.1, 0.12);
pub const TEXT_DIM: Color = Color::srgb(0.6, 0.6, 0.65);
pub const ACCENT: Color = Color::srgb(0.95, 0.8, 0.3);

pub fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont {
            font_size: size,
            ..Default::default()
        },
        TextColor(color),
    )
}

pub fn button_node() -> Node {
    Node {
        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
        margin: UiRect::all(Val::Px(4.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

/// Generic hover tint for any enabled button.
pub fn tint_buttons(
    mut query: Query<(&Interaction, &mut BackgroundColor, Option<&Disabled>), With<Button>>,
) {
    for (interaction, mut bg, disabled) in query.iter_mut() {
        if disabled.is_some() {
            *bg = BTN_BG_DISABLED.into();
            continue;
        }
        *bg = match interaction {
            Interaction::Hovered | Interaction::Pressed => BTN_BG_HOVER.into(),
            Interaction::None => BTN_BG.into(),
        };
    }
}

#[derive(Component)]
pub struct Disabled;

// ---------------------------------------------------------------------------
// HUD
// ---------------------------------------------------------------------------

#[derive(Component)]
struct HudText;

#[derive(Component)]
struct LogPanel;

#[derive(Component)]
struct LogLine;

#[derive(Component)]
struct InspectText;

#[derive(Component)]
struct ClockText;

#[derive(Component, Clone, Copy)]
enum SpeedButton {
    Pause,
    Normal,
    Fast,
    SkipToDawn,
}

#[derive(Component)]
struct BuildHudButton;

#[derive(Component)]
struct RegionHudButton;

#[derive(Component)]
struct AutoHudButton;

fn spawn_hud(mut commands: Commands) {
    // top bar
    commands
        .spawn((
            DespawnOnExit(AppState::FortressView),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                padding: UiRect::all(Val::Px(8.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|parent| {
            parent.spawn((HudText, text("", 16.0, Color::WHITE)));
            parent
                .spawn(Node {
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    ..Default::default()
                })
                .with_children(|cluster| {
                    cluster
                        .spawn((
                            BuildHudButton,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                margin: UiRect::all(Val::Px(2.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text("build (B)", 14.0, Color::WHITE));
                        });
                    cluster
                        .spawn((
                            RegionHudButton,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                margin: UiRect::all(Val::Px(2.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text("region (R)", 14.0, Color::WHITE));
                        });
                    cluster
                        .spawn((
                            AutoHudButton,
                            Button,
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                margin: UiRect::all(Val::Px(2.0)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text("auto (A)", 14.0, Color::WHITE));
                        });
                    cluster.spawn((ClockText, text("", 16.0, ACCENT)));
                    for (which, label) in [
                        (SpeedButton::Pause, "||"),
                        (SpeedButton::Normal, ">"),
                        (SpeedButton::Fast, ">>"),
                        (SpeedButton::SkipToDawn, "dawn"),
                    ] {
                        cluster
                            .spawn((
                                which,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                                    margin: UiRect::all(Val::Px(2.0)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    ..Default::default()
                                },
                                BackgroundColor(BTN_BG),
                            ))
                            .with_children(|b| {
                                b.spawn(text(label, 14.0, Color::WHITE));
                            });
                    }
                });
        });

    // log panel, bottom — to the right of the roster column; lines are
    // spawned per-entry so each can be colored by what kind of news it is.
    commands.spawn((
        LogPanel,
        DespawnOnExit(AppState::FortressView),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(8.0),
            left: Val::Px(246.0),
            width: Val::Px(440.0),
            padding: UiRect::all(Val::Px(8.0)),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..Default::default()
        },
        BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
    ));

    // inspect panel, right
    commands
        .spawn((
            DespawnOnExit(AppState::FortressView),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(56.0),
                right: Val::Px(8.0),
                width: Val::Px(300.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            BackgroundColor(PANEL_BG),
        ))
        .with_children(|parent| {
            parent.spawn((InspectText, text("", 15.0, Color::WHITE)));
        });
}

fn update_hud_text(
    game: Res<Game>,
    auto: Res<AutoMode>,
    mut query: Query<&mut Text, With<HudText>>,
) {
    let Ok(mut t) = query.single_mut() else { return };
    let gs = &game.0;
    // hybrid: exact number + band word, e.g. "food 34 (adequate)"
    let stores: Vec<String> = [
        fortress_core::ResourceKind::Food,
        fortress_core::ResourceKind::Wood,
        fortress_core::ResourceKind::Stone,
    ]
    .iter()
    .map(|k| format!("{} {} ({})", k.name(), gs.resources.get(*k), gs.resources.band(*k).name()))
    .collect();
    // a compact always-on darkness gauge so the war has a presence
    let dark = gs.region.darkness.clamp(0, 100);
    let filled = (dark / 10) as usize;
    let gauge = format!("{}{}", "█".repeat(filled), "░".repeat(10 - filled));
    **t = format!(
        "Day {} — {} ({})  |  {}{}  |  Morale {}  Def {}  |  Pop {}/{}  |  {}  |  Dark {} {} ({})",
        gs.fortress.day,
        gs.fortress.name,
        gs.fortress.settlement_tier.name(),
        gs.world.describe(),
        if auto.0 { "  [AUTO]" } else { "" },
        gs.fortress.morale,
        gs.fortress.defense,
        gs.inhabitants.count_alive(),
        gs.fortress.max_population,
        stores.join(" · "),
        gauge,
        dark,
        gs.region.band().name(),
    );
}

/// The day's news, colored by what kind of news it is.
fn log_line_color(line: &str) -> Color {
    let l = line.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if has(&["falls", "dies", "succumb", "slips away", "deserts", "go hungry", "buckles", "lost", "sleep rough"]) {
        Color::srgb(0.9, 0.4, 0.4) // loss
    } else if has(&["muster", "wound", "holds the breach", "breaks and scatters", "banner", "battle", "raid", "siege"]) {
        Color::srgb(0.95, 0.65, 0.35) // battle
    } else if has(&["darkness", "portal", "refugee", "the dark", "veil"]) {
        Color::srgb(0.72, 0.55, 0.95) // the war beyond the walls
    } else if has(&["harvest", "forge", "armory", "trade", "valuables", "timber", "wood", "stone", "tools", "stores", "tavern"]) {
        Color::srgb(0.5, 0.82, 0.5) // economy
    } else if has(&["quiet day"]) {
        Color::srgb(0.45, 0.45, 0.5) // calm
    } else {
        TEXT_DIM
    }
}

fn update_log(
    mut commands: Commands,
    log: Res<GameLog>,
    panel: Query<Entity, With<LogPanel>>,
    lines: Query<Entity, With<LogLine>>,
) {
    if !log.is_changed() {
        return;
    }
    let Ok(panel) = panel.single() else { return };
    for line in lines.iter() {
        commands.entity(line).despawn();
    }
    let recent: Vec<&String> = log.0.iter().rev().take(8).collect();
    commands.entity(panel).with_children(|p| {
        for line in recent.into_iter().rev() {
            p.spawn((LogLine, text(line.clone(), 13.0, log_line_color(line))));
        }
    });
}

fn upgrade_blurb(u: Upgrade) -> &'static str {
    match u {
        Upgrade::Watchtower => "Watchtower\n+5 defense. Eyes on the horizon —\nscouts bring warning of attacks.",
        Upgrade::Farm => "Farm\n+3 food every day.",
        Upgrade::Infirmary => "Infirmary\nDisasters strike half as hard.\nHealers recover spirit daily.",
        Upgrade::Blacksmith => "Blacksmith\nYour people take less harm in battle.",
        Upgrade::Granary => "Granary\nSafer stores. Trade caravans now\nseek out the fortress.",
        Upgrade::Barracks => "Barracks\n+max population & defense.\nMore bunks for guards each tier.",
        Upgrade::Housing => "Housing\n+5 max population, +5 beds per plot.\nUp to four plots line the gate road.",
        Upgrade::Tavern => "Tavern\nDrink and song. Lifts morale\nmore with every tier.",
        Upgrade::Workshop => "Workshop\nTools and tinkering. Trains\nCrafting once raised to II.",
        Upgrade::Lumberyard => "Lumberyard\nFelled timber stacked high —\nwood every day, more each tier.",
        Upgrade::Shrine => "Shrine\nFaith against the dark. Softens\ndemon dread, more each tier.",
        Upgrade::TrainingYard => "Training Yard\nDrill and spar. Guards earn\nCombat practice every day.",
        Upgrade::Mine => "Mine\nStone every day — and a little raw\nmetal for the forge.",
        Upgrade::Graveyard => "Graveyard\nThe dead are honored here; grief\nweighs lighter on the living.",
        Upgrade::WizardTower => "Wizard Tower\nA seat for the arcane: enchanting\nand stranger work to come.",
    }
}

/// What a soul is carrying, one line — empty slots are simply omitted. Returns
/// "(unarmed)" when nothing is held, so the panel always says something.
fn loadout_line(loadout: &fortress_core::Loadout) -> String {
    use fortress_core::ItemKind;
    let mut parts = Vec::new();
    if let Some(w) = loadout.get(ItemKind::Weapon) {
        parts.push(format!("wields {}", w.label()));
    }
    if let Some(a) = loadout.get(ItemKind::Armor) {
        parts.push(format!("wears {}", a.label()));
    }
    if let Some(t) = loadout.get(ItemKind::Tool) {
        parts.push(format!("carries {}", t.label()));
    }
    if parts.is_empty() {
        "(unequipped)".to_string()
    } else {
        parts.join("\n")
    }
}

/// One bar per skill (tier 0..=7), for the inspect panel.
fn skill_bars(skills: &fortress_core::SkillSet) -> String {
    fortress_core::Skill::ALL
        .iter()
        .map(|s| {
            let tier = skills.tier(*s);
            let idx = (tier.index() as usize).min(7);
            let bar = format!("{}{}", "█".repeat(idx), "░".repeat(7 - idx));
            format!("{:<9} {} {}", s.name(), bar, tier.name())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Building inspect: current tier + effect blurb + next-tier cost preview.
fn building_inspect(game: &Game, u: Upgrade) -> String {
    let f = &game.0.fortress;
    let level = f.building_level(u);
    let mut s = if level == 0 {
        format!("{}\n\n(not yet built)", upgrade_blurb(u))
    } else if u == Upgrade::Housing {
        format!("{}\n\nplots: {}/{}", upgrade_blurb(u), f.housing_units(), fortress_core::HOUSING_PLOTS)
    } else {
        format!("{}\n\nstanding: tier {}", upgrade_blurb(u), fortress_core::level_numeral(level))
    };
    match f.next_build_level(u) {
        Some(next) => s.push_str(&format!(
            "\n→ next: tier {} (costs {})",
            fortress_core::level_numeral(next),
            u.build_cost(next).describe_cost()
        )),
        None => s.push_str("\n→ at its height"),
    }
    s
}

/// A few lines on the armory: counts by kind, the focus the forge is set to,
/// and a roll-call of the named artifacts the hold keeps.
fn armory_summary(gs: &fortress_core::GameState) -> String {
    use fortress_core::ItemKind;
    if gs.items.count() == 0 {
        return "  (empty)".to_string();
    }
    let mut lines: Vec<String> = ItemKind::ALL
        .iter()
        .map(|k| format!("  spare {}s: {}", k.name(), gs.items.count_kind(*k)))
        .collect();
    lines.push(format!("  forge focus: {}", gs.fortress.craft_focus.name()));
    let artifacts: Vec<String> =
        gs.items.items.iter().filter(|i| i.artifact).map(|i| i.label()).collect();
    if !artifacts.is_empty() {
        lines.push(format!("  artifacts: {}", artifacts.join(", ")));
    }
    lines.join("\n")
}

fn update_inspect(
    game: Res<Game>,
    selected: Res<Selected>,
    mut query: Query<&mut Text, With<InspectText>>,
) {
    let Ok(mut t) = query.single_mut() else { return };
    **t = match &selected.0 {
        None => {
            let gs = &game.0;
            let commander = gs
                .player
                .as_ref()
                .map(|p| format!("{} the {}  (hp {} · mo {})", p.name, p.class.name(), p.health, p.morale))
                .unwrap_or_default();
            let stores: Vec<String> = [
                fortress_core::ResourceKind::Valuables,
                fortress_core::ResourceKind::Gear,
                fortress_core::ResourceKind::Tools,
                fortress_core::ResourceKind::Ore,
                fortress_core::ResourceKind::Residue,
            ]
            .iter()
            .map(|k| format!("{}: {}", k.name(), gs.resources.band(*k).name()))
            .collect();
            let armory = armory_summary(gs);
            let renown = match gs.reputation {
                0..=19 => "unknown",
                20..=39 => "local",
                40..=59 => "known",
                60..=79 => "famed",
                _ => "legendary",
            };
            let heroes = if gs.adventurers.is_empty() {
                "none yet".to_string()
            } else {
                gs.adventurers
                    .iter()
                    .map(|a| {
                        format!(
                            "{} the {} ({} {})",
                            a.name,
                            a.class.name(),
                            a.perk_tier().name(),
                            a.class.home_skill().practitioner()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            format!(
                "{}\n\n{}\nRenown: {}\n\nArmory:\n{}\n\nHeroes:\n{}\n\nClick an inhabitant or building\nto inspect it.",
                commander,
                stores.join("\n"),
                renown,
                armory,
                heroes,
            )
        }
        Some(Selection::Keep) => format!(
            "The Keep\nSeat of {}.\nUpgrades built: {}",
            game.0.fortress.name,
            if game.0.fortress.buildings.is_empty() {
                "none".to_string()
            } else {
                game.0
                    .fortress
                    .buildings
                    .iter()
                    .map(|b| {
                        format!("{} {}", b.kind.name(), fortress_core::level_numeral(b.level))
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
        Some(Selection::Gate) => "The Gate\nAll trouble arrives here first.".to_string(),
        Some(Selection::Building(u)) => building_inspect(&game, *u),
        Some(Selection::Commander) => match &game.0.player {
            Some(p) => format!(
                "{} the {}\nCommander of the hold\nHealth {}  Morale {}\nMight {}  Wit {}  Heart {}\n\n{}\n\n{}",
                p.name,
                p.class.name(),
                p.health,
                p.morale,
                p.stats.might,
                p.stats.wit,
                p.stats.heart,
                loadout_line(&p.loadout),
                skill_bars(&p.skills),
            ),
            None => String::new(),
        },
        Some(Selection::Hero(name)) => {
            match game.0.adventurers.iter().find(|a| &a.name == name) {
                Some(a) => format!(
                    "{} the {}\nWandering hero\nPerk: {} ({} {})\n{}\n\n{}",
                    a.name,
                    a.class.name(),
                    a.class.perk_name(),
                    a.perk_tier().name(),
                    a.class.home_skill().practitioner(),
                    loadout_line(&a.loadout),
                    skill_bars(&a.skills),
                ),
                None => String::new(),
            }
        }
        Some(Selection::Inhabitant(name)) => {
            match game.0.inhabitants.inhabitants.iter().find(|i| &i.name == name) {
                Some(i) => {
                    // hidden traits (e.g. infiltrators) are never shown
                    let shown: Vec<&str> =
                        i.traits.iter().filter(|t| !t.is_hidden()).map(|t| t.name()).collect();
                    let traits = if shown.is_empty() { "—".to_string() } else { shown.join(", ") };
                    let flavor = match i.role {
                        Role::Guard => "Keeps watch through the long nights.",
                        Role::Farmer => "Coaxes life from stubborn soil.",
                        Role::Blacksmith => "The ring of the hammer is constant.",
                        Role::Healer => "Mends what the world breaks.",
                        Role::Miner => "Hews stone and ore from the deep seam.",
                        Role::Peasant => "Willing hands, waiting for a trade.",
                    };
                    format!(
                        "{}\n{}\nHealth {}  Morale {}\nTraits: {}\n{}\n\n{}\n\n{}\n\nAssign: [1]guard [2]farmer [3]smith\n[4]healer [5]miner [6]peasant",
                        i.name,
                        i.role.name(),
                        i.health,
                        i.morale,
                        traits,
                        loadout_line(&i.loadout),
                        skill_bars(&i.skills),
                        flavor
                    )
                }
                None => String::new(),
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Clock display + speed controls
// ---------------------------------------------------------------------------

fn update_clock_text(clock: Res<GameClock>, mut query: Query<&mut Text, With<ClockText>>) {
    let Ok(mut t) = query.single_mut() else { return };
    let phase = match clock.phase() {
        DayPhase::Dawn => "dawn",
        DayPhase::Day => "day",
        DayPhase::Dusk => "dusk",
        DayPhase::Night => "night",
    };
    let speed = match clock.speed {
        ClockSpeed::Paused => " [PAUSED]",
        ClockSpeed::Normal => "",
        ClockSpeed::Fast => " [x3]",
    };
    **t = format!("{} ({phase}){speed}  ", clock.readout());
}

fn build_hud_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<BuildHudButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        next_state.set(AppState::BuildMenu);
    }
}

fn region_hud_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<RegionHudButton>)>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        next_state.set(AppState::RegionView);
    }
}

fn auto_hud_button(
    interactions: Query<&Interaction, (Changed<Interaction>, With<AutoHudButton>)>,
    mut auto: ResMut<AutoMode>,
    mut log: ResMut<GameLog>,
) {
    if interactions.iter().any(|i| *i == Interaction::Pressed) {
        auto.0 = !auto.0;
        log.push(if auto.0 {
            "Auto-mode ON — the fortress runs itself.".to_string()
        } else {
            "Auto-mode OFF — you have the reins again.".to_string()
        });
    }
}

fn speed_buttons(
    interactions: Query<(&Interaction, &SpeedButton), Changed<Interaction>>,
    mut clock: ResMut<GameClock>,
) {
    for (interaction, button) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match button {
            SpeedButton::Pause => clock.speed = ClockSpeed::Paused,
            SpeedButton::Normal => clock.speed = ClockSpeed::Normal,
            SpeedButton::Fast => clock.speed = ClockSpeed::Fast,
            SpeedButton::SkipToDawn => clock.skip_to_dawn(),
        }
    }
}
