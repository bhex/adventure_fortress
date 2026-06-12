# ⚔ Adventure Fortress

A terminal roguelite where you command a fortress on the edge of a dangerous fantasy world. Every day brings an event; every event forces a choice; every choice has a cost. Survive 30 days — if morale hits zero, the fortress falls.

Built with Python and [rich](https://github.com/Textualize/rich) for a fully colored, panel-driven terminal experience.

## Play

```bash
git clone <repo-url> adventure_fortress
cd adventure_fortress
pip install -r requirements.txt
python main.py
```

## Features

- **40 hand-written events** across three escalating acts — raids, plagues, festivals, dragons, sieges
- **Meaningful choices** with asymmetric tradeoffs and resource costs; unaffordable options are greyed out, not hidden
- **Six fortress upgrades** (Watchtower, Farm, Infirmary, Blacksmith, Granary, Barracks) with passive daily effects and upgrade-gated events
- **Living inhabitants** with names, roles, and traits (`brave`, `loyal`, `sickly`...) that change event outcomes
- **Auto-save every day** — quit anytime, continue where you left off
- **Seeded runs** — every game is reproducible from its run seed, shown on the final screen

## How it works

The engine is fully data-driven: events live as JSON in `content/events/` and are resolved by a generic effect dispatcher. Adding an event requires zero engine code — just JSON:

```json
{
  "name": "Hidden Cache",
  "description": "Repairs to the cellar wall reveal a forgotten strongbox.",
  "tags": ["economy"],
  "min_day": 3,
  "choices": [
    {
      "label": "Crack it open",
      "description": "Whatever's inside is yours now.",
      "effects": [
        {"kind": "resource", "params": {"gold": 20}},
        {"kind": "morale", "params": {"amount": 3}}
      ]
    }
  ]
}
```

Effect kinds: `resource`, `morale`, `defense`, `spawn_inhabitant`, `kill_inhabitant`, `remove_inhabitant`, `apply_to_role`, `add_upgrade`. Events can be gated by day range, morale band, resources, roles, or built upgrades.

## Development

```bash
pip install -r requirements.txt
pytest tests/ -v        # run the test suite
python main.py          # play
```

The core game logic is deliberately free of Python-specific tricks (no reflection, enums everywhere, injected RNG, JSON-stable schemas) — it is designed to be ported to Rust, with the `content/` files reused verbatim.

## Project layout

```
main.py                  # game loop and I/O orchestration
content/events/*.json    # all game content (data, no code)
src/core/                # pure game state: fortress, resources, inhabitants
src/events/              # event model, JSON loader, effect dispatcher
src/ui/display.py        # all rendering (rich)
tests/                   # pytest suite for core logic
```
