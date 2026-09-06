//! Money. Shippers pay what it would cost them not to use the railroad; the railroad pays
//! for fuel, crews, equipment, track and wrecks. See docs/economy.md.

use std::collections::{HashMap, VecDeque};

use glam::DVec2;
use hat_sim::*;

use crate::industry::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Account {
    Freight,
    Fuel,
    Crew,
    Equipment,
    Track,
    Wreck,
    Purchase,
}

pub const ACCOUNTS: [Account; 7] = [Account::Freight, Account::Fuel, Account::Crew, Account::Equipment, Account::Track, Account::Wreck, Account::Purchase];

impl Account {
    pub fn name(self) -> &'static str {
        match self {
            Account::Freight => "Freight",
            Account::Fuel => "Fuel",
            Account::Crew => "Crews",
            Account::Equipment => "Equipment",
            Account::Track => "Track",
            Account::Wreck => "Wrecks",
            Account::Purchase => "Purchases",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub t: f64,
    pub account: Account,
    /// Minor units (cents). Positive is income.
    pub amount: i64,
    pub note: String,
}

#[derive(Clone, Debug, Default)]
pub struct Ledger {
    pub balance: i64,
    pub entries: VecDeque<Entry>,
    pub totals: HashMap<Account, i64>,
}

impl Ledger {
    pub fn post(&mut self, t: f64, account: Account, amount: i64, note: impl Into<String>) {
        if amount == 0 {
            return;
        }
        self.balance += amount;
        *self.totals.entry(account).or_default() += amount;
        self.entries.push_back(Entry { t, account, amount, note: note.into() });
        while self.entries.len() > 300 {
            self.entries.pop_front();
        }
    }

    pub fn total(&self, account: Account) -> i64 {
        self.totals.get(&account).copied().unwrap_or(0)
    }
}

/// Money in minor units as text.
pub fn money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    let dollars = abs / 100;
    if dollars >= 1_000_000 {
        format!("{sign}${:.2}M", dollars as f64 / 1e6)
    } else if dollars >= 10_000 {
        format!("{sign}${:.1}k", dollars as f64 / 1e3)
    } else {
        format!("{sign}${dollars}")
    }
}

#[derive(Clone, Debug)]
pub struct Prices {
    /// Trucking, $ per tonne per road km.
    pub truck_per_tkm: f64,
    /// Loading and unloading a truck, $ per tonne, each end.
    pub truck_handling: f64,
    /// Road distance over straight-line distance.
    pub road_factor: f64,
    pub fuel_per_l: f64,
    /// Rail work per litre burned: thermal efficiency times fuel energy.
    pub loco_efficiency: f64,
    pub fuel_j_per_l: f64,
    /// Litres per second a running locomotive burns doing nothing.
    pub idle_l_per_s: f64,
    pub crew_per_hour: f64,
    pub car_per_day: f64,
    pub loco_per_day: f64,
    pub track_per_km_day: f64,
}

impl Default for Prices {
    fn default() -> Self {
        Prices {
            truck_per_tkm: 0.15,
            truck_handling: 3.0,
            road_factor: 1.4,
            fuel_per_l: 1.20,
            loco_efficiency: 0.30,
            fuel_j_per_l: 36.0e6,
            idle_l_per_s: 0.004,
            crew_per_hour: 60.0,
            car_per_day: 20.0,
            loco_per_day: 400.0,
            track_per_km_day: 40.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Economy {
    pub industries: Vec<Industry>,
    pub ledger: Ledger,
    pub prices: Prices,
    pub track_km: f64,
    /// Accrued but unposted costs, cents, per account.
    accrued: HashMap<Account, f64>,
    last_post: f64,
    /// Tractive work already billed per locomotive car.
    work_seen: HashMap<CarId, f64>,
    pub log: VecDeque<(f64, String)>,
}

impl Economy {
    pub fn new(industries: Vec<Industry>, graph: &TrackGraph, opening_balance: i64) -> Self {
        let track_km = graph.edges.iter().map(|e| e.length).sum::<f64>() / 1000.0;
        let mut ledger = Ledger::default();
        ledger.post(0.0, Account::Purchase, opening_balance, "Opening balance");
        Economy { industries, ledger, prices: Prices::default(), track_km, accrued: HashMap::new(), last_post: 0.0, work_seen: HashMap::new(), log: VecDeque::new() }
    }

    pub fn industry(&self, id: u32) -> Option<&Industry> {
        self.industries.iter().find(|i| i.id == id)
    }

    fn note(&mut self, t: f64, s: String) {
        self.log.push_back((t, s));
        while self.log.len() > 60 {
            self.log.pop_front();
        }
    }

    /// Straight-line distance dressed up as road, km.
    pub fn road_km(&self, a: DVec2, b: DVec2) -> f64 {
        a.distance(b) * self.prices.road_factor / 1000.0
    }

    /// What trucking would cost the consumer per tonne from its nearest producer.
    pub fn direct_trucking(&self, consumer: &Industry) -> f64 {
        let nearest = self.industries.iter().filter(|p| p.is_producer() && p.commodity == consumer.commodity).map(|p| self.road_km(p.pos, consumer.pos)).fold(f64::INFINITY, f64::min);
        if !nearest.is_finite() {
            return 0.0;
        }
        2.0 * self.prices.truck_handling + self.prices.truck_per_tkm * nearest
    }

    /// Last-mile trucking the shipper still pays for when a railhead is not at the door.
    fn last_mile(&self, graph: &TrackGraph, industry: &Industry) -> f64 {
        let nearest = industry
            .facilities
            .iter()
            .map(|&e| {
                let edge = graph.edge(e);
                let s = edge.facility.and_then(|f| f.spot()).unwrap_or(edge.length * 0.5);
                self.road_km(graph.pose_on_edge(e, s).pos, industry.pos)
            })
            .fold(f64::INFINITY, f64::min);
        if !nearest.is_finite() || nearest < 0.05 {
            return 0.0;
        }
        self.prices.truck_handling + self.prices.truck_per_tkm * nearest
    }

    /// Freight the consumer pays per tonne delivered by rail from `origin`.
    pub fn offer(&self, graph: &TrackGraph, consumer: &Industry, origin: Option<u32>) -> f64 {
        let direct = self.direct_trucking(consumer);
        let mut last = self.last_mile(graph, consumer);
        if let Some(o) = origin.and_then(|id| self.industry(id)) {
            last += self.last_mile(graph, o);
        }
        let discount = 0.6 + 0.4 * consumer.service.clamp(0.0, 1.0);
        ((direct - last).max(0.0) * discount * 100.0).round() / 100.0
    }

    /// Before the sim step: tell every facility what it may move.
    pub fn before_step(&self, world: &mut World) {
        for i in &self.industries {
            i.publish(&mut world.graph);
        }
    }

    /// After the sim step: settle industries, pay freight, accrue running costs.
    pub fn after_step(&mut self, world: &World, events: &[SimEvent], dt: f64, crews_on_duty: usize) {
        let t = world.t;
        for i in &mut self.industries {
            i.settle(&world.graph, t, dt);
        }
        for e in events {
            if let SimEvent::Unloaded { mass, commodity, industry: Some(id), origin, .. } = e {
                let Some(consumer) = self.industry(*id).cloned() else { continue };
                if consumer.is_producer() || consumer.commodity != *commodity {
                    continue;
                }
                let rate = self.offer(&world.graph, &consumer, *origin);
                let tonnes = mass / 1000.0;
                let cents = (rate * tonnes * 100.0).round() as i64;
                let from = origin.and_then(|o| self.industry(o)).map(|o| o.name.clone()).unwrap_or_else(|| "?".into());
                self.ledger.post(t, Account::Freight, cents, format!("{:.0} t {:?} {} to {} at ${:.2}/t", tonnes, commodity, from, consumer.name, rate));
            }
        }
        // Running costs.
        let p = &self.prices;
        let types = &world.car_types;
        let mut cars = 0usize;
        let mut locos = 0usize;
        let mut litres = 0.0;
        for tr in &world.trains {
            for c in &tr.cars {
                if types[c.type_id as usize].is_loco() {
                    locos += 1;
                    let seen = self.work_seen.entry(c.id).or_insert(c.work_j);
                    let dj = (c.work_j - *seen).max(0.0);
                    *seen = c.work_j;
                    litres += dj / (p.loco_efficiency * p.fuel_j_per_l) + p.idle_l_per_s * dt;
                } else {
                    cars += 1;
                }
            }
        }
        *self.accrued.entry(Account::Fuel).or_default() -= litres * p.fuel_per_l * 100.0;
        *self.accrued.entry(Account::Crew).or_default() -= crews_on_duty as f64 * p.crew_per_hour * dt / 3600.0 * 100.0;
        *self.accrued.entry(Account::Equipment).or_default() -= (cars as f64 * p.car_per_day + locos as f64 * p.loco_per_day) * dt / DAY * 100.0;
        *self.accrued.entry(Account::Track).or_default() -= self.track_km * p.track_per_km_day * dt / DAY * 100.0;
        if t - self.last_post >= 3600.0 {
            self.last_post = t;
            for acct in [Account::Fuel, Account::Crew, Account::Equipment, Account::Track] {
                let amount = self.accrued.remove(&acct).unwrap_or(0.0).round() as i64;
                self.ledger.post(t, acct, amount, format!("{} for the hour", acct.name()));
            }
        }
    }

    /// Cash the accrued costs so far into the ledger, for a report.
    pub fn flush(&mut self, t: f64) {
        for acct in ACCOUNTS {
            if let Some(v) = self.accrued.remove(&acct) {
                self.ledger.post(t, acct, v.round() as i64, format!("{} to date", acct.name()));
            }
        }
        self.last_post = t;
    }

    pub fn say(&mut self, t: f64, s: impl Into<String>) {
        self.note(t, s.into());
    }
}
