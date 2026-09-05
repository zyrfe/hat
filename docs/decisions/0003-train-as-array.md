# ADR-0003: Train as a contiguous array; sim crate independent of Bevy

Status: accepted, 2026-09-04

## Context

Chain physics needs each car's neighbors every step. ECS entities per car make neighbor
access indirect and coupling and uncoupling awkward. Bevy's API changes often.

## Decision

- `hat_sim` is a pure Rust crate with no Bevy dependency. It exposes `step(&mut World, dt)`
  and emits events. It is deterministic and unit tested with trajectory regressions.
- A `Train` owns `Vec<CarState>` in coupled order. Coupling splices two arrays; uncoupling
  splits one. Trains step in parallel; cross-train events are collected and resolved after.
- `hat_app` mirrors cars into ECS entities for picking, manifests and rendering, only for
  cars near the view, synced each render frame.

## Consequences

- The hot loop is cache-friendly and trivially parallel by train.
- Per-car metadata that the sim does not need lives on the ECS side or in a side table keyed
  by car id.
- Bevy upgrades touch `hat_app` only.
