use fortress_core::*;
use rand::SeedableRng;

fn test_state() -> GameState {
    let mut gs = GameState::new(1);
    gs.fortress.name = "Test".to_string();
    gs.resources.apply_delta(&ResourceDelta { food: 50, valuables: 50, ..Default::default() });
    gs
}

fn guard(name: &str) -> Inhabitant {
    Inhabitant::new(name, Role::Guard)
}

/// Drive every queued build straight to completion (materials already paid by
/// `construct`), isolating construction from the rest of the daily tick.
fn finish_projects(gs: &mut GameState) {
    loop {
        let done = gs.fortress.advance_projects(1000);
        if done.is_empty() {
            break;
        }
        for u in done {
            gs.build_upgrade(u);
        }
    }
}

fn make_event(choices: Vec<Choice>, tags: Vec<&str>) -> Event {
    serde_json::from_value(serde_json::json!({
        "name": "Test Event",
        "description": "desc",
        "choices": [],
        "tags": tags,
    }))
    .map(|mut e: Event| {
        e.choices = choices;
        e
    })
    .unwrap()
}

fn simple_choice(effects: Vec<Effect>) -> Choice {
    Choice {
        label: "Go".to_string(),
        description: String::new(),
        effects,
        cost: ResourceDelta::default(),
        requires_stat: Default::default(),
        stat_check: None,
        flavor: None,
    }
}

// ----------------------------------------------------------------------
// resources
// ----------------------------------------------------------------------

#[test]
fn resources_apply_and_clamp() {
    let mut r = Resources { food: 3, ..Default::default() };
    r.apply_delta(&ResourceDelta { food: -10, valuables: 5, ..Default::default() });
    assert_eq!(r.food, 0);
    assert_eq!(r.valuables, 5);
}

#[test]
fn resources_can_afford() {
    let r = Resources { food: 10, valuables: 5, ..Default::default() };
    assert!(r.can_afford(&ResourceDelta { food: 10, valuables: 5, ..Default::default() }));
    assert!(!r.can_afford(&ResourceDelta { valuables: 6, ..Default::default() }));
    assert!(r.can_afford(&ResourceDelta::default()));
}

// ----------------------------------------------------------------------
// fortress
// ----------------------------------------------------------------------

#[test]
fn morale_clamps_and_defeat_at_zero() {
    let mut f = Fortress::new("T");
    f.apply_morale_delta(100);
    assert_eq!(f.morale, 100);
    f.apply_morale_delta(-150);
    assert_eq!(f.morale, 0);
    assert!(f.is_defeated());
}

#[test]
fn defense_never_negative() {
    let mut f = Fortress::new("T");
    f.apply_defense_delta(-99);
    assert_eq!(f.defense, 0);
}

#[test]
fn upgrades_no_duplicates() {
    let mut f = Fortress::new("T");
    f.add_building(Upgrade::Farm);
    f.add_building(Upgrade::Farm);
    assert_eq!(f.buildings.len(), 1);
}

// ----------------------------------------------------------------------
// inhabitants
// ----------------------------------------------------------------------

#[test]
fn sickly_takes_double_damage() {
    let mut i = guard("G");
    i.traits.push(Trait::Sickly);
    i.damage(10);
    assert_eq!(i.health, 80);
}

#[test]
fn damage_kills_at_zero() {
    let mut i = guard("G");
    i.health = 10;
    i.damage(10);
    assert!(!i.is_alive);
}

#[test]
fn apply_to_role_reports_deaths() {
    let mut m = InhabitantManager::default();
    let mut doomed = guard("Doomed");
    doomed.health = 5;
    m.add(doomed);
    m.add(guard("Tough"));
    let deaths = m.apply_to_role(Role::Guard, -10, 0);
    assert_eq!(deaths, vec!["Doomed".to_string()]);
    assert_eq!(m.count_alive(), 1);
    assert_eq!(m.count_dead(), 1);
}

#[test]
fn random_non_loyal_skips_loyal() {
    let mut m = InhabitantManager::default();
    let mut loyal = guard("Loyal");
    loyal.traits.push(Trait::Loyal);
    m.add(loyal);
    let mut rng = GameRng::seed_from_u64(0);
    assert!(m.random_non_loyal_name(&mut rng).is_none());
    m.add(guard("Flighty"));
    assert_eq!(m.random_non_loyal_name(&mut rng).unwrap(), "Flighty");
}

#[test]
fn average_morale_defaults_and_floors() {
    let mut m = InhabitantManager::default();
    assert_eq!(m.average_morale(), 50);
    let mut a = guard("A");
    a.morale = 20;
    let mut b = guard("B");
    b.morale = 41;
    m.add(a);
    m.add(b);
    assert_eq!(m.average_morale(), 30); // floor(61/2)
}

#[test]
fn generate_inhabitant_deterministic_per_seed() {
    let a = generate_inhabitant(Role::Healer, &mut GameRng::seed_from_u64(7));
    let b = generate_inhabitant(Role::Healer, &mut GameRng::seed_from_u64(7));
    assert_eq!(a, b);
}

// ----------------------------------------------------------------------
// engine: eligibility
// ----------------------------------------------------------------------

#[test]
fn eligibility_filters() {
    let gs = test_state();
    let mut e = make_event(vec![simple_choice(vec![])], vec![]);

    e.min_day = 5;
    assert!(eligible_events(std::slice::from_ref(&e), 3, &gs, None).is_empty());
    assert_eq!(eligible_events(std::slice::from_ref(&e), 5, &gs, None).len(), 1);

    e.min_day = 1;
    e.min_morale = 60;
    assert!(eligible_events(std::slice::from_ref(&e), 1, &gs, None).is_empty());

    e.min_morale = 0;
    e.min_resource = ResourceDelta { valuables: 999, ..Default::default() };
    assert!(eligible_events(std::slice::from_ref(&e), 1, &gs, None).is_empty());

    e.min_resource = ResourceDelta::default();
    e.requires_role = Some(Role::Healer);
    assert!(eligible_events(std::slice::from_ref(&e), 1, &gs, None).is_empty());

    e.requires_role = None;
    e.requires_upgrade = Some(Upgrade::Watchtower);
    assert!(eligible_events(std::slice::from_ref(&e), 1, &gs, None).is_empty());

    e.requires_upgrade = None;
    assert!(eligible_events(std::slice::from_ref(&e), 1, &gs, Some("Test Event")).is_empty());
}

// ----------------------------------------------------------------------
// engine: effects
// ----------------------------------------------------------------------

fn resolve_single(gs: &mut GameState, effect: Effect, tags: Vec<&str>) -> EventResult {
    let event = make_event(vec![simple_choice(vec![effect])], tags);
    resolve(&event, 0, gs)
}

#[test]
fn resource_morale_defense_effects() {
    let mut gs = test_state();
    resolve_single(&mut gs, Effect::Resource(ResourceDelta { food: 5, valuables: -10, ..Default::default() }), vec![]);
    assert_eq!(gs.resources.food, 55);
    assert_eq!(gs.resources.valuables, 40);

    resolve_single(&mut gs, Effect::Morale { amount: -10 }, vec![]);
    assert_eq!(gs.fortress.morale, 40);

    resolve_single(&mut gs, Effect::Defense { amount: 3 }, vec![]);
    assert_eq!(gs.fortress.defense, 13);
}

#[test]
fn spawn_respects_max_population() {
    let mut gs = test_state();
    gs.fortress.max_population = 0;
    resolve_single(&mut gs, Effect::SpawnInhabitant { role: None }, vec![]);
    assert_eq!(gs.inhabitants.count_alive(), 0);

    gs.fortress.max_population = 5;
    resolve_single(&mut gs, Effect::SpawnInhabitant { role: Some(Role::Guard) }, vec![]);
    assert!(gs.inhabitants.has_role(Role::Guard));
}

#[test]
fn kill_inhabitant_applies_morale_penalty() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("Victim"));
    let morale_before = gs.fortress.morale;
    resolve_single(&mut gs, Effect::KillInhabitant { role: None }, vec![]);
    assert_eq!(gs.inhabitants.count_alive(), 0);
    assert_eq!(gs.fortress.morale, morale_before - 3);
}

#[test]
fn add_upgrade_applies_immediate_bonus() {
    let mut gs = test_state();
    let base = gs.fortress.defense;
    resolve_single(&mut gs, Effect::AddUpgrade { name: Upgrade::Watchtower }, vec![]);
    assert!(gs.fortress.has_upgrade(Upgrade::Watchtower));
    assert_eq!(gs.fortress.defense, base + 5);
}

#[test]
fn choice_cost_is_paid() {
    let mut gs = test_state();
    let mut choice = simple_choice(vec![]);
    choice.cost = ResourceDelta { valuables: 20, ..Default::default() };
    let event = make_event(vec![choice], vec![]);
    resolve(&event, 0, &mut gs);
    assert_eq!(gs.resources.valuables, 30);
}

// ----------------------------------------------------------------------
// engine: mitigation parity with Python integer math
// ----------------------------------------------------------------------

#[test]
fn blacksmith_mitigates_combat_damage() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Blacksmith);
    gs.inhabitants.add(guard("G"));
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    // -(-(-20)*3/4) = -15
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 85);
}

#[test]
fn infirmary_halves_disaster_damage() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Infirmary);
    gs.inhabitants.add(Inhabitant::new("F", Role::Farmer));
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Farmer, health: -20, morale: 0 },
        vec!["disaster"],
    );
    assert_eq!(gs.inhabitants.get_by_role(Role::Farmer)[0].health, 90);
}

#[test]
fn mitigation_python_parity_on_odd_values() {
    // Python: -(-(-15) * 3 // 4) = -11; -(-(-15) // 2) = -7 (floor == trunc for positives)
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Blacksmith);
    gs.inhabitants.add(guard("G"));
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -15, morale: 0 },
        vec!["combat"],
    );
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 100 - 11);
}

// ----------------------------------------------------------------------
// game state: daily tick
// ----------------------------------------------------------------------

#[test]
fn daily_farm_yield() {
    let mut gs = test_state();
    gs.resources.food = 30; // below the granary-less cap, so nothing spoils
    gs.fortress.add_building(Upgrade::Farm);
    gs.apply_daily_effects();
    assert_eq!(gs.resources.food, 33); // Farm I base harvest of 3
}

#[test]
fn daily_upkeep_rounds_up() {
    let mut gs = test_state();
    for n in 0..5 {
        gs.inhabitants.add(guard(&format!("G{n}")));
    }
    gs.apply_daily_effects();
    assert_eq!(gs.resources.food, 47); // (5+1)/2 = 3
}

#[test]
fn starvation_drains_morale() {
    let mut gs = test_state();
    gs.resources.food = 0;
    gs.inhabitants.add(guard("Hungry"));
    gs.apply_daily_effects();
    // -5 starvation, +1 sleeps-warm (1 inhabitant, plenty of beds)
    assert_eq!(gs.fortress.morale, 46);
}

#[test]
fn morale_cascade_bands() {
    let mut gs = test_state();
    let mut happy = guard("Happy");
    happy.morale = 90;
    gs.inhabitants.add(happy);
    gs.apply_daily_effects();
    // +2 cascade, +1 sleeps-warm
    assert_eq!(gs.fortress.morale, 53);

    let mut gs2 = test_state();
    let mut sad = guard("Sad");
    sad.morale = 10;
    gs2.inhabitants.add(sad);
    gs2.apply_daily_effects();
    // -2 cascade, +1 sleeps-warm
    assert_eq!(gs2.fortress.morale, 49);
}

#[test]
fn sleeping_capacity_and_rough_sleepers() {
    let mut gs = test_state();
    assert_eq!(gs.fortress.sleeping_capacity(), 6);
    gs.fortress.add_building(Upgrade::Barracks);
    assert_eq!(gs.fortress.sleeping_capacity(), 16); // the Barracks sleeps a crowd

    // 20 inhabitants vs 16 beds: the 4 in iteration overflow sleep rough.
    for n in 0..20 {
        let mut g = guard(&format!("G{n}"));
        g.morale = 50;
        gs.inhabitants.add(g);
    }
    let morale_before = gs.fortress.morale;
    gs.apply_daily_effects();
    let rough: Vec<i32> = gs.inhabitants.inhabitants.iter().skip(16).map(|i| i.morale).collect();
    assert!(rough.iter().all(|&m| m < 50), "rough sleepers lose morale: {rough:?}");
    assert!(gs.inhabitants.inhabitants.iter().take(16).all(|i| i.morale >= 50));
    // no +1 warm-sleep when over capacity
    assert!(gs.fortress.morale <= morale_before);
}

#[test]
fn fortress_survives_past_day_30() {
    // No win condition — the fortress must keep running past what used to be the victory day.
    let mut gs = test_state();
    gs.fortress.day = 30;
    assert!(!gs.is_game_over());
    gs.fortress.advance_day();
    assert!(!gs.is_game_over());
    assert_eq!(gs.fortress.day, 31);
}

// ----------------------------------------------------------------------
// player: stats, gating, checks, passives
// ----------------------------------------------------------------------

fn player_with(might: u8, class: ClassKind) -> PlayerCharacter {
    PlayerCharacter::new("Hero", class, Stats { might, wit: 3, heart: 3 })
}

#[test]
fn stat_gating_locks_choice() {
    let mut gs = test_state();
    gs.player = Some(player_with(3, ClassKind::Warlord));
    let mut choice = simple_choice(vec![]);
    choice.requires_stat.insert(StatKind::Might, 6);
    let event = make_event(vec![choice], vec![]);
    assert_eq!(
        choice_availability(&event.choices[0], &event, &gs),
        ChoiceAvailability::StatLocked(StatKind::Might, 6)
    );
    gs.player = Some(player_with(6, ClassKind::Warlord));
    assert_eq!(
        choice_availability(&event.choices[0], &event, &gs),
        ChoiceAvailability::Ok
    );
}

#[test]
fn stat_check_applies_branch_effects() {
    let mut gs = test_state();
    gs.player = Some(player_with(8, ClassKind::Warlord)); // 8 + d6 >= 2 always succeeds
    let mut choice = simple_choice(vec![]);
    choice.stat_check = Some(StatCheck {
        stat: StatKind::Might,
        difficulty: 2,
        success_effects: vec![Effect::Morale { amount: 10 }],
        failure_effects: vec![Effect::Morale { amount: -10 }],
    });
    let event = make_event(vec![choice], vec![]);
    let result = resolve(&event, 0, &mut gs);
    assert_eq!(gs.fortress.morale, 60);
    assert!(result.lines.iter().any(|l| l.contains("success")));
}

#[test]
fn steward_discount_on_economy_costs() {
    let mut gs = test_state();
    gs.player = Some(player_with(3, ClassKind::Steward));
    let mut choice = simple_choice(vec![]);
    choice.cost = ResourceDelta { valuables: 10, ..Default::default() };
    let event = make_event(vec![choice], vec!["economy"]);
    resolve(&event, 0, &mut gs);
    assert_eq!(gs.resources.valuables, 42); // paid 8, not 10
}

#[test]
fn warlord_extra_combat_mitigation() {
    let mut gs = test_state();
    gs.player = Some(player_with(8, ClassKind::Warlord));
    gs.inhabitants.add(guard("G"));
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 85);
}

// ----------------------------------------------------------------------
// skills: tiers, training, effects
// ----------------------------------------------------------------------

#[test]
fn skill_tier_thresholds() {
    assert_eq!(tier_for_xp(0), SkillTier::Dabbling);
    assert_eq!(tier_for_xp(19), SkillTier::Dabbling);
    assert_eq!(tier_for_xp(20), SkillTier::Novice);
    assert_eq!(tier_for_xp(90), SkillTier::Skilled);
    assert_eq!(tier_for_xp(400), SkillTier::Legendary);
    assert_eq!(tier_for_xp(9999), SkillTier::Legendary);
}

#[test]
fn train_reports_tier_up_only_on_crossing() {
    let mut s = SkillSet::default();
    assert_eq!(s.train(Skill::Combat, 10), None);
    assert_eq!(s.train(Skill::Combat, 10), Some(SkillTier::Novice));
    assert_eq!(s.train(Skill::Combat, 5), None);
    // xp caps at the legendary threshold
    s.train(Skill::Combat, 100_000);
    assert_eq!(s.xp(Skill::Combat), 400);
}

#[test]
fn combat_event_trains_guards() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("G"));
    let before = gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat);
    resolve_single(
        &mut gs,
        Effect::Morale { amount: 0 },
        vec!["combat"],
    );
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), before + 8);
}

#[test]
fn guard_combat_tier_up_grants_defense() {
    let mut gs = test_state();
    let mut g = guard("G");
    g.skills.train(Skill::Combat, 19); // 1 xp short of Novice
    gs.inhabitants.add(g);
    let def_before = gs.fortress.defense;
    resolve_single(&mut gs, Effect::Morale { amount: 0 }, vec!["combat"]);
    assert_eq!(gs.fortress.defense, def_before + 1);
}

#[test]
fn daily_practice_requires_workplace() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("G"));
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), 0);

    gs.fortress.add_building(Upgrade::Barracks);
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), 2);
}

#[test]
fn skilled_farmers_raise_harvest() {
    let mut gs = test_state();
    gs.resources.food = 30; // below the granary-less cap, so nothing spoils
    gs.fortress.add_building(Upgrade::Farm);
    let mut f = Inhabitant::new("F", Role::Farmer);
    f.skills.train(Skill::Farming, 140); // Proficient, index 4
    gs.inhabitants.add(f);
    let food_before = gs.resources.food;
    gs.apply_daily_effects();
    // Farm I base 3 + field_hands 2 (one farmer) + skill 4/2=2 = 7, minus 1 upkeep
    assert_eq!(gs.resources.food, food_before + 7 - 1);
}

#[test]
fn healers_tend_the_weakest() {
    let mut gs = test_state();
    let mut h = Inhabitant::new("H", Role::Healer);
    h.skills.train(Skill::Medicine, 50); // Competent, index 2 -> heals 4
    gs.inhabitants.add(h);
    let mut hurt = guard("Hurt");
    hurt.health = 40;
    gs.inhabitants.add(hurt);
    gs.apply_daily_effects();
    let healed = gs.inhabitants.inhabitants.iter().find(|i| i.name == "Hurt").unwrap();
    assert_eq!(healed.health, 44);
}

#[test]
fn veteran_guard_mitigates_combat() {
    let mut gs = test_state();
    let mut g = guard("Vet");
    g.skills.train(Skill::Combat, 90); // Skilled
    gs.inhabitants.add(g);
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    // one 25% step: -20 -> -15
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 85);
}

// ----------------------------------------------------------------------
// construction: costs, gating, founding grants
// ----------------------------------------------------------------------

#[test]
fn construct_pays_materials_and_builds() {
    let mut gs = test_state();
    gs.resources.apply_delta(&ResourceDelta { wood: 20, stone: 20, ..Default::default() });
    let wood_before = gs.resources.wood;
    let stone_before = gs.resources.stone;
    assert_eq!(gs.build_availability(Upgrade::Watchtower), BuildAvailability::Ok);
    // breaking ground pays the materials up front but only enqueues the labor
    gs.construct(Upgrade::Watchtower).expect("buildable");
    let cost = Upgrade::Watchtower.build_cost(1);
    assert_eq!(gs.resources.wood, wood_before - cost.wood);
    assert_eq!(gs.resources.stone, stone_before - cost.stone);
    assert!(!gs.fortress.has_upgrade(Upgrade::Watchtower), "not raised until the labor is done");
    assert_eq!(gs.build_availability(Upgrade::Watchtower), BuildAvailability::InProgress);
    // the workforce finishes it
    finish_projects(&mut gs);
    assert!(gs.fortress.has_upgrade(Upgrade::Watchtower));
    assert_eq!(gs.fortress.building_level(Upgrade::Watchtower), 1);
    // building it again tiers it up rather than duplicating
    gs.resources.apply_delta(&ResourceDelta { wood: 99, stone: 99, ..Default::default() });
    gs.construct(Upgrade::Watchtower).expect("upgradeable to II");
    finish_projects(&mut gs);
    assert_eq!(gs.fortress.building_level(Upgrade::Watchtower), 2);
    assert_eq!(gs.fortress.buildings.len(), 1);
}

#[test]
fn construct_requires_the_specialist() {
    let mut gs = test_state();
    gs.resources.apply_delta(&ResourceDelta { wood: 99, stone: 99, ..Default::default() });
    // no blacksmith lives here yet
    assert_eq!(
        gs.build_availability(Upgrade::Blacksmith),
        BuildAvailability::MissingRole(Role::Blacksmith)
    );
    assert!(gs.construct(Upgrade::Blacksmith).is_err());
    gs.inhabitants.add(Inhabitant::new("Smith", Role::Blacksmith));
    assert_eq!(gs.build_availability(Upgrade::Blacksmith), BuildAvailability::Ok);
    gs.construct(Upgrade::Blacksmith).expect("buildable with a smith");
}

#[test]
fn construct_blocked_without_materials() {
    let mut gs = test_state(); // 50 food, 50 valuables, no wood/stone
    assert_eq!(gs.build_availability(Upgrade::Watchtower), BuildAvailability::CantAfford);
    assert!(gs.construct(Upgrade::Watchtower).is_err());
    assert!(!gs.fortress.has_upgrade(Upgrade::Watchtower));
}

#[test]
fn a_build_consumes_worker_days_before_finishing() {
    let mut gs = test_state();
    gs.resources.apply_delta(&ResourceDelta { wood: 30, stone: 30, ..Default::default() });
    // a lone hold with no laborers musters a workforce of 1 a day
    assert_eq!(gs.build_workforce(), 1);
    gs.construct(Upgrade::Watchtower).expect("buildable"); // 4 worker-days
    // one day's labor is not enough
    let done = gs.fortress.advance_projects(gs.build_workforce());
    assert!(done.is_empty());
    assert!(!gs.fortress.has_upgrade(Upgrade::Watchtower));
    // a crew of laborers finishes it far faster
    for n in ["A", "B", "C", "D"] {
        gs.inhabitants.add(Inhabitant::new(n, Role::Peasant));
    }
    assert_eq!(gs.build_workforce(), 5); // 4 peasants + baseline
    let done = gs.fortress.advance_projects(gs.build_workforce());
    assert_eq!(done, vec![Upgrade::Watchtower]);
    gs.build_upgrade(Upgrade::Watchtower);
    assert!(gs.fortress.has_upgrade(Upgrade::Watchtower));
    assert!(gs.fortress.projects.is_empty());
}

#[test]
fn a_renowned_hold_earns_one_lasting_feature() {
    let mut gs = test_state();
    gs.reputation = 60; // a true name made
    // drive enough days that the low daily roll lands at least once
    let mut granted = false;
    for _ in 0..200 {
        if gs.maybe_grant_feature().is_some() {
            granted = true;
            break;
        }
    }
    assert!(granted, "a renowned hold should eventually earn a feature");
    assert_eq!(gs.fortress.features.len(), 1);
    // never a second one, however long it stands
    for _ in 0..200 {
        gs.maybe_grant_feature();
    }
    assert_eq!(gs.fortress.features.len(), 1, "at most one feature a run");
}

#[test]
fn deep_cellars_deepen_the_larder() {
    use fortress_core::FortressFeature;
    let mut gs = test_state();
    gs.inhabitants.add(Inhabitant::new("F", Role::Farmer));
    gs.resources.food = 200; // well over the base 60 cap
    gs.apply_daily_effects();
    let capped_plain = gs.resources.food;
    // with the Deep Cellars, far more grain survives the spoilage sweep
    let mut gs2 = test_state();
    gs2.inhabitants.add(Inhabitant::new("F", Role::Farmer));
    gs2.fortress.features.push(FortressFeature::DeepCellars);
    gs2.resources.food = 200;
    gs2.apply_daily_effects();
    assert!(gs2.resources.food > capped_plain);
}

#[test]
fn a_miner_draws_more_from_the_seam_than_a_peasant() {
    let base = |role: Role| {
        let mut gs = test_state();
        gs.fortress.add_building(Upgrade::Mine); // tier I: 3 stone, 2 ore
        gs.inhabitants.add(Inhabitant::new("W", role));
        let before = gs.resources.stone;
        gs.apply_daily_effects();
        gs.resources.stone - before
    };
    assert!(base(Role::Miner) > base(Role::Peasant), "a miner outdigs a peasant filling in");
}

#[test]
fn founding_grant_is_free_and_applies_bonuses() {
    let mut gs = test_state(); // no wood/stone — couldn't pay for it
    let pop_before = gs.fortress.max_population;
    let food_before = gs.resources.food;
    gs.build_upgrade(Upgrade::Housing);
    assert!(gs.fortress.has_upgrade(Upgrade::Housing));
    assert_eq!(gs.fortress.max_population, pop_before + 5);
    assert_eq!(gs.resources.food, food_before); // charter grant, not purchase
}

#[test]
fn housing_adds_beds_and_population() {
    let mut gs = test_state();
    let pop_before = gs.fortress.max_population;
    assert_eq!(gs.fortress.sleeping_capacity(), 6);
    gs.build_upgrade(Upgrade::Housing);
    assert_eq!(gs.fortress.sleeping_capacity(), 12); // +6 beds per plot
    assert_eq!(gs.fortress.max_population, pop_before + 5);
}

#[test]
fn tavern_lifts_morale_daily() {
    let mut gs = test_state();
    gs.build_upgrade(Upgrade::Tavern);
    let morale_before = gs.fortress.morale;
    gs.apply_daily_effects();
    // Tavern I cheers the hold by +1 morale a day
    assert!(gs.fortress.morale >= morale_before + 1);
}

#[test]
fn mine_yields_stone() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Mine);
    let before = gs.resources.stone;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.stone, before + 3); // Mine I
}

#[test]
fn build_events_skip_already_built_buildings() {
    let mut gs = test_state();
    let deck =
        vec![make_event(vec![simple_choice(vec![Effect::AddUpgrade { name: Upgrade::Tavern }])], vec![])];
    // offered while the building doesn't exist
    assert_eq!(eligible_events(&deck, 5, &gs, None).len(), 1);
    gs.fortress.add_building(Upgrade::Tavern);
    // suppressed once it's standing (no "build what we already have")
    assert!(eligible_events(&deck, 5, &gs, None).is_empty());
}

#[test]
fn lumberyard_yields_wood_by_tier() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Lumberyard); // I
    let before = gs.resources.wood;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.wood, before + 2);
    gs.fortress.add_building(Upgrade::Lumberyard); // II
    let before = gs.resources.wood;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.wood, before + 3);
    gs.fortress.add_building(Upgrade::Lumberyard); // III
    let before = gs.resources.wood;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.wood, before + 5);
}

#[test]
fn training_yard_drills_the_guard() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::TrainingYard);
    gs.inhabitants.add(guard("G"));
    gs.apply_daily_effects();
    // Training Yard I gives the guard +2 Combat practice
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), 2);
}

#[test]
fn workshop_trains_crafting_at_tier_two() {
    let mut gs = test_state();
    // tier I trains no crafting; only a working bench (II+) does
    gs.fortress.add_building(Upgrade::Workshop); // I
    gs.inhabitants.add(guard("G"));
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Crafting), 0);
    gs.fortress.add_building(Upgrade::Workshop); // II
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Crafting), 1);
}

#[test]
fn shrine_softens_demon_dread() {
    // a demon-tagged morale loss, with and without a shrine
    let demon_hit = || {
        let event = make_event(
            vec![simple_choice(vec![Effect::Morale { amount: -8 }])],
            vec!["demon"],
        );
        event
    };
    let mut bare = test_state();
    let before = bare.fortress.morale;
    resolve(&demon_hit(), 0, &mut bare);
    let bare_loss = before - bare.fortress.morale;

    let mut warded = test_state();
    warded.fortress.add_building(Upgrade::Shrine); // I softens 25%
    warded.fortress.add_building(Upgrade::Shrine); // II softens 50%
    let before = warded.fortress.morale;
    resolve(&demon_hit(), 0, &mut warded);
    let warded_loss = before - warded.fortress.morale;

    assert_eq!(bare_loss, 8);
    assert_eq!(warded_loss, 4); // -((8*2)/4)
}

// ----------------------------------------------------------------------
// the mortal commander
// ----------------------------------------------------------------------

fn with_commander(class: ClassKind) -> GameState {
    let mut gs = test_state();
    gs.player = Some(PlayerCharacter::new("Cmd", class, Stats::default()));
    gs
}

#[test]
fn commander_eats_too() {
    let mut bare = test_state(); // no commander, no inhabitants -> no upkeep
    bare.apply_daily_effects();
    assert_eq!(bare.resources.food, 50);

    let mut led = with_commander(ClassKind::Warlord); // one mouth
    led.apply_daily_effects();
    assert_eq!(led.resources.food, 49); // (1+1)/2 = 1 upkeep
}

#[test]
fn fallen_commander_ends_the_run() {
    let mut gs = with_commander(ClassKind::Warlord);
    assert!(!gs.is_game_over());
    gs.player.as_mut().unwrap().damage(100);
    assert!(gs.commander_has_fallen());
    assert!(gs.is_game_over());
}

#[test]
fn most_arrivals_are_common_folk() {
    let mut rng = fortress_core::GameRng::seed_from_u64(1);
    let peasants = (0..500)
        .filter(|_| fortress_core::inhabitants::random_arrival_role(&mut rng) == Role::Peasant)
        .count();
    assert!(peasants > 200, "peasants should dominate arrivals: {peasants}/500");
}

#[test]
fn peasants_learn_a_trade_over_time() {
    let mut gs = test_state();
    gs.resources.food = 500; // keep them fed across many days
    let mut p = Inhabitant::new("Pat", Role::Peasant);
    p.skills.train(Skill::Farming, 40); // shows a farmer's aptitude
    gs.inhabitants.add(p);
    let mut became = None;
    for _ in 0..200 {
        gs.apply_daily_effects();
        let role = gs.inhabitants.inhabitants[0].role;
        if role != Role::Peasant {
            became = Some(role);
            break;
        }
    }
    assert_eq!(became, Some(Role::Farmer));
}

#[test]
fn classes_start_with_their_skill_profile() {
    // Wizard is selectable and starts a trained caster, not a fighter.
    let wiz = PlayerCharacter::new("W", ClassKind::Wizard, Stats::default());
    assert!(wiz.skills.tier(Skill::Sorcery) >= SkillTier::Skilled);
    assert!(wiz.skills.tier(Skill::Warding) >= SkillTier::Competent);
    assert!(wiz.class.is_mage());

    // Warlord is a fighter, no magic.
    let war = PlayerCharacter::new("A", ClassKind::Warlord, Stats::default());
    assert!(war.skills.tier(Skill::Combat) >= SkillTier::Proficient);
    assert_eq!(war.skills.xp(Skill::Sorcery), 0);
    assert!(!war.class.is_mage());

    // every class is constructible and seeds at least one skill
    for class in ClassKind::ALL {
        let p = PlayerCharacter::new("P", class, Stats::default());
        assert!(Skill::ALL.iter().any(|s| p.skills.xp(*s) > 0), "{:?} has no skills", class);
    }
}

#[test]
fn commander_drills_their_home_skill() {
    let mut gs = with_commander(ClassKind::Steward); // home skill Crafting
    let before = gs.player.as_ref().unwrap().skills.xp(Skill::Crafting);
    gs.apply_daily_effects();
    // drills home skill by +2/day on top of the class's starting xp
    assert_eq!(gs.player.as_ref().unwrap().skills.xp(Skill::Crafting), before + 2);
}

#[test]
fn healers_tend_a_wounded_commander() {
    let mut gs = with_commander(ClassKind::Warlord);
    gs.player.as_mut().unwrap().damage(60); // commander at 40, most wounded
    let mut healer = Inhabitant::new("H", Role::Healer);
    healer.skills.train(Skill::Medicine, 50); // Competent -> heals 4
    gs.inhabitants.add(healer);
    gs.fortress.add_building(Upgrade::Infirmary);
    gs.apply_daily_effects();
    assert!(gs.player.as_ref().unwrap().health > 40);
}

// ----------------------------------------------------------------------
// battle reports
// ----------------------------------------------------------------------

fn battle_event(tags: Vec<&str>) -> Event {
    make_event(vec![], tags)
}

#[test]
fn battle_is_deterministic_per_seed() {
    let run = || {
        let mut gs = GameState::new(7);
        gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats::default()));
        for n in 0..4 {
            gs.inhabitants.add(guard(&format!("G{n}")));
        }
        let ev = make_event(
            vec![simple_choice(vec![Effect::Battle { power: 15, loot_valuables: 2 }])],
            vec!["combat"],
        );
        resolve(&ev, 0, &mut gs).lines
    };
    assert_eq!(run(), run());
}

#[test]
fn strong_garrison_routs_a_weak_foe() {
    let mut gs = test_state();
    gs.player =
        Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats { might: 8, wit: 3, heart: 3 }));
    let mut vet = guard("Vet");
    vet.skills.train(Skill::Combat, 300); // a hardened veteran
    gs.inhabitants.add(vet);
    let valuables_before = gs.resources.valuables;
    let report = fight_battle(3, 5, &battle_event(vec!["combat"]), &mut gs);
    assert!(report.victory);
    assert_eq!(gs.resources.valuables, valuables_before + 5); // looted on victory
}

#[test]
fn outmatched_garrison_is_overrun() {
    let mut gs = test_state(); // no commander, no guards
    let def_before = gs.fortress.defense;
    let morale_before = gs.fortress.morale;
    let report = fight_battle(40, 0, &battle_event(vec!["combat"]), &mut gs);
    assert!(!report.victory);
    assert!(gs.fortress.defense < def_before);
    assert!(gs.fortress.morale < morale_before);
}

#[test]
fn demon_battles_are_deadlier_in_deep_darkness() {
    let win = |seed: u64, darkness: i32| {
        let mut gs = GameState::new(seed);
        gs.player = Some(PlayerCharacter::new(
            "Cmd",
            ClassKind::Warlord,
            Stats { might: 6, wit: 3, heart: 3 },
        ));
        for n in 0..3 {
            let mut g = guard(&format!("G{n}"));
            g.skills.train(Skill::Combat, 150);
            gs.inhabitants.add(g);
        }
        gs.region.darkness = darkness;
        fight_battle(16, 0, &battle_event(vec!["combat", "demon"]), &mut gs).victory
    };
    let wins_quiet = (0..200u64).filter(|s| win(*s, 0)).count();
    let wins_deep = (0..200u64).filter(|s| win(*s, 90)).count();
    assert!(wins_deep < wins_quiet, "deep darkness should cost victories: {wins_deep} vs {wins_quiet}");
}

#[test]
fn a_catastrophic_battle_can_fell_the_commander() {
    let fell = (0..100u64).any(|seed| {
        let mut gs = GameState::new(seed);
        gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Mystic, Stats::default()));
        for _ in 0..6 {
            if gs.commander_has_fallen() {
                break;
            }
            fight_battle(60, 0, &battle_event(vec!["combat"]), &mut gs);
        }
        gs.commander_has_fallen()
    });
    assert!(fell, "a lone commander should eventually fall to an overwhelming foe");
}

// ----------------------------------------------------------------------
// story flags & event chains
// ----------------------------------------------------------------------

fn flagged_event(name: &str, requires: Vec<&str>, forbids: Vec<&str>) -> Event {
    let mut e = make_event(vec![simple_choice(vec![])], vec![]);
    e.name = name.to_string();
    e.requires_flags = requires.into_iter().map(String::from).collect();
    e.forbids_flags = forbids.into_iter().map(String::from).collect();
    e
}

#[test]
fn set_and_clear_flag_round_trip() {
    let mut gs = test_state();
    let setter = make_event(vec![simple_choice(vec![Effect::SetFlag { flag: "met_lord".into() }])], vec![]);
    resolve(&setter, 0, &mut gs);
    assert!(gs.flags.contains("met_lord"));
    let clearer = make_event(vec![simple_choice(vec![Effect::ClearFlag { flag: "met_lord".into() }])], vec![]);
    resolve(&clearer, 0, &mut gs);
    assert!(!gs.flags.contains("met_lord"));
}

#[test]
fn requires_flag_gates_eligibility() {
    let mut gs = test_state();
    let deck = vec![flagged_event("Callback", vec!["debt_owed"], vec![])];
    // not eligible until the prerequisite flag is set
    assert!(eligible_events(&deck, 5, &gs, None).is_empty());
    gs.flags.insert("debt_owed".to_string());
    assert_eq!(eligible_events(&deck, 5, &gs, None).len(), 1);
}

#[test]
fn forbids_flag_retires_an_event() {
    let mut gs = test_state();
    let deck = vec![flagged_event("Intro", vec![], vec!["intro_done"])];
    assert_eq!(eligible_events(&deck, 5, &gs, None).len(), 1);
    gs.flags.insert("intro_done".to_string());
    assert!(eligible_events(&deck, 5, &gs, None).is_empty());
}

// ----------------------------------------------------------------------
// day cadence: quiet days
// ----------------------------------------------------------------------

#[test]
fn quiet_days_thin_out_as_darkness_rises() {
    let deck = vec![make_event(vec![simple_choice(vec![])], vec![])];
    let events_fired = |darkness: i32| {
        (0..300u64)
            .filter(|seed| {
                let mut gs = GameState::new(*seed);
                gs.region.darkness = darkness;
                roll(&deck, 1, &mut gs, None).is_some()
            })
            .count()
    };
    let calm = events_fired(0); // event_chance 55 -> many quiet days
    let dark = events_fired(100); // event_chance 100 -> relentless
    assert!(dark > calm, "deeper darkness should bring more events: {dark} vs {calm}");
}

// ----------------------------------------------------------------------
// region: the darkness war beyond the walls
// ----------------------------------------------------------------------

#[test]
fn region_seeding_is_deterministic() {
    let a = GameState::new(7);
    let b = GameState::new(7);
    assert_eq!(a.region, b.region);
    assert!(a.region.sites.len() >= 5, "expected a populated region");
    let c = GameState::new(8);
    assert_ne!(a.region, c.region, "different seeds should differ");
}

#[test]
fn darkness_stays_in_bounds_and_fluctuates() {
    let mut gs = test_state();
    let mut values = Vec::new();
    for _ in 0..60 {
        gs.region.tick(&mut gs.rng);
        assert!((0..=100).contains(&gs.region.darkness));
        values.push(gs.region.darkness);
    }
    // not monotonic: at least one day the darkness dipped
    assert!(values.windows(2).any(|w| w[1] < w[0]), "darkness never receded: {values:?}");
}

#[test]
fn overrun_site_spikes_darkness_and_sends_refugees() {
    let mut gs = test_state();
    gs.region.darkness = 80;
    gs.region.sites = vec![Site { name: "Vell".to_string(), kind: SiteKind::City, strength: 1 }];
    let mut fell = false;
    for _ in 0..20 {
        let before = gs.region.darkness;
        let lines = gs.region.tick(&mut gs.rng);
        if gs.region.sites.is_empty() {
            fell = true;
            assert!(gs.region.refugee_wave_days >= 1, "a fallen site owes a refugee wave");
            assert!(
                gs.region.darkness >= before.min(90),
                "darkness should spike when a site falls"
            );
            assert!(lines.iter().any(|l| l.contains("Vell")), "the fall should be told");
            break;
        }
    }
    assert!(fell, "a strength-1 site at darkness 80 must fall within 20 days");
}

#[test]
fn refugee_wave_brings_arrivals() {
    let mut gs = test_state();
    gs.resources.apply_delta(&ResourceDelta { food: 99, ..Default::default() });
    gs.region.refugee_wave_days = 1;
    gs.region.darkness = 0;
    gs.region.portal_pressure = 0; // keep the tick quiet for the assertion
    let before = gs.inhabitants.count_alive();
    gs.apply_daily_effects();
    let after = gs.inhabitants.count_alive();
    assert!(after >= before + 2, "a wave should bring at least 2 refugees");
    assert_eq!(gs.region.refugee_wave_days, 0);
}

#[test]
fn demon_events_gate_on_darkness() {
    let mut e = make_event(vec![simple_choice(vec![])], vec!["demon"]);
    e.min_darkness = Some(50);
    let mut gs = test_state();
    gs.region.darkness = 10;
    assert!(eligible_events(&[e.clone()], 1, &gs, None).is_empty());
    gs.region.darkness = 60;
    assert_eq!(eligible_events(&[e.clone()], 1, &gs, None).len(), 1);
    // and the inverse gate
    let mut low = make_event(vec![simple_choice(vec![])], vec!["demon"]);
    low.max_darkness = Some(30);
    assert!(eligible_events(&[low], 1, &gs, None).is_empty());
}

#[test]
fn demon_damage_scales_with_darkness() {
    let mut gs = test_state();
    gs.region.darkness = 80; // overwhelming
    gs.inhabitants.add(guard("G"));
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["demon"],
    );
    // -20 + (-20/2) = -30
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 70);
}

#[test]
fn region_effect_moves_the_war() {
    let mut gs = test_state();
    gs.region.darkness = 50;
    let strength_before = gs.region.total_strength();
    resolve_single(
        &mut gs,
        Effect::Region { darkness: -5, site_strength: 4, pressure: -1 },
        vec!["demon"],
    );
    assert_eq!(gs.region.darkness, 45);
    assert_eq!(gs.region.total_strength(), strength_before + 4);
    assert_eq!(gs.region.portal_pressure, 1);
}

// ----------------------------------------------------------------------
// reputation & adventurers
// ----------------------------------------------------------------------

#[test]
fn reputation_moves_with_fortune() {
    let mut gs = test_state();
    let base = gs.reputation;
    // buildings raise renown
    gs.build_upgrade(Upgrade::Granary);
    assert_eq!(gs.reputation, base + 2);
    // deaths spend it
    gs.inhabitants.add(guard("Doomed"));
    resolve_single(&mut gs, Effect::KillInhabitant { role: None }, vec![]);
    assert_eq!(gs.reputation, base);
    // surviving combat earns it back
    resolve_single(&mut gs, Effect::Morale { amount: 0 }, vec!["combat"]);
    assert_eq!(gs.reputation, base + 1);
    // clamped to 0-100
    gs.apply_reputation_delta(1000);
    assert_eq!(gs.reputation, 100);
}

#[test]
fn renown_and_darkness_draw_heroes() {
    // Heroes come for renown and a fight — no guild needed anymore.
    let mut gs = test_state();
    gs.reputation = 100;
    for _ in 0..60 {
        gs.apply_daily_effects();
        gs.region.darkness = 80; // heroes go where the fight is
        gs.resources.food = 500;
        gs.fortress.morale = 50;
    }
    assert!(!gs.adventurers.is_empty(), "renown + darkness should draw heroes");
    assert!(gs.adventurers.len() <= MAX_ADVENTURERS);
}

#[test]
fn low_renown_draws_no_heroes() {
    let mut gs = test_state();
    gs.reputation = ADVENTURER_MIN_REPUTATION - 1;
    for _ in 0..60 {
        gs.apply_daily_effects();
        gs.reputation = ADVENTURER_MIN_REPUTATION - 1;
    }
    assert!(gs.adventurers.is_empty());
}

#[test]
fn knight_perk_scales_with_combat_tier() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("G"));
    let mut knight = Adventurer {
        name: "Ser Test".to_string(),
        class: AdventurerClass::Knight,
        skills: SkillSet::default(),
        loadout: Default::default(),
    };
    // a dabbling knight doesn't help yet
    gs.adventurers.push(knight.clone());
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 80);
    // a skilled knight shields the defenders
    knight.skills.train(Skill::Combat, 90);
    gs.adventurers[0] = knight;
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    // -20 softened 25%: -15
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 80 - 15);
}

#[test]
fn hero_perks_work_daily_and_train_by_use() {
    let mut gs = test_state();
    let mut ranger = Adventurer {
        name: "Kestrel".to_string(),
        class: AdventurerClass::Ranger,
        skills: SkillSet::default(),
        loadout: Default::default(),
    };
    ranger.skills.train(Skill::Combat, 90); // Skilled: index 3
    gs.adventurers.push(ranger);
    let food_before = gs.resources.food;
    gs.apply_daily_effects();
    assert!(gs.resources.food > food_before - 2, "ranger hunting should offset upkeep");
    // event training: combat events sharpen the hero
    let xp_before = gs.adventurers[0].skills.xp(Skill::Combat);
    resolve_single(&mut gs, Effect::Morale { amount: 0 }, vec!["combat"]);
    assert_eq!(gs.adventurers[0].skills.xp(Skill::Combat), xp_before + 8);
}

// ----------------------------------------------------------------------
// items: equipment, crafting, loot, artifacts
// ----------------------------------------------------------------------

fn smith(name: &str, smithing_xp: u32) -> Inhabitant {
    let mut s = Inhabitant::new(name, Role::Blacksmith);
    s.skills.train(Skill::Smithing, smithing_xp);
    s
}

#[test]
fn quality_tracks_smith_tier_and_rating_scales() {
    assert_eq!(Quality::from_smith_tier(0), Quality::Crude);
    assert_eq!(Quality::from_smith_tier(3), Quality::Plain);
    assert_eq!(Quality::from_smith_tier(5), Quality::Fine);
    assert_eq!(Quality::from_smith_tier(7), Quality::Masterwork);

    // rating = quality+1, lifted by a helpful enchant, dropped by a curse
    assert_eq!(Item::new(ItemKind::Weapon, Quality::Crude).rating(), 1);
    assert_eq!(Item::new(ItemKind::Weapon, Quality::Masterwork).rating(), 4);
    assert_eq!(
        Item::enchanted(ItemKind::Weapon, Quality::Masterwork, Enchant::Keen).rating(),
        6
    );
    assert_eq!(
        Item::enchanted(ItemKind::Armor, Quality::Plain, Enchant::Hexed).rating(),
        1 // (1+1) - 2, floored to 1 while whole
    );
}

#[test]
fn forge_works_ore_into_an_item() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Blacksmith);
    gs.inhabitants.add(smith("Smith", 140)); // Proficient -> Fine work
    gs.resources.ore = 10;
    gs.fortress.craft_focus = ItemKind::Armor;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.ore, 7, "a forged item costs ore");
    assert_eq!(gs.items.count_kind(ItemKind::Armor), 1);
    assert!(gs.items.items[0].quality >= Quality::Fine, "a proficient smith makes fine work");
}

#[test]
fn forge_idle_without_ore_or_smith() {
    // a smithy with no ore makes nothing
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Blacksmith);
    gs.inhabitants.add(smith("Smith", 60));
    gs.apply_daily_effects();
    assert_eq!(gs.items.count(), 0);

    // ore but no smithy makes nothing either
    let mut gs2 = test_state();
    gs2.resources.ore = 20;
    gs2.apply_daily_effects();
    assert_eq!(gs2.items.count(), 0);
}

#[test]
fn fine_armor_turns_a_blow() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("G"));
    gs.items.add(Item::new(ItemKind::Armor, Quality::Fine)); // rating 3
    gs.redistribute_equipment(); // the guard takes up the harness
    assert!(gs.best_combat_armor() >= 3, "the guard now wears it");
    resolve_single(
        &mut gs,
        Effect::ApplyToRole { role: Role::Guard, health: -20, morale: 0 },
        vec!["combat"],
    );
    // one 25% step from the worn armor alone: -20 -> -15
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].health, 85);
}

#[test]
fn good_blades_win_more_battles() {
    let win = |seed: u64, arm: bool| {
        let mut gs = GameState::new(seed);
        gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats::default()));
        for n in 0..3 {
            gs.inhabitants.add(guard(&format!("G{n}")));
        }
        if arm {
            for _ in 0..4 {
                gs.items.add(Item::new(ItemKind::Weapon, Quality::Masterwork));
            }
        }
        fight_battle(14, 0, &battle_event(vec!["combat"]), &mut gs).victory
    };
    let bare = (0..200u64).filter(|s| win(*s, false)).count();
    let armed = (0..200u64).filter(|s| win(*s, true)).count();
    assert!(armed > bare, "masterwork weapons should win more fights: {armed} vs {bare}");
}

#[test]
fn fine_tools_lift_the_harvest() {
    let mut gs = test_state();
    gs.resources.food = 30; // below the granary-less cap
    gs.fortress.add_building(Upgrade::Farm);
    gs.inhabitants.add(Inhabitant::new("F", Role::Farmer));
    gs.items.add(Item::new(ItemKind::Tool, Quality::Fine)); // rating 3 -> +1 harvest
    gs.apply_daily_effects();
    // Farm I base 3 + field_hands 2 (one farmer) + tool 1 = 6, minus 1 upkeep
    assert_eq!(gs.resources.food, 30 + 6 - 1);
}

#[test]
fn the_best_blade_reaches_the_ablest_fighter() {
    let mut gs = test_state();
    gs.player =
        Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats { might: 8, wit: 3, heart: 3 }));
    gs.inhabitants.add(guard("Rook"));
    gs.items.add(Item::new(ItemKind::Weapon, Quality::Masterwork)); // rating 4
    gs.items.add(Item::new(ItemKind::Weapon, Quality::Crude)); // rating 1
    gs.redistribute_equipment();
    // the commander (might 8 + combat) is the ablest hand → the masterwork
    assert_eq!(gs.player.as_ref().unwrap().loadout.rating(ItemKind::Weapon), 4);
    // the rookie guard takes what's left; nothing lingers in the armory
    assert_eq!(gs.inhabitants.get_by_role(Role::Guard)[0].loadout.rating(ItemKind::Weapon), 1);
    assert_eq!(gs.items.count_kind(ItemKind::Weapon), 0);
}

#[test]
fn a_tool_goes_to_a_worker_not_a_fighter() {
    let mut gs = test_state();
    gs.inhabitants.add(Inhabitant::new("F", Role::Farmer));
    gs.inhabitants.add(guard("G"));
    gs.items.add(Item::new(ItemKind::Tool, Quality::Fine));
    gs.redistribute_equipment();
    assert!(gs.inhabitants.get_by_role(Role::Farmer)[0].loadout.tool.is_some());
    assert!(gs.inhabitants.get_by_role(Role::Guard)[0].loadout.tool.is_none());
}

#[test]
fn wizard_tower_enchants_with_residue() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::WizardTower);
    let mut mage = Inhabitant::new("Mage", Role::Peasant);
    mage.skills.train(Skill::Sorcery, 40);
    gs.inhabitants.add(mage);
    gs.items.add(Item::new(ItemKind::Weapon, Quality::Plain));
    gs.resources.residue = 5;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.residue, 2, "binding an enchant spends residue");
    assert!(gs.items.items[0].enchant.is_some(), "the item is now enchanted");
}

#[test]
fn a_greater_binding_needs_a_defter_mage_and_more_residue() {
    let make = |residue: i64| {
        let mut gs = test_state();
        gs.fortress.add_building(Upgrade::WizardTower);
        let mut mage = Inhabitant::new("Mage", Role::Peasant);
        mage.skills.train(Skill::Sorcery, 100); // Skilled — deft enough for Greater
        gs.inhabitants.add(mage);
        gs.items.add(Item::new(ItemKind::Weapon, Quality::Plain));
        gs.resources.residue = residue;
        gs.apply_daily_effects();
        gs
    };
    // ample residue + a Skilled mage -> a Greater binding, deeper cost
    let gs = make(6);
    let (kind, tier) = gs.items.items[0].enchant.expect("the blade is enchanted");
    assert_eq!((kind, tier), (Enchant::Keen, EnchantTier::Greater));
    assert_eq!(gs.resources.residue, 0, "a Greater binding spends the deeper residue");
    // the same mage with only Lesser's worth of residue settles for Lesser
    let gs = make(5);
    assert_eq!(gs.items.items[0].enchant.unwrap().1, EnchantTier::Lesser);
    assert_eq!(gs.resources.residue, 2);
}

#[test]
fn the_wizard_tower_wards_against_a_pressing_dark() {
    let bind_under = |darkness: i32| {
        let mut gs = test_state();
        gs.fortress.add_building(Upgrade::WizardTower);
        let mut mage = Inhabitant::new("Mage", Role::Peasant);
        mage.skills.train(Skill::Sorcery, 100);
        gs.inhabitants.add(mage);
        gs.items.add(Item::new(ItemKind::Armor, Quality::Fine));
        gs.resources.residue = 6;
        gs.region.darkness = darkness;
        gs.apply_daily_effects();
        gs.items.items[0].enchant.expect("the harness is enchanted").0
    };
    // when the dark presses, the tower wards; in calm it does what suits the kind
    assert_eq!(bind_under(95), Enchant::Warding, "heavy darkness calls for a ward");
    assert_eq!(bind_under(0), Enchant::Guarding, "calm: armor takes its natural guard");
}

#[test]
fn a_master_mage_lifts_a_curse_cleanly() {
    let lift = || {
        let mut gs = test_state();
        gs.fortress.add_building(Upgrade::WizardTower);
        let mut mage = Inhabitant::new("Mage", Role::Peasant);
        mage.skills.train(Skill::Sorcery, 300); // Master — lifts without botch
        gs.inhabitants.add(mage);
        gs.items.add(Item::enchanted(ItemKind::Armor, Quality::Fine, Enchant::Hexed));
        gs.items.add(Item::new(ItemKind::Weapon, Quality::Plain));
        gs.resources.residue = 8;
        gs.apply_daily_effects();
        gs
    };
    let gs = lift();
    // no item anywhere still bears the curse, and the plain blade went untouched
    // (a day's working is spent on the lifting, not a fresh binding)
    assert!(
        gs.items.items.iter().all(|i| i.enchant.is_none()),
        "the curse is broken and nothing new was bound the same day"
    );
    assert_eq!(gs.resources.residue, 3, "lifting spends residue (8 - 5)");
    // and the whole thing replays identically
    assert_eq!(
        serde_json::to_string(&gs).unwrap(),
        serde_json::to_string(&lift()).unwrap()
    );
}

#[test]
fn enchanting_needs_a_mage() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::WizardTower);
    gs.inhabitants.add(guard("Mundane")); // no magic skill
    gs.items.add(Item::new(ItemKind::Weapon, Quality::Plain));
    gs.resources.residue = 5;
    gs.apply_daily_effects();
    assert_eq!(gs.resources.residue, 5, "no mage, no enchant, no spent residue");
    // the guard takes up the blade during the auto-equip pass; it stays plain
    let blade = gs.inhabitants.get_by_role(Role::Guard)[0].loadout.weapon.as_ref();
    assert!(blade.is_some_and(|w| w.enchant.is_none()));
}

#[test]
fn worn_gear_breaks_but_the_smith_keeps_it_up() {
    // with no smith, a near-spent blade carried into use wears out and is scrapped
    let mut gs = test_state();
    gs.inhabitants.add(guard("G")); // a hand to carry it
    let mut worn = Item::new(ItemKind::Weapon, Quality::Plain);
    worn.condition = 2;
    gs.items.add(worn);
    gs.apply_daily_effects();
    assert_eq!(gs.items.count(), 0, "the armory is empty");
    assert!(
        gs.inhabitants.get_by_role(Role::Guard)[0].loadout.weapon.is_none(),
        "the worn blade broke in the guard's hand and was scrapped"
    );

    // a smithy with a smith repairs faster than the gear wears
    let mut gs2 = test_state();
    gs2.fortress.add_building(Upgrade::Blacksmith);
    gs2.inhabitants.add(guard("G"));
    gs2.inhabitants.add(smith("Smith", 60));
    let mut item = Item::new(ItemKind::Weapon, Quality::Plain);
    item.condition = 50;
    gs2.items.add(item);
    gs2.apply_daily_effects();
    let blade = gs2.inhabitants.get_by_role(Role::Guard)[0].loadout.weapon.as_ref();
    assert!(blade.is_some_and(|w| w.condition > 50), "the smith keeps the gear in trim");
}

#[test]
fn crafted_arms_carry_a_name_of_quality_material_and_form() {
    use fortress_core::{ItemForm, Material};
    let mut rng = GameRng::seed_from_u64(7);
    let item = Item::crafted(ItemKind::Weapon, Quality::Fine, Material::Steel, &mut rng);
    // form belongs to the kind, material and quality are carried, name reads well
    assert_eq!(item.form.unwrap().kind(), ItemKind::Weapon);
    assert_eq!(item.material, Some(Material::Steel));
    let label = item.label();
    assert!(label.starts_with("fine steel "), "got {label:?}");
    assert!(
        ItemForm::forms_for(ItemKind::Weapon).iter().any(|f| label.ends_with(f.name())),
        "label {label:?} should end in a weapon form"
    );
    // quality drives the rating; the descriptive form/material do not change it
    assert_eq!(item.rating(), Item::new(ItemKind::Weapon, Quality::Fine).rating());
    // and naming is deterministic per seed
    let mut rng_again = GameRng::seed_from_u64(7);
    let twin = Item::crafted(ItemKind::Weapon, Quality::Fine, Material::Steel, &mut rng_again);
    assert_eq!(item.form, twin.form);
}

#[test]
fn smiths_of_higher_tier_work_finer_metal() {
    use fortress_core::Material;
    assert_eq!(Material::from_smith_tier(0), Material::Bronze);
    assert_eq!(Material::from_smith_tier(3), Material::Iron);
    assert_eq!(Material::from_smith_tier(5), Material::Steel);
    assert_eq!(Material::from_smith_tier(7), Material::Silver);
}

#[test]
fn artifacts_never_wear_out() {
    let mut art = Item {
        kind: ItemKind::Weapon,
        quality: Quality::Masterwork,
        enchant: Some((Enchant::Keen, EnchantTier::Greater)),
        condition: 100,
        artifact: true,
        name: Some("Dawnedge".to_string()),
        form: None,
        material: None,
    };
    let rating_before = art.rating();
    art.degrade(500);
    assert_eq!(art.condition, 100, "an artifact does not degrade");
    assert!(!art.is_broken());
    assert_eq!(art.rating(), rating_before);
    assert_eq!(art.label(), "Dawnedge");
}

#[test]
fn demon_battles_drop_residue() {
    let mut gs = test_state();
    gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats { might: 8, wit: 3, heart: 3 }));
    let mut vet = guard("Vet");
    vet.skills.train(Skill::Combat, 300);
    gs.inhabitants.add(vet);
    let report = fight_battle(2, 0, &battle_event(vec!["combat", "demon"]), &mut gs);
    assert!(report.victory);
    assert!(gs.resources.residue >= 1, "a beaten demon leaves residue");
}

// ----------------------------------------------------------------------
// multi-round combat & the morale passive (Stage 5)
// ----------------------------------------------------------------------

#[test]
fn battles_play_out_over_rounds() {
    let mut gs = test_state();
    gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats { might: 6, wit: 3, heart: 3 }));
    for n in 0..3 {
        gs.inhabitants.add(guard(&format!("G{n}")));
    }
    let report = fight_battle(12, 0, &battle_event(vec!["combat"]), &mut gs);
    let rounds = report.lines.iter().filter(|l| l.starts_with("Round ")).count();
    assert!(rounds >= 2, "a battle should resolve over several rounds: {:?}", report.lines);
}

#[test]
fn a_caster_commander_throws_spells() {
    let mut gs = test_state();
    // a Wizard leads with the bolt, not the blade
    gs.player = Some(PlayerCharacter::new("Archon", ClassKind::Wizard, Stats::default()));
    let report = fight_battle(2, 0, &battle_event(vec!["combat"]), &mut gs);
    assert!(
        report.lines.iter().any(|l| l.contains("bolt")),
        "a caster commander should sling sorcery: {:?}",
        report.lines
    );
}

#[test]
fn wards_blunt_the_foe() {
    let mut gs = test_state();
    let mut warden = Inhabitant::new("Wardel", Role::Peasant);
    warden.skills.train(Skill::Warding, 60); // Competent warder
    gs.inhabitants.add(warden);
    let report = fight_battle(6, 0, &battle_event(vec!["combat"]), &mut gs);
    assert!(report.lines[0].contains("wards"), "warders should be noted in the muster: {:?}", report.lines[0]);
}

#[test]
fn a_breach_throws_everyone_to_the_wall() {
    let mut gs = test_state();
    gs.inhabitants.add(guard("Lone")); // a single defender on the line
    for n in 0..4 {
        gs.inhabitants.add(Inhabitant::new(&format!("P{n}"), Role::Peasant)); // reserves
    }
    let report = fight_battle(28, 0, &battle_event(vec!["combat"]), &mut gs);
    assert!(!report.victory, "a lone guard can't hold a foe of 28");
    assert!(
        report.lines.iter().any(|l| l.contains("gate is forced")),
        "the breach should call up the reserves: {:?}",
        report.lines
    );
}

#[test]
fn high_morale_wins_more_fights() {
    let win = |seed: u64, morale: i32| {
        let mut gs = GameState::new(seed);
        gs.fortress.morale = morale;
        gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats { might: 4, wit: 3, heart: 3 }));
        for n in 0..2 {
            let mut g = guard(&format!("G{n}"));
            g.skills.train(Skill::Combat, 50);
            gs.inhabitants.add(g);
        }
        // re-pin morale (the muster reads it; tick doesn't run here)
        fight_battle(11, 0, &battle_event(vec!["combat"]), &mut gs).victory
    };
    let low = (0..200u64).filter(|s| win(*s, 15)).count();
    let high = (0..200u64).filter(|s| win(*s, 90)).count();
    assert!(high > low, "high spirits should win more close fights: {high} vs {low}");
}

#[test]
fn morale_at_the_cap_converts_to_renown() {
    let mut gs = test_state();
    gs.fortress.morale = 99;
    let rep_before = gs.reputation;
    let result = resolve_single(&mut gs, Effect::Morale { amount: 12 }, vec![]);
    assert_eq!(gs.fortress.morale, 100, "morale still tops out at 100");
    assert!(gs.reputation > rep_before, "the wasted cheer should spread the fortress's name");
    assert!(result.lines.iter().any(|l| l.contains("renown")));
}

#[test]
fn thriving_holds_train_harder() {
    let mut gs = test_state();
    gs.fortress.add_building(Upgrade::Barracks);
    gs.inhabitants.add(guard("G"));
    gs.fortress.morale = 85; // a thriving hold: +1 practice
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), 3); // 2 + 1 passive
}

// ----------------------------------------------------------------------
// seasons, weather & world end-state (Stage 6)
// ----------------------------------------------------------------------

#[test]
fn seasons_follow_the_calendar() {
    assert_eq!(Season::for_day(1), Season::Spring);
    assert_eq!(Season::for_day(12), Season::Spring);
    assert_eq!(Season::for_day(13), Season::Summer);
    assert_eq!(Season::for_day(25), Season::Autumn);
    assert_eq!(Season::for_day(37), Season::Winter);
    assert_eq!(Season::for_day(49), Season::Spring); // the wheel turns round
}

#[test]
fn weather_is_derived_and_the_founding_day_is_calm() {
    // same seed + day -> same skies, every time (no rng draw)
    assert_eq!(World::for_day(42, 17), World::for_day(42, 17));
    // day one always dawns clear, whatever the seed
    for seed in 0..20 {
        assert_eq!(World::for_day(seed, 1).weather, Weather::Clear);
    }
}

#[test]
fn season_and_weather_multipliers() {
    assert_eq!(World { season: Season::Spring, weather: Weather::Clear }.farm_mult_pct(), 100);
    assert_eq!(World { season: Season::Summer, weather: Weather::Clear }.farm_mult_pct(), 115);
    assert_eq!(World { season: Season::Winter, weather: Weather::Clear }.farm_mult_pct(), 50);
    // foul weather compounds with the season
    assert_eq!(World { season: Season::Winter, weather: Weather::Snow }.farm_mult_pct(), 30);
}

#[test]
fn winter_thins_the_harvest() {
    let mut gs = test_state();
    gs.resources.food = 10; // well below the spoilage cap
    gs.fortress.add_building(Upgrade::Farm);
    gs.inhabitants.add(Inhabitant::new("F", Role::Farmer)); // no farming skill
    gs.fortress.day = 37; // deep winter
    gs.apply_daily_effects();
    let w = gs.world;
    assert_eq!(w.season, Season::Winter);
    // base 3 + field_hands 2 (one farmer), no skill/tools -> raw 5; scaled by
    // season+weather; -1 upkeep
    let expected = 10 + 5 * w.farm_mult_pct() / 100 - 1;
    assert_eq!(gs.resources.food, expected);
    assert!(gs.resources.food < 12, "winter should bite into the larder");
}

#[test]
fn storms_hamper_the_defenders() {
    let win = |seed: u64, weather: Weather| {
        let mut gs = GameState::new(seed);
        gs.world.weather = weather; // battles read the standing weather
        gs.player = Some(PlayerCharacter::new("Cmd", ClassKind::Warlord, Stats::default()));
        for n in 0..2 {
            let mut g = guard(&format!("G{n}"));
            g.skills.train(Skill::Combat, 50);
            gs.inhabitants.add(g);
        }
        fight_battle(11, 0, &battle_event(vec!["combat"]), &mut gs).victory
    };
    let clear = (0..200u64).filter(|s| win(*s, Weather::Clear)).count();
    let storm = (0..200u64).filter(|s| win(*s, Weather::Storm)).count();
    assert!(storm < clear, "a storm should cost the defenders fights: {storm} vs {clear}");
}

#[test]
fn seasonal_events_fire_only_in_their_season() {
    let mut e = make_event(vec![simple_choice(vec![])], vec![]);
    e.requires_season = Some(Season::Winter);
    let gs = test_state();
    // day 5 is spring — the winter event stays in the deck
    assert!(eligible_events(std::slice::from_ref(&e), 5, &gs, None).is_empty());
    // day 37 is winter — now it may fire
    assert_eq!(eligible_events(std::slice::from_ref(&e), 37, &gs, None).len(), 1);
}

#[test]
fn a_fallen_world_stills_the_envoys_then_rebuilds() {
    let mut gs = test_state();
    // an envoy event needs a living realm beyond the walls
    let envoy = make_event(vec![simple_choice(vec![])], vec!["diplomacy"]);
    assert_eq!(eligible_events(std::slice::from_ref(&envoy), 5, &gs, None).len(), 1);

    // the whole region falls
    gs.region.sites.clear();
    assert!(gs.region.all_fallen());
    assert!(
        eligible_events(std::slice::from_ref(&envoy), 5, &gs, None).is_empty(),
        "no envoys come once the world has fallen"
    );

    // but survivors regroup, and a fragile camp rises from the ruin
    let mut rebuilt = false;
    for _ in 0..200 {
        gs.region.darkness = 20; // keep the window open for the test
        gs.region.tick(&mut gs.rng);
        if !gs.region.sites.is_empty() {
            rebuilt = true;
            break;
        }
    }
    assert!(rebuilt, "survivors should eventually rebuild");
    assert_eq!(gs.region.sites[0].kind, SiteKind::Survivors);
}

// ----------------------------------------------------------------------
// auto-mode & town groundwork (Stage 7)
// ----------------------------------------------------------------------

#[test]
fn auto_pick_prefers_the_better_choice() {
    let gs = test_state();
    let bad = simple_choice(vec![Effect::Morale { amount: -10 }]);
    let good = simple_choice(vec![Effect::Morale { amount: 10 }]);
    let event = make_event(vec![bad, good], vec![]); // the good one is second
    assert_eq!(auto_pick(&event, &gs), Some(1), "auto-mode should take the gain, not the loss");
}

#[test]
fn auto_pick_skips_unaffordable_choices() {
    let gs = test_state(); // 50 food, 50 valuables
    let mut lavish = simple_choice(vec![Effect::Morale { amount: 100 }]);
    lavish.cost = ResourceDelta { valuables: 9999, ..Default::default() }; // unaffordable
    let modest = simple_choice(vec![Effect::Morale { amount: 1 }]);
    let event = make_event(vec![lavish, modest], vec![]);
    assert_eq!(auto_pick(&event, &gs), Some(1), "it can't pick what it can't pay for");
}

#[test]
fn auto_pick_none_when_all_locked() {
    let gs = test_state();
    let mut a = simple_choice(vec![]);
    a.cost = ResourceDelta { valuables: 9999, ..Default::default() };
    let mut b = simple_choice(vec![]);
    b.cost = ResourceDelta { food: 9999, ..Default::default() };
    let event = make_event(vec![a, b], vec![]);
    assert_eq!(auto_pick(&event, &gs), None);
}

#[test]
fn a_crowded_built_up_hold_grows_into_a_village() {
    let mut gs = test_state();
    for u in [Upgrade::Farm, Upgrade::Workshop, Upgrade::Tavern] {
        gs.fortress.add_building(u); // three standing buildings
    }
    gs.fortress.max_population = 20;
    for n in 0..16 {
        gs.inhabitants.add(guard(&format!("G{n}"))); // 16/20 = 80% full
    }
    let grown = gs.fortress.try_promote(gs.inhabitants.count_alive());
    assert_eq!(grown, Some(SettlementTier::Village));
    assert_eq!(gs.fortress.settlement_tier, SettlementTier::Village);
    assert_eq!(gs.fortress.max_population, 35); // 20 + (35 - 20) step
}

#[test]
fn a_sparse_hold_stays_a_hamlet() {
    let mut gs = test_state();
    for u in [Upgrade::Farm, Upgrade::Workshop, Upgrade::Tavern] {
        gs.fortress.add_building(u);
    }
    gs.inhabitants.add(guard("A")); // nowhere near crowded
    assert_eq!(gs.fortress.try_promote(gs.inhabitants.count_alive()), None);
    assert_eq!(gs.fortress.settlement_tier, SettlementTier::Hamlet);
}

#[test]
fn grant_item_effect_places_an_artifact() {
    let mut gs = test_state();
    let effect = Effect::GrantItem {
        kind: ItemKind::Weapon,
        quality: Quality::Masterwork,
        enchant: Some(Enchant::Keen),
        artifact: true,
        name: Some("The Sunblade".to_string()),
    };
    resolve_single(&mut gs, effect, vec![]);
    assert_eq!(gs.items.count(), 1);
    assert!(gs.items.items[0].artifact);
    assert_eq!(gs.items.items[0].label(), "The Sunblade");
}
