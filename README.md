# HAT: Huge-Ass Trains

An overhead rail-network game about stupidly long trains, honest yard work,
real logistics and engineering trade-offs. Rust on Bevy.

Design docs live in `docs/`. Code is a Cargo workspace under `crates/`. Four scenarios are
playable: Yard Shift, Doubling, the Terminal with its main-line loop, hump and road
traffic (the default), and the Branch, an open-country loop with industries, scheduled
trains and money.


## Docs

| Doc | What it holds |
|---|---|
| [docs/vision.md](docs/vision.md) | Pitch, pillars, anti-goals, genre notes |
| [docs/goals.md](docs/goals.md) | Milestones and what "done" means for each |
| [docs/prototype.md](docs/prototype.md) | Yard Shift: the shortest path to a playable build, build order, cut list |
| [docs/crew.md](docs/crew.md) | Orders, plan and execution: the crew works the yard from a switch list or a train from a schedule, with manual override |
| [docs/economy.md](docs/economy.md) | Industries, stockpiles, trucks as the competitor, what shippers pay, running costs, wrecks |
| [docs/features.md](docs/features.md) | Feature tracker by area, with milestone and status |
| [docs/open-questions.md](docs/open-questions.md) | Unresolved design and tech questions |
| [docs/architecture.md](docs/architecture.md) | Engine, crate layout, sim/render wall, data layout, LOD |
| [docs/art.md](docs/art.md) | Art direction, zoom bands, palette, asset pipeline, tools |
| [docs/sim/units.md](docs/sim/units.md) | SI conventions, numeric types, display conversion |
| [docs/sim/train-dynamics.md](docs/sim/train-dynamics.md) | 1D longitudinal model, couplers, brakes, derailment |
| [docs/sim/car-model.md](docs/sim/car-model.md) | Box model, mass, center of gravity vs. loading |
| [docs/sim/lateral-motion.md](docs/sim/lateral-motion.md) | Sway, hunting, joint and frog excitation |
| [docs/world/terrain.md](docs/world/terrain.md) | Mutable heightfield, strata, earthworks cost |
| [docs/world/construction.md](docs/world/construction.md) | Everything is a thing: materials, machines, labor, time |
| [docs/world/operations.md](docs/world/operations.md) | Real bulk handling, yards and dispatching as mechanics |
| [docs/decisions/](docs/decisions/) | Short ADRs, one per decision that is expensive to reverse |
| [docs/reference.md](docs/reference.md) | Real-world numbers in SI, reading list |
| [docs/glossary.md](docs/glossary.md) | Rail and earthworks terms used across the docs |

## Running

```
cargo test -p hat_units -p hat_sim -p hat_world   # fast: no Bevy compile
cargo test --release -p hat_sim -- --ignored   # 100k-car budget test
cargo run --release -p hat_app          # the game, fullscreen (F11 toggles)
cargo run -p hat_app --features dev     # fast-iteration build with dynamic linking
```

Press F1 in the game for controls. Drag the ground or click the minimap to move around;
the drive keys never move the camera. `HAT_SCREENSHOT=out.png cargo run --release -p hat_app`
saves one frame and exits, for quick visual checks from a script; it also writes `out.cab.png`
from the cab camera, which renders offscreen and stays valid while the display sleeps.
`HAT_SCENARIO=0|1|2|3` picks the starting scenario; 3 is the Branch.

## Conventions

- SI units in every doc and every line of sim code. Display units are a presentation setting.
- `features.md` and `goals.md` are updated in the same change as the code they describe.
- A decision that would be expensive to reverse gets an ADR before code.
- Real-world figures go in `reference.md` with a source, in SI, with the original unit in a note.
