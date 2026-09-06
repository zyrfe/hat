# ADR-0009: Shippers pay what the alternative would cost them

Status: accepted, 2026-09-06

## Context

Transport Tycoon pays by distance and speed, so the winning move is to haul across the map.
Real shippers have a budget: coal delivered to a power plant is worth what the nearest mine
plus trucking would cost, and no more. The game's pillars say no cheating and everything
costs what it costs.

## Decision

- A consumer pays per tonne delivered by rail. The rate is the cost of trucking from the
  nearest producer of that commodity, minus any last-mile trucking the shipper still has to
  do because the railhead is not at its door, scaled by how much it trusts the railroad.
- Rate is independent of the route the train took and of the origin's distance.
- Industries keep producing and consuming when unserved; the difference is trucked and
  remembered.
- Money is the score. Costs come from the sim: tractive work becomes fuel, time becomes crew
  hours, the graph becomes track maintenance.

## Consequences

- Short lanes on small maps pay little, because trucks win short hauls. Handling charges at
  each end keep the floor above zero. Maps and prices are tuned around that, not around
  distance bonuses.
- Getting the railhead to the door (a spur, a loop, a pit) is worth money, which is the
  construction game's motive.
- Reliability is paid for through trust, which replaces station ratings.
- Long trains are rewarded only through cost: the same tonne earns the same, so cycle time
  and fuel per tonne decide the margin.
