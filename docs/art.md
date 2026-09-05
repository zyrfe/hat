# Art and assets

Readability beats fidelity. Every asset must be identifiable at every zoom band, and the
look lives in the gap between realistic and cartoony: flat-shaded low-poly, a single
limited palette, strong plan-view silhouettes, and detail that only exists up close. See
[ADR-0008](decisions/0008-art-direction.md).

Style references: Islanders and Timberborn for flat-shaded low-poly, Poly Bridge and
Kenney's kits for how little geometry is needed, NIMBY Rails and transit maps for the far
zoom, Rail Route for state coloring.

## Zoom bands

Three bands by camera height, crossfaded with hysteresis so nothing pops. The same palette
in every band, so a coal train is the same near-black everywhere.

| Band | What renders | Style |
|---|---|---|
| Map (far) | Track as constant-pixel-width lines. Trains as true-length colored strokes along the track. Signals, turnouts, terminals as fixed-pixel icons. Labels. Terrain as desaturated hillshade tint. | Schematic. This is a map, not a scene. |
| Network (mid) | 3D terrain and track meshes. Trains as instanced palette boxes with dark end caps so gaps read. Structures as blocks. Edge darkening on. No cast shadows. | Blocky and clean. |
| Yard (near) | Full low-poly meshes with yaw, roll and bounce applied. Couplers, trucks, payload heaps, particles, tiny decals. Soft shadows. | Low-poly, flat shaded. |

## Readability rules

- **Shape is category.** Each car type has a distinct plan-view silhouette and roof: open
  hopper (open top, payload visible, ribbed sides), covered hopper (trough hatches), tank
  (the only cylinder), boxcar (flat roof, running board line), gondola (open, dark inside),
  flat and well (thin, load on top), autorack (tall, slotted). Locomotives read by long
  hood, cab and radiator block.
- **Color is information.** Commodity color in open loads. Livery or route color as a roof
  band. State overlays identical in every band: selected, brakes applied, emergency,
  bad-order, derailed.
- **Gaps always read.** Dark end caps at mid zoom, real couplers at near, stroke breaks at
  far. Coupler gaps are drawn exaggerated by a tunable factor, default 3, so slack action is
  visible at all.
- **Tilt by default.** A straight-down camera hides every side face. Default tilt around
  25 degrees, clamped, so boxes read as boxes. A soft contact shadow under each car seats it
  on the track.
- **Terrain recedes.** Surface is desaturated so rolling stock pops. Cut faces show strata
  bands in the terrain material palette, which makes the earthworks mechanic legible for
  free.
- **Screen-space minimums.** Track never thinner than two pixels. Icons and labels fixed
  pixel size. Trains never thinner than the track.
- **No high-frequency texture anywhere.** Large-scale color variation only. Detail is
  geometry at near zoom, never a texture at far zoom.

## Palette

One 256-entry palette texture. Every mesh either UV-maps into it or carries vertex colors
sampled from it. One material for everything, so instancing is free and the whole game can
be recolored from one file.

Sub-palettes: commodities, liveries and routes, state overlays, terrain materials, structure
families, UI accents. The egui theme and the schematic layer draw from the same palette.
Figma holds the palette as design tokens; the PNG is exported from it.

## Making the assets

**Procedural, in code.** Anything parametric is generated from sim data so it always
matches the box the sim thinks it has:
- Cars from car-model parameters: a 2D profile per body type extruded to length, with LODs
  by dropping profile detail.
- Track along splines: rails as an extruded profile, ties instanced, ballast as a shaped
  strip, turnouts from templates.
- Terrain, spoil and ballast piles, payload heaps in open cars, cut faces with strata.
- Simple structures kitbashed from primitives: silos, bins, conveyors, retarders, bumpers.

**Blender, for hero pieces.** Locomotives, the rotary dumper and indexer, the track-laying
machine, excavators, cranes, bridge details, signals. Model in meters, origin at rail-top
center, flat shading, palette UVs, LODs as named child nodes. Export glTF binary from a
batch script so the whole library rebuilds in one command. Blender's Python does
parametric families, for example locomotive variants by length and hood style.

**Voxels, optional.** MagicaVoxel or Goxel sit naturally in the abstraction band and export
to mesh, but fight curves. Not the default. Acceptable for structures.

**Third-party packs.** Kenney and Quaternius (CC0), Poly Pizza as an index, Synty (paid,
commercial). Placeholders first; ship quality for vegetation and generic buildings once
remapped to the palette. Every third-party asset is listed with its license in
`assets/LICENSES.md`.

**2D and icons.** Illustrator or Figma for the schematic symbol set and UI icons. Export
SVG, then either rasterize per DPI or triangulate to meshes for scale-invariant drawing.

**Shading does most of the work.** A custom Bevy material in WGSL: palette lookup, flat
normals, a two- or three-step ramp, hemisphere light, cheap edge darkening. An optional
screen-space outline pass at mid zoom. Desaturation and height fog rise with camera height.

**Generators, for placeholders and more.** Anything that gets a stand-in from a generator
today is something the sim can be tested against today. Output never goes in raw. See the
Generators section below.

## Generators

Anything that gets a stand-in today lets the sim be tested against it today. Generators are
a first-class source, procedural and AI alike. Two rules: generated output never ships raw,
it passes through normalization first; and it is flagged as a placeholder in the manifest
until someone signs off on it.

| Need | Generator | Notes |
|---|---|---|
| Blocking meshes for structures, machines, locomotives | Text or image to 3D. Hosted services with an API for batch runs; the open models mostly want a CUDA GPU, so not this Mac. | Output is high-poly and textured. Normalization turns it into flat-shaded palette geometry. Good enough to test scale and silhouette. |
| Concept and silhouette sheets | Image generation, local on Apple silicon or hosted. | Pick silhouettes, then model or generate from the chosen sheet. Check the model license; some open weights are non-commercial. |
| Icons and schematic symbols | Vector generation in Figma or Illustrator, then hand cleanup. | Must snap to the icon grid and the palette. |
| Kitbash buildings and industrial clutter | Blender geometry nodes: parametric silo, bin, conveyor, shed, pipe rack from footprint and seed. | Deterministic. Not AI, still a generator. |
| Yard and terminal layouts for testing | Ladder yards from track count and length, loop terminals from radius, hump bowls from track count. | Later doubles as player-facing templates. |
| Trees and ground clutter | L-system or scattered primitives. | Far zoom needs none. Near zoom needs little. |
| Liveries and reporting marks | Palette stripe generator, marks generator from railroad names. | Distinct trains without art time. |
| Sound placeholders | Synthesis: filtered noise bursts for clacks and impacts, resonant sweeps for squeals, pulse trains for diesel notches. Hosted SFX generation where texture is needed. | Synthesis stays in as the fallback layer under samples. |
| Terrain and geology | Noise, uplift and erosion, flow accumulation. | Already the plan. |

### Normalization

Every generated or third-party mesh passes through one script, Blender headless or Rust,
before it becomes an asset:

1. Scale and orient to meters, origin at rail-top center or footprint center.
2. Fit to the sim box if the asset has one. Refuse if the silhouette does not fit.
3. Remesh or planar-decimate to the triangle budget for each zoom band, flat facets.
4. Quantize color: sample albedo per face, snap to the nearest palette entry, drop the
   texture.
5. Emit LODs as named nodes and write the glTF.
6. Record provenance: generator, model, prompt or seed, license, date, `placeholder: true`.

The lineup sheet highlights placeholders so nothing generated slips out unreviewed.

### Rules

- Consistency over variety. One normalization pass, one palette, one budget per band.
- Generated meshes are never the source of truth for dimensions. The car model is.
- License is checked before use, and recorded next to the asset and in `assets/LICENSES.md`.
- A placeholder is replaced by editing the manifest entry, never by renaming files, so
  references in code do not change.

## Audio assets

Few base samples with heavy runtime variation. Coupler impact in several takes, joint clack,
frog clunk, air release, emergency dump, brake squeal loop, flange squeal loop, diesel
notch loops per notch, horn, bell, retarder screech, dump and flood-loader roar.

Pitch and gain follow speed and mass through the audio event pipeline, so a handful of
samples covers a hundred thousand cars. Procedural synthesis for squeals and rumble if
samples repeat audibly. Sources: Freesound with license check, Sonniss GDC bundles, own
recordings. Same license file as models.

## Pipeline

```
assets/
  palette.png              exported from Figma tokens
  manifest.ron             every asset: id, path, source, placeholder flag, provenance
  src/*.blend              hero sources
  src/export.py            batch glTF export, run headless
  src/normalize.py         normalization pass for generated and third-party meshes
  src/gen_placeholders.py  batch requests to a hosted 3D generator from a prompt list
  models/*.glb             exported, committed
  audio/                   processed samples and synthesis presets
  LICENSES.md              every third-party or generated file and its license
```


- Scale: 1 unit = 1 m. glTF handles axis conversion.
- Headless export: `Blender -b file.blend -P assets/src/export.py`.
- Mesh optimization with meshoptimizer when sizes matter.
- **Lineup scene.** A test scene renders every car type and structure at all three zoom
  bands into a contact sheet. CI screenshots and diffs it. Reviewing the sheet is the
  readability ritual before any asset ships.

## Tools on this machine

Installed: Blender 5.1 (bundled Python 3.13 with NumPy, enough for export and generator
scripts), Adobe Illustrator, Figma, ffmpeg, Rust toolchain, Node. Godot is also
present but not used.

To add when needed: ImageMagick (palette and sprite batch work), meshoptimizer's gltfpack
(mesh compression and LOD), Aseprite (palette design), Audacity or ocenaudio (sample
editing), MagicaVoxel (optional), Pillow and NumPy via uv for asset scripts.
