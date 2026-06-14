//! The turning year: seasons and weather that colour every day.
//!
//! Both are **derived deterministically from the run seed and the day**, never
//! rolled through `gs.rng` — so adding the calendar doesn't perturb any other
//! random draw, and a restored save sees exactly the same skies. Season runs on
//! a fixed cycle (Spring→Winter); weather is a seeded pick from that season's
//! moods. The first day is always calm, so the founding day reads cleanly.

use serde::{Deserialize, Serialize};

/// Days each season holds before the wheel turns. A full year is four of these.
pub const DAYS_PER_SEASON: u32 = 12;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub const ALL: [Season; 4] = [Season::Spring, Season::Summer, Season::Autumn, Season::Winter];

    pub fn name(&self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }

    /// Which season a given day falls in (day 1 = the first day of spring).
    pub fn for_day(day: u32) -> Season {
        let idx = ((day.saturating_sub(1)) / DAYS_PER_SEASON) % 4;
        Season::ALL[idx as usize]
    }

    /// Percent multiplier on farm yield (100 = unchanged). Spring/autumn are the
    /// growing seasons' baseline; summer pushes; winter starves the fields.
    pub fn farm_mult_pct(&self) -> i64 {
        match self {
            Season::Spring => 100,
            Season::Summer => 115,
            Season::Autumn => 100,
            Season::Winter => 50,
        }
    }

    /// Extra firewood the cold demands, on top of the nightly burn.
    pub fn heating_extra(&self) -> i64 {
        match self {
            Season::Winter => 1,
            _ => 0,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Weather {
    Clear,
    Rain,
    Fog,
    Storm,
    Heatwave,
    Snow,
}

impl Weather {
    pub fn name(&self) -> &'static str {
        match self {
            Weather::Clear => "clear",
            Weather::Rain => "rain",
            Weather::Fog => "fog",
            Weather::Storm => "storm",
            Weather::Heatwave => "heatwave",
            Weather::Snow => "snow",
        }
    }

    /// Percent multiplier on farm yield from the day's skies.
    pub fn farm_mult_pct(&self) -> i64 {
        match self {
            Weather::Clear => 100,
            Weather::Rain => 115,
            Weather::Fog => 95,
            Weather::Storm => 80,
            Weather::Heatwave => 70,
            Weather::Snow => 60,
        }
    }

    /// A small daily morale nudge — foul weather wears on the hold.
    pub fn morale_delta(&self) -> i32 {
        match self {
            Weather::Storm | Weather::Snow | Weather::Heatwave => -1,
            _ => 0,
        }
    }

    /// Extra firewood burned for warmth and dry light.
    pub fn heating_extra(&self) -> i64 {
        match self {
            Weather::Snow | Weather::Storm => 1,
            _ => 0,
        }
    }

    /// How the footing tells on the wall: storms and snow hamper the defenders.
    pub fn combat_edge(&self) -> i32 {
        match self {
            Weather::Storm => -2,
            Weather::Snow | Weather::Fog => -1,
            _ => 0,
        }
    }

    /// Whether this sky is worth a line in the log when it arrives.
    pub fn is_notable(&self) -> bool {
        !matches!(self, Weather::Clear)
    }

    fn for_day(run_seed: u64, day: u32, season: Season) -> Weather {
        if day <= 1 {
            return Weather::Clear; // the founding day dawns calm
        }
        let roll = hash(run_seed ^ (day as u64).wrapping_mul(0x9E3779B97F4A7C15));
        let table: &[(Weather, u32)] = match season {
            Season::Spring => {
                &[(Weather::Clear, 45), (Weather::Rain, 30), (Weather::Fog, 15), (Weather::Storm, 10)]
            }
            Season::Summer => {
                &[(Weather::Clear, 50), (Weather::Heatwave, 25), (Weather::Storm, 15), (Weather::Rain, 10)]
            }
            Season::Autumn => {
                &[(Weather::Clear, 40), (Weather::Rain, 25), (Weather::Fog, 20), (Weather::Storm, 15)]
            }
            Season::Winter => {
                &[(Weather::Snow, 40), (Weather::Clear, 30), (Weather::Storm, 20), (Weather::Fog, 10)]
            }
        };
        pick(roll, table)
    }
}

/// The calendar as it stands on a given day: the season and the day's weather.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct World {
    pub season: Season,
    pub weather: Weather,
}

impl Default for World {
    fn default() -> World {
        World { season: Season::Spring, weather: Weather::Clear }
    }
}

impl World {
    /// The skies for `day` under this run's seed — pure, no rng draw.
    pub fn for_day(run_seed: u64, day: u32) -> World {
        let season = Season::for_day(day);
        World { season, weather: Weather::for_day(run_seed, day, season) }
    }

    /// Combined farm multiplier (percent) from season and weather.
    pub fn farm_mult_pct(&self) -> i64 {
        self.season.farm_mult_pct() * self.weather.farm_mult_pct() / 100
    }

    pub fn heating_extra(&self) -> i64 {
        self.season.heating_extra() + self.weather.heating_extra()
    }

    /// A short HUD descriptor, e.g. "winter · snow".
    pub fn describe(&self) -> String {
        format!("{} · {}", self.season.name(), self.weather.name())
    }
}

fn pick(roll: u64, table: &[(Weather, u32)]) -> Weather {
    let total: u32 = table.iter().map(|(_, w)| *w).sum();
    let mut r = (roll % total as u64) as u32;
    for (w, weight) in table {
        if r < *weight {
            return *w;
        }
        r -= *weight;
    }
    table[0].0
}

/// SplitMix64 — a cheap, well-distributed hash so weather varies by day and run
/// without drawing from the game rng.
fn hash(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}
