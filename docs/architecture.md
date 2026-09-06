# Architecture

## Engine

Bevy, Rust. See [ADR-0001](decisions/0001-engine-bevy.md). Cross-platform through wgpu:
Metal on macOS, DX12 or Vulkan on Windows. One `cargo build` per platform.

## Crates

A workspace with a hard wall between simulation and presentation so Bevy release churn
touches one crate.

| Crate | Depends on | Holds |
|---|---|---|
| `hat_units` | nothing | SI constants, conversion constants, display formatting |
| `hat_sim` | `hat_units` | Track graph (nodes, straight and arc edges, turnouts), train paths, train dynamics, couplers, brakes, car model, derail checks, contacts. Pure Rust, deterministic, unit tested. No Bevy. |
| `hat_world` | `hat_sim` | Yard and map generators, scenarios, scoring, crews and schedules, the graph router, industries, the ledger, the wreck crew. Later: splines, terrain heightfield and strata, structures, projects |
| `hat_app` | all, Bevy | Rendering, camera, input, UI, audio, particles, wreck physics, save/load |

Third party:

- `bevy`: app, ECS, render, fixed timestep
- `bevy_egui`: tool-style UI for schedules, manifests, interlocking, estimates
- `bevy_kira_audio`: mixing, voice limiting, spatial one-shots
- `bevy_hanabi`: GPU particles
- `avian3d`: rigid bodies for derailed cars only
- `serde` + `ron`: data files and saves

## Simulation shape

**Train as a contiguous array.** A `Train` owns `Vec<CarState>` in coupled order. Chain
physics needs neighbor access, so this beats one ECS entity per car for the hot loop.
Coupling is a splice, uncoupling is a split. Cars also have ECS entities for picking,
manifests and rendering, synced from the array each render frame and only for cars near
the view. See [ADR-0003](decisions/0003-train-as-array.md).

**Fixed step.** `hat_sim::step(&mut World, dt)` runs at dt = 1/120 s inside Bevy's
`FixedUpdate`. Trains step in parallel. Interactions between trains, meaning coupling and
collision, are collected as events during the parallel pass and resolved afterwards in a
deterministic order. The renderer interpolates between the last two sim states.

**Determinism.** f64 state, fixed dt, stable iteration order, no wall clock in the sim,
seeded RNG per subsystem. Needed for save and replay and for regression tests that compare
trajectories across commits and platforms.

**Positions.** A car position is `(edge_id, s)` with `s` in meters from the edge's start
node. The sim never holds a global float coordinate. World-space transforms are computed
for rendering only. This keeps precision high on large maps.

**Hybrid physics.** On rail: 1D longitudinal plus per-car lateral oscillators. Off rail: a
derailed car spawns a rigid body with the box dims, mass and center of gravity from the
car model, seeded with its current velocity vector. Bodies run for a bounded time then
freeze into wreck entities that block track until a clearing project removes them. See
[ADR-0005](decisions/0005-hybrid-physics.md).

## Rendering

LOD by camera height:

- Far: each train is a polyline along the track spline, colored by state.
- Mid: instanced boxes per car.
- Near: instanced low-poly meshes with yaw and roll applied, particles, detailed sound.

Track is spline meshes in chunks rebuilt when dirty. Terrain is a chunked heightfield with
its own LOD. Camera is 3D overhead with tilt and zoom. See
[ADR-0006](decisions/0006-camera.md).

## Audio

Every sim event that should be heard is an `AudioEvent { kind, position, speed, mass, .. }`
emitted by the sim. The app culls by viewport, caps voices per kind, merges distant events
into rumble, and scales pitch and gain by parameters. Listener distance derives from camera
height, so zooming in reveals detail.

## UI

egui for the whole tool surface: timetables, manifests, yard consists, signal panels,
construction estimates. All numbers formatted through `hat_units` so the display unit
setting applies everywhere.

## Platform

Develop on macOS. CI builds and smoke-tests on Windows from M0 so porting never becomes a
phase.
