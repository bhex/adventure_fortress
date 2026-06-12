from __future__ import annotations
from typing import Optional

from src.core.game_state import GameState
from src.core.fortress import Upgrade
from src.core.inhabitants import Role, Trait, generate_inhabitant
from src.events.event_base import Choice, Effect, EffectKind, Event, EventResult


class EventEngine:
    def __init__(self, events: list[Event], game_state: GameState):
        self.events = events
        self.game_state = game_state
        self.last_event_name: Optional[str] = None

    # ------------------------------------------------------------------
    # Selection
    # ------------------------------------------------------------------

    def eligible_events(self, day: int) -> list[Event]:
        gs = self.game_state
        return [
            e for e in self.events
            if e.min_day <= day
            and (e.max_day is None or day <= e.max_day)
            and e.min_morale <= gs.fortress.morale <= e.max_morale
            and gs.resources.can_afford(e.min_resource)
            and (e.requires_role is None or gs.inhabitants.has_role(Role(e.requires_role)))
            and (e.requires_upgrade is None or gs.fortress.has_upgrade(Upgrade(e.requires_upgrade)))
            and e.name != self.last_event_name
        ]

    def roll(self, day: int) -> Optional[Event]:
        pool = self.eligible_events(day)
        if not pool:
            return None
        event = self.game_state.rng.choices(pool, weights=[e.weight for e in pool], k=1)[0]
        self.last_event_name = event.name
        return event

    # ------------------------------------------------------------------
    # Resolution
    # ------------------------------------------------------------------

    def choice_available(self, choice: Choice) -> bool:
        return self.game_state.resources.can_afford(choice.cost)

    def resolve(self, event: Event, choice_index: int) -> EventResult:
        choice = event.choices[choice_index]
        result = EventResult(event_name=event.name, choice_label=choice.label)

        if choice.cost:
            self.game_state.resources.apply_delta({k: -v for k, v in choice.cost.items()})
            paid = ", ".join(f"{v} {k}" for k, v in choice.cost.items())
            result.lines.append(f"Paid {paid}.")

        for effect in choice.effects:
            self._apply_effect(effect, event, result)

        self.game_state.events_resolved += 1
        return result

    def _apply_effect(self, effect: Effect, event: Event, result: EventResult):
        gs = self.game_state
        p = effect.params

        if effect.kind == EffectKind.RESOURCE:
            gs.resources.apply_delta(p)
            parts = [f"{'+' if v >= 0 else ''}{v} {k}" for k, v in p.items()]
            result.lines.append(", ".join(parts) + ".")

        elif effect.kind == EffectKind.MORALE:
            amount = p["amount"]
            gs.fortress.apply_morale_delta(amount)
            result.lines.append(f"Fortress morale {'+' if amount >= 0 else ''}{amount}.")

        elif effect.kind == EffectKind.DEFENSE:
            amount = p["amount"]
            gs.fortress.apply_defense_delta(amount)
            result.lines.append(f"Defense {'+' if amount >= 0 else ''}{amount}.")

        elif effect.kind == EffectKind.SPAWN_INHABITANT:
            if gs.inhabitants.count_alive() >= gs.fortress.max_population:
                result.lines.append("The fortress is full — they move on.")
                return
            role = Role(p["role"]) if "role" in p else gs.rng.choice(list(Role))
            newcomer = generate_inhabitant(role, gs.rng)
            gs.inhabitants.add(newcomer)
            traits = f" ({', '.join(newcomer.traits)})" if newcomer.traits else ""
            result.lines.append(f"{newcomer.name} the {newcomer.role}{traits} joins the fortress.")

        elif effect.kind == EffectKind.KILL_INHABITANT:
            role = Role(p["role"]) if "role" in p else None
            victim = gs.inhabitants.random_survivor(gs.rng, role)
            if victim:
                victim.is_alive = False
                victim.health = 0
                result.lines.append(f"{victim.name} the {victim.role} has died.")
                gs.fortress.apply_morale_delta(-3)

        elif effect.kind == EffectKind.REMOVE_INHABITANT:
            deserter = gs.inhabitants.random_non_loyal(gs.rng)
            if deserter:
                gs.inhabitants.remove(deserter.name)
                result.lines.append(f"{deserter.name} the {deserter.role} slips away in the night.")
            else:
                result.lines.append("The inhabitants stand together — no one deserts.")

        elif effect.kind == EffectKind.APPLY_TO_ROLE:
            role = Role(p["role"])
            health = p.get("health", 0)
            morale = p.get("morale", 0)
            if health < 0:
                health = self._mitigate_damage(health, event)
            deaths = gs.inhabitants.apply_to_role(role, health_delta=health, morale_delta=morale)
            if health or morale:
                desc = []
                if health:
                    desc.append(f"{'+' if health > 0 else ''}{health} health")
                if morale:
                    desc.append(f"{'+' if morale > 0 else ''}{morale} morale")
                result.lines.append(f"All {role}s: {', '.join(desc)}.")
            for dead in deaths:
                result.lines.append(f"{dead.name} the {dead.role} succumbs.")
                gs.fortress.apply_morale_delta(-3)

        elif effect.kind == EffectKind.ADD_UPGRADE:
            result.lines.append(gs.build_upgrade(Upgrade(p["name"])))

    def _mitigate_damage(self, health: int, event: Event) -> int:
        """Traits and upgrades soften incoming damage based on event tags."""
        gs = self.game_state
        if "combat" in event.tags:
            if gs.fortress.has_upgrade(Upgrade.BLACKSMITH):
                health = -(-health * 3 // 4)  # 25% reduction, rounds toward zero
            if any(i.has_trait(Trait.BRAVE) for i in gs.inhabitants.get_by_role(Role.GUARD)):
                health = -(-health * 3 // 4)
        if "disaster" in event.tags and gs.fortress.has_upgrade(Upgrade.INFIRMARY):
            health = -(-health // 2)
        return health
