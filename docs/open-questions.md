# Open questions

Unresolved items that need a decision. When one is resolved, move it into an ADR or the
relevant doc and delete it here.

## Design

Resolved into ADRs since the last pass: time scale (ADR-0010), money (ADR-0009), interchange
(ADR-0011, proposed).

- The Engineer as a character. The cab window carries a reactive portrait now. Later: a crew
  model with names, skills, fatigue and hours of service, Kerbal-style reactions, maybe Star
  Control-style dialogue. Where does it live (hat_world), and how much personality before it
  gets in the way of the dispatching game?

- Map scale. Real geography at 1:1 makes a 100 km line feel right for 3 km trains but is
  slow to traverse in play. A compressed scale breaks the "no cheating" pillar if
  terminals shrink but trains do not. Leaning 1:1 with time acceleration.
- Passenger service at all? Not in the first five milestones.
- Two-way traffic. The Branch loop runs one way so no train ever meets another. Meets need
  blocks, a dispatcher and sidings; where does the player sit in that?
- Order semantics. "Load" takes what is there and "load full" waits. Real shuttles want
  "load N cars", "wait for the block", "spot and pull". How rich before it is a program?
- Prices. Trucking at 0.15 $/t·km with 3 $/t handling makes a 4 km lane pay about 5 $/t.
  Fine for a start; a bigger map or a "hills and bad roads" factor is the lever.
- The wreck crew. A timer today. A train with a crane under M6, which means the player can
  be too broke or too blocked to recover a wreck.

## Technical

- Integrator for the chain: semi-implicit Euler at 120 Hz vs. XPBD at 60 Hz. Stiff draft
  gear favors XPBD. Decide in M0 with both prototyped.
- f64 everywhere in the sim vs. f32 with edge-local positions. Starting f64. Revisit only if
  the 100k-car budget test fails.
- Newtype units in the hot loop (`uom` or hand-rolled) vs. plain f64 with conventions.
  Starting plain f64 in the sim, newtypes at data-file and UI boundaries. See ADR-0002.
- Camera: pure orthographic overhead vs. narrow perspective with tilt. See ADR-0006.
- Heightfield resolution and strata column encoding. See terrain.md.
- Wreck physics crate: avian3d vs. rapier3d. Avian is Bevy-native; rapier is more mature.
- Track spline family: clothoid (matches real alignment practice) vs. cubic Bezier
  (easier tooling). Grades want a separate vertical profile either way.
