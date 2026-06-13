use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::fortress::Upgrade;
use crate::inhabitants::Role;
use crate::player::StatKind;
use crate::resources::ResourceDelta;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", content = "params", rename_all = "snake_case")]
pub enum Effect {
    Resource(ResourceDelta),
    Morale { amount: i32 },
    Defense { amount: i32 },
    SpawnInhabitant {
        #[serde(default)]
        role: Option<Role>,
    },
    KillInhabitant {
        #[serde(default)]
        role: Option<Role>,
    },
    RemoveInhabitant {},
    ApplyToRole {
        role: Role,
        #[serde(default)]
        health: i32,
        #[serde(default)]
        morale: i32,
    },
    AddUpgrade { name: Upgrade },
    /// A pitched fight: the garrison musters against a foe of `power` and the
    /// clash is resolved as a narrated battle report (see `battle.rs`).
    Battle {
        power: i32,
        #[serde(default)]
        loot_valuables: i64,
    },
    /// Push back (or feed) the regional darkness war: tweak darkness directly,
    /// bolster a random surviving site, or shift the portal pressure.
    Region {
        #[serde(default)]
        darkness: i32,
        #[serde(default)]
        site_strength: i32,
        #[serde(default)]
        pressure: i32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatCheck {
    pub stat: StatKind,
    pub difficulty: i32,
    #[serde(default)]
    pub success_effects: Vec<Effect>,
    #[serde(default)]
    pub failure_effects: Vec<Effect>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Choice {
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub cost: ResourceDelta,
    #[serde(default)]
    pub requires_stat: HashMap<StatKind, u8>,
    #[serde(default)]
    pub stat_check: Option<StatCheck>,
    #[serde(default)]
    pub flavor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub name: String,
    pub description: String,
    pub choices: Vec<Choice>,
    #[serde(default = "default_min_day")]
    pub min_day: u32,
    #[serde(default)]
    pub max_day: Option<u32>,
    #[serde(default)]
    pub min_morale: i32,
    #[serde(default = "default_max_morale")]
    pub max_morale: i32,
    #[serde(default)]
    pub min_resource: ResourceDelta,
    #[serde(default)]
    pub requires_role: Option<Role>,
    #[serde(default)]
    pub requires_upgrade: Option<Upgrade>,
    /// Darkness gates: demon events key off the regional darkness, not the day.
    #[serde(default)]
    pub min_darkness: Option<i32>,
    #[serde(default)]
    pub max_darkness: Option<i32>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// Auto events resolve without asking the player — a single foregone
    /// choice, applied straight to the log. Must have exactly one choice.
    #[serde(default)]
    pub auto: bool,
}

fn default_min_day() -> u32 {
    1
}

fn default_max_morale() -> i32 {
    100
}

fn default_weight() -> f64 {
    1.0
}

impl Event {
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventResult {
    pub event_name: String,
    pub choice_label: String,
    pub lines: Vec<String>,
}
