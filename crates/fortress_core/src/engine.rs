use rand::Rng;

use crate::events::{Choice, Effect, Event, EventResult};
use crate::fortress::Upgrade;
use crate::game_state::GameState;
use crate::inhabitants::{generate_inhabitant, Role, Trait};
use crate::player::{ClassKind, PlayerCharacter, StatKind};
use crate::resources::ResourceDelta;
use crate::rng::weighted_index;
use crate::skills::{Skill, SkillTier};

/// Who trains what from living through an event with this tag.
fn training_for_tag(tag: &str) -> &'static [(Role, Skill, u32)] {
    match tag {
        "combat" => &[(Role::Guard, Skill::Combat, 8)],
        "disaster" => &[(Role::Healer, Skill::Medicine, 8)],
        "economy" => &[
            (Role::Blacksmith, Skill::Smithing, 6),
            (Role::Farmer, Skill::Crafting, 6),
        ],
        _ => &[],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChoiceAvailability {
    Ok,
    CantAfford,
    StatLocked(StatKind, u8),
}

/// A plain-language preview of what a choice's effects will do, for the modal.
/// Story-flag bookkeeping is hidden; resource changes stay in the soft style.
pub fn describe_effects(effects: &[crate::events::Effect]) -> String {
    use crate::events::Effect;
    let mut parts: Vec<String> = Vec::new();
    for e in effects {
        let clause = match e {
            Effect::Resource(d) => d.describe(),
            Effect::Morale { amount } => signed("morale", *amount),
            Effect::Defense { amount } => signed("defense", *amount),
            Effect::ApplyToRole { role, health, morale } => {
                let mut bits = Vec::new();
                if *health != 0 {
                    bits.push(signed("health", *health));
                }
                if *morale != 0 {
                    bits.push(signed("morale", *morale));
                }
                format!("{}s: {}", role.name(), bits.join(", "))
            }
            Effect::Battle { power, .. } => format!("a battle (foe strength ~{power})"),
            Effect::SpawnInhabitant { .. } => "a newcomer may join".to_string(),
            Effect::KillInhabitant { .. } | Effect::RemoveInhabitant {} => {
                "someone may be lost".to_string()
            }
            Effect::AddUpgrade { name } => format!("raise the {}", name.name()),
            Effect::Region { .. } => "shifts the war beyond the walls".to_string(),
            Effect::GrantItem { artifact, name, .. } => {
                if *artifact {
                    match name {
                        Some(n) => format!("the artifact {n}"),
                        None => "an artifact for the armory".to_string(),
                    }
                } else {
                    "an item for the armory".to_string()
                }
            }
            // story bookkeeping the player needn't see
            Effect::SetFlag { .. } | Effect::ClearFlag { .. } => String::new(),
        };
        if !clause.is_empty() {
            parts.push(clause);
        }
    }
    parts.join(" · ")
}

fn signed(label: &str, amount: i32) -> String {
    format!("{}{} {}", if amount >= 0 { "+" } else { "" }, amount, label)
}

/// Rough odds of passing a stat check, given the commander — for the modal.
pub fn stat_check_odds(check: &crate::events::StatCheck, player: &PlayerCharacter) -> &'static str {
    let stat = player.stats.get(check.stat) as i32;
    // success if stat + d6 >= difficulty, i.e. the die must land >= (difficulty - stat)
    let need = check.difficulty - stat;
    let favorable = (7 - need).clamp(0, 6);
    match favorable {
        6 => "certain",
        5 => "very likely",
        4 => "likely",
        3 => "even",
        2 => "unlikely",
        1 => "remote",
        _ => "hopeless",
    }
}

pub fn eligible_events<'a>(
    deck: &'a [Event],
    day: u32,
    gs: &GameState,
    last_event_name: Option<&str>,
) -> Vec<&'a Event> {
    deck.iter()
        .filter(|e| {
            e.min_day <= day
                && e.max_day.is_none_or(|max| day <= max)
                && e.min_morale <= gs.fortress.morale
                && gs.fortress.morale <= e.max_morale
                && gs.resources.can_afford(&e.min_resource)
                && e.requires_role.is_none_or(|r| gs.inhabitants.has_role(r))
                && e.requires_upgrade.is_none_or(|u| gs.fortress.has_upgrade(u))
                && e.min_darkness.is_none_or(|d| gs.region.darkness >= d)
                && e.max_darkness.is_none_or(|d| gs.region.darkness <= d)
                // seasonal one-shots fire only in their real season
                && e.requires_season.is_none_or(|s| crate::world::Season::for_day(day) == s)
                // once the whole region has fallen, no envoy, caravan, or lord
                // takes the road — the outside world is gone until it rebuilds
                && !(gs.region.all_fallen() && needs_outside_world(e))
                // story flags: every required flag set, no forbidden flag set
                && e.requires_flags.iter().all(|f| gs.flags.contains(f))
                && !e.forbids_flags.iter().any(|f| gs.flags.contains(f))
                // don't offer to "build" something the fortress already has —
                // these one-shots are framed as raising a new building.
                && !offers_existing_building(e, gs)
                && last_event_name != Some(e.name.as_str())
        })
        .collect()
}

/// Events that presume a living realm beyond the walls — trade, envoys, lords.
/// When every site has fallen there is no one left out there to send them.
fn needs_outside_world(event: &Event) -> bool {
    event.has_tag("diplomacy") || event.has_tag("trade")
}

/// True if any choice proposes raising a building the fortress already has.
fn offers_existing_building(event: &Event, gs: &GameState) -> bool {
    event.choices.iter().any(|c| {
        c.effects
            .iter()
            .any(|e| matches!(e, Effect::AddUpgrade { name } if gs.fortress.has_upgrade(*name)))
    })
}

pub fn roll<'a>(
    deck: &'a [Event],
    day: u32,
    gs: &mut GameState,
    last_event_name: Option<&str>,
) -> Option<&'a Event> {
    // Not every day brings a crisis. The chance that *something* happens climbs
    // with the darkness — quiet days are common early, vanishing as the dark
    // deepens. Rolled before deck filtering so the sim and the game agree.
    let event_chance = (55 + gs.region.darkness / 2).clamp(0, 100);
    if gs.rng.random_range(0..100) >= event_chance {
        return None; // a quiet day
    }

    let pool = eligible_events(deck, day, gs, last_event_name);
    if pool.is_empty() {
        return None;
    }
    let weights: Vec<f64> = pool.iter().map(|e| e.weight).collect();
    let idx = weighted_index(&mut gs.rng, &weights)?;
    Some(pool[idx])
}

/// Effective cost of a choice for this player (Steward discount on economy events).
pub fn effective_cost(choice: &Choice, event: &Event, player: Option<&PlayerCharacter>) -> ResourceDelta {
    let mut cost = choice.cost.clone();
    let is_steward = player.is_some_and(|p| p.class == ClassKind::Steward);
    if is_steward && event.has_tag("economy") && !cost.is_zero() {
        for v in [&mut cost.food, &mut cost.valuables, &mut cost.stone, &mut cost.wood] {
            if *v > 0 {
                *v = (*v * 4 / 5).max(1);
            }
        }
    }
    cost
}

pub fn choice_availability(
    choice: &Choice,
    event: &Event,
    gs: &GameState,
) -> ChoiceAvailability {
    if let Some(player) = &gs.player {
        for (stat, min) in &choice.requires_stat {
            if player.stats.get(*stat) < *min {
                return ChoiceAvailability::StatLocked(*stat, *min);
            }
        }
    }
    let cost = effective_cost(choice, event, gs.player.as_ref());
    if !gs.resources.can_afford(&cost) {
        return ChoiceAvailability::CantAfford;
    }
    ChoiceAvailability::Ok
}

pub fn resolve(event: &Event, choice_index: usize, gs: &mut GameState) -> EventResult {
    let choice = &event.choices[choice_index];
    let mut result = EventResult {
        event_name: event.name.clone(),
        choice_label: choice.label.clone(),
        lines: Vec::new(),
    };

    let cost = effective_cost(choice, event, gs.player.as_ref());
    if !cost.is_zero() {
        gs.resources.apply_delta(&cost.negated());
        result.lines.push(format!("Paid {}.", cost.describe_cost()));
    }

    if let Some(flavor) = &choice.flavor {
        let name = gs.player.as_ref().map(|p| p.name.clone()).unwrap_or_default();
        result.lines.push(flavor.replace("{player}", &name));
    }

    for effect in &choice.effects {
        apply_effect(effect, event, gs, &mut result);
    }

    if let Some(check) = &choice.stat_check {
        if let Some(player) = gs.player.clone() {
            let stat_value = player.stats.get(check.stat) as i32;
            let die = gs.rng.random_range(1..=6);
            let total = stat_value + die;
            let difficulty = check.difficulty;
            let success = total >= difficulty;
            result.lines.push(format!(
                "{} check: {} + {} = {} vs {} — {}",
                check.stat.name(),
                stat_value,
                die,
                total,
                difficulty,
                if success { "success!" } else { "failure." }
            ));
            let branch = if success { &check.success_effects } else { &check.failure_effects };
            for effect in branch {
                apply_effect(effect, event, gs, &mut result);
            }
        }
    }

    // Living through events is training: survivors learn from what they faced.
    for tag in &event.tags {
        for (role, skill, xp) in training_for_tag(tag) {
            for line in train_role(gs, *role, *skill, *xp) {
                result.lines.push(line);
            }
        }
    }
    // Resident heroes fight and learn alongside everyone else.
    if event.has_tag("combat") || event.has_tag("disaster") {
        for hero in &mut gs.adventurers {
            let skill = hero.class.home_skill();
            if let Some(tier) = hero.skills.train(skill, 8) {
                result.lines.push(format!(
                    "{} is now a {} {}.",
                    hero.name,
                    tier.name(),
                    skill.practitioner()
                ));
            }
        }
    }

    // A fortress that stands through battle is talked about.
    if event.has_tag("combat") {
        gs.apply_reputation_delta(1);
    }

    // Battle spends steel: arms break, arrows fly, edges dull.
    if event.has_tag("combat") && gs.resources.gear > 0 {
        let spent = gs.resources.gear.min(3);
        gs.resources.gear -= spent;
        if gs.resources.gear == 0 {
            result.lines.push("The armory stands empty.".to_string());
        }
    }

    gs.events_resolved += 1;
    result
}

/// Train every living member of a role; returns tier-up log lines.
/// A guard reaching a new Combat tier hardens the fortress (+1 defense).
pub fn train_role(gs: &mut GameState, role: Role, skill: Skill, xp: u32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut guard_tier_ups = 0;
    for i in gs.inhabitants.inhabitants.iter_mut().filter(|i| i.is_alive && i.role == role) {
        if let Some(tier) = i.skills.train(skill, xp) {
            lines.push(format!(
                "{} is now a {} {}.",
                i.name,
                tier.name(),
                skill.practitioner()
            ));
            if role == Role::Guard && skill == Skill::Combat {
                guard_tier_ups += 1;
            }
        }
    }
    for _ in 0..guard_tier_ups {
        gs.fortress.apply_defense_delta(1);
        lines.push("The watch grows harder. (+1 defense)".to_string());
    }
    lines
}

fn apply_effect(effect: &Effect, event: &Event, gs: &mut GameState, result: &mut EventResult) {
    match effect {
        Effect::Resource(delta) => {
            gs.resources.apply_delta(delta);
            result.lines.push(format!("{}.", delta.describe()));
        }

        Effect::Morale { amount } => {
            let mut amount = *amount;
            // The Shrine steadies hearts against the dark: demon-driven
            // despair softened by 25/50/75% per tier.
            if amount < 0 && event.has_tag("demon") {
                let kept = match gs.fortress.building_level(Upgrade::Shrine) {
                    0 => 4,
                    1 => 3,
                    2 => 2,
                    _ => 1,
                };
                amount = -((-amount * kept) / 4);
            }
            // A hold already at full heart wastes nothing: cheer it can't hold
            // spreads its name instead — the morale cap becomes a small, lasting
            // boon rather than a number that goes nowhere.
            let headroom = (100 - gs.fortress.morale).max(0);
            if amount > headroom {
                let overflow = amount - headroom;
                let boon = (overflow / 3).max(1);
                gs.apply_reputation_delta(boon);
                result.lines.push(format!(
                    "Spirits are already at their height — the overflowing goodwill spreads the fortress's name. (+{boon} renown)"
                ));
            }
            gs.fortress.apply_morale_delta(amount);
            result.lines.push(format!(
                "Fortress morale {}{}.",
                if amount >= 0 { "+" } else { "" },
                amount
            ));
        }

        Effect::Defense { amount } => {
            let amount = *amount;
            gs.fortress.apply_defense_delta(amount);
            result.lines.push(format!(
                "Defense {}{}.",
                if amount >= 0 { "+" } else { "" },
                amount
            ));
        }

        Effect::SpawnInhabitant { role } => {
            if gs.inhabitants.count_alive() as u32 >= gs.fortress.max_population {
                result.lines.push("The fortress is full — they move on.".to_string());
                return;
            }
            let role = role.unwrap_or_else(|| crate::inhabitants::random_arrival_role(&mut gs.rng));
            let newcomer = generate_inhabitant(role, &mut gs.rng);
            let traits = if newcomer.traits.is_empty() {
                String::new()
            } else {
                format!(
                    " ({})",
                    newcomer.traits.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")
                )
            };
            result.lines.push(format!(
                "{} the {}{} joins the fortress.",
                newcomer.name,
                newcomer.role.name(),
                traits
            ));
            gs.inhabitants.add(newcomer);
        }

        Effect::KillInhabitant { role } => {
            if let Some(name) = gs.inhabitants.random_survivor_name(&mut gs.rng, *role) {
                if let Some(victim) = gs.inhabitants.find_mut(&name) {
                    victim.is_alive = false;
                    victim.health = 0;
                    let role_name = victim.role.name();
                    result.lines.push(format!("{name} the {role_name} has died."));
                }
                gs.fortress.apply_morale_delta(-3);
                gs.apply_reputation_delta(-2);
            }
        }

        Effect::RemoveInhabitant {} => {
            // A lively Tavern (II+) gives the restless somewhere to belong:
            // half the time the would-be deserter thinks better of it.
            if gs.fortress.building_level(Upgrade::Tavern) >= 2 && gs.rng.random_range(0..2) == 0 {
                result
                    .lines
                    .push("Good cheer at the tavern keeps the restless from leaving.".to_string());
                return;
            }
            if let Some(name) = gs.inhabitants.random_non_loyal_name(&mut gs.rng) {
                let role_name = gs
                    .inhabitants
                    .find_mut(&name)
                    .map(|i| i.role.name())
                    .unwrap_or("inhabitant");
                result.lines.push(format!("{name} the {role_name} slips away in the night."));
                gs.inhabitants.remove(&name);
                gs.apply_reputation_delta(-2);
            } else {
                result.lines.push("The inhabitants stand together — no one deserts.".to_string());
            }
        }

        Effect::ApplyToRole { role, health, morale } => {
            let mut health = *health;
            if health < 0 {
                health = mitigate_damage(health, event, gs);
            }
            let deaths = gs.inhabitants.apply_to_role(*role, health, *morale);
            if health != 0 || *morale != 0 {
                let mut desc = Vec::new();
                if health != 0 {
                    desc.push(format!("{}{} health", if health > 0 { "+" } else { "" }, health));
                }
                if *morale != 0 {
                    desc.push(format!("{}{} morale", if *morale > 0 { "+" } else { "" }, morale));
                }
                result.lines.push(format!("All {}s: {}.", role.name(), desc.join(", ")));
            }
            for name in deaths {
                result.lines.push(format!("{} the {} succumbs.", name, role.name()));
                gs.fortress.apply_morale_delta(-3);
                gs.apply_reputation_delta(-2);
            }
        }

        Effect::AddUpgrade { name } => {
            let line = gs.build_upgrade(*name);
            result.lines.push(line);
        }

        Effect::Battle { power, loot_valuables } => {
            let report = crate::battle::fight_battle(*power, *loot_valuables, event, gs);
            result.lines.extend(report.lines);
        }

        Effect::SetFlag { flag } => {
            gs.flags.insert(flag.clone());
        }
        Effect::ClearFlag { flag } => {
            gs.flags.remove(flag);
        }

        Effect::GrantItem { kind, quality, enchant, artifact, name } => {
            let item = crate::items::Item {
                kind: *kind,
                quality: *quality,
                enchant: *enchant,
                condition: 100,
                artifact: *artifact,
                name: name.clone(),
            };
            let label = item.label();
            gs.items.add(item);
            result.lines.push(if *artifact {
                format!("{label} — an artifact — comes into the fortress's keeping.")
            } else {
                format!("A {label} is added to the armory.")
            });
        }

        Effect::Region { darkness, site_strength, pressure } => {
            if *darkness != 0 {
                gs.region.darkness = (gs.region.darkness + darkness).clamp(0, 100);
                result.lines.push(if *darkness < 0 {
                    "The darkness recedes, a little.".to_string()
                } else {
                    "The darkness thickens.".to_string()
                });
            }
            if *pressure != 0 {
                gs.region.portal_pressure = (gs.region.portal_pressure + pressure).max(0);
                result.lines.push(if *pressure < 0 {
                    "A portal gutters and shrinks.".to_string()
                } else {
                    "The portals yawn wider.".to_string()
                });
            }
            if *site_strength != 0 {
                if let Some(name) = gs.region.adjust_random_site(&mut gs.rng, *site_strength) {
                    result.lines.push(if *site_strength > 0 {
                        format!("Your aid reaches {name} — they fight on.")
                    } else {
                        format!("{name} bears the cost.")
                    });
                }
            }
        }
    }
}

/// Traits, upgrades, and abilities soften incoming damage based on event tags.
/// Integer math: 25% steps via -(-h*3//4), 50% via -(-h//2).
pub(crate) fn mitigate_damage(health: i32, event: &Event, gs: &GameState) -> i32 {
    let mut h = health;
    // Demons strike harder as the darkness deepens (h is negative here).
    if event.has_tag("demon") {
        match gs.region.band() {
            crate::region::DarknessBand::Deep => h += h / 4,
            crate::region::DarknessBand::Overwhelming => h += h / 2,
            _ => {}
        }
    }
    if event.has_tag("combat") {
        if gs.fortress.has_upgrade(Upgrade::Blacksmith) {
            h = -((-h * 3) / 4);
        }
        // A master forge (tier III) turns even demon steel.
        if gs.fortress.building_level(Upgrade::Blacksmith) >= 3 {
            h = -((-h * 3) / 4);
        }
        if gs.inhabitants.get_by_role(Role::Guard).iter().any(|i| i.has_trait(Trait::Brave)) {
            h = -((-h * 3) / 4);
        }
        // A veteran on the walls: best guard at Skilled+ softens the blow.
        if gs
            .inhabitants
            .get_by_role(Role::Guard)
            .iter()
            .any(|i| i.skills.tier(Skill::Combat) >= SkillTier::Skilled)
        {
            h = -((-h * 3) / 4);
        }
        // Shield of the Walls: a seasoned knight holds the line.
        if gs.adventurers.iter().any(|a| {
            a.class == crate::adventurers::AdventurerClass::Knight
                && a.perk_tier() >= SkillTier::Skilled
        }) {
            h = -((-h * 3) / 4);
        }
        // A well-stocked armory: good gear turns blades.
        if gs.resources.band(crate::resources::ResourceKind::Gear)
            >= crate::resources::StockBand::Adequate
        {
            h = -((-h * 3) / 4);
        }
        // Proper armor in the racks: a fine harness (or better) turns a blow.
        if gs.items.best_rating(crate::items::ItemKind::Armor) >= 3 {
            h = -((-h * 3) / 4);
        }
        // A Warlord commander steadies the line; mitigation scales with their
        // own Combat tier rather than a fixed talent.
        if let Some(p) = gs.player.as_ref() {
            if p.class == ClassKind::Warlord && p.skills.tier(Skill::Combat) >= SkillTier::Skilled {
                h = -((-h * 3) / 4);
            }
        }
    }
    if event.has_tag("disaster") && gs.fortress.has_upgrade(Upgrade::Infirmary) {
        h = -((-h) / 2);
    }
    h
}
