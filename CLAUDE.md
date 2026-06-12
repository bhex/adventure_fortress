# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
python main.py          # play (run from project root)
pytest tests/ -v        # full test suite
pytest tests/test_event_engine.py -v   # single test file
pip install -r requirements.txt        # rich + pytest
```

## Strategy constraint: Rust port

This Python codebase is a **prototype that will be ported to Rust**. All code must stay port-friendly:

- `StrEnum` for every closed string set (`EffectKind`, `Role`, `Trait`, `Upgrade`) — never bare magic strings
- **No reflection** — no `getattr`/`setattr`/`hasattr` dispatch, no `**data` unpacking into constructors; explicit `to_dict`/`from_dict` everywhere
- **Game content is data**: events live in `content/events/*.json` and must remain engine-agnostic — the Rust port reuses these files verbatim
- **Deterministic core**: all randomness goes through `GameState.rng` (a `random.Random` seeded by `run_seed`), never the global `random` module. RNG state is serialized in saves so restored runs continue identically
- Pure logic (`src/core/`, `src/events/`) never imports UI; all I/O lives in `main.py` and `src/ui/display.py`

## Architecture

Game loop (in `main.py`): show status → `EventEngine.roll()` → player picks a choice → `EventEngine.resolve()` → `GameState.apply_daily_effects()` → `advance_day()` → auto-save. Defeat at morale 0; victory after day 30.

```
GameState (src/core/game_state.py)      # owns rng, win/loss, daily tick, save/load
  ├─ Fortress      # day, morale, defense, upgrades (Upgrade enum)
  ├─ Resources     # food/gold/wood/stone, apply_delta/can_afford
  └─ InhabitantManager  # Inhabitant list: Role + Trait enums, damage/morale helpers

EventEngine (src/events/event_engine.py)
  ├─ roll()     # filters events by day/morale/resources/role/upgrade; weighted pick; never repeats last event
  └─ resolve()  # pays Choice.cost, then dispatches each Effect by EffectKind — no per-event logic
```

**Events are pure data.** `src/events/event_pool.py` is just a JSON loader; `event_base.py` holds the dataclasses (`Event`, `Choice`, `Effect`, `EventResult`) and their `from_dict` parsers. To add an event, edit a JSON file in `content/events/` — never touch the engine. To add a new effect type, add an `EffectKind` member plus one branch in `EventEngine._apply_effect()`.

**Damage mitigation** is tag-driven (`EventEngine._mitigate_damage`): `combat` events are softened by the Blacksmith upgrade and brave guards; `disaster` events are halved by the Infirmary.

**Daily tick** (`GameState.apply_daily_effects`): Farm yields food, food upkeep (1 per 2 inhabitants), starvation morale penalty, inhabitant-morale cascade into fortress morale.

## Conventions

- Dataclasses for all models; type hints everywhere; `from __future__ import annotations`
- All state mutation through model methods (`apply_morale_delta`, `damage`, `apply_delta`) — never raw attribute math in the engine
- New core logic gets a pytest test; UI code is untested by design
- `save.json` (project root) is the autosave — deleted on game end, gitignored
