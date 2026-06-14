//! Character creation: name, class, stat allocation.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use rand::Rng;

use fortress_core::player::{CLASS_BONUS, FREE_POINTS, STAT_BASE, STAT_CAP};
use fortress_core::{ClassKind, GameState, PlayerCharacter, StatKind, Stats, Upgrade};

use crate::bridge::{Game, GameLog};
use crate::ui::{button_node, text, tint_buttons, ACCENT, BTN_BG, PANEL_BG, TEXT_DIM};
use crate::AppState;

pub struct CharCreatePlugin;

impl Plugin for CharCreatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Draft>()
            .add_systems(OnEnter(AppState::CharCreation), (reset_draft, spawn_screen))
            .add_systems(
                Update,
                (
                    name_input,
                    class_buttons,
                    stat_buttons,
                    founding_buttons,
                    begin_button,
                    refresh_labels,
                    tint_buttons,
                )
                    .run_if(in_state(AppState::CharCreation)),
            );
    }
}

/// Founding charter: one building the realm grants the new commander for free.
const FOUNDING_CHOICES: [Upgrade; 4] =
    [Upgrade::Watchtower, Upgrade::Barracks, Upgrade::Housing, Upgrade::WizardTower];

#[derive(Resource)]
struct Draft {
    name: String,
    fortress_name: String,
    editing_fortress: bool,
    class: ClassKind,
    free: u8,
    alloc: Stats, // allocated points only (base/class added at confirm)
    founding: Upgrade,
}

impl Default for Draft {
    fn default() -> Draft {
        Draft {
            name: String::new(),
            fortress_name: String::new(),
            editing_fortress: false,
            class: ClassKind::Warlord,
            free: FREE_POINTS,
            alloc: Stats { might: 0, wit: 0, heart: 0 },
            founding: Upgrade::Watchtower,
        }
    }
}

impl Draft {
    fn final_stats(&self) -> Stats {
        let mut s = Stats {
            might: STAT_BASE + self.alloc.might,
            wit: STAT_BASE + self.alloc.wit,
            heart: STAT_BASE + self.alloc.heart,
        };
        *s.get_mut(self.class.bonus_stat()) += CLASS_BONUS;
        s
    }
}

fn reset_draft(mut draft: ResMut<Draft>) {
    *draft = Draft::default();
}

#[derive(Component, Clone, Copy)]
struct ClassButton(ClassKind);

#[derive(Component, Clone, Copy)]
struct StatButton(StatKind, i8);

#[derive(Component, Clone, Copy)]
struct FoundingButton(Upgrade);

#[derive(Component)]
struct FoundingLabel;

#[derive(Component)]
struct BeginButton;

#[derive(Component)]
struct NameLabel;

#[derive(Component)]
struct FortressLabel;

#[derive(Component)]
struct StatsLabel;

#[derive(Component)]
struct ClassLabel;

fn spawn_screen(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(AppState::CharCreation),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(8.0),
                ..Default::default()
            },
            BackgroundColor(Color::srgb(0.02, 0.02, 0.04)),
        ))
        .with_children(|parent| {
            parent.spawn(text("FORGE YOUR COMMANDER", 26.0, ACCENT));
            parent.spawn(text(
                "Type your name. TAB switches to the fortress name. Click a class, allocate stats, then begin.",
                14.0,
                TEXT_DIM,
            ));

            parent.spawn((NameLabel, text("", 20.0, Color::WHITE)));
            parent.spawn((FortressLabel, text("", 20.0, Color::WHITE)));

            // class row
            parent
                .spawn(Node {
                    column_gap: Val::Px(8.0),
                    margin: UiRect::top(Val::Px(12.0)),
                    ..Default::default()
                })
                .with_children(|row| {
                    for class in ClassKind::ALL {
                        row.spawn((
                            ClassButton(class),
                            Button,
                            Node {
                                width: Val::Px(240.0),
                                padding: UiRect::all(Val::Px(10.0)),
                                flex_direction: FlexDirection::Column,
                                align_items: AlignItems::Center,
                                ..Default::default()
                            },
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text(class.name(), 18.0, Color::WHITE));
                            b.spawn(text(class.description(), 13.0, TEXT_DIM));
                        });
                    }
                });

            parent.spawn((ClassLabel, text("", 15.0, ACCENT)));

            // founding charter: one free building to start with
            parent.spawn(text(
                "The realm's charter grants one building, already standing:",
                14.0,
                TEXT_DIM,
            ));
            parent
                .spawn(Node {
                    column_gap: Val::Px(8.0),
                    ..Default::default()
                })
                .with_children(|row| {
                    for upgrade in FOUNDING_CHOICES {
                        row.spawn((
                            FoundingButton(upgrade),
                            Button,
                            button_node(),
                            BackgroundColor(BTN_BG),
                        ))
                        .with_children(|b| {
                            b.spawn(text(upgrade.name(), 15.0, Color::WHITE));
                        });
                    }
                });
            parent.spawn((FoundingLabel, text("", 15.0, ACCENT)));

            // stat allocation
            parent.spawn((StatsLabel, text("", 18.0, Color::WHITE)));
            parent
                .spawn(Node {
                    column_gap: Val::Px(16.0),
                    ..Default::default()
                })
                .with_children(|row| {
                    for stat in StatKind::ALL {
                        row.spawn(Node {
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(4.0),
                            ..Default::default()
                        })
                        .with_children(|cell| {
                            cell.spawn(text(stat.name(), 16.0, TEXT_DIM));
                            cell.spawn((
                                StatButton(stat, -1),
                                Button,
                                button_node(),
                                BackgroundColor(BTN_BG),
                            ))
                            .with_children(|b| {
                                b.spawn(text("-", 18.0, Color::WHITE));
                            });
                            cell.spawn((
                                StatButton(stat, 1),
                                Button,
                                button_node(),
                                BackgroundColor(BTN_BG),
                            ))
                            .with_children(|b| {
                                b.spawn(text("+", 18.0, Color::WHITE));
                            });
                        });
                    }
                });

            parent
                .spawn((
                    BeginButton,
                    Button,
                    Node {
                        margin: UiRect::top(Val::Px(16.0)),
                        padding: UiRect::axes(Val::Px(28.0), Val::Px(12.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..Default::default()
                    },
                    BackgroundColor(BTN_BG),
                ))
                .with_children(|b| {
                    b.spawn(text("Begin the Watch »", 20.0, ACCENT));
                });

            parent.spawn((
                text("", 12.0, TEXT_DIM),
                BackgroundColor(PANEL_BG),
            ));
        });
}

fn name_input(mut events: MessageReader<KeyboardInput>, mut draft: ResMut<Draft>) {
    for ev in events.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        let target = if draft.editing_fortress {
            &mut draft.fortress_name
        } else {
            &mut draft.name
        };
        match &ev.logical_key {
            Key::Character(s) => {
                if target.chars().count() < 16 {
                    for c in s.chars().filter(|c| c.is_alphanumeric() || *c == '\'' || *c == '-') {
                        target.push(c);
                    }
                }
            }
            Key::Space => {
                if !target.is_empty() && target.chars().count() < 16 {
                    target.push(' ');
                }
            }
            Key::Backspace => {
                target.pop();
            }
            Key::Tab => {
                draft.editing_fortress = !draft.editing_fortress;
            }
            _ => {}
        }
    }
}

fn founding_buttons(
    interactions: Query<(&Interaction, &FoundingButton), Changed<Interaction>>,
    mut draft: ResMut<Draft>,
) {
    for (interaction, button) in interactions.iter() {
        if *interaction == Interaction::Pressed {
            draft.founding = button.0;
        }
    }
}

fn class_buttons(
    interactions: Query<(&Interaction, &ClassButton), Changed<Interaction>>,
    mut draft: ResMut<Draft>,
) {
    for (interaction, button) in interactions.iter() {
        if *interaction == Interaction::Pressed {
            draft.class = button.0;
        }
    }
}

fn stat_buttons(
    interactions: Query<(&Interaction, &StatButton), Changed<Interaction>>,
    mut draft: ResMut<Draft>,
) {
    for (interaction, button) in interactions.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let StatButton(stat, delta) = *button;
        let alloc = draft.alloc.get(stat);
        if delta > 0 && draft.free > 0 {
            let would_be = STAT_BASE
                + alloc
                + 1
                + if draft.class.bonus_stat() == stat { CLASS_BONUS } else { 0 };
            if would_be <= STAT_CAP {
                *draft.alloc.get_mut(stat) += 1;
                draft.free -= 1;
            }
        } else if delta < 0 && alloc > 0 {
            *draft.alloc.get_mut(stat) -= 1;
            draft.free += 1;
        }
    }
}

fn refresh_labels(
    draft: Res<Draft>,
    mut labels: ParamSet<(
        Query<&mut Text, With<NameLabel>>,
        Query<&mut Text, With<FortressLabel>>,
        Query<&mut Text, With<StatsLabel>>,
        Query<&mut Text, With<ClassLabel>>,
        Query<&mut Text, With<FoundingLabel>>,
    )>,
) {
    let cursor_a = if draft.editing_fortress { " " } else { "_" };
    let cursor_b = if draft.editing_fortress { "_" } else { " " };
    if let Ok(mut t) = labels.p0().single_mut() {
        **t = format!("Commander: {}{}", draft.name, cursor_a);
    }
    if let Ok(mut t) = labels.p1().single_mut() {
        **t = format!("Fortress:  {}{}", draft.fortress_name, cursor_b);
    }
    if let Ok(mut t) = labels.p2().single_mut() {
        let s = draft.final_stats();
        **t = format!(
            "Might {}   Wit {}   Heart {}    (points left: {})",
            s.might, s.wit, s.heart, draft.free
        );
    }
    if let Ok(mut t) = labels.p3().single_mut() {
        **t = format!("Chosen path: {}", draft.class.name());
    }
    if let Ok(mut t) = labels.p4().single_mut() {
        **t = format!("Founding charter: {}", draft.founding.name());
    }
}

fn begin_button(
    mut commands: Commands,
    interactions: Query<&Interaction, (Changed<Interaction>, With<BeginButton>)>,
    draft: Res<Draft>,
    mut log: ResMut<GameLog>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let clicked = interactions.iter().any(|i| *i == Interaction::Pressed);
    if !clicked {
        return;
    }

    let name = if draft.name.trim().is_empty() { "Wanderer" } else { draft.name.trim() };
    let fortress = if draft.fortress_name.trim().is_empty() {
        "Greyhold"
    } else {
        draft.fortress_name.trim()
    };

    let player = PlayerCharacter::new(name, draft.class, draft.final_stats());
    let seed: u64 = rand::rng().random();
    let mut gs = GameState::new_game(seed, fortress, player);
    gs.build_upgrade(draft.founding);

    log.0.clear();
    log.push(format!(
        "{} the {} takes command of {}.",
        name,
        draft.class.name(),
        fortress
    ));
    log.push(format!(
        "By the realm's charter, a {} already stands.",
        draft.founding.name()
    ));
    commands.insert_resource(Game(gs));
    commands.insert_resource(crate::clock::GameClock::default());
    next_state.set(AppState::FortressView);
}
