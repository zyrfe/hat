# ADR-0011: The interchange is where things come from

Status: proposed, 2026-09-06

## Context

Rolling stock, rail, ties and locomotives have to come from somewhere. An on-map industry
for everything is a lot of map; a depot that spawns cars for money is the cheat the vision
forbids. Real railroads interchange with a neighbour at a junction.

## Decision

- Each map has one or more interchange tracks at its edge. Purchased equipment arrives there
  after a delivery delay, as a cut set out by the foreign road, and has to be moved by the
  player's own crews. Nothing arrives if the interchange track is occupied.
- Foreign cars that arrive with traffic go home the same way. Cargo bound off the map is
  delivered by setting it out on the interchange.
- The Terminal's portal, where trains appear and vanish, is the same idea before the rule
  existed and will be replaced by an interchange.

## Consequences

- Buying equipment is a logistics act, not a menu.
- Off-map demand becomes a market: the neighbour pays a rate for what it takes away.
- The dispatcher's spawn code becomes an interchange model.
