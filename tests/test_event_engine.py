from src.core.fortress import Upgrade
from src.core.game_state import GameState
from src.core.inhabitants import Inhabitant, Role, Trait
from src.events.event_base import Choice, Effect, EffectKind, Event
from src.events.event_engine import EventEngine
from src.events.event_pool import load_events


def make_state(seed=42, food=50, gold=50, morale=50) -> GameState:
    gs = GameState(run_seed=seed)
    gs.fortress.name = "Test"
    gs.fortress.morale = morale
    gs.resources.apply_delta({"food": food, "gold": gold})
    return gs


def make_event(**overrides) -> Event:
    defaults = dict(
        name="Test Event",
        description="desc",
        choices=[Choice(label="Go", description="", effects=[])],
    )
    defaults.update(overrides)
    return Event(**defaults)


# ----------------------------------------------------------------------
# roll() filtering
# ----------------------------------------------------------------------

def test_roll_filters_by_day():
    gs = make_state()
    engine = EventEngine([make_event(min_day=5), make_event(name="Late", min_day=1, max_day=2)], gs)
    assert engine.eligible_events(day=3) == []


def test_roll_filters_by_morale():
    gs = make_state(morale=50)
    engine = EventEngine([make_event(min_morale=60), make_event(name="Low", max_morale=40)], gs)
    assert engine.eligible_events(day=1) == []


def test_roll_filters_by_resource():
    gs = make_state(gold=5)
    engine = EventEngine([make_event(min_resource={"gold": 10})], gs)
    assert engine.eligible_events(day=1) == []


def test_roll_filters_by_role():
    gs = make_state()
    engine = EventEngine([make_event(requires_role="healer")], gs)
    assert engine.eligible_events(day=1) == []
    gs.inhabitants.add(Inhabitant(name="H", role=Role.HEALER))
    assert len(engine.eligible_events(day=1)) == 1


def test_roll_filters_by_upgrade():
    gs = make_state()
    engine = EventEngine([make_event(requires_upgrade="Watchtower")], gs)
    assert engine.eligible_events(day=1) == []
    gs.fortress.add_upgrade(Upgrade.WATCHTOWER)
    assert len(engine.eligible_events(day=1)) == 1


def test_roll_never_repeats_previous_event():
    gs = make_state()
    engine = EventEngine([make_event(name="Only")], gs)
    assert engine.roll(day=1).name == "Only"
    assert engine.roll(day=1) is None


def test_roll_is_deterministic_for_seed():
    events = load_events()
    names_a = [EventEngine(events, make_state(seed=99)).roll(day=1).name for _ in range(1)]
    names_b = [EventEngine(events, make_state(seed=99)).roll(day=1).name for _ in range(1)]
    assert names_a == names_b


# ----------------------------------------------------------------------
# resolve() effects
# ----------------------------------------------------------------------

def resolve_single(gs, effect: Effect, tags=None, cost=None):
    event = make_event(
        choices=[Choice(label="Go", description="", effects=[effect], cost=cost or {})],
        tags=tags or [],
    )
    return EventEngine([event], gs).resolve(event, 0)


def test_resource_effect():
    gs = make_state(food=10)
    resolve_single(gs, Effect(EffectKind.RESOURCE, {"food": 5, "gold": -10}))
    assert gs.resources.food == 15
    assert gs.resources.gold == 40


def test_morale_and_defense_effects():
    gs = make_state(morale=50)
    resolve_single(gs, Effect(EffectKind.MORALE, {"amount": -10}))
    assert gs.fortress.morale == 40
    resolve_single(gs, Effect(EffectKind.DEFENSE, {"amount": 3}))
    assert gs.fortress.defense == 13


def test_choice_cost_is_paid():
    gs = make_state(gold=50)
    resolve_single(gs, Effect(EffectKind.MORALE, {"amount": 0}), cost={"gold": 20})
    assert gs.resources.gold == 30


def test_spawn_inhabitant_respects_max_population():
    gs = make_state()
    gs.fortress.max_population = 0
    resolve_single(gs, Effect(EffectKind.SPAWN_INHABITANT, {}))
    assert gs.inhabitants.count_alive() == 0

    gs.fortress.max_population = 5
    resolve_single(gs, Effect(EffectKind.SPAWN_INHABITANT, {"role": "guard"}))
    assert gs.inhabitants.has_role(Role.GUARD)


def test_kill_inhabitant():
    gs = make_state()
    gs.inhabitants.add(Inhabitant(name="Victim", role=Role.FARMER))
    resolve_single(gs, Effect(EffectKind.KILL_INHABITANT, {}))
    assert gs.inhabitants.count_alive() == 0


def test_remove_inhabitant_respects_loyal():
    gs = make_state()
    gs.inhabitants.add(Inhabitant(name="Stays", role=Role.GUARD, traits=[Trait.LOYAL]))
    resolve_single(gs, Effect(EffectKind.REMOVE_INHABITANT, {}))
    assert gs.inhabitants.count_alive() == 1


def test_apply_to_role():
    gs = make_state()
    gs.inhabitants.add(Inhabitant(name="G", role=Role.GUARD, health=50, morale=50))
    resolve_single(gs, Effect(EffectKind.APPLY_TO_ROLE, {"role": "guard", "health": -10, "morale": 5}))
    guard = gs.inhabitants.get_by_role(Role.GUARD)[0]
    assert guard.health == 40
    assert guard.morale == 55


def test_add_upgrade_applies_immediate_bonus():
    gs = make_state()
    base_defense = gs.fortress.defense
    resolve_single(gs, Effect(EffectKind.ADD_UPGRADE, {"name": "Watchtower"}))
    assert gs.fortress.has_upgrade(Upgrade.WATCHTOWER)
    assert gs.fortress.defense == base_defense + 5


def test_blacksmith_mitigates_combat_damage():
    gs = make_state()
    gs.fortress.add_upgrade(Upgrade.BLACKSMITH)
    gs.inhabitants.add(Inhabitant(name="G", role=Role.GUARD, health=100))
    resolve_single(gs, Effect(EffectKind.APPLY_TO_ROLE, {"role": "guard", "health": -20}), tags=["combat"])
    assert gs.inhabitants.get_by_role(Role.GUARD)[0].health == 85  # 25% mitigated


def test_infirmary_halves_disaster_damage():
    gs = make_state()
    gs.fortress.add_upgrade(Upgrade.INFIRMARY)
    gs.inhabitants.add(Inhabitant(name="F", role=Role.FARMER, health=100))
    resolve_single(gs, Effect(EffectKind.APPLY_TO_ROLE, {"role": "farmer", "health": -20}), tags=["disaster"])
    assert gs.inhabitants.get_by_role(Role.FARMER)[0].health == 90


# ----------------------------------------------------------------------
# content sanity
# ----------------------------------------------------------------------

def test_all_content_events_parse_and_have_choices():
    events = load_events()
    assert len(events) >= 35
    for event in events:
        assert event.choices, f"{event.name} has no choices"
        for choice in event.choices:
            for effect in choice.effects:
                assert isinstance(effect.kind, EffectKind)


def test_all_content_events_resolvable():
    """Every choice of every event resolves without raising."""
    for event in load_events():
        for idx in range(len(event.choices)):
            gs = make_state(food=999, gold=999)
            gs.resources.apply_delta({"wood": 999, "stone": 999})
            for role in Role:
                gs.inhabitants.add(Inhabitant(name=f"T-{role}", role=role))
            EventEngine([event], gs).resolve(event, idx)
