use serde::{Deserialize, Serialize};

use crate::inhabitants::Role;
use crate::resources::ResourceDelta;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Upgrade {
    Watchtower,
    Farm,
    Infirmary,
    Blacksmith,
    Granary,
    Barracks,
    Housing,
    Tavern,
    Workshop,
    Lumberyard,
    Shrine,
    TrainingYard,
    Mine,
    Graveyard,
    WizardTower,
}

/// Buildings rise once and are then raised through tiers (I → II → III).
/// Housing is the exception: plain roofs, built plot by plot, never tiered.
pub const MAX_BUILDING_LEVEL: u8 = 3;
pub const HOUSING_PLOTS: usize = 4;

impl Upgrade {
    pub const ALL: [Upgrade; 15] = [
        Upgrade::Watchtower,
        Upgrade::Farm,
        Upgrade::Infirmary,
        Upgrade::Blacksmith,
        Upgrade::Granary,
        Upgrade::Barracks,
        Upgrade::Housing,
        Upgrade::Tavern,
        Upgrade::Workshop,
        Upgrade::Lumberyard,
        Upgrade::Shrine,
        Upgrade::TrainingYard,
        Upgrade::Mine,
        Upgrade::Graveyard,
        Upgrade::WizardTower,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Upgrade::Watchtower => "Watchtower",
            Upgrade::Farm => "Farm",
            Upgrade::Infirmary => "Infirmary",
            Upgrade::Blacksmith => "Blacksmith",
            Upgrade::Granary => "Granary",
            Upgrade::Barracks => "Barracks",
            Upgrade::Housing => "Housing",
            Upgrade::Tavern => "Tavern",
            Upgrade::Workshop => "Workshop",
            Upgrade::Lumberyard => "Lumberyard",
            Upgrade::Shrine => "Shrine",
            Upgrade::TrainingYard => "Training Yard",
            Upgrade::Mine => "Mine",
            Upgrade::Graveyard => "Graveyard",
            Upgrade::WizardTower => "Wizard Tower",
        }
    }

    /// Materials to raise this building at `level` (1 = first build).
    /// Tier costs grow ~×1.6 per level; housing always pays the base price.
    pub fn build_cost(&self, level: u8) -> ResourceDelta {
        let (food, wood, stone) = match self {
            Upgrade::Watchtower => (0, 10, 8),
            Upgrade::Farm => (0, 15, 0),
            Upgrade::Infirmary => (0, 12, 5),
            Upgrade::Blacksmith => (0, 10, 8),
            Upgrade::Granary => (0, 8, 12),
            Upgrade::Barracks => (0, 12, 12),
            Upgrade::Housing => (6, 14, 6),
            Upgrade::Tavern => (4, 12, 4),
            Upgrade::Workshop => (0, 12, 6),
            Upgrade::Lumberyard => (0, 6, 8),
            Upgrade::Shrine => (0, 6, 12),
            Upgrade::TrainingYard => (0, 10, 6),
            Upgrade::Mine => (0, 14, 6),
            Upgrade::Graveyard => (0, 4, 10),
            Upgrade::WizardTower => (0, 12, 16),
        };
        let (num, den): (i64, i64) = match level {
            0 | 1 => (1, 1),
            2 => (8, 5),
            _ => (64, 25),
        };
        let scale = |v: i64| if v == 0 { 0 } else { (v * num + den - 1) / den };
        ResourceDelta { food: scale(food), wood: scale(wood), stone: scale(stone), ..Default::default() }
    }

    /// Specialist who must live here before the building can go up.
    pub fn required_role(&self) -> Option<Role> {
        match self {
            Upgrade::Farm | Upgrade::Lumberyard => Some(Role::Farmer),
            Upgrade::Infirmary | Upgrade::Shrine => Some(Role::Healer),
            Upgrade::Blacksmith => Some(Role::Blacksmith),
            Upgrade::Barracks | Upgrade::TrainingYard => Some(Role::Guard),
            Upgrade::Watchtower
            | Upgrade::Granary
            | Upgrade::Housing
            | Upgrade::Tavern
            | Upgrade::Workshop
            | Upgrade::Mine
            | Upgrade::Graveyard
            | Upgrade::WizardTower => None,
        }
    }

    /// A one-line summary of what this building does at a given level — used by
    /// the build menu and inspect panel. Level 0 describes the first tier.
    pub fn effect_summary(&self, level: u8) -> String {
        let lvl = level.max(1);
        match self {
            Upgrade::Watchtower => format!("+{} defense", [5, 8, 12][(lvl - 1).min(2) as usize]),
            Upgrade::Farm => format!("+{} food/day", [3, 5, 7][(lvl - 1).min(2) as usize]),
            Upgrade::Infirmary => "heals the wounded; disasters hit softer".to_string(),
            Upgrade::Blacksmith => "forges gear from smithing skill".to_string(),
            Upgrade::Granary => format!("food cap {}", [60, 90, 130][(lvl - 1).min(2) as usize]),
            Upgrade::Barracks => "bunks for many guards; +defense".to_string(),
            Upgrade::Housing => "+5 beds, +5 max population".to_string(),
            Upgrade::Tavern => format!("+{} morale/day", lvl.min(3)),
            Upgrade::Workshop => "tools; trains crafting at II+".to_string(),
            Upgrade::Lumberyard => format!("+{} wood/day", [2, 3, 5][(lvl - 1).min(2) as usize]),
            Upgrade::Shrine => "softens demon dread".to_string(),
            Upgrade::TrainingYard => "drills the guard's combat".to_string(),
            Upgrade::Mine => format!("+{} stone/day", [3, 5, 8][(lvl - 1).min(2) as usize]),
            Upgrade::Graveyard => "honors the dead; eases grief".to_string(),
            Upgrade::WizardTower => "a seat for magic; enchanting".to_string(),
        }
    }
}

/// "I", "II", "III" — the steward chisels tiers above the door.
pub fn level_numeral(level: u8) -> &'static str {
    match level {
        0 | 1 => "I",
        2 => "II",
        _ => "III",
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Building {
    pub kind: Upgrade,
    pub level: u8,
}

/// What happened when ground was broken (or wasn't).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildOutcome {
    Built,
    Upgraded(u8),
    AtMax,
    NoPlots,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Fortress {
    pub name: String,
    pub day: u32,
    pub morale: i32,
    pub defense: i32,
    pub max_population: u32,
    pub buildings: Vec<Building>,
}

impl Fortress {
    pub fn new(name: &str) -> Fortress {
        Fortress {
            name: name.to_string(),
            day: 1,
            morale: 50,
            defense: 10,
            max_population: 20,
            buildings: Vec::new(),
        }
    }

    pub fn advance_day(&mut self) {
        self.day += 1;
    }

    pub fn apply_morale_delta(&mut self, amount: i32) {
        self.morale = (self.morale + amount).clamp(0, 100);
    }

    pub fn apply_defense_delta(&mut self, amount: i32) {
        self.defense = (self.defense + amount).max(0);
    }

    /// Build at level 1, or raise an existing building one tier.
    pub fn add_building(&mut self, kind: Upgrade) -> BuildOutcome {
        if kind == Upgrade::Housing {
            if self.housing_units() >= HOUSING_PLOTS {
                return BuildOutcome::NoPlots;
            }
            self.buildings.push(Building { kind, level: 1 });
            return BuildOutcome::Built;
        }
        match self.buildings.iter_mut().find(|b| b.kind == kind) {
            Some(b) if b.level >= MAX_BUILDING_LEVEL => BuildOutcome::AtMax,
            Some(b) => {
                b.level += 1;
                BuildOutcome::Upgraded(b.level)
            }
            None => {
                self.buildings.push(Building { kind, level: 1 });
                BuildOutcome::Built
            }
        }
    }

    pub fn has_upgrade(&self, kind: Upgrade) -> bool {
        self.buildings.iter().any(|b| b.kind == kind)
    }

    /// Current tier (0 = not built). Housing is always tier 1.
    pub fn building_level(&self, kind: Upgrade) -> u8 {
        self.buildings.iter().filter(|b| b.kind == kind).map(|b| b.level).max().unwrap_or(0)
    }

    pub fn housing_units(&self) -> usize {
        self.buildings.iter().filter(|b| b.kind == Upgrade::Housing).count()
    }

    /// The level a fresh build/upgrade would reach, or None when nothing
    /// more can rise (tier III, or all housing plots taken).
    pub fn next_build_level(&self, kind: Upgrade) -> Option<u8> {
        if kind == Upgrade::Housing {
            return (self.housing_units() < HOUSING_PLOTS).then_some(1);
        }
        match self.building_level(kind) {
            0 => Some(1),
            l if l < MAX_BUILDING_LEVEL => Some(l + 1),
            _ => None,
        }
    }

    /// Beds available: the Keep sleeps 6; the Barracks bunks grow with its
    /// tier; every housing plot shelters 5. Overflow sleeps rough.
    pub fn sleeping_capacity(&self) -> u32 {
        let mut beds = 6; // the Keep's own beds
        // The Barracks is built for numbers — plain bunks, but it sleeps a crowd.
        beds += match self.building_level(Upgrade::Barracks) {
            0 => 0,
            1 => 10,
            2 => 16,
            _ => 24,
        };
        beds += self.housing_units() as u32 * 6;
        // Every other standing building keeps a few cots for its workers.
        let workshops = self
            .buildings
            .iter()
            .filter(|b| b.kind != Upgrade::Barracks && b.kind != Upgrade::Housing)
            .count() as u32;
        beds += workshops * 2;
        beds
    }

    /// How many of the dead the Graveyard can honor — eases the morale of loss.
    pub fn graveyard_level(&self) -> u8 {
        self.building_level(Upgrade::Graveyard)
    }

    pub fn is_defeated(&self) -> bool {
        self.morale == 0
    }
}
