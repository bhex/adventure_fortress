# ⚔ Adventure Fortress

A real-time terminal roguelite where you command a fortress on the edge of a
darkening fantasy world. The clock runs, the seasons turn, and the regional war
against the demon portals grinds on beyond your walls. Events force choices,
choices have costs, and your commander — like everyone else — can fall. Hold the
line as long as you can.

Built in Rust as a Bevy game (`fortress_game`) on top of a pure, deterministic
logic crate (`fortress_core`). Game content lives as engine-agnostic JSON in
`content/events/`.

<img width="1918" height="1078" alt="{89FF470D-DCBD-40CA-8A8B-35BFB5FC9F9C}" src="https://github.com/user-attachments/assets/76c0661a-3c24-4b3d-99d1-40e80766b3d5" />

<img width="1918" height="1078" alt="{F2835613-E6FA-4B91-A928-9B86D7A71701}" src="https://github.com/user-attachments/assets/549993a3-e425-41c9-9807-255f8f6a7c16" />

<img width="1918" height="1078" alt="{D86E4DC7-1CA1-48F0-82B4-670057160D3D}" src="https://github.com/user-attachments/assets/904d8453-7312-4103-821d-c50893ed925d" />


## Download & run

Prebuilt binaries for **Linux**, **macOS** (Apple silicon & Intel), and **Windows** are
published on the [Releases page](../../releases). Download the archive for your
system, unpack it, and run the `fortress_game` executable **from inside the
unpacked folder** — the game loads `content/` from beside the executable, so keep
them together. Saves are written to `save.json` in that folder.

On Linux you still need Bevy's runtime libraries (see
[Linux build dependencies](#linux-build-dependencies)).

## Play (from source)

```bash
cargo run -p fortress_game        # play (opens a window)
```

## Develop

```bash
cargo test                         # full workspace test suite
cargo run -p fortress_core --example sim 42 150   # headless simulation (seed, days)
cargo clippy --workspace
```

### Linux build dependencies

The game crate uses Bevy, which links against system libraries:

```bash
sudo apt-get install -y pkg-config libasound2-dev libudev-dev \
  libwayland-dev libxkbcommon-dev
```

## Layout

- `crates/fortress_core` — deterministic game logic: state, events engine,
  region/darkness sim, battles, skills. No UI, fully tested.
- `crates/fortress_game` — the Bevy front-end: ASCII map, panels, clock, modals.
- `content/events/*.json` — all events, as data the engine reads at runtime.
