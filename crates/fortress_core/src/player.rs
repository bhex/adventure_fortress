use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::rng::GameRng;
use crate::skills::{Skill, SkillSet};

// ---------------------------------------------------------------------------
// Abilities
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerAbility {
    IronWill,
    Tactician,
    IronRations,
    WarCry,
    OathKeeper,
    SilverTongue,
    BattleHardened,
    LivingLegend,
    Resourceful,
    Fortify,
}

impl PlayerAbility {
    pub const ALL: [PlayerAbility; 10] = [
        PlayerAbility::IronWill,
        PlayerAbility::Tactician,
        PlayerAbility::IronRations,
        PlayerAbility::WarCry,
        PlayerAbility::OathKeeper,
        PlayerAbility::SilverTongue,
        PlayerAbility::BattleHardened,
        PlayerAbility::LivingLegend,
        PlayerAbility::Resourceful,
        PlayerAbility::Fortify,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            PlayerAbility::IronWill => "Iron Will",
            PlayerAbility::Tactician => "Tactician",
            PlayerAbility::IronRations => "Iron Rations",
            PlayerAbility::WarCry => "War Cry",
            PlayerAbility::OathKeeper => "Oath Keeper",
            PlayerAbility::SilverTongue => "Silver Tongue",
            PlayerAbility::BattleHardened => "Battle Hardened",
            PlayerAbility::LivingLegend => "Living Legend",
            PlayerAbility::Resourceful => "Resourceful",
            PlayerAbility::Fortify => "Fortify",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            PlayerAbility::IronWill => "Each morale loss to the fortress is reduced by 2.",
            PlayerAbility::Tactician => "Your defense cannot fall below 10.",
            PlayerAbility::IronRations => "Food upkeep reduced by 1 each day.",
            PlayerAbility::WarCry => "After each combat event, all guards gain +10 morale.",
            PlayerAbility::OathKeeper => "Inhabitants will never abandon the fortress.",
            PlayerAbility::SilverTongue => "All stat check difficulties are reduced by 1.",
            PlayerAbility::BattleHardened => "Combat damage to your people is reduced by an additional 25%.",
            PlayerAbility::LivingLegend => "Successful stat checks restore +3 fortress morale.",
            PlayerAbility::Resourceful => "The fortress gathers +2 wood and +1 stone each day.",
            PlayerAbility::Fortify => "The fortress gains +1 defense at the start of each day.",
        }
    }
}

/// Returns up to 3 random abilities the player does not already possess.
pub fn ability_offers(player: &PlayerCharacter, rng: &mut GameRng) -> Vec<PlayerAbility> {
    let owned: std::collections::HashSet<PlayerAbility> = player.abilities.iter().cloned().collect();
    let mut available: Vec<PlayerAbility> = PlayerAbility::ALL
        .iter()
        .cloned()
        .filter(|a| !owned.contains(a))
        .collect();
    available.shuffle(rng);
    available.into_iter().take(3).collect()
}

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
    Warlord,
    Steward,
    Mystic,
}

impl ClassKind {
    pub const ALL: [ClassKind; 3] = [ClassKind::Warlord, ClassKind::Steward, ClassKind::Mystic];

    pub fn name(&self) -> &'static str {
        match self {
            ClassKind::Warlord => "Warlord",
            ClassKind::Steward => "Steward",
            ClassKind::Mystic => "Mystic",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ClassKind::Warlord => "+2 Might. Your people take less harm in battle.",
            ClassKind::Steward => "+2 Wit. Starts with extra valuables; barter deals cost less.",
            ClassKind::Mystic => "+2 Heart. +1 daily morale while the people's spirits hold.",
        }
    }

    pub fn bonus_stat(&self) -> StatKind {
        match self {
            ClassKind::Warlord => StatKind::Might,
            ClassKind::Steward => StatKind::Wit,
            ClassKind::Mystic => StatKind::Heart,
        }
    }

    /// The trade a commander hones by ruling: drilled daily like any worker.
    pub fn home_skill(&self) -> Skill {
        match self {
            ClassKind::Warlord => Skill::Combat,
            ClassKind::Steward => Skill::Crafting,
            ClassKind::Mystic => Skill::Medicine,
        }
    }
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum StatKind {
    Might,
    Wit,
    Heart,
}

impl StatKind {
    pub const ALL: [StatKind; 3] = [StatKind::Might, StatKind::Wit, StatKind::Heart];

    pub fn name(&self) -> &'static str {
        match self {
            StatKind::Might => "Might",
            StatKind::Wit => "Wit",
            StatKind::Heart => "Heart",
        }
    }
}

pub const STAT_BASE: u8 = 3;
pub const STAT_CAP: u8 = 8;
pub const FREE_POINTS: u8 = 5;
pub const CLASS_BONUS: u8 = 2;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub might: u8,
    pub wit: u8,
    pub heart: u8,
}

impl Default for Stats {
    fn default() -> Stats {
        Stats { might: STAT_BASE, wit: STAT_BASE, heart: STAT_BASE }
    }
}

impl Stats {
    pub fn get(&self, kind: StatKind) -> u8 {
        match kind {
            StatKind::Might => self.might,
            StatKind::Wit => self.wit,
            StatKind::Heart => self.heart,
        }
    }

    pub fn get_mut(&mut self, kind: StatKind) -> &mut u8 {
        match kind {
            StatKind::Might => &mut self.might,
            StatKind::Wit => &mut self.wit,
            StatKind::Heart => &mut self.heart,
        }
    }

    pub fn total(&self) -> u8 {
        self.might + self.wit + self.heart
    }
}

// ---------------------------------------------------------------------------
// Player character
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerCharacter {
    pub name: String,
    pub class: ClassKind,
    pub stats: Stats,
    pub level: u32,
    pub abilities: Vec<PlayerAbility>,
    // The commander lives by the same rules as everyone else: they can be
    // wounded, lose heart, and fall — and the realm falls with them.
    #[serde(default = "default_player_health")]
    pub health: i32,
    #[serde(default = "default_player_morale")]
    pub morale: i32,
    #[serde(default)]
    pub skills: SkillSet,
}

fn default_player_health() -> i32 {
    100
}

fn default_player_morale() -> i32 {
    50
}

impl PlayerCharacter {
    pub fn new(name: &str, class: ClassKind, stats: Stats) -> PlayerCharacter {
        PlayerCharacter {
            name: name.to_string(),
            class,
            stats,
            level: 1,
            abilities: Vec::new(),
            health: default_player_health(),
            morale: default_player_morale(),
            skills: SkillSet::default(),
        }
    }

    pub fn has_ability(&self, ability: PlayerAbility) -> bool {
        self.abilities.contains(&ability)
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0
    }

    /// Mirrors `Inhabitant::damage` clamping — the commander has no traits,
    /// so wounds land at face value.
    pub fn damage(&mut self, amount: i32) {
        self.health = (self.health - amount).max(0);
    }

    pub fn heal(&mut self, amount: i32) {
        self.health = (self.health + amount).min(100);
    }

    pub fn apply_morale(&mut self, amount: i32) {
        self.morale = (self.morale + amount).clamp(0, 100);
    }
}
