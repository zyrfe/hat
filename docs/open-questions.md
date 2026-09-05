# Open questions

Unresolved items that need a decision. When one is resolved, move it into an ADR or the
relevant doc and delete it here.

## Design

- The Engineer as a character. The cab window carries a reactive portrait now. Later: a crew
  model with names, skills, fatigue and hours of service, Kerbal-style reactions, maybe Star
  Control-style dialogue. Where does it live (hat_world), and how much personality before it
  gets in the way of the dispatching game?

- Map scale. Real geography at 1:1 makes a 100 km line feel right for 3 km trains but is
  slow to traverse in play. A compressed scale breaks the "no cheating" pillar if
  terminals shrink but trains do not. Leaning 1:1 with time acceleration.
- Time scale. Sim seconds vs. game clock. Loading a unit train takes hours in reality.
  Options: real time with fast-forward, or a fixed ratio like 1 sim second = 10 game seconds
  with physics unchanged.
- Economy. Money, or materials and labor only? Contracts as the main goal driver?
- Passenger service at all? Not in the first five milestones.
- Interchange at the map edge as the source of rolling stock and rail steel, or on-map
  industry for everything?

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
