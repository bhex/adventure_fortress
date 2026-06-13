# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo run -p fortress_game        # play (opens a Bevy window)
cargo test                        # full workspace test suite
cargo test -p fortress_core       # core logic + content tests only
cargo run -p fortress_core --example sim 42 150   # headless sim: seed, max days
cargo clippy --workspace
```

Linux build needs Bevy's system libs: `pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.

## Core conventions (port-friendly, deterministic)

- **Enums for every closed set** (`EffectKind`/`Effect`, `Role`, `Trait`, `Upgrade`, `ClassKind`, `Skill`) — never bare magic strings. Open, content-defined sets (event `tags`, story `flags`) are `String`, matching how the JSON authors them.
- **No reflection** — explicit `to_dict`/`from_dict`-style (here: serde) everywhere; no dynamic dispatch by name.
- **Game content is data**: events live in `content/events/*.json` and stay engine-agnostic. To add an event, edit JSON — never touch the engine. To add an effect type, add an `Effect` variant plus one arm in `engine::apply_effect`.
- **Deterministic core**: all randomness goes through `GameState.rng` (a seeded `GameRng`), never thread-local rng. RNG state is serialized in saves so restored runs continue identically. UI-only randomness (e.g. event fire-hour) uses `rand::rng()` and never touches game state.
- **Core never imports UI**: `crates/fortress_core` is pure logic; all rendering/I/O lives in `crates/fortress_game`.

## Architecture

Two crates in a Cargo workspace:

```
crates/fortress_core/         # pure, deterministic, fully tested
  game_state.rs   # owns rng, daily tick, save/load, win/loss, SAVE_VERSION
  fortress.rs     # day, morale, defense, buildings (Building{kind,level}), build costs
  resources.rs    # food/valuables/wood/stone/gear/tools; numbers + adjective bands
  inhabitants.rs  # Inhabitant list: Role + Trait enums, damage/heal/morale
  player.rs       # PlayerCharacter (the commander) + ClassKind
  skills.rs       # SkillSet: skills × tiers, xp-based growth
  region.rs       # the darkness war: Sites, darkness 0-100, portal pressure, refugee waves
  adventurers.rs  # heroes who arrive on renown/darkness
  battle.rs       # fight_battle -> narrated BattleReport
  engine.rs       # roll() (quiet-day gate + eligibility), resolve(), apply_effect, mitigate_damage
  events.rs       # Event/Choice/Effect/StatCheck dataclasses + serde
  content.rs      # JSON loader (globs content/events/*.json)
  examples/sim.rs # headless bot for verification

crates/fortress_game/         # Bevy front-end (untested by design)
  main.rs, ui.rs, map.rs, actors.rs, clock.rs, modal.rs, build.rs,
  region_panel.rs, roster.rs, charcreate.rs, gameover.rs, picking.rs, bridge.rs
```

Game loop: clock runs in real time → at dawn `engine::roll()` picks today's event (or a quiet day) → fires at a random hour as a modal (or auto-resolves if `event.auto`) → `resolve()` pays cost and dispatches effects → at midnight `apply_daily_effects()` runs the daily tick and `advance_day()` + autosave. Defeat at morale 0 **or** if the commander falls.

- **Events are pure data**; `engine::eligible_events` gates by day/morale/resources/role/upgrade/darkness and **story flags** (`requires_flags`/`forbids_flags`); `Effect::SetFlag`/`ClearFlag` drive multi-step arcs.
- **Damage mitigation** is tag-driven (`engine::mitigate_damage`): `combat` softened by Blacksmith/skill/gear/class; `disaster` halved by Infirmary; `demon` scaled up by darkness and softened by the Shrine.
- **Daily tick**: building yields, food upkeep, starvation/sleep/morale cascades, skill training, region tick, refugee/hero arrivals.

## Conventions

- All state mutation through model methods (`apply_morale_delta`, `damage`, `apply_delta`, `add_building`) — never raw attribute math in the engine.
- New core logic gets a test in `crates/fortress_core/tests/`; UI is untested by design.
- Bump `SAVE_VERSION` (game_state.rs) whenever the save shape changes; `#[serde(default)]` for additive fields.
- `save.json` (project root) is the autosave — deleted on game end, gitignored.
