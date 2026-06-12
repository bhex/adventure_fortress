from __future__ import annotations
import json
from random import Random
from src.core.fortress import Fortress, Upgrade
from src.core.resources import Resources
from src.core.inhabitants import InhabitantManager, Role


class GameState:
    VICTORY_DAY = 30

    def __init__(self, run_seed: int | None = None):
        if run_seed is None:
            run_seed = Random().randint(0, 2**31 - 1)
        self.run_seed = run_seed
        self.rng = Random(run_seed)
        self.fortress = Fortress(name="")
        self.resources = Resources()
        self.inhabitants = InhabitantManager()
        self.events_resolved = 0

    # ------------------------------------------------------------------
    # Progression
    # ------------------------------------------------------------------

    def build_upgrade(self, upgrade: Upgrade) -> str:
        if self.fortress.has_upgrade(upgrade):
            return f"{upgrade} is already built."
        self.fortress.add_upgrade(upgrade)
        if upgrade == Upgrade.WATCHTOWER:
            self.fortress.apply_defense_delta(5)
        elif upgrade == Upgrade.BARRACKS:
            self.fortress.max_population += 5
            self.fortress.apply_defense_delta(2)
        return f"{upgrade} has been built!"

    def apply_daily_effects(self) -> list[str]:
        """Day-end passive tick: upgrades, morale cascade, starvation. Returns log lines."""
        lines: list[str] = []

        if self.fortress.has_upgrade(Upgrade.FARM):
            self.resources.apply_delta({"food": 3})
            lines.append("The farm yields 3 food.")
        if self.fortress.has_upgrade(Upgrade.INFIRMARY):
            for healer in self.inhabitants.get_by_role(Role.HEALER):
                healer.apply_morale(2)

        # Everyone eats: 1 food per 2 alive inhabitants (rounded up)
        alive = self.inhabitants.count_alive()
        if alive > 0:
            upkeep = (alive + 1) // 2
            if self.resources.food >= upkeep:
                self.resources.apply_delta({"food": -upkeep})
            else:
                self.resources.food = 0
                self.fortress.apply_morale_delta(-5)
                lines.append("Not enough food! The people go hungry. (-5 morale)")

        # Inhabitant morale cascades into fortress morale
        avg = self.inhabitants.average_morale()
        if avg >= 65:
            self.fortress.apply_morale_delta(2)
            lines.append("Spirits are high among the inhabitants. (+2 morale)")
        elif avg <= 30:
            self.fortress.apply_morale_delta(-2)
            lines.append("Grumbling spreads through the halls. (-2 morale)")

        return lines

    # ------------------------------------------------------------------
    # Win / loss
    # ------------------------------------------------------------------

    def is_game_over(self) -> bool:
        return self.fortress.is_defeated()

    def is_victory(self) -> bool:
        return self.fortress.day > self.VICTORY_DAY

    # ------------------------------------------------------------------
    # Serialization
    # ------------------------------------------------------------------

    def to_dict(self) -> dict:
        return {
            "run_seed": self.run_seed,
            "rng_state": self._encode_rng_state(),
            "events_resolved": self.events_resolved,
            "fortress": self.fortress.to_dict(),
            "resources": self.resources.to_dict(),
            "inhabitants": self.inhabitants.to_dict(),
        }

    @classmethod
    def from_dict(cls, data: dict) -> GameState:
        gs = cls(run_seed=data["run_seed"])
        if data.get("rng_state"):
            gs._decode_rng_state(data["rng_state"])
        gs.events_resolved = data.get("events_resolved", 0)
        gs.fortress = Fortress.from_dict(data["fortress"])
        gs.resources = Resources.from_dict(data["resources"])
        gs.inhabitants = InhabitantManager.from_dict(data["inhabitants"])
        return gs

    def _encode_rng_state(self) -> list:
        version, internal, gauss = self.rng.getstate()
        return [version, list(internal), gauss]

    def _decode_rng_state(self, state: list):
        version, internal, gauss = state
        self.rng.setstate((version, tuple(internal), gauss))

    def save(self, path: str):
        with open(path, "w") as f:
            json.dump(self.to_dict(), f, indent=2)

    @classmethod
    def load(cls, path: str) -> GameState:
        with open(path) as f:
            return cls.from_dict(json.load(f))
