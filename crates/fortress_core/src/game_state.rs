use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::adventurers::{generate_adventurer, Adventurer, AdventurerClass};
use crate::engine::train_role;
use crate::fortress::{Fortress, Upgrade};
use crate::region::DarknessBand;
use crate::inhabitants::{generate_inhabitant, InhabitantManager, Role};
use crate::player::{ability_offers, ClassKind, PlayerAbility, PlayerCharacter};
use crate::region::Region;
use crate::resources::{ResourceDelta, Resources};
use crate::rng::GameRng;
use crate::skills::Skill;

pub const SAVE_VERSION: u32 = 4;

/// Events resolved per commander level. Every threshold crossed triggers an ability draft.
pub const LEVEL_UP_INTERVAL: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported save version {0} (expected {SAVE_VERSION})")]
    Version(u32),
}

/// Why a building can or can't go up right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildAvailability {
    Ok,
    AlreadyBuilt,
    MissingRole(Role),
    CantAfford,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameState {
    pub version: u32,
    pub run_seed: u64,
    pub rng: GameRng,
    pub events_resolved: u32,
    pub fortress: Fortress,
    pub resources: Resources,
    pub inhabitants: InhabitantManager,
    pub player: Option<PlayerCharacter>,
    pub region: Region,
    /// Renown 0-100: victories and prosperity raise it, deaths and desertions
    /// spend it. Gates adventurer arrivals.
    pub reputation: i32,
    pub adventurers: Vec<Adventurer>,
}

/// Most heroes a fortress can host at once.
pub const MAX_ADVENTURERS: usize = 4;
/// Renown below this and no hero bothers with the road.
pub const ADVENTURER_MIN_REPUTATION: i32 = 20;

impl GameState {
    pub fn new(run_seed: u64) -> GameState {
        let mut rng = GameRng::seed_from_u64(run_seed);
        let region = Region::generate(&mut rng);
        GameState {
            version: SAVE_VERSION,
            run_seed,
            rng,
            events_resolved: 0,
            fortress: Fortress::new(""),
            resources: Resources::default(),
            inhabitants: InhabitantManager::default(),
            player: None,
            region,
            reputation: 10,
            adventurers: Vec::new(),
        }
    }

    pub fn apply_reputation_delta(&mut self, amount: i32) {
        self.reputation = (self.reputation + amount).clamp(0, 100);
    }

    pub fn new_game(run_seed: u64, fortress_name: &str, player: PlayerCharacter) -> GameState {
        let mut gs = GameState::new(run_seed);
        gs.fortress.name = fortress_name.to_string();
        gs.resources.apply_delta(&ResourceDelta {
            food: 40,
            valuables: if player.class == ClassKind::Steward { 14 } else { 8 },
            wood: 20,
            stone: 10,
            tools: 4,
            ..Default::default()
        });
        for role in [Role::Guard, Role::Farmer, Role::Farmer, Role::Healer] {
            let inhabitant = generate_inhabitant(role, &mut gs.rng);
            gs.inhabitants.add(inhabitant);
        }
        gs.player = Some(player);
        gs
    }

    // ------------------------------------------------------------------
    // Progression
    // ------------------------------------------------------------------

    pub fn build_upgrade(&mut self, upgrade: Upgrade) -> String {
        if self.fortress.has_upgrade(upgrade) {
            return format!("{} is already built.", upgrade.name());
        }
        self.fortress.add_upgrade(upgrade);
        match upgrade {
            Upgrade::Watchtower => self.fortress.apply_defense_delta(5),
            Upgrade::Barracks => {
                self.fortress.max_population += 5;
                self.fortress.apply_defense_delta(2);
            }
            Upgrade::Inn => self.fortress.max_population += 5,
            _ => {}
        }
        // Word of a growing fortress travels.
        self.apply_reputation_delta(2);
        format!("{} has been built!", upgrade.name())
    }

    /// Whether the build menu may raise this upgrade right now.
    pub fn build_availability(&self, upgrade: Upgrade) -> BuildAvailability {
        if self.fortress.has_upgrade(upgrade) {
            return BuildAvailability::AlreadyBuilt;
        }
        if let Some(role) = upgrade.required_role() {
            if !self.inhabitants.has_role(role) {
                return BuildAvailability::MissingRole(role);
            }
        }
        if !self.resources.can_afford(&upgrade.build_cost()) {
            return BuildAvailability::CantAfford;
        }
        BuildAvailability::Ok
    }

    /// Pay the materials and raise the building. Errs with the blocking reason.
    pub fn construct(&mut self, upgrade: Upgrade) -> Result<String, BuildAvailability> {
        match self.build_availability(upgrade) {
            BuildAvailability::Ok => {
                self.resources.apply_delta(&upgrade.build_cost().negated());
                Ok(self.build_upgrade(upgrade))
            }
            blocked => Err(blocked),
        }
    }

    /// Returns true when the most recently resolved event crosses a level-up threshold.
    pub fn should_level_up(&self) -> bool {
        self.events_resolved > 0 && self.events_resolved % LEVEL_UP_INTERVAL == 0
    }

    /// Generates up to 3 ability offers for the current player, consuming RNG.
    pub fn ability_offers(&mut self) -> Vec<PlayerAbility> {
        if let Some(player) = &self.player {
            ability_offers(player, &mut self.rng)
        } else {
            vec![]
        }
    }

    /// Applies a chosen ability and increments the player's level.
    pub fn apply_level_up(&mut self, ability: PlayerAbility) {
        if let Some(player) = &mut self.player {
            player.level += 1;
            player.abilities.push(ability);
        }
    }

    /// Day-end passive tick: upgrades, food upkeep, morale cascade. Returns log lines.
    pub fn apply_daily_effects(&mut self) -> Vec<String> {
        let mut lines = Vec::new();

        // The wider war: darkness shifts, sites hold or fall.
        lines.extend(self.region.tick(&mut self.rng));

        // Refugee waves from fallen sites: survivors reach the gates over the
        // following days — the main road to rare specialists.
        if self.region.refugee_wave_days > 0 {
            self.region.refugee_wave_days -= 1;
            let arrivals = self.rng.random_range(2..=3);
            let mut joined = 0;
            for _ in 0..arrivals {
                if self.inhabitants.count_alive() as u32 >= self.fortress.max_population {
                    break;
                }
                let role = Role::ALL[self.rng.random_range(0..Role::ALL.len())];
                let refugee = generate_inhabitant(role, &mut self.rng);
                lines.push(format!(
                    "{} the {} arrives with the refugees.",
                    refugee.name,
                    refugee.role.name()
                ));
                self.inhabitants.add(refugee);
                joined += 1;
            }
            if joined == 0 {
                lines.push("Refugees pass the gates by — the fortress has no room.".to_string());
            }
        }

        // Adventurers: heroes seek a guild, a name worth the road — and a fight.
        // The deeper the darkness, the more of them come looking for it.
        if self.fortress.has_upgrade(Upgrade::AdventurersGuild)
            && self.reputation >= ADVENTURER_MIN_REPUTATION
            && self.adventurers.len() < MAX_ADVENTURERS
        {
            let mut chance = self.reputation; // per-mille
            match self.region.band() {
                DarknessBand::Deep => chance *= 2,
                DarknessBand::Overwhelming => chance *= 3,
                _ => {}
            }
            if self.rng.random_range(0..1000) < chance {
                let hero = generate_adventurer(&mut self.rng);
                lines.push(format!(
                    "{} the {} signs the guild ledger. ({})",
                    hero.name,
                    hero.class.name(),
                    hero.class.perk_name()
                ));
                self.adventurers.push(hero);
            }
        }

        // Heroes keep their edge, and their perks work for the fortress.
        for hero in &mut self.adventurers {
            hero.skills.train(hero.class.home_skill(), 2);
        }
        let mut ranger_food = 0i64;
        let mut veil_push = 0;
        let mut cleric_heal = 0i32;
        for hero in &self.adventurers {
            let tier = hero.perk_tier().index();
            match hero.class {
                AdventurerClass::Ranger => ranger_food += tier as i64,
                AdventurerClass::Sorcerer => veil_push += (tier as i32) / 2,
                AdventurerClass::Cleric => cleric_heal += 3 * tier as i32,
                AdventurerClass::Knight => {} // passive: softens combat damage
            }
        }
        if ranger_food > 0 {
            self.resources.apply_delta(&ResourceDelta { food: ranger_food, ..Default::default() });
            lines.push("The rangers return from the hunt with game.".to_string());
        }
        if veil_push > 0 {
            self.region.darkness = (self.region.darkness - veil_push).max(0);
        }
        if cleric_heal > 0 {
            if let Some(patient) = self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.health < 100)
                .min_by_key(|i| i.health)
            {
                patient.heal(cleric_heal);
                let name = patient.name.clone();
                lines.push(format!("The cleric tends {name}. (+{cleric_heal} health)"));
            }
        }

        // Fortify: +1 defense per day
        if self.player.as_ref().is_some_and(|p| p.has_ability(PlayerAbility::Fortify)) {
            self.fortress.apply_defense_delta(1);
            lines.push("The fortress grows stronger. (+1 defense)".to_string());
        }

        // Resourceful: +2 wood, +1 stone per day
        if self.player.as_ref().is_some_and(|p| p.has_ability(PlayerAbility::Resourceful)) {
            self.resources.apply_delta(&ResourceDelta { wood: 2, stone: 1, ..Default::default() });
            lines.push("Resourceful hands gather supplies. (+2 wood, +1 stone)".to_string());
        }

        // Daily practice: working your trade slowly builds the skill.
        const WORKPLACES: [(Role, Upgrade); 4] = [
            (Role::Guard, Upgrade::Barracks),
            (Role::Farmer, Upgrade::Farm),
            (Role::Healer, Upgrade::Infirmary),
            (Role::Blacksmith, Upgrade::Blacksmith),
        ];
        for (role, workplace) in WORKPLACES {
            if self.fortress.has_upgrade(workplace) {
                lines.extend(train_role(self, role, role.home_skill(), 2));
            }
        }

        // Craftwork: smiths forge gear at the smithy; everyone whittles tools.
        if self.fortress.has_upgrade(Upgrade::Blacksmith) {
            let forged: i64 = self
                .inhabitants
                .get_by_role(Role::Blacksmith)
                .iter()
                .map(|i| i.skills.tier(Skill::Smithing).index() as i64)
                .sum();
            if forged > 0 && self.resources.gear < 60 {
                self.resources.apply_delta(&ResourceDelta { gear: forged, ..Default::default() });
                lines.push("The forge rings; the armory grows.".to_string());
            }
        }
        let whittled: i64 = self
            .inhabitants
            .get_alive()
            .iter()
            .map(|i| i.skills.tier(Skill::Crafting).index() as i64)
            .sum::<i64>()
            / 2;
        if whittled > 0 && self.resources.tools < 60 {
            self.resources.apply_delta(&ResourceDelta { tools: whittled, ..Default::default() });
        }

        if self.fortress.has_upgrade(Upgrade::Farm) {
            let skill_bonus: u32 = self
                .inhabitants
                .get_by_role(Role::Farmer)
                .iter()
                .map(|i| i.skills.tier(Skill::Farming).index())
                .sum::<u32>()
                / 2;
            let tool_bonus: i64 =
                if self.resources.band(crate::resources::ResourceKind::Tools) >= crate::resources::StockBand::Adequate {
                    1
                } else {
                    0
                };
            let harvest = 3 + skill_bonus as i64 + tool_bonus;
            self.resources.apply_delta(&ResourceDelta { food: harvest, ..Default::default() });
            lines.push("The farm brings in the harvest.".to_string());
        }
        // The Inn: a warm hearth and a full common room lift every heart.
        if self.fortress.has_upgrade(Upgrade::Inn) {
            self.fortress.apply_morale_delta(1);
            lines.push("Laughter drifts from the inn. (+1 morale)".to_string());
        }

        if self.fortress.has_upgrade(Upgrade::Infirmary) {
            for i in self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.role == Role::Healer)
            {
                i.apply_morale(2);
            }
        }

        // Medicine: the healers tend the worst-off patient.
        let healing: i32 = self
            .inhabitants
            .get_by_role(Role::Healer)
            .iter()
            .map(|i| 2 * i.skills.tier(Skill::Medicine).index() as i32)
            .sum();
        if healing > 0 {
            if let Some(patient) = self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.health < 100)
                .min_by_key(|i| i.health)
            {
                patient.heal(healing);
                let name = patient.name.clone();
                lines.push(format!("The healers tend {name}. (+{healing} health)"));
            }
        }

        // Food upkeep: 1 per 2 alive inhabitants; Iron Rations reduces it by 1
        let alive = self.inhabitants.count_alive() as i64;
        if alive > 0 {
            let base_upkeep = (alive + 1) / 2;
            let discount = if self.player.as_ref().is_some_and(|p| p.has_ability(PlayerAbility::IronRations)) {
                1
            } else {
                0
            };
            let upkeep = (base_upkeep - discount).max(0);
            if self.resources.food >= upkeep {
                self.resources.apply_delta(&ResourceDelta { food: -upkeep, ..Default::default() });
            } else {
                self.resources.food = 0;
                self.fortress.apply_morale_delta(-5);
                lines.push("Not enough food! The people go hungry. (-5 morale)".to_string());
            }
        }

        // Sleep quality: enough beds lift spirits; the overflow sleeps rough.
        // Bed assignment is deterministic: first-come keeps the beds.
        if alive > 0 {
            let beds = self.fortress.sleeping_capacity() as i64;
            if alive <= beds {
                self.fortress.apply_morale_delta(1);
                lines.push("Everyone sleeps warm tonight. (+1 morale)".to_string());
            } else {
                let rough = alive - beds;
                for i in self
                    .inhabitants
                    .inhabitants
                    .iter_mut()
                    .filter(|i| i.is_alive)
                    .skip(beds as usize)
                {
                    i.apply_morale(-1);
                }
                lines.push(format!(
                    "{rough} sleep rough in the stables and courtyard. (-1 morale for them)"
                ));
            }
        }

        // Inhabitant morale cascades into fortress morale — and into renown:
        // travelers carry word of a thriving hold, or a miserable one.
        let avg = self.inhabitants.average_morale();
        if avg >= 65 {
            self.fortress.apply_morale_delta(2);
            self.apply_reputation_delta(1);
            lines.push("Spirits are high among the inhabitants. (+2 morale)".to_string());
        } else if avg <= 30 {
            self.fortress.apply_morale_delta(-2);
            self.apply_reputation_delta(-1);
            lines.push("Grumbling spreads through the halls. (-2 morale)".to_string());
        }

        // Mystic passive
        if let Some(player) = &self.player {
            if player.class == ClassKind::Mystic && avg >= 50 {
                self.fortress.apply_morale_delta(1);
            }
        }

        lines
    }

    // ------------------------------------------------------------------
    // Win / loss — no victory condition, the fortress always eventually falls
    // ------------------------------------------------------------------

    pub fn is_game_over(&self) -> bool {
        self.fortress.is_defeated()
    }

    // ------------------------------------------------------------------
    // Serialization
    // ------------------------------------------------------------------

    pub fn save(&self, path: &Path) -> Result<(), SaveError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<GameState, SaveError> {
        let json = std::fs::read_to_string(path)?;
        let gs: GameState = serde_json::from_str(&json)?;
        if gs.version != SAVE_VERSION {
            return Err(SaveError::Version(gs.version));
        }
        Ok(gs)
    }
}
