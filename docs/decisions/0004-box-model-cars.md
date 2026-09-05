# ADR-0004: Cars as a box model with loading-dependent center of gravity

Status: accepted, 2026-09-04

## Context

Derailment, sway, wreck physics and clearance all depend on car mass distribution. A full
multibody vehicle model is far more than the game needs.

## Decision

Each car is a rectangular body on two trucks with static dimensions, tare mass and tare
CoG height, plus a payload whose mass and CoG follow from commodity density and fill, with
optional lateral and longitudinal offsets. Combined CoG is the mass-weighted average.
Details in sim/car-model.md.

## Consequences

- Derail checks use real quantities: CoG height for overturn, mass for stringlining and
  buff, offsets for asymmetry.
- The same box, mass and CoG feed the rigid-body collider at derail time with no extra data.
- Empties behave differently from loads for free, which is the real hazard mix.
- Tank car slosh and container stacking are refinements on the same model.
