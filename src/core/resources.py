from __future__ import annotations
from dataclasses import dataclass


@dataclass
class Resources:
    food: int = 0
    gold: int = 0
    stone: int = 0
    wood: int = 0

    def apply_delta(self, delta: dict):
        self.food += delta.get("food", 0)
        self.gold += delta.get("gold", 0)
        self.stone += delta.get("stone", 0)
        self.wood += delta.get("wood", 0)
        self.clamp()

    def can_afford(self, cost: dict) -> bool:
        return (
            self.food >= cost.get("food", 0)
            and self.gold >= cost.get("gold", 0)
            and self.stone >= cost.get("stone", 0)
            and self.wood >= cost.get("wood", 0)
        )

    def clamp(self):
        self.food = max(0, self.food)
        self.gold = max(0, self.gold)
        self.stone = max(0, self.stone)
        self.wood = max(0, self.wood)

    def to_dict(self) -> dict:
        return {"food": self.food, "gold": self.gold, "stone": self.stone, "wood": self.wood}

    @classmethod
    def from_dict(cls, data: dict) -> Resources:
        return cls(
            food=data.get("food", 0),
            gold=data.get("gold", 0),
            stone=data.get("stone", 0),
            wood=data.get("wood", 0),
        )
