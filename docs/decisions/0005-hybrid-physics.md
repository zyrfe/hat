# ADR-0005: 1D dynamics on rail, rigid bodies only on derail

Status: accepted, 2026-09-04

## Context

The game needs stretch, bunch, coupling impacts, sway and convincing wrecks for up to 100k
cars. A general rigid-body simulation at that scale is neither affordable nor stable.

## Decision

- On rail: 1D longitudinal dynamics per train (couplers, brakes, resistance, grade) plus
  three cheap oscillators per car for yaw, roll and bounce driven by track features.
- Derailment is a set of criteria checked per car per step.
- On derail: the car hands off to a rigid-body engine (avian3d) with box, mass, CoG and
  velocity. Bodies run for a bounded time, then freeze into wreck obstacles.

## Consequences

- Physics cost is O(cars) and parallel by train.
- Wrecks look right because they are simulated, but only a handful of bodies exist at once.
- Lateral motion never feeds back into longitudinal motion. This is a deliberate limit.
