# ADR-0001: Engine is Bevy

Status: accepted, 2026-09-04

## Context

Overhead rail network game with up to 100k simulated cars, tool-heavy UI, macOS development
and a trivial Windows port. Rust preferred over Go, C#, C++.

Considered: Bevy, Godot 4 with a Rust core via GDExtension, Unity DOTS, Fyrox, bare wgpu
plus winit plus egui, Unreal.

## Decision

Bevy.

## Why

- Rust, data-oriented ECS, parallel systems, fixed timestep schedule.
- wgpu gives Metal on macOS and DX12 or Vulkan on Windows with no porting phase.
- The genre's hard parts are custom code in any engine: 1D train sim, track graph,
  reservation, UI. Bevy does not get in the way of any of them.
- MIT/Apache licensed, no runtime fee risk.
- egui via bevy_egui suits the tool-like UI. NIMBY Rails ships on Dear ImGui, a precedent.

## Consequences

- Bevy breaks API roughly three times a year. Mitigated by the sim/presentation wall in
  ADR-0003: only `hat_app` depends on Bevy.
- No editor. Acceptable: the world is player-built and procedural, not authored scenes.
- UI polish costs more than in Godot or Unity. Accepted for a spreadsheet-flavored genre.
- Fallback if Bevy churn becomes intolerable: keep `hat_sim` and `hat_world`, replace
  `hat_app` with bare wgpu, winit and egui.
