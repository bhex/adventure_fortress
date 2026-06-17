//! The region beyond the walls: named sites that fight the spreading darkness.
//! Portals push darkness up, standing sites push it down — it fluctuates rather
//! than marching upward. When a site is overrun the darkness spikes and its
//! survivors arrive at the fortress gates in waves.
//!
//! Intentionally minimal and self-contained; the region will grow later.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::rng::GameRng;

/// The logical region grid. Sites, portals and the fortress live on these
/// coordinates; the UI renders a terminal of the same dimensions. The fortress
/// sits at the centre as the map's focal point.
pub const REGION_W: i16 = 64;
pub const REGION_H: i16 = 40;
pub const FORTRESS_POS: Coord = Coord { x: REGION_W / 2, y: REGION_H / 2 };

/// How far a portal's blight can reach at full darkness, in tiles.
const BLIGHT_MAX_REACH: i32 = 22;

/// A point on the region grid. Plain data — core carries no glam/Bevy dep.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Coord {
    pub x: i16,
    pub y: i16,
}

impl Coord {
    pub fn new(x: i16, y: i16) -> Coord {
        Coord { x, y }
    }

    /// Euclidean distance, rounded to whole tiles.
    pub fn dist(self, other: Coord) -> i32 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        (dx * dx + dy * dy).sqrt().round() as i32
    }
}

/// Nearest portal distance to a point; a large sentinel when there are none.
fn nearest_dist(portals: &[Coord], pos: Coord) -> i32 {
    portals.iter().map(|p| p.dist(pos)).min().unwrap_or(i32::MAX / 2)
}

/// Deterministically place a point that clears the fortress and existing points
/// by the given margins, sampling against the run rng (capped retries).
fn place_clear(rng: &mut GameRng, taken: &[Coord], from_fortress: i32, from_others: i32) -> Coord {
    let mut last = FORTRESS_POS;
    for _ in 0..128 {
        let c = Coord::new(
            rng.random_range(2..REGION_W - 2),
            rng.random_range(2..REGION_H - 2),
        );
        last = c;
        if c.dist(FORTRESS_POS) >= from_fortress
            && taken.iter().all(|t| c.dist(*t) >= from_others)
        {
            return c;
        }
    }
    last // give up gracefully after retries — still deterministic
}

/// A point on (or near) the map edge for a portal, spread from existing portals.
fn edge_point(rng: &mut GameRng, taken: &[Coord]) -> Coord {
    let mut last = Coord::new(1, 1);
    for _ in 0..32 {
        let c = match rng.random_range(0..4) {
            0 => Coord::new(rng.random_range(1..REGION_W - 1), 1),
            1 => Coord::new(rng.random_range(1..REGION_W - 1), REGION_H - 2),
            2 => Coord::new(1, rng.random_range(1..REGION_H - 1)),
            _ => Coord::new(REGION_W - 2, rng.random_range(1..REGION_H - 1)),
        };
        last = c;
        if taken.iter().all(|t| c.dist(*t) >= 14) {
            return c;
        }
    }
    last
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    City,
    Fortress,
    MercCompany,
    AdventurerBand,
    /// A camp of survivors regrouping after the region fell — the seed of a
    /// rebuilt world. Fragile, but it can grow back into a proper hold.
    Survivors,
}

impl SiteKind {
    pub fn name(&self) -> &'static str {
        match self {
            SiteKind::City => "city",
            SiteKind::Fortress => "fortress",
            SiteKind::MercCompany => "mercenary company",
            SiteKind::AdventurerBand => "adventurer band",
            SiteKind::Survivors => "survivor camp",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub name: String,
    pub kind: SiteKind,
    pub strength: i32,
    /// Where the site sits on the region grid (the map's focal layout).
    #[serde(default)]
    pub pos: Coord,
}

impl Site {
    /// How the site is faring, for at-a-glance UI — the number stays internal.
    pub fn strength_band(&self) -> &'static str {
        match self.strength {
            10.. => "thriving",
            6..=9 => "holding",
            3..=5 => "failing",
            _ => "besieged",
        }
    }
}

/// Adjective bands for the HUD — the number stays internal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DarknessBand {
    Quiet,
    Gathering,
    Deep,
    Overwhelming,
}

impl DarknessBand {
    pub fn name(&self) -> &'static str {
        match self {
            DarknessBand::Quiet => "quiet",
            DarknessBand::Gathering => "gathering",
            DarknessBand::Deep => "deep",
            DarknessBand::Overwhelming => "overwhelming",
        }
    }
}

pub fn darkness_band(darkness: i32) -> DarknessBand {
    match darkness {
        i32::MIN..=24 => DarknessBand::Quiet,
        25..=49 => DarknessBand::Gathering,
        50..=74 => DarknessBand::Deep,
        _ => DarknessBand::Overwhelming,
    }
}

const CITY_NAMES: [&str; 6] = ["Vell", "Carrow", "Ostmere", "Dunhollow", "Bray", "Lanrick"];
const FORT_NAMES: [&str; 6] =
    ["Stonewatch", "Greyspire", "Caer Morgan", "Highgate", "Thornkeep", "Redwall"];
const MERC_NAMES: [&str; 4] =
    ["the Iron Pact", "the Crowfeather Company", "the Sundered Shields", "the Ashen Banner"];
const BAND_NAMES: [&str; 4] =
    ["the Lantern Bearers", "the Greenwood Five", "the Oathbound", "the Last Toast"];
const SURVIVOR_NAMES: [&str; 4] = ["Hopewell", "the Ashfields", "New Vell", "Candlemarsh"];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// 0 (dawn-bright) .. 100 (the world drowns).
    pub darkness: i32,
    /// Daily upward push from the demon portals.
    pub portal_pressure: i32,
    pub sites: Vec<Site>,
    /// Days of inbound refugees still owed from fallen sites.
    pub refugee_wave_days: u32,
    /// Demon portals on the map edges — the darkness emanates from these, and
    /// sites nearest a portal fall first.
    #[serde(default)]
    pub portals: Vec<Coord>,
}

impl Region {
    /// Deterministically seed 6-8 named sites from the run rng.
    pub fn generate(rng: &mut GameRng) -> Region {
        let mut sites = Vec::new();
        let count = rng.random_range(6..=8);
        for i in 0..count {
            // round-robin kinds so every run has a mix; cities are sturdiest
            let kind = [
                SiteKind::City,
                SiteKind::Fortress,
                SiteKind::MercCompany,
                SiteKind::AdventurerBand,
            ][i % 4];
            let pool: &[&str] = match kind {
                SiteKind::City => &CITY_NAMES,
                SiteKind::Fortress => &FORT_NAMES,
                SiteKind::MercCompany => &MERC_NAMES,
                SiteKind::AdventurerBand => &BAND_NAMES,
                SiteKind::Survivors => &SURVIVOR_NAMES,
            };
            let name = pool[rng.random_range(0..pool.len())].to_string();
            if sites.iter().any(|s: &Site| s.name == name) {
                continue; // skip duplicates; 6-8 becomes "up to 8", still >= 5
            }
            let strength = match kind {
                SiteKind::City => rng.random_range(10..=14),
                SiteKind::Fortress => rng.random_range(8..=12),
                SiteKind::MercCompany => rng.random_range(5..=9),
                SiteKind::AdventurerBand | SiteKind::Survivors => rng.random_range(4..=7),
            };
            // place it clear of the fortress and the other holds
            let taken: Vec<Coord> = sites.iter().map(|s: &Site| s.pos).collect();
            let pos = place_clear(rng, &taken, 6, 8);
            sites.push(Site { name, kind, strength, pos });
        }

        // demon portals brood on the edges; the darkness leaks from them
        let mut portals = Vec::new();
        for _ in 0..rng.random_range(2..=4) {
            portals.push(edge_point(rng, &portals));
        }

        Region {
            darkness: rng.random_range(5..=15),
            portal_pressure: 2,
            sites,
            refugee_wave_days: 0,
            portals,
        }
    }

    pub fn band(&self) -> DarknessBand {
        darkness_band(self.darkness)
    }

    pub fn total_strength(&self) -> i32 {
        self.sites.iter().map(|s| s.strength).sum()
    }

    /// Free-world sites still standing against the dark.
    pub fn standing_sites(&self) -> usize {
        self.sites.len()
    }

    /// The whole region has gone dark — nothing free is left out there.
    pub fn all_fallen(&self) -> bool {
        self.sites.is_empty()
    }

    /// Whether survivors of fallen sites are still on the road to the gates.
    pub fn refugees_incoming(&self) -> bool {
        self.refugee_wave_days > 0
    }

    /// Distance from a point to the nearest portal (large sentinel if none).
    pub fn nearest_portal_dist(&self, pos: Coord) -> i32 {
        nearest_dist(&self.portals, pos)
    }

    /// How far the blight reaches from each portal, scaling with darkness.
    pub fn blight_radius(&self) -> i32 {
        self.darkness * BLIGHT_MAX_REACH / 100
    }

    /// Whether a point lies within the blight (a portal's darkened reach).
    pub fn in_blight(&self, pos: Coord) -> bool {
        let reach = self.blight_radius();
        reach > 0 && self.nearest_portal_dist(pos) <= reach
    }

    /// Days for an expedition to reach a site and back — scales with distance.
    pub fn expedition_days(&self, site_name: &str) -> i32 {
        match self.sites.iter().find(|s| s.name == site_name) {
            Some(site) => (2 + FORTRESS_POS.dist(site.pos) / 7).clamp(3, 12),
            None => 5,
        }
    }

    /// Pick a site to grind, biased toward those nearest a portal — the dark
    /// closes on the exposed first. Falls back to uniform when there are no
    /// portals. rng-driven and deterministic.
    fn weighted_strike_idx(&self, rng: &mut GameRng) -> usize {
        if self.portals.is_empty() {
            return rng.random_range(0..self.sites.len());
        }
        let weights: Vec<i32> = self
            .sites
            .iter()
            .map(|s| 4 + (BLIGHT_MAX_REACH - self.nearest_portal_dist(s.pos)).max(0) / 6)
            .collect();
        let total: i32 = weights.iter().sum();
        let mut roll = rng.random_range(0..total);
        for (i, w) in weights.iter().enumerate() {
            roll -= w;
            if roll < 0 {
                return i;
            }
        }
        self.sites.len() - 1
    }

    /// One day in the wider war. Returns log lines worth telling the player.
    pub fn tick(&mut self, rng: &mut GameRng) -> Vec<String> {
        let mut lines = Vec::new();

        // The portals widen, slowly and unevenly.
        if rng.random_range(0..10) == 0 && self.portal_pressure < 6 {
            self.portal_pressure += 1;
            lines.push("The night sky flickers — the portals widen.".to_string());
        }

        // Darkness: pressure and noise up, the free peoples push back.
        let push = self.portal_pressure + rng.random_range(-2..=2);
        let resist = self.total_strength() / 20;
        self.darkness = (self.darkness + push - resist).clamp(0, 100);

        // The world end-state: with every site fallen, the war beyond the walls
        // is over — but life is stubborn. As the dark eases, survivors regroup
        // into a fragile camp that can grow back into a hold (and resume sending
        // aid, envoys, and trade), so the late game stays coherent.
        if self.sites.is_empty() {
            if self.darkness < 80 && rng.random_range(0..100) < 6 {
                let name = SURVIVOR_NAMES[rng.random_range(0..SURVIVOR_NAMES.len())].to_string();
                let pos = place_clear(rng, &[], 6, 8);
                self.sites.push(Site {
                    name: name.clone(),
                    kind: SiteKind::Survivors,
                    strength: rng.random_range(3..=5),
                    pos,
                });
                lines.push(format!(
                    "Out of the ruin, survivors gather at {name} and begin to rebuild."
                ));
            }
            return lines; // nothing left to grind down this turn
        }

        // A standing survivor camp that weathers the storm can grow into a
        // proper free hold once more.
        for site in self.sites.iter_mut() {
            if site.kind == SiteKind::Survivors
                && self.darkness < 40
                && site.strength < 8
                && rng.random_range(0..100) < 20
            {
                site.strength += 1;
            }
        }

        // High darkness grinds the sites down — geography decides who first.
        // The total grind matches the old uniform model (balance-neutral); only
        // the *target* is biased, so the holds nearest a portal fall first.
        if self.darkness >= 50 && !self.sites.is_empty() {
            let strikes = if self.darkness >= 75 { 2 } else { 1 };
            for _ in 0..strikes {
                if self.sites.is_empty() {
                    break;
                }
                let idx = self.weighted_strike_idx(rng);
                self.sites[idx].strength -= rng.random_range(1..=3);
            }

            // Removal sweep: any hold ground to nothing falls, and the dark
            // surges where it broke through.
            let mut i = 0;
            while i < self.sites.len() {
                if self.sites[i].strength <= 0 {
                    let fallen = self.sites.remove(i);
                    self.darkness = (self.darkness + 10).min(100);
                    self.refugee_wave_days += rng.random_range(1..=2);
                    lines.push(format!(
                        "Word arrives: {} of {} has fallen to the darkness. Refugees take to the roads.",
                        fallen.kind.name(),
                        fallen.name
                    ));
                } else {
                    i += 1;
                }
            }
        }

        lines
    }

    /// Bolster (or sap) a random surviving site — the target of aid sent out.
    pub fn adjust_random_site(&mut self, rng: &mut GameRng, amount: i32) -> Option<String> {
        if self.sites.is_empty() {
            return None;
        }
        let idx = rng.random_range(0..self.sites.len());
        let site = &mut self.sites[idx];
        site.strength = (site.strength + amount).max(0);
        Some(site.name.clone())
    }
}
