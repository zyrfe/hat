# ADR-0008: Flat-shaded low-poly with one palette, schematic at far zoom

Status: proposed, 2026-09-04

## Context

Assets must read from network scale down to yard scale. Realistic detail is noise from
above and expensive to make; full cartoon loses the industrial weight the game trades on.
The team is small and most geometry is boxes.

## Decision

- Flat-shaded low-poly meshes, no high-frequency textures, one 256-entry palette texture
  shared by every mesh, the UI and the schematic layer.
- Three zoom bands: schematic map far, instanced palette boxes mid, low-poly meshes near.
  Same palette and state colors in all three.
- Parametric assets (cars, track, terrain, piles, simple structures) are generated in code
  from sim data. Hero pieces are modeled in Blender or produced by generators, and every
  external or generated mesh passes one normalization step before glTF export.
- Shape encodes category, color encodes information, detail exists only near.

## Consequences

- One material for everything makes instancing trivial and recoloring a one-file change.
- The car mesh always matches the sim's box because it is generated from it.
- Realistic liveries, weathering and decals are out except as tiny near-zoom accents.
- A lineup contact-sheet test is required so readability regressions are caught.
- Generated and third-party assets are welcome as placeholders and, after normalization and
  review, as shipped assets. Provenance and license are recorded per asset.
