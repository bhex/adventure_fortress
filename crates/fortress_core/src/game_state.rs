use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

use crate::adventurers::{generate_adventurer, Adventurer, AdventurerClass};
use crate::engine::train_role;
use crate::fortress::{level_numeral, BuildOutcome, Fortress, Upgrade};
use crate::region::DarknessBand;
use crate::inhabitants::{generate_inhabitant, InhabitantManager, Role, Trait};
use crate::player::{ClassKind, PlayerCharacter};
use crate::region::Region;
use crate::resources::{ResourceDelta, Resources};
use crate::rng::GameRng;
use crate::skills::Skill;

pub const SAVE_VERSION: u32 = 7;

/// Events resolved per commander level. Every threshold crossed triggers an ability draft.

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported save version {0} (expected {SAVE_VERSION})")]
    Version(u32),
}

/// Why a building can or can't go up (or tier up) right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildAvailability {
    Ok,
    /// Tier III already, or every housing plot taken.
    MaxLevel,
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
    /// Story flags raised by events, gating multi-step arcs (see `engine`).
    /// A `BTreeSet` (not `HashSet`) so saves serialize in a stable order — the
    /// deterministic-run guarantee depends on it.
    #[serde(default)]
    pub flags: BTreeSet<String>,
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
            flags: BTreeSet::new(),
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

    /// Build at level 1 or raise one tier, applying the step's bonuses.
    /// Events use this too: granting an already-built building upgrades it.
    pub fn build_upgrade(&mut self, upgrade: Upgrade) -> String {
        match self.fortress.add_building(upgrade) {
            BuildOutcome::Built => {
                match upgrade {
                    Upgrade::Watchtower => self.fortress.apply_defense_delta(5),
                    Upgrade::Barracks => {
                        self.fortress.max_population += 5;
                        self.fortress.apply_defense_delta(2);
                    }
                    Upgrade::Housing => self.fortress.max_population += 5,
                    _ => {}
                }
                // Word of a growing fortress travels.
                self.apply_reputation_delta(2);
                format!("{} has been built!", upgrade.name())
            }
            BuildOutcome::Upgraded(level) => {
                match (upgrade, level) {
                    (Upgrade::Watchtower, 2) => self.fortress.apply_defense_delta(3),
                    (Upgrade::Watchtower, _) => self.fortress.apply_defense_delta(4),
                    (Upgrade::Barracks, _) => {
                        self.fortress.max_population += 2;
                        self.fortress.apply_defense_delta(1);
                    }
                    _ => {}
                }
                self.apply_reputation_delta(1);
                format!("The {} has been raised to tier {}!", upgrade.name(), level_numeral(level))
            }
            BuildOutcome::AtMax => {
                format!("The {} already stands at its height.", upgrade.name())
            }
            BuildOutcome::NoPlots => "There is no room for more housing.".to_string(),
        }
    }

    /// Whether the build menu may raise (or tier up) this building right now.
    pub fn build_availability(&self, upgrade: Upgrade) -> BuildAvailability {
        let Some(next_level) = self.fortress.next_build_level(upgrade) else {
            return BuildAvailability::MaxLevel;
        };
        if let Some(role) = upgrade.required_role() {
            if !self.inhabitants.has_role(role) {
                return BuildAvailability::MissingRole(role);
            }
        }
        if !self.resources.can_afford(&upgrade.build_cost(next_level)) {
            return BuildAvailability::CantAfford;
        }
        BuildAvailability::Ok
    }

    /// Pay the materials and raise the building. Errs with the blocking reason.
    pub fn construct(&mut self, upgrade: Upgrade) -> Result<String, BuildAvailability> {
        match self.build_availability(upgrade) {
            BuildAvailability::Ok => {
                let level = self.fortress.next_build_level(upgrade).unwrap_or(1);
                self.resources.apply_delta(&upgrade.build_cost(level).negated());
                Ok(self.build_upgrade(upgrade))
            }
            blocked => Err(blocked),
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
                let role = crate::inhabitants::random_arrival_role(&mut self.rng);
                let mut refugee = generate_inhabitant(role, &mut self.rng);
                // The deeper the dark, the likelier a refugee is something else
                // wearing a refugee's face — a spy that bides, then betrays.
                let infiltrate_chance = (self.region.darkness - 30).max(0) / 3; // 0..~23%
                if self.rng.random_range(0..100) < infiltrate_chance {
                    refugee.traits.push(crate::inhabitants::Trait::Infiltrator);
                }
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

        // Heroes seek a name worth the road — and a fight. Renown alone draws
        // them now (no guild needed); the deeper the dark, the more come.
        let hero_cap = match self.reputation {
            r if r >= 80 => MAX_ADVENTURERS,
            r if r >= 55 => 3,
            r if r >= 35 => 2,
            _ => 1,
        };
        if self.reputation >= ADVENTURER_MIN_REPUTATION && self.adventurers.len() < hero_cap {
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
            if let Some(name) = self.tend_most_wounded(cleric_heal) {
                lines.push(format!("The cleric tends {name}. (+{cleric_heal} health)"));
            }
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
        // The Training Yard drills the guards harder with every tier.
        let yard_xp = match self.fortress.building_level(Upgrade::TrainingYard) {
            0 => 0,
            1 => 2,
            2 => 4,
            _ => 6,
        };
        if yard_xp > 0 {
            lines.extend(train_role(self, Role::Guard, Skill::Combat, yard_xp));
        }
        // A good Workshop makes crafters of everyone.
        if self.fortress.building_level(Upgrade::Workshop) >= 2 {
            for i in self.inhabitants.inhabitants.iter_mut().filter(|i| i.is_alive) {
                i.skills.train(Skill::Crafting, 1);
            }
        }
        // The commander hones their own trade by ruling, like any worker.
        if let Some(player) = &mut self.player {
            if player.is_alive() {
                player.skills.train(player.class.home_skill(), 2);
            }
        }

        // Peasants find their calling: idle hands pick up general craft, and
        // now and then take up the trade they've shown the most aptitude for.
        {
            let rng = &mut self.rng;
            let mut drifted = Vec::new();
            for i in self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.role == Role::Peasant)
            {
                i.skills.train(Skill::Crafting, 1);
                if rng.random_range(0..100) < 8 {
                    let best = Role::TRADES
                        .iter()
                        .copied()
                        .max_by_key(|r| i.skills.xp(r.home_skill()))
                        .unwrap();
                    if i.skills.xp(best.home_skill()) >= 20 {
                        i.role = best;
                        drifted.push((i.name.clone(), best));
                    }
                }
            }
            for (name, role) in drifted {
                lines.push(format!("{name} takes up the life of a {}.", role.name()));
            }
        }

        // Infiltrators bide until the dark runs strong, then strike and flee.
        if self.region.darkness >= 40 {
            let spy = self
                .inhabitants
                .inhabitants
                .iter()
                .find(|i| i.is_alive && i.has_trait(Trait::Infiltrator))
                .map(|i| i.name.clone());
            if let Some(name) = spy {
                if self.rng.random_range(0..100) < 15 {
                    self.resources.apply_delta(&ResourceDelta {
                        food: -8,
                        valuables: -4,
                        ..Default::default()
                    });
                    self.fortress.apply_morale_delta(-5);
                    self.apply_reputation_delta(-2);
                    self.inhabitants.remove(&name);
                    lines.push(format!(
                        "{name} was no refugee but a spy — stores plundered, then gone into the dark. (-5 morale)"
                    ));
                }
            }
        }

        // Craftwork: smiths forge gear at the smithy; everyone whittles tools.
        let smithy_level = self.fortress.building_level(Upgrade::Blacksmith);
        if smithy_level > 0 {
            let tier_sum: i64 = self
                .inhabitants
                .get_by_role(Role::Blacksmith)
                .iter()
                .map(|i| i.skills.tier(Skill::Smithing).index() as i64)
                .sum();
            // ×1 / ×1.5 / ×2 by smithy tier, integer math
            let forged = tier_sum * (smithy_level as i64 + 1) / 2;
            if forged > 0 && self.resources.gear < 60 {
                self.resources.apply_delta(&ResourceDelta { gear: forged, ..Default::default() });
                lines.push("The forge rings; the armory grows.".to_string());
            }
        }
        // Tool output scales with the Workshop: ×1 base, ×1.5/×2/×3 by tier.
        let whittled: i64 = self
            .inhabitants
            .get_alive()
            .iter()
            .map(|i| i.skills.tier(Skill::Crafting).index() as i64)
            .sum::<i64>()
            * match self.fortress.building_level(Upgrade::Workshop) {
                0 => 2,
                1 => 3,
                2 => 4,
                _ => 6,
            }
            / 2;
        if whittled > 0 && self.resources.tools < 60 {
            self.resources.apply_delta(&ResourceDelta { tools: whittled, ..Default::default() });
        }

        // The Lumberyard works the woods.
        let yard_wood = match self.fortress.building_level(Upgrade::Lumberyard) {
            0 => 0,
            1 => 2,
            2 => 3,
            _ => 5,
        };
        if yard_wood > 0 {
            self.resources.apply_delta(&ResourceDelta { wood: yard_wood, ..Default::default() });
        }

        // The Mine answers the one shortage you can't trade away: stone (and a
        // little raw metal for the forge).
        let mine_level = self.fortress.building_level(Upgrade::Mine);
        if mine_level > 0 {
            let stone = [0, 3, 5, 8][mine_level.min(3) as usize];
            let gear = [0, 1, 1, 2][mine_level.min(3) as usize];
            self.resources.apply_delta(&ResourceDelta { stone, gear, ..Default::default() });
        }

        // Night fires: the hold burns timber for warmth and light. A real cost
        // once the woodpile matters; nothing burns if there's nothing to burn.
        let pop = self.inhabitants.count_alive() as i64;
        let firewood = if pop > 0 { (pop / 6).max(1) } else { 0 };
        if firewood > 0 && self.resources.wood > 0 {
            let burned = firewood.min(self.resources.wood);
            self.resources.apply_delta(&ResourceDelta { wood: -burned, ..Default::default() });
        }

        let farm_level = self.fortress.building_level(Upgrade::Farm);
        if farm_level > 0 {
            let base: i64 = match farm_level {
                1 => 3,
                2 => 5,
                _ => 7,
            };
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
            let harvest = base + skill_bonus as i64 + tool_bonus;
            self.resources.apply_delta(&ResourceDelta { food: harvest, ..Default::default() });
            lines.push("The farm brings in the harvest.".to_string());
        }

        // Spoilage: grain rots beyond what the stores can keep dry — the
        // Granary is what makes a deep larder possible.
        let food_cap = match self.fortress.building_level(Upgrade::Granary) {
            0 => 50,
            1 => 60,
            2 => 90,
            _ => 130,
        };
        if self.resources.food > food_cap {
            let excess = self.resources.food - food_cap;
            self.resources.food = food_cap + excess * 3 / 4;
            lines.push("Some of the surplus grain spoils in the open.".to_string());
        }

        // The Tavern: a warm hearth and a full common room lift every heart.
        let tavern_cheer = self.fortress.building_level(Upgrade::Tavern) as i32;
        if tavern_cheer > 0 {
            self.fortress.apply_morale_delta(tavern_cheer);
            lines.push(format!("Laughter drifts from the tavern. (+{tavern_cheer} morale)"));
        }

        let infirmary_level = self.fortress.building_level(Upgrade::Infirmary);
        if infirmary_level > 0 {
            for i in self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.role == Role::Healer)
            {
                i.apply_morale(2);
            }
        }

        // Medicine: the healers tend the worst-off; a great Infirmary heals
        // deeper (tier II) and has beds for a second patient (tier III).
        let mut healing: i32 = self
            .inhabitants
            .get_by_role(Role::Healer)
            .iter()
            .map(|i| 2 * i.skills.tier(Skill::Medicine).index() as i32)
            .sum();
        if healing > 0 && infirmary_level >= 2 {
            healing += 2;
        }
        if healing > 0 {
            let patients = if infirmary_level >= 3 { 2 } else { 1 };
            for _ in 0..patients {
                if let Some(name) = self.tend_most_wounded(healing) {
                    lines.push(format!("The healers tend {name}. (+{healing} health)"));
                }
            }
        }

        // Food upkeep: 1 per 2 mouths; the commander eats too. Iron Rations -1.
        let alive = self.inhabitants.count_alive() as i64;
        let commander = i64::from(self.player.is_some());
        let mouths = alive + commander;
        if mouths > 0 {
            let upkeep = (mouths + 1) / 2;
            if self.resources.food >= upkeep {
                self.resources.apply_delta(&ResourceDelta { food: -upkeep, ..Default::default() });
            } else {
                self.resources.food = 0;
                self.fortress.apply_morale_delta(-5);
                lines.push("Not enough food! The people go hungry. (-5 morale)".to_string());
            }
        }

        // Sleep quality: enough beds lift spirits; the overflow sleeps rough.
        // The commander always takes the first Keep bed — it is their keep —
        // so the rough nights fall on the inhabitants.
        if mouths > 0 {
            let beds = self.fortress.sleeping_capacity() as i64;
            if mouths <= beds {
                self.fortress.apply_morale_delta(1);
                lines.push("Everyone sleeps warm tonight. (+1 morale)".to_string());
            } else {
                let beds_for_inhabitants = (beds - commander).max(0);
                let rough = alive - beds_for_inhabitants;
                for i in self
                    .inhabitants
                    .inhabitants
                    .iter_mut()
                    .filter(|i| i.is_alive)
                    .skip(beds_for_inhabitants as usize)
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

    /// Tend the single most-wounded soul (the commander included) by `amount`.
    /// The commander is tended only when more hurt than any inhabitant.
    /// Returns the name of whoever was healed, or None if all are hale.
    fn tend_most_wounded(&mut self, amount: i32) -> Option<String> {
        let worst_inhab = self
            .inhabitants
            .inhabitants
            .iter()
            .filter(|i| i.is_alive && i.health < 100)
            .map(|i| i.health)
            .min();
        let cmd_health = self
            .player
            .as_ref()
            .filter(|p| p.is_alive() && p.health < 100)
            .map(|p| p.health);
        let tend_commander = match (cmd_health, worst_inhab) {
            (Some(c), Some(w)) => c < w,
            (Some(_), None) => true,
            _ => false,
        };
        if tend_commander {
            let player = self.player.as_mut()?;
            player.heal(amount);
            Some(player.name.clone())
        } else {
            let patient = self
                .inhabitants
                .inhabitants
                .iter_mut()
                .filter(|i| i.is_alive && i.health < 100)
                .min_by_key(|i| i.health)?;
            patient.heal(amount);
            Some(patient.name.clone())
        }
    }

    // ------------------------------------------------------------------
    // Win / loss — no victory condition, the fortress always eventually falls
    // ------------------------------------------------------------------

    pub fn is_game_over(&self) -> bool {
        self.fortress.is_defeated() || self.commander_has_fallen()
    }

    /// The realm falls with its commander: health at zero ends the run.
    pub fn commander_has_fallen(&self) -> bool {
        self.player.as_ref().is_some_and(|p| !p.is_alive())
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
