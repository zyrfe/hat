# ADR-0002: SI internally, f64 in the sim

Status: accepted, 2026-09-04

## Context

Rail practice in North America is in miles, mph, tons, psi, lbf and degrees of curve. Mixing
these into physics code produces bugs and unreadable constants.

## Decision

- All simulation state and all data files are SI: m, kg, s, N, Pa, W, rad. Grade is a ratio,
  curvature is 1/m, money is an integer in minor units.
- f64 for sim state. f32 only at the GPU boundary.
- Positions are edge-local `(edge_id, s)`, never a global float.
- Plain f64 fields with the unit in the doc comment inside the hot loop. Newtypes and a
  `Quantity` tag at data-file and UI boundaries only.
- One conversion constants module in `hat_units`. Real-world figures are imported through it
  and recorded in reference.md with their original unit.
- The player picks a display unit system. Formatting is the only code that knows it.

## Consequences

- Reading real-world sources takes a conversion step every time. The constants table makes
  it mechanical.
- No dimensional analysis at compile time in the hot loop. Tests on known scenarios and
  code review carry that load. Revisit `uom` if unit bugs appear.
