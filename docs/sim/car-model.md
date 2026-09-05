# Car model

Cars are boxes. A car is a rectangular body on two trucks, with a tare mass and center of
gravity, plus a payload whose mass and center of gravity depend on what is loaded and how
much. See [ADR-0004](../decisions/0004-box-model-cars.md). All SI.

## Static definition (per car type)

| Field | Unit | Notes |
|---|---|---|
| `length_over_couplers` | m | Spacing in the chain when coupled, minus slack |
| `body_length`, `body_width`, `body_height` | m | The box |
| `floor_height` | m | Rail to inside floor |
| `truck_spacing` | m | For joint timing and truck loads |
| `n_axles` | count | 4 typical, 6 or 8 heavy |
| `wheel_diameter` | m | Sound and animation |
| `m_tare` | kg | |
| `h_cg_tare` | m | Tare center of gravity height above rail |
| `m_payload_max` | kg | |
| `v_payload` | m³ | Usable volume |
| `body_type` | enum | Hopper, gondola, tank, flat, well, box, autorack, covered hopper |
| `commodities` | set | What it may carry |
| `rotary_coupler` | bool | Dump without uncoupling |
| `brake` | enum | Conventional, ECP-capable |
| `load_empty_sensor` | bool | |
| `coupler` | struct | Slack, draft gear stiffness and travel, knuckle rating |
| `clearance_height` | m | For tunnels and bridges; double-stack wells are the tall case |

## Dynamic state

| Field | Unit | Notes |
|---|---|---|
| `commodity` | id | |
| `m_payload` | kg | |
| `x_cg_offset`, `y_cg_offset` | m | Longitudinal and lateral shift of payload CoG |
| `damage` | 0..1 | From hard couplings, collisions |

## Derived

Total mass:

    m = m_tare + m_payload

Fill height for bulk in an open box:

    h_fill = m_payload / (rho * A_floor)

Payload CoG height by body type:

- Bulk in hopper or gondola: `floor_height + h_fill / 2`, plus a heap correction above the
  top chord for open-tops loaded past the sides.
- Liquid in a tank: cylinder centroid weighted by fill, plus a slosh oscillator later.
- Containers in a well: stacked box centroids. Double stack is high.
- Flat loads: given per load.

Combined CoG height:

    h_cg = (m_tare * h_cg_tare + m_payload * h_payload) / m

Axle load `m * g / n_axles` in newtons, checked against track class limits.

Roll natural frequency from `h_cg` and suspension stiffness, used by the lateral model.
Higher CoG means slower, larger sway and earlier overturn.

## Where it is used

| Consumer | What it takes |
|---|---|
| [train-dynamics.md](train-dynamics.md) | `m` for all forces, `load_empty_sensor` for brake ratio |
| Derailment | `h_cg`, `y_cg_offset` for overturn; `m` for stringlining and buff |
| [lateral-motion.md](lateral-motion.md) | Roll frequency and amplitude scaling from `h_cg` |
| Rendering | Box dims for mid LOD, mesh selection for near LOD |
| Wreck handoff | Box collider, `m`, CoG offset |
| Terminals | `v_payload`, `commodities`, `rotary_coupler`, `body_type` |
| Track and structures | Axle load vs. track class, `clearance_height` vs. structure gauge |

## Loading state and behavior

- Empties are light and have low CoG, so they hunt earlier and stringline on tight curves
  under draft. Mixed empties and loads in one train is a real hazard and a real puzzle.
- Loaded high-CoG cars overturn earlier and rock harder on jointed rail.
- Shifted loads bias the derail check toward one curve direction and overload one truck.
- Tare and payload both count toward tonnage ratings against ruling grade.
