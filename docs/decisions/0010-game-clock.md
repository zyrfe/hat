# ADR-0010: One sim clock, compressed up to 60x, physics unchanged

Status: accepted, 2026-09-06

## Context

Loading a unit train takes hours. A shift is eight of them. Playing at 1:1 is honest but
slow; scaling the geometry breaks the no-cheating pillar because terminals shrink and trains
do not; a separate game clock that runs faster than the physics makes the physics a lie.

## Decision

- There is one clock. Sim time is game time. The fixed step stays 1/120 s.
- Time compression runs the sim more steps per frame: 1x, 4x, 10x, 30x and 60x. Nothing in
  the sim knows the compression exists.
- Industry rates, wages and lease costs are per sim second or per sim day (86,400 s).
- Wall-clock budget: a few hundred cars at 60x must fit in a frame on the development
  machine, which sets the per-step cost target at about 10 microseconds for that world.

## Consequences

- A unit train still takes an hour to load, it takes a minute of the player's time at 60x.
- Every per-step cost matters at 60x: the brake-pipe command history had to be capped when
  a dithering controller made a 30-car train cost 150 microseconds a step.
- Sound and the cab view at 30x and 60x will need their own treatment; they are tuned for 1x
  to 10x today.
