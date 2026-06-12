from src.core.resources import Resources


def test_apply_delta_adds_and_subtracts():
    r = Resources(food=10, gold=5)
    r.apply_delta({"food": 5, "gold": -3})
    assert r.food == 15
    assert r.gold == 2


def test_apply_delta_clamps_at_zero():
    r = Resources(food=3)
    r.apply_delta({"food": -10})
    assert r.food == 0


def test_can_afford():
    r = Resources(food=10, gold=5)
    assert r.can_afford({"food": 10, "gold": 5})
    assert not r.can_afford({"gold": 6})
    assert r.can_afford({})


def test_serialization_round_trip():
    r = Resources(food=1, gold=2, stone=3, wood=4)
    assert Resources.from_dict(r.to_dict()) == r
