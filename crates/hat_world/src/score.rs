//! Scoring for a shift. Consumes sim events and inspects the world.

use hat_sim::*;

use crate::yard::*;

#[derive(Clone, Debug, Default)]
pub struct Score {
    pub elapsed: f64,
    pub hard_couplings: u32,
    pub bumps: u32,
    pub knuckle_breaks: u32,
    pub derailments: u32,
    pub bumper_hits: u32,
    pub emergencies: u32,
    pub damage: f64,
    pub cars_total: usize,
    pub cars_placed: usize,
    pub cars_secured: usize,
    pub cars_fouling: usize,
    pub complete: bool,
    pub points: i64,
    pub log: Vec<(f64, String)>,
}

impl Score {
    pub fn ingest(&mut self, t: f64, events: &[SimEvent], yard: &Yard) {
        for e in events {
            match e {
                SimEvent::Coupled { rel_speed, hard, .. } => {
                    if *hard {
                        self.hard_couplings += 1;
                        self.note(t, format!("Hard coupling at {:.1} m/s", rel_speed));
                    }
                }
                SimEvent::Bumped { rel_speed, .. } => {
                    if *rel_speed > 0.3 {
                        self.bumps += 1;
                    }
                }
                SimEvent::KnuckleBreak { .. } => {
                    self.knuckle_breaks += 1;
                    self.note(t, "Knuckle broke".to_string());
                }
                SimEvent::Derail { cause, .. } => {
                    self.derailments += 1;
                    self.note(t, format!("Derailment: {cause:?}"));
                }
                SimEvent::BumperHit { speed, .. } => {
                    self.bumper_hits += 1;
                    self.note(t, format!("Hit the bumper at {:.1} m/s", speed));
                }
                SimEvent::Emergency { .. } => {
                    self.emergencies += 1;
                }
                SimEvent::SwitchThrown { node, .. } => {
                    let _ = (node, yard);
                }
                _ => {}
            }
        }
        if self.log.len() > 200 {
            self.log.drain(0..self.log.len() - 200);
        }
    }

    fn note(&mut self, t: f64, s: String) {
        self.log.push((t, s));
    }

    /// Recompute placement from the world. A car is placed when it stands on its
    /// destination track in a train with no locomotive. A standing cut is secured when it
    /// carries at least one hand brake per ten cars.
    pub fn evaluate(&mut self, world: &World, yard: &Yard, time_budget: f64) {
        self.elapsed = world.t;
        let types = &world.car_types;
        let mut total = 0;
        let mut placed = 0;
        let mut secured = 0;
        let mut fouling = 0;
        let mut damage = 0.0;
        for tr in &world.trains {
            let has_loco = tr.control_car(types).is_some();
            let hand_brakes = tr.cars.iter().filter(|c| c.hand_brake > 0.5).count();
            let need = (tr.cars.len() + 9) / 10;
            let cut_secured = !has_loco && hand_brakes >= need;
            for (i, c) in tr.cars.iter().enumerate() {
                damage += c.damage;
                let Some(dest) = c.dest else { continue };
                total += 1;
                let track = world.car_location(tr, i).and_then(|l| world.graph.edge(l.edge).track);
                if track == Some(TRACK_LEAD) {
                    fouling += 1;
                }
                if !has_loco && track == Some(dest) {
                    placed += 1;
                    if cut_secured {
                        secured += 1;
                    }
                }
            }
        }
        let _ = yard;
        self.cars_total = total;
        self.cars_placed = placed;
        self.cars_secured = secured;
        self.cars_fouling = fouling;
        self.damage = damage;
        self.complete = total > 0 && secured == total;

        let over = (self.elapsed - time_budget).max(0.0);
        let mut pts: i64 = 10_000;
        pts -= (self.elapsed / 6.0) as i64;
        pts -= (over / 2.0) as i64;
        pts -= 200 * self.hard_couplings as i64;
        pts -= 50 * self.bumps as i64;
        pts -= 500 * self.knuckle_breaks as i64;
        pts -= 1500 * self.derailments as i64;
        pts -= 300 * self.bumper_hits as i64;
        pts -= (self.damage * 100.0) as i64;
        pts += 100 * self.cars_secured as i64;
        self.points = pts;
    }
}
