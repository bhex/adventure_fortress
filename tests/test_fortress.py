from src.core.fortress import Fortress, Upgrade


def test_advance_day():
    f = Fortress(name="Test")
    f.advance_day()
    assert f.day == 2


def test_morale_clamps_to_bounds():
    f = Fortress(name="Test", morale=95)
    f.apply_morale_delta(20)
    assert f.morale == 100
    f.apply_morale_delta(-150)
    assert f.morale == 0


def test_defense_never_negative():
    f = Fortress(name="Test", defense=3)
    f.apply_defense_delta(-10)
    assert f.defense == 0


def test_is_defeated_only_at_zero_morale():
    f = Fortress(name="Test", morale=1)
    assert not f.is_defeated()
    f.apply_morale_delta(-1)
    assert f.is_defeated()


def test_upgrades_no_duplicates():
    f = Fortress(name="Test")
    f.add_upgrade(Upgrade.FARM)
    f.add_upgrade(Upgrade.FARM)
    assert f.upgrades == [Upgrade.FARM]
    assert f.has_upgrade(Upgrade.FARM)
    assert not f.has_upgrade(Upgrade.GRANARY)


def test_serialization_round_trip():
    f = Fortress(name="Test", day=5, morale=33, upgrades=[Upgrade.WATCHTOWER])
    restored = Fortress.from_dict(f.to_dict())
    assert restored == f
    assert isinstance(restored.upgrades[0], Upgrade)
