# Crew: orders, plan, execution

The cab window is manual control. The game is played one level up: you say where cars should
end up and the crew works out the moves. Real yards already split this way, so the design
borrows the roles and the paperwork.

## Three layers

**Switch list (yardmaster, the player).** Declarative intent, edited in the UI:
- Car or block to track: "#12 to Track 3", "these eight to Track 1 in this order".
- Standing orders: keep the lead clear, tie down anything left standing, bleed before switching.
- Priorities and holds: do Track 1 first, leave Track 5 alone.
The scenario's per-car destinations are already a switch list. The UI grows drag-to-track
and block selection on the consist panel.

**Plan (conductor).** Turns the switch list into an ordered list of moves, each a pickup and a
delivery: pull these cars from here, deliver them there, by shove to a joint or by kick.
First version is greedy block switching: from the cars on the engine, take the trailing block
that shares a destination, deliver it, repeat; when a needed car is buried, dig by setting out
the cars in front on a free track and come back. Track length and the no-foul rule on the
lead are hard constraints. Infeasible lists are reported, not silently mangled. Later: proper
multi-track sorting with fewest engine moves, and re-planning when the world diverges.

**Execution (engineer and switchman).** A state machine of maneuvers driven at the fixed
step, each reporting done or failed:

| Maneuver | What it does | Rule it obeys |
|---|---|---|
| Route(track) | Line each switch on the way, waiting for occupied ones | switch occupancy |
| MoveTo(point, speed) | Speed profile from stopping distance and the lookahead | speed limits, bumper |
| CoupleTo(cut) | Approach and couple under 1.8 m/s | coupling speed |
| PullClear(switch) | Move until the whole train is past a switch | no fouling |
| ShoveToJoint(track) | Shove until the leading car couples to what stands there | coupling speed |
| Kick(block) | Shove, pull the pin on bunched slack, brake the rest | pin needs bunched slack |
| Cut(coupler) | Bunch, pull pin, bottle air if the cut is charged | vents unless bottled |
| TieDown(cars) / Bleed / Lace | Hand brakes at one per ten, air handling | securement |

The playthrough test in `hat_world` already implements CoupleTo, PullClear, Route and Kick as
a script. The executor is that code made into a resumable machine.

## Manual override

The cab levers stay live. Touching a lever pauses the executor ("You have the engine") and
the plan waits. A Resume button hands it back. The crew never fights the player.

## Feedback

- Plan panel: the move list with the current one highlighted and progress on each.
- Radio: the engineer's one-liners become crew talk driven by the executor ("three cars,
  that'll do", "that's enough, stop", "pin's pulled").
- Time: crews work in sim time, so 4x and 10x turn a shift into a TTD-style schedule playing
  out, and par time for scoring comes from the AI crew's own run.

## Crew as a mechanic later

Walking time: pins and hand brakes are pulled by a switchman who has to be there. Distance
from the engine costs seconds. Skill: a green engineer couples harder and kicks less
precisely. Hours of service already exist in the operations design. This is where the
Engineer character stops being a portrait and starts being a person you assign. Multiple
crews and engines make this the dispatching game.

## Build order

1. `hat_world::crew`: `Order`, `SwitchList`, `Move`, `Maneuver`, `Executor`. Deterministic,
   tested headlessly. First test: the crew completes the Yard Shift unaided with no hard
   couplings; its time becomes par.
2. App: Auto toggle, plan panel, override and resume, radio lines from crew events.
3. Planner v2: dig moves, sorting with fewest moves, re-planning.
4. Walking time and crew skill.
