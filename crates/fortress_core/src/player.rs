use serde::{Deserialize, Serialize};

use crate::skills::{Skill, SkillSet};

// ---------------------------------------------------------------------------
// Classes
// ---------------------------------------------------------------------------
//
// The commander is "an inhabitant who leads": no levels, no talent picks —
// just a class that grants a starting skill profile, which then grows by use
// like everyone else's. Magic is skill-driven (see `Skill`), so a class is
// really just where its skills begin.

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassKind {
    Warlord,
    Steward,
    Wizard,
    Mystic,
    Warlock,
    Sorcerer,
}

impl ClassKind {
    pub const ALL: [ClassKind; 6] = [
        ClassKind::Warlord,
        ClassKind::Steward,
        ClassKind::Wizard,
        ClassKind::Mystic,
        ClassKind::Warlock,
        ClassKind::Sorcerer,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ClassKind::Warlord => "Warlord",
            ClassKind::Steward => "Steward",
            ClassKind::Wizard => "Wizard",
            ClassKind::Mystic => "Mystic",
            ClassKind::Warlock => "Warlock",
            ClassKind::Sorcerer => "Sorcerer",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ClassKind::Warlord => "A war-leader. Skilled in arms; the garrison fights harder at their side.",
            ClassKind::Steward => "An administrator. Deep in crafts and logistics; barter comes cheaper.",
            ClassKind::Wizard => "A trained spellcaster — wards, bolts, and utility, with a blade in a pinch.",
            ClassKind::Mystic => "A shadow-walker: stealth and battle-magic, some steel, a little healing.",
            ClassKind::Warlock => "A dabbler in darker arts — potent, perilous, and unloved by the light.",
            ClassKind::Sorcerer => "Raw innate power: fearsome offensive magic, hard to fully control.",
        }
    }

    pub fn bonus_stat(&self) -> StatKind {
        match self {
            ClassKind::Warlord => StatKind::Might,
            ClassKind::Steward => StatKind::Wit,
            ClassKind::Wizard => StatKind::Wit,
            ClassKind::Mystic => StatKind::Heart,
            ClassKind::Warlock => StatKind::Heart,
            ClassKind::Sorcerer => StatKind::Might,
        }
    }

    /// The trade the commander hones daily by ruling (drilled like any worker).
    pub fn home_skill(&self) -> Skill {
        match self {
            ClassKind::Warlord => Skill::Combat,
            ClassKind::Steward => Skill::Crafting,
            ClassKind::Wizard => Skill::Sorcery,
            ClassKind::Mystic => Skill::Stealth,
            ClassKind::Warlock => Skill::DarkArts,
            ClassKind::Sorcerer => Skill::Sorcery,
        }
    }

    pub fn is_mage(&self) -> bool {
        matches!(self, ClassKind::Wizard | ClassKind::Mystic | ClassKind::Warlock | ClassKind::Sorcerer)
    }

    /// Where this class's skills begin (xp). Everything else starts at zero.
    pub fn starting_skills(&self) -> &'static [(Skill, u32)] {
        match self {
            ClassKind::Warlord => &[(Skill::Combat, 140), (Skill::Crafting, 30)],
            ClassKind::Steward => &[(Skill::Crafting, 120), (Skill::Farming, 40)],
            ClassKind::Wizard => &[(Skill::Sorcery, 110), (Skill::Warding, 90), (Skill::Combat, 20)],
            ClassKind::Mystic => {
                &[(Skill::Stealth, 110), (Skill::Sorcery, 60), (Skill::Combat, 40), (Skill::Medicine, 20)]
            }
            ClassKind::Warlock => &[(Skill::DarkArts, 130), (Skill::Sorcery, 50)],
            ClassKind::Sorcerer => &[(Skill::Sorcery, 160), (Skill::Warding, 30)],
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
// Player character — the commander. Lives by the same rules as everyone else:
// can be wounded, lose heart, and fall, and the realm falls with them.
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerCharacter {
    pub name: String,
    pub class: ClassKind,
    pub stats: Stats,
    #[serde(default = "default_player_health")]
    pub health: i32,
    #[serde(default = "default_player_morale")]
    pub morale: i32,
    #[serde(default)]
    pub skills: SkillSet,
    /// The arms the commander bears — set by the daily auto-equip pass.
    #[serde(default)]
    pub loadout: crate::items::Loadout,
}

fn default_player_health() -> i32 {
    100
}

fn default_player_morale() -> i32 {
    50
}

impl PlayerCharacter {
    pub fn new(name: &str, class: ClassKind, stats: Stats) -> PlayerCharacter {
        let mut skills = SkillSet::default();
        for (skill, xp) in class.starting_skills() {
            skills.train(*skill, *xp);
        }
        PlayerCharacter {
            name: name.to_string(),
            class,
            stats,
            health: default_player_health(),
            morale: default_player_morale(),
            skills,
            loadout: crate::items::Loadout::default(),
        }
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
