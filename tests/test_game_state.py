from src.core.fortress import Upgrade
from src.core.game_state import GameState
from src.core.inhabitants import Inhabitant, Role


def make_state(morale: int = 50, food: int = 0, day: int = 1) -> GameState:
    gs = GameState(run_seed=1)
    gs.fortress.name = "Test"
    gs.fortress.morale = morale
    gs.fortress.day = day
    gs.resources.food = food
    return gs


def test_game_over_at_zero_morale():
    gs = make_state(morale=0)
    assert gs.is_game_over()


def test_victory_after_day_30():
    gs = make_state(day=30)
    assert not gs.is_victory()
    gs.fortress.advance_day()
    assert gs.is_victory()


def test_build_upgrade_watchtower_bonus():
    gs = make_state()
    base = gs.fortress.defense
    gs.build_upgrade(Upgrade.WATCHTOWER)
    assert gs.fortress.defense == base + 5
    # building twice does not stack
    gs.build_upgrade(Upgrade.WATCHTOWER)
    assert gs.fortress.defense == base + 5


def test_daily_farm_yield():
    gs = make_state(food=10)
    gs.fortress.add_upgrade(Upgrade.FARM)
    gs.apply_daily_effects()
    assert gs.resources.food == 13  # +3 farm, no inhabitants to feed


def test_daily_upkeep_feeds_inhabitants():
    gs = make_state(food=10)
    for n in range(4):
        gs.inhabitants.add(Inhabitant(name=f"I{n}", role=Role.FARMER))
    gs.apply_daily_effects()
    assert gs.resources.food == 8  # 4 alive -> upkeep 2


def test_starvation_drains_morale():
    gs = make_state(food=0, morale=50)
    gs.inhabitants.add(Inhabitant(name="Hungry", role=Role.FARMER))
    gs.apply_daily_effects()
    assert gs.fortress.morale < 50


def test_morale_cascade_high_and_low():
    gs = make_state(morale=50, food=10)
    gs.inhabitants.add(Inhabitant(name="Happy", role=Role.FARMER, morale=90))
    gs.apply_daily_effects()
    assert gs.fortress.morale == 52

    gs2 = make_state(morale=50, food=10)
    gs2.inhabitants.add(Inhabitant(name="Sad", role=Role.FARMER, morale=10))
    gs2.apply_daily_effects()
    assert gs2.fortress.morale == 48


def test_save_load_round_trip(tmp_path):
    gs = make_state()
    gs.resources.apply_delta({"food": 25, "gold": 7})
    gs.fortress.add_upgrade(Upgrade.GRANARY)
    gs.inhabitants.add(Inhabitant(name="Keeper", role=Role.GUARD))
    gs.events_resolved = 9

    path = tmp_path / "save.json"
    gs.save(str(path))
    restored = GameState.load(str(path))

    assert restored.to_dict() == gs.to_dict()
    # RNG continues identically after restore
    assert restored.rng.random() == gs.rng.random()
