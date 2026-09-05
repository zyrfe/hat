//! The dispatcher: road trains in and out through the portal, and the yard master that
//! hands the yard crew its next job.

use hat_sim::*;

use crate::crew::*;
use crate::terminal::*;
use crate::yard::*;

#[derive(Clone, Debug)]
pub struct Dispatcher {
    pub yard: Yard,
    pub next_inbound_at: f64,
    pub inbound_interval: f64,
    pub min_departure_cars: usize,
    /// One road crew at a time.
    pub road: Option<Crew>,
    pub road_is_outbound: bool,
    pub inbound_count: u32,
    pub outbound_count: u32,
    pub cars_delivered: u32,
    /// Payload tonnage that left on outbound trains, kg.
    pub cargo_delivered: f64,
    pub log: Vec<(f64, String)>,
    rng: u64,
}

impl Dispatcher {
    pub fn new(yard: Yard) -> Self {
        Dispatcher {
            yard,
            next_inbound_at: 30.0,
            inbound_interval: 90.0 * 60.0,
            min_departure_cars: 8,
            road: None,
            road_is_outbound: false,
            inbound_count: 0,
            outbound_count: 0,
            cars_delivered: 0,
            cargo_delivered: 0.0,
            log: Vec::new(),
            rng: 0x5EED_7A11,
        }
    }

    fn rnd(&mut self) -> u64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.rng >> 33
    }

    fn note(&mut self, t: f64, s: impl Into<String>) {
        self.log.push((t, s.into()));
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }

    pub fn step(&mut self, world: &mut World, yard_crew: &mut Crew, events: &[SimEvent], dt: f64) {
        let t = world.t;
        let dep = self.yard.departure.unwrap_or(1);

        // Finished cars go to the departure track.
        for e in events {
            match e {
                SimEvent::Loaded { car, .. } | SimEvent::Unloaded { car, .. } => {
                    if let Some((ti, ci)) = world.train_of_car(*car) {
                        world.trains[ti].cars[ci].dest = Some(dep);
                    }
                }
                _ => {}
            }
        }

        // Road crew.
        if let Some(road) = self.road.as_mut() {
            road.step(world, dt);
            let leaving = matches!(road.road_phase(), Some(RoadPhase::Leaving) | Some(RoadPhase::Done));
            let tid = road.train_id(world);
            let at_portal = tid
                .and_then(|id| world.train(id))
                .and_then(|tr| {
                    let leading = if tr.speed(&world.car_types) >= 0.0 { End::Head } else { End::Tail };
                    let x = world.face_x(tr, leading);
                    tr.locate(&world.graph, x).map(|l| world.graph.edge(l.edge).track == Some(TRACK_PORTAL))
                })
                .unwrap_or(false);
            let failed = matches!(road.status, Status::Failed(_));
            if failed {
                let why = road.status.clone();
                self.note(t, format!("Road crew stuck: {why:?}"));
                self.road = None;
            } else if leaving && at_portal {
                if let Some(id) = tid {
                    if let Some(tr) = world.remove_train(id) {
                        let types = &world.car_types;
                        let cars = tr.cars.iter().filter(|c| !types[c.type_id as usize].is_loco()).count();
                        let payload: f64 = tr.cars.iter().map(|c| c.m_payload).sum();
                        if self.road_is_outbound {
                            self.cars_delivered += cars as u32;
                            self.cargo_delivered += payload;
                            self.note(t, format!("Outbound left with {cars} cars, {:.0} t of cargo.", payload / 1000.0));
                        } else {
                            self.note(t, "Inbound road engine has left.");
                        }
                    }
                }
                self.road = None;
            }
        }

        // Yard master.
        if self.road.is_none() && yard_crew.is_idle() {
            if let Some(job) = self.next_yard_job(world) {
                let what = match &job {
                    Program::Hump(_) => "Hump the receiving cut.".to_string(),
                    Program::Sort(j) => format!("Work the cut on {}.", j.source_track.map(|tk| self.yard.track_name(tk)).unwrap_or_default()),
                    _ => "Idle".to_string(),
                };
                self.note(t, format!("Yard crew: {what}"));
                yard_crew.assign(job, t);
                yard_crew.say(t, what);
            }
        }

        // Road traffic, only while the yard crew is out of the way.
        if self.road.is_none() && yard_crew.is_idle() {
            let ready = self.departure_ready(world);
            if ready >= self.min_departure_cars {
                self.spawn_outbound(world, t);
            } else if t >= self.next_inbound_at && cuts_on_track(world, TRACK_RECEIVING).is_empty() {
                self.spawn_inbound(world, t);
                self.next_inbound_at = t + self.inbound_interval;
            }
        }
    }

    fn departure_ready(&self, world: &World) -> usize {
        let dep = self.yard.departure.unwrap_or(1);
        cuts_on_track(world, dep).iter().filter_map(|id| world.train(*id)).map(|tr| tr.cars.iter().filter(|c| c.dest == Some(dep)).count()).sum()
    }

    fn next_yard_job(&self, world: &World) -> Option<Program> {
        if !cuts_on_track(world, TRACK_RECEIVING).is_empty() && self.yard.hump.is_some() {
            return Some(Crew::hump_job(&self.yard));
        }
        let dep = self.yard.departure.unwrap_or(1);
        for bowl in BOWL_BASE + 1..=BOWL_BASE + 4 {
            let cuts = cuts_on_track(world, bowl);
            if cuts.iter().filter_map(|id| world.train(*id)).any(|tr| tr.cars.iter().any(|c| c.dest.is_some())) {
                return Some(Crew::sort_job(&self.yard, Some(bowl)));
            }
        }
        for tk in [TRACK_LOADER, TRACK_DUMPER] {
            let cuts = cuts_on_track(world, tk);
            if cuts.iter().filter_map(|id| world.train(*id)).any(|tr| tr.cars.iter().any(|c| c.dest == Some(dep))) {
                return Some(Crew::sort_job(&self.yard, Some(tk)));
            }
        }
        None
    }

    fn portal_spawn(&self, world: &mut World, cars: Vec<CarState>) -> Option<TrainId> {
        let pe = portal_edge(&world.graph, &self.yard)?;
        world.spawn_train(cars, pe, 560.0, true).ok()
    }

    fn spawn_inbound(&mut self, world: &mut World, t: f64) {
        let n = 14 + (self.rnd() % 7) as usize;
        let mut cars = Vec::with_capacity(n + 1);
        let mut l = world.new_car(TYPE_SWITCHER);
        l.brake = Brake::charged();
        l.knuckle_open = [true, true];
        cars.push(l);
        for _ in 0..n {
            let r = self.rnd() % 10;
            let mut c = match r {
                0..=3 => {
                    let mut c = world.new_car(TYPE_OPEN_HOPPER);
                    c.dest = Some(TRACK_LOADER);
                    c
                }
                4..=6 => {
                    let mut c = world.new_car(TYPE_OPEN_HOPPER);
                    c.m_payload = 100_000.0;
                    c.commodity = Commodity::Coal;
                    c.dest = Some(TRACK_DUMPER);
                    c
                }
                7 => {
                    let mut c = world.new_car(TYPE_BOXCAR);
                    c.m_payload = 30_000.0;
                    c.commodity = Commodity::Mixed;
                    c.dest = Some(TRACK_DEPARTURE);
                    c
                }
                8 => {
                    let mut c = world.new_car(TYPE_TANK);
                    c.m_payload = 90_000.0;
                    c.commodity = Commodity::Oil;
                    c.dest = Some(TRACK_DEPARTURE);
                    c
                }
                _ => {
                    let mut c = world.new_car(TYPE_GONDOLA);
                    c.m_payload = 80_000.0;
                    c.commodity = Commodity::Aggregate;
                    c.dest = Some(TRACK_DEPARTURE);
                    c
                }
            };
            c.brake = Brake::charged();
            c.knuckle_open = [true, true];
            cars.push(c);
        }
        if let Some(id) = self.portal_spawn(world, cars) {
            if let Some(tr) = world.train_mut(id) {
                for h in &mut tr.hoses {
                    *h = true;
                }
            }
            let loco_car = world.train(id).unwrap().cars[0].id;
            let crew = Crew::new("Road", loco_car, Program::RoadIn(RoadJob { yard: self.yard.clone(), phase: RoadPhase::Lining }), t);
            self.road = Some(crew);
            self.road_is_outbound = false;
            self.inbound_count += 1;
            self.note(t, format!("Inbound #{} arriving with {n} cars.", self.inbound_count));
        }
    }

    fn spawn_outbound(&mut self, world: &mut World, t: f64) {
        let mut l = world.new_car(TYPE_SWITCHER);
        l.brake = Brake::charged();
        l.knuckle_open = [true, true];
        if let Some(id) = self.portal_spawn(world, vec![l]) {
            let loco_car = world.train(id).unwrap().cars[0].id;
            let crew = Crew::new("Road", loco_car, Program::RoadOut(RoadJob { yard: self.yard.clone(), phase: RoadPhase::Lining }), t);
            self.road = Some(crew);
            self.road_is_outbound = true;
            self.outbound_count += 1;
            self.note(t, format!("Road engine coming for outbound #{}.", self.outbound_count));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::*;

    #[test]
    fn terminal_traffic_runs_unaided() {
        let sc = terminal();
        let Built { mut world, loco, mut dispatcher, initial_program, .. } = build(&sc);
        let mut dispatcher = dispatcher.take().unwrap();
        let loco_car = world.train(loco).unwrap().cars[0].id;
        let mut crew = Crew::new("R. Casey", loco_car, initial_program, 0.0);
        let mut events: Vec<SimEvent> = Vec::new();
        let mut derails = 0;
        let mut hard = 0;
        let mut last_report = 0.0;
        while world.t < 6.0 * 3600.0 {
            dispatcher.step(&mut world, &mut crew, &events, DT);
            crew.step(&mut world, DT);
            world.step(DT);
            events = world.take_events();
            for e in &events {
                match e {
                    SimEvent::Derail { .. } => derails += 1,
                    SimEvent::Coupled { hard: is_hard, rel_speed, train, a, b, pos } => {
                        if *is_hard {
                            hard += 1;
                        }
                        if *is_hard {
                            eprintln!("  HARD joint [{:5.1} min] cars #{a} and #{b} at {:.2} m/s -> train {train} at ({:.0}, {:.1})", world.t / 60.0, rel_speed, pos.x, pos.y);
                        }
                    }
                    _ => {}
                }
            }
            if world.t - last_report > 600.0 {
                last_report = world.t;
                eprintln!("[{:5.0} min] yard: {} | road: {} | trains {} | in {} out {} delivered {} cars {:.0} t | loaded {:.0} t unloaded {:.0} t",
                    world.t / 60.0, crew.describe(), dispatcher.road.as_ref().map(|r| r.describe()).unwrap_or_else(|| "-".into()), world.trains.len(),
                    dispatcher.inbound_count, dispatcher.outbound_count, dispatcher.cars_delivered, dispatcher.cargo_delivered / 1000.0, world.cargo_loaded / 1000.0, world.cargo_unloaded / 1000.0);
            }
            if matches!(crew.status, Status::Failed(_)) {
                break;
            }
            if dispatcher.outbound_count >= 1 && dispatcher.cars_delivered > 0 {
                break;
            }
        }
        for (t, l) in dispatcher.log.iter() {
            eprintln!("  dispatch [{:5.0} min] {l}", t / 60.0);
        }
        for (t, l) in crew.radio.iter().rev().take(24).collect::<Vec<_>>().into_iter().rev() {
            eprintln!("  crew     [{:5.0} min] {l}", t / 60.0);
        }
        eprintln!("derails {derails} hard joints {hard} t={:.0} min", world.t / 60.0);
        if matches!(crew.status, Status::Failed(_)) {
            eprintln!("{}", crew.debug_state(&world));
        }
        assert!(!matches!(crew.status, Status::Failed(_)), "yard crew stuck: {:?}", crew.status);
        assert_eq!(derails, 0);
        assert!(dispatcher.inbound_count >= 1);
        assert!(world.cargo_loaded > 0.0 || world.cargo_unloaded > 0.0, "facilities should have worked cars");
        assert!(dispatcher.cars_delivered > 0, "an outbound should have left with cars");
    }
}
