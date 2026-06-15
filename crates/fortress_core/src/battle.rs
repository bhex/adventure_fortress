//! Battle resolution: several rounds, deterministic, narrated.
//!
//! Combat events deal their harm through `fight_battle` rather than flat role
//! damage. The defenders muster — the commander and heroes strike, mages cast
//! Sorcery or throw up Wards, the guard holds the line — and the fight plays out
//! over a handful of rounds as momentum swings. Lose ground badly enough and the
//! gate breaks: then *everyone* takes up arms, the skilled worth far more than
//! the levy. The wounded are named individuals; wounds run through the same
//! `damage()` paths and combat mitigation as every other hit, so traits, gear,
//! armor, and class skills all still tell.

use crate::adventurers::AdventurerClass;
use crate::engine::mitigate_damage;
use crate::events::Event;
use crate::game_state::GameState;
use crate::inhabitants::Role;
use crate::items::ItemKind;
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

/// At most this many exchanges; a decisive swing ends it sooner.
const MAX_ROUNDS: usize = 5;
/// Momentum past which one side has plainly broken the other.
const DECISIVE: i32 = 16;
/// Momentum at which the line fails and the gate is forced — every hand fights.
const BREACH_AT: i32 = -8;

/// What a combatant is, for narration and for who can be wounded.
#[derive(Clone, Copy, PartialEq)]
enum ActorKind {
    Commander,
    Caster, // commander or inhabitant fighting chiefly with offensive magic
    Guard,
    Knight, // a sworn hero — lends prowess, kept whole (no health model)
    Levy,   // a non-combatant pressed to the wall on a breach
}

/// One participant on the wall. `push` is their per-round contribution; only
/// `mortal` ones can be wounded.
struct Combatant {
    name: String,
    push: i32,
    mortal: bool,
    kind: ActorKind,
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

    // Everyone grabs their best arms before the foe arrives.
    gs.redistribute_equipment();

    // ---- muster the defenders ----
    let mut frontline: Vec<Combatant> = Vec::new();
    let mut reserves: Vec<Combatant> = Vec::new();
    let mut warding = 0i32; // Wards blunt the foe rather than adding to our push

    let commander_fights = gs.player.as_ref().is_some_and(|p| p.is_alive());
    let commander_name: Option<String> =
        commander_fights.then(|| gs.player.as_ref().unwrap().name.clone());
    if let Some(p) = &gs.player {
        if p.is_alive() {
            // the commander's own blade lends its weight to their martial push
            let weapon = p.loadout.rating(ItemKind::Weapon);
            let martial = p.stats.might as i32 + p.skills.tier(Skill::Combat).index() as i32 + weapon;
            let offense = magic_offense(&p.skills);
            let ward = p.skills.tier(Skill::Warding).index() as i32;
            // A caster commander leads with the stronger of blade or bolt.
            let (push, kind) = if offense > martial {
                (offense, ActorKind::Caster)
            } else {
                (martial, ActorKind::Commander)
            };
            warding += ward;
            frontline.push(Combatant { name: p.name.clone(), push, mortal: true, kind });
        }
    }

    // Guards stand the line; mage-folk among the inhabitants step up to cast or
    // to ward; everyone else is a reserve, called up only on a breach.
    for i in gs.inhabitants.get_alive() {
        match i.role {
            Role::Guard => {
                let push = i.skills.tier(Skill::Combat).index() as i32
                    + 1
                    + i.loadout.rating(ItemKind::Weapon);
                frontline.push(Combatant {
                    name: i.name.clone(),
                    push,
                    mortal: true,
                    kind: ActorKind::Guard,
                });
            }
            _ => {
                let offense = magic_offense(&i.skills);
                let ward = i.skills.tier(Skill::Warding).index() as i32;
                if offense >= 4 || ward >= 2 {
                    // a true mage: bolts add to our push, wards blunt the foe
                    warding += ward;
                    frontline.push(Combatant {
                        name: i.name.clone(),
                        push: offense.max(1),
                        mortal: true,
                        kind: ActorKind::Caster,
                    });
                } else {
                    // ordinary folk — fight only in extremis, and poorly
                    let push = i.skills.tier(Skill::Combat).index() as i32;
                    reserves.push(Combatant {
                        name: i.name.clone(),
                        push,
                        mortal: true,
                        kind: ActorKind::Levy,
                    });
                }
            }
        }
    }

    for a in gs.adventurers.iter().filter(|a| a.class == AdventurerClass::Knight) {
        frontline.push(Combatant {
            name: a.name.clone(),
            push: a.perk_tier().index() as i32 + a.loadout.rating(ItemKind::Weapon),
            mortal: false,
            kind: ActorKind::Knight,
        });
    }

    // ---- the foe ----
    let mut enemy = power.max(1);
    if event.has_tag("demon") {
        match gs.region.band() {
            DarknessBand::Deep => enemy += enemy / 4,
            DarknessBand::Overwhelming => enemy += enemy / 2,
            _ => {}
        }
    }
    let enemy_effective = (enemy - warding).max(1);

    // High hearts press the attack; a sullen hold gives ground — the morale
    // passive made flesh on the wall.
    let morale_edge = match gs.fortress.morale {
        m if m >= 75 => 3,
        m if m <= 25 => -3,
        _ => 0,
    };

    lines.push(format!(
        "{} muster against a foe of strength {}{}.",
        muster_phrase(&frontline, commander_name.as_deref()),
        enemy,
        if warding > 0 { " (the wards bite into it)" } else { "" },
    ));

    // ---- the rounds ----
    let mut momentum = 0i32;
    let mut breached = false;
    let mut rounds = 0usize;
    while rounds < MAX_ROUNDS && !frontline.is_empty() {
        rounds += 1;
        let our = side_strength(&frontline, gs, morale_edge) + momentum / 5;
        let our_roll = our + gs.rng.random_range(1..=6);
        let foe_roll = enemy_effective + gs.rng.random_range(1..=6);
        let round_margin = our_roll - foe_roll;
        momentum += round_margin;

        lines.push(round_line(rounds, &frontline, round_margin, gs));

        // a lost round draws blood; the bigger the loss, the more blows land
        if round_margin < 0 {
            let blows = (1 + (-round_margin) / 5).clamp(1, 3);
            for _ in 0..blows {
                if let Some(line) = land_a_blow(&mut frontline, event, gs) {
                    lines.push(line);
                }
                if frontline.is_empty() {
                    break;
                }
            }
        }

        // the gate gives: every hand to the wall (once)
        if momentum <= BREACH_AT && !breached && !reserves.is_empty() {
            breached = true;
            let n = reserves.len();
            frontline.append(&mut reserves);
            lines.push(format!(
                "The gate is forced — {n} more snatch up whatever will cut and rush the breach!"
            ));
        }

        if momentum.abs() >= DECISIVE {
            break; // one side has plainly broken
        }
    }

    let victory = momentum >= 0 && !frontline.is_empty();

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
        lines.extend(roll_loot(event, gs));
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

/// Best offensive-magic contribution from a skill set: Sorcery or the DarkArts,
/// whichever runs stronger, worth twice its tier in raw push.
fn magic_offense(skills: &crate::skills::SkillSet) -> i32 {
    let sorcery = skills.tier(Skill::Sorcery).index() as i32;
    let dark = skills.tier(Skill::DarkArts).index() as i32;
    sorcery.max(dark) * 2
}

/// Our whole strength this round: every active fighter's push (their own arms
/// already folded in at muster), plus the bulk armory, the walls, and the day's
/// heart.
fn side_strength(active: &[Combatant], gs: &GameState, morale_edge: i32) -> i32 {
    let combat: i32 = active.iter().map(|c| c.push).sum();
    let gear = match gs.resources.band(ResourceKind::Gear) {
        StockBand::Exhausted => 0,
        StockBand::Scarce | StockBand::Lean => 1,
        StockBand::Adequate => 2,
        StockBand::Comfortable => 3,
        StockBand::Plentiful => 4,
    };
    combat + gear + gs.fortress.defense / 10 + morale_edge + gs.world.weather.combat_edge()
}

/// Pick the round's standout and tell what they did, coloured by the tide.
fn round_line(round: usize, active: &[Combatant], margin: i32, gs: &GameState) -> String {
    let won = margin >= 0;
    // the one with the most to give carries the round's narration
    let hero = active.iter().max_by_key(|c| c.push);
    let action = match hero.map(|h| (h.kind, h.name.as_str())) {
        Some((ActorKind::Commander, n)) => format!("{n} carves into the enemy ranks"),
        Some((ActorKind::Caster, n)) => format!("{n} looses a searing bolt"),
        Some((ActorKind::Knight, n)) => format!("{n} anchors the shieldwall"),
        Some((ActorKind::Guard, n)) => format!("{n} and the watch hold formation"),
        Some((ActorKind::Levy, n)) => format!("{n} swings a borrowed blade"),
        None => "the defenders rally".to_string(),
    };
    let tide = if won {
        if gs.fortress.morale >= 75 { "the hold surges forward" } else { "and the line presses on" }
    } else {
        "but the foe drives them back a step"
    };
    format!("Round {round}: {action} — {tide}.")
}

/// One blow lands on a random mortal of the frontline, through the shared
/// `damage()`/mitigation path. Returns a line; drops the slain from the line
/// and grieves the hold.
fn land_a_blow(active: &mut Vec<Combatant>, event: &Event, gs: &mut GameState) -> Option<String> {
    let mortal_idxs: Vec<usize> =
        active.iter().enumerate().filter(|(_, c)| c.mortal).map(|(i, _)| i).collect();
    if mortal_idxs.is_empty() {
        return None; // only sworn heroes left standing — the walls take the rest
    }
    let pick = mortal_idxs[gs.rng.random_range(0..mortal_idxs.len())];
    let target = active[pick].name.clone();
    let raw = -gs.rng.random_range(12..=22);
    let wound = -mitigate_damage(raw, event, gs); // positive after mitigation
    let died = apply_wound(gs, &target, wound);
    if died {
        let grief = if gs.fortress.graveyard_level() > 0 { -1 } else { -3 };
        gs.fortress.apply_morale_delta(grief);
        gs.apply_reputation_delta(-1);
        active.remove(pick);
        Some(format!("{target} falls in the press."))
    } else {
        Some(format!("{target} takes a wound. (-{wound} health)"))
    }
}

/// What a beaten foe leaves on the field — keyed off the event's tags, the
/// closest thing to a per-enemy loot table. Demons leave the residue that
/// holds enchantments; raiders and the like leave usable arms now and then.
fn roll_loot(event: &Event, gs: &mut GameState) -> Vec<String> {
    let mut lines = Vec::new();
    // Demon foes burn away into portal residue — the rarer the dark, the more.
    if event.has_tag("demon") {
        let mut amount = gs.rng.random_range(1..=3) as i64;
        if matches!(gs.region.band(), DarknessBand::Deep | DarknessBand::Overwhelming) {
            amount += 1;
        }
        gs.resources.apply_delta(&ResourceDelta { residue: amount, ..Default::default() });
        lines.push(format!("The demons leave only smoking residue. (+{amount} residue)"));
    }
    // Mortal raiders drop their gear: usually scrap for the armory, sometimes a
    // whole serviceable weapon or piece of armor worth keeping.
    if event.has_tag("combat") && !event.has_tag("demon") {
        if gs.rng.random_range(0..100) < 35 {
            let kind = if gs.rng.random_range(0..2) == 0 {
                crate::items::ItemKind::Weapon
            } else {
                crate::items::ItemKind::Armor
            };
            // battlefield finds are rough — crude or plain at best
            let quality = if gs.rng.random_range(0..100) < 25 {
                crate::items::Quality::Fine
            } else if gs.rng.random_range(0..2) == 0 {
                crate::items::Quality::Plain
            } else {
                crate::items::Quality::Crude
            };
            let item = crate::items::Item::new(kind, quality);
            lines.push(format!("A {} is taken from the fallen.", item.label()));
            gs.items.add(item);
        } else {
            let gear = gs.rng.random_range(2..=5);
            gs.resources.apply_delta(&ResourceDelta { gear, ..Default::default() });
            lines.push(format!("Broken arms are gathered for the smith. (+{gear} gear)"));
        }
    }
    lines
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

fn muster_phrase(frontline: &[Combatant], commander_name: Option<&str>) -> String {
    let mut parts = Vec::new();
    if commander_name.is_some() {
        parts.push("the commander".to_string());
    }
    let guards = frontline.iter().filter(|c| c.kind == ActorKind::Guard).count();
    match guards {
        0 => {}
        1 => parts.push("a lone guard".to_string()),
        n => parts.push(format!("{n} guards")),
    }
    // casters other than the commander
    let mages = frontline
        .iter()
        .filter(|c| c.kind == ActorKind::Caster && Some(c.name.as_str()) != commander_name)
        .count();
    match mages {
        0 => {}
        1 => parts.push("a mage".to_string()),
        n => parts.push(format!("{n} mages")),
    }
    let knights = frontline.iter().filter(|c| c.kind == ActorKind::Knight).count();
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
