# ADR-0007: Heightfield terrain with per-cell strata columns

Status: proposed, 2026-09-04

## Context

Construction cost must depend on what is being dug, and terrain must be mutable by the
player and by mines and quarries.

## Decision

A chunked heightfield of surface heights plus, per cell, a column of layer boundary depths
over a fixed material set and a water table depth. Earthworks estimates intersect a
proposed profile with the columns to price cut and fill per material and to size slopes.
Details in world/terrain.md.

## Consequences

- Estimates are cheap enough to update live while dragging a profile.
- No true 3D volumes: caves, overhangs and folded geology are out. Tunnels are a structure
  along a line with a per-meter cost by the material at that depth.
- Cell size sets the smallest meaningful earthwork. Decided in open-questions.md.
