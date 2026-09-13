//! Keyboard and mouse: train controls, crew actions, picking.

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PrimaryWindow, WindowMode};
use hat_sim::params::{FULL_SERVICE_PIPE, PIPE_REF};
use hat_sim::*;
use hat_units::UnitSystem;

use crate::camera::{cursor_ground, world_to_sim, MainCamera, Rig};
use crate::sim::Sim;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Selection>().init_resource::<UiState>().add_systems(Update, (keyboard, mouse_pick));
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Sel {
    #[default]
    None,
    Car(CarId),
    /// Front car id, rear car id.
    Coupler(CarId, CarId),
}

#[derive(Resource, Default)]
pub struct Selection {
    pub sel: Sel,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub pointer_over_ui: bool,
    pub show_help: bool,
    pub fps: f32,
    pub units: UnitSystem,
    /// Track picked in each train's order editor, by locomotive car.
    pub order_track: std::collections::HashMap<CarId, u32>,
}

fn keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim: ResMut<Sim>,
    mut sel: ResMut<Selection>,
    mut ui: ResMut<UiState>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    mut exit: MessageWriter<AppExit>,
) {
    if keys.just_pressed(KeyCode::KeyM) {
        sim.toggle_manual();
    }
    // The emergency handle always works: it takes the engine from the crew on the way.
    if keys.just_pressed(KeyCode::Backspace) && !sim.is_manual() {
        sim.set_manual(true);
    }
    // Drive keys belong to the player only in Manual; otherwise WASD pans the map (camera.rs).
    let manual = sim.is_manual();
    if !manual {
        drive_free_keys(&keys, &mut sim, &mut sel, &mut ui, &mut window, &mut exit);
        return;
    }
    if keys.just_pressed(KeyCode::KeyW) {
        sim.controls.throttle = (sim.controls.throttle + 1).min(8);
    }
    if keys.just_pressed(KeyCode::KeyS) {
        sim.controls.throttle = sim.controls.throttle.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::KeyD) {
        sim.controls.reverser = match sim.controls.reverser {
            Reverser::Reverse => Reverser::Neutral,
            _ => Reverser::Forward,
        };
    }
    if keys.just_pressed(KeyCode::KeyA) {
        sim.controls.reverser = match sim.controls.reverser {
            Reverser::Forward => Reverser::Neutral,
            _ => Reverser::Reverse,
        };
    }
    if keys.just_pressed(KeyCode::KeyX) {
        sim.controls.independent = (sim.controls.independent + 0.25).min(1.0);
    }
    if keys.just_pressed(KeyCode::KeyZ) {
        sim.controls.independent = (sim.controls.independent - 0.25).max(0.0);
    }
    if keys.just_pressed(KeyCode::KeyV) {
        sim.controls.auto_target = (sim.controls.auto_target - 30_000.0).max(FULL_SERVICE_PIPE);
        sim.controls.emergency = false;
    }
    if keys.just_pressed(KeyCode::KeyC) {
        sim.controls.auto_target = PIPE_REF;
        sim.controls.emergency = false;
    }
    if keys.just_pressed(KeyCode::Backspace) {
        sim.controls.emergency = true;
        sim.controls.throttle = 0;
        sim.say("EMERGENCY: pipe dumped");
    }
    drive_free_keys(&keys, &mut sim, &mut sel, &mut ui, &mut window, &mut exit);
}

/// Keys that mean the same thing whether the player or the crew is driving.
fn drive_free_keys(keys: &ButtonInput<KeyCode>, sim: &mut Sim, sel: &mut Selection, ui: &mut UiState, window: &mut Window, exit: &mut MessageWriter<AppExit>) {
    let s = sel.sel;
    if keys.just_pressed(KeyCode::Space) {
        sim.action_pull_pin(s);
    }
    if keys.just_pressed(KeyCode::KeyH) {
        sim.action_hand_brake(s);
    }
    if keys.just_pressed(KeyCode::KeyL) {
        sim.action_bleed(s);
    }
    if keys.just_pressed(KeyCode::KeyK) {
        sim.action_connect_hoses(s);
    }
    if keys.just_pressed(KeyCode::KeyJ) {
        sim.action_bottle_air(s);
    }

    if keys.just_pressed(KeyCode::KeyP) {
        sim.time_scale = if sim.time_scale == 0 { 1 } else { 0 };
    }
    if keys.just_pressed(KeyCode::Digit1) {
        sim.time_scale = 1;
    }
    if keys.just_pressed(KeyCode::Digit2) {
        sim.time_scale = 4;
    }
    if keys.just_pressed(KeyCode::Digit3) {
        sim.time_scale = 10;
    }
    if keys.just_pressed(KeyCode::Digit4) {
        sim.time_scale = 30;
    }
    if keys.just_pressed(KeyCode::Digit5) {
        sim.time_scale = 60;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        sim.action_rerail(s);
    }

    if keys.just_pressed(KeyCode::Escape) {
        sel.sel = Sel::None;
    }
    if keys.just_pressed(KeyCode::F1) {
        ui.show_help = !ui.show_help;
    }
    if keys.just_pressed(KeyCode::KeyU) {
        ui.units = ui.units.toggle();
    }
    if keys.just_pressed(KeyCode::F11) {
        window.mode = match window.mode {
            WindowMode::Windowed => WindowMode::BorderlessFullscreen(MonitorSelection::Primary),
            _ => WindowMode::Windowed,
        };
    }
    let modifier = keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight) || keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if modifier && keys.just_pressed(KeyCode::KeyQ) {
        exit.write(AppExit::Success);
    }
}

#[derive(Clone, Copy, Debug)]
enum Pick {
    Switch(NodeId),
    Car(CarId),
    Coupler(CarId, CarId),
}

fn mouse_pick(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    ui: Res<UiState>,
    window: Single<&Window, With<PrimaryWindow>>,
    cam: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    hf: Option<Res<crate::terrain::Heightfield>>,
    rig: Res<Rig>,
    mut sim: ResMut<Sim>,
    mut sel: ResMut<Selection>,
) {
    // A click is a press and release without dragging; drags pan the map instead.
    if !buttons.just_released(MouseButton::Left) || rig.drag_moved || rig.press_over_ui || ui.pointer_over_ui || time.elapsed_secs() < 0.5 {
        return;
    }
    let (camera, tf) = *cam;
    let Some(hit) = cursor_ground(&window, camera, tf, hf.as_deref()) else { return };
    let p = world_to_sim(hit);
    let scale = (rig.distance as f64 / 300.0).max(1.0);
    let mut best: Option<(f64, Pick)> = None;
    let mut consider = |d: f64, pick: Pick| {
        if d < 1.0 && best.map_or(true, |b| d < b.0) {
            best = Some((d, pick));
        }
    };
    for (ni, node) in sim.world.graph.nodes.iter().enumerate() {
        if matches!(node.kind, NodeKind::Turnout { .. }) {
            consider(node.pos.distance(p) / (6.0 * scale), Pick::Switch(ni as NodeId));
        }
    }
    let types = &sim.world.car_types;
    for tr in &sim.world.trains {
        for (ci, c) in tr.cars.iter().enumerate() {
            if let Some(pose) = sim.world.car_pose(tr, ci) {
                let ct = &types[c.type_id as usize];
                let rel = p - pose.pos;
                let dir = pose.dir();
                let along = rel.dot(dir);
                let perp = rel.perp_dot(dir).abs();
                if along.abs() <= ct.length / 2.0 && perp <= 3.5 * scale {
                    consider(0.9 * perp / (3.5 * scale), Pick::Car(c.id));
                }
            }
        }
        for k in 1..tr.cars.len() {
            if let Some(pose) = sim.world.coupler_pose(tr, k) {
                consider(0.7 * pose.pos.distance(p) / (3.0 * scale), Pick::Coupler(tr.cars[k - 1].id, tr.cars[k].id));
            }
        }
    }
    match best.map(|b| b.1) {
        Some(Pick::Switch(n)) => sim.action_throw_switch(n),
        Some(Pick::Car(id)) => {
            sel.sel = Sel::Car(id);
            let is_loco = sim.world.train_of_car(id).map(|(ti, ci)| sim.world.car_types[sim.world.trains[ti].cars[ci].type_id as usize].is_loco()).unwrap_or(false);
            if is_loco {
                sim.take_loco(id);
            }
        }
        Some(Pick::Coupler(a, b)) => sel.sel = Sel::Coupler(a, b),
        None => sel.sel = Sel::None,
    }
}
