//! Fortress map layout and terminal rendering.

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_ascii_terminal::*;

use fortress_core::{Building, Role, Upgrade};

use crate::actors::{Actor, Glyph, GridPos};
use crate::bridge::{Game, Selected, Selection};
use crate::clock::{DayPhase, GameClock};
use crate::picking::Hovered;
use crate::AppState;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapLayout>()
            .add_systems(
                Update,
                (rebuild_layout, redraw_terminal)
                    .chain()
                    .run_if(in_state(AppState::FortressView).or(in_state(AppState::EventModal))),
            );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileKind {
    Wall,
    Gate,
    Keep,
    Building(Upgrade),
}

#[derive(Resource, Default)]
pub struct MapLayout {
    pub tiles: HashMap<IVec2, TileKind>,
    pub walkable: HashSet<IVec2>,
    pub anchors: HashMap<AnchorKind, Vec<IVec2>>,
    pub built: Vec<Building>,
    /// Bed spots in front of each built housing plot.
    pub housing_beds: Vec<IVec2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnchorKind {
    Gate,
    Keep,
    Farm,
    Walls,
    Building(Upgrade),
    Courtyard,
}

const WALL_MIN: IVec2 = IVec2::new(1, 1);
const WALL_MAX: IVec2 = IVec2::new(38, 23);

/// Plots along the gate road where housing goes up, one per unit built.
pub const HOUSING_PLOT_RECTS: [(IVec2, IVec2); 4] = [
    (IVec2::new(11, 2), IVec2::new(13, 3)),
    (IVec2::new(15, 2), IVec2::new(17, 3)),
    (IVec2::new(23, 2), IVec2::new(25, 3)),
    (IVec2::new(27, 2), IVec2::new(29, 3)),
];

fn building_rect(u: Upgrade) -> (IVec2, IVec2) {
    match u {
        Upgrade::Watchtower => (IVec2::new(2, 21), IVec2::new(3, 22)),
        Upgrade::Farm => (IVec2::new(4, 12), IVec2::new(8, 14)),
        Upgrade::Granary => (IVec2::new(4, 5), IVec2::new(6, 7)),
        Upgrade::Blacksmith => (IVec2::new(28, 5), IVec2::new(30, 7)),
        Upgrade::Infirmary => (IVec2::new(28, 12), IVec2::new(30, 14)),
        Upgrade::Barracks => (IVec2::new(32, 17), IVec2::new(34, 19)),
        Upgrade::Tavern => (IVec2::new(11, 5), IVec2::new(14, 7)),
        Upgrade::Workshop => (IVec2::new(11, 12), IVec2::new(14, 14)),
        Upgrade::Lumberyard => (IVec2::new(4, 17), IVec2::new(6, 19)),
        Upgrade::Shrine => (IVec2::new(33, 9), IVec2::new(35, 11)),
        Upgrade::TrainingYard => (IVec2::new(33, 4), IVec2::new(35, 6)),
        Upgrade::AdventurersGuild => (IVec2::new(25, 20), IVec2::new(28, 22)),
        Upgrade::Housing => HOUSING_PLOT_RECTS[0], // plot picked per instance
    }
}

pub fn building_glyph(u: Upgrade) -> (char, Color) {
    match u {
        Upgrade::Farm => ('"', Color::srgb(0.4, 0.8, 0.2)),
        Upgrade::Granary => ('G', Color::srgb(0.85, 0.7, 0.3)),
        Upgrade::Blacksmith => ('B', Color::srgb(0.9, 0.55, 0.2)),
        Upgrade::Infirmary => ('I', Color::srgb(0.9, 0.9, 0.95)),
        Upgrade::Barracks => ('K', Color::srgb(0.7, 0.3, 0.3)),
        Upgrade::Housing => ('H', Color::srgb(0.9, 0.7, 0.4)),
        Upgrade::AdventurersGuild => ('A', Color::srgb(0.7, 0.4, 0.9)),
        Upgrade::Watchtower => ('T', Color::srgb(0.8, 0.8, 0.6)),
        Upgrade::Tavern => ('V', Color::srgb(0.95, 0.65, 0.3)),
        Upgrade::Workshop => ('W', Color::srgb(0.7, 0.6, 0.4)),
        Upgrade::Lumberyard => ('L', Color::srgb(0.55, 0.7, 0.3)),
        Upgrade::Shrine => ('S', Color::srgb(0.75, 0.85, 1.0)),
        Upgrade::TrainingYard => ('Y', Color::srgb(0.85, 0.45, 0.45)),
    }
}

pub fn role_glyph(role: Role) -> (char, Color) {
    match role {
        Role::Guard => ('g', Color::srgb(0.95, 0.3, 0.3)),
        Role::Farmer => ('f', Color::srgb(0.3, 0.9, 0.3)),
        Role::Blacksmith => ('b', Color::srgb(0.95, 0.75, 0.2)),
        Role::Healer => ('h', Color::srgb(0.3, 0.9, 0.95)),
    }
}

fn rebuild_layout(game: Res<Game>, mut layout: ResMut<MapLayout>) {
    if layout.built == game.0.fortress.buildings && !layout.tiles.is_empty() {
        return;
    }
    let built = game.0.fortress.buildings.clone();
    let mut tiles = HashMap::new();
    let mut anchors: HashMap<AnchorKind, Vec<IVec2>> = HashMap::new();

    // perimeter wall with a 2-tile gate at bottom-center
    for x in WALL_MIN.x..=WALL_MAX.x {
        for y in [WALL_MIN.y, WALL_MAX.y] {
            tiles.insert(IVec2::new(x, y), TileKind::Wall);
        }
    }
    for y in WALL_MIN.y..=WALL_MAX.y {
        for x in [WALL_MIN.x, WALL_MAX.x] {
            tiles.insert(IVec2::new(x, y), TileKind::Wall);
        }
    }
    for x in [19, 20] {
        tiles.insert(IVec2::new(x, WALL_MIN.y), TileKind::Gate);
    }
    anchors.entry(AnchorKind::Gate).or_default().extend([IVec2::new(19, 2), IVec2::new(20, 2)]);
    anchors.entry(AnchorKind::Walls).or_default().extend([
        IVec2::new(3, 2),
        IVec2::new(36, 2),
        IVec2::new(3, 22),
        IVec2::new(36, 22),
    ]);

    // keep, top-center 6x4
    for x in 17..=22 {
        for y in 18..=21 {
            tiles.insert(IVec2::new(x, y), TileKind::Keep);
        }
    }
    anchors.entry(AnchorKind::Keep).or_default().push(IVec2::new(19, 17));

    // built buildings; housing fills its plots in build order
    let mut housing_beds = Vec::new();
    let mut housing_idx = 0;
    for b in &built {
        let (min, max) = if b.kind == Upgrade::Housing {
            let rect = HOUSING_PLOT_RECTS[housing_idx.min(HOUSING_PLOT_RECTS.len() - 1)];
            housing_idx += 1;
            rect
        } else {
            building_rect(b.kind)
        };
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                tiles.insert(IVec2::new(x, y), TileKind::Building(b.kind));
            }
        }
        // door side: south of the building, except housing whose plots back
        // onto the wall — their doors (and beds) face north
        let door_y = if b.kind == Upgrade::Housing { max.y + 1 } else { min.y - 1 };
        anchors
            .entry(AnchorKind::Building(b.kind))
            .or_default()
            .push(IVec2::new((min.x + max.x) / 2, door_y));
        if b.kind == Upgrade::Farm {
            anchors
                .entry(AnchorKind::Farm)
                .or_default()
                .push(IVec2::new((min.x + max.x) / 2, door_y));
        }
        if b.kind == Upgrade::Housing {
            for x in min.x..=max.x {
                housing_beds.push(IVec2::new(x, max.y + 1));
            }
        }
    }

    // walkable = inside walls, not wall/keep/building (farm patches ARE walkable)
    let mut walkable = HashSet::new();
    let mut courtyard = Vec::new();
    for x in (WALL_MIN.x + 1)..WALL_MAX.x {
        for y in (WALL_MIN.y + 1)..WALL_MAX.y {
            let p = IVec2::new(x, y);
            let blocked = matches!(
                tiles.get(&p),
                Some(TileKind::Wall) | Some(TileKind::Keep) | Some(TileKind::Building(_))
            ) && !matches!(tiles.get(&p), Some(TileKind::Building(Upgrade::Farm)));
            if !blocked {
                walkable.insert(p);
                if !tiles.contains_key(&p) {
                    courtyard.push(p);
                }
            }
        }
    }
    anchors.insert(AnchorKind::Courtyard, courtyard);

    layout.tiles = tiles;
    layout.walkable = walkable;
    layout.anchors = anchors;
    layout.built = built;
    layout.housing_beds = housing_beds;
}

/// Daylight grading: warm at dawn/dusk, cold and dim at night.
fn phase_tint(c: Color, phase: DayPhase) -> Color {
    let s = c.to_srgba();
    match phase {
        DayPhase::Day => c,
        DayPhase::Dawn => Color::srgb(s.red * 0.95, s.green * 0.85, s.blue * 0.75),
        DayPhase::Dusk => Color::srgb(s.red * 0.9, s.green * 0.7, s.blue * 0.6),
        DayPhase::Night => Color::srgb(s.red * 0.35, s.green * 0.4, s.blue * 0.55 + 0.05),
    }
}

fn redraw_terminal(
    mut terminals: Query<&mut Terminal>,
    layout: Res<MapLayout>,
    clock: Res<GameClock>,
    hovered: Res<Hovered>,
    selected: Res<Selected>,
    actors: Query<(&Actor, &GridPos, &Glyph)>,
) {
    let Ok(mut term) = terminals.single_mut() else {
        return;
    };
    term.clear();

    let phase = clock.phase();
    let ground = Color::srgb(0.18, 0.16, 0.12);
    for x in 0..crate::MAP_W as i32 {
        for y in 0..crate::MAP_H as i32 {
            let p = IVec2::new(x, y);
            let (ch, fg) = match layout.tiles.get(&p) {
                Some(TileKind::Wall) => ('#', Color::srgb(0.55, 0.55, 0.6)),
                Some(TileKind::Gate) => ('+', Color::srgb(0.7, 0.5, 0.25)),
                Some(TileKind::Keep) => ('█', Color::srgb(0.45, 0.45, 0.55)),
                Some(TileKind::Building(u)) => building_glyph(*u),
                None => {
                    if x > WALL_MIN.x && x < WALL_MAX.x && y > WALL_MIN.y && y < WALL_MAX.y {
                        ('.', ground)
                    } else {
                        (' ', Color::BLACK)
                    }
                }
            };
            term.put_char([x as usize, y as usize], ch).fg(phase_tint(fg, phase));
        }
    }

    for (_, pos, glyph) in actors.iter() {
        let ch = if phase == DayPhase::Night { 'z' } else { glyph.ch };
        term.put_char([pos.0.x as usize, pos.0.y as usize], ch)
            .fg(phase_tint(glyph.color, phase));
    }

    // hover + selection highlight via background tint
    let mut highlights: Vec<(IVec2, Color)> = Vec::new();
    if let Some(p) = hovered.0 {
        highlights.push((p, Color::srgb(0.25, 0.25, 0.35)));
    }
    if let Some(Selection::Inhabitant(name)) = &selected.0 {
        for (actor, pos, _) in actors.iter() {
            if &actor.name == name {
                highlights.push((pos.0, Color::srgb(0.2, 0.35, 0.2)));
            }
        }
    }
    for (p, bg) in highlights {
        if p.x >= 0 && p.y >= 0 && (p.x as usize) < crate::MAP_W && (p.y as usize) < crate::MAP_H {
            term.tile_mut([p.x as usize, p.y as usize]).bg(bg);
        }
    }
}
