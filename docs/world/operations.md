# Operations

How real railroads handle very long trains and bulk cargo without infinite depots, and how
each practice becomes a mechanic. All SI in the sim; figures here are rounded for reading,
precise values in [reference.md](../reference.md).

## The principle

The facility is small and the train keeps moving through it. A train is on network track
while it works. Terminals are a structure over a through track with a processing rate and a
speed band. Finding room for the train is the player's problem.

## Bulk loading

**Flood loader on a loop track.** Coal, ore, grain. The train enters a balloon loop and
rolls under a silo at 0.15 to 0.25 m/s without stopping. The silo weighs each batch and
drops it as the car passes. Too fast underfills. A full unit train loads in a few hours.
Mechanic: a loader structure with a fill rate in kg/s and a speed band; cars passing under it
gain payload; the loop is just track the player laid with a big enough radius.

**Grain shuttle.** An elevator loads a fixed-size train inside a contract window or pays a
penalty. Mechanic: time-window contracts that reward having the train and the crew there on
time.

**Bottom-dump.** Hoppers open doors over a pit or trestle while rolling slowly or stopped.
Automated openers work in motion. Mechanic: a pit structure with a rate; cars with
bottom doors unload passing over it.

**Rotary dumper.** Cars with rotary couplers, one end marked, roll over while still coupled.
An indexer arm advances the train one car at a time. Around 30 cars an hour for a single
dumper, double for tandem. Cars without rotary couplers must be cut off and dumped singly.
Mechanic: a dumper structure with a per-car cycle time; the train is stepped through it;
car type gates whether it stays coupled.

## Yards

**Hump yard.** A shove engine pushes a cut over a crest at walking pace. A crew pulls pins
at the top and cars roll by gravity down into the bowl, steered by switches, slowed by
retarders so they couple under 1.8 m/s. A trim engine assembles outbound trains at the
far end. Mechanic: falls out of free-rolling 1D cars under grade with retarders as
speed-dependent decelerators on a track segment. The player builds the crest, the ladder,
the bowl and the retarders. Capacity is track length.

**Flat yard.** Kick a car: accelerate, pull the pin, let it roll into a track. Shove to a
joint: push a cut until it couples. Slower, cheaper, needs less ground.

**Receiving, classification, departure.** Long yards are three yards. Inbound trains wait in
receiving until humped, and a full receiving yard blocks the main.

**Doubling.** A train longer than any yard track is split into two cuts on two tracks and
reassembled on departure. Mechanic: every track has a length and the crew must plan cuts.

**Hand brakes and air.** Cars left standing need hand brakes or bottled air, and enough of
them for the grade. Mechanic: rollaways.

## Mainline

**Signals and blocks.** Track is divided into blocks; a signal protects entry. Path
reservation ahead of a train as in Factorio; a long train holds many blocks and blocks
crossings while it does.

**Sidings and meets.** A siding shorter than the train cannot hold it, so the shorter train
takes the siding, or a saw-by is performed with much delay. Long trains on a single-track
line are constrained by the shortest siding, not the longest.

**Distributed power and ECP.** See [train-dynamics.md](../sim/train-dynamics.md). Long
trains without them break knuckles and stall.

**Helpers.** Extra locomotives cut in behind or ahead for a grade and cut off at the summit,
sometimes on the fly. They need crews, fuel, and a place to turn or a return path.

**Wayside detectors.** Hot bearing and dragging equipment detectors flag a car; the train
stops and sets it out at the next siding.

**Crews and hours of service.** A crew works a fixed span then must stop wherever it is.
Meets, slow orders and yard congestion eat that span. Recrews are things with vans.

**Brake tests and recharge.** After making up a train, someone walks it to check brake
application on every car, and the pipe and reservoirs must be charged. Time scales with
length.

## Traffic types

**Unit trains.** One commodity, origin to destination, cycle forever. Where the long-train
gameplay lives. Loop loaders and dumpers at each end.

**Manifest.** Mixed cars with individual waybills, classified at yards. Where the yard
gameplay lives. Cars are routed as individuals across trains.

**Local and work trains.** Serve industries with a few cars; deliver ballast and rail. They
use the same track as everything else.

## Dispatch UI

Timetables with orders per train and interrupts on conditions. A schematic track diagram
with block occupancy for the dispatcher view. A yard consist view listing every car, its
waybill and brake state. All in egui.
