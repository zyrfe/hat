# Reference

Real-world figures in SI, with the original unit in the note. Approximate unless marked.
Sources are industry rules of thumb, AAR and FRA practice, and published locomotive specs;
firm up with citations as they are used in code.

## Couplers and braking

| Item | SI | Note |
|---|---|---|
| Max coupling speed | 1.8 m/s | 4 mph AAR guidance |
| Free slack per coupler pair | 0.02 to 0.03 m | About an inch |
| Draft gear travel, each | 0.07 to 0.08 m | 2.75 to 3.25 in |
| Draft gear linearized stiffness | 1e7 to 2e7 N/m | Rated force over travel |
| Knuckle ultimate strength | ~1.8 MN | Grade E, ~400,000 lbf |
| Recommended max draft force | ~1.1 MN | ~250,000 lbf |
| Brake pipe pressure, freight | 620 kPa | 90 psi |
| Full service reduction | ~180 kPa | 26 psi |
| Cylinder pressure, full service | ~440 kPa | 64 psi |
| Cylinder pressure, emergency | ~530 kPa | 77 psi |
| Service propagation | ~150 m/s | ~500 ft/s |
| Emergency propagation | ~280 m/s | ~900 ft/s |
| Cylinder to pipe reduction ratio | 2.5 : 1 | |
| DPU radio command delay | ~1 s | |

## Rolling resistance (modified Davis, SI form)

    R = m * (A + B * v) + C * n_axles + D * v²

| Coefficient | Value | Unit | Note |
|---|---|---|---|
| A | 2.9e-3 | N/kg | 0.6 lb/ton |
| B | 1.1e-4 | N/(kg·m/s) | 0.01 lb/ton per mph |
| C | 89 | N per axle | 20 lb per axle |
| D | 1.6 | N·s²/m² | K = 0.07 conventional freight; 3.6 for trailers and containers |

Curve resistance: about `m * 7 / R` N, from 0.8 lb/ton per degree of curve.
Grade resistance: `m * g * grade`.

## Cars and locomotives

| Item | SI | Note |
|---|---|---|
| Gross rail load, modern car | 129.7 t | 286,000 lb |
| Axle load at that gross | 32.4 t | 4 axles |
| Coal hopper, length over couplers | ~16.2 m | 53 ft |
| Coal hopper, tare | ~20 to 25 t | |
| Grain covered hopper, tare | ~27 t | |
| Bulk coal density | ~800 to 900 kg/m³ | |
| Wheat density | ~770 kg/m³ | |
| Iron ore density | ~2,400 kg/m³ | Short ore cars |
| Double-stack clearance height | ~6.15 m | Plate H, 20 ft 2 in |
| Standard gauge | 1.435 m | |
| Max superelevation | ~0.15 m | 6 in |
| Six-axle AC road locomotive, power | ~3.2 MW at rail | ~4,300 hp |
| Starting tractive effort | ~800 kN | ~180,000 lbf |
| Continuous tractive effort | ~700 kN | |
| Mass | ~190 t | |
| Adhesion, AC traction | 0.30 to 0.40 | Lower in rain |

## Track

| Item | SI | Note |
|---|---|---|
| Jointed rail length | 11.9 m | 39 ft |
| Mainline min radius, typical | 350 to 600 m | 5° to 3° |
| Sharp mainline curve | ~175 m | 10° |
| Yard curves | ~150 m | 12° |
| Degree of curve to radius | R = 1746.4 / D | 100 ft chord definition |
| Ruling grade, flat country | 0.005 | 0.5 % |
| Ruling grade, typical main | 0.010 | 1 % |
| Ruling grade, mountain | 0.022 | 2.2 % |
| Turnout speed, #10 | ~6.7 m/s | 15 mph |
| Turnout speed, #20 | ~20 m/s | 45 mph |
| FRA class 1 | 4.5 m/s | 10 mph freight |
| FRA class 2 | 11 m/s | 25 mph |
| FRA class 3 | 18 m/s | 40 mph |
| FRA class 4 | 27 m/s | 60 mph |
| Hunting onset, empties, worn wheels | ~20 m/s | ~45 mph |
| Hump crest release, cars couple under | 1.8 m/s | 4 mph |

## Terminals and operations

| Item | SI | Note |
|---|---|---|
| Flood loader train speed | 0.15 to 0.25 m/s | 0.3 to 0.5 mph |
| Unit train load time, flood loader | 2 to 4 h | ~130 cars |
| Rotary dumper, single | ~30 cars/h | Tandem about double |
| Grain shuttle | 110 cars, load within 15 h | BNSF-style contract |
| Big hump yard | 2,000 to 3,000 cars/day | |
| Crew hours of service | 12 h | US |
| Long train | 3,000 to 4,500 m | 2 to 3 miles, 200+ cars |
| Siding that a 3,600 m train cannot use | 2,400 m | 12,000 vs 8,000 ft |

## Earthworks and construction

| Item | SI | Note |
|---|---|---|
| Rock swell, in place to loose | ~1.5 | |
| Soil shrink, loose to compacted | ~0.9 | |
| Track-laying machine | up to ~1.5 km/day | With full material supply |
| Hand gang track laying | tens of m/day | |
| Drill and blast tunnel advance | ~3 to 6 m/day per face | |
| Tunnel boring machine | ~10 to 30 m/day | Huge mobilization |

## Reading

- Factorio Friday Facts on rail signals, train pathfinding and the rail planner.
- NIMBY Rails devlog: 1D train simulation at real scale, immediate-mode UI.
- Open Rails physics documentation and source: longitudinal dynamics and brake pipe.
- AREMA Manual for Railway Engineering: alignment, earthworks, track structure.
- Hay, Railroad Engineering. Armstrong, The Railroad: What It Is, What It Does.
- AAR Field Manual and Interchange Rules for coupler and brake practice.
- TrainDy and other longitudinal train dynamics papers for coupler and brake models.
- Workers & Resources: Soviet Republic for construction-as-logistics design.
