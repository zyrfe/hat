//! Industries: places that make or take a commodity, with a stockpile and a railhead.
//! What the railroad does not move, trucks move, and the industry remembers.

use glam::DVec2;
use hat_sim::*;

pub const DAY: f64 = 86_400.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndustryKind {
    CoalMine,
    PowerPlant,
    Elevator,
    GrainTerminal,
}

#[derive(Clone, Debug)]
pub struct Industry {
    pub id: u32,
    pub name: String,
    pub kind: IndustryKind,
    pub pos: DVec2,
    pub commodity: Commodity,
    /// kg/s. Positive produces into the stockpile, negative consumes from it.
    pub rate: f64,
    /// kg on hand: waiting for pickup at a producer, waiting to be burned at a consumer.
    pub stock: f64,
    pub capacity: f64,
    /// Edges carrying this industry's facilities.
    pub facilities: Vec<EdgeId>,
    /// Lifetime kg moved by rail and by truck.
    pub rail: f64,
    pub trucked: f64,
    day_rail: f64,
    day_truck: f64,
    day: u64,
    /// 0..1: how much of its business this industry trusts to the railroad, from recent days.
    pub service: f64,
}

impl Industry {
    pub fn producer(id: u32, name: &str, kind: IndustryKind, pos: DVec2, commodity: Commodity, rate: f64, capacity: f64, stock: f64) -> Self {
        Industry { id, name: name.into(), kind, pos, commodity, rate, stock, capacity, facilities: vec![], rail: 0.0, trucked: 0.0, day_rail: 0.0, day_truck: 0.0, day: 0, service: 0.5 }
    }

    pub fn consumer(id: u32, name: &str, kind: IndustryKind, pos: DVec2, commodity: Commodity, rate: f64, capacity: f64, stock: f64) -> Self {
        let mut i = Self::producer(id, name, kind, pos, commodity, -rate, capacity, stock);
        i.day = 0;
        i
    }

    pub fn is_producer(&self) -> bool {
        self.rate > 0.0
    }

    /// Tonnes per day the industry makes or takes.
    pub fn tonnes_per_day(&self) -> f64 {
        self.rate.abs() * DAY / 1000.0
    }

    /// Hand the facilities what they may move this step.
    pub fn publish(&self, graph: &mut TrackGraph) {
        let budget = if self.is_producer() { self.stock } else { (self.capacity - self.stock).max(0.0) };
        for &e in &self.facilities {
            if let Some(f) = graph.edges[e as usize].facility.as_mut() {
                f.budget = budget;
            }
        }
    }

    /// Read back what moved, then run the industry's own clock.
    pub fn settle(&mut self, graph: &TrackGraph, t: f64, dt: f64) {
        let budget_before = if self.is_producer() { self.stock } else { (self.capacity - self.stock).max(0.0) };
        // Every facility saw the same budget; the smallest remaining one is the truth.
        let budget_after = self.facilities.iter().filter_map(|&e| graph.edge(e).facility.map(|f| f.budget)).fold(budget_before, f64::min);
        let moved = (budget_before - budget_after).max(0.0);
        self.rail += moved;
        self.day_rail += moved;
        if self.is_producer() {
            self.stock -= moved;
            self.stock += self.rate * dt;
            if self.stock > self.capacity {
                let over = self.stock - self.capacity;
                self.trucked += over;
                self.day_truck += over;
                self.stock = self.capacity;
            }
        } else {
            self.stock += moved;
            let need = -self.rate * dt;
            if self.stock >= need {
                self.stock -= need;
            } else {
                let short = need - self.stock;
                self.stock = 0.0;
                self.trucked += short;
                self.day_truck += short;
            }
        }
        let day = (t / DAY) as u64;
        if day != self.day {
            self.day = day;
            let total = self.day_rail + self.day_truck;
            if total > 0.0 {
                let share = self.day_rail / total;
                self.service = 0.6 * self.service + 0.4 * share;
            }
            self.day_rail = 0.0;
            self.day_truck = 0.0;
        }
    }

    /// Share of the stockpile in use, 0..1.
    pub fn fill(&self) -> f64 {
        if self.capacity > 0.0 { (self.stock / self.capacity).clamp(0.0, 1.0) } else { 0.0 }
    }
}
