# Lateral motion

Sway, hunting and jostle are cheap oscillators layered on the 1D model. They feed rendering,
audio and the derail check, and nothing else.

## Oscillators per car

Three damped second-order oscillators:

| Name | Symbol | What it drives |
|---|---|---|
| Yaw | psi | Rotation about vertical; car angle vs. track tangent |
| Roll | phi | Rotation about the longitudinal axis; sway |
| Bounce | z | Vertical offset |

Each has a natural frequency and damping ratio. Roll frequency comes from the car model's
CoG height and suspension stiffness, so loaded high cars sway slow and wide.

Update per step:

    x'' = -2 * zeta * omega * x' - omega² * (x - x_target) + impulses

Cost is a few floats per car. All 100k cars run it.

## Excitation

Impulses and targets come from the track under each truck, evaluated at the truck's own
`s`, not the car center. Because the two trucks are `truck_spacing` apart and adjacent cars
hit the same feature `length / v` seconds apart, wave effects emerge without coupling.

| Source | Effect | Notes |
|---|---|---|
| Rail joint | Bounce and roll impulse per truck | Jointed rail every 11.9 m; none on welded rail. Staggered joints alternate roll direction. |
| Turnout frog and points | Sharp bounce and yaw impulse | Distinct clunk. Diverging route adds a yaw kick. |
| Curve spiral | Roll target ramps to the cant angle | Enter and exit transitions |
| Bridge deck ends | Bounce impulse | |
| Roughness | Low-amplitude noise from a per-edge seed and quality | Quality degrades with tonnage, restored by maintenance |
| Hunting | Yaw damping goes negative above a critical speed, clamped by a stop | Critical speed lower for empties and worn wheels |
| Coupler impact | Yaw and bounce impulse on both cars | From the 1D model's events |
| Neighbor yaw | Weak spring on yaw difference across the coupler | Cohesion along the train |

Rock-and-roll: on staggered jointed rail at a specific speed the roll excitation matches the
car's roll frequency and amplitude grows until high-CoG cars lift a wheel. This is a later
feature that falls straight out of the model once roll amplitude feeds the overturn check.

## Outputs

**Render.** Yaw rotates the car about its center. Roll rotates it about the rail axis in 3D;
in a straight-down view it shows as body offset against the trucks. Bounce lifts the body.

**Audio.** JointClack per truck per joint with speed and load, FrogClunk per truck per
frog, FlangeSqueal continuous by curve radius and speed, plus a hunting rumble when the yaw
amplitude is high. Two clacks per car per joint gives the rhythm for free.

**Derailment.** Roll amplitude adds to the overturn check. Yaw amplitude from hunting adds
to the wheel-climb risk on empties.

## Not modeled

Lateral position never feeds back into longitudinal speed or coupler forces. Track never
moves under the train. Wheel and rail wear are tonnage counters, not geometry.
