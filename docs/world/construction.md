# Construction and logistics

Everything is a thing. Nothing is built by clicking. A project is a plan for moving
materials, machines and people to a place and spending time there, and the railroad itself
is the main way those things move. The reference for this feel is Workers & Resources:
Soviet Republic.

## Project lifecycle

1. **Plan.** The player draws the alignment, structure or facility.
2. **Estimate.** The game returns material quantities, machine-hours, crew-days, track
   occupancy, calendar time and money, broken down. See
   [terrain.md](terrain.md) for earthworks.
3. **Schedule.** The player assigns machines and crews, picks material sources, and sets
   priority against revenue traffic. Work trains need paths and windows like any train.
4. **Execute.** Materials flow, progress advances only when materials, machines and crews
   are on site. The railhead advances at the lay rate. Shortfalls stall the job visibly.
5. **Commission.** Track opens at a low class until tamped and settled, then rises.

## Materials as cargo

| Material | Comes from | Moves as | Notes |
|---|---|---|---|
| Rail | Steel mill or interchange | Rail train, long strings for welded rail | Welded rail needs a welding plant |
| Ties | Tie plant (timber or concrete) | Flatcars or tie train | Timber cheap and light, concrete heavy and durable |
| Ballast | Quarry | Hopper trains | Quality matters; bad ballast means constant tamping |
| Sub-ballast | Gravel pit | Hoppers or trucks | |
| Fasteners, plates, anchors | Interchange | Boxcar | |
| Turnout panels | Fabrication yard | Flatcar | Pre-assembled |
| Signal and interlocking gear | Interchange | Boxcar | |
| Bridge steel, concrete, timber | Fabrication yard, plant, mill | Flatcars, mixers by road | |
| Tunnel lining | Plant | Flatcars | |
| Spoil and fill | The cut you are digging | Dump trucks or air-dump cars | Mass-haul decides distance |
| Diesel fuel | Refinery or interchange | Tank cars, then fuel racks | Locomotives and machines both burn it |
| Sand, water | Local | | Locomotive sand, steam if ever |

## Machines

| Machine | Job | Moves by | Rate driver |
|---|---|---|---|
| Excavator, dozer, grader | Cut, fill, shape | Road or flatcar | Material excavation multiplier |
| Haul truck, air-dump car | Move spoil and fill | Road or rail | Haul distance |
| Drill rig, blasting crew | Rock cut and tunnel | Road or flatcar | Rock type |
| Pile driver, crane | Bridges and trestles | Rail | Span and height |
| Track-laying machine | Ties and rail at the railhead | Rail, needs continuous track behind it | Up to a kilometer or more per day with supply |
| Tamper, regulator, stabilizer | Ballast finishing and maintenance | Rail | Track class achieved |
| Ballast cleaner, rail grinder | Maintenance | Rail | Tonnage since last pass |
| Work locomotives | Pull all of the above | Rail | Steal from the revenue pool or buy |

Rail-borne machines can only reach the end of existing track. Lines grow from the network
outward. Isolated construction needs road access or a temporary connection.

## Labor

Pools by skill with hire time and training: track gangs, bridge and building gangs, signal
maintainers, engineers and conductors, yard crews, mechanical, dispatchers, surveyors.
Remote work needs camps with housing and supply. Contractors exist at a premium.

Train and engine crews have hours of service. A crew that times out stops the train where it
is, on the main, until a relief crew arrives. Long trains and slow meets make this common.
Crew change points and crew vans are things.

## Maintenance

Track accumulates gross tonnage. Class drops with tonnage and weather until a maintenance
project restores it. Lower class means lower speed, more roughness in the lateral model, and
higher derail risk. Maintenance uses the same machines, materials and crews as
construction, and the same track windows. Rail wears on curves. Ties rot. Ballast fouls.

## Rolling stock, fuel and shops

Locomotives and cars are bought and delivered through the interchange or built on-map later.
Locomotives burn fuel by power output and need fueling facilities that are themselves
supplied. Shops do inspections and repairs; cars flagged bad-order are set out and routed to
one. Everything has a position at all times.

## Trade-offs the design must keep alive

| Choice | Cheap and fast | Expensive and slow | The catch |
|---|---|---|---|
| Summit | Steep direct grade | Long detour, deep cuts, or tunnel | Steep grade taxes every train forever |
| River | Timber trestle | Steel bridge or high fill | Trestle is slow, decays, burns |
| Rail | Jointed, small gang | Welded, welding plant | Jointed rides rough and needs joint maintenance |
| Ties | Timber | Concrete | Timber rots; concrete needs a plant and heavy trains to deliver |
| Capacity | Single track with sidings | Double track | Sidings must fit the longest train, or the short one waits |
| Long trains | Helpers | Distributed power | Helpers need crews and a place to turn; DPU needs equipped units |
| Yard | Flat switching | Hump yard | Hump is fast but huge and only pays at volume |
| Terminal | Stub tracks and a loader | Loop track and flood loader | Loop needs land and a radius; stub needs cuts and switching time |

There is no build order that dominates. The estimate screen has to make each of these
legible so the player can be wrong in interesting ways.
