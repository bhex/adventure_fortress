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

## Releases

`.github/workflows/rust.yml` builds + tests on every push/PR to main. `.github/workflows/release.yml` cuts cross-platform release binaries (Linux/macOS/Windows) **on a `v*` tag**: `git tag vX.Y.Z && git push origin vX.Y.Z`. Each job builds `-p fortress_game --release` for its target and uploads an archive of the binary **plus `content/`** (the game loads `content/events/*.json` from beside the executable — see `content::default_content_dir`'s exe-relative fallback). A `workflow_dispatch` run packages the same archives as artifacts without publishing (dry run). Bump `version` in `crates/fortress_game/Cargo.toml` before tagging.

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
  resources.rs    # food/valuables/wood/stone/ore/residue; numbers + adjective bands (gear/tools retired — items are the sole combat/work backbone)
  items.rs        # ItemStock armory + per-bearer Loadout{weapon,armor,tool}; typed, named Item{kind,quality,enchant:(Enchant,EnchantTier),condition,artifact,form,material} — ItemForm/Material name it, quality+enchant drive the rating; Enchant: Keen/Guarding/Tireless/Warding/Vital/Hexed × Lesser|Greater
  inhabitants.rs  # Inhabitant list: Role (incl. Miner) + Trait enums, damage/heal/morale
  player.rs       # PlayerCharacter (the commander) + ClassKind
  skills.rs       # SkillSet: skills × tiers, xp-based growth
  region.rs       # the darkness war: positioned Sites (Coord on a REGION_W×REGION_H grid, fortress at FORTRESS_POS centre), demon portals, darkness 0-100 + spatial blight reach, portal-proximity fall order, distance-based expedition_days, refugees, world end-state (survivor camps)
  world.rs        # the turning year: Season + Weather, derived from (run_seed, day); farm/heating/morale/combat modifiers
  adventurers.rs  # heroes who arrive on renown/darkness
  battle.rs       # fight_battle -> multi-round, per-combatant, narrated BattleReport
  engine.rs       # roll() (quiet-day gate + eligibility), resolve(), apply_effect, mitigate_damage, auto_pick() (heuristic event picker)
  events.rs       # Event/Choice/Effect/StatCheck dataclasses + serde
  content.rs      # JSON loader (globs content/events/*.json)
  examples/sim.rs # headless auto-mode bot + survival harness (`sim band` sweeps seeds 1..=20, tallies survived/starved/morale-0/commander-fell)

crates/fortress_game/         # Bevy front-end (untested by design)
  main.rs, ui.rs, map.rs, actors.rs, clock.rs, modal.rs, build.rs,
  region_map.rs (overworld terrain+roads, value-noise from run_seed, never serialized),
  region_panel.rs (regional map view: half-block terrain into the shared terminal + docked panel),
  roster.rs, charcreate.rs, gameover.rs, picking.rs, bridge.rs
```

Game loop (event-driven, `clock.rs`'s `DayCycle`): each day *arrives* with a short dawn gradient sweep (the map lightens night→day over `SWEEP_SECONDS`) while `engine::roll()` picks today's event (or a quiet day) → when the sweep completes the event fires as a modal (or auto-resolves if `event.auto`, no choice, or auto-mode) → `resolve()` pays cost and dispatches effects → `bridge::finish_day` runs `apply_daily_effects()` + `advance_day()` + autosave (inline for the auto paths; in `settle_after_modal` on returning from the modal) → the day **settles** and waits for the player to advance (Space / N / the HUD "next day" button), or auto-advances under auto-mode. There is no real-time clock and no speed controls; the map is static (no wandering — `actors.rs` places each soul at their work station) and only redraws when the gradient, layout, actors, hover, or selection change. Defeat at morale 0 **or** if the commander falls.

- **Events are pure data**; `engine::eligible_events` gates by day/morale/resources/role/upgrade/darkness and **story flags** (`requires_flags`/`forbids_flags`); `Effect::SetFlag`/`ClearFlag` drive multi-step arcs.
- **Damage mitigation** is tag-driven (`engine::mitigate_damage`): `combat` softened by Blacksmith/skill/gear/armor-items/class; `disaster` halved by Infirmary; `demon` scaled up by darkness and softened by the Shrine.
- **Items & owned loadouts** (`items.rs`, `game_state.rs`): items are the **sole** combat/work backbone — the bulk `gear`/`tools` resources are retired. The forge turns **ore** into typed, **named** quality items (`Item::crafted` picks a `form` from `Fortress.craft_focus`'s repertoire and a `material` from the smith's tier — e.g. "fine steel sword"; `label()` reads quality→enchant→material→form; quality alone drives the rating); the Wizard Tower binds enchants with **residue** (dropped by demon battles). Raider loot drops a named item or melts to **ore**; gear/tools content references migrated to `GrantItem`/ore/valuables.
- **Enchantment 2.0** (`game_state::work_the_wizard_tower`/`pick_binding`, `items.rs`): the Wizard Tower's daily working is hands-off but **threat-aware + tiered**. `pick_binding` (rng-free) chooses by the pressing threat — **Warding** when `region.darkness` is Deep/Overwhelming (on the best armor, else a blade), **Vital** when morale < 40 (on armor), else the kind's natural enchant (`Enchant::for_kind`). Tier is **Greater** when the best mage (`best_magic_tier`, max over MAGIC skills) is ≥ Skilled and residue ≥ 6, else **Lesser** (residue ≥ 3). An **Expert+** mage instead spends a day **lifting a Hexed curse** (`first_cursed_index`, residue 5); only below Master is a 30% rng botch possible. **Warding** softens demon damage in `mitigate_damage` via `best_equipped_ward` (Greater halves, Lesser −25%); **Vital** lifts morale daily via `best_equipped_vital` (Greater +2/Lesser +1). One working per day. Effects read off the enchant tier; `label()` names a Greater binding (e.g. "fine greater warding steel plate"). Each `Inhabitant`/`PlayerCharacter`/`Adventurer` owns a `Loadout{weapon,armor,tool}`; `GameState::redistribute_equipment` runs each daily tick (and at the start of every battle) to **pool all items + reassign best-to-best by role** — weapons/armor to the ablest fighters (commander, guards, knight heroes), tools to the most skilled workers — leftovers back to the armory. Deterministic (no rng; stable sort). Items wear per-bearer (`Loadout::degrade_in_use`); the smith's `maintain_equipment` keeps held + armory gear in trim. Artifacts (rare, sometimes cursed) arrive via `Effect::GrantItem` and never degrade.
- **Combat** (`battle.rs`): resolves over up to `MAX_ROUNDS` rounds with swinging `momentum`; per-combatant actions (commander/heroes strike, casters bolt with Sorcery or blunt the foe with Warding, guards hold the line); a `BREACH_AT` momentum gates the gate-breach that throws non-combatant reserves to the wall. Each fighter's **own** weapon folds into their push at muster; armor mitigation reads the best armor actually **worn** by a defender (`GameState::best_combat_armor`, used in `engine::mitigate_damage`).
- **Morale passive**: high morale (≥75) adds a combat edge and (≥80) a day's extra practice + faster job drift; `Effect::Morale` overflow above the 100 cap converts to permanent renown so it isn't wasted.
- **Seasons & weather** (`world.rs`): `GameState.world` is recomputed each tick from `(run_seed, day)` — **never** via `gs.rng`, so it perturbs no other draw. Season (12-day quarters) and weather modulate farm yield, heating burn, morale, and combat (`side_strength`). Seasonal content events gate on `Event.requires_season`. **World end-state**: when `region.all_fallen()`, `diplomacy`/`trade` events are suppressed (no outside world) until survivors regroup into `SiteKind::Survivors` camps that can grow back.
- **Auto-mode** (`engine::auto_pick` + `GameState::auto_build_pick`): a Progress-Quest-style toggle (`AutoMode` resource in the game; `A` key or the HUD `auto` button). `auto_pick` scores each available choice (`score_choice`/`effect_score`) and returns the best; `auto_build_pick` picks the next building to raise (survival economy → capacity → strength priority, new before tiering up); `clock.rs` (dawn) and the `sim` example drive both, so a hold left on auto plays events *and* builds itself.
- **Balance / survival** (Stage 5): the harvest scales with field hands (`2*farmers + peasants/2`) so a growing hold feeds itself; Granary food caps (60/100/160/220, +60 DeepCellars) let a summer surplus bridge winter. ~17/20 auto-runs survive to day 150 (`sim band`); the rest mostly lose the *darkness war* (darkness pinned at 100). Regression guard: `an_auto_run_survives_well_past_the_turning_seasons` (content test, seed 4 to day 120).
- **Settlement growth** (`fortress.rs`): `SettlementTier` (Hamlet→Village→Town→City) scaffolds fortress→town→city. Each daily tick `try_promote(alive)` checks crowding (≥80% of `max_population`) plus a buildings-built threshold, then bumps the cap and tier. `map.rs` has a `Zone` stub (Keep/Commons/CraftQuarter/Fields/Walls) for a future district pass. No full town yet — just the data seams.
- **Construction** (`fortress.rs`): `construct()` pays materials up front and **enqueues** a `BuildProject` (`worker_days_remaining`); the daily tick's `advance_projects(workforce)` draws it down by `build_workforce()` (laborers — peasants/miners — plus a baseline) and calls `build_upgrade` on completion. Event-granted buildings (`Effect::AddUpgrade`) and the founding charter still build instantly via `build_upgrade`. `BuildAvailability::InProgress` gates re-queuing; the build menu shows ETA.
- **Fortress features** (`fortress.rs`): `FortressFeature` (Ramparts/DeepCellars/GreatHearth/MasterForge) — one rare permanent boon per run, granted by `maybe_grant_feature` on a low daily roll once renown ≥ 50; effects read off `has_feature` where they apply (defense, food cap, heating burn, craft quality).
- **Daily tick**: building yields, food upkeep, starvation/sleep/morale cascades, skill training, craft/enchant/maintain, region tick, refugee/hero arrivals, settlement promotion.

## Conventions

- All state mutation through model methods (`apply_morale_delta`, `damage`, `apply_delta`, `add_building`) — never raw attribute math in the engine.
- New core logic gets a test in `crates/fortress_core/tests/`; UI is untested by design.
- Bump `SAVE_VERSION` (game_state.rs) whenever the save shape changes; `#[serde(default)]` for additive fields.
- `save.json` (project root) is the autosave — deleted on game end, gitignored.
