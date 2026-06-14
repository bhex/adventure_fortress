use serde::{Deserialize, Serialize};

/// Stocks are simulated as fine-grained numbers but presented only as
/// adjective bands — the steward of a fortress knows "the larder is lean",
/// not "we have 23 food".
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum StockBand {
    Exhausted,
    Scarce,
    Lean,
    Adequate,
    Comfortable,
    Plentiful,
}

impl StockBand {
    pub fn name(&self) -> &'static str {
        match self {
            StockBand::Exhausted => "exhausted",
            StockBand::Scarce => "scarce",
            StockBand::Lean => "lean",
            StockBand::Adequate => "adequate",
            StockBand::Comfortable => "comfortable",
            StockBand::Plentiful => "plentiful",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Food,
    Valuables,
    Stone,
    Wood,
    Gear,
    Tools,
    /// Raw metal from the Mine — the smith's feedstock for forging items.
    Ore,
    /// Demon/portal residue, dropped by demon battles — the only stuff that
    /// will hold an enchantment at the Wizard Tower. Rare by nature.
    Residue,
}

impl ResourceKind {
    pub const ALL: [ResourceKind; 8] = [
        ResourceKind::Food,
        ResourceKind::Valuables,
        ResourceKind::Stone,
        ResourceKind::Wood,
        ResourceKind::Gear,
        ResourceKind::Tools,
        ResourceKind::Ore,
        ResourceKind::Residue,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            ResourceKind::Food => "food",
            ResourceKind::Valuables => "valuables",
            ResourceKind::Stone => "stone",
            ResourceKind::Wood => "timber",
            ResourceKind::Gear => "gear",
            ResourceKind::Tools => "tools",
            ResourceKind::Ore => "ore",
            ResourceKind::Residue => "residue",
        }
    }

    /// Band thresholds: [scarce, lean, adequate, comfortable, plentiful) lower bounds.
    fn thresholds(&self) -> [i64; 5] {
        match self {
            ResourceKind::Food => [1, 16, 31, 61, 101],
            ResourceKind::Valuables => [1, 6, 13, 26, 51],
            ResourceKind::Stone | ResourceKind::Wood => [1, 11, 26, 51, 91],
            ResourceKind::Gear | ResourceKind::Tools | ResourceKind::Ore => [1, 6, 13, 26, 51],
            ResourceKind::Residue => [1, 3, 6, 11, 21],
        }
    }
}

pub fn band_for(kind: ResourceKind, value: i64) -> StockBand {
    let t = kind.thresholds();
    if value >= t[4] {
        StockBand::Plentiful
    } else if value >= t[3] {
        StockBand::Comfortable
    } else if value >= t[2] {
        StockBand::Adequate
    } else if value >= t[1] {
        StockBand::Lean
    } else if value >= t[0] {
        StockBand::Scarce
    } else {
        StockBand::Exhausted
    }
}

/// Soft quantity words for event text — never raw numbers.
pub fn amount_phrase(v: i64) -> &'static str {
    match v.abs() {
        0 => "none",
        1..=5 => "a little",
        6..=15 => "some",
        16..=30 => "much",
        _ => "a great deal of",
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Resources {
    pub food: i64,
    #[serde(alias = "gold")]
    pub valuables: i64,
    pub stone: i64,
    pub wood: i64,
    #[serde(default)]
    pub gear: i64,
    #[serde(default)]
    pub tools: i64,
    #[serde(default)]
    pub ore: i64,
    #[serde(default)]
    pub residue: i64,
}

/// Matches JSON shapes like {"food": 5, "valuables": -10} — used for effects,
/// choice costs, and event min_resource gates. "gold" is accepted as a legacy
/// alias for valuables.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ResourceDelta {
    #[serde(default)]
    pub food: i64,
    #[serde(default, alias = "gold")]
    pub valuables: i64,
    #[serde(default)]
    pub stone: i64,
    #[serde(default)]
    pub wood: i64,
    #[serde(default)]
    pub gear: i64,
    #[serde(default)]
    pub tools: i64,
    #[serde(default)]
    pub ore: i64,
    #[serde(default)]
    pub residue: i64,
}

impl ResourceDelta {
    fn fields(&self) -> [(ResourceKind, i64); 8] {
        [
            (ResourceKind::Food, self.food),
            (ResourceKind::Valuables, self.valuables),
            (ResourceKind::Stone, self.stone),
            (ResourceKind::Wood, self.wood),
            (ResourceKind::Gear, self.gear),
            (ResourceKind::Tools, self.tools),
            (ResourceKind::Ore, self.ore),
            (ResourceKind::Residue, self.residue),
        ]
    }

    pub fn is_zero(&self) -> bool {
        self.fields().iter().all(|(_, v)| *v == 0)
    }

    pub fn negated(&self) -> ResourceDelta {
        ResourceDelta {
            food: -self.food,
            valuables: -self.valuables,
            stone: -self.stone,
            wood: -self.wood,
            gear: -self.gear,
            tools: -self.tools,
            ore: -self.ore,
            residue: -self.residue,
        }
    }

    /// Soft narration of an effect: "Gained some food; lost a little timber".
    pub fn describe(&self) -> String {
        let mut gains = Vec::new();
        let mut losses = Vec::new();
        for (kind, v) in self.fields() {
            if v > 0 {
                gains.push(format!("{} {}", amount_phrase(v), kind.name()));
            } else if v < 0 {
                losses.push(format!("{} {}", amount_phrase(v), kind.name()));
            }
        }
        match (gains.is_empty(), losses.is_empty()) {
            (false, false) => format!("Gained {}; lost {}", gains.join(", "), losses.join(", ")),
            (false, true) => format!("Gained {}", gains.join(", ")),
            (true, false) => format!("Lost {}", losses.join(", ")),
            (true, true) => String::new(),
        }
    }

    /// Soft cost listing: "some food, a little timber".
    pub fn describe_cost(&self) -> String {
        let mut parts = Vec::new();
        for (kind, v) in self.fields() {
            if v != 0 {
                parts.push(format!("{} {}", amount_phrase(v), kind.name()));
            }
        }
        parts.join(", ")
    }
}

impl Resources {
    pub fn get(&self, kind: ResourceKind) -> i64 {
        match kind {
            ResourceKind::Food => self.food,
            ResourceKind::Valuables => self.valuables,
            ResourceKind::Stone => self.stone,
            ResourceKind::Wood => self.wood,
            ResourceKind::Gear => self.gear,
            ResourceKind::Tools => self.tools,
            ResourceKind::Ore => self.ore,
            ResourceKind::Residue => self.residue,
        }
    }

    pub fn band(&self, kind: ResourceKind) -> StockBand {
        band_for(kind, self.get(kind))
    }

    pub fn apply_delta(&mut self, delta: &ResourceDelta) {
        self.food += delta.food;
        self.valuables += delta.valuables;
        self.stone += delta.stone;
        self.wood += delta.wood;
        self.gear += delta.gear;
        self.tools += delta.tools;
        self.ore += delta.ore;
        self.residue += delta.residue;
        self.clamp();
    }

    pub fn can_afford(&self, cost: &ResourceDelta) -> bool {
        self.food >= cost.food
            && self.valuables >= cost.valuables
            && self.stone >= cost.stone
            && self.wood >= cost.wood
            && self.gear >= cost.gear
            && self.tools >= cost.tools
            && self.ore >= cost.ore
            && self.residue >= cost.residue
    }

    pub fn clamp(&mut self) {
        self.food = self.food.max(0);
        self.valuables = self.valuables.max(0);
        self.stone = self.stone.max(0);
        self.wood = self.wood.max(0);
        self.gear = self.gear.max(0);
        self.tools = self.tools.max(0);
        self.ore = self.ore.max(0);
        self.residue = self.residue.max(0);
    }
}
