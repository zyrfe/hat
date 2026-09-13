//! Terrain stand-in: a heightfield shaped by the track and by the world's landforms.
//! Rolling country from noise, hills and river valleys from the landforms, a flat
//! formation under every track and industry pad, embankments and cuts at 1.5:1 between.
//! Where a fill would be too high the track gets a bridge instead, where a cut would be
//! too deep it gets a tunnel, and where one track crosses over another the upper one is
//! bridged. The mutable strata heightfield of ADR-0007 will replace where the heights come
//! from; the meshing, colouring, spans and picking here stay.

use std::collections::HashMap;
use std::f32::consts::TAU;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use hat_sim::*;
use hat_world::Landform;

use crate::camera::{pose_to_world, sim_to_world};
use crate::sim::Sim;

/// Rail top to formation, the ground the ballast sits on, m.
pub const FORMATION_DROP: f32 = 0.75;
/// Natural ground averages this far below rail top, so level track sits on a low fill.
pub const GROUND_MEAN: f32 = -1.0;
/// Half width of the flat formation either side of a track centreline, m. Yard tracks are
/// 4.5 m apart, so this makes a yard one continuous pad.
const FORMATION_HALF_W: f32 = 3.0;
/// Horizontal run per metre of rise on embankment and cut slopes.
const SLOPE_RUN: f32 = 1.5;
/// Reach of a track's earthworks, m. Beyond this the ground is whatever the noise says.
const REACH: f32 = 22.0;
/// A fill higher than this is a bridge; a cut deeper than this is a tunnel.
const MAX_FILL: f32 = 9.0;
/// Track this close to a river's centreline, as a fraction of the bank half width, is
/// bridged whatever the fill, so the valley stays open under the deck.
const RIVER_BRIDGE: f32 = 0.7;
const MAX_CUT: f32 = 12.0;
/// A track left hanging this far above ground it did not win is bridged.
const HANGING: f32 = 1.5;
const CELL: f32 = 4.0;
const MARGIN: f32 = 240.0;
/// The coarse ring around the fine field, and its cell. Relief fades to level over the
/// outer band so it meets the flat horizon plane.
const RING: f32 = 2400.0;
const RING_CELL: f32 = 32.0;
const EDGE_FADE: f32 = 600.0;
/// Cells per chunk side. Chunks share their border vertices.
const CHUNK: usize = 64;
/// Industry pad half extents, m. Buildings are placed within this.
const PAD: Vec2 = Vec2::new(80.0, 60.0);
/// Track sample spacing along edges, m.
const STEP: f64 = 2.0;

/// A stretch of one edge that is bridged or tunnelled, in metres along the edge.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    pub edge: EdgeId,
    pub s0: f64,
    pub s1: f64,
}

/// A water surface: a level ribbon along a polyline, world x and z.
#[derive(Clone, Debug)]
pub struct Water {
    pub points: Vec<Vec2>,
    pub half_w: f32,
    pub level: f32,
}

#[derive(Resource)]
pub struct Heightfield {
    /// World x and z of vertex (0, 0).
    pub origin: Vec2,
    pub cell: f32,
    pub nx: usize,
    pub nz: usize,
    pub h: Vec<f32>,
    /// Per vertex: 0 open ground, 1 earthworks slope, 2 formation.
    kind: Vec<u8>,
    pub bridges: Vec<Span>,
    pub tunnels: Vec<Span>,
    pub rivers: Vec<Water>,
    site: Site,
}

impl Heightfield {
    fn at(&self, i: usize, j: usize) -> f32 {
        self.h[j * self.nx + i]
    }

    fn max(&self) -> Vec2 {
        self.origin + Vec2::new((self.nx - 1) as f32, (self.nz - 1) as f32) * self.cell
    }

    /// Outer bounds of the coarse ring, world x and z. Level ground continues beyond.
    pub fn outer_bounds(&self) -> (Vec2, Vec2) {
        (self.site.outer_min, self.site.outer_max)
    }

    /// Surface height at a world x, z: bilinear inside the field, the natural ground beyond.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let fx = (x - self.origin.x) / self.cell;
        let fz = (z - self.origin.y) / self.cell;
        if fx < 0.0 || fz < 0.0 || fx > (self.nx - 1) as f32 || fz > (self.nz - 1) as f32 {
            return self.site.ground(x, z);
        }
        let (i, j) = ((fx as usize).min(self.nx - 2), (fz as usize).min(self.nz - 2));
        let (tx, tz) = (fx - i as f32, fz - j as f32);
        let top = self.at(i, j) * (1.0 - tx) + self.at(i + 1, j) * tx;
        let bot = self.at(i, j + 1) * (1.0 - tx) + self.at(i + 1, j + 1) * tx;
        top * (1.0 - tz) + bot * tz
    }

    /// Surface normal from central differences.
    fn normal(&self, i: usize, j: usize) -> Vec3 {
        let (i0, i1) = (i.saturating_sub(1), (i + 1).min(self.nx - 1));
        let (j0, j1) = (j.saturating_sub(1), (j + 1).min(self.nz - 1));
        let dx = (self.at(i1, j) - self.at(i0, j)) / ((i1 - i0) as f32 * self.cell);
        let dz = (self.at(i, j1) - self.at(i, j0)) / ((j1 - j0) as f32 * self.cell);
        Vec3::new(-dx, 1.0, -dz).normalize()
    }

    /// Where a ray meets the surface, by iterating the plane hit at the surface height.
    /// Converges in a few steps on ground this gentle.
    pub fn raycast(&self, origin: Vec3, dir: Vec3) -> Option<Vec3> {
        if dir.y.abs() < 1e-6 {
            return None;
        }
        let mut y = 0.0;
        let mut hit = origin;
        for _ in 0..8 {
            let t = (y - origin.y) / dir.y;
            if t < 0.0 {
                return None;
            }
            hit = origin + dir * t;
            let next = self.height_at(hit.x, hit.z);
            if (next - y).abs() < 0.02 {
                break;
            }
            y = next;
        }
        Some(hit)
    }
}

/// A landform in world x, z.
enum Form {
    Hill { center: Vec2, height: f32, radius: Vec2 },
    River { points: Vec<Vec2>, half_w: f32, bed: f32 },
}

/// The natural ground, before any track touches it.
struct Site {
    /// Outer bounds of the coarse ring; relief fades to level inside them.
    outer_min: Vec2,
    outer_max: Vec2,
    forms: Vec<Form>,
}

impl Site {
    /// Distance to the nearest river centreline as a fraction of its bank half width.
    fn river_frac(&self, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z);
        self.forms
            .iter()
            .filter_map(|f| match f {
                Form::River { points, half_w, .. } => Some(polyline_distance(points, p) / half_w),
                _ => None,
            })
            .fold(f32::MAX, f32::min)
    }

    fn ground(&self, x: f32, z: f32) -> f32 {
        let edge = (x - self.outer_min.x).min(self.outer_max.x - x).min(z - self.outer_min.y).min(self.outer_max.y - z);
        let fade = smoothstep((edge / EDGE_FADE).clamp(0.0, 1.0));
        let mut h = GROUND_MEAN + relief(x, z) * fade;
        let p = Vec2::new(x, z);
        for f in &self.forms {
            match f {
                Form::Hill { center, height, radius } => {
                    let d = (p - *center) / *radius;
                    h += height * (-d.length_squared()).exp() * fade;
                }
                Form::River { points, half_w, bed } => {
                    let d = polyline_distance(points, p);
                    if d < *half_w {
                        h = bed + (h - bed) * smoothstep(d / half_w);
                    }
                }
            }
        }
        h
    }
}

fn polyline_distance(points: &[Vec2], p: Vec2) -> f32 {
    points
        .windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f32::MAX, f32::min)
}

struct Sample {
    x: f32,
    z: f32,
    /// Formation height.
    zf: f32,
    edge: EdgeId,
    s: f64,
}

/// Buckets of sample indices, `REACH` metres square.
struct Grid {
    cells: HashMap<(i32, i32), Vec<u32>>,
}

impl Grid {
    fn key(x: f32, z: f32) -> (i32, i32) {
        ((x / REACH).floor() as i32, (z / REACH).floor() as i32)
    }
    fn insert(&mut self, i: u32, s: &Sample) {
        self.cells.entry(Self::key(s.x, s.z)).or_default().push(i);
    }
    /// Every sample within `r` of the point, with its distance.
    fn near<'a>(&'a self, samples: &'a [Sample], x: f32, z: f32, r: f32) -> impl Iterator<Item = (f32, &'a Sample)> + 'a {
        let (kx, kz) = Self::key(x, z);
        let span = (r / REACH).ceil() as i32;
        (-span..=span).flat_map(move |dz| (-span..=span).map(move |dx| (kx + dx, kz + dz))).filter_map(move |k| self.cells.get(&k)).flatten().filter_map(move |&i| {
            let s = &samples[i as usize];
            let d = ((s.x - x).powi(2) + (s.z - z).powi(2)).sqrt();
            (d <= r).then_some((d, s))
        })
    }
}

/// Build the field for the current world.
pub fn build(sim: &Sim) -> Heightfield {
    let g = &sim.world.graph;
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    let mut grow = |w: Vec3| {
        min = min.min(Vec2::new(w.x, w.z));
        max = max.max(Vec2::new(w.x, w.z));
    };
    for n in &g.nodes {
        grow(sim_to_world(n.pos));
    }
    if let Some(e) = &sim.economy {
        for ind in &e.industries {
            grow(sim_to_world(ind.pos));
        }
    }
    // The fine field spans whole ring cells, so the ring meets it on shared grid lines.
    let min = min - MARGIN;
    let per_ring = (RING_CELL / CELL) as usize;
    let cells_x = (((max.x + MARGIN - min.x) / RING_CELL).ceil() as usize) * per_ring;
    let cells_z = (((max.y + MARGIN - min.y) / RING_CELL).ceil() as usize) * per_ring;
    let (nx, nz) = (cells_x + 1, cells_z + 1);
    let max = min + Vec2::new(cells_x as f32, cells_z as f32) * CELL;

    let to_world = |p: glam::DVec2| {
        let w = sim_to_world(p);
        Vec2::new(w.x, w.z)
    };
    let forms = sim
        .yard
        .landforms
        .iter()
        .map(|l| match l {
            Landform::Hill { center, height, radius } => Form::Hill { center: to_world(*center), height: *height as f32, radius: radius.as_vec2() },
            Landform::River { points, width, bed, .. } => Form::River { points: points.iter().map(|&p| to_world(p)).collect(), half_w: *width as f32 * 0.5, bed: *bed as f32 },
        })
        .collect();
    let rivers = sim
        .yard
        .landforms
        .iter()
        .filter_map(|l| match l {
            Landform::River { points, width, bed, depth } => Some(Water { points: points.iter().map(|&p| to_world(p)).collect(), half_w: *width as f32 * 0.24, level: (*bed + *depth) as f32 }),
            _ => None,
        })
        .collect();
    let site = Site { outer_min: min - RING, outer_max: max + RING, forms };

    // Track samples at formation height. A fill too high is a bridge, a cut too deep a
    // tunnel; neither shapes the ground.
    let mut samples = Vec::new();
    let mut flags: Vec<u8> = Vec::new();
    let mut grid = Grid { cells: HashMap::new() };
    for (ei, e) in g.edges.iter().enumerate() {
        let n = ((e.length / STEP).ceil() as usize).max(1);
        for i in 0..=n {
            let s = e.length * i as f64 / n as f64;
            let p = g.pose_on_edge(ei as EdgeId, s);
            let w = pose_to_world(&p);
            let sample = Sample { x: w.x, z: w.z, zf: w.y - FORMATION_DROP, edge: ei as EdgeId, s };
            let base = site.ground(w.x, w.z);
            let flag = if sample.zf - base > MAX_FILL || site.river_frac(w.x, w.z) < RIVER_BRIDGE {
                1
            } else if base - sample.zf > MAX_CUT {
                2
            } else {
                0
            };
            if flag == 0 {
                grid.insert(samples.len() as u32, &sample);
            }
            samples.push(sample);
            flags.push(flag);
        }
    }
    // Industry pads, level with the nearest track.
    if let Some(e) = &sim.economy {
        let mut pads = Vec::new();
        for ind in &e.industries {
            let c = sim_to_world(ind.pos);
            let zf = grid.near(&samples, c.x, c.z, 260.0).min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, s)| s.zf).unwrap_or(GROUND_MEAN);
            let (kx, kz) = ((PAD.x / CELL) as i32, (PAD.y / CELL) as i32);
            for j in -kz..=kz {
                for i in -kx..=kx {
                    pads.push(Sample { x: c.x + i as f32 * CELL, z: c.z + j as f32 * CELL, zf, edge: EdgeId::MAX, s: 0.0 });
                }
            }
        }
        for s in pads {
            grid.insert(samples.len() as u32, &s);
            samples.push(s);
            flags.push(0);
        }
    }

    let mut h = vec![0.0; nx * nz];
    let mut kind = vec![0u8; nx * nz];
    for j in 0..nz {
        for i in 0..nx {
            let (x, z) = (min.x + i as f32 * CELL, min.y + j as f32 * CELL);
            let base = site.ground(x, z);
            // Each nearby track sample bounds the ground to a slope envelope around its
            // formation. Where the envelopes agree the ground is clamped into all of them;
            // where two tracks at different heights conflict, the nearest wins.
            let (mut lo, mut hi) = (f32::MIN, f32::MAX);
            let mut nearest: Option<(f32, f32)> = None;
            for (d, s) in grid.near(&samples, x, z, REACH) {
                let slack = (d - FORMATION_HALF_W).max(0.0) / SLOPE_RUN;
                lo = lo.max(s.zf - slack);
                hi = hi.min(s.zf + slack);
                if nearest.map_or(true, |(nd, _)| d < nd) {
                    nearest = Some((d, s.zf));
                }
            }
            let k = j * nx + i;
            h[k] = match nearest {
                None => base,
                Some((d, zf)) => {
                    let v = if lo <= hi {
                        base.clamp(lo, hi)
                    } else {
                        let slack = (d - FORMATION_HALF_W).max(0.0) / SLOPE_RUN;
                        base.clamp(zf - slack, zf + slack)
                    };
                    kind[k] = if d <= FORMATION_HALF_W {
                        2
                    } else if (v - base).abs() > 1e-4 {
                        1
                    } else {
                        0
                    };
                    v
                }
            };
        }
    }
    let mut hf = Heightfield { origin: min, cell: CELL, nx, nz, h, kind, bridges: Vec::new(), tunnels: Vec::new(), rivers, site };

    // A track that lost its ground to a lower one crossing it is bridged too.
    for (k, s) in samples.iter().enumerate() {
        if flags[k] == 0 && s.edge != EdgeId::MAX && s.zf - hf.height_at(s.x, s.z) > HANGING {
            flags[k] = 1;
        }
    }
    // Runs of flagged samples along an edge become spans.
    let half = STEP * 0.5;
    let mut run: Option<(u8, Span)> = None;
    let close = |run: &mut Option<(u8, Span)>, hf: &mut Heightfield| {
        if let Some((flag, sp)) = run.take() {
            if sp.s1 - sp.s0 >= 4.0 {
                if flag == 1 {
                    hf.bridges.push(sp)
                } else {
                    hf.tunnels.push(sp)
                }
            }
        }
    };
    for (k, s) in samples.iter().enumerate() {
        let flagged = flags[k] != 0 && s.edge != EdgeId::MAX;
        let len = g.edges.get(s.edge as usize).map(|e| e.length).unwrap_or(0.0);
        let same = run.as_ref().map_or(false, |(f, sp)| *f == flags[k] && sp.edge == s.edge);
        if flagged && same {
            run.as_mut().unwrap().1.s1 = (s.s + half).min(len);
        } else {
            close(&mut run, &mut hf);
            if flagged {
                run = Some((flags[k], Span { edge: s.edge, s0: (s.s - half).max(0.0), s1: (s.s + half).min(len) }));
            }
        }
    }
    close(&mut run, &mut hf);
    hf
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Rolling country: three octaves of gradient noise, metres.
fn relief(x: f32, z: f32) -> f32 {
    let p = Vec2::new(x, z);
    7.0 * perlin(p / 700.0) + 1.6 * perlin(p / 170.0 + Vec2::new(31.7, 8.3)) + 0.3 * perlin(p / 40.0 + Vec2::new(-5.1, 17.9))
}

fn hash(i: i32, j: i32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x8da6_b343) ^ (j as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x9e37_79b1);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

/// Classic 2D gradient noise, roughly in [-0.7, 0.7].
fn perlin(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    let (i, j) = (i.x as i32, i.y as i32);
    let g = |di: i32, dj: i32| {
        let a = hash(i + di, j + dj) * TAU;
        Vec2::new(a.cos(), a.sin()).dot(f - Vec2::new(di as f32, dj as f32))
    };
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let x0 = g(0, 0) + (g(1, 0) - g(0, 0)) * u.x;
    let x1 = g(0, 1) + (g(1, 1) - g(0, 1)) * u.x;
    x0 + (x1 - x0) * u.y
}

/// Surface tints, linear. Grass by height, earth on slopes, cinders on the formation.
struct Tints {
    grass_lo: Vec3,
    grass_hi: Vec3,
    earth: Vec3,
    formation: Vec3,
}

impl Tints {
    fn new() -> Self {
        Tints { grass_lo: lin(0.17, 0.24, 0.13), grass_hi: lin(0.38, 0.36, 0.20), earth: lin(0.40, 0.33, 0.24), formation: lin(0.30, 0.28, 0.25) }
    }

    /// Colour of a vertex from its height, normal and what shaped it, with a mottle and a
    /// faint step every two metres of height: contour lines for free.
    fn at(&self, x: f32, z: f32, y: f32, n: Vec3, kind: u8) -> [f32; 4] {
        let grad = (1.0 - n.y * n.y).max(0.0).sqrt() / n.y.max(0.05);
        let by_h = ((y - GROUND_MEAN) / 14.0 + 0.4).clamp(0.0, 1.0);
        let mut c = self.grass_lo.lerp(self.grass_hi, by_h);
        c = c.lerp(self.earth, ((grad - 0.12) / 0.4).clamp(0.0, 1.0));
        if kind == 2 {
            c = self.formation;
        }
        c *= 1.0 + 0.06 * perlin(Vec2::new(x, z) / 70.0 + Vec2::new(3.3, -9.1));
        c *= 1.0 + 0.07 * (((y - GROUND_MEAN) / 2.0).rem_euclid(1.0) - 0.5);
        [c.x, c.y, c.z, 1.0]
    }
}

fn lin(r: f32, g: f32, b: f32) -> Vec3 {
    let c = Color::srgb(r, g, b).to_linear();
    Vec3::new(c.red, c.green, c.blue)
}

/// The colour of flat ground at datum, for the horizon plane beyond the ring. The mottle
/// averages out; the contour step at datum is the low half.
pub fn level_ground_color() -> Color {
    let t = Tints::new();
    let base = t.grass_lo.lerp(t.grass_hi, 0.4) * (1.0 - 0.035);
    Color::linear_rgb(base.x, base.y, base.z)
}

fn grid_mesh(pos: Vec<[f32; 3]>, nrm: Vec<[f32; 3]>, col: Vec<[f32; 4]>, idx: Vec<u32>) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, pos)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, nrm)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, col)
        .with_inserted_indices(Indices::U32(idx))
}

/// One mesh per chunk of the fine field, then the coarse ring around it. World-space
/// vertices, smooth normals, vertex colours.
pub fn chunk_meshes(hf: &Heightfield) -> Vec<Mesh> {
    let t = Tints::new();
    let mut out = Vec::new();
    let (cx_n, cz_n) = ((hf.nx - 1).div_ceil(CHUNK), (hf.nz - 1).div_ceil(CHUNK));
    for cz in 0..cz_n {
        for cx in 0..cx_n {
            let (i0, j0) = (cx * CHUNK, cz * CHUNK);
            let (i1, j1) = ((i0 + CHUNK).min(hf.nx - 1), (j0 + CHUNK).min(hf.nz - 1));
            let (w, d) = (i1 - i0 + 1, j1 - j0 + 1);
            let mut pos = Vec::with_capacity(w * d);
            let mut nrm = Vec::with_capacity(w * d);
            let mut col = Vec::with_capacity(w * d);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let (x, z) = (hf.origin.x + i as f32 * hf.cell, hf.origin.y + j as f32 * hf.cell);
                    let y = hf.at(i, j);
                    let n = hf.normal(i, j);
                    pos.push([x, y, z]);
                    nrm.push(n.to_array());
                    col.push(t.at(x, z, y, n, hf.kind[j * hf.nx + i]));
                }
            }
            out.push(grid_mesh(pos, nrm, col, grid_indices(w, d)));
        }
    }
    out.extend(ring_meshes(hf, &t));
    out
}

fn grid_indices(w: usize, d: usize) -> Vec<u32> {
    let mut idx = Vec::with_capacity((w - 1) * (d - 1) * 6);
    for j in 0..d as u32 - 1 {
        for i in 0..w as u32 - 1 {
            let a = j * w as u32 + i;
            let b = a + w as u32;
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    idx
}

/// Coarse ground from the fine field out to the ring's edge, in chunks, leaving the fine
/// field's footprint open. Heights straight from the natural ground.
fn ring_meshes(hf: &Heightfield, t: &Tints) -> Vec<Mesh> {
    let (fmin, fmax) = (hf.origin, hf.max());
    let (omin, omax) = (hf.site.outer_min, hf.site.outer_max);
    let nx = ((omax.x - omin.x) / RING_CELL).round() as usize + 1;
    let nz = ((omax.y - omin.y) / RING_CELL).round() as usize + 1;
    let at = |i: usize, j: usize| {
        let (x, z) = (omin.x + i as f32 * RING_CELL, omin.y + j as f32 * RING_CELL);
        (x, z, hf.site.ground(x, z))
    };
    let mut out = Vec::new();
    let (cx_n, cz_n) = ((nx - 1).div_ceil(CHUNK), (nz - 1).div_ceil(CHUNK));
    for cz in 0..cz_n {
        for cx in 0..cx_n {
            let (i0, j0) = (cx * CHUNK, cz * CHUNK);
            let (i1, j1) = ((i0 + CHUNK).min(nx - 1), (j0 + CHUNK).min(nz - 1));
            let w = i1 - i0 + 1;
            let mut pos = Vec::new();
            let mut nrm = Vec::new();
            let mut col = Vec::new();
            let mut idx = Vec::new();
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let (x, z, y) = at(i, j);
                    let (xa, _, ya) = at(i.saturating_sub(1), j);
                    let (xb, _, yb) = at((i + 1).min(nx - 1), j);
                    let (_, za, yc) = at(i, j.saturating_sub(1));
                    let (_, zb, yd) = at(i, (j + 1).min(nz - 1));
                    let n = Vec3::new(-(yb - ya) / (xb - xa).max(1.0), 1.0, -(yd - yc) / (zb - za).max(1.0)).normalize();
                    pos.push([x, y, z]);
                    nrm.push(n.to_array());
                    col.push(t.at(x, z, y, n, 0));
                }
            }
            for j in 0..(j1 - j0) {
                for i in 0..(i1 - i0) {
                    let (x, z, _) = at(i0 + i, j0 + j);
                    let inside = x >= fmin.x - 0.5 && x + RING_CELL <= fmax.x + 0.5 && z >= fmin.y - 0.5 && z + RING_CELL <= fmax.y + 0.5;
                    if inside {
                        continue;
                    }
                    let a = (j * w + i) as u32;
                    let b = a + w as u32;
                    idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
                }
            }
            if !idx.is_empty() {
                out.push(grid_mesh(pos, nrm, col, idx));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Branch's river is carved to its bed under both mains, each main bridges it,
    /// and the spiral's exit is carried over its own entry.
    #[test]
    fn branch_river_is_carved_and_bridged_and_the_spiral_crosses_itself() {
        let sim = Sim::load(3, 0);
        let hf = build(&sim);
        assert_eq!(hf.rivers.len(), 1);
        let bed = hf.height_at(-3278.0, 0.0);
        assert!(bed < -7.0, "river bed under the south main is at {bed}");
        let far = hf.height_at(-3262.0, -400.0);
        assert!(far < -7.0, "river bed away from track is at {far}");
        let g = &sim.world.graph;
        let crossing = |x: f32, z: f32| hf.bridges.iter().any(|sp| {
            let p = pose_to_world(&g.pose_on_edge(sp.edge, (sp.s0 + sp.s1) * 0.5));
            (p.x - x).abs() < 60.0 && (p.z - z).abs() < 60.0
        });
        assert!(crossing(-3278.0, 0.0), "no bridge over the river on the south main: {:?}", hf.bridges);
        assert!(crossing(-3240.0, -700.0), "no bridge over the river on the north main");
        let exit_bridged = hf.bridges.iter().any(|sp| {
            let p = pose_to_world(&g.pose_on_edge(sp.edge, sp.s0));
            (p.x - hat_world::SPIRAL_X as f32).abs() < 5.0 && p.z.abs() < 5.0 && p.y > 10.0
        });
        assert!(exit_bridged, "the spiral exit is not bridged over its entry: {:?}", hf.bridges);
    }
}
