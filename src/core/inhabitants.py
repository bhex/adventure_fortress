from __future__ import annotations
from dataclasses import dataclass, field
from enum import StrEnum
from random import Random
from typing import Optional


class Role(StrEnum):
    GUARD = "guard"
    FARMER = "farmer"
    BLACKSMITH = "blacksmith"
    HEALER = "healer"


class Trait(StrEnum):
    BRAVE = "brave"      # better combat outcomes
    SKILLED = "skilled"  # better craft/build outcomes
    SICKLY = "sickly"    # takes double health damage
    LOYAL = "loyal"      # immune to desertion
    GREEDY = "greedy"    # demands gold occasionally


NAMES: dict[Role, list[str]] = {
    Role.GUARD: ["Aldric", "Bran", "Cedric", "Doran", "Edric", "Farrell", "Gareth", "Hadwin", "Idris", "Jareth"],
    Role.FARMER: ["Abel", "Barrett", "Colm", "Davin", "Emmet", "Finley", "Greer", "Hayden", "Ivar", "Jowan"],
    Role.BLACKSMITH: ["Aldous", "Bryn", "Cade", "Duncan", "Eamon", "Fergus", "Gawain", "Hadleigh", "Ivan", "Jorin"],
    Role.HEALER: ["Aideen", "Brenna", "Ciara", "Deirdre", "Eileen", "Fiona", "Grainne", "Hilde", "Isla", "Jorah"],
}

ROLE_ICONS: dict[Role, str] = {
    Role.GUARD: "⚔",
    Role.FARMER: "🌾",
    Role.BLACKSMITH: "🔨",
    Role.HEALER: "✚",
}


@dataclass
class Inhabitant:
    name: str
    role: Role
    health: int = 100
    morale: int = 50
    traits: list[Trait] = field(default_factory=list)
    is_alive: bool = True

    def has_trait(self, trait: Trait) -> bool:
        return trait in self.traits

    def damage(self, amount: int):
        actual = amount * 2 if self.has_trait(Trait.SICKLY) else amount
        self.health = max(0, self.health - actual)
        if self.health == 0:
            self.is_alive = False

    def heal(self, amount: int):
        self.health = min(100, self.health + amount)

    def apply_morale(self, amount: int):
        self.morale = max(0, min(100, self.morale + amount))

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "role": str(self.role),
            "health": self.health,
            "morale": self.morale,
            "traits": [str(t) for t in self.traits],
            "is_alive": self.is_alive,
        }

    @classmethod
    def from_dict(cls, data: dict) -> Inhabitant:
        return cls(
            name=data["name"],
            role=Role(data["role"]),
            health=data["health"],
            morale=data["morale"],
            traits=[Trait(t) for t in data["traits"]],
            is_alive=data["is_alive"],
        )


def generate_inhabitant(role: Role, rng: Random) -> Inhabitant:
    name = rng.choice(NAMES[role])
    num_traits = rng.choices([0, 1, 2], weights=[4, 4, 2])[0]
    traits = rng.sample(list(Trait), k=num_traits)
    health = 70 if Trait.SICKLY in traits else 100
    morale = rng.randint(40, 70)
    return Inhabitant(name=name, role=role, health=health, morale=morale, traits=traits)


class InhabitantManager:
    def __init__(self):
        self.inhabitants: list[Inhabitant] = []

    def add(self, inhabitant: Inhabitant):
        self.inhabitants.append(inhabitant)

    def remove(self, name: str):
        self.inhabitants = [i for i in self.inhabitants if i.name != name]

    def get_alive(self) -> list[Inhabitant]:
        return [i for i in self.inhabitants if i.is_alive]

    def get_by_role(self, role: Role) -> list[Inhabitant]:
        return [i for i in self.get_alive() if i.role == role]

    def count_alive(self) -> int:
        return len(self.get_alive())

    def count_dead(self) -> int:
        return len(self.inhabitants) - self.count_alive()

    def has_role(self, role: Role) -> bool:
        return bool(self.get_by_role(role))

    def random_survivor(self, rng: Random, role: Optional[Role] = None) -> Optional[Inhabitant]:
        pool = self.get_by_role(role) if role else self.get_alive()
        return rng.choice(pool) if pool else None

    def random_non_loyal(self, rng: Random) -> Optional[Inhabitant]:
        pool = [i for i in self.get_alive() if not i.has_trait(Trait.LOYAL)]
        return rng.choice(pool) if pool else None

    def average_morale(self) -> int:
        alive = self.get_alive()
        if not alive:
            return 50
        return sum(i.morale for i in alive) // len(alive)

    def apply_to_role(self, role: Role, health_delta: int = 0, morale_delta: int = 0) -> list[Inhabitant]:
        """Returns the list of inhabitants who died as a result."""
        deaths = []
        for i in self.get_by_role(role):
            if health_delta < 0:
                i.damage(-health_delta)
            elif health_delta > 0:
                i.heal(health_delta)
            i.apply_morale(morale_delta)
            if not i.is_alive:
                deaths.append(i)
        return deaths

    def to_dict(self) -> dict:
        return {"inhabitants": [i.to_dict() for i in self.inhabitants]}

    @classmethod
    def from_dict(cls, data: dict) -> InhabitantManager:
        manager = cls()
        manager.inhabitants = [Inhabitant.from_dict(i) for i in data.get("inhabitants", [])]
        return manager
