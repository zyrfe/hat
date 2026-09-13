//! The cab window: a live picture-in-picture view that follows the locomotive, a control
//! stand drawn as levers you drag, gauges, a slack strip along the train, and the engineer.

use std::f32::consts::PI;

use bevy::camera::{ClearColorConfig, RenderTarget};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy_egui::{egui, EguiTextureHandle, EguiUserTextures};
use egui::{Color32, Stroke, StrokeKind};
use hat_sim::params::{BRAKE_RATIO, FULL_SERVICE_CYL, FULL_SERVICE_PIPE, PIPE_REF};
use hat_sim::*;
use hat_units::fmt as uf;
use hat_units::{UnitSystem, G};

use crate::camera::pose_to_world;
use crate::sim::{Engineer, Mood, Sim};

pub const CAB_W: u32 = 512;
pub const CAB_H: u32 = 288;

pub struct CabPlugin;

impl Plugin for CabPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(Update, follow);
    }
}

#[derive(Resource)]
pub struct CabView {
    /// Kept so the render target outlives any reload.
    pub _image: Handle<Image>,
    pub tex: egui::TextureId,
}

impl CabView {
    pub fn image(&self) -> Handle<Image> {
        self._image.clone()
    }
}

#[derive(Component)]
pub struct CabCamera;

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut textures: ResMut<EguiUserTextures>) {
    let image = images.add(Image::new_target_texture(CAB_W, CAB_H, TextureFormat::Rgba8UnormSrgb, None));
    let tex = textures.add_image(EguiTextureHandle::Strong(image.clone()));
    commands.spawn((
        Camera3d::default(),
        Camera { order: -1, clear_color: ClearColorConfig::Custom(crate::light::SKY), ..default() },
        RenderTarget::from(image.clone()),
        Projection::Perspective(PerspectiveProjection { fov: 50f32.to_radians(), near: 0.3, far: 6000.0, ..default() }),
        Transform::from_xyz(0.0, 20.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        CabCamera,
    ));
    commands.insert_resource(CabView { _image: image, tex });
}

/// Chase view over the locomotive's front, smoothed.
fn follow(sim: Res<Sim>, time: Res<Time>, mut q: Query<&mut Transform, With<CabCamera>>) {
    let Some((ti, ci)) = sim.loco_pos() else { return };
    let tr = &sim.world.trains[ti];
    let car = &tr.cars[ci];
    let Some(pose) = sim.world.car_pose(tr, ci) else { return };
    let mut heading = pose.heading as f32;
    if !car.facing_head {
        heading += PI;
    }
    let dir = Vec3::new(heading.cos(), 0.0, -heading.sin());
    let loco = pose_to_world(&pose);
    let target_pos = loco - dir * 38.0 + Vec3::Y * 15.0;
    let target_look = loco + dir * 20.0 + Vec3::Y * 2.0;
    let k = 1.0 - (-time.delta_secs() * 6.0).exp();
    for mut t in &mut q {
        let pos = t.translation.lerp(target_pos, k);
        *t = Transform::from_translation(pos).looking_at(target_look, Vec3::Y);
    }
}

pub fn mood_color(m: Mood) -> Color32 {
    match m {
        Mood::Calm => Color32::from_gray(120),
        Mood::Focused => Color32::from_rgb(110, 150, 200),
        Mood::Pleased => Color32::from_rgb(110, 200, 120),
        Mood::Worried => Color32::from_rgb(240, 190, 60),
        Mood::Alarmed => Color32::from_rgb(250, 130, 50),
        Mood::Panic => Color32::from_rgb(240, 60, 50),
    }
}

pub fn mood_word(m: Mood) -> &'static str {
    match m {
        Mood::Calm => "calm",
        Mood::Focused => "focused",
        Mood::Pleased => "pleased",
        Mood::Worried => "worried",
        Mood::Alarmed => "alarmed",
        Mood::Panic => "panicking",
    }
}

pub fn cab_window(ctx: &egui::Context, sim: &mut Sim, view: &CabView, u: UnitSystem) {
    let screen = ctx.content_rect();
    egui::Window::new("Cab")
        .default_pos(egui::pos2(screen.left() + 410.0, screen.bottom() - 470.0))
        .resizable(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new(egui::load::SizedTexture::new(view.tex, egui::vec2(CAB_W as f32 * 0.9, CAB_H as f32 * 0.9))));
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        portrait(ui, &sim.engineer, sim.world.t);
                        ui.vertical(|ui| {
                            ui.strong(format!("Engineer {}", sim.engineer.name));
                            ui.colored_label(mood_color(sim.engineer.mood), mood_word(sim.engineer.mood));
                            ui.label(egui::RichText::new(format!("\u{201c}{}\u{201d}", sim.engineer.line)).italics());
                        });
                    });
                    gauges(ui, sim, u);
                    levers(ui, sim);
                    if !sim.is_manual() {
                        ui.small(egui::RichText::new("Crew has the levers. Drag one or press M to take them.").color(egui::Color32::from_rgb(110, 200, 120)));
                    }
                });
            });
            ui.separator();
            readouts(ui, sim, u);
            slack_strip(ui, sim);
        });
}

fn gauges(ui: &mut egui::Ui, sim: &Sim, u: UnitSystem) {
    let v = sim.loco_speed().abs();
    let (speed_val, speed_max, unit) = match u {
        UnitSystem::Metric => (v * 3.6, 100.0, "km/h"),
        UnitSystem::UsCustomary => (v / hat_units::consts::MPH, 60.0, "mph"),
    };
    let conv = |mps: f64| match u {
        UnitSystem::Metric => mps * 3.6,
        UnitSystem::UsCustomary => mps / hat_units::consts::MPH,
    };
    let red_from = sim.cur_limit.map(|l| conv(l) as f32);
    let amber = match sim.ahead {
        Some(Ahead::Restriction { limit, .. }) => Some(conv(limit) as f32),
        _ => None,
    };
    let (pipe, cyl) = sim.loco_car().map(|c| (c.brake.p_pipe, c.brake.p_cyl)).unwrap_or((0.0, 0.0));
    let (p_scale, p_unit) = match u {
        UnitSystem::Metric => (1.0 / 1000.0, "kPa"),
        UnitSystem::UsCustomary => (1.0 / hat_units::consts::PSI, "psi"),
    };
    let p_max = match u {
        UnitSystem::Metric => 800.0,
        UnitSystem::UsCustomary => 120.0,
    };
    ui.horizontal(|ui| {
        gauge(ui, "SPEED", speed_val as f32, speed_max, red_from, amber, None, format!("{:.0} {unit}", speed_val));
        gauge(
            ui,
            "AIR",
            (pipe * p_scale) as f32,
            p_max,
            None,
            None,
            Some(((cyl * p_scale) as f32, Color32::from_rgb(240, 90, 70))),
            format!("{:.0}/{:.0} {p_unit}", pipe * p_scale, cyl * p_scale),
        );
        force_bars(ui, sim, u);
    });
}

fn gauge(ui: &mut egui::Ui, label: &str, value: f32, max: f32, red_from: Option<f32>, amber_at: Option<f32>, needle2: Option<(f32, Color32)>, text: String) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(104.0, 92.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, Color32::from_gray(28));
    let c = egui::pos2(rect.center().x, rect.top() + 54.0);
    let r = 38.0;
    let a0 = 225f32.to_radians();
    let a1 = -45f32.to_radians();
    let ang = |v: f32| a0 - (v / max).clamp(0.0, 1.0) * (a0 - a1);
    let pt = |a: f32, rr: f32| egui::pos2(c.x + a.cos() * rr, c.y - a.sin() * rr);
    let arc: Vec<egui::Pos2> = (0..=40).map(|i| pt(a0 - (a0 - a1) * i as f32 / 40.0, r)).collect();
    p.add(egui::Shape::line(arc, Stroke::new(2.0, Color32::from_gray(150))));
    if let Some(rf) = red_from {
        let s = ang(rf);
        let red: Vec<egui::Pos2> = (0..=20).map(|i| pt(s - (s - a1) * i as f32 / 20.0, r)).collect();
        p.add(egui::Shape::line(red, Stroke::new(4.0, Color32::from_rgb(220, 60, 50))));
    }
    if let Some(am) = amber_at {
        let a = ang(am);
        p.line_segment([pt(a, r - 7.0), pt(a, r + 5.0)], Stroke::new(2.5, Color32::from_rgb(250, 190, 60)));
    }
    for i in 0..=10 {
        let a = ang(max * i as f32 / 10.0);
        p.line_segment([pt(a, r - 5.0), pt(a, r)], Stroke::new(1.0, Color32::from_gray(190)));
    }
    if let Some((v2, col)) = needle2 {
        let a = ang(v2);
        p.line_segment([c, pt(a, r - 8.0)], Stroke::new(2.0, col));
    }
    let a = ang(value);
    p.line_segment([c, pt(a, r - 6.0)], Stroke::new(2.5, Color32::WHITE));
    p.circle_filled(c, 3.0, Color32::WHITE);
    p.text(egui::pos2(c.x, rect.bottom() - 9.0), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(11.0), Color32::WHITE);
    p.text(egui::pos2(c.x, rect.top() + 9.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), Color32::LIGHT_GRAY);
}

fn force_bars(ui: &mut egui::Ui, sim: &Sim, u: UnitSystem) {
    let (te, brake) = sim
        .loco_car()
        .map(|c| {
            let ct = &sim.world.car_types[c.type_id as usize];
            let spec = ct.loco.as_ref().unwrap();
            let m = c.mass(ct);
            let ctl = &sim.controls;
            let te = if ctl.reverser != Reverser::Neutral && ctl.throttle > 0 {
                (ctl.throttle.min(8) as f64 / 8.0) * (spec.power_rail / c.v.abs().max(0.5)).min(spec.adhesion * m * G)
            } else {
                0.0
            };
            let air = (ct.m_tare * G * BRAKE_RATIO * (c.brake.p_cyl / FULL_SERVICE_CYL)).min(0.15 * m * G);
            (te, air + ctl.independent.clamp(0.0, 1.0) * spec.independent_max)
        })
        .unwrap_or((0.0, 0.0));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(104.0, 92.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 6.0, Color32::from_gray(28));
    let bar = |y: f32, frac: f64, color: Color32, label: String| {
        let full = egui::Rect::from_min_max(egui::pos2(rect.left() + 8.0, y), egui::pos2(rect.right() - 8.0, y + 14.0));
        p.rect_filled(full, 2.0, Color32::from_gray(12));
        let w = full.width() * frac.clamp(0.0, 1.0) as f32;
        p.rect_filled(egui::Rect::from_min_max(full.min, egui::pos2(full.left() + w, full.bottom())), 2.0, color);
        p.text(egui::pos2(full.center().x, y - 7.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(9.0), Color32::LIGHT_GRAY);
    };
    bar(rect.top() + 22.0, te / 300_000.0, Color32::from_rgb(90, 200, 110), format!("PULL {}", uf::force(te, u)));
    bar(rect.top() + 60.0, brake / 300_000.0, Color32::from_rgb(230, 100, 80), format!("BRAKE {}", uf::force(brake, u)));
}

fn levers(ui: &mut egui::Ui, sim: &mut Sim) {
    let c = sim.controls.clone();
    let mut next = c.clone();
    ui.horizontal(|ui| {
        if let Some(v) = lever(ui, "THR", c.throttle as f32 / 8.0, Some(8), &["0", "", "2", "", "4", "", "6", "", "8"], None) {
            next.throttle = (v * 8.0).round() as u8;
        }
        let rv = match c.reverser {
            Reverser::Reverse => 0.0,
            Reverser::Neutral => 0.5,
            Reverser::Forward => 1.0,
        };
        if let Some(v) = lever(ui, "REV", rv, Some(2), &["R", "N", "F"], None) {
            next.reverser = if v < 0.25 {
                Reverser::Reverse
            } else if v > 0.75 {
                Reverser::Forward
            } else {
                Reverser::Neutral
            };
        }
        if let Some(v) = lever(ui, "IND", c.independent as f32, Some(20), &["REL", "", "FULL"], None) {
            next.independent = v as f64;
        }
        let auto_v = if c.emergency {
            1.0
        } else {
            let red = (PIPE_REF - c.auto_target).clamp(0.0, PIPE_REF - FULL_SERVICE_PIPE);
            if red <= 0.0 { 0.0 } else { 0.12 + 0.76 * (red / (PIPE_REF - FULL_SERVICE_PIPE)) as f32 }
        };
        if let Some(v) = lever(ui, "AUTO", auto_v, None, &["REL", "SERVICE", "EMERG"], Some(0.93)) {
            if v >= 0.93 {
                next.emergency = true;
                next.throttle = 0;
            } else if v < 0.08 {
                next.emergency = false;
                next.auto_target = PIPE_REF;
            } else {
                next.emergency = false;
                let frac = ((v - 0.12) / 0.76).clamp(0.02, 1.0) as f64;
                next.auto_target = PIPE_REF - frac * (PIPE_REF - FULL_SERVICE_PIPE);
            }
        }
    });
    if next != c {
        if next.emergency && !c.emergency {
            sim.say("EMERGENCY: pipe dumped");
        }
        sim.controls = next;
        sim.manual_touch = true;
    }
}

/// A vertical lever. Returns the new 0..1 value while the handle is being clicked or dragged.
fn lever(ui: &mut egui::Ui, label: &str, value: f32, notches: Option<usize>, labels: &[&str], red_above: Option<f32>) -> Option<f32> {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(52.0, 150.0), egui::Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 5.0, Color32::from_gray(28));
    let slot = egui::Rect::from_min_max(egui::pos2(rect.center().x - 4.0, rect.top() + 20.0), egui::pos2(rect.center().x + 4.0, rect.bottom() - 16.0));
    p.rect_filled(slot, 3.0, Color32::from_gray(10));
    let y_of = |v: f32| slot.bottom() - v.clamp(0.0, 1.0) * slot.height();
    if let Some(ra) = red_above {
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(slot.left() - 1.0, y_of(1.0) - 3.0), egui::pos2(slot.right() + 1.0, y_of(ra))), 2.0, Color32::from_rgb(160, 40, 30));
    }
    if let Some(n) = notches {
        for i in 0..=n {
            let y = y_of(i as f32 / n as f32);
            p.line_segment([egui::pos2(slot.right() + 3.0, y), egui::pos2(slot.right() + 8.0, y)], Stroke::new(1.0, Color32::from_gray(120)));
        }
    }
    for (i, l) in labels.iter().enumerate() {
        let v = if labels.len() > 1 { i as f32 / (labels.len() - 1) as f32 } else { 0.0 };
        p.text(egui::pos2(slot.left() - 4.0, y_of(v)), egui::Align2::RIGHT_CENTER, *l, egui::FontId::proportional(9.0), Color32::from_gray(170));
    }
    let hy = y_of(value);
    let handle = egui::Rect::from_center_size(egui::pos2(rect.center().x + 2.0, hy), egui::vec2(28.0, 12.0));
    let active = resp.hovered() || resp.dragged();
    p.rect_filled(handle, 3.0, if active { Color32::from_rgb(245, 205, 90) } else { Color32::from_rgb(200, 160, 60) });
    p.rect_stroke(handle, 3.0, Stroke::new(1.0, Color32::from_gray(20)), StrokeKind::Inside);
    p.text(egui::pos2(rect.center().x, rect.top() + 9.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(10.0), Color32::LIGHT_GRAY);
    if resp.clicked() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let mut v = ((slot.bottom() - pos.y) / slot.height()).clamp(0.0, 1.0);
            if let Some(n) = notches {
                v = (v * n as f32).round() / n as f32;
            }
            return Some(v);
        }
    }
    None
}

fn readouts(ui: &mut egui::Ui, sim: &Sim, u: UnitSystem) {
    ui.horizontal(|ui| {
        if let Some(l) = sim.cur_limit {
            ui.label(format!("Limit {}", uf::speed(l, u)));
        }
        let v = sim.loco_speed().abs();
        match sim.ahead {
            Some(Ahead::Restriction { limit, distance, track }) => {
                let too_fast = v > limit && distance < (v * v - limit * limit) / (2.0 * 0.6) * 1.3;
                let color = if too_fast { Color32::from_rgb(250, 90, 70) } else { Color32::from_rgb(250, 190, 60) };
                let where_ = track.map(|t| sim.yard.track_name(t)).unwrap_or_default();
                ui.colored_label(color, format!("Next: {} in {} ({where_})", uf::speed(limit, u), uf::length(distance, u)));
            }
            Some(Ahead::SplitSwitch { node, distance }) => {
                ui.colored_label(Color32::from_rgb(250, 90, 70), format!("{} set AGAINST us in {}", sim.world.graph.node(node).name, uf::length(distance, u)));
            }
            Some(Ahead::End { distance }) => {
                let color = if distance < 100.0 && v > 1.0 { Color32::from_rgb(250, 90, 70) } else { Color32::LIGHT_GRAY };
                ui.colored_label(color, format!("Bumper in {}", uf::length(distance, u)));
            }
            None => {
                ui.label("Clear ahead");
            }
        }
        if let Some(tr) = sim.loco_train() {
            let types = &sim.world.car_types;
            let (mut buff, mut draft) = (0, 0);
            for z in &tr.zones {
                match z {
                    Zone::Buff => buff += 1,
                    Zone::Draft => draft += 1,
                    Zone::Free => {}
                }
            }
            ui.separator();
            ui.label(format!("{} cars {} {}   slack: {buff} bunched, {draft} stretched", tr.cars.len(), uf::length(tr.length(types), u), uf::mass(tr.total_mass(types), u)));
        }
    });
}

fn slack_strip(ui: &mut egui::Ui, sim: &Sim) {
    let Some((ti, li)) = sim.loco_pos() else { return };
    let tr = &sim.world.trains[ti];
    let n = tr.cars.len();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width().max(200.0), 20.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_gray(22));
    let w = rect.width() / n as f32;
    for (i, car) in tr.cars.iter().enumerate() {
        let x0 = rect.left() + i as f32 * w;
        let color = if i == li {
            Color32::from_rgb(255, 190, 80)
        } else if car.derailed {
            Color32::from_rgb(230, 80, 70)
        } else if car.brake.p_cyl > 30_000.0 {
            Color32::from_rgb(120, 70, 70)
        } else {
            Color32::from_gray(75)
        };
        p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0 + 1.0, rect.top() + 5.0), egui::pos2((x0 + w - 1.0).max(x0 + 1.5), rect.bottom() - 5.0)), 1.0, color);
        if car.hand_brake > 0.5 {
            p.rect_filled(egui::Rect::from_min_max(egui::pos2(x0 + 1.0, rect.top() + 1.0), egui::pos2((x0 + w - 1.0).max(x0 + 1.5), rect.top() + 4.0)), 0.0, Color32::YELLOW);
        }
    }
    for k in 1..n {
        let x = rect.left() + k as f32 * w;
        let (color, h) = match tr.zones[k - 1] {
            Zone::Buff => (Color32::from_rgb(240, 80, 70), 16.0),
            Zone::Free => (Color32::from_gray(140), 8.0),
            Zone::Draft => (Color32::from_rgb(90, 220, 110), 16.0),
        };
        p.line_segment([egui::pos2(x, rect.center().y - h / 2.0), egui::pos2(x, rect.center().y + h / 2.0)], Stroke::new(2.0, color));
    }
    ui.small("Head at left. Couplers: red bunched, green stretched, grey free. Yellow tick: hand brake. Dark red car: air applied.");
}

fn portrait(ui: &mut egui::Ui, eng: &Engineer, t: f64) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(88.0, 88.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let frame = mood_color(eng.mood);
    p.rect_filled(rect, 6.0, Color32::from_gray(18));
    p.rect_stroke(rect, 6.0, Stroke::new(2.0, frame), StrokeKind::Inside);
    let c = egui::pos2(rect.center().x, rect.center().y + 4.0);
    let skin = Color32::from_rgb(214, 170, 130);
    let dark = Color32::from_rgb(45, 32, 22);
    // Cap.
    p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x, c.y - 27.0), egui::vec2(46.0, 14.0)), 4.0, Color32::from_rgb(40, 62, 115));
    p.rect_filled(egui::Rect::from_center_size(egui::pos2(c.x + 7.0, c.y - 20.0), egui::vec2(56.0, 5.0)), 1.0, Color32::from_rgb(28, 42, 80));
    // Head.
    p.circle_filled(egui::pos2(c.x, c.y + 2.0), 24.0, skin);
    // Eyes and brows.
    let eye_r = match eng.mood {
        Mood::Calm => 2.6,
        Mood::Focused => 3.0,
        Mood::Pleased => 2.6,
        Mood::Worried => 3.6,
        Mood::Alarmed => 4.6,
        Mood::Panic => 5.6,
    };
    let blink = (t * 0.31).fract() < 0.035 && !matches!(eng.mood, Mood::Alarmed | Mood::Panic);
    let (brow_dy, tilt) = match eng.mood {
        Mood::Calm => (-11.0, 0.0),
        Mood::Focused => (-10.0, 1.6),
        Mood::Pleased => (-12.0, -1.0),
        Mood::Worried => (-13.0, 2.6),
        Mood::Alarmed => (-15.0, 0.0),
        Mood::Panic => (-16.5, -2.0),
    };
    for side in [-1.0f32, 1.0] {
        let e = egui::pos2(c.x + side * 9.0, c.y - 4.0);
        if blink {
            p.line_segment([egui::pos2(e.x - 3.0, e.y), egui::pos2(e.x + 3.0, e.y)], Stroke::new(1.5, dark));
        } else {
            p.circle_filled(e, eye_r, Color32::WHITE);
            p.circle_filled(egui::pos2(e.x, e.y + 0.5), eye_r * 0.55, dark);
        }
        let inner = egui::pos2(c.x + side * 4.0, c.y + brow_dy - tilt);
        let outer = egui::pos2(c.x + side * 14.0, c.y + brow_dy + tilt);
        p.line_segment([inner, outer], Stroke::new(2.0, dark));
    }
    // Mouth.
    let m = egui::pos2(c.x, c.y + 12.0);
    match eng.mood {
        Mood::Pleased => {
            let pts: Vec<egui::Pos2> = (0..=10).map(|i| {
                let x = -8.0 + 16.0 * i as f32 / 10.0;
                egui::pos2(m.x + x, m.y - 2.0 + (1.0 - (x / 8.0).powi(2)) * 4.0)
            }).collect();
            p.add(egui::Shape::line(pts, Stroke::new(2.0, dark)));
        }
        Mood::Calm => p.line_segment([egui::pos2(m.x - 7.0, m.y), egui::pos2(m.x + 7.0, m.y)], Stroke::new(2.0, dark)).into_noop(),
        Mood::Focused => p.line_segment([egui::pos2(m.x - 5.0, m.y), egui::pos2(m.x + 5.0, m.y)], Stroke::new(2.0, dark)).into_noop(),
        Mood::Worried => {
            let pts: Vec<egui::Pos2> = (0..=10).map(|i| {
                let x = -6.0 + 12.0 * i as f32 / 10.0;
                egui::pos2(m.x + x, m.y + 2.0 - (1.0 - (x / 6.0).powi(2)) * 3.0)
            }).collect();
            p.add(egui::Shape::line(pts, Stroke::new(2.0, dark)));
        }
        Mood::Alarmed => {
            p.circle_filled(m, 4.5, dark);
        }
        Mood::Panic => {
            p.circle_filled(egui::pos2(m.x, m.y + 2.0), 7.0, dark);
            p.circle_filled(egui::pos2(m.x, m.y + 4.5), 4.0, Color32::from_rgb(150, 60, 60));
        }
    }
}

trait Noop {
    fn into_noop(self);
}

impl Noop for egui::layers::ShapeIdx {
    fn into_noop(self) {}
}
