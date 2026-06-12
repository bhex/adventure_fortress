from random import Random

from src.core.inhabitants import (
    Inhabitant,
    InhabitantManager,
    Role,
    Trait,
    generate_inhabitant,
)


def make(name="Bran", role=Role.GUARD, **kwargs) -> Inhabitant:
    return Inhabitant(name=name, role=role, **kwargs)


def test_add_remove_and_count():
    m = InhabitantManager()
    m.add(make("A"))
    m.add(make("B", role=Role.FARMER))
    assert m.count_alive() == 2
    m.remove("A")
    assert m.count_alive() == 1
    assert not m.has_role(Role.GUARD)


def test_get_by_role_excludes_dead():
    m = InhabitantManager()
    m.add(make("A"))
    m.add(make("B", is_alive=False))
    assert [i.name for i in m.get_by_role(Role.GUARD)] == ["A"]


def test_damage_kills_at_zero_health():
    i = make(health=10)
    i.damage(10)
    assert not i.is_alive


def test_sickly_takes_double_damage():
    i = make(health=100, traits=[Trait.SICKLY])
    i.damage(10)
    assert i.health == 80


def test_apply_to_role_returns_deaths():
    m = InhabitantManager()
    m.add(make("Doomed", health=5))
    m.add(make("Tough", health=100))
    deaths = m.apply_to_role(Role.GUARD, health_delta=-10)
    assert [d.name for d in deaths] == ["Doomed"]
    assert m.count_alive() == 1
    assert m.count_dead() == 1


def test_random_non_loyal_skips_loyal():
    m = InhabitantManager()
    m.add(make("Loyal", traits=[Trait.LOYAL]))
    rng = Random(0)
    assert m.random_non_loyal(rng) is None
    m.add(make("Flighty"))
    assert m.random_non_loyal(rng).name == "Flighty"


def test_average_morale():
    m = InhabitantManager()
    assert m.average_morale() == 50  # empty default
    m.add(make("A", morale=20))
    m.add(make("B", morale=40))
    assert m.average_morale() == 30


def test_generate_inhabitant_is_deterministic_per_seed():
    a = generate_inhabitant(Role.HEALER, Random(7))
    b = generate_inhabitant(Role.HEALER, Random(7))
    assert a == b


def test_serialization_round_trip():
    m = InhabitantManager()
    m.add(make("A", traits=[Trait.BRAVE], health=42))
    restored = InhabitantManager.from_dict(m.to_dict())
    assert restored.inhabitants == m.inhabitants
    assert isinstance(restored.inhabitants[0].role, Role)
    assert isinstance(restored.inhabitants[0].traits[0], Trait)
