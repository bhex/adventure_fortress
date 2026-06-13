//! The region beyond the walls: named sites that fight the spreading darkness.
//! Portals push darkness up, standing sites push it down — it fluctuates rather
//! than marching upward. When a site is overrun the darkness spikes and its
//! survivors arrive at the fortress gates in waves.
//!
//! Intentionally minimal and self-contained; the region will grow later.

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::rng::GameRng;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    City,
    Fortress,
    MercCompany,
    AdventurerBand,
}

impl SiteKind {
    pub fn name(&self) -> &'static str {
        match self {
            SiteKind::City => "city",
            SiteKind::Fortress => "fortress",
            SiteKind::MercCompany => "mercenary company",
            SiteKind::AdventurerBand => "adventurer band",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub name: String,
    pub kind: SiteKind,
    pub strength: i32,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Region {
    /// 0 (dawn-bright) .. 100 (the world drowns).
    pub darkness: i32,
    /// Daily upward push from the demon portals.
    pub portal_pressure: i32,
    pub sites: Vec<Site>,
    /// Days of inbound refugees still owed from fallen sites.
    pub refugee_wave_days: u32,
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
            };
            let name = pool[rng.random_range(0..pool.len())].to_string();
            if sites.iter().any(|s: &Site| s.name == name) {
                continue; // skip duplicates; 6-8 becomes "up to 8", still >= 5
            }
            let strength = match kind {
                SiteKind::City => rng.random_range(10..=14),
                SiteKind::Fortress => rng.random_range(8..=12),
                SiteKind::MercCompany => rng.random_range(5..=9),
                SiteKind::AdventurerBand => rng.random_range(4..=7),
            };
            sites.push(Site { name, kind, strength });
        }
        Region {
            darkness: rng.random_range(5..=15),
            portal_pressure: 2,
            sites,
            refugee_wave_days: 0,
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

    /// Whether survivors of fallen sites are still on the road to the gates.
    pub fn refugees_incoming(&self) -> bool {
        self.refugee_wave_days > 0
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

        // High darkness grinds the sites down.
        if self.darkness >= 50 && !self.sites.is_empty() {
            let strikes = if self.darkness >= 75 { 2 } else { 1 };
            for _ in 0..strikes {
                if self.sites.is_empty() {
                    break;
                }
                let idx = rng.random_range(0..self.sites.len());
                self.sites[idx].strength -= rng.random_range(1..=3);
                if self.sites[idx].strength <= 0 {
                    let fallen = self.sites.remove(idx);
                    self.darkness = (self.darkness + 10).min(100);
                    self.refugee_wave_days += rng.random_range(1..=2);
                    lines.push(format!(
                        "Word arrives: {} of {} has fallen to the darkness. Refugees take to the roads.",
                        fallen.kind.name(),
                        fallen.name
                    ));
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
