//! Procedural terrain, track and rolling-stock meshes, car entity sync, gizmo overlays.
//!
//! Vertical datum: a pose's `z` is rail top. Ties, ballast and the formation hang below
//! it; car bodies stand on it.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use hat_sim::*;

use crate::camera::{pose_to_world, sim_to_world};
use crate::input::{Sel, Selection};
use crate::sim::Sim;
use crate::terrain::{self, Heightfield, GROUND_MEAN};

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CarEntities>().add_systems(Startup, setup).add_systems(Update, (sync_cars, overlays, rebuild_scene));
    }
}

/// Rail section height, m. Rail top is the pose height.
const RAIL_H: f32 = 0.17;
const RAIL_HALF_W: f32 = 0.035;
const HALF_GAUGE: f32 = 0.7175;
const TIE_TOP: f32 = -RAIL_H;
const TIE_BOT: f32 = -0.35;
const TIE_HALF_LEN: f32 = 1.3;
const TIE_HALF_W: f32 = 0.12;
const TIE_STEP: f64 = 0.6;
/// Ballast crown, shoulder and toe. The toe is buried a little below the formation so the
/// prism never floats over a cell of terrain that dips.
const BALLAST_TOP: f32 = TIE_BOT;
const BALLAST_TOP_HALF_W: f32 = 1.9;
const BALLAST_BOT: f32 = -0.85;
const BALLAST_BOT_HALF_W: f32 = 2.9;
/// Edge softening on car bodies, m. Catches a highlight line under the sun.
const CHAMFER: f32 = 0.12;

#[derive(Resource)]
pub struct Palette {
    pub cube: Handle<Mesh>,
    pub cylinder: Handle<Mesh>,
    pub frame: Handle<StandardMaterial>,
    pub loco: Handle<StandardMaterial>,
    pub loco_cab: Handle<StandardMaterial>,
    pub hopper: Handle<StandardMaterial>,
    pub covered: Handle<StandardMaterial>,
    pub tank: Handle<StandardMaterial>,
    pub boxcar: Handle<StandardMaterial>,
    pub gondola: Handle<StandardMaterial>,
    pub flat: Handle<StandardMaterial>,
    pub coal: Handle<StandardMaterial>,
    pub grain: Handle<StandardMaterial>,
    pub aggregate: Handle<StandardMaterial>,
    pub lumber: Handle<StandardMaterial>,
    pub mixed: Handle<StandardMaterial>,
    pub hand_brake: Handle<StandardMaterial>,
    pub bumper: Handle<StandardMaterial>,
    pub switch: Handle<StandardMaterial>,
    pub ballast: Handle<StandardMaterial>,
    pub tie: Handle<StandardMaterial>,
    pub rail: Handle<StandardMaterial>,
    /// White; the terrain mesh carries its colour per vertex.
    pub terrain: Handle<StandardMaterial>,
    /// White, for meshes that carry their own colours: trucks, couplers, bridges.
    pub parts: Handle<StandardMaterial>,
    pub water: Handle<StandardMaterial>,
}

/// Meshes built on demand from car dimensions, shared between cars of a type.
#[derive(Resource)]
pub struct Shapes {
    boxes: HashMap<[i32; 4], Handle<Mesh>>,
    ribs: HashMap<[i32; 3], Handle<Mesh>>,
    trucks: HashMap<[i32; 3], Handle<Mesh>>,
    heap: Handle<Mesh>,
    coupler: Handle<Mesh>,
}

impl Shapes {
    fn boxed(&mut self, meshes: &mut Assets<Mesh>, size: Vec3, chamfer: f32) -> Handle<Mesh> {
        let key = [(size.x * 100.0) as i32, (size.y * 100.0) as i32, (size.z * 100.0) as i32, (chamfer * 1000.0) as i32];
        self.boxes.entry(key).or_insert_with(|| meshes.add(chamfered_box(size, chamfer))).clone()
    }
    fn ribs(&mut self, meshes: &mut Assets<Mesh>, len: f32, w: f32, y0: f32, y1: f32) -> Handle<Mesh> {
        let key = [(len * 100.0) as i32, (w * 100.0) as i32, ((y1 - y0) * 100.0) as i32];
        self.ribs.entry(key).or_insert_with(|| meshes.add(ribs_mesh(len, w, y0, y1))).clone()
    }
    fn truck(&mut self, meshes: &mut Assets<Mesh>, axles: usize, r: f32, wb: f32) -> Handle<Mesh> {
        let key = [axles as i32, (r * 100.0) as i32, (wb * 100.0) as i32];
        self.trucks.entry(key).or_insert_with(|| meshes.add(truck_mesh(axles, r, wb))).clone()
    }
}

/// Anything built from the world: terrain, track, structures. Dropped and rebuilt when the
/// scenario changes.
#[derive(Component)]
pub struct SceneVisual {
    pub generation: u32,
}

#[derive(Component)]
pub struct CarVisual {
    pub id: CarId,
    pub hb_marker: Entity,
    pub generation: u32,
}

#[derive(Resource, Default)]
pub struct CarEntities(pub HashMap<CarId, Entity>);

fn mat(m: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32) -> Handle<StandardMaterial> {
    m.add(StandardMaterial { base_color: Color::srgb(r, g, b), perceptual_roughness: 0.85, ..default() })
}

fn unlit(m: &mut Assets<StandardMaterial>, r: f32, g: f32, b: f32) -> Handle<StandardMaterial> {
    m.add(StandardMaterial { base_color: Color::srgb(r, g, b), unlit: true, ..default() })
}

fn setup(mut commands: Commands, sim: Res<Sim>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cylinder = meshes.add(Cylinder::new(0.5, 1.0));
    let heap = meshes.add(Sphere::new(0.5).mesh().ico(1).expect("icosphere").with_duplicated_vertices().with_computed_flat_normals());
    let m = &mut materials;
    let palette = Palette {
        cube: cube.clone(),
        cylinder: cylinder.clone(),
        frame: mat(m, 0.12, 0.12, 0.13),
        loco: mat(m, 0.86, 0.55, 0.12),
        loco_cab: mat(m, 0.16, 0.16, 0.20),
        hopper: mat(m, 0.38, 0.19, 0.14),
        covered: mat(m, 0.72, 0.70, 0.64),
        tank: mat(m, 0.17, 0.17, 0.19),
        boxcar: mat(m, 0.55, 0.19, 0.14),
        gondola: mat(m, 0.30, 0.33, 0.30),
        flat: mat(m, 0.36, 0.28, 0.20),
        coal: mat(m, 0.05, 0.05, 0.06),
        grain: mat(m, 0.86, 0.70, 0.30),
        aggregate: mat(m, 0.62, 0.61, 0.58),
        lumber: mat(m, 0.80, 0.64, 0.38),
        mixed: mat(m, 0.50, 0.50, 0.50),
        hand_brake: unlit(m, 1.0, 0.85, 0.10),
        bumper: unlit(m, 0.85, 0.15, 0.10),
        switch: unlit(m, 0.95, 0.85, 0.20),
        ballast: m.add(StandardMaterial { base_color: Color::srgb(0.46, 0.44, 0.41), perceptual_roughness: 1.0, ..default() }),
        tie: m.add(StandardMaterial { base_color: Color::srgb(0.30, 0.22, 0.16), perceptual_roughness: 1.0, ..default() }),
        rail: m.add(StandardMaterial { base_color: Color::srgb(0.62, 0.62, 0.64), perceptual_roughness: 0.4, metallic: 0.7, ..default() }),
        terrain: m.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 1.0, ..default() }),
        parts: m.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.75, ..default() }),
        water: m.add(StandardMaterial { base_color: Color::srgb(0.26, 0.42, 0.55), perceptual_roughness: 0.10, ..default() }),
    };

    let hf = spawn_scene(&mut commands, &sim, &mut meshes, &mut materials, &palette);
    commands.insert_resource(hf);
    commands.insert_resource(palette);
    let coupler = meshes.add(coupler_mesh());
    commands.insert_resource(Shapes { boxes: HashMap::new(), ribs: HashMap::new(), trucks: HashMap::new(), heap, coupler });
}

/// Terrain, track and structures for the current world. Returns the heightfield they sit on.
fn spawn_scene(commands: &mut Commands, sim: &Sim, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, palette: &Palette) -> Heightfield {
    let t0 = std::time::Instant::now();
    let hf = terrain::build(sim);
    let chunks = terrain::chunk_meshes(&hf);
    info!("terrain: {}x{} cells, {} chunks, {} bridges, {} tunnels, {:.0} ms", hf.nx - 1, hf.nz - 1, chunks.len(), hf.bridges.len(), hf.tunnels.len(), t0.elapsed().as_secs_f32() * 1000.0);
    for mesh in chunks {
        commands.spawn((Mesh3d(meshes.add(mesh)), MeshMaterial3d(palette.terrain.clone()), SceneVisual { generation: sim.generation }));
    }
    spawn_track(commands, sim, meshes, palette);
    spawn_structures(commands, sim, meshes, palette, &hf);
    spawn_horizon(commands, sim, meshes, materials, &hf);
    spawn_industries(commands, sim, palette, materials, &hf);
    hf
}

/// Level ground from the ring's edge out to the horizon: four strips around the field,
/// none under it, so valleys the field carves below datum stay open.
fn spawn_horizon(commands: &mut Commands, sim: &Sim, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, hf: &Heightfield) {
    let (lo, hi) = hf.outer_bounds();
    let far = 30_000.0f32;
    let mat = materials.add(StandardMaterial { base_color: terrain::level_ground_color(), perceptual_roughness: 1.0, ..default() });
    // (x0, x1, z0, z1) for the west, east, north and south strips.
    let strips = [(-far, lo.x, -far, far), (hi.x, far, -far, far), (lo.x, hi.x, -far, lo.y), (lo.x, hi.x, hi.y, far)];
    for (x0, x1, z0, z1) in strips {
        let mesh = meshes.add(Plane3d::default().mesh().size(x1 - x0, z1 - z0));
        commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat.clone()), Transform::from_xyz((x0 + x1) * 0.5, GROUND_MEAN - 0.05, (z0 + z1) * 0.5), SceneVisual { generation: sim.generation }));
    }
}

/// Industry buildings: boxes that read as what they are from above, on their pad.
fn spawn_industries(commands: &mut Commands, sim: &Sim, palette: &Palette, materials: &mut Assets<StandardMaterial>, hf: &Heightfield) {
    let Some(e) = sim.economy.as_ref() else { return };
    let gen = sim.generation;
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.15, 0.14), perceptual_roughness: 1.0, ..default() });
    let brick = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.30, 0.22), perceptual_roughness: 0.9, ..default() });
    let silo = materials.add(StandardMaterial { base_color: Color::srgb(0.82, 0.80, 0.72), perceptual_roughness: 0.7, ..default() });
    let steel = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.48, 0.52), perceptual_roughness: 0.6, metallic: 0.4, ..default() });
    for ind in &e.industries {
        let mut base = sim_to_world(ind.pos);
        base.y = hf.height_at(base.x, base.z);
        let mut spawn = |mesh: Handle<Mesh>, mat: Handle<StandardMaterial>, offset: Vec3, scale: Vec3| {
            commands.spawn((Mesh3d(mesh), MeshMaterial3d(mat), Transform { translation: base + offset + Vec3::Y * (scale.y * 0.5), scale, ..default() }, SceneVisual { generation: gen }));
        };
        match ind.kind {
            hat_world::IndustryKind::CoalMine => {
                spawn(palette.cube.clone(), steel.clone(), Vec3::new(0.0, 0.0, 0.0), Vec3::new(14.0, 18.0, 14.0));
                spawn(palette.cube.clone(), dark.clone(), Vec3::new(-30.0, 0.0, -10.0), Vec3::new(40.0, 7.0, 26.0));
                spawn(palette.cube.clone(), palette.coal.clone(), Vec3::new(-30.0, 7.0, -10.0), Vec3::new(30.0, 6.0, 18.0));
            }
            hat_world::IndustryKind::PowerPlant => {
                spawn(palette.cube.clone(), brick.clone(), Vec3::new(0.0, 0.0, -20.0), Vec3::new(60.0, 22.0, 40.0));
                spawn(palette.cylinder.clone(), silo.clone(), Vec3::new(40.0, 0.0, -30.0), Vec3::new(8.0, 70.0, 8.0));
                spawn(palette.cube.clone(), palette.coal.clone(), Vec3::new(-30.0, 0.0, 10.0), Vec3::new(50.0, 5.0, 24.0));
            }
            hat_world::IndustryKind::Elevator => {
                spawn(palette.cube.clone(), silo.clone(), Vec3::new(0.0, 0.0, 0.0), Vec3::new(12.0, 26.0, 10.0));
                spawn(palette.cube.clone(), brick.clone(), Vec3::new(16.0, 0.0, 0.0), Vec3::new(18.0, 8.0, 12.0));
            }
            hat_world::IndustryKind::GrainTerminal => {
                for k in 0..6 {
                    spawn(palette.cylinder.clone(), silo.clone(), Vec3::new(-25.0 + 10.0 * k as f32, 0.0, 0.0), Vec3::new(9.0, 30.0, 9.0));
                }
                spawn(palette.cube.clone(), steel.clone(), Vec3::new(0.0, 30.0, 0.0), Vec3::new(64.0, 5.0, 10.0));
            }
        }
    }
}

/// Track, bumpers and switch discs for the current world.
fn spawn_track(commands: &mut Commands, sim: &Sim, meshes: &mut Assets<Mesh>, palette: &Palette) {
    let gen = sim.generation;
    let tag = || SceneVisual { generation: gen };
    let ballast_profile = [Vec2::new(BALLAST_BOT_HALF_W, BALLAST_BOT), Vec2::new(BALLAST_TOP_HALF_W, BALLAST_TOP), Vec2::new(-BALLAST_TOP_HALF_W, BALLAST_TOP), Vec2::new(-BALLAST_BOT_HALF_W, BALLAST_BOT)];
    let rail_profile = |o: f32| [Vec2::new(o + RAIL_HALF_W, -RAIL_H), Vec2::new(o + RAIL_HALF_W, 0.0), Vec2::new(o - RAIL_HALF_W, 0.0), Vec2::new(o - RAIL_HALF_W, -RAIL_H)];
    for (ei, _edge) in sim.world.graph.edges.iter().enumerate() {
        let poses = edge_samples(&sim.world.graph, ei as EdgeId);
        let mut ballast = Geo::default();
        profile_strip(&mut ballast, &poses, &ballast_profile);
        commands.spawn((Mesh3d(meshes.add(ballast.mesh())), MeshMaterial3d(palette.ballast.clone()), tag()));
        let mut rails = Geo::default();
        for o in [HALF_GAUGE, -HALF_GAUGE] {
            profile_strip(&mut rails, &poses, &rail_profile(o));
        }
        commands.spawn((Mesh3d(meshes.add(rails.mesh())), MeshMaterial3d(palette.rail.clone()), tag()));
        commands.spawn((Mesh3d(meshes.add(ties(&sim.world.graph, ei as EdgeId).mesh())), MeshMaterial3d(palette.tie.clone()), tag()));
    }
    for (ni, node) in sim.world.graph.nodes.iter().enumerate() {
        match &node.kind {
            NodeKind::End => {
                let heading = node
                    .edges
                    .first()
                    .map(|&e| {
                        let edge = sim.world.graph.edge(e);
                        if edge.a == ni as NodeId { edge.geom.start().heading } else { edge.geom.end().heading }
                    })
                    .unwrap_or(0.0) as f32;
                commands.spawn((
                    Mesh3d(palette.cube.clone()),
                    MeshMaterial3d(palette.bumper.clone()),
                    Transform { translation: sim_to_world(node.pos) + Vec3::Y * (0.8 + node.z as f32), rotation: Quat::from_rotation_y(heading), scale: Vec3::new(1.2, 1.5, 3.2) },
                    tag(),
                ));
            }
            NodeKind::Turnout { .. } => {
                commands.spawn((
                    Mesh3d(palette.cylinder.clone()),
                    MeshMaterial3d(palette.switch.clone()),
                    Transform { translation: sim_to_world(node.pos) + Vec3::Y * (0.15 + node.z as f32), scale: Vec3::new(3.2, 0.3, 3.2), ..default() },
                    tag(),
                ));
            }
            NodeKind::Plain => {}
        }
    }
}

/// Bridges, tunnel portals and water for the current world.
fn spawn_structures(commands: &mut Commands, sim: &Sim, meshes: &mut Assets<Mesh>, palette: &Palette, hf: &Heightfield) {
    let g = &sim.world.graph;
    let gen = sim.generation;
    let steel = lin(0.22, 0.25, 0.30);
    let concrete = lin(0.56, 0.54, 0.50);
    let mut decks = Geo::default();
    let mut piers = Geo::default();
    for sp in &hf.bridges {
        let len = sp.s1 - sp.s0;
        let n = ((len / 3.0).ceil() as usize).max(1);
        let poses: Vec<Pose> = (0..=n).map(|i| g.pose_on_edge(sp.edge, sp.s0 + len * i as f64 / n as f64)).collect();
        // Deck slab under the ballast, a plate girder either side.
        decks.pen = Some(steel);
        profile_strip(&mut decks, &poses, &[Vec2::new(2.7, -1.15), Vec2::new(2.7, -0.85), Vec2::new(-2.7, -0.85), Vec2::new(-2.7, -1.15), Vec2::new(2.7, -1.15)]);
        for o in [2.4f32, -2.4] {
            let (a, b) = (o + 0.15, o - 0.15);
            profile_strip(&mut decks, &poses, &[Vec2::new(a, -2.3), Vec2::new(a, -0.85), Vec2::new(b, -0.85), Vec2::new(b, -2.3), Vec2::new(a, -2.3)]);
        }
        // Piers every 24 m and at both ends, down into the ground.
        let np = ((len / 24.0).ceil() as usize).max(1);
        piers.pen = Some(concrete);
        for i in 0..=np {
            let p = g.pose_on_edge(sp.edge, sp.s0 + len * i as f64 / np as f64);
            let c = pose_to_world(&p);
            let ground = hf.height_at(c.x, c.z);
            let top = c.y - 2.3;
            if top > ground + 0.3 {
                piers.box5(c, forward_of(&p), left_of(&p), 1.0, 2.2, ground - 1.0, top);
            }
        }
    }
    // Tunnel mouths: two walls and a lintel; the hill hides the rest.
    piers.pen = Some(concrete);
    for sp in &hf.tunnels {
        for s in [sp.s0, sp.s1] {
            let p = g.pose_on_edge(sp.edge, s);
            let c = pose_to_world(&p);
            let (f, l) = (forward_of(&p), left_of(&p));
            for side in [-1.0f32, 1.0] {
                piers.box5(c + l * (side * 3.2), f, l, 0.7, 0.6, c.y - 1.5, c.y + 5.6);
            }
            piers.box5(c, f, l, 0.7, 3.8, c.y + 4.8, c.y + 5.6);
        }
    }
    for geo in [decks, piers] {
        if !geo.v.is_empty() {
            commands.spawn((Mesh3d(meshes.add(geo.mesh())), MeshMaterial3d(palette.parts.clone()), SceneVisual { generation: gen }));
        }
    }
    for river in &hf.rivers {
        let mut water = Geo::default();
        ribbon(&mut water, &river.points, river.half_w, river.level);
        commands.spawn((Mesh3d(meshes.add(water.mesh())), MeshMaterial3d(palette.water.clone()), SceneVisual { generation: gen }));
    }
}

/// A level ribbon along a polyline in world x, z, facing up.
fn ribbon(geo: &mut Geo, pts: &[Vec2], half_w: f32, y: f32) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let base = geo.v.len() as u32;
    let col = geo.col();
    for i in 0..n {
        let (a, b) = (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)]);
        let d = (b - a).normalize_or_zero();
        let l = Vec2::new(-d.y, d.x) * half_w;
        for p in [pts[i] - l, pts[i] + l] {
            geo.v.push([p.x, y, p.y]);
            geo.n.push([0.0, 1.0, 0.0]);
            geo.c.push(col);
        }
    }
    for i in 0..(n as u32 - 1) {
        let a = base + 2 * i;
        geo.i.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
}

/// When the scenario changes, the ground, track and structures do too.
fn rebuild_scene(mut commands: Commands, sim: Res<Sim>, pal: Option<Res<Palette>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, q: Query<(Entity, &SceneVisual)>) {
    let Some(pal) = pal else { return };
    let stale: Vec<Entity> = q.iter().filter(|(_, t)| t.generation != sim.generation).map(|(e, _)| e).collect();
    if stale.is_empty() {
        return;
    }
    for e in stale {
        commands.entity(e).despawn();
    }
    let hf = spawn_scene(&mut commands, &sim, &mut meshes, &mut materials, &pal);
    commands.insert_resource(hf);
}

/// Triangle soup with flat or strip normals and a colour per vertex, built face by face.
#[derive(Default)]
struct Geo {
    v: Vec<[f32; 3]>,
    n: Vec<[f32; 3]>,
    c: Vec<[f32; 4]>,
    i: Vec<u32>,
    /// Colour for the faces pushed next. None is white, which leaves the material's colour alone.
    pen: Option<[f32; 4]>,
}

impl Geo {
    fn col(&self) -> [f32; 4] {
        self.pen.unwrap_or([1.0; 4])
    }

    /// A quad around `c` spanned by half-extents `u` and `v`, with `u x v` along the outward normal.
    fn quad(&mut self, c: Vec3, u: Vec3, v: Vec3) {
        self.face([c - u - v, c + u - v, c + u + v, c - u + v], u.cross(v));
    }

    /// Four corners in loop order, wound so the normal points along `out`.
    fn face(&mut self, mut p: [Vec3; 4], out: Vec3) {
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(out) < 0.0 {
            p.swap(1, 3);
        }
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize().to_array();
        let b = self.v.len() as u32;
        let col = self.col();
        for q in p {
            self.v.push(q.to_array());
            self.n.push(n);
            self.c.push(col);
        }
        self.i.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }

    fn tri(&mut self, mut p: [Vec3; 3], out: Vec3) {
        if (p[1] - p[0]).cross(p[2] - p[0]).dot(out) < 0.0 {
            p.swap(1, 2);
        }
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize().to_array();
        let b = self.v.len() as u32;
        let col = self.col();
        for q in p {
            self.v.push(q.to_array());
            self.n.push(n);
            self.c.push(col);
        }
        self.i.extend_from_slice(&[b, b + 1, b + 2]);
    }

    /// Five faces of a box, no bottom. `f` and `l` are unit forward and left; `y0..y1` is height.
    fn box5(&mut self, c: Vec3, f: Vec3, l: Vec3, hf: f32, hl: f32, y0: f32, y1: f32) {
        let hy = (y1 - y0) * 0.5;
        let c = Vec3::new(c.x, y0 + hy, c.z);
        let (f, l, y) = (f * hf, l * hl, Vec3::Y * hy);
        self.quad(c + y, f, l);
        self.quad(c + f, l, y);
        self.quad(c - f, y, l);
        self.quad(c + l, y, f);
        self.quad(c - l, f, y);
    }

    /// A box from its centre and three half-extent vectors, any orientation.
    fn cuboid(&mut self, c: Vec3, ax: Vec3, ay: Vec3, az: Vec3) {
        for (a, b, d) in [(ax, ay, az), (ay, az, ax), (az, ax, ay)] {
            self.face([c + a - b - d, c + a + b - d, c + a + b + d, c + a - b + d], a);
            self.face([c - a - b - d, c - a + b - d, c - a + b + d, c - a - b + d], -a);
        }
    }

    /// A bar between two points in the x-y plane at depth `z`, `thick` tall and `2 * hz` deep.
    fn bar(&mut self, from: Vec2, to: Vec2, thick: f32, z: f32, hz: f32) {
        let d = to - from;
        let dir = d.normalize();
        let c = (from + to) * 0.5;
        self.cuboid(Vec3::new(c.x, c.y, z), Vec3::new(dir.x, dir.y, 0.0) * (d.length() * 0.5), Vec3::new(-dir.y, dir.x, 0.0) * (thick * 0.5), Vec3::Z * hz);
    }

    /// A faceted cylinder along `axis`, flat shaded, with separate colours for side and ends.
    fn cylinder(&mut self, c: Vec3, axis: Vec3, r: f32, half: f32, sides: usize, side: [f32; 4], cap: [f32; 4]) {
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        let ring = |k: usize, s: f32| {
            let a = TAU * (k % sides) as f32 / sides as f32;
            c + axis * s + (u * a.cos() + v * a.sin()) * r
        };
        for k in 0..sides {
            let (p0, p1, p2, p3) = (ring(k, -half), ring(k + 1, -half), ring(k + 1, half), ring(k, half));
            self.pen = Some(side);
            self.face([p0, p1, p2, p3], (p0 + p1) * 0.5 - (c - axis * half));
            self.pen = Some(cap);
            self.tri([c + axis * half, p3, p2], axis);
            self.tri([c - axis * half, p0, p1], -axis);
        }
    }

    fn mesh(self) -> Mesh {
        let n = self.v.len();
        debug_assert!(self.n.len() == n && self.c.len() == n, "geometry attributes out of step: {} positions, {} normals, {} colours", n, self.n.len(), self.c.len());
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.v)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.n)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.c)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
            .with_inserted_indices(Indices::U32(self.i))
    }
}

fn edge_samples(g: &TrackGraph, id: EdgeId) -> Vec<Pose> {
    let e = g.edge(id);
    let graded = (g.node(e.a).z - g.node(e.b).z).abs() > 1e-6;
    let n = match e.geom {
        Geometry::Straight { .. } if !graded => 1,
        Geometry::Straight { .. } => ((e.length / 10.0).ceil() as usize).max(2),
        Geometry::Arc { .. } => ((e.length / 1.5).ceil() as usize).max(6),
    };
    (0..=n).map(|i| g.pose_on_edge(id, e.length * i as f64 / n as f64)).collect()
}

fn left_of(p: &Pose) -> Vec3 {
    let h = p.heading as f32;
    Vec3::new(-h.sin(), 0.0, -h.cos())
}

fn forward_of(p: &Pose) -> Vec3 {
    let h = p.heading as f32;
    Vec3::new(h.cos(), 0.0, -h.sin())
}

/// Extrude a cross-section along the poses. The profile is (offset to the left, height)
/// pairs listed from the right-hand toe over the top to the left-hand toe, so every face
/// winds outward. Flat across the profile, smooth along the track, following the grade.
fn profile_strip(geo: &mut Geo, poses: &[Pose], profile: &[Vec2]) {
    let centers: Vec<Vec3> = poses.iter().map(pose_to_world).collect();
    let fwd: Vec<Vec3> = (0..poses.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(1), (i + 1).min(poses.len() - 1));
            (centers[b] - centers[a]).try_normalize().unwrap_or_else(|| forward_of(&poses[i]))
        })
        .collect();
    for seg in profile.windows(2) {
        let (p0, p1) = (seg[0], seg[1]);
        let base = geo.v.len() as u32;
        for (i, p) in poses.iter().enumerate() {
            let l = left_of(p);
            let t = l * (p1.x - p0.x) + Vec3::Y * (p1.y - p0.y);
            let n = t.cross(fwd[i]).normalize().to_array();
            geo.v.push((centers[i] + l * p0.x + Vec3::Y * p0.y).to_array());
            geo.v.push((centers[i] + l * p1.x + Vec3::Y * p1.y).to_array());
            geo.n.push(n);
            geo.n.push(n);
            let col = geo.col();
            geo.c.push(col);
            geo.c.push(col);
        }
        for i in 0..(poses.len() as u32 - 1) {
            let a = base + 2 * i;
            geo.i.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
        }
    }
}

fn ties(g: &TrackGraph, id: EdgeId) -> Geo {
    let e = g.edge(id);
    let n = (e.length / TIE_STEP).floor() as usize;
    let mut geo = Geo::default();
    for i in 0..n {
        let p = g.pose_on_edge(id, (i as f64 + 0.5) * TIE_STEP);
        let c = pose_to_world(&p);
        geo.box5(c, forward_of(&p), left_of(&p), TIE_HALF_W, TIE_HALF_LEN, c.y + TIE_BOT, c.y + TIE_TOP);
    }
    geo
}

/// A box with its twelve edges bevelled: six octagons, twelve bevel quads, eight corner
/// triangles, every face flat. Faces are found by which coordinates their points share, so
/// nothing here depends on hand-ordered vertex tables.
fn chamfered_box(size: Vec3, c: f32) -> Mesh {
    let h = size * 0.5;
    let c = c.min(h.min_element() * 0.45);
    let mut pts: Vec<Vec3> = Vec::with_capacity(24);
    for k in 0..8u32 {
        let s = Vec3::new(if k & 1 == 0 { -1.0 } else { 1.0 }, if k & 2 == 0 { -1.0 } else { 1.0 }, if k & 4 == 0 { -1.0 } else { 1.0 });
        for axis in 0..3 {
            let mut inset = Vec3::ZERO;
            inset[axis] = c;
            pts.push(s * (h - inset));
        }
    }
    let eq = |a: f32, b: f32| (a - b).abs() < 1e-4;
    let mut faces: Vec<Vec<usize>> = Vec::new();
    for axis in 0..3 {
        for s in [-1.0f32, 1.0] {
            faces.push((0..24).filter(|&p| eq(pts[p][axis], s * h[axis])).collect());
        }
    }
    for a in 0..3 {
        for b in (a + 1)..3 {
            for sa in [-1.0f32, 1.0] {
                for sb in [-1.0f32, 1.0] {
                    faces.push((0..24).filter(|&p| (eq(pts[p][a], sa * (h[a] - c)) && eq(pts[p][b], sb * h[b])) || (eq(pts[p][a], sa * h[a]) && eq(pts[p][b], sb * (h[b] - c)))).collect());
                }
            }
        }
    }
    for k in 0..8 {
        faces.push(vec![k * 3, k * 3 + 1, k * 3 + 2]);
    }
    let mut geo = Geo::default();
    for f in faces {
        let centroid = f.iter().map(|&p| pts[p]).sum::<Vec3>() / f.len() as f32;
        // Sort the face's points counter-clockwise around the outward direction, then fan.
        let out = centroid.normalize();
        let u = out.any_orthonormal_vector();
        let v = out.cross(u);
        let mut ring = f.clone();
        ring.sort_by(|&a, &b| {
            let (pa, pb) = (pts[a] - centroid, pts[b] - centroid);
            pa.dot(v).atan2(pa.dot(u)).total_cmp(&pb.dot(v).atan2(pb.dot(u)))
        });
        let n = (pts[ring[1]] - pts[ring[0]]).cross(pts[ring[2]] - pts[ring[0]]).normalize().to_array();
        let base = geo.v.len() as u32;
        let col = geo.col();
        for &p in &ring {
            geo.v.push(pts[p].to_array());
            geo.n.push(n);
            geo.c.push(col);
        }
        for k in 1..ring.len() as u32 - 1 {
            geo.i.extend_from_slice(&[base, base + k, base + k + 1]);
        }
    }
    geo.mesh()
}

/// Side posts along both sides of an open car, in the car's frame.
fn ribs_mesh(len: f32, w: f32, y0: f32, y1: f32) -> Mesh {
    let mut geo = Geo::default();
    let n = 7;
    for side in [-1.0f32, 1.0] {
        for k in 0..n {
            let x = -len * 0.42 + len * 0.84 * k as f32 / (n - 1) as f32;
            geo.box5(Vec3::new(x, 0.0, side * (w * 0.5 + 0.05)), Vec3::X, Vec3::NEG_Z, 0.06, 0.05, y0, y1);
        }
    }
    geo.mesh()
}

fn lin(r: f32, g: f32, b: f32) -> [f32; 4] {
    let c = Color::srgb(r, g, b).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// A truck in the car's frame, x along the car, y up from rail top: axles with faceted
/// wheels, two sideframes with journal boxes, spring windows and springs, and a bolster.
/// Two axles gives the three-piece freight truck; three gives a locomotive unit with a
/// taller frame. One mesh, so a car's running gear is two entities.
fn truck_mesh(axles: usize, r: f32, wb: f32) -> Mesh {
    let dark = lin(0.10, 0.10, 0.11);
    let rust = lin(0.33, 0.20, 0.14);
    let steel = lin(0.55, 0.55, 0.56);
    let mut g = Geo::default();
    let xs: Vec<f32> = (0..axles).map(|k| (k as f32 - (axles - 1) as f32 / 2.0) * wb).collect();
    for &x in &xs {
        g.cylinder(Vec3::new(x, r, 0.0), Vec3::Z, 0.07, 0.68, 8, rust, rust);
        for sz in [-1.0f32, 1.0] {
            g.cylinder(Vec3::new(x, r, sz * 0.75), Vec3::Z, r, 0.07, 12, steel, rust);
        }
    }
    let hz = 0.08;
    let top = if axles == 2 { 0.70 } else { 0.95 };
    for sz in [-1.0f32, 1.0] {
        let z = sz * 0.98;
        g.pen = Some(dark);
        for &x in &xs {
            g.cuboid(Vec3::new(x, r + 0.02, z), Vec3::X * 0.18, Vec3::Y * 0.15, Vec3::Z * 0.10);
        }
        if axles == 2 {
            // Window between the columns; top and bottom members run out to the journals.
            g.cuboid(Vec3::new(0.0, top, z), Vec3::X * 0.45, Vec3::Y * 0.05, Vec3::Z * hz);
            g.cuboid(Vec3::new(0.0, 0.30, z), Vec3::X * 0.45, Vec3::Y * 0.04, Vec3::Z * hz);
            for sx in [-1.0f32, 1.0] {
                g.cuboid(Vec3::new(sx * 0.45, 0.50, z), Vec3::X * 0.04, Vec3::Y * 0.20, Vec3::Z * hz);
                g.bar(Vec2::new(sx * 0.45, top), Vec2::new(sx * wb * 0.5, r + 0.13), 0.10, z, hz);
                g.bar(Vec2::new(sx * 0.45, 0.30), Vec2::new(sx * wb * 0.5, r - 0.09), 0.08, z, hz);
                g.cylinder(Vec3::new(sx * 0.17, 0.50, z), Vec3::Y, 0.07, 0.16, 6, dark, dark);
            }
        } else {
            let (x0, x1) = (xs[0], xs[axles - 1]);
            g.cuboid(Vec3::new(0.0, top, z), Vec3::X * ((x1 - x0) * 0.5 + 0.3), Vec3::Y * 0.06, Vec3::Z * hz);
            g.cuboid(Vec3::new(0.0, 0.30, z), Vec3::X * ((x1 - x0) * 0.5 + 0.1), Vec3::Y * 0.04, Vec3::Z * hz);
            for x in [x0 + 0.45, -0.45, 0.45, x1 - 0.45] {
                g.pen = Some(dark);
                g.cuboid(Vec3::new(x, (top + 0.30) * 0.5, z), Vec3::X * 0.04, Vec3::Y * ((top - 0.30) * 0.5), Vec3::Z * hz);
            }
            for x in [-wb * 0.5 - 0.15, -wb * 0.5 + 0.15, wb * 0.5 - 0.15, wb * 0.5 + 0.15] {
                g.cylinder(Vec3::new(x, 0.52, z), Vec3::Y, 0.07, 0.18, 6, dark, dark);
            }
        }
    }
    g.pen = Some(dark);
    g.cuboid(Vec3::new(0.0, top - 0.10, 0.0), Vec3::X * 0.20, Vec3::Y * 0.12, Vec3::Z * 1.06);
    g.mesh()
}

/// A knuckle coupler pointing along +x from the car end: shank, then the head.
fn coupler_mesh() -> Mesh {
    let mut g = Geo::default();
    g.pen = Some(lin(0.10, 0.10, 0.11));
    g.cuboid(Vec3::new(0.30, 0.0, 0.0), Vec3::X * 0.30, Vec3::Y * 0.08, Vec3::Z * 0.10);
    g.cuboid(Vec3::new(0.72, 0.0, 0.0), Vec3::X * 0.14, Vec3::Y * 0.14, Vec3::Z * 0.17);
    g.mesh()
}

pub fn car_transform(pose: &Pose, car: &CarState) -> Transform {
    let mut heading = pose.heading as f32;
    if !car.facing_head {
        heading += PI;
    }
    let mut t = Transform::from_translation(pose_to_world(pose)).with_rotation(Quat::from_rotation_y(heading));
    if car.derailed {
        let left = Vec3::new(-heading.sin(), 0.0, -heading.cos());
        t.translation += left * 1.4;
        t.rotation *= Quat::from_rotation_x(0.3);
    }
    t
}

/// Collects the parts of one car.
struct Kit<'a> {
    shapes: &'a mut Shapes,
    meshes: &'a mut Assets<Mesh>,
    parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>,
}

impl Kit<'_> {
    fn at(x: f32, y: f32, z: f32) -> Transform {
        Transform::from_xyz(x, y, z)
    }
    /// A bevelled box of the given size, centred at (x, y, z).
    fn body(&mut self, m: &Handle<StandardMaterial>, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) {
        let mesh = self.shapes.boxed(self.meshes, Vec3::new(sx, sy, sz), CHAMFER);
        self.parts.push((mesh, m.clone(), Self::at(x, y, z)));
    }
    /// A sharp box, for fittings.
    fn block(&mut self, cube: &Handle<Mesh>, m: &Handle<StandardMaterial>, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) {
        self.parts.push((cube.clone(), m.clone(), Transform { translation: Vec3::new(x, y, z), scale: Vec3::new(sx, sy, sz), ..default() }));
    }
    /// Payload heap poking above an open top at height `rim`.
    fn heap(&mut self, m: &Handle<StandardMaterial>, rim: f32, len: f32, w: f32, rise: f32) {
        self.parts.push((self.shapes.heap.clone(), m.clone(), Transform { translation: Vec3::new(0.0, rim - rise * 0.15, 0.0), scale: Vec3::new(len, rise * 2.0, w), ..default() }));
    }
    /// Dark bands wrapping both ends of a body, so the gap to the next car reads from above.
    fn end_caps(&mut self, m: &Handle<StandardMaterial>, len: f32, y: f32, h: f32, w: f32) {
        for sx in [-1.0f32, 1.0] {
            self.body(m, sx * (len * 0.5 - 0.13), y, 0.0, 0.3, h + 0.04, w + 0.04);
        }
    }
}

fn spawn_car(commands: &mut Commands, pal: &Palette, shapes: &mut Shapes, meshes: &mut Assets<Mesh>, ct: &CarType, car: &CarState, tf: Transform, generation: u32) -> Entity {
    let len = (ct.length - 1.0) as f32;
    let w = ct.width as f32;
    let h = ct.height as f32;
    let hb = commands
        .spawn((Mesh3d(pal.cube.clone()), MeshMaterial3d(pal.hand_brake.clone()), Transform { translation: Vec3::new(-len * 0.38, h + 0.5, 0.0), scale: Vec3::splat(0.9), ..default() }, Visibility::Hidden))
        .id();
    let parent = commands.spawn((tf, Visibility::default(), CarVisual { id: car.id, hb_marker: hb, generation })).id();
    let mut kit = Kit { shapes, meshes, parts: Vec::new() };
    let cube = &pal.cube;
    let loco = ct.kind == CarKind::Locomotive;
    // Running gear, then a thin underframe with the body floor on it. Coupler centres at
    // 0.88 above rail, close to the real 0.876.
    let floor = if loco { 1.2 } else { 1.0 };
    let axles = (ct.n_axles as usize / 2).clamp(2, 3);
    let (r, wb) = if loco { (0.50, 2.0) } else { (0.42, 1.78) };
    let truck = kit.shapes.truck(kit.meshes, axles, r, wb);
    let setback = if loco { 2.8 } else { 1.9 } + if axles == 3 { wb * 0.5 } else { 0.0 };
    for sx in [-1.0f32, 1.0] {
        kit.parts.push((truck.clone(), pal.parts.clone(), Transform::from_xyz(sx * (len * 0.5 - setback), 0.0, 0.0)));
        let rotation = if sx < 0.0 { Quat::from_rotation_y(PI) } else { Quat::IDENTITY };
        kit.parts.push((kit.shapes.coupler.clone(), pal.parts.clone(), Transform { translation: Vec3::new(sx * (len * 0.5 - 0.05), 0.88, 0.0), rotation, ..default() }));
    }
    kit.body(&pal.frame, 0.0, floor - 0.06, 0.0, len, 0.12, w * 0.85);
    let body_h = (h - floor).max(0.6);
    let mid = floor + body_h / 2.0;
    let payload_mat = match car.commodity {
        Commodity::Coal => &pal.coal,
        Commodity::Grain => &pal.grain,
        Commodity::Aggregate => &pal.aggregate,
        Commodity::Lumber => &pal.lumber,
        Commodity::Mixed => &pal.mixed,
        Commodity::Oil => &pal.tank,
        Commodity::Empty => &pal.frame,
    }
    .clone();
    let loaded = car.m_payload > 0.0;
    match ct.kind {
        CarKind::Locomotive => {
            let hood_h = 2.5;
            kit.block(cube, &pal.frame, 0.0, 0.85, 0.0, len * 0.26, 0.5, w * 0.7);
            kit.body(&pal.loco, len * 0.14, floor + hood_h / 2.0, 0.0, len * 0.66, hood_h, w * 0.8);
            kit.body(&pal.loco_cab, -len * 0.28, floor + 1.65, 0.0, len * 0.2, 3.3, w);
            kit.body(&pal.loco, -len * 0.44, floor + 0.75, 0.0, len * 0.1, 1.5, w * 0.8);
            for x in [len * 0.40, len * 0.40 - 1.7] {
                kit.parts.push((pal.cylinder.clone(), pal.frame.clone(), Transform { translation: Vec3::new(x, floor + hood_h + 0.06, 0.0), scale: Vec3::new(1.4, 0.12, 1.4), ..default() }));
            }
            kit.block(cube, &pal.frame, len * 0.02, floor + hood_h + 0.17, 0.0, 0.6, 0.35, 0.9);
        }
        CarKind::OpenHopper | CarKind::Gondola => {
            let m = if ct.kind == CarKind::OpenHopper { &pal.hopper } else { &pal.gondola };
            kit.body(m, 0.0, mid, 0.0, len, body_h, w);
            let ribs = kit.shapes.ribs(kit.meshes, len, w, floor + 0.15, floor + body_h - 0.1);
            kit.parts.push((ribs, m.clone(), Transform::IDENTITY));
            let inner_top = floor + body_h * 0.9;
            kit.block(cube, &pal.frame, 0.0, (floor + inner_top) / 2.0 + 0.1, 0.0, len * 0.94, inner_top - floor - 0.2, w * 0.86);
            if loaded {
                let rim = if ct.kind == CarKind::OpenHopper { floor + body_h } else { inner_top };
                kit.heap(&payload_mat, rim, len * 0.9, w * 0.84, 0.6);
            }
            kit.end_caps(&pal.frame, len, mid, body_h, w);
        }
        CarKind::CoveredHopper => {
            kit.body(&pal.covered, 0.0, mid, 0.0, len, body_h, w);
            kit.body(&pal.covered, 0.0, floor + body_h + 0.2, 0.0, len * 0.95, 0.4, w * 0.45);
            for x in [-len * 0.3, 0.0, len * 0.3] {
                kit.block(cube, &pal.frame, x, floor + body_h + 0.4 + 0.12, 0.0, 1.6, 0.24, 1.4);
            }
            kit.end_caps(&pal.frame, len, mid, body_h, w);
        }
        CarKind::Tank => {
            let r = w * 0.45;
            kit.parts.push((pal.cylinder.clone(), pal.tank.clone(), Transform { translation: Vec3::new(0.0, floor + r, 0.0), rotation: Quat::from_rotation_z(FRAC_PI_2), scale: Vec3::new(2.0 * r, len * 0.96, 2.0 * r), ..default() }));
            for sx in [-1.0f32, 1.0] {
                kit.block(cube, &pal.frame, sx * len * 0.3, floor + 0.3, 0.0, 1.2, 0.6, w * 0.9);
            }
            kit.block(cube, &pal.tank, 0.0, floor + 2.0 * r + 0.15, 0.0, 1.0, 0.4, 1.0);
            kit.block(cube, &pal.frame, 0.0, floor + 2.0 * r + 0.04, 0.0, len * 0.5, 0.08, 0.8);
        }
        CarKind::Boxcar => {
            kit.body(&pal.boxcar, 0.0, mid, 0.0, len, body_h, w);
            for sz in [-1.0f32, 1.0] {
                kit.block(cube, &pal.boxcar, 0.0, floor + body_h * 0.45, sz * (w * 0.5 + 0.02), 2.6, body_h * 0.85, 0.08);
            }
            kit.block(cube, &pal.frame, 0.0, floor + body_h + 0.03, 0.0, len * 0.9, 0.06, 0.6);
            kit.end_caps(&pal.frame, len, mid, body_h, w);
        }
        CarKind::Flatcar => {
            kit.body(&pal.flat, 0.0, floor + 0.15, 0.0, len, 0.3, w);
            if loaded {
                for sx in [-1.0f32, 1.0] {
                    kit.body(&payload_mat, sx * len * 0.24, floor + 0.3 + 0.8, 0.0, len * 0.42, 1.6, w * 0.8);
                }
            }
        }
    }
    for (mesh, material, t) in kit.parts {
        let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(material), t)).id();
        commands.entity(parent).add_child(e);
    }
    commands.entity(parent).add_child(hb);
    parent
}

fn sync_cars(
    mut commands: Commands,
    sim: Res<Sim>,
    pal: Res<Palette>,
    mut shapes: ResMut<Shapes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut ents: ResMut<CarEntities>,
    mut q: Query<(Entity, &CarVisual, &mut Transform)>,
    mut vis: Query<&mut Visibility>,
) {
    let gen = sim.generation;
    for (e, cv, _) in q.iter() {
        if cv.generation != gen {
            commands.entity(e).despawn();
            ents.0.remove(&cv.id);
        }
    }
    for tr in &sim.world.trains {
        for (ci, c) in tr.cars.iter().enumerate() {
            if !ents.0.contains_key(&c.id) {
                let ct = &sim.world.car_types[c.type_id as usize];
                let tf = sim.world.car_pose(tr, ci).map(|p| car_transform(&p, c)).unwrap_or_default();
                let e = spawn_car(&mut commands, &pal, &mut shapes, &mut meshes, ct, c, tf, gen);
                ents.0.insert(c.id, e);
            }
        }
    }
    for (_, cv, mut tf) in q.iter_mut() {
        if cv.generation != gen {
            continue;
        }
        let Some(&(ti, ci)) = sim.car_index.get(&cv.id) else { continue };
        let tr = &sim.world.trains[ti];
        let car = &tr.cars[ci];
        if let Some(p) = sim.world.car_pose(tr, ci) {
            *tf = car_transform(&p, car);
        }
        if let Ok(mut v) = vis.get_mut(cv.hb_marker) {
            let want = if car.hand_brake > 0.5 { Visibility::Inherited } else { Visibility::Hidden };
            if *v != want {
                *v = want;
            }
        }
    }
}

fn overlays(mut gz: Gizmos, sim: Res<Sim>, sel: Res<Selection>) {
    let g = &sim.world.graph;
    for (ni, node) in g.nodes.iter().enumerate() {
        if let NodeKind::Turnout { toe, normal, diverging, setting } = &node.kind {
            let (set, unset) = if *setting == Route::Normal { (*normal, *diverging) } else { (*diverging, *normal) };
            draw_leg(&mut gz, g, set, ni as NodeId, Color::srgb(0.3, 1.0, 0.4));
            draw_leg(&mut gz, g, unset, ni as NodeId, Color::srgb(1.0, 0.25, 0.2));
            draw_leg(&mut gz, g, *toe, ni as NodeId, Color::srgb(0.9, 0.9, 0.9));
        }
    }
    match sel.sel {
        Sel::Car(id) => {
            if let Some(&(ti, ci)) = sim.car_index.get(&id) {
                let tr = &sim.world.trains[ti];
                if let Some(p) = sim.world.car_pose(tr, ci) {
                    let ct = &sim.world.car_types[tr.cars[ci].type_id as usize];
                    rect(&mut gz, &p, ct.length as f32 / 2.0 + 0.6, ct.width as f32 / 2.0 + 0.9, Color::WHITE);
                }
            }
        }
        Sel::Coupler(f, r) => {
            if let Some((tid, k)) = sim.resolve_coupler(f, r) {
                if let Some(tr) = sim.world.train(tid) {
                    if let Some(p) = sim.world.coupler_pose(tr, k) {
                        diamond(&mut gz, pose_to_world(&p), 2.2, Color::srgb(1.0, 1.0, 0.3));
                    }
                }
            }
        }
        Sel::None => {}
    }
}

fn draw_leg(gz: &mut Gizmos, g: &TrackGraph, edge: EdgeId, node: NodeId, color: Color) {
    let e = g.edge(edge);
    let from_a = e.a == node;
    let steps = 8;
    let span = e.length.min(14.0);
    for i in 0..steps {
        let (s0, s1) = (span * i as f64 / steps as f64, span * (i + 1) as f64 / steps as f64);
        let (a, b) = if from_a { (s0, s1) } else { (e.length - s0, e.length - s1) };
        gz.line(pose_to_world(&g.pose_on_edge(edge, a)) + Vec3::Y * 0.7, pose_to_world(&g.pose_on_edge(edge, b)) + Vec3::Y * 0.7, color);
    }
}

fn rect(gz: &mut Gizmos, p: &Pose, half_l: f32, half_w: f32, color: Color) {
    let c = pose_to_world(p) + Vec3::Y * 0.4;
    let f = forward_of(p) * half_l;
    let l = left_of(p) * half_w;
    let pts = [c + f + l, c + f - l, c - f - l, c - f + l];
    for i in 0..4 {
        gz.line(pts[i], pts[(i + 1) % 4], color);
    }
}

fn diamond(gz: &mut Gizmos, c: Vec3, r: f32, color: Color) {
    let c = c + Vec3::Y * 0.6;
    let pts = [c + Vec3::X * r, c + Vec3::Z * r, c - Vec3::X * r, c - Vec3::Z * r];
    for i in 0..4 {
        gz.line(pts[i], pts[(i + 1) % 4], color);
    }
}
