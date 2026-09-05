//! Overhead camera rig with pan, zoom, orbit and follow, plus ground picking.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy_egui::{EguiGlobalSettings, PrimaryEguiContext};

use crate::input::UiState;
use crate::sim::Sim;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Rig>().add_systems(Startup, setup).add_systems(Update, control);
    }
}

#[derive(Resource)]
pub struct Rig {
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub follow: bool,
}

impl Default for Rig {
    fn default() -> Self {
        Rig { focus: Vec3::new(60.0, 0.0, -14.0), distance: 260.0, yaw: 0.0, pitch: 60f32.to_radians(), follow: false }
    }
}

impl Rig {
    pub fn transform(&self) -> Transform {
        let dir = Vec3::new(self.yaw.sin() * self.pitch.cos(), self.pitch.sin(), self.yaw.cos() * self.pitch.cos());
        Transform::from_translation(self.focus + dir * self.distance).looking_at(self.focus, Vec3::Y)
    }
}

#[derive(Component)]
pub struct MainCamera;

/// Sim plane (x east, y north) to world (x east, z south, y up).
pub fn sim_to_world(p: glam::DVec2) -> Vec3 {
    Vec3::new(p.x as f32, 0.0, -(p.y as f32))
}

pub fn world_to_sim(p: Vec3) -> glam::DVec2 {
    glam::DVec2::new(p.x as f64, -(p.z as f64))
}

/// Point on the ground plane under the cursor.
pub fn cursor_ground(window: &Window, cam: &Camera, tf: &GlobalTransform) -> Option<Vec3> {
    let cursor = window.cursor_position()?;
    let ray = cam.viewport_to_world(tf, cursor).ok()?;
    let d = ray.direction.as_vec3();
    if d.y.abs() < 1e-6 {
        return None;
    }
    let t = -ray.origin.y / d.y;
    if t < 0.0 {
        return None;
    }
    Some(ray.origin + d * t)
}

fn setup(mut commands: Commands, rig: Res<Rig>, mut egui_settings: ResMut<EguiGlobalSettings>) {
    egui_settings.auto_create_primary_context = false;
    commands.spawn((
        Camera3d::default(),
        PrimaryEguiContext,
        Projection::Perspective(PerspectiveProjection { fov: 45f32.to_radians(), near: 0.5, far: 30_000.0, ..default() }),
        rig.transform(),
        MainCamera,
    ));
    commands.spawn((
        DirectionalLight { illuminance: 9_000.0, shadow_maps_enabled: false, ..default() },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 0.7, -1.05, 0.0)),
    ));
}

fn control(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
    ui: Res<UiState>,
    sim: Res<Sim>,
    mut rig: ResMut<Rig>,
    mut cam: Single<&mut Transform, With<MainCamera>>,
) {
    let dt = time.delta_secs();
    let pan = rig.distance * 0.9 * dt;
    let (sy, cy) = rig.yaw.sin_cos();
    let fwd = Vec3::new(-sy, 0.0, -cy);
    let right = Vec3::new(cy, 0.0, -sy);
    let mut moved = false;
    if keys.pressed(KeyCode::ArrowUp) {
        rig.focus += fwd * pan;
        moved = true;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        rig.focus -= fwd * pan;
        moved = true;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        rig.focus += right * pan;
        moved = true;
    }
    if keys.pressed(KeyCode::ArrowLeft) {
        rig.focus -= right * pan;
        moved = true;
    }
    if !ui.pointer_over_ui {
        if scroll.delta.y != 0.0 {
            let factor = match scroll.unit {
                MouseScrollUnit::Line => 0.88f32.powf(scroll.delta.y),
                MouseScrollUnit::Pixel => 0.997f32.powf(scroll.delta.y),
            };
            rig.distance = (rig.distance * factor).clamp(12.0, 7000.0);
        }
        if buttons.pressed(MouseButton::Right) {
            rig.yaw -= motion.delta.x * 0.005;
            rig.pitch = (rig.pitch + motion.delta.y * 0.005).clamp(0.25, 1.52);
        }
        if buttons.pressed(MouseButton::Middle) {
            let s = rig.distance * 0.0016;
            rig.focus += (-right * motion.delta.x + fwd * motion.delta.y) * s;
            moved = true;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) {
        rig.follow = !rig.follow;
    }
    if keys.just_pressed(KeyCode::Home) {
        *rig = Rig::default();
    }
    if moved {
        rig.follow = false;
    }
    if rig.follow {
        if let Some((ti, ci)) = sim.loco_pos() {
            let tr = &sim.world.trains[ti];
            if let Some(p) = sim.world.car_pose(tr, ci) {
                rig.focus = sim_to_world(p.pos);
            }
        }
    }
    rig.focus.y = 0.0;
    **cam = rig.transform();
}
