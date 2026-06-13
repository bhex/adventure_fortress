//! Battle resolution: one pass, deterministic, narrated.
//!
//! Combat events deal their harm through `fight_battle` rather than flat role
//! damage. The commander and the guard muster their prowess, one clash decides
//! the day, and the wounded are named individuals — wounds run through the same
//! `damage()` paths and combat mitigation as every other hit, so traits, gear,
//! and abilities all still tell.

use crate::adventurers::AdventurerClass;
use crate::engine::mitigate_damage;
use crate::events::Event;
use crate::game_state::GameState;
use crate::inhabitants::Role;
use crate::region::DarknessBand;
use crate::resources::{ResourceDelta, ResourceKind, StockBand};
use crate::skills::Skill;
use rand::Rng;

/// The outcome of one battle: the narrated blow-by-blow and whether the
/// fortress held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BattleReport {
    pub lines: Vec<String>,
    pub victory: bool,
}

/// Resolve one battle against a foe of the given `power`, paying out
/// `loot_valuables` on victory. Fully deterministic through `gs.rng`.
pub fn fight_battle(
    power: i32,
    loot_valuables: i64,
    event: &Event,
    gs: &mut GameState,
) -> BattleReport {
    let mut lines = Vec::new();

    // ---- muster the defenders ----
    // `mortals` are the woundable participants (commander + guards); knights
    // lend their prowess and hold the line but have no health model.
    let mut prowess = 0i32;
    let mut mortals: Vec<String> = Vec::new();

    let commander_fights = gs.player.as_ref().is_some_and(|p| p.is_alive());
    if let Some(p) = &gs.player {
        if p.is_alive() {
            prowess += p.stats.might as i32 + p.skills.tier(Skill::Combat).index() as i32;
            mortals.push(p.name.clone());
        }
    }
    let guard_names: Vec<String> = gs
        .inhabitants
        .get_by_role(Role::Guard)
        .iter()
        .map(|g| g.name.clone())
        .collect();
    for g in gs.inhabitants.get_by_role(Role::Guard) {
        prowess += g.skills.tier(Skill::Combat).index() as i32 + 1;
    }
    let guard_count = guard_names.len();
    mortals.extend(guard_names);

    let knights: Vec<String> = gs
        .adventurers
        .iter()
        .filter(|a| a.class == AdventurerClass::Knight)
        .map(|a| {
            prowess += a.perk_tier().index() as i32;
            a.name.clone()
        })
        .collect();

    // a stocked armory and stout walls add their weight
    prowess += match gs.resources.band(ResourceKind::Gear) {
        StockBand::Exhausted => 0,
        StockBand::Scarce => 1,
        StockBand::Lean => 1,
        StockBand::Adequate => 2,
        StockBand::Comfortable => 3,
        StockBand::Plentiful => 4,
    };
    prowess += gs.fortress.defense / 10;

    // ---- the foe ----
    let mut enemy = power.max(1);
    if event.has_tag("demon") {
        match gs.region.band() {
            DarknessBand::Deep => enemy += enemy / 4,
            DarknessBand::Overwhelming => enemy += enemy / 2,
            _ => {}
        }
    }

    lines.push(format!(
        "{} muster against a foe of strength {}.",
        muster_phrase(commander_fights, guard_count, knights.len()),
        enemy,
    ));

    // ---- the clash: one contested roll ----
    let our_roll = prowess + gs.rng.random_range(1..=6);
    let their_roll = enemy + gs.rng.random_range(1..=6);
    let margin = our_roll - their_roll;
    let victory = margin >= 0;

    // a hard-won victory still draws blood; a rout lands many blows
    let blows = if victory {
        ((4 - margin) / 4).clamp(0, 3)
    } else {
        (1 + (-margin) / 3).clamp(1, 5)
    };

    for _ in 0..blows {
        if mortals.is_empty() {
            // no one left to bleed — the walls take what comes
            break;
        }
        let idx = gs.rng.random_range(0..mortals.len());
        let target = mortals[idx].clone();
        let raw = -gs.rng.random_range(12..=22);
        let wound = -mitigate_damage(raw, event, gs); // positive after mitigation
        let died = apply_wound(gs, &target, wound);
        if died {
            lines.push(format!("{target} falls in the press."));
            mortals.remove(idx);
        } else {
            lines.push(format!("{target} takes a wound. (-{wound} health)"));
        }
    }

    // knights hold the breach regardless of the tide
    if let Some(first) = knights.first() {
        lines.push(format!("{first} holds the breach."));
    }

    // ---- the reckoning ----
    if victory {
        gs.fortress.apply_morale_delta(5);
        gs.apply_reputation_delta(3);
        if commander_fights {
            if let Some(p) = &gs.player {
                lines.push(format!("{} cuts the enemy banner down. The fortress holds!", p.name));
            }
        } else {
            lines.push("The foe breaks and scatters. The fortress holds!".to_string());
        }
        if loot_valuables > 0 {
            gs.resources
                .apply_delta(&ResourceDelta { valuables: loot_valuables, ..Default::default() });
            lines.push(format!("The field is stripped of spoils. (+{loot_valuables} valuables)"));
        }
    } else {
        gs.fortress.apply_defense_delta(-3);
        gs.fortress.apply_morale_delta(-8);
        gs.apply_reputation_delta(-2);
        lines.push(
            "The line buckles; the enemy takes their toll before they withdraw. (-3 defense, -8 morale)"
                .to_string(),
        );
    }

    BattleReport { lines, victory }
}

/// Wound a named defender through the same `damage()` path everyone uses, so
/// the Sickly trait (and the commander's lack of one) still tell. Returns
/// whether the blow was fatal.
fn apply_wound(gs: &mut GameState, name: &str, wound: i32) -> bool {
    if let Some(p) = gs.player.as_mut() {
        if p.name == name {
            p.damage(wound);
            return !p.is_alive();
        }
    }
    if let Some(inh) = gs.inhabitants.find_mut(name) {
        inh.damage(wound);
        return !inh.is_alive;
    }
    false
}

fn muster_phrase(commander: bool, guards: usize, knights: usize) -> String {
    let mut parts = Vec::new();
    if commander {
        parts.push("the commander".to_string());
    }
    match guards {
        0 => {}
        1 => parts.push("a lone guard".to_string()),
        n => parts.push(format!("{n} guards")),
    }
    match knights {
        0 => {}
        1 => parts.push("a sworn knight".to_string()),
        n => parts.push(format!("{n} knights")),
    }
    if parts.is_empty() {
        return "The walls alone".to_string();
    }
    // join with commas and a trailing "and"
    if parts.len() == 1 {
        capitalize(&parts[0])
    } else {
        let last = parts.pop().unwrap();
        capitalize(&format!("{} and {}", parts.join(", "), last))
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
