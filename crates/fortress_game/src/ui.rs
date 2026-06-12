//! HUD (top bar), log panel, inspect panel, End Day button — plus small
//! UI helpers shared by the other screens.

use bevy::prelude::*;

use fortress_core::{Role, Upgrade};

use crate::bridge::{Game, GameLog, Selected, Selection};
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
                    update_log_text,
                    update_inspect,
                    speed_buttons,
                    build_hud_button,
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
struct LogText;

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

    // log panel, bottom — to the right of the roster column
    commands
        .spawn((
            DespawnOnExit(AppState::FortressView),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(8.0),
                left: Val::Px(246.0),
                width: Val::Px(440.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            BackgroundColor(Color::srgb(0.05, 0.05, 0.08)),
        ))
        .with_children(|parent| {
            parent.spawn((LogText, text("", 14.0, TEXT_DIM)));
        });

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

fn update_hud_text(game: Res<Game>, mut query: Query<&mut Text, With<HudText>>) {
    let Ok(mut t) = query.single_mut() else { return };
    let gs = &game.0;
    let stores: Vec<String> = [
        fortress_core::ResourceKind::Food,
        fortress_core::ResourceKind::Wood,
        fortress_core::ResourceKind::Stone,
    ]
    .iter()
    .map(|k| format!("{} {}", k.name(), gs.resources.band(*k).name()))
    .collect();
    **t = format!(
        "Day {} — {}  |  Morale {}  Def {}  |  Pop {}/{}  |  {}",
        gs.fortress.day,
        gs.fortress.name,
        gs.fortress.morale,
        gs.fortress.defense,
        gs.inhabitants.count_alive(),
        gs.fortress.max_population,
        stores.join(" · "),
    );
}

fn update_log_text(log: Res<GameLog>, mut query: Query<&mut Text, With<LogText>>) {
    let Ok(mut t) = query.single_mut() else { return };
    let lines: Vec<&str> = log.0.iter().rev().take(6).map(|s| s.as_str()).collect();
    **t = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
}

fn upgrade_blurb(u: Upgrade) -> &'static str {
    match u {
        Upgrade::Watchtower => "Watchtower\n+5 defense. Eyes on the horizon —\nscouts bring warning of attacks.",
        Upgrade::Farm => "Farm\n+3 food every day.",
        Upgrade::Infirmary => "Infirmary\nDisasters strike half as hard.\nHealers recover spirit daily.",
        Upgrade::Blacksmith => "Blacksmith\nYour people take less harm in battle.",
        Upgrade::Granary => "Granary\nSafer stores. Trade caravans now\nseek out the fortress.",
        Upgrade::Barracks => "Barracks\n+5 max population, +2 defense.",
        Upgrade::Inn => "Inn\n+5 max population, +5 beds.\nLaughter lifts morale every day.",
        Upgrade::AdventurersGuild => {
            "Adventurers' Guild\nWord spreads. Heroes will seek\nout a fortress of renown."
        }
    }
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
                .map(|p| format!("{} the {}  Lv.{}", p.name, p.class.name(), p.level))
                .unwrap_or_default();
            let abilities = gs.player.as_ref().map(|p| {
                if p.abilities.is_empty() {
                    "none yet — survive to earn them".to_string()
                } else {
                    p.abilities.iter().map(|a| a.name()).collect::<Vec<_>>().join(", ")
                }
            }).unwrap_or_default();
            let stores: Vec<String> = [
                fortress_core::ResourceKind::Valuables,
                fortress_core::ResourceKind::Gear,
                fortress_core::ResourceKind::Tools,
            ]
            .iter()
            .map(|k| format!("{}: {}", k.name(), gs.resources.band(*k).name()))
            .collect();
            format!(
                "{}\nAbilities: {}\n\n{}\n\nClick an inhabitant or building\nto inspect it.",
                commander,
                abilities,
                stores.join("\n"),
            )
        }
        Some(Selection::Keep) => format!(
            "The Keep\nSeat of {}.\nUpgrades built: {}",
            game.0.fortress.name,
            if game.0.fortress.upgrades.is_empty() {
                "none".to_string()
            } else {
                game.0
                    .fortress
                    .upgrades
                    .iter()
                    .map(|u| u.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ),
        Some(Selection::Gate) => "The Gate\nAll trouble arrives here first.".to_string(),
        Some(Selection::Building(u)) => upgrade_blurb(*u).to_string(),
        Some(Selection::Inhabitant(name)) => {
            match game.0.inhabitants.inhabitants.iter().find(|i| &i.name == name) {
                Some(i) => {
                    let traits = if i.traits.is_empty() {
                        "—".to_string()
                    } else {
                        i.traits.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")
                    };
                    let skills: Vec<String> = fortress_core::Skill::ALL
                        .iter()
                        .filter(|s| i.skills.tier(**s) > fortress_core::SkillTier::Dabbling)
                        .map(|s| format!("{} {}", i.skills.tier(*s).name(), s.name()))
                        .collect();
                    let skills = if skills.is_empty() {
                        "nothing of note yet".to_string()
                    } else {
                        skills.join(", ")
                    };
                    let flavor = match i.role {
                        Role::Guard => "Keeps watch through the long nights.",
                        Role::Farmer => "Coaxes life from stubborn soil.",
                        Role::Blacksmith => "The ring of the hammer is constant.",
                        Role::Healer => "Mends what the world breaks.",
                    };
                    format!(
                        "{}\n{}\nHealth {}  Morale {}\nTraits: {}\nSkills: {}\n\n{}",
                        i.name,
                        i.role.name(),
                        i.health,
                        i.morale,
                        traits,
                        skills,
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
