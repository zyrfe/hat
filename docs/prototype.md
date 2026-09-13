# Playable prototype: Yard Shift

The shortest path to something playable is a flat switching yard. It tests the two riskiest
bets with the least code: that slack, brakes and coupling feel good from overhead, and that
yard work is a fun loop. No terrain, no construction, no signals, no economy, no art.

This is M0 plus the yard slice of M1 plus a scoring loop. It sits before M1 in
[goals.md](goals.md).

## Status

First pass built 2026-09-04. Steps 1 through 8 exist: workspace with tests, the chain
with slack and draft gear, the brake pipe, the Bevy picture, the ladder-yard track graph,
yard rules, synthesized sound, and the egui game layer with scoring. Step 9, the
doubling scenario, is generated and loads but has not been played through. A scripted crew
plays the Yard Shift opening headlessly as an integration test. A TTD-style cab window
with a live chase view, drag levers, gauges, a slack strip and a reactive engineer portrait
was added after the first feel session, along with a speed-restriction lookahead and
derail banners. Known gaps:
no lateral motion, no rigid-body wrecks (derailed cars freeze tilted), no interpolation
between sim steps, stepping is sequential. A GitHub Actions workflow for macOS and Windows
is scaffolded but has not run yet because the project is not in git.

## Beyond P0

The Terminal scenario is the second world: a closed main-line loop with a portal where road
trains appear and leave, a double-ended receiving siding, the flat ladder yard with a loader
and a dumper track, and a hump with a four-track bowl. On Auto the crew and the dispatcher
run the whole flow unaided; a headless test proves an inbound train is humped, sorted,
loaded, unloaded and sent out as a departure. Take the levers whenever you like.

The Branch is the third world and the first with money: one loop with a mine, a power plant,
three elevators and a harbour, two trains on schedules, a switcher in the yard, and a
ledger. Its east end climbs a spiral around a knoll and cuts through a ridge; a river
crosses the west end under two bridges. Shippers pay the trucking alternative; you pay for
fuel, crews, cars, track and wrecks. Derailments are recovered by calling the wreck crew.
See [economy.md](economy.md).

## The loop

A small flat yard: a lead, a ladder of five or six body tracks of different lengths, a
runaround, and a main. One switcher locomotive. An inbound cut of 20 to 40 cars, each with a
destination track by waybill, some with hand brakes set. Sort the cars to their tracks and
assemble an outbound train to a given consist on the departure track, within a time budget,
without damage.

Stretch scenario: a 150-car train arrives on the main and must be doubled into the yard.

**Scoring.** Time. Hard couplings above 1.8 m/s count as damage. Cars on wrong tracks. Cars
left unsecured that roll. Run-throughs and split switches. Knuckle breaks. Cars fouling the
ladder.

**Controls.** Throttle notch and direction. Independent brake. Automatic brake as a pipe
reduction. Select a coupler and pull the pin, allowed only when slack is bunched. Select a
car and set or release its hand brake. Click a switch to throw it. Camera pan and zoom. Time
scale pause, 1x, 4x.

## Build order

Each step leaves a runnable program.

1. **Workspace.** `hat_units`, `hat_sim`, `hat_app`. CI builds and tests on macOS and Windows
   from the first commit.
2. **Chain on one straight edge.** `Train` owns `Vec<CarState>`. Tractive effort, air brake
   force, Davis resistance, coupler slack and draft gear. Semi-implicit Euler at 1/120 s.
   Tests: stretch wave order, run-in wave order, determinism hash across runs.
3. **Brake pipe.** Signal-delay model, control valve, recharge time. Test: rear of a 1600 m
   train applies about ten seconds after the head end.
4. **First picture.** Bevy app, overhead camera with pan and zoom, one box per car from a
   mesh primitive, exaggerated coupler gaps, keyboard throttle and brake. First feel
   checkpoint.
5. **Track graph.** Nodes and edges, straight and circular-arc edges only, turnouts with a
   setting, position advance across nodes, edge-local `s`. Yard layout loaded from RON.
6. **Yard rules.** Couple and uncouple by rule, free-rolling cars, hand brakes, knuckle
   break, split switch flags a derail and freezes the car red. Kick a car.
7. **Sound.** kira, synthesized placeholder clack, bang and hiss. Event pipeline with
   viewport culling and voice caps.
8. **Game.** Scenario file, scoring, egui HUD with gauges, consist panel, waybills and score.
   Time scale.
9. **Doubling.** The 150-car inbound scenario.

## Locked for the prototype

- f64 state, fixed 1/120 s, semi-implicit Euler. XPBD only if draft gear stiffness misbehaves.
- Signal-delay brake pipe.
- Straight and arc edges. Splines are a later edge type behind the same interface.
- Edge-local positions from day one. Sim crate independent of Bevy from day one. Both are
  cheap now and expensive later.
- Boxes and gizmos. SI display only.

## Cut from the prototype

Terrain and grades, construction, signals and timetables, distributed power and ECP, lateral
motion, particles, rigid-body wrecks, palette and material system, hero meshes, display unit
toggle, save and load, economy.

## Feel checkpoints

- After step 4: 100 cars, notch up. Does the stretch wave read at 1x and 4x? Is the gap
  exaggeration right?
- After step 6: kicking a car so it coasts to a joint at a good speed should feel like a
  skill.
- After step 8: a 30-car sort in fifteen game minutes should be a satisfying puzzle. If it is
  not, rethink the loop before adding systems.

## Done when

A stranger can play a yard shift for twenty minutes on macOS or Windows, sort the cut, build
the outbound train, and explain what slack is without being told.

## Estimate

Roughly three to four focused weeks for one person. The sim core is the first week. Step 8
is where most of the unknowns live.
