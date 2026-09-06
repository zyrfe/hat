//! The wreck crew: called to a derailment, it takes time and money to put cars back on.

use hat_sim::*;

use crate::economy::{Account, Ledger};

/// Minutes to reach the site.
const CALL_OUT_S: f64 = 15.0 * 60.0;
/// Per derailed car: jacks, frogs, a cable, and a lot of standing around.
const PER_DERAILED_CAR_S: f64 = 25.0 * 60.0;
/// Per other car in the train: inspection before it moves.
const PER_OTHER_CAR_S: f64 = 90.0;
const CALL_OUT_CENTS: i64 = 300_000;
const PER_DERAILED_CAR_CENTS: i64 = 250_000;
const PER_DAMAGE_POINT_CENTS: i64 = 40_000;

#[derive(Clone, Debug)]
pub struct RerailJob {
    pub train: TrainId,
    pub started: f64,
    pub done_at: f64,
    pub cost: i64,
    pub cars: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Wrecker {
    pub jobs: Vec<RerailJob>,
    pub log: Vec<(f64, String)>,
}

impl Wrecker {
    /// Seconds, cents and derailed-car count to rerail `train`, if it is derailed.
    pub fn estimate(world: &World, train: TrainId) -> Option<(f64, i64, usize)> {
        let tr = world.train(train)?;
        if !tr.derailed {
            return None;
        }
        let derailed = tr.cars.iter().filter(|c| c.derailed).count().max(1);
        let others = tr.cars.len().saturating_sub(derailed);
        let damage: f64 = tr.cars.iter().filter(|c| c.derailed).map(|c| c.damage).sum();
        let secs = CALL_OUT_S + PER_DERAILED_CAR_S * derailed as f64 + PER_OTHER_CAR_S * others as f64;
        let cents = CALL_OUT_CENTS + PER_DERAILED_CAR_CENTS * derailed as i64 + (PER_DAMAGE_POINT_CENTS as f64 * damage) as i64;
        Some((secs, cents, derailed))
    }

    pub fn job_for(&self, train: TrainId) -> Option<&RerailJob> {
        self.jobs.iter().find(|j| j.train == train)
    }

    /// Progress of the job on `train`, 0..1.
    pub fn progress(&self, train: TrainId, t: f64) -> Option<f64> {
        let j = self.job_for(train)?;
        Some(((t - j.started) / (j.done_at - j.started)).clamp(0.0, 1.0))
    }

    pub fn request(&mut self, world: &World, train: TrainId) -> Result<RerailJob, &'static str> {
        if self.job_for(train).is_some() {
            return Err("the wreck crew is already on it");
        }
        let (secs, cost, cars) = Self::estimate(world, train).ok_or("that train is not derailed")?;
        let job = RerailJob { train, started: world.t, done_at: world.t + secs, cost, cars };
        self.jobs.push(job.clone());
        self.log.push((world.t, format!("Wreck crew called for train {train}: {cars} on the ground.")));
        Ok(job)
    }

    /// Advance the jobs. Finished ones rerail their train and bill the ledger.
    pub fn step(&mut self, world: &mut World, mut ledger: Option<&mut Ledger>) {
        let t = world.t;
        let mut i = 0;
        while i < self.jobs.len() {
            let job = &mut self.jobs[i];
            if world.train(job.train).is_none() {
                self.jobs.remove(i);
                continue;
            }
            if t < job.done_at {
                i += 1;
                continue;
            }
            match world.rerail(job.train) {
                Ok(n) => {
                    if let Some(l) = ledger.as_deref_mut() {
                        l.post(t, Account::Wreck, -job.cost, format!("Rerailed {n} cars of train {}", job.train));
                    }
                    self.log.push((t, format!("Train {} is back on the rails.", job.train)));
                    self.jobs.remove(i);
                }
                Err(e) => {
                    self.log.push((t, format!("Wreck crew on train {}: {e}", job.train)));
                    job.done_at = t + 120.0;
                    i += 1;
                }
            }
        }
        while self.log.len() > 40 {
            self.log.remove(0);
        }
    }
}
