//! Dwarf-Fortress-style skills: every unit has every skill at a tier,
//! grown by use and daily practice — never assigned, never reset.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Skill {
    Combat,
    Farming,
    Medicine,
    Smithing,
    Crafting,
    // Magic is skill-driven: classes differ by which of these they start with.
    Sorcery, // raw offensive magic
    Warding, // defensive & utility magic
    Stealth, // shadows, infiltration (Mystic)
    DarkArts, // the darker, riskier arts (Warlock)
}

impl Skill {
    pub const ALL: [Skill; 9] = [
        Skill::Combat,
        Skill::Farming,
        Skill::Medicine,
        Skill::Smithing,
        Skill::Crafting,
        Skill::Sorcery,
        Skill::Warding,
        Skill::Stealth,
        Skill::DarkArts,
    ];

    /// The magic skills — used to flag mages and gate arcane effects.
    pub const MAGIC: [Skill; 4] =
        [Skill::Sorcery, Skill::Warding, Skill::Stealth, Skill::DarkArts];

    pub fn is_magic(&self) -> bool {
        Skill::MAGIC.contains(self)
    }

    pub fn name(&self) -> &'static str {
        match self {
            Skill::Combat => "combat",
            Skill::Farming => "farming",
            Skill::Medicine => "medicine",
            Skill::Smithing => "smithing",
            Skill::Crafting => "crafting",
            Skill::Sorcery => "sorcery",
            Skill::Warding => "warding",
            Skill::Stealth => "stealth",
            Skill::DarkArts => "dark arts",
        }
    }

    /// What you call someone practicing this skill ("a skilled fighter").
    pub fn practitioner(&self) -> &'static str {
        match self {
            Skill::Combat => "fighter",
            Skill::Farming => "farmer",
            Skill::Medicine => "physician",
            Skill::Smithing => "smith",
            Skill::Crafting => "crafter",
            Skill::Sorcery => "sorcerer",
            Skill::Warding => "warden",
            Skill::Stealth => "shadow",
            Skill::DarkArts => "warlock",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkillTier {
    Dabbling,
    Novice,
    Competent,
    Skilled,
    Proficient,
    Expert,
    Master,
    Legendary,
}

/// XP required to reach each tier (index-aligned with SkillTier).
const TIER_THRESHOLDS: [u32; 8] = [0, 20, 50, 90, 140, 200, 280, 400];

impl SkillTier {
    pub const ALL: [SkillTier; 8] = [
        SkillTier::Dabbling,
        SkillTier::Novice,
        SkillTier::Competent,
        SkillTier::Skilled,
        SkillTier::Proficient,
        SkillTier::Expert,
        SkillTier::Master,
        SkillTier::Legendary,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            SkillTier::Dabbling => "dabbling",
            SkillTier::Novice => "novice",
            SkillTier::Competent => "competent",
            SkillTier::Skilled => "skilled",
            SkillTier::Proficient => "proficient",
            SkillTier::Expert => "expert",
            SkillTier::Master => "master",
            SkillTier::Legendary => "legendary",
        }
    }

    /// 0 (Dabbling) .. 7 (Legendary) — used for scaling effects.
    pub fn index(&self) -> u32 {
        SkillTier::ALL.iter().position(|t| t == self).unwrap() as u32
    }
}

pub fn tier_for_xp(xp: u32) -> SkillTier {
    let mut tier = SkillTier::Dabbling;
    for (i, threshold) in TIER_THRESHOLDS.iter().enumerate() {
        if xp >= *threshold {
            tier = SkillTier::ALL[i];
        }
    }
    tier
}

/// Per-unit skill XP. Explicit fields (no maps) for port-friendly serde.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SkillSet {
    pub combat: u32,
    pub farming: u32,
    pub medicine: u32,
    pub smithing: u32,
    pub crafting: u32,
    #[serde(default)]
    pub sorcery: u32,
    #[serde(default)]
    pub warding: u32,
    #[serde(default)]
    pub stealth: u32,
    #[serde(default)]
    pub dark_arts: u32,
}

impl SkillSet {
    pub fn xp(&self, skill: Skill) -> u32 {
        match skill {
            Skill::Combat => self.combat,
            Skill::Farming => self.farming,
            Skill::Medicine => self.medicine,
            Skill::Smithing => self.smithing,
            Skill::Crafting => self.crafting,
            Skill::Sorcery => self.sorcery,
            Skill::Warding => self.warding,
            Skill::Stealth => self.stealth,
            Skill::DarkArts => self.dark_arts,
        }
    }

    fn xp_mut(&mut self, skill: Skill) -> &mut u32 {
        match skill {
            Skill::Combat => &mut self.combat,
            Skill::Farming => &mut self.farming,
            Skill::Medicine => &mut self.medicine,
            Skill::Smithing => &mut self.smithing,
            Skill::Crafting => &mut self.crafting,
            Skill::Sorcery => &mut self.sorcery,
            Skill::Warding => &mut self.warding,
            Skill::Stealth => &mut self.stealth,
            Skill::DarkArts => &mut self.dark_arts,
        }
    }

    pub fn tier(&self, skill: Skill) -> SkillTier {
        tier_for_xp(self.xp(skill))
    }

    /// Add XP from use or practice. Returns the new tier if one was reached.
    pub fn train(&mut self, skill: Skill, amount: u32) -> Option<SkillTier> {
        let before = self.tier(skill);
        let xp = self.xp_mut(skill);
        *xp = (*xp + amount).min(TIER_THRESHOLDS[TIER_THRESHOLDS.len() - 1]);
        let after = self.tier(skill);
        (after > before).then_some(after)
    }

    /// Best (tier, skill) pair — what this unit is known for.
    pub fn signature(&self) -> (SkillTier, Skill) {
        let mut best = (self.tier(Skill::Combat), Skill::Combat);
        for skill in Skill::ALL {
            let tier = self.tier(skill);
            if tier > best.0 {
                best = (tier, skill);
            }
        }
        best
    }
}
