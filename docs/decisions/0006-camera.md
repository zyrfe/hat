# ADR-0006: 3D overhead camera with tilt

Status: proposed, 2026-09-04

## Context

The pitch is an overhead view. Sway, roll and exhaust read poorly straight down in 2D.
Low-poly instanced 3D is cheap in Bevy and the art budget for boxes on wheels is small.

## Decision

3D scene, overhead camera, zoom from network scale to car scale, tilt allowed within limits.
Orthographic vs. narrow perspective is left open. LOD switches from polylines to instanced
boxes to meshes by camera height.

## Consequences

- Roll and sway are real rotations, not faked offsets.
- Terrain must be a real mesh with LOD from M3.
- Art pass is deferred: boxes and debug lines are legitimate until the sim is proven.
