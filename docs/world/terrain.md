# Terrain

Terrain is mutable. Building a railroad is mostly moving dirt, and what the dirt is made of
decides what it costs. See [ADR-0007](../decisions/0007-terrain-strata.md). All SI.

## Representation

**Heightfield.** A grid of surface heights in meters, chunked for rendering and LOD. Cell
size in the low meters; final value in open-questions.md. The player and the game both
modify it: cuts, fills, pits, spoil piles, quarries.

**Strata columns.** Each cell holds a column of layer boundaries from the surface down, as
depths, plus a water table depth. Layers are drawn from a small material set. The column is
what makes excavation cost depend on where you dig and how deep.

**Water.** Rivers and lakes as surface features with a level. Crossing them needs a bridge
or a culvert. Drainage matters later.

## Materials

Relative cost multipliers, not final numbers. Excavation cost is machine-hours and money
per cubic meter in place.

| Material | Excavation | Cut slope (H:V) | Fill use | Bearing | Notes |
|---|---|---|---|---|---|
| Topsoil | 1.0 | strip | none, stockpile | poor | Must be stripped before fill |
| Clay and silt | 1.5 | 2:1 | poor, shrinks, drains badly | fair | Settles under embankments |
| Sand and gravel | 1.2 | 1.5:1 | excellent, sub-ballast source | good | |
| Weathered rock | 3.0 | 1:1 | good | good | Ripping |
| Hard rock | 8 to 12 | 0.25:1 | excellent, ballast source if suitable | excellent | Drill and blast, slow |
| Peat and muskeg | 4.0 | none | none, must be removed or displaced | none | Bottomless in effect |
| Permafrost | special | | | | Thaw settlement if disturbed. Optional biome. |

Swell and shrink: excavated rock bulks to about 1.5 times its in-place volume; soil compacted
into fill shrinks to about 0.9. Haul is charged on loose volume.

## Earthworks

A proposed alignment is a horizontal spline plus a vertical profile. The estimate:

1. Sample the profile against the heightfield to get cut and fill depth along the line.
2. For each cut, intersect the depth with the strata column to get volume per material and
   the required slope, which widens the cut.
3. For each fill, compute embankment volume at the fill slope, and flag soft ground that
   needs removal or a longer structure.
4. Balance cut against fill along the line with a mass-haul diagram. Surplus is spoil that
   must go somewhere. Deficit is borrow from a pit or a quarry.
5. Cost is excavation per material, haul distance times volume, compaction, and time from
   machine count and rates.

The player sees cost and time before committing and can drag the profile to trade grade for
earthwork. Steeper grade means less dirt and a lower tonnage rating forever. This is the
core engineering loop.

## Structures

| Structure | Cost drivers | When it wins |
|---|---|---|
| Culvert | Diameter, length | Every drainage crossing. Skipping them causes washouts later. |
| Bridge | Span, height, deck type | Deep valleys and rivers where fill would be enormous |
| Trestle (timber) | Height, length | Cheap and fast, low speed, decays, fire risk |
| Tunnel | Length, rock type, bore method | Avoiding a summit grade. Slow to build. |
| Retaining wall | Height, length | Narrow benches where a slope will not fit |
| Snowshed, rockfall shed | Length | Mountain biomes |

Tunnel bore rates depend on rock: drill and blast a few meters a day per face, a boring
machine much faster but with a huge mobilization cost. Two faces from both ends halves the
schedule.

## Consequences that persist

- Ruling grade on a line caps train tonnage per locomotive. Helpers or distributed power
  are the operational fix and cost forever.
- Curve radius caps speed and adds curve resistance and wear.
- Embankments on clay settle for years; speed restrictions until the track is re-tamped.
- Cuts in soil slide in wet weather. Cuts in rock drop rocks. Both are maintenance events.
- Clearance through tunnels and under bridges limits car height on that route.

## Game-driven terrain changes

- Open-pit mines grow as they produce. Spoil piles grow next to them.
- Quarries produce ballast and rock fill and leave a hole.
- Yards and terminals need flat ground, which is usually made, not found.
- Reservoirs, if hydro power appears, flood valleys and reroute lines.

## Generation

Procedural with a geology pass: layer thickness fields from noise, uplift and erosion to
shape relief, rivers by flow accumulation, water table from drainage. Authored maps also
supported. Scale and resolution are in open-questions.md.
