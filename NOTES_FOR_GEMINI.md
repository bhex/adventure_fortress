# Handoff notes (Claude → Gemini)

Context: the previous expansion (your Phase 1–3: Scholar/Herbalist roles,
Market/Alchemist/Library, expeditions + artifact quest, famine/mutiny/diplomacy
tags) is **merged into `main`** along with a follow-up pass. Start here.

## Current state
- Branch: `main`, clean. Last real work landed via merged **PR #1** (`75ffc1f`).
- `cargo test -p fortress_core` → **138 pass**. `cargo run -p fortress_core --example sim band` → **16/20** survive to day 150.
- `SAVE_VERSION = 17`. Read `CLAUDE.md` first — it's the architecture source of truth and is accurate.

## What this pass added (on top of your work)
1. **Crash fix** — Graveyard glyph `'†'` isn't in the terminal's CP437 font and panicked `bevy_ascii_terminal`. Now `'+'`. See gotcha below.
2. **Class-gated event choices** — `Choice.requires_class: Option<ClassKind>` (events.rs); engine `ChoiceAvailability::ClassLocked` (engine.rs); modal renders it greyed; auto-pick scoring nudge. Content: `content/events/class_deeds.json`.
3. **Class-specific battle** — `Doctrine` per `ClassKind` in `battle.rs` (`class_doctrine`): rally / extra_ward / opening_strike / soften + a muster narration line. Warlock pays morale.
4. **Building-driven events** — `content/events/building_life.json`, gated via existing `Event.requires_upgrade`.
5. **Market reverse-trade** — Market also buys scarce wood/ore with valuables (`game_state.rs`, in the Market daily block).
6. **Build queue** (replaced the old pledge-to-build) — `Fortress.projects` is now a **strict-FIFO queue**. `BuildProject` carries `funded`/`materials_owed`; `GameState::queue_build` appends an order (affordable or "on credit"), `try_fund_front` pays the front order in full the moment it's affordable (never out of order), `advance_projects` only labors the funded front. `Fortress::move_project` reorders, `GameState::cancel_build` cancels (refunding a funded order). Build menu has a queue panel with ▲/▼/✕.
7. **Keep building** — `Upgrade::Keep`: tier I from founding (no stored entry; `Fortress::keep_level()` clamps to ≥1), upgradeable to III for beds/defense/pop. Special-cased in `add_building`/`next_build_level`/`sleeping_capacity`.
8. **Starvation cascade** — `GameState.famine_days` + `deepen_famine`: deepening toll → weakest starve → cannibalism/murder/madness/betrayal once grim, gated by `famine_crisis` flag.
9. **Wild foraging** — when the hold lacks a Lumberyard/Mine/Farm, its laborers (`Peasant`/`Miner`, plus `Farmer`s for food) gather wood/stone/food from the wild (`apply_daily_effects`). Capped below a built yield (wood ≤4, stone ≤3, **no ore**, food ≤8) and scaled by the region darkness band (`forage_pct`: 100/75/40/15) — so it makes the opening days survivable but the dark chokes it off, and a built yield replaces it. No laborers, no forage.

## Gotchas (important)
- **CP437 only**: every `char` drawn by the terminal must be in code page 437. `'█'` (0xDB) is fine; `'†'` is not. Check `map.rs` `building_glyph`/`role_glyph` before adding glyphs.
- **Determinism**: all game randomness goes through `gs.rng`. The famine cascade draws rng **only when food hits 0**, deliberately — so well-fed runs stay bit-for-bit identical and the survival regression tests hold. Keep any new daily-tick rng behind a condition that doesn't fire on normal runs, or you'll shift every seed.
- **`ClassKind` serializes PascalCase** in JSON (`"requires_class": "Warlord"`), unlike `Role`/`StatKind` which are lowercase. `Upgrade` is also PascalCase in content.
- **Auto-build priority** (`game_state.rs auto_build_pick`) is a fixed list that intentionally excludes Keep, Market, Alchemist, Library — adding them changes sim balance (the ~16/20). Decide deliberately if you want auto-mode to build them.
- **`ResourceDelta` is now `Copy`** — don't `.clone()` it (clippy will complain).
- Bump `SAVE_VERSION` + `#[serde(default)]` new fields whenever the save shape changes.
- New core logic needs a test in `crates/fortress_core/tests/`; UI is untested by design.

## Possible next steps (none committed to)
- Let auto-mode invest in Keep/Market/Alchemist/Library (re-tune sim).
- More class-gated content; richer per-class battle specials (once-per-battle moves).
- Famine-crisis *events* (player choices) gated on the `famine_crisis` flag, not just automatic outcomes.
- District/Zone pass (map.rs has a `Zone` stub) now that the Keep is real.

— Claude
