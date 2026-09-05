# Decisions

One file per decision that would be expensive to reverse. Format: context, decision,
consequences, status. Numbered, never renumbered. Superseded ADRs stay and point forward.

| ADR | Title | Status |
|---|---|---|
| [0001](0001-engine-bevy.md) | Engine: Bevy | accepted |
| [0002](0002-si-units.md) | SI units internally, f64 in sim | accepted |
| [0003](0003-train-as-array.md) | Train as contiguous array; sim crate has no Bevy | accepted |
| [0004](0004-box-model-cars.md) | Cars as a box model with loading-dependent CoG | accepted |
| [0005](0005-hybrid-physics.md) | 1D on rail, rigid bodies only on derail | accepted |
| [0006](0006-camera.md) | 3D overhead camera with tilt | proposed |
| [0007](0007-terrain-strata.md) | Heightfield terrain with strata columns | proposed |
| [0008](0008-art-direction.md) | Flat-shaded low-poly, one palette, schematic far zoom | proposed |
