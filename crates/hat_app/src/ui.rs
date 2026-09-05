//! egui HUD: gauges, consist, yard, selection, score, map labels, help.

use std::collections::HashMap;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};
use egui::Color32;
use hat_sim::*;
use hat_units::fmt as uf;
use hat_units::UnitSystem;
use hat_world::*;

use crate::cab::{cab_window, CabView};
use crate::camera::{sim_to_world, MainCamera, Rig};
use crate::input::{Sel, Selection, UiState};
use crate::sim::{route_name, Sim};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, fps).add_systems(EguiPrimaryContextPass, hud);
    }
}

fn fps(diag: Res<DiagnosticsStore>, mut ui: ResMut<UiState>) {
    if let Some(v) = diag.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()) {
        ui.fps = v as f32;
    }
}

#[derive(Default, Clone, Copy)]
struct TrackStat {
    on: usize,
    dest: usize,
    placed: usize,
}

fn track_stats(sim: &Sim) -> HashMap<u32, TrackStat> {
    let mut m: HashMap<u32, TrackStat> = HashMap::new();
    let types = &sim.world.car_types;
    for tr in &sim.world.trains {
        let has_loco = tr.control_car(types).is_some();
        for (ci, c) in tr.cars.iter().enumerate() {
            let track = sim.world.car_location(tr, ci).and_then(|l| sim.world.graph.edge(l.edge).track);
            if let Some(t) = track {
                m.entry(t).or_default().on += 1;
            }
            if let Some(d) = c.dest {
                m.entry(d).or_default().dest += 1;
                if !has_loco && track == Some(d) {
                    m.entry(d).or_default().placed += 1;
                }
            }
        }
    }
    m
}

fn hud(
    mut contexts: EguiContexts,
    mut sim: ResMut<Sim>,
    mut sel: ResMut<Selection>,
    mut ui_state: ResMut<UiState>,
    cam: Single<(&Camera, &GlobalTransform), With<MainCamera>>,
    rig: Res<Rig>,
    cab: Option<Res<CabView>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let mut root = egui::Ui::new(ctx.clone(), "hat_root".into(), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()));
    let sim = &mut *sim;
    let u = ui_state.units;
    let (camera, cam_tf) = *cam;

    // Map labels and messages, painted straight onto the background layer.
    {
        let painter = ctx.layer_painter(egui::LayerId::background());
        let project = |p: glam::DVec2| camera.world_to_viewport(cam_tf, sim_to_world(p) + Vec3::Y * 1.5).ok().map(|v| egui::pos2(v.x, v.y));
        let font = egui::FontId::proportional(if rig.distance < 700.0 { 15.0 } else { 12.0 });
        for t in &sim.yard.tracks {
            let e = sim.world.graph.edge(t.edges[1]);
            let end = sim.world.graph.node(e.b).pos;
            let start = sim.world.graph.node(e.a).pos;
            for p in [end + glam::DVec2::new(10.0, 0.0), start + glam::DVec2::new(-6.0, 0.0)] {
                if let Some(sp) = project(p) {
                    painter.text(sp, egui::Align2::CENTER_CENTER, t.id.to_string(), font.clone(), Color32::WHITE);
                }
            }
        }
        for node in &sim.world.graph.nodes {
            if matches!(node.kind, NodeKind::Turnout { .. }) {
                if let Some(sp) = project(node.pos + glam::DVec2::new(0.0, -5.0)) {
                    painter.text(sp, egui::Align2::CENTER_CENTER, &node.name, egui::FontId::proportional(11.0), Color32::from_rgb(250, 220, 90));
                }
            }
        }
        let screen = ctx.content_rect();
        let mut y = screen.top() + 44.0;
        if let Some(b) = &sim.banner {
            if sim.world.t < b.until {
                let color = Color32::from_rgb(b.color[0], b.color[1], b.color[2]);
                let pos = egui::pos2(screen.center().x, screen.top() + 52.0);
                let font = egui::FontId::proportional(28.0);
                let r = painter.text(pos, egui::Align2::CENTER_CENTER, &b.text, font.clone(), color);
                painter.rect_filled(r.expand(10.0), 6.0, Color32::from_black_alpha(200));
                painter.rect_stroke(r.expand(10.0), 6.0, egui::Stroke::new(2.0, color), egui::StrokeKind::Inside);
                painter.text(pos, egui::Align2::CENTER_CENTER, &b.text, font, color);
                y = r.bottom() + 24.0;
            }
        }
        for (_, m) in &sim.messages {
            painter.text(egui::pos2(screen.center().x, y), egui::Align2::CENTER_TOP, m, egui::FontId::proportional(18.0), Color32::from_rgb(255, 240, 200));
            y += 24.0;
        }
    }

    egui::Panel::top("top").show(&mut root, |ui| {
        ui.horizontal(|ui| {
            ui.heading(sim.scenario.name);
            if sim.auto {
                let paused = sim.crew.as_ref().map(|c| c.paused).unwrap_or(false);
                ui.colored_label(if paused { Color32::from_rgb(250, 190, 60) } else { Color32::from_rgb(110, 200, 120) }, if paused { "AUTO paused: you have the engine" } else { "AUTO: crew driving" });
            }
            ui.separator();
            ui.label(format!("t {}", uf::duration(sim.world.t)));
            if sim.time_scale == 0 {
                ui.colored_label(Color32::LIGHT_RED, "PAUSED");
            } else {
                ui.label(format!("{}x", sim.time_scale));
            }
            ui.separator();
            let c = &sim.controls;
            ui.label(egui::RichText::new(uf::speed(sim.loco_speed(), u)).strong());
            let rev = match c.reverser {
                Reverser::Forward => "FWD",
                Reverser::Neutral => "N",
                Reverser::Reverse => "REV",
            };
            ui.label(format!("Notch {}  {rev}", c.throttle));
            ui.label(format!("Ind {:.0}%", c.independent * 100.0));
            if let Some(l) = sim.loco_car() {
                let auto = if c.emergency { "EMERG".to_string() } else { format!("{:.0} kPa set", c.auto_target / 1000.0) };
                ui.label(format!("Auto {auto}  Pipe {}  Cyl {}", uf::pressure(l.brake.p_pipe, u), uf::pressure(l.brake.p_cyl, u)));
            }
            if let Some((ti, ci)) = sim.loco_pos() {
                let tr = &sim.world.trains[ti];
                if let Some(l) = sim.world.car_location(tr, ci) {
                    let e = sim.world.graph.edge(l.edge);
                    ui.label(format!("On {} (limit {})", sim.yard.track_name(e.track.unwrap_or(0)), uf::speed(e.speed_limit, u)));
                }
            }
            {
                let v = sim.loco_speed().abs();
                match sim.ahead {
                    Some(Ahead::Restriction { limit, distance, .. }) => {
                        let too_fast = v > limit && distance < (v * v - limit * limit) / (2.0 * 0.6) * 1.3;
                        let color = if too_fast { Color32::from_rgb(250, 90, 70) } else { Color32::from_rgb(250, 190, 60) };
                        ui.colored_label(color, format!("Next {} in {}", uf::speed(limit, u), uf::length(distance, u)));
                    }
                    Some(Ahead::SplitSwitch { node, distance }) => {
                        ui.colored_label(Color32::from_rgb(250, 90, 70), format!("{} AGAINST in {}", sim.world.graph.node(node).name, uf::length(distance, u)));
                    }
                    Some(Ahead::End { distance }) => {
                        ui.label(format!("Bumper in {}", uf::length(distance, u)));
                    }
                    None => {}
                }
            }
            ui.separator();
            ui.label(egui::RichText::new(format!("Score {}", sim.score.points)).strong());
            ui.label(format!("Placed {}/{}  secured {}", sim.score.cars_placed, sim.score.cars_total, sim.score.cars_secured));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(format!("{:.0} fps", ui_state.fps));
                if ui.button(match u {
                    UnitSystem::Metric => "SI",
                    UnitSystem::UsCustomary => "US",
                }).clicked()
                {
                    ui_state.units = u.toggle();
                }
                if ui.button("Help (F1)").clicked() {
                    ui_state.show_help = !ui_state.show_help;
                }
            });
        });
    });

    egui::Panel::left("consist").default_size(400.0).show(&mut root, |ui| {
        ui.heading("Consist");
        if let Some((ti, li)) = sim.loco_pos() {
            let types = &sim.world.car_types;
            let tr = &sim.world.trains[ti];
            let n = tr.cars.len();
            ui.label(format!("{} cars, {}, {}", n, uf::length(tr.length(types), u), uf::mass(tr.total_mass(types), u)));
            ui.small("Head at top. Click a car or the coupler line between two cars.");
            let mut clicked: Option<Sel> = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for ci in 0..n {
                    let c = &tr.cars[ci];
                    let ct = &types[c.type_id as usize];
                    let selected = sel.sel == Sel::Car(c.id);
                    let air = if c.brake.emergency {
                        "EMERG".to_string()
                    } else if c.brake.is_bled() {
                        "bled".to_string()
                    } else {
                        format!("cyl {:>3.0}", c.brake.p_cyl / 1000.0)
                    };
                    let dest = c.dest.map(|d| format!("→{d}")).unwrap_or_default();
                    let text = format!(
                        "{:>3} #{:<3} {:<14} {:<9} {:<4} {} {}{}",
                        ci + 1,
                        c.id,
                        ct.name,
                        format!("{:?}", c.commodity),
                        dest,
                        if c.hand_brake > 0.5 { "HB" } else { "  " },
                        air,
                        if c.derailed { " DERAILED" } else { "" }
                    );
                    let color = if ci == li {
                        Color32::from_rgb(255, 190, 80)
                    } else if c.derailed {
                        Color32::LIGHT_RED
                    } else {
                        Color32::LIGHT_GRAY
                    };
                    if ui.selectable_label(selected, egui::RichText::new(text).monospace().color(color)).clicked() {
                        clicked = Some(Sel::Car(c.id));
                    }
                    if ci + 1 < n {
                        let rear = tr.cars[ci + 1].id;
                        let ktext = format!(
                            "        ⟷ {:<5} {:+5.0} mm {}",
                            format!("{:?}", tr.zones[ci]),
                            tr.coupler_extension(types, ci + 1) * 1000.0,
                            if tr.hoses[ci] { "air" } else { "" }
                        );
                        let ksel = sel.sel == Sel::Coupler(c.id, rear);
                        if ui.selectable_label(ksel, egui::RichText::new(ktext).monospace().small().color(Color32::GRAY)).clicked() {
                            clicked = Some(Sel::Coupler(c.id, rear));
                        }
                    }
                }
            });
            if let Some(s) = clicked {
                sel.sel = s;
            }
        } else {
            ui.label("No locomotive in the world.");
        }
    });

    egui::Panel::right("yard").default_size(340.0).show(&mut root, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Scenario");
            ui.horizontal_wrapped(|ui| {
                for (i, sc) in all_scenarios().iter().enumerate() {
                    if ui.selectable_label(i == sim.scenario_index, sc.name).clicked() {
                        sim.reload = Some(i);
                    }
                }
                if ui.button("Restart").clicked() {
                    sim.reload = Some(sim.scenario_index);
                }
            });
            ui.small(sim.scenario.description);
            ui.separator();

            ui.heading("Yard");
            let stats = track_stats(sim);
            egui::Grid::new("tracks").striped(true).show(ui, |ui| {
                ui.strong("Track");
                ui.strong("Length");
                ui.strong("Cars");
                ui.strong("Placed");
                ui.end_row();
                for t in &sim.yard.tracks {
                    let s = stats.get(&t.id).copied().unwrap_or_default();
                    ui.label(&t.name);
                    ui.label(uf::length(t.length, u));
                    ui.label(s.on.to_string());
                    ui.label(format!("{}/{}", s.placed, s.dest));
                    ui.end_row();
                }
                for (id, name) in [(TRACK_MAIN, "Main"), (TRACK_LEAD, "Lead")] {
                    let s = stats.get(&id).copied().unwrap_or_default();
                    ui.label(name);
                    ui.label("");
                    ui.label(s.on.to_string());
                    ui.label("");
                    ui.end_row();
                }
            });
            ui.separator();

            ui.heading("Switches");
            ui.small("Click a switch on the map or here. Green leg is the set route.");
            let mut throw: Option<NodeId> = None;
            ui.horizontal_wrapped(|ui| {
                for (ni, node) in sim.world.graph.nodes.iter().enumerate() {
                    if let NodeKind::Turnout { setting, .. } = &node.kind {
                        if ui.button(format!("{} {}", node.name, route_name(*setting))).clicked() {
                            throw = Some(ni as NodeId);
                        }
                    }
                }
            });
            if let Some(n) = throw {
                sim.action_throw_switch(n);
            }
            ui.separator();

            ui.heading("Crew");
            crew_panel(ui, sim);
            ui.separator();

            if sim.dispatcher.is_some() {
                ui.heading("Traffic");
                traffic_panel(ui, sim);
                ui.separator();
            }

            ui.heading("Selection");
            selection_panel(ui, sim, sel.sel, u);
            ui.separator();

            ui.heading("Score");
            let s = &sim.score;
            ui.label(format!("{} points", s.points));
            ui.label(format!("Time {} of {} budget", uf::duration(s.elapsed), uf::duration(sim.scenario.time_budget)));
            ui.label(format!("Placed {}/{}, secured {}, fouling lead {}", s.cars_placed, s.cars_total, s.cars_secured, s.cars_fouling));
            ui.label(format!("Hard couplings {}  bumps {}  knuckles {}  derails {}  bumper hits {}", s.hard_couplings, s.bumps, s.knuckle_breaks, s.derailments, s.bumper_hits));
            ui.label(format!("Damage {:.1}", s.damage));
            ui.separator();

            ui.heading("Log");
            for (t, m) in sim.score.log.iter().rev().take(12) {
                ui.small(format!("{} {}", uf::duration(*t), m));
            }
        });
    });

    if let Some(cab) = cab.as_deref() {
        cab_window(ctx, sim, cab, u);
    }

    if ui_state.show_help {
        egui::Window::new("Controls").collapsible(false).show(ctx, |ui| {
            ui.monospace(
                "Locomotive\n\
                 W / S      throttle notch up / down\n\
                 D / A      reverser toward forward / reverse (neutral between)\n\
                 X / Z      independent brake more / less\n\
                 V / C      automatic brake apply 30 kPa / release\n\
                 Backspace  emergency\n\
                 \n\
                 Crew (select first: click a car, a coupler gap, or a switch)\n\
                 Space      pull pin at selected coupler (needs bunched slack)\n\
                 H          hand brake on/off on selected car\n\
                 L          bleed selected car (no air brake until recharged)\n\
                 K          lace air hoses on the selected train\n\
                 J          bottle the air at the selected coupler\n\
                 click      throw a switch (refused if occupied)\n\
                 \n\
                 Cab window: drag the levers (THR, REV, IND, AUTO), or click a notch. The\n\
                 picture follows the locomotive. Red gauge arc is the current limit, amber\n\
                 tick the next one. The strip below is your train: red couplers bunched,\n\
                 green stretched.\n\
                 \n\
                 View\n\
                 arrows     pan    wheel  zoom    right-drag  orbit    middle-drag  pan\n\
                 F          follow locomotive     Home  reset camera\n\
                 \n\
                 Time       P pause    1 / 2 / 3  speed 1x / 4x / 10x\n\
                 U units    F11 fullscreen    Esc deselect    Cmd/Ctrl+Q quit\n\
                 \n\
                 Real rules in play: couple under 1.8 m/s or take damage. Pins pull only\n\
                 with slack bunched, so shove before you cut. A cut with laced hoses vents\n\
                 to emergency when parted unless you bottle the air. Bled cars roll free.\n\
                 A car is placed when it stands on its track with no locomotive attached\n\
                 and its cut carries one hand brake per ten cars.",
            );
        });
    }

    if sim.score.complete {
        egui::Window::new("Shift complete").anchor(egui::Align2::CENTER_CENTER, [0.0, -60.0]).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.heading(format!("{} points", sim.score.points));
            ui.label(format!("{} in {}", sim.scenario.name, uf::duration(sim.score.elapsed)));
            ui.label(format!("Hard couplings {}, derailments {}, damage {:.1}", sim.score.hard_couplings, sim.score.derailments, sim.score.damage));
            if ui.button("Next scenario").clicked() {
                sim.reload = Some(sim.scenario_index + 1);
            }
        });
    }

    ui_state.pointer_over_ui = ctx.egui_wants_pointer_input() || ctx.is_pointer_over_egui();
    Ok(())
}

fn selection_panel(ui: &mut egui::Ui, sim: &mut Sim, s: Sel, u: UnitSystem) {
    match s {
        Sel::None => {
            ui.label("Click a car, the gap between two cars, or a switch.");
        }
        Sel::Car(id) => {
            let Some((tid, ci)) = sim.resolve_car(id) else {
                ui.label("Car gone");
                return;
            };
            let (line1, line2, line3, hb) = {
                let tr = sim.world.train(tid).unwrap();
                let c = &tr.cars[ci];
                let ct = &sim.world.car_types[c.type_id as usize];
                (
                    format!("Car #{id}: {} ({:?})", ct.name, c.commodity),
                    format!("{} gross, destination {}, damage {:.1}", uf::mass(c.mass(ct), u), c.dest.map(|d| sim.yard.track_name(d)).unwrap_or_else(|| "-".into()), c.damage),
                    format!(
                        "Hand brake {}. Air: pipe {} aux {} cyl {}{}",
                        if c.hand_brake > 0.5 { "SET" } else { "off" },
                        uf::pressure(c.brake.p_pipe, u),
                        uf::pressure(c.brake.p_aux, u),
                        uf::pressure(c.brake.p_cyl, u),
                        if c.brake.emergency { " EMERGENCY" } else { "" }
                    ),
                    c.hand_brake > 0.5,
                )
            };
            ui.label(line1);
            ui.label(line2);
            ui.label(line3);
            let (cur_dest, kind) = {
                let tr = sim.world.train(tid).unwrap();
                let c = &tr.cars[ci];
                (c.dest, sim.world.car_types[c.type_id as usize].kind)
            };
            let mut new_dest: Option<Option<u32>> = None;
            let mut all_kind = false;
            ui.horizontal_wrapped(|ui| {
                ui.label("Send to:");
                let ids: Vec<u32> = sim.yard.tracks.iter().map(|t| t.id).collect();
                for t in ids {
                    if ui.selectable_label(cur_dest == Some(t), format!("{t}")).clicked() {
                        new_dest = Some(Some(t));
                    }
                }
                if ui.selectable_label(cur_dest.is_none(), "none").clicked() {
                    new_dest = Some(None);
                }
                if ui.button(format!("all {kind:?}")).clicked() {
                    all_kind = true;
                }
            });
            if let Some(d) = new_dest {
                sim.set_dest(id, d);
            }
            if all_kind {
                sim.set_dest_for_kind(kind, cur_dest);
            }
            ui.horizontal_wrapped(|ui| {
                if ui.button(if hb { "Release hand brake (H)" } else { "Set hand brake (H)" }).clicked() {
                    sim.action_hand_brake(s);
                }
                if ui.button("Bleed (L)").clicked() {
                    sim.action_bleed(s);
                }
                if ui.button("Lace hoses (K)").clicked() {
                    sim.action_connect_hoses(s);
                }
            });
        }
        Sel::Coupler(f, r) => {
            let Some((tid, k)) = sim.resolve_coupler(f, r) else {
                ui.label("Those cars are no longer coupled.");
                return;
            };
            let text = {
                let tr = sim.world.train(tid).unwrap();
                format!(
                    "Coupler between #{f} and #{r}: {:?}, {:+.0} mm, hose {}",
                    tr.zones[k - 1],
                    tr.coupler_extension(&sim.world.car_types, k) * 1000.0,
                    if tr.hoses[k - 1] { "connected" } else { "open" }
                )
            };
            ui.label(text);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Pull pin (Space)").clicked() {
                    sim.action_pull_pin(s);
                }
                if ui.button("Bottle air (J)").clicked() {
                    sim.action_bottle_air(s);
                }
            });
        }
    }
}

fn crew_panel(ui: &mut egui::Ui, sim: &mut Sim) {
    let mut auto = sim.auto;
    if ui.checkbox(&mut auto, "Auto: the crew works the switch list").changed() {
        if auto {
            sim.resume_crew();
        } else {
            sim.auto = false;
            if let Some(c) = sim.crew.as_mut() {
                c.paused = true;
            }
        }
    }
    let Some(crew) = sim.crew.as_ref() else {
        ui.label("No crew.");
        return;
    };
    let status = match &crew.status {
        Status::Running => if crew.paused { "paused, you have the engine".to_string() } else { crew.describe() },
        Status::Done => format!("shift complete in {}", uf::duration(crew.finished_at.unwrap_or(0.0) - crew.started_at)),
        Status::Failed(why) => format!("stuck: {why}"),
    };
    ui.label(format!("{}: {status}", crew.name));
    let mut resume = false;
    if crew.paused && sim.auto || matches!(crew.status, Status::Failed(_)) {
        if ui.button("Resume crew").clicked() {
            resume = true;
        }
    }
    let plan = crew.plan_preview(&sim.world);
    if !plan.is_empty() {
        ui.small(format!("{} moves left: ", plan.len()) + &plan.iter().take(8).map(|(t, n)| format!("T{t}\u{2190}{n}")).collect::<Vec<_>>().join("  ") + if plan.len() > 8 { " \u{2026}" } else { "" });
    }
    for (t, line) in crew.radio.iter().rev().take(5) {
        ui.small(format!("{} {line}", uf::duration(*t)));
    }
    if resume {
        sim.resume_crew();
    }
}

fn traffic_panel(ui: &mut egui::Ui, sim: &Sim) {
    let Some(d) = sim.dispatcher.as_ref() else { return };
    let t = sim.world.t;
    ui.label(format!("Inbound {}  outbound {}  delivered {} cars, {:.0} t", d.inbound_count, d.outbound_count, d.cars_delivered, d.cargo_delivered / 1000.0));
    ui.label(format!("Loaded {:.0} t   unloaded {:.0} t", sim.world.cargo_loaded / 1000.0, sim.world.cargo_unloaded / 1000.0));
    match d.road.as_ref() {
        Some(r) => ui.label(format!("Road crew: {}", r.describe())),
        None => ui.label(format!("Next inbound in {}", uf::duration((d.next_inbound_at - t).max(0.0)))),
    };
    for (lt, line) in d.log.iter().rev().take(5) {
        ui.small(format!("{} {line}", uf::duration(*lt)));
    }
}
