use serde::{Deserialize, Serialize};

use crate::inhabitants::Role;
use crate::items::ItemKind;
use crate::resources::ResourceDelta;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Upgrade {
    /// The heart of the hold — standing from the founding at level 1, raised
    /// through tiers for more beds, defense, and a higher population cap.
    Keep,
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
    Market,
    Alchemist,
    Library,
}

/// Buildings rise once and are then raised through tiers (I → II → III).
/// Housing is the exception: plain roofs, built plot by plot, never tiered.
pub const MAX_BUILDING_LEVEL: u8 = 3;
pub const HOUSING_PLOTS: usize = 4;

impl Upgrade {
    pub const ALL: [Upgrade; 19] = [
        Upgrade::Keep,
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
        Upgrade::Market,
        Upgrade::Alchemist,
        Upgrade::Library,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Upgrade::Keep => "Keep",
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
            Upgrade::Market => "Market",
            Upgrade::Alchemist => "Alchemist",
            Upgrade::Library => "Library",
        }
    }

    /// Materials to raise this building at `level` (1 = first build).
    /// Tier costs grow ~×1.6 per level; housing always pays the base price.
    pub fn build_cost(&self, level: u8) -> ResourceDelta {
        let (food, wood, stone) = match self {
            // The grandest works of the hold — only ever paid for at tier II/III,
            // since level I stands from the founding.
            Upgrade::Keep => (0, 20, 20),
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
            Upgrade::Market => (0, 10, 10),
            Upgrade::Alchemist => (0, 8, 12),
            Upgrade::Library => (0, 15, 10),
        };
        let (num, den): (i64, i64) = match level {
            0 | 1 => (1, 1),
            2 => (8, 5),
            _ => (64, 25),
        };
        let scale = |v: i64| if v == 0 { 0 } else { (v * num + den - 1) / den };
        ResourceDelta { food: scale(food), wood: scale(wood), stone: scale(stone), ..Default::default() }
    }

    /// Worker-days of labor to raise this building at `level`. A project draws
    /// this down by the hold's available workforce each day until it completes.
    pub fn build_worker_days(&self, level: u8) -> i32 {
        let base = match self {
            Upgrade::Housing | Upgrade::Farm | Upgrade::Graveyard => 3,
            Upgrade::Watchtower | Upgrade::Lumberyard | Upgrade::Tavern | Upgrade::Market => 4,
            Upgrade::Barracks | Upgrade::Mine | Upgrade::WizardTower | Upgrade::Alchemist | Upgrade::Library => 6,
            Upgrade::Keep => 8, // the great work — long in the raising
            _ => 5,
        };
        base + (level.saturating_sub(1) as i32) * 2
    }

    /// Specialist who must live here before the building can go up.
    pub fn required_role(&self) -> Option<Role> {
        match self {
            Upgrade::Farm | Upgrade::Lumberyard => Some(Role::Farmer),
            Upgrade::Infirmary | Upgrade::Shrine => Some(Role::Healer),
            Upgrade::Blacksmith => Some(Role::Blacksmith),
            Upgrade::Barracks | Upgrade::TrainingYard => Some(Role::Guard),
            Upgrade::Alchemist => Some(Role::Herbalist),
            Upgrade::Library => Some(Role::Scholar),
            Upgrade::Keep
            | Upgrade::Watchtower
            | Upgrade::Granary
            | Upgrade::Housing
            | Upgrade::Tavern
            | Upgrade::Workshop
            | Upgrade::Mine
            | Upgrade::Graveyard
            | Upgrade::Market
            | Upgrade::WizardTower => None,
        }
    }

    /// A one-line summary of what this building does at a given level — used by
    /// the build menu and inspect panel. Level 0 describes the first tier.
    pub fn effect_summary(&self, level: u8) -> String {
        let lvl = level.max(1);
        match self {
            Upgrade::Keep => format!(
                "+{} beds, +{} defense, +{} max pop",
                [6, 10, 14][(lvl - 1).min(2) as usize],
                [0, 6, 12][(lvl - 1).min(2) as usize],
                [0, 6, 14][(lvl - 1).min(2) as usize],
            ),
            Upgrade::Watchtower => format!("+{} defense", [5, 8, 12][(lvl - 1).min(2) as usize]),
            Upgrade::Farm => format!("+{} food/day", [3, 5, 7][(lvl - 1).min(2) as usize]),
            Upgrade::Infirmary => "heals the wounded; disasters hit softer".to_string(),
            Upgrade::Blacksmith => "forges arms & armor from ore".to_string(),
            Upgrade::Granary => format!("food cap {}", [60, 90, 130][(lvl - 1).min(2) as usize]),
            Upgrade::Barracks => "bunks for many guards; +defense".to_string(),
            Upgrade::Housing => "+5 beds, +5 max population".to_string(),
            Upgrade::Tavern => format!("+{} morale/day", lvl.min(3)),
            Upgrade::Workshop => "trains crafting at II+".to_string(),
            Upgrade::Lumberyard => format!("+{} wood/day", [2, 3, 5][(lvl - 1).min(2) as usize]),
            Upgrade::Shrine => "softens demon dread".to_string(),
            Upgrade::TrainingYard => "drills the guard's combat".to_string(),
            Upgrade::Mine => format!("+{} stone/day", [3, 5, 8][(lvl - 1).min(2) as usize]),
            Upgrade::Graveyard => "honors the dead; eases grief".to_string(),
            Upgrade::WizardTower => "a seat for magic; enchanting".to_string(),
            Upgrade::Market => "trades excess resources for valuables".to_string(),
            Upgrade::Alchemist => "brews helpful potions, heals the sick".to_string(),
            Upgrade::Library => "trains scholars and speeds magic".to_string(),
        }
    }
}

/// How far the hold has grown from a huddle of huts toward a true city. A
/// scaffold for the fortress→town→city arc: each tier lifts the population cap
/// and (later passes) will unlock districts and town-scale systems.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
#[serde(rename_all = "lowercase")]
pub enum SettlementTier {
    #[default]
    Hamlet,
    Village,
    Town,
    City,
}

impl SettlementTier {
    pub const ALL: [SettlementTier; 4] =
        [SettlementTier::Hamlet, SettlementTier::Village, SettlementTier::Town, SettlementTier::City];

    pub fn name(&self) -> &'static str {
        match self {
            SettlementTier::Hamlet => "Hamlet",
            SettlementTier::Village => "Village",
            SettlementTier::Town => "Town",
            SettlementTier::City => "City",
        }
    }

    /// The baseline population a settlement of this tier supports.
    pub fn base_population(&self) -> u32 {
        match self {
            SettlementTier::Hamlet => 20,
            SettlementTier::Village => 35,
            SettlementTier::Town => 60,
            SettlementTier::City => 100,
        }
    }

    /// How many standing buildings a hold needs before it can grow to this tier.
    fn buildings_required(&self) -> usize {
        match self {
            SettlementTier::Hamlet => 0,
            SettlementTier::Village => 3,
            SettlementTier::Town => 6,
            SettlementTier::City => 10,
        }
    }

    pub fn next(&self) -> Option<SettlementTier> {
        match self {
            SettlementTier::Hamlet => Some(SettlementTier::Village),
            SettlementTier::Village => Some(SettlementTier::Town),
            SettlementTier::Town => Some(SettlementTier::City),
            SettlementTier::City => None,
        }
    }
}

/// A rare, permanent boon a hold earns at most once a run — by surviving a
/// calamity or making a true name for itself (crossing a renown threshold).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FortressFeature {
    /// Sheer stone walls — a standing defense bonus.
    Ramparts,
    /// Cold, dry vaults — the larder keeps far more grain.
    DeepCellars,
    /// A great central hearth — the hold burns less timber against the cold.
    GreatHearth,
    /// A peerless anvil — the forge turns out finer work.
    MasterForge,
}

impl FortressFeature {
    pub const ALL: [FortressFeature; 4] = [
        FortressFeature::Ramparts,
        FortressFeature::DeepCellars,
        FortressFeature::GreatHearth,
        FortressFeature::MasterForge,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            FortressFeature::Ramparts => "the Ramparts",
            FortressFeature::DeepCellars => "the Deep Cellars",
            FortressFeature::GreatHearth => "the Great Hearth",
            FortressFeature::MasterForge => "the Master Forge",
        }
    }

    pub fn blurb(&self) -> &'static str {
        match self {
            FortressFeature::Ramparts => "Sheer stone walls stand against any assault.",
            FortressFeature::DeepCellars => "Cold vaults keep the larder full through any winter.",
            FortressFeature::GreatHearth => "A great hearth warms the whole hold for little fuel.",
            FortressFeature::MasterForge => "A peerless anvil; the smith's work is the finer for it.",
        }
    }
}

/// A build or upgrade in the build queue. Orders are worked strictly in queue
/// order: only the front project draws labor, and only once `funded`. An
/// unfunded project owes `materials_owed` — paid in full the first day it sits
/// at the front and the hold can afford it (see `GameState::try_fund_front`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildProject {
    pub upgrade: Upgrade,
    pub target_level: u8,
    pub worker_days_remaining: i32,
    /// Materials paid: until then the order draws no labor, only waits its turn.
    #[serde(default)]
    pub funded: bool,
    /// What the order still owes the stores before work can begin.
    #[serde(default)]
    pub materials_owed: ResourceDelta,
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
    /// What the forge concentrates on when ore is worked into items.
    #[serde(default = "default_craft_focus")]
    pub craft_focus: ItemKind,
    /// How far the hold has grown — hamlet → village → town → city.
    #[serde(default)]
    pub settlement_tier: SettlementTier,
    /// Builds underway — materials paid, labor still owed. The front of the
    /// queue is the one the workforce is on; the rest wait their turn.
    #[serde(default)]
    pub projects: Vec<BuildProject>,
    /// Rare permanent boons; at most one per run (see `FortressFeature`).
    #[serde(default)]
    pub features: Vec<FortressFeature>,
}

fn default_craft_focus() -> ItemKind {
    ItemKind::Weapon
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
            craft_focus: default_craft_focus(),
            settlement_tier: SettlementTier::Hamlet,
            projects: Vec::new(),
            features: Vec::new(),
        }
    }

    pub fn has_feature(&self, feature: FortressFeature) -> bool {
        self.features.contains(&feature)
    }

    /// A project (build or upgrade) of this kind is already underway.
    pub fn has_project(&self, kind: Upgrade) -> bool {
        self.projects.iter().any(|p| p.upgrade == kind)
    }

    /// Add a build/upgrade to the back of the queue, owing `cost` in materials.
    /// Nothing is paid now — the order is funded the first day it reaches the
    /// front and the hold can afford it (see `GameState::try_fund_front`).
    pub fn enqueue_project(&mut self, kind: Upgrade, target_level: u8, cost: ResourceDelta) {
        self.projects.push(BuildProject {
            upgrade: kind,
            target_level,
            worker_days_remaining: kind.build_worker_days(target_level),
            funded: false,
            materials_owed: cost,
        });
    }

    /// Put a day's `workforce` into the front project, but only if it's funded.
    /// Returns the upgrades that completed today (usually none or one), to be
    /// applied by the caller. Strict FIFO: nothing behind the front advances.
    pub fn advance_projects(&mut self, workforce: i32) -> Vec<Upgrade> {
        let mut done = Vec::new();
        if let Some(front) = self.projects.first() {
            if front.funded {
                let project = &mut self.projects[0];
                project.worker_days_remaining -= workforce.max(1);
                if project.worker_days_remaining <= 0 {
                    done.push(project.upgrade);
                    self.projects.remove(0);
                }
            }
        }
        done
    }

    /// Move the queued project at `idx` one place toward the front (`up`) or the
    /// back. Returns whether it moved (it can't past either end).
    pub fn move_project(&mut self, idx: usize, up: bool) -> bool {
        if up {
            if idx == 0 || idx >= self.projects.len() {
                return false;
            }
            self.projects.swap(idx, idx - 1);
        } else {
            if idx + 1 >= self.projects.len() {
                return false;
            }
            self.projects.swap(idx, idx + 1);
        }
        true
    }

    /// Grow to the next settlement tier when the hold is crowded *and* built up
    /// enough to sustain it — raising the population cap by the tier's step.
    /// Returns the new tier if it grew. (Town groundwork; gameplay grows later.)
    pub fn try_promote(&mut self, alive: usize) -> Option<SettlementTier> {
        let next = self.settlement_tier.next()?;
        let crowded = (alive as u32) * 5 >= self.max_population * 4; // ≥80% full
        let built_up = self.buildings.len() >= next.buildings_required();
        if crowded && built_up {
            let step = next.base_population() - self.settlement_tier.base_population();
            self.max_population += step;
            self.settlement_tier = next;
            Some(next)
        } else {
            None
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
        // The Keep stands at level I from the founding without a stored entry;
        // the first "build" is really the raising to tier II.
        if kind == Upgrade::Keep && !self.has_upgrade(Upgrade::Keep) {
            self.buildings.push(Building { kind, level: 2 });
            return BuildOutcome::Upgraded(2);
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

    /// The Keep's tier — level I from the founding even with no stored entry.
    pub fn keep_level(&self) -> u8 {
        self.building_level(Upgrade::Keep).max(1)
    }

    /// The level a fresh build/upgrade would reach, or None when nothing
    /// more can rise (tier III, or all housing plots taken).
    pub fn next_build_level(&self, kind: Upgrade) -> Option<u8> {
        if kind == Upgrade::Housing {
            return (self.housing_units() < HOUSING_PLOTS).then_some(1);
        }
        if kind == Upgrade::Keep {
            // Level I stands already; the next build is II, then III.
            let lvl = self.keep_level();
            return (lvl < MAX_BUILDING_LEVEL).then_some(lvl + 1);
        }
        match self.building_level(kind) {
            0 => Some(1),
            l if l < MAX_BUILDING_LEVEL => Some(l + 1),
            _ => None,
        }
    }

    /// Beds available: the Keep sleeps more as it is raised (6/10/14 by tier);
    /// the Barracks bunks grow with its tier; every housing plot shelters 6.
    /// Overflow sleeps rough.
    pub fn sleeping_capacity(&self) -> u32 {
        // The Keep's own beds grow with its tier.
        let mut beds = [6, 10, 14][(self.keep_level() - 1).min(2) as usize];
        // The Barracks is built for numbers — plain bunks, but it sleeps a crowd.
        beds += match self.building_level(Upgrade::Barracks) {
            0 => 0,
            1 => 10,
            2 => 16,
            _ => 24,
        };
        beds += self.housing_units() as u32 * 6;
        // Every other standing building keeps a few cots for its workers (the
        // Keep's own beds are already counted above).
        let workshops = self
            .buildings
            .iter()
            .filter(|b| {
                b.kind != Upgrade::Barracks && b.kind != Upgrade::Housing && b.kind != Upgrade::Keep
            })
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
