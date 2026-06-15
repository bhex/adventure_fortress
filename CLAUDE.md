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
  fortress.rs     # day, morale, defense, buildings (Building{kind,level}), build costs + worker-days, BuildProject queue, FortressFeature, SettlementTier (hamlet→city) + try_promote
  resources.rs    # food/valuables/wood/stone/gear/tools/ore/residue; numbers + adjective bands
  items.rs        # ItemStock: typed Item{kind,quality,enchant,condition,artifact}; auto-equip ratings
  inhabitants.rs  # Inhabitant list: Role (incl. Miner) + Trait enums, damage/heal/morale
  player.rs       # PlayerCharacter (the commander) + ClassKind
  skills.rs       # SkillSet: skills × tiers, xp-based growth
  region.rs       # the darkness war: Sites, darkness 0-100, portal pressure, refugees, world end-state (survivor camps)
  world.rs        # the turning year: Season + Weather, derived from (run_seed, day); farm/heating/morale/combat modifiers
  adventurers.rs  # heroes who arrive on renown/darkness
  battle.rs       # fight_battle -> multi-round, per-combatant, narrated BattleReport
  engine.rs       # roll() (quiet-day gate + eligibility), resolve(), apply_effect, mitigate_damage, auto_pick() (heuristic event picker)
  events.rs       # Event/Choice/Effect/StatCheck dataclasses + serde
  content.rs      # JSON loader (globs content/events/*.json)
  examples/sim.rs # headless bot for verification

crates/fortress_game/         # Bevy front-end (untested by design)
  main.rs, ui.rs, map.rs, actors.rs, clock.rs, modal.rs, build.rs,
  region_panel.rs, roster.rs, charcreate.rs, gameover.rs, picking.rs, bridge.rs
```

Game loop: clock runs in real time → at dawn `engine::roll()` picks today's event (or a quiet day) → fires at a random hour as a modal (or auto-resolves if `event.auto`) → `resolve()` pays cost and dispatches effects → at midnight `apply_daily_effects()` runs the daily tick and `advance_day()` + autosave. Defeat at morale 0 **or** if the commander falls.

- **Events are pure data**; `engine::eligible_events` gates by day/morale/resources/role/upgrade/darkness and **story flags** (`requires_flags`/`forbids_flags`); `Effect::SetFlag`/`ClearFlag` drive multi-step arcs.
- **Damage mitigation** is tag-driven (`engine::mitigate_damage`): `combat` softened by Blacksmith/skill/gear/armor-items/class; `disaster` halved by Infirmary; `demon` scaled up by darkness and softened by the Shrine.
- **Items** (`items.rs`): the forge turns **ore** into typed quality items (`Fortress.craft_focus`), the Wizard Tower binds enchants with **residue** (dropped by demon battles), the best items auto-equip into combat/work via `equip_rating`, and items wear out unless the smith maintains them. Artifacts (rare, sometimes cursed) arrive via `Effect::GrantItem` event chains and never degrade.
- **Combat** (`battle.rs`): resolves over up to `MAX_ROUNDS` rounds with swinging `momentum`; per-combatant actions (commander/heroes strike, casters bolt with Sorcery or blunt the foe with Warding, guards hold the line); a `BREACH_AT` momentum gates the gate-breach that throws non-combatant reserves to the wall. Weapons/armor/morale all feed prowess.
- **Morale passive**: high morale (≥75) adds a combat edge and (≥80) a day's extra practice + faster job drift; `Effect::Morale` overflow above the 100 cap converts to permanent renown so it isn't wasted.
- **Seasons & weather** (`world.rs`): `GameState.world` is recomputed each tick from `(run_seed, day)` — **never** via `gs.rng`, so it perturbs no other draw. Season (12-day quarters) and weather modulate farm yield, heating burn, morale, and combat (`side_strength`). Seasonal content events gate on `Event.requires_season`. **World end-state**: when `region.all_fallen()`, `diplomacy`/`trade` events are suppressed (no outside world) until survivors regroup into `SiteKind::Survivors` camps that can grow back.
- **Auto-mode** (`engine::auto_pick`): a Progress-Quest-style toggle (`AutoMode` resource in the game; `A` key or the HUD `auto` button). `auto_pick` scores each available choice (`score_choice`/`effect_score`) and returns the best; `clock.rs` resolves it without a modal. The `sim` example and a content test drive whole runs through it.
- **Settlement growth** (`fortress.rs`): `SettlementTier` (Hamlet→Village→Town→City) scaffolds fortress→town→city. Each daily tick `try_promote(alive)` checks crowding (≥80% of `max_population`) plus a buildings-built threshold, then bumps the cap and tier. `map.rs` has a `Zone` stub (Keep/Commons/CraftQuarter/Fields/Walls) for a future district pass. No full town yet — just the data seams.
- **Construction** (`fortress.rs`): `construct()` pays materials up front and **enqueues** a `BuildProject` (`worker_days_remaining`); the daily tick's `advance_projects(workforce)` draws it down by `build_workforce()` (laborers — peasants/miners — plus a baseline) and calls `build_upgrade` on completion. Event-granted buildings (`Effect::AddUpgrade`) and the founding charter still build instantly via `build_upgrade`. `BuildAvailability::InProgress` gates re-queuing; the build menu shows ETA.
- **Fortress features** (`fortress.rs`): `FortressFeature` (Ramparts/DeepCellars/GreatHearth/MasterForge) — one rare permanent boon per run, granted by `maybe_grant_feature` on a low daily roll once renown ≥ 50; effects read off `has_feature` where they apply (defense, food cap, heating burn, craft quality).
- **Daily tick**: building yields, food upkeep, starvation/sleep/morale cascades, skill training, craft/enchant/maintain, region tick, refugee/hero arrivals, settlement promotion.

## Conventions

- All state mutation through model methods (`apply_morale_delta`, `damage`, `apply_delta`, `add_building`) — never raw attribute math in the engine.
- New core logic gets a test in `crates/fortress_core/tests/`; UI is untested by design.
- Bump `SAVE_VERSION` (game_state.rs) whenever the save shape changes; `#[serde(default)]` for additive fields.
- `save.json` (project root) is the autosave — deleted on game end, gitignored.
