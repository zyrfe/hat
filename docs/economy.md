# Economy: cargo, money and the trucks you compete with

The railroad is a carrier. Industries make and take bulk commodities whether or not a train
ever shows up; what the railroad does not move, trucks move. Money is the score that falls
out of doing the work well. Built 2026-09-06 with the Branch scenario; decisions in
[ADR-0009](decisions/0009-pay-by-alternative.md) and [ADR-0010](decisions/0010-game-clock.md).

## Industries

An industry is a place with a commodity, a rate and a stockpile. A producer's rate fills the
pile; a consumer's rate drains it. The pile has a capacity. Each industry owns one or more
facilities on track edges: the tipple, the pit, the spout. All quantities SI in the sim,
kg and kg/s; the UI shows tonnes and tonnes per day.

| Industry | Commodity | Rate | Facility | Standard practice |
|---|---|---|---|---|
| Coal mine | coal | +40 kg/s (3,500 t/day) | flood loader, one chute, train creeps under | cars fill in motion; too fast underfills |
| Power plant | coal | -40 kg/s | pit, one hopper opening, train creeps over | bottom-dump in motion |
| Country elevator | grain | +3 kg/s (260 t/day) | spout, one spot, cars stand | spot each car, shove to the next |
| Harbour elevator | grain | -10 kg/s | pit, train creeps | unit-train unloading |

**Stock and budgets.** Every sim step the industry publishes what its facilities may move:
a producer's pile, a consumer's remaining room. The sim decrements that budget as it loads
or unloads; the industry reads it back and runs its own clock. Nothing appears from
nowhere: an empty pile loads nothing, a full plant takes nothing.

**Trucks.** A producer whose pile hits capacity trucks the overflow away. A consumer whose
pile runs dry trucks its shortfall in. Both are tallied, and both lower the industry's
trust in the railroad. Trust is a daily moving average of the share moved by rail, 0 to 1.
Trucks are accounting only for now; no truck is drawn.

## What shippers pay

A consumer pays per tonne delivered by rail, at a rate set by what the alternative would
cost it, never by how far the train went. See ADR-0009.

```
direct   = 2 * handling + truck_rate * road_km(nearest producer, consumer)
last     = per leg where the railhead is not at the door: handling + truck_rate * road_km
offer    = max(0, direct - last) * (0.6 + 0.4 * trust)
```

| Constant | Value | Note |
|---|---|---|
| truck_rate | 0.15 $/t·km | bulk trucking, rounded |
| handling | 3 $/t | loading or unloading a truck, each end |
| road_factor | 1.4 | road km over straight-line km |

Consequences that fall out: a long haul from a far producer earns the same as a short one
from the near producer and costs more fuel and crew time; a spur to the door is worth
building because it removes the last mile; a lane too short to beat the trucks pays nothing
and the industry simply trucks. Payment posts on each car's Unloaded event with the car's
origin industry, so a car loaded at a team track pays less than one loaded at the tipple.

## What the railroad pays

| Cost | Rate | Source |
|---|---|---|
| Fuel | 1.20 $/L | rail work integrated per locomotive per step at 30 % efficiency, 36 MJ/L, plus idle 0.004 L/s |
| Crews | 60 $/h per crew on a job | |
| Equipment | 20 $/car·day, 400 $/loco·day | ownership or lease |
| Track | 40 $/km·day | every metre of graph, maintained or not |
| Wrecks | see below | |

Costs accrue every step and post to the ledger hourly. The ledger keeps a balance, totals
per account and the last three hundred entries. Money is an integer in cents (ADR-0002).

## Wrecks and rerailing

A derailed train freezes where it stands and blocks its track. The player calls the wreck
crew for it (R, or the button in the selection panel). The crew takes fifteen minutes to
arrive, twenty-five minutes per derailed car and ninety seconds per other car for
inspection, then puts the train back on the rails where it stands. The switches under the
wheels are lined for them, the way a wreck crew lines them before jacking. Cars that
overlap another standing train cannot be rerailed until that train is moved. It costs a
$3,000 call-out, $2,500 per derailed car and $400 per damage point, posted to Wrecks. A car
takes damage when it leaves the rails: a flat half point plus a fifth of a point per m/s.

The wreck crew is abstract for now. Under M6 it becomes a train with a crane that has to
get there.

## Schedules

A road train carries an order list, TTD style: go to a track, load at a track (optionally
waiting for a full load), unload at a track, repeat. The crew routes over the graph, lines
switches as they come within reach, holds short of any it cannot line yet, and works the
facility the way its mode demands: a creep under a chute at the speed that fills each car
before it passes, or a stop at the spout for every car in turn. Details in
[crew.md](crew.md).

## The Branch

One loop with everything on it, trains circulating one way so there are no meets yet. A
double-ended four-track yard on the south main; a mine siding with a flood loader; three
elevator sidings with spouts; a power plant siding with a pit and a harbour elevator siding
with a pit on the north main; an interchange siding for equipment that arrives later. Two
road trains start with schedules and a switcher waits in the yard. The opening balance is
$250,000.

Sixty times real time is the top clock, and the sim keeps every step at 1/120 s: a unit
train still takes an hour to load, it just takes a minute of your time.

## Not yet

- Trucks on screen. Only the accounting exists.
- Equipment purchase and delivery through the interchange (ADR-0011, proposed).
- Contracts with time windows, seasonal production, harvest surges.
- Two-way traffic, meets, and the dispatching that needs. The loop avoids the problem.
- Rotary dumpers with indexers and rotary-coupler car constraints.
- Track maintenance that responds to tonnage.
