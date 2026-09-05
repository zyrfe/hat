# Feature tracker

Status: `idea` (not committed), `planned` (in a milestone), `wip`, `done`.
Milestones are defined in [goals.md](goals.md). Update this file in the same change as the code.

## Core simulation

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Sim crate with no Bevy dependency, fixed 1/120 s step | M0 | done | `hat_sim` |
| Deterministic across runs and platforms (f64, stable order, seeded RNG) | M0 | done | trajectory regression tests |
| Train as contiguous car array; couple = splice, uncouple = split | M0 | done | ADR-0003 |
| Edge-local positions `(edge_id, s)` | M1 | done | path of edges per train |
| Parallel per-train stepping, inter-train events deferred | M0 | wip | events deferred; stepping still sequential |
| 100k cars at 120 Hz budget test | M0 | done | 2.6 ms/step release, ignored test |
| Scripted crew playthrough: couple, pull clear, line switches, kick into track 1 | P0 | done | hat_world scenario test |
| Save/replay from deterministic trajectory | later | idea | |

## Couplers and brakes

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Slack dead zone + draft gear spring-damper per coupler pair | M0 | done | |
| Knuckle break above force limit; both halves to emergency | M0 | done | |
| Couple below 1.8 m/s; bump or damage above; closed knuckles just bump | M0 | done | |
| Pin pull requires bunched slack | M0 | done | |
| Brake pipe propagation, service ~150 m/s, emergency ~280 m/s | M0 | done | |
| Control valve: cylinder pressure vs. pipe reduction, direct release | M0 | done | |
| Recharge time scales with train length | M0 | done | |
| Rolling resistance in Davis form | M0 | done | coefficients in reference.md |
| Hand brakes per car | M1 | done | rollaways |
| Dynamic brake and independent brake on locomotives | M1 | wip | independent done, dynamic not |
| Load/empty brake sensor | M2 | planned | |
| Distributed power: remote locos, radio delay, multiple pipe sources | M2 | planned | |
| ECP brakes, equipped cars only | M2 | planned | |
| Brake test time proportional to length | M4 | planned | |

## Car model

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Box model: dims over couplers, tare mass, tare CoG height | M0 | wip | dims and tare done, CoG pending |
| Payload mass, commodity density, fill height, combined CoG | M2 | planned | |
| Lateral and longitudinal CoG offsets for shifted loads | M2 | planned | |
| Axle load and clearance envelope vs. track and structures | M3 | planned | |
| Rotary coupler flag and car-type constraints at terminals | M4 | planned | |
| Liquid slosh oscillator for tank cars | later | idea | |

## Lateral motion

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Yaw, roll, bounce damped oscillators per car | M2 | planned | |
| Joint impulse per truck; jointed vs. welded rail | M2 | planned | 11.9 m joints |
| Frog and points clunk at turnouts | M2 | planned | |
| Roll to cant through curve spirals | M2 | planned | |
| Hunting above critical speed, worse for empties | M2 | planned | |
| Neighbor yaw coupling through coupler | M2 | planned | |
| Roughness by track class, degrades with tonnage | M5 | planned | |
| Rock-and-roll resonance on staggered joints for high-CoG cars | later | idea | |

## Track and graph

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Spline track with snapping, live radius and grade readout | M1 | wip | straight and arc edges, generator only |
| Turnouts with frog number speed limits | M1 | done | |
| Graph edges and nodes, occupancy | M1 | done | |
| Free-rolling cars, grade, retarders | M1 | done | |
| Derailers and bumpers | M1 | wip | bumpers done |
| Curve resistance | M1 | done | |
| Superelevation | M2 | planned | |
| Track classes and speed limits | M3 | planned | |

## Derailment and wrecks

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Criteria: overturn, stringlining, buff, split switch, collision, coupler break, rollaway | M2 | wip | split switch, collision, overspeed, bumper, coupler break done |
| Handoff to rigid bodies with box dims, mass, CoG | M2 | planned | avian3d |
| Freeze to wreck obstacles; clearing as a project | M2 / M5 | planned | |

## Signalling and dispatch

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Blocks, signals, path reservation | M2 | planned | |
| Sidings, meets, saw-by | M2 | planned | |
| Timetables, orders, interrupts | M2 | planned | |
| Crew hours of service, crew changes, dead on the law | M4 | planned | |
| Helpers cut in and cut off on the fly | M4 | planned | |
| Wayside detectors and set-outs | M4 | planned | |

## Yards

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Hump with retarders and bowl tracks | M1 | done | crest cut on the move, weight-and-distance retarders, bowl profile |
| Flat switching: kick, shove to a joint | M1 | done | |
| Receiving, classification, departure model | M4 | done | terminal: receiving siding, bowl, departure track |
| Doubling into short tracks | M4 | planned | |
| Waybills and car routing | M4 | planned | |

## Terminals and bulk handling

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Flood loader on loop track: speed band, fill rate | M4 | wip | loader track fills standing cars; a loop-track flood loader is the next step |
| Rotary dumper with indexer | M4 | planned | |
| Bottom-dump pit or trestle | M4 | done | dumper track unloads standing cars |
| Grain shuttle contracts with time window | M4 | planned | |
| Intermodal pad with cuts | later | idea | |
| Passenger: selective door operation, double stop | later | idea | |

## Terrain and earthworks

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Chunked heightfield with LOD | M3 | planned | |
| Strata columns: excavation cost, slope limits, fill usability, bearing | M3 | planned | |
| Cut/fill estimate for a proposed alignment; mass-haul | M3 | planned | |
| Bridges, tunnels, culverts, retaining walls | M3 | planned | |
| Mines and quarries mutate terrain; spoil piles | M5 | planned | |
| Drainage, washouts, embankment settlement | later | idea | |

## Construction and logistics

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Projects: plan, estimate, schedule, execute | M3 | planned | |
| Materials as cargo: rail, ties, ballast, sub-ballast, turnout panels | M5 | planned | |
| Work trains and track-laying machine with lay rate; railhead reachability | M5 | planned | |
| Earthmoving machines and haul | M5 | planned | |
| Labor pools by skill, hiring, camps | M5 | planned | |
| Fuel consumption and fueling | M5 | planned | |
| Maintenance: tonnage degradation, tamping, rail replacement | M5 | planned | |
| Rolling stock acquisition via interchange | M4 | done | portal on the main-line loop spawns and removes road trains |

## Presentation

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| Debug-line rendering of track and cars | M0 | done | boxes and procedural track meshes |
| LOD: polyline, instanced boxes, low-poly meshes | M2 | planned | |
| 3D overhead camera with tilt and zoom | M1 | done | ADR-0006 |
| Track spline meshes in dirty-chunk rebuild | M1 | wip | built once at startup |
| Audio event pipeline: culling, voice caps, distance merge | M1 | done | bevy_audio, synthesized placeholders |
| Flange squeal, rumble, horn and bell | M2 | planned | |
| Particles: notch-up puff, brake smoke, dust, wreck smoke | later | planned | hanabi |
| egui tool UI: schedules, manifests, interlocking, estimates | M2 | wip | consist, yard, switches, selection, score |
| Display unit setting: SI or US customary | M0 | done | |
| Single palette texture and unified WGSL material | M1 | planned | art.md |
| Procedural car meshes generated from car-model parameters | M1 | planned | |
| Procedural track meshes along splines | M1 | planned | |
| Schematic map layer at far zoom: constant-width lines, true-length train strokes, fixed-pixel icons | M2 | planned | |
| Exaggerated coupler gap rendering, tunable factor | M0 | planned | slack must read |
| Strata bands on terrain cut faces | M3 | planned | |
| Hero meshes from Blender via headless glTF export script | later | planned | |
| Lineup contact-sheet screenshot test across zoom bands | M2 | planned | CI |
| Cab window: live picture-in-picture view following the locomotive, drag levers for throttle, reverser and brakes, gauges, slack strip | P0 | done | TTD vehicle window |
| Speed restriction lookahead: next limit, bumper or split switch ahead with distance | P0 | done | sim `lookahead` |
| Derail and hard-coupling banners with cause | P0 | done | |
| Engineer portrait with reactive mood and one-liners | P0 | done | placeholder for a crew model |
| Crew executor: resumable maneuvers (route, move, couple, pull clear, shove, kick, cut, tie down) | M1 | done | plus hump cutting on the move, bleed, lace, charge; kick later |
| Switch list orders and greedy block planner | M1 | done | per-car destinations editable in the selection panel |
| Auto/manual override with plan panel and radio lines | M1 | done | touching a control pauses the crew |
| Walking time and crew skill | M2 | idea | crew.md |
| Dispatcher: road trains in and out through a portal, yard master assigning hump and sort jobs | M2 | done | traffic.rs; one road train at a time |
| Terminal scenario runs unaided end to end as a test | M2 | done | inbound, hump, sort, load, unload, departure |
| In-app screenshot hook: `HAT_SCREENSHOT=path` saves a frame after a few seconds and exits | M2 | done | works with the display asleep; for scripts and CI |
| Asset manifest with placeholder flag and provenance; lineup sheet highlights placeholders | M1 | planned | art.md |
| Normalization script: fit to sim box, flat facets, palette quantize, LODs | M1 | planned | Blender headless or Rust |
| Placeholder mesh batch generation from a prompt list via hosted 3D generator | M1 | planned | |
| Procedural audio synthesis as placeholder and fallback layer | M1 | done | |
| Livery and reporting-mark generators | M2 | planned | |
| Geometry-node kitbash generators for structures | M3 | planned | |
| Yard and terminal layout generators | M4 | done | ladder yard, terminal with loop, siding, hump and bowl |

## Platform

| Feature | Milestone | Status | Notes |
|---|---|---|---|
| macOS dev build | M0 | done | |
| Windows CI build and smoke test | M0 | planned | |
| Save and load | later | idea | |
| Modding hooks | later | idea | |
