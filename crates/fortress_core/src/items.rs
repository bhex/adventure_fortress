//! Equipment: typed, quality-graded, enchantable item objects held in the
//! fortress armory. Unlike the bulk `gear`/`tools` stockpiles (which back the
//! garrison's general readiness), items are individual things — a fine sword,
//! a hexed amulet, a masterwork hauberk — that the best hands take up first.
//!
//! Nothing here is assigned by hand. Every day the most capable fighters and
//! workers auto-equip the best item for their need; combat and work read the
//! armory's top items directly (`equip_rating`). Items wear with use and must
//! be kept up at the forge or they break. Artifacts are the exception: rare,
//! powerful, and beyond ordinary wear.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Weapon,
    Armor,
    Tool,
}

impl ItemKind {
    pub const ALL: [ItemKind; 3] = [ItemKind::Weapon, ItemKind::Armor, ItemKind::Tool];

    pub fn name(&self) -> &'static str {
        match self {
            ItemKind::Weapon => "weapon",
            ItemKind::Armor => "armor",
            ItemKind::Tool => "tool",
        }
    }

    /// A plain noun for narration, scaling a little with quality is the item's job.
    pub fn noun(&self) -> &'static str {
        match self {
            ItemKind::Weapon => "blade",
            ItemKind::Armor => "harness",
            ItemKind::Tool => "tool",
        }
    }
}

/// How well-made a thing is. The index drives every rating; a masterwork is
/// worth roughly four crude ones.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Crude,
    Plain,
    Fine,
    Masterwork,
}

impl Quality {
    pub const ALL: [Quality; 4] =
        [Quality::Crude, Quality::Plain, Quality::Fine, Quality::Masterwork];

    pub fn name(&self) -> &'static str {
        match self {
            Quality::Crude => "crude",
            Quality::Plain => "plain",
            Quality::Fine => "fine",
            Quality::Masterwork => "masterwork",
        }
    }

    /// 0 (crude) .. 3 (masterwork).
    pub fn index(&self) -> i32 {
        Quality::ALL.iter().position(|q| q == self).unwrap() as i32
    }

    /// The quality a smith of this skill tier (0..7) usually turns out.
    pub fn from_smith_tier(tier: u32) -> Quality {
        match tier {
            0 | 1 => Quality::Crude,
            2 | 3 => Quality::Plain,
            4 | 5 => Quality::Fine,
            _ => Quality::Masterwork,
        }
    }
}

/// A worked-in magical property. Most help; the Hexed curse hurts and rides in
/// on artifacts you can't simply discard.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Enchant {
    /// Weapon: bites deeper.
    Keen,
    /// Armor: turns more blows.
    Guarding,
    /// Tool: never tires, never dulls.
    Tireless,
    /// A cursed thing: drags on whoever bears it.
    Hexed,
}

impl Enchant {
    pub fn name(&self) -> &'static str {
        match self {
            Enchant::Keen => "keen",
            Enchant::Guarding => "guarding",
            Enchant::Tireless => "tireless",
            Enchant::Hexed => "hexed",
        }
    }

    /// The enchant that best suits a freshly-made item of this kind.
    pub fn for_kind(kind: ItemKind) -> Enchant {
        match kind {
            ItemKind::Weapon => Enchant::Keen,
            ItemKind::Armor => Enchant::Guarding,
            ItemKind::Tool => Enchant::Tireless,
        }
    }

    /// Flat bonus (or penalty) added to the item's rating.
    pub fn rating_delta(&self) -> i32 {
        match self {
            Enchant::Keen | Enchant::Guarding | Enchant::Tireless => 2,
            Enchant::Hexed => -2,
        }
    }
}

/// Condition at which an ordinary item finally breaks.
const BROKEN: i32 = 0;
const FULL_CONDITION: i32 = 100;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: ItemKind,
    pub quality: Quality,
    #[serde(default)]
    pub enchant: Option<Enchant>,
    #[serde(default = "full_condition")]
    pub condition: i32,
    /// Artifacts are rare, named, and do not wear out.
    #[serde(default)]
    pub artifact: bool,
    /// A proper name for artifacts; ordinary items go unnamed.
    #[serde(default)]
    pub name: Option<String>,
}

fn full_condition() -> i32 {
    FULL_CONDITION
}

impl Item {
    pub fn new(kind: ItemKind, quality: Quality) -> Item {
        Item { kind, quality, enchant: None, condition: FULL_CONDITION, artifact: false, name: None }
    }

    pub fn enchanted(kind: ItemKind, quality: Quality, enchant: Enchant) -> Item {
        Item { enchant: Some(enchant), ..Item::new(kind, quality) }
    }

    /// What this item is worth to whoever carries it: quality plus enchant,
    /// dulled when it's worn down past half condition. Always at least 1 for a
    /// whole item so even a crude blade beats bare hands.
    pub fn rating(&self) -> i32 {
        let mut r = self.quality.index() + 1;
        if let Some(e) = self.enchant {
            r += e.rating_delta();
        }
        // A worn item helps less; a wreck (but not yet broken) barely at all.
        if !self.artifact && self.condition <= FULL_CONDITION / 2 {
            r -= 1;
        }
        r.max(if self.is_broken() { 0 } else { 1 })
    }

    pub fn is_broken(&self) -> bool {
        !self.artifact && self.condition <= BROKEN
    }

    /// Wear from a day's use. Artifacts never degrade.
    pub fn degrade(&mut self, amount: i32) {
        if !self.artifact {
            self.condition -= amount;
        }
    }

    pub fn repair(&mut self, amount: i32) {
        self.condition = (self.condition + amount).min(FULL_CONDITION);
    }

    /// "masterwork keen blade", "the Crown of Vell" — for the log and inspect.
    pub fn label(&self) -> String {
        if let Some(name) = &self.name {
            return name.clone();
        }
        let mut s = String::from(self.quality.name());
        if let Some(e) = self.enchant {
            s.push(' ');
            s.push_str(e.name());
        }
        s.push(' ');
        s.push_str(self.kind.noun());
        s
    }
}

/// What one soul carries: at most a weapon, a suit of armor, and a tool. Filled
/// each day by the fortress's auto-equip pass (`GameState::redistribute_equipment`)
/// — the ablest fighters take the best arms, the workers the best tools — and the
/// items here wear with their bearer's use.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct Loadout {
    #[serde(default)]
    pub weapon: Option<Item>,
    #[serde(default)]
    pub armor: Option<Item>,
    #[serde(default)]
    pub tool: Option<Item>,
}

impl Loadout {
    fn slot_mut(&mut self, kind: ItemKind) -> &mut Option<Item> {
        match kind {
            ItemKind::Weapon => &mut self.weapon,
            ItemKind::Armor => &mut self.armor,
            ItemKind::Tool => &mut self.tool,
        }
    }

    pub fn get(&self, kind: ItemKind) -> Option<&Item> {
        match kind {
            ItemKind::Weapon => self.weapon.as_ref(),
            ItemKind::Armor => self.armor.as_ref(),
            ItemKind::Tool => self.tool.as_ref(),
        }
    }

    /// The rating of the item in the given slot, or 0 if empty.
    pub fn rating(&self, kind: ItemKind) -> i32 {
        self.get(kind).map(|i| i.rating()).unwrap_or(0)
    }

    /// Take up an item, displacing whatever shared its slot (the caller pools
    /// the return for the rest of the redistribution).
    pub fn equip(&mut self, item: Item) -> Option<Item> {
        self.slot_mut(item.kind).replace(item)
    }

    /// Empty every slot into a `Vec` — the start of the daily redistribution.
    pub fn drain(&mut self) -> Vec<Item> {
        [self.weapon.take(), self.armor.take(), self.tool.take()].into_iter().flatten().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Item> {
        [self.weapon.as_ref(), self.armor.as_ref(), self.tool.as_ref()].into_iter().flatten()
    }

    /// A day's wear on the items in hand. Returns the labels of any that broke
    /// (their slots are cleared so the bearer re-equips next pass).
    pub fn degrade_in_use(&mut self, amount: i32) -> Vec<String> {
        let mut broken = Vec::new();
        for slot in [&mut self.weapon, &mut self.armor, &mut self.tool] {
            if let Some(item) = slot.as_mut() {
                item.degrade(amount);
                if item.is_broken() {
                    broken.push(item.label());
                    *slot = None;
                }
            }
        }
        broken
    }

    pub fn repair_all(&mut self, points: i32) {
        for item in [&mut self.weapon, &mut self.armor, &mut self.tool].into_iter().flatten() {
            item.repair(points);
        }
    }
}

/// The fortress armory: every item not yet broken or lost. Order is insertion
/// order; rankings are computed on demand so saves stay deterministic.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ItemStock {
    pub items: Vec<Item>,
}

impl ItemStock {
    pub fn add(&mut self, item: Item) {
        self.items.push(item);
    }

    pub fn count(&self) -> usize {
        self.items.len()
    }

    pub fn count_kind(&self, kind: ItemKind) -> usize {
        self.items.iter().filter(|i| i.kind == kind).count()
    }

    /// Items of a kind, best first — a stable sort so equal ratings keep their
    /// insertion order and runs stay deterministic.
    pub fn ranked(&self, kind: ItemKind) -> Vec<&Item> {
        let mut v: Vec<&Item> = self.items.iter().filter(|i| i.kind == kind).collect();
        v.sort_by_key(|i| std::cmp::Reverse(i.rating()));
        v
    }

    pub fn best_rating(&self, kind: ItemKind) -> i32 {
        self.items.iter().filter(|i| i.kind == kind).map(|i| i.rating()).max().unwrap_or(0)
    }

    /// The combined readiness the best `slots` items of a kind lend — the top
    /// hands each take the best thing for their need (auto-equip). With more
    /// hands than items, the surplus go bare; with more items than hands, only
    /// the best are carried.
    pub fn equip_rating(&self, kind: ItemKind, slots: usize) -> i32 {
        if slots == 0 {
            return 0;
        }
        self.ranked(kind).iter().take(slots).map(|i| i.rating()).sum()
    }

    /// The single best item that could still take an enchantment (not artifact,
    /// not already enchanted). Used by the Wizard Tower.
    pub fn best_unenchanted_index(&self) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.enchant.is_none() && !i.artifact && !i.is_broken())
            .max_by_key(|(_, i)| i.rating())
            .map(|(idx, _)| idx)
    }

    /// A day's wear across the carried items, then sweep up anything that broke.
    /// Only the items actually in use (the top of each ranking) wear down.
    /// Returns the labels of items that broke today.
    pub fn degrade_in_use(&mut self, slots_per_kind: usize, amount: i32) -> Vec<String> {
        // Collect the in-use items' identities by ranking, then apply wear by
        // matching on a raw pointer-free key: we degrade the top N of each kind.
        for kind in ItemKind::ALL {
            // indices of this kind, best first
            let mut idxs: Vec<usize> =
                self.items.iter().enumerate().filter(|(_, i)| i.kind == kind).map(|(i, _)| i).collect();
            idxs.sort_by(|&a, &b| self.items[b].rating().cmp(&self.items[a].rating()));
            for &i in idxs.iter().take(slots_per_kind) {
                self.items[i].degrade(amount);
            }
        }
        let broken: Vec<String> =
            self.items.iter().filter(|i| i.is_broken()).map(|i| i.label()).collect();
        self.items.retain(|i| !i.is_broken());
        broken
    }

    /// The smith keeps the gear in trim: repair the most worn items by `points`
    /// each. Returns how many items were touched.
    pub fn maintain(&mut self, items_repaired: usize, points: i32) -> usize {
        let mut idxs: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| !i.artifact && i.condition < FULL_CONDITION)
            .map(|(i, _)| i)
            .collect();
        idxs.sort_by(|&a, &b| self.items[a].condition.cmp(&self.items[b].condition));
        let touched = idxs.iter().take(items_repaired).count();
        for &i in idxs.iter().take(items_repaired) {
            self.items[i].repair(points);
        }
        touched
    }
}
