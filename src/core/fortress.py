from __future__ import annotations
from dataclasses import dataclass, field
from enum import StrEnum


class Upgrade(StrEnum):
    WATCHTOWER = "Watchtower"   # +5 defense, unlocks scout events
    FARM = "Farm"               # +3 food/day
    INFIRMARY = "Infirmary"     # healers recover morale daily, softens plagues
    BLACKSMITH = "Blacksmith"   # improves combat outcomes
    GRANARY = "Granary"         # enables trade events, softens famine
    BARRACKS = "Barracks"       # +5 max population, +defense


@dataclass
class Fortress:
    name: str
    day: int = 1
    morale: int = 50
    defense: int = 10
    max_population: int = 20
    upgrades: list[Upgrade] = field(default_factory=list)

    def advance_day(self):
        self.day += 1

    def apply_morale_delta(self, amount: int):
        self.morale = max(0, min(100, self.morale + amount))

    def apply_defense_delta(self, amount: int):
        self.defense = max(0, self.defense + amount)

    def add_upgrade(self, upgrade: Upgrade):
        if upgrade not in self.upgrades:
            self.upgrades.append(upgrade)

    def has_upgrade(self, upgrade: Upgrade) -> bool:
        return upgrade in self.upgrades

    def is_defeated(self) -> bool:
        return self.morale == 0

    def to_dict(self) -> dict:
        return {
            "name": self.name,
            "day": self.day,
            "morale": self.morale,
            "defense": self.defense,
            "max_population": self.max_population,
            "upgrades": [str(u) for u in self.upgrades],
        }

    @classmethod
    def from_dict(cls, data: dict) -> Fortress:
        return cls(
            name=data["name"],
            day=data["day"],
            morale=data["morale"],
            defense=data["defense"],
            max_population=data["max_population"],
            upgrades=[Upgrade(u) for u in data["upgrades"]],
        )
