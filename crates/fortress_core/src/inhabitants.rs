use rand::seq::{IndexedRandom, SliceRandom};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::rng::{weighted_index, GameRng};
use crate::skills::{Skill, SkillSet};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Guard,
    Farmer,
    Blacksmith,
    Healer,
    /// Works the Mine — better stone and ore than a peasant filling in.
    Miner,
    /// The unspecialized arrival: general labor, learns a trade over time.
    Peasant,
}

impl Role {
    pub const ALL: [Role; 6] =
        [Role::Guard, Role::Farmer, Role::Blacksmith, Role::Healer, Role::Miner, Role::Peasant];

    /// Roles a peasant can drift into / be assigned to (the real trades).
    /// Mining isn't a drift target — it has no distinct skill of its own — so
    /// miners are made by hand, not by aptitude.
    pub const TRADES: [Role; 4] = [Role::Guard, Role::Farmer, Role::Blacksmith, Role::Healer];

    pub fn name(&self) -> &'static str {
        match self {
            Role::Guard => "guard",
            Role::Farmer => "farmer",
            Role::Blacksmith => "blacksmith",
            Role::Healer => "healer",
            Role::Miner => "miner",
            Role::Peasant => "peasant",
        }
    }

    /// The skill this role practices daily when its workplace exists.
    pub fn home_skill(&self) -> Skill {
        match self {
            Role::Guard => Skill::Combat,
            Role::Farmer => Skill::Farming,
            Role::Blacksmith => Skill::Smithing,
            Role::Healer => Skill::Medicine,
            // Miners are laborers underground — they keep their hand in at craft.
            Role::Miner | Role::Peasant => Skill::Crafting,
        }
    }

    /// Only guards stand in the line by default; the rest fight only when the
    /// gate is breached (handled in combat). Peasants are non-combatants.
    pub fn is_combatant(&self) -> bool {
        matches!(self, Role::Guard)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Trait {
    Brave,
    Skilled,
    Sickly,
    Loyal,
    Greedy,
    Strong,
    Clever,
    Hardy,
    Cowardly,
    Lazy,
    Devout,
    /// Hidden: a spy slipped in from the dark. Never shown; bides, then betrays.
    Infiltrator,
}

impl Trait {
    /// Traits an ordinary arrival may roll (excludes the hidden Infiltrator,
    /// which is seeded separately when the darkness runs deep).
    pub const ROLLABLE: [Trait; 11] = [
        Trait::Brave,
        Trait::Skilled,
        Trait::Sickly,
        Trait::Loyal,
        Trait::Greedy,
        Trait::Strong,
        Trait::Clever,
        Trait::Hardy,
        Trait::Cowardly,
        Trait::Lazy,
        Trait::Devout,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Trait::Brave => "brave",
            Trait::Skilled => "skilled",
            Trait::Sickly => "sickly",
            Trait::Loyal => "loyal",
            Trait::Greedy => "greedy",
            Trait::Strong => "strong",
            Trait::Clever => "clever",
            Trait::Hardy => "hardy",
            Trait::Cowardly => "cowardly",
            Trait::Lazy => "lazy",
            Trait::Devout => "devout",
            Trait::Infiltrator => "infiltrator",
        }
    }

    /// Hidden traits are never surfaced to the player in the roster/inspect.
    pub fn is_hidden(&self) -> bool {
        matches!(self, Trait::Infiltrator)
    }
}

const GUARD_NAMES: [&str; 10] = ["Aldric", "Bran", "Cedric", "Doran", "Edric", "Farrell", "Gareth", "Hadwin", "Idris", "Jareth"];
const FARMER_NAMES: [&str; 10] = ["Abel", "Barrett", "Colm", "Davin", "Emmet", "Finley", "Greer", "Hayden", "Ivar", "Jowan"];
const BLACKSMITH_NAMES: [&str; 10] = ["Aldous", "Bryn", "Cade", "Duncan", "Eamon", "Fergus", "Gawain", "Hadleigh", "Ivan", "Jorin"];
const HEALER_NAMES: [&str; 10] = ["Aideen", "Brenna", "Ciara", "Deirdre", "Eileen", "Fiona", "Grainne", "Hilde", "Isla", "Jorah"];

const PEASANT_NAMES: [&str; 10] =
    ["Alby", "Bryn", "Cleg", "Dell", "Emm", "Fenn", "Gil", "Hob", "Ned", "Ott"];

const MINER_NAMES: [&str; 10] =
    ["Borin", "Delf", "Grum", "Korin", "Maddoc", "Nael", "Orin", "Pike", "Rorek", "Thane"];

fn names_for(role: Role) -> &'static [&'static str] {
    match role {
        Role::Guard => &GUARD_NAMES,
        Role::Farmer => &FARMER_NAMES,
        Role::Blacksmith => &BLACKSMITH_NAMES,
        Role::Healer => &HEALER_NAMES,
        Role::Miner => &MINER_NAMES,
        Role::Peasant => &PEASANT_NAMES,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Inhabitant {
    pub name: String,
    pub role: Role,
    pub health: i32,
    pub morale: i32,
    pub traits: Vec<Trait>,
    pub is_alive: bool,
    #[serde(default)]
    pub skills: SkillSet,
    /// What this soul carries — set by the daily auto-equip pass.
    #[serde(default)]
    pub loadout: crate::items::Loadout,
}

impl Inhabitant {
    pub fn new(name: &str, role: Role) -> Inhabitant {
        Inhabitant {
            name: name.to_string(),
            role,
            health: 100,
            morale: 50,
            traits: Vec::new(),
            is_alive: true,
            skills: SkillSet::default(),
            loadout: crate::items::Loadout::default(),
        }
    }

    pub fn has_trait(&self, t: Trait) -> bool {
        self.traits.contains(&t)
    }

    pub fn damage(&mut self, amount: i32) {
        let actual = if self.has_trait(Trait::Sickly) { amount * 2 } else { amount };
        self.health = (self.health - actual).max(0);
        if self.health == 0 {
            self.is_alive = false;
        }
    }

    pub fn heal(&mut self, amount: i32) {
        self.health = (self.health + amount).min(100);
    }

    pub fn apply_morale(&mut self, amount: i32) {
        self.morale = (self.morale + amount).clamp(0, 100);
    }
}

/// Most who reach the gates are common folk; trained specialists are rarer.
/// You shape your garrison by assigning roles, not by who happens to arrive.
pub fn random_arrival_role(rng: &mut GameRng) -> Role {
    match weighted_index(rng, &[5.0, 2.0, 2.0, 1.0, 1.0]).unwrap_or(0) {
        0 => Role::Peasant,
        1 => Role::Farmer,
        2 => Role::Guard,
        3 => Role::Blacksmith,
        _ => Role::Healer,
    }
}

pub fn generate_inhabitant(role: Role, rng: &mut GameRng) -> Inhabitant {
    let name = names_for(role).choose(rng).unwrap().to_string();
    let num_traits = weighted_index(rng, &[4.0, 4.0, 2.0]).unwrap_or(0);
    let mut pool = Trait::ROLLABLE.to_vec();
    pool.shuffle(rng);
    let traits: Vec<Trait> = pool.into_iter().take(num_traits).collect();
    let health = if traits.contains(&Trait::Sickly) { 70 } else { 100 };
    let morale = rng.random_range(40..=70);
    // Everyone arrives with some practice in their trade; the Skilled trait
    // means real prior experience.
    let mut skills = SkillSet::default();
    let starting_xp = if traits.contains(&Trait::Skilled) {
        rng.random_range(50..=90)
    } else {
        rng.random_range(5..=25)
    };
    skills.train(role.home_skill(), starting_xp);
    // A rare arrival has the arcane gift. Magic-users are uncommon, and never
    // healers — healing magic is rarer still and not granted here.
    if rng.random_range(0..100) < 6 {
        let school = [Skill::Sorcery, Skill::Warding, Skill::DarkArts][rng.random_range(0..3)];
        skills.train(school, rng.random_range(40..=90));
    }
    Inhabitant {
        name,
        role,
        health,
        morale,
        traits,
        is_alive: true,
        skills,
        loadout: crate::items::Loadout::default(),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct InhabitantManager {
    pub inhabitants: Vec<Inhabitant>,
}

impl InhabitantManager {
    pub fn add(&mut self, inhabitant: Inhabitant) {
        self.inhabitants.push(inhabitant);
    }

    pub fn remove(&mut self, name: &str) {
        self.inhabitants.retain(|i| i.name != name);
    }

    pub fn get_alive(&self) -> Vec<&Inhabitant> {
        self.inhabitants.iter().filter(|i| i.is_alive).collect()
    }

    pub fn get_by_role(&self, role: Role) -> Vec<&Inhabitant> {
        self.inhabitants
            .iter()
            .filter(|i| i.is_alive && i.role == role)
            .collect()
    }

    pub fn count_alive(&self) -> usize {
        self.inhabitants.iter().filter(|i| i.is_alive).count()
    }

    pub fn count_dead(&self) -> usize {
        self.inhabitants.len() - self.count_alive()
    }

    pub fn has_role(&self, role: Role) -> bool {
        self.inhabitants.iter().any(|i| i.is_alive && i.role == role)
    }

    pub fn random_survivor_name(&self, rng: &mut GameRng, role: Option<Role>) -> Option<String> {
        let pool: Vec<&Inhabitant> = match role {
            Some(r) => self.get_by_role(r),
            None => self.get_alive(),
        };
        pool.choose(rng).map(|i| i.name.clone())
    }

    pub fn random_non_loyal_name(&self, rng: &mut GameRng) -> Option<String> {
        let pool: Vec<&Inhabitant> = self
            .inhabitants
            .iter()
            .filter(|i| i.is_alive && !i.has_trait(Trait::Loyal))
            .collect();
        pool.choose(rng).map(|i| i.name.clone())
    }

    pub fn find_mut(&mut self, name: &str) -> Option<&mut Inhabitant> {
        self.inhabitants.iter_mut().find(|i| i.name == name)
    }

    pub fn average_morale(&self) -> i32 {
        let alive = self.get_alive();
        if alive.is_empty() {
            return 50;
        }
        alive.iter().map(|i| i.morale as i64).sum::<i64>() as i32 / alive.len() as i32
    }

    /// Returns names of inhabitants who died as a result.
    pub fn apply_to_role(&mut self, role: Role, health_delta: i32, morale_delta: i32) -> Vec<String> {
        let mut deaths = Vec::new();
        for i in self.inhabitants.iter_mut().filter(|i| i.is_alive && i.role == role) {
            if health_delta < 0 {
                i.damage(-health_delta);
            } else if health_delta > 0 {
                i.heal(health_delta);
            }
            i.apply_morale(morale_delta);
            if !i.is_alive {
                deaths.push(i.name.clone());
            }
        }
        deaths
    }
}
