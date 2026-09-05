# Goals and milestones

Milestones are ordered so each one proves the riskiest remaining idea. A milestone is done
when its "done when" list holds in a build on both macOS and Windows.

## P0 Yard Shift: playable prototype

M0 plus the yard slice of M1 plus a scoring loop, built first. A flat switching yard with
one locomotive, a cut to sort, and an outbound train to build. Full plan, build order and
cut list in [prototype.md](prototype.md).

## M0 Slack: the feel of a long train

Bare Bevy window, one straight track, debug-line rendering. One locomotive and N cars as a
contiguous chain. No art.

Done when:
- 1D chain with coupler slack and draft gear runs at a fixed 120 Hz step and is
  deterministic across runs and platforms.
- Brake pipe propagation delays rear braking. Stretch on start and run-in on braking are
  visible as gaps and audible as coupler events.
- Coupling follows the rules: below 1.8 m/s couples, faster damages, pin pull requires
  bunched slack, cutting the train parts the pipe and both halves go to emergency.
- 100,000 cars across many trains at 120 Hz fits in one core's budget with headroom.
- The sim crate has no Bevy dependency and has trajectory regression tests.
- Everything in SI; a display-unit toggle already exists in the debug overlay.

## M1 Yard: track graph and loose cars

Done when:
- Spline track with turnouts, edge-local positions, and a graph you can lay in-game.
- Free-rolling cars under grade, rolling and curve resistance. Hand brakes hold or do not.
- A hump with retarders sorts a cut into bowl tracks. Kicks in a flat yard work.
- Sound events for couplers, joints, frogs and air, with viewport culling and voice caps.

## M2 Mainline: dispatching and long trains

Done when:
- Signal blocks, path reservation, sidings, meets, timetables and interrupts in an egui UI.
- Distributed power and ECP as train configurations that change slack and braking.
- Derailment criteria hand cars to a rigid-body sim for the wreck, then freeze them as
  obstacles.
- Lateral motion: sway, hunting, joint and frog excitation, visible and audible.
- Car model carries payload and center of gravity, and derail checks use it.

## M3 Dirt: terrain and earthworks

Done when:
- Mutable heightfield with per-cell strata. Cut and fill with per-layer cost and time.
- Grades, curves, bridges, tunnels and culverts chosen by the player with visible cost and
  consequence before committing.
- Ruling grade sets tonnage ratings; helpers or distributed power are the alternatives.

## M4 Bulk: terminals with no cheating

Done when:
- Flood loader on a loop track, rotary dumper with indexer, bottom-dump pit, grain shuttle
  contract with a time window.
- Car types with rotary couplers, capacities and commodity densities that matter.
- Doubling into short yards, siding length checks, crew hours of service.

## M5 Everything is a thing

Done when:
- Track is built by work trains carrying rail, ties and ballast from wherever they come
  from, at a lay rate, occupying track that revenue trains want.
- Crews, machines and fuel are finite, hired, moved and consumed.
- Track degrades with tonnage and is repaired through the same logistics.

## Later

3D art pass, particles, audio polish, save and load, modding, scenarios or campaign,
intermodal and passenger.
