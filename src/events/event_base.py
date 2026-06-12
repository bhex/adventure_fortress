from __future__ import annotations
from dataclasses import dataclass, field
from enum import StrEnum
from typing import Optional


class EffectKind(StrEnum):
    RESOURCE = "resource"              # params: {<resource_name>: delta, ...}
    MORALE = "morale"                  # params: {"amount": int}
    DEFENSE = "defense"                # params: {"amount": int}
    SPAWN_INHABITANT = "spawn_inhabitant"  # params: {"role": str} or {} for random role
    KILL_INHABITANT = "kill_inhabitant"    # params: {"role": str (optional)}
    REMOVE_INHABITANT = "remove_inhabitant"  # desertion: removes a random non-loyal inhabitant
    APPLY_TO_ROLE = "apply_to_role"    # params: {"role": str, "health": int, "morale": int}
    ADD_UPGRADE = "add_upgrade"        # params: {"name": str}


@dataclass
class Effect:
    """Atomic mutation applied to GameState when a Choice is resolved."""
    kind: EffectKind
    params: dict

    def to_dict(self) -> dict:
        return {"kind": str(self.kind), "params": self.params}

    @classmethod
    def from_dict(cls, data: dict) -> Effect:
        return cls(kind=EffectKind(data["kind"]), params=data.get("params", {}))


@dataclass
class Choice:
    """One option the player can select for an Event."""
    label: str
    description: str
    effects: list[Effect]
    cost: dict = field(default_factory=dict)  # resources required to enable this choice

    @classmethod
    def from_dict(cls, data: dict) -> Choice:
        return cls(
            label=data["label"],
            description=data.get("description", ""),
            effects=[Effect.from_dict(e) for e in data.get("effects", [])],
            cost=data.get("cost", {}),
        )


@dataclass
class EventResult:
    event_name: str
    choice_label: str
    lines: list[str] = field(default_factory=list)


@dataclass
class Event:
    name: str
    description: str
    choices: list[Choice]
    # filtering constraints
    min_day: int = 1
    max_day: Optional[int] = None
    min_morale: int = 0
    max_morale: int = 100
    min_resource: dict = field(default_factory=dict)
    requires_role: Optional[str] = None
    requires_upgrade: Optional[str] = None
    tags: list[str] = field(default_factory=list)
    weight: float = 1.0

    @classmethod
    def from_dict(cls, data: dict) -> Event:
        return cls(
            name=data["name"],
            description=data["description"],
            choices=[Choice.from_dict(c) for c in data["choices"]],
            min_day=data.get("min_day", 1),
            max_day=data.get("max_day"),
            min_morale=data.get("min_morale", 0),
            max_morale=data.get("max_morale", 100),
            min_resource=data.get("min_resource", {}),
            requires_role=data.get("requires_role"),
            requires_upgrade=data.get("requires_upgrade"),
            tags=data.get("tags", []),
            weight=data.get("weight", 1.0),
        )
