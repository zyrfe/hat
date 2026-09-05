# Train dynamics

The train is a 1D problem. Each car has a position along the track, a velocity and a mass,
and every force acts along the track. Lateral motion is layered on separately in
[lateral-motion.md](lateral-motion.md) and never feeds back into this model except through
derailment.

All quantities SI. See [units.md](units.md).

## State per car

| Field | Unit | Notes |
|---|---|---|
| `edge`, `s` | id, m | Position of car center along the current edge |
| `v` | m/s | Signed along the train's forward direction |
| `m` | kg | Tare plus payload from [car-model.md](car-model.md) |
| `p_pipe` | Pa | Brake pipe pressure at this car |
| `p_aux`, `p_emg` | Pa | Auxiliary and emergency reservoirs |
| `p_cyl` | Pa | Brake cylinder pressure |
| `valve` | enum | Release, service, emergency, lap |
| `hand_brake` | 0..1 | Applied fraction |
| `coupler_a`, `coupler_b` | struct | Knuckle open/closed, rotary flag, draft gear state |

Locomotives add throttle notch, dynamic brake setting, independent brake, fuel, and a
tractive effort curve.

## Forces

Sum per car, positive forward.

**Tractive effort** (locomotives). Power-limited above a corner speed and adhesion-limited
below it:

    F_te(v) = min(P_rail / max(v, v_min), mu * m_loco * g) * notch_fraction

`P_rail` is power at the rail after transmission losses. `mu` is adhesion, around 0.30 to
0.40 for AC traction, lower in rain.

**Dynamic brake** (locomotives). A speed-dependent retarding curve, head-end only unless
distributed power is present. This is what bunches a train from the front.

**Air brake.** Cylinder pressure to shoe force through a lever ratio, then shoe friction
that falls with speed:

    F_brake = p_cyl * A_cyl * lever_ratio * n_shoes * mu_shoe(v)

Cars with a load/empty sensor scale the lever ratio down when empty so they do not slide.

**Hand brake.** A constant holding force when applied, released only by a crew action.

**Rolling resistance.** Davis form in SI, coefficients in [reference.md](../reference.md):

    R = m * (A + B * v) + C * n_axles + D * v²

**Grade.** `m * g * grade`, with grade the local slope of the vertical profile.

**Curve resistance.** Empirical, about `m * 7 / R` newtons for radius `R` in meters.

**Coupler force.** Between adjacent cars, from the coupler model below.

## Couplers and slack

Each pair of mated couplers has an extension `e` equal to the actual center-to-center
distance minus the nominal coupled length.

- Slack zone: `|e| <= delta_slack`, force zero. Total free slack per pair is a few
  centimeters.
- Draft gear: beyond the slack zone, a spring-damper `F = -k * (e - sign(e) * delta) - c * de/dt`
  with `k` around 1e7 to 2e7 N/m, `c` tuned near critical damping.
- Travel stop: beyond draft gear travel, a much stiffer spring. Impact energy that gets here
  is a hard bang.
- Break: if `|F|` exceeds the knuckle rating (around 1.8 MN ultimate, operate below 1.1 MN),
  the knuckle fails, the train splits, and both halves go to emergency because the pipe parts.

Stretch and bunch are emergent. Locomotive pulls, the first coupler takes up its slack, then
the next, and a gap wave with a chain of impacts travels to the rear. Braking from the head
end runs the rear in. Engineers stretch the train gently before notching up; players who do
not will break knuckles.

Every slack-zone exit and every travel-stop hit emits an `AudioEvent::CouplerImpact` with
relative speed and the two masses.

## Coupling and uncoupling

**Coupling.** When two cars on the same edge close to coupled length:

- Both knuckles closed: bump, no couple. Force exchange through the travel stop.
- At least one open, relative speed below 1.8 m/s: couple. Splice the arrays. Bang scaled
  by speed.
- At least one open, faster: couple with damage recorded on both cars. Above a higher
  threshold, treat as a collision and run the derail check.

- Below 0.08 m/s closing speed the knuckle does not lock. Two cars riding together in
  contact just touch, which is what happens right after a kick while the cut's head still
  coasts. They separate once the run-in from the braking locomotive reaches the head.

**Uncoupling.** A crew action at a coupler. For two seconds after a pin pull the parted ends
refuse to couple, so a shove does not re-lock the knuckle you just opened. Allowed only when the slack at that coupler is
bunched, meaning `e <= 0`. Under tension the pin will not pull. So the move is: stop, shove
back a little, pull the pin, pull ahead. Splitting the array parts the brake pipe, and both
halves apply emergency. That is what holds the cut. A crew can bottle the air or set hand
brakes before parting to avoid the emergency application on the standing portion.

**Angle cocks and hoses.** Modeled as pipe continuity flags at each coupler. A closed angle
cock isolates everything behind it. Forgetting to open one is a classic real error and a
legitimate failure mode.

## Air brake pipe

Standard pipe pressure is 620 kPa. The engineer sets a target at the head end. The pressure
change propagates along the pipe:

- Service reduction: roughly 150 m/s.
- Emergency: roughly 280 m/s, because each car's valve vents the pipe locally and relays.

On a 1600 m train the rear sees a service application around ten seconds after the head
end. Two models, pick in M0:

1. Signal delay: each car sees the head-end command delayed by `distance / c`. Cheap and
   good enough for feel.
2. 1D diffusion: `p_pipe` per car, exchange with neighbors each step, vent at valves. More
   honest about recharge and leakage.

**Control valve.** Cylinder pressure follows pipe reduction at about 2.5 to 1 up to full
service, then emergency fills the cylinder from both reservoirs. Freight valves are direct
release: any pipe rise fully releases the car. Release propagates at recharge speed, which
is slow, so a long train takes minutes to release and much longer to recharge reservoirs
after an emergency application. Recharge time is a real cost of length.

**Distributed power.** Remote locomotives receive the head-end command over radio with a
delay near one second and act as additional pipe sources and additional tractive or dynamic
force points. This splits slack forces and halves brake propagation distance. Required in
practice for the longest trains.

**ECP.** Electronically controlled pneumatic brakes apply every equipped car at once from a
trainline. All cars in the consist must be equipped or the train runs conventional.

## Integration

Per train, per step, single threaded within the train and parallel across trains:

1. Compute forces for each car.
2. Integrate velocity and position. Semi-implicit Euler at 120 Hz, or XPBD with the coupler
   as a compliant two-sided distance constraint at 60 Hz. Decide in M0.
3. Advance `(edge, s)` through nodes, following the turnout setting at each node.
4. Run derail checks and coupling checks, emitting events.

Cross-train events are resolved after the parallel pass, sorted by a stable key.

## Derailment

Checked per car per step. Each returns a severity; the first above threshold derails the car
and everything that then piles into it.

| Check | Condition | Uses |
|---|---|---|
| Overturn | Net lateral acceleration `v²/R - g * cant / gauge` times CoG height exceeds `g * gauge / 2` | CoG height from car model |
| Stringlining | Draft force times `L_car / R` produces lateral force over a fraction of vertical load | Empties on tight curves under high draft |
| Buff jackknife | Same with compressive force | Run-in on curves |
| Split switch | Facing-point move into a turnout set against the route | Trailing through is a run-through: switch damage, no derail |
| Turnout overspeed | Speed above the frog-number limit through the diverging route | |
| Collision | Relative closing speed above the damage threshold | |
| Coupler break | Covered above; the split itself can derail if on a curve at speed | |
| Rollaway | Not a derail, but unattended cars on grade without hand brakes will roll | |
| Derailer | A device set to derail anything that passes | Yard protection |

On derail, cars hand off to the rigid-body sim with box dims, mass, CoG offset and velocity.
Cars still on rail ahead keep going if the coupler broke, or get dragged if it held.
Bodies run for a bounded time, then freeze into wreck obstacles that block the edge.

## Audio events emitted

CouplerImpact, KnuckleBreak, PipePart, AirRelease, EmergencyDump, BrakeSqueal (continuous
while `p_cyl` high and `v` low), FlangeSqueal (continuous by radius and speed), Rumble
(continuous by speed and mass), JointClack and FrogClunk (from lateral model), HornBell.
