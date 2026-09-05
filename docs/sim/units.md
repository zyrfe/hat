# Units

Everything in the simulation is SI. Conversions happen at two boundaries only: importing
real-world data, and formatting for display. See [ADR-0002](../decisions/0002-si-units.md).

## Quantities

| Quantity | Unit | Notes |
|---|---|---|
| Length, position along edge | m | |
| Mass | kg | |
| Time | s | Sim time. Fixed dt = 1/120 s. |
| Velocity | m/s | |
| Acceleration | m/s² | g = 9.80665 |
| Force | N | Tractive effort, brake force, coupler force |
| Pressure | Pa | Brake pipe, cylinder, reservoir |
| Energy | J | Fuel energy, kinetic energy at impact |
| Power | W | Locomotive rating |
| Angle | rad | |
| Grade | ratio | Rise over run. Display as %. |
| Curvature | 1/m | Display as radius in m or degrees of curve |
| Cant (superelevation) | m | Outer rail raise |
| Density | kg/m³ | Commodities |
| Volume | m³ | Car capacity, cut and fill |
| Mass flow | kg/s | Loaders, dumpers |
| Temperature | K | Only if bearings or thermal loads appear |
| Money | integer minor units | Never a float |

## Numeric types

- f64 in the sim. f32 only at the GPU boundary.
- Positions are edge-local `(edge_id, s)`, never a global coordinate, so precision is
  never an issue on large maps.
- Plain f64 fields with the unit in the doc comment, for example `/// Brake pipe pressure, Pa`.
  No newtypes in the hot loop.
- Newtypes at the boundaries: data files, saves, UI formatting. A `Quantity` enum in
  `hat_units` tags a value with its dimension for formatting.
- Constants in `hat_units::consts`, one definition each. No literals like `0.44704` in
  sim code; use `MPH` from the conversion table.

## Conversion constants

For importing real-world figures. Exact by definition unless noted.

| From | To SI | Constant |
|---|---|---|
| 1 mile | 1609.344 m | `MILE` |
| 1 foot | 0.3048 m | `FOOT` |
| 1 inch | 0.0254 m | `INCH` |
| 1 mph | 0.44704 m/s | `MPH` |
| 1 ft/s | 0.3048 m/s | `FOOT_PER_SECOND` |
| 1 lbf | 4.4482216152605 N | `LBF` |
| 1 lb (mass) | 0.45359237 kg | `LB` |
| 1 short ton | 907.18474 kg | `SHORT_TON` |
| 1 long ton | 1016.0469088 kg | `LONG_TON` |
| 1 psi | 6894.757293168 Pa | `PSI` |
| 1 hp (mechanical) | 745.69987158227 W | `HP` |
| 1 US gallon | 0.003785411784 m³ | `US_GALLON` |
| 1 bushel (US) | 0.03523907 m³ | `BUSHEL` |
| 1 degree of curve (100 ft chord) | radius ≈ 1746.4 / D m | helper `radius_from_degree_of_curve` |

## Display

The player picks a unit system. Formatting lives in `hat_units::fmt` and takes a
`Quantity` plus the setting. US customary shows mph, tons, feet, psi, degrees of curve and
% grade. Metric shows km/h, t, m, kPa, radius and per-mille or % grade. Nothing outside
`hat_units::fmt` knows which system is active.
