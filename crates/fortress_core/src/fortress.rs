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
    Inn,
    AdventurersGuild,
}

impl Upgrade {
    pub const ALL: [Upgrade; 8] = [
        Upgrade::Watchtower,
        Upgrade::Farm,
        Upgrade::Infirmary,
        Upgrade::Blacksmith,
        Upgrade::Granary,
        Upgrade::Barracks,
        Upgrade::Inn,
        Upgrade::AdventurersGuild,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Upgrade::Watchtower => "Watchtower",
            Upgrade::Farm => "Farm",
            Upgrade::Infirmary => "Infirmary",
            Upgrade::Blacksmith => "Blacksmith",
            Upgrade::Granary => "Granary",
            Upgrade::Barracks => "Barracks",
            Upgrade::Inn => "Inn",
            Upgrade::AdventurersGuild => "Adventurers' Guild",
        }
    }

    /// Materials to raise this building via the build menu. Events may still
    /// grant upgrades at their own (often discounted) prices.
    pub fn build_cost(&self) -> ResourceDelta {
        let (food, wood, stone) = match self {
            Upgrade::Watchtower => (0, 10, 8),
            Upgrade::Farm => (0, 15, 0),
            Upgrade::Infirmary => (0, 12, 5),
            Upgrade::Blacksmith => (0, 10, 8),
            Upgrade::Granary => (0, 8, 12),
            Upgrade::Barracks => (0, 12, 12),
            Upgrade::Inn => (6, 14, 6),
            Upgrade::AdventurersGuild => (0, 16, 10),
        };
        ResourceDelta { food, wood, stone, ..Default::default() }
    }

    /// Specialist who must live here before the building can go up.
    pub fn required_role(&self) -> Option<Role> {
        match self {
            Upgrade::Farm => Some(Role::Farmer),
            Upgrade::Infirmary => Some(Role::Healer),
            Upgrade::Blacksmith => Some(Role::Blacksmith),
            Upgrade::Barracks => Some(Role::Guard),
            Upgrade::Watchtower | Upgrade::Granary | Upgrade::Inn | Upgrade::AdventurersGuild => {
                None
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Fortress {
    pub name: String,
    pub day: u32,
    pub morale: i32,
    pub defense: i32,
    pub max_population: u32,
    pub upgrades: Vec<Upgrade>,
}

impl Fortress {
    pub fn new(name: &str) -> Fortress {
        Fortress {
            name: name.to_string(),
            day: 1,
            morale: 50,
            defense: 10,
            max_population: 20,
            upgrades: Vec::new(),
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

    pub fn add_upgrade(&mut self, upgrade: Upgrade) {
        if !self.has_upgrade(upgrade) {
            self.upgrades.push(upgrade);
        }
    }

    pub fn has_upgrade(&self, upgrade: Upgrade) -> bool {
        self.upgrades.contains(&upgrade)
    }

    /// Beds available: the Keep sleeps 6; Barracks and Inn add more.
    /// Anyone over capacity sleeps rough in the stables or courtyard.
    pub fn sleeping_capacity(&self) -> u32 {
        let mut beds = 6;
        if self.has_upgrade(Upgrade::Barracks) {
            beds += 5;
        }
        if self.has_upgrade(Upgrade::Inn) {
            beds += 5;
        }
        beds
    }

    pub fn is_defeated(&self) -> bool {
        self.morale == 0
    }
}
