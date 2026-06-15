//! Adventurers: wandering heroes who take residence once the fortress has a
//! Guild and a name worth traveling for. Each has a class and a perk whose
//! strength scales with the skill they practice — no levels, only use.

use rand::seq::IndexedRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::rng::GameRng;
use crate::skills::{Skill, SkillSet, SkillTier};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AdventurerClass {
    Knight,
    Ranger,
    Sorcerer,
    Cleric,
}

impl AdventurerClass {
    pub const ALL: [AdventurerClass; 4] = [
        AdventurerClass::Knight,
        AdventurerClass::Ranger,
        AdventurerClass::Sorcerer,
        AdventurerClass::Cleric,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            AdventurerClass::Knight => "knight",
            AdventurerClass::Ranger => "ranger",
            AdventurerClass::Sorcerer => "sorcerer",
            AdventurerClass::Cleric => "cleric",
        }
    }

    pub fn perk_name(&self) -> &'static str {
        match self {
            AdventurerClass::Knight => "Shield of the Walls",
            AdventurerClass::Ranger => "Provider",
            AdventurerClass::Sorcerer => "Veilbane",
            AdventurerClass::Cleric => "Mender",
        }
    }

    /// The skill the perk scales with — and what they practice daily.
    pub fn home_skill(&self) -> Skill {
        match self {
            AdventurerClass::Knight | AdventurerClass::Ranger | AdventurerClass::Sorcerer => {
                Skill::Combat
            }
            AdventurerClass::Cleric => Skill::Medicine,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Adventurer {
    pub name: String,
    pub class: AdventurerClass,
    pub skills: SkillSet,
    /// Arms the hero bears — only the sworn knights take fortress steel; other
    /// heroes carry their own and leave the loadout empty.
    #[serde(default)]
    pub loadout: crate::items::Loadout,
}

impl Adventurer {
    /// The tier driving this hero's perk strength.
    pub fn perk_tier(&self) -> SkillTier {
        self.skills.tier(self.class.home_skill())
    }
}

const ADVENTURER_NAMES: [&str; 12] = [
    "Ser Branwen",
    "Kestrel",
    "Maro the Grey",
    "Ysolde",
    "Talric",
    "Wren Halfboot",
    "Dame Orla",
    "Corvin",
    "Saphira",
    "Old Edda",
    "Lucan",
    "Merryl",
];

/// Heroes arrive with real experience — that's why they're heroes.
pub fn generate_adventurer(rng: &mut GameRng) -> Adventurer {
    let class = *AdventurerClass::ALL.choose(rng).unwrap();
    let name = ADVENTURER_NAMES.choose(rng).unwrap().to_string();
    let mut skills = SkillSet::default();
    skills.train(class.home_skill(), rng.random_range(50..=120));
    Adventurer { name, class, skills, loadout: crate::items::Loadout::default() }
}
