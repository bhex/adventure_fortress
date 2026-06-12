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
    f.add_upgrade(Upgrade::Farm);
    f.add_upgrade(Upgrade::Farm);
    assert_eq!(f.upgrades.len(), 1);
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
    gs.fortress.add_upgrade(Upgrade::Blacksmith);
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
    gs.fortress.add_upgrade(Upgrade::Infirmary);
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
    gs.fortress.add_upgrade(Upgrade::Blacksmith);
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
    gs.fortress.add_upgrade(Upgrade::Farm);
    gs.apply_daily_effects();
    assert_eq!(gs.resources.food, 53);
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
    gs.fortress.add_upgrade(Upgrade::Barracks);
    assert_eq!(gs.fortress.sleeping_capacity(), 11);

    // 13 inhabitants vs 11 beds: 2 sleep rough, no warm-sleep bonus,
    // and the last 2 by iteration order lose a point of morale.
    for n in 0..13 {
        let mut g = guard(&format!("G{n}"));
        g.morale = 50;
        gs.inhabitants.add(g);
    }
    let morale_before = gs.fortress.morale;
    gs.apply_daily_effects();
    let rough: Vec<i32> = gs.inhabitants.inhabitants.iter().skip(11).map(|i| i.morale).collect();
    assert_eq!(rough, vec![49, 49]);
    assert!(gs.inhabitants.inhabitants.iter().take(11).all(|i| i.morale >= 50));
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

    gs.fortress.add_upgrade(Upgrade::Barracks);
    gs.apply_daily_effects();
    assert_eq!(gs.inhabitants.inhabitants[0].skills.xp(Skill::Combat), 2);
}

#[test]
fn skilled_farmers_raise_harvest() {
    let mut gs = test_state();
    gs.fortress.add_upgrade(Upgrade::Farm);
    let mut f = Inhabitant::new("F", Role::Farmer);
    f.skills.train(Skill::Farming, 140); // Proficient, index 4
    gs.inhabitants.add(f);
    let food_before = gs.resources.food;
    gs.apply_daily_effects();
    // base 3 + 4/2 = 5 harvest, minus 1 upkeep (1 inhabitant), +2 practice xp doesn't tier
    assert_eq!(gs.resources.food, food_before + 5 - 1);
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
    gs.construct(Upgrade::Watchtower).expect("buildable");
    assert!(gs.fortress.has_upgrade(Upgrade::Watchtower));
    let cost = Upgrade::Watchtower.build_cost();
    assert_eq!(gs.resources.wood, wood_before - cost.wood);
    assert_eq!(gs.resources.stone, stone_before - cost.stone);
    // can't raise it twice
    assert_eq!(gs.construct(Upgrade::Watchtower), Err(BuildAvailability::AlreadyBuilt));
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
fn founding_grant_is_free_and_applies_bonuses() {
    let mut gs = test_state(); // no wood/stone — couldn't pay for it
    let pop_before = gs.fortress.max_population;
    let food_before = gs.resources.food;
    gs.build_upgrade(Upgrade::Inn);
    assert!(gs.fortress.has_upgrade(Upgrade::Inn));
    assert_eq!(gs.fortress.max_population, pop_before + 5);
    assert_eq!(gs.resources.food, food_before); // charter grant, not purchase
}

#[test]
fn inn_adds_beds_and_daily_morale() {
    let mut gs = test_state();
    assert_eq!(gs.fortress.sleeping_capacity(), 6);
    gs.build_upgrade(Upgrade::Inn);
    assert_eq!(gs.fortress.sleeping_capacity(), 11);
    let morale_before = gs.fortress.morale;
    gs.apply_daily_effects();
    // +1 inn, +1 warm sleep (0 inhabitants in test_state, so no upkeep penalty)
    assert!(gs.fortress.morale >= morale_before + 1);
}
