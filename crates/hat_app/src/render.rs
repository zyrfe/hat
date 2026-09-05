//! Procedural track and rolling-stock meshes, car entity sync, gizmo overlays.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI};

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use hat_sim::*;

use crate::camera::{pose_to_world, sim_to_world};
use crate::input::{Sel, Selection};
use crate::sim::Sim;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CarEntities>().add_systems(Startup, setup).add_systems(Update, (sync_cars, overlays));
    }
}

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
    commands.insert_resource(GlobalAmbientLight { color: Color::WHITE, brightness: 320.0, ..default() });
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let cylinder = meshes.add(Cylinder::new(0.5, 1.0));
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
    };

    let ground = meshes.add(Plane3d::default().mesh().size(9000.0, 3000.0));
    let ground_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.21, 0.25, 0.16), perceptual_roughness: 1.0, ..default() });
    commands.spawn((Mesh3d(ground), MeshMaterial3d(ground_mat), Transform::from_xyz(0.0, -0.05, -20.0)));

    let ballast_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.40, 0.37), perceptual_roughness: 1.0, cull_mode: None, ..default() });
    let tie_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.30, 0.22, 0.16), perceptual_roughness: 1.0, cull_mode: None, ..default() });
    let rail_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.60, 0.60, 0.62), perceptual_roughness: 0.45, metallic: 0.6, cull_mode: None, ..default() });
    for (ei, _edge) in sim.world.graph.edges.iter().enumerate() {
        let poses = edge_samples(&sim.world.graph, ei as EdgeId);
        let (bv, bi) = strip(&poses, 0.0, 2.2, 0.06);
        commands.spawn((Mesh3d(meshes.add(mesh_from(bv, bi))), MeshMaterial3d(ballast_mat.clone())));
        let mut rv = Vec::new();
        let mut ri = Vec::new();
        for off in [-0.72f32, 0.72] {
            let (v, i) = strip(&poses, off, 0.08, 0.36);
            append(&mut rv, &mut ri, v, i);
        }
        commands.spawn((Mesh3d(meshes.add(mesh_from(rv, ri))), MeshMaterial3d(rail_mat.clone())));
        let (tv, ti) = ties(&sim.world.graph, ei as EdgeId);
        commands.spawn((Mesh3d(meshes.add(mesh_from(tv, ti))), MeshMaterial3d(tie_mat.clone())));
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
                    Mesh3d(cube.clone()),
                    MeshMaterial3d(palette.bumper.clone()),
                    Transform { translation: sim_to_world(node.pos) + Vec3::Y * (0.8 + node.z as f32), rotation: Quat::from_rotation_y(heading), scale: Vec3::new(1.2, 1.5, 3.2) },
                ));
            }
            NodeKind::Turnout { .. } => {
                commands.spawn((
                    Mesh3d(cylinder.clone()),
                    MeshMaterial3d(palette.switch.clone()),
                    Transform { translation: sim_to_world(node.pos) + Vec3::Y * (0.15 + node.z as f32), scale: Vec3::new(3.2, 0.3, 3.2), ..default() },
                ));
            }
            NodeKind::Plain => {}
        }
    }
    commands.insert_resource(palette);
}

fn mesh_from(verts: Vec<[f32; 3]>, idx: Vec<u32>) -> Mesh {
    let n = verts.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, verts)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; n])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; n])
        .with_inserted_indices(Indices::U32(idx))
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

fn strip(poses: &[Pose], offset: f32, half_w: f32, y: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut v = Vec::with_capacity(poses.len() * 2);
    let mut idx = Vec::with_capacity(poses.len() * 6);
    for p in poses {
        let l = left_of(p);
        let c = pose_to_world(p) + l * offset + Vec3::Y * y;
        v.push((c + l * half_w).to_array());
        v.push((c - l * half_w).to_array());
    }
    for i in 0..(poses.len() as u32 - 1) {
        let a = 2 * i;
        idx.extend_from_slice(&[a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
    (v, idx)
}

fn ties(g: &TrackGraph, id: EdgeId) -> (Vec<[f32; 3]>, Vec<u32>) {
    let e = g.edge(id);
    let step = 0.6;
    let n = (e.length / step).floor() as usize;
    let mut v = Vec::with_capacity(n * 4);
    let mut idx = Vec::with_capacity(n * 6);
    for i in 0..n {
        let s = (i as f64 + 0.5) * step;
        let p = g.pose_on_edge(id, s);
        let c = pose_to_world(&p) + Vec3::Y * 0.2;
        let l = left_of(&p) * 1.3;
        let f = forward_of(&p) * 0.12;
        let base = v.len() as u32;
        v.push((c + l - f).to_array());
        v.push((c - l - f).to_array());
        v.push((c + l + f).to_array());
        v.push((c - l + f).to_array());
        idx.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
    }
    (v, idx)
}

fn append(rv: &mut Vec<[f32; 3]>, ri: &mut Vec<u32>, v: Vec<[f32; 3]>, i: Vec<u32>) {
    let base = rv.len() as u32;
    rv.extend(v);
    ri.extend(i.into_iter().map(|x| x + base));
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

fn bx(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> Transform {
    Transform { translation: Vec3::new(x, y, z), scale: Vec3::new(sx, sy, sz), ..default() }
}

fn spawn_car(commands: &mut Commands, pal: &Palette, ct: &CarType, car: &CarState, tf: Transform, generation: u32) -> Entity {
    let len = (ct.length - 1.0) as f32;
    let w = ct.width as f32;
    let h = ct.height as f32;
    let hb = commands
        .spawn((Mesh3d(pal.cube.clone()), MeshMaterial3d(pal.hand_brake.clone()), bx(-len * 0.38, h + 0.5, 0.0, 0.9, 0.9, 0.9), Visibility::Hidden))
        .id();
    let parent = commands.spawn((tf, Visibility::default(), CarVisual { id: car.id, hb_marker: hb, generation })).id();
    let cube = &pal.cube;
    let mut parts: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)> = Vec::new();
    parts.push((cube.clone(), pal.frame.clone(), bx(0.0, 0.55, 0.0, len, 0.5, w * 0.85)));
    let body_h = (h - 0.8).max(0.6);
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
    match ct.kind {
        CarKind::Locomotive => {
            parts.push((cube.clone(), pal.loco.clone(), bx(len * 0.12, 0.8 + 1.45, 0.0, len * 0.72, 2.9, w * 0.88)));
            parts.push((cube.clone(), pal.loco_cab.clone(), bx(-len * 0.31, 0.8 + 1.85, 0.0, len * 0.20, 3.7, w)));
            parts.push((cube.clone(), pal.loco.clone(), bx(-len * 0.31, 0.8 + 3.78, 0.0, len * 0.22, 0.16, w * 1.03)));
        }
        CarKind::OpenHopper | CarKind::Gondola => {
            let m = if ct.kind == CarKind::OpenHopper { &pal.hopper } else { &pal.gondola };
            parts.push((cube.clone(), m.clone(), bx(0.0, 0.8 + body_h / 2.0, 0.0, len, body_h, w)));
            if car.m_payload > 0.0 {
                parts.push((cube.clone(), payload_mat.clone(), bx(0.0, 0.8 + body_h + 0.12, 0.0, len * 0.92, 0.35, w * 0.8)));
            }
        }
        CarKind::CoveredHopper => {
            parts.push((cube.clone(), pal.covered.clone(), bx(0.0, 0.8 + body_h / 2.0, 0.0, len, body_h, w)));
            parts.push((cube.clone(), pal.covered.clone(), bx(0.0, 0.8 + body_h + 0.15, 0.0, len * 0.95, 0.4, w * 0.45)));
        }
        CarKind::Tank => {
            parts.push((
                pal.cylinder.clone(),
                pal.tank.clone(),
                Transform { translation: Vec3::new(0.0, 0.8 + w * 0.45, 0.0), rotation: Quat::from_rotation_z(FRAC_PI_2), scale: Vec3::new(w * 0.9, len * 0.96, w * 0.9) },
            ));
            parts.push((cube.clone(), pal.tank.clone(), bx(0.0, 0.8 + w * 0.9 + 0.15, 0.0, 1.0, 0.4, 1.0)));
        }
        CarKind::Boxcar => {
            parts.push((cube.clone(), pal.boxcar.clone(), bx(0.0, 0.8 + body_h / 2.0, 0.0, len, body_h, w)));
        }
        CarKind::Flatcar => {
            parts.push((cube.clone(), pal.flat.clone(), bx(0.0, 0.95, 0.0, len, 0.3, w)));
            if car.m_payload > 0.0 {
                for sx in [-1.0f32, 1.0] {
                    parts.push((cube.clone(), payload_mat.clone(), bx(sx * len * 0.24, 1.1 + 0.8, 0.0, len * 0.42, 1.6, w * 0.8)));
                }
            }
        }
    }
    for (mesh, material, t) in parts {
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
                let e = spawn_car(&mut commands, &pal, ct, c, tf, gen);
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
                        diamond(&mut gz, sim_to_world(p.pos), 2.2, Color::srgb(1.0, 1.0, 0.3));
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
