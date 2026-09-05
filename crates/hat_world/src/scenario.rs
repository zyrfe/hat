//! Scenarios: what is in the yard when the shift starts, and what done means.

use hat_sim::*;

use crate::yard::*;

#[derive(Clone, Debug)]
pub struct CarSpec {
    pub type_id: CarTypeId,
    pub payload: f64,
    pub commodity: Commodity,
    pub dest: u32,
    pub hand_brake: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioKind {
    /// Sort a bled cut to its tracks and secure it.
    Sort,
    /// Double a charged road train into the yard.
    Double,
}

#[derive(Clone, Debug)]
pub struct Scenario {
    pub name: &'static str,
    pub kind: ScenarioKind,
    pub description: &'static str,
    pub yard: YardParams,
    pub inbound: Vec<CarSpec>,
    /// Tail of the inbound cut sits here on the east main, m east of the lead switch.
    pub inbound_tail_offset: f64,
    /// Locomotive head face sits here on the west main, m west of the lead switch.
    pub loco_offset: f64,
    pub inbound_charged: bool,
    /// Seconds. Points bleed after this.
    pub time_budget: f64,
}

/// Minimal deterministic generator so scenarios are the same every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

fn car_for_track(track: u32, rng: &mut Lcg) -> CarSpec {
    let (type_id, commodity, payload) = match track {
        1 => match rng.below(3) {
            0 => (TYPE_BOXCAR, Commodity::Mixed, 40_000.0),
            1 => (TYPE_COVERED_HOPPER, Commodity::Grain, 95_000.0),
            _ => (TYPE_TANK, Commodity::Oil, 90_000.0),
        },
        2 => (TYPE_OPEN_HOPPER, Commodity::Coal, if rng.below(4) == 0 { 0.0 } else { 100_000.0 }),
        3 => (TYPE_COVERED_HOPPER, Commodity::Grain, if rng.below(4) == 0 { 0.0 } else { 95_000.0 }),
        4 => (TYPE_TANK, Commodity::Oil, if rng.below(3) == 0 { 0.0 } else { 90_000.0 }),
        5 => {
            if rng.below(2) == 0 {
                (TYPE_BOXCAR, Commodity::Mixed, 30_000.0)
            } else {
                (TYPE_GONDOLA, Commodity::Aggregate, 90_000.0)
            }
        }
        _ => (TYPE_FLATCAR, Commodity::Lumber, if rng.below(3) == 0 { 0.0 } else { 50_000.0 }),
    };
    let commodity = if payload == 0.0 { Commodity::Empty } else { commodity };
    CarSpec { type_id, payload, commodity, dest: track, hand_brake: false }
}

pub fn yard_shift() -> Scenario {
    let mut rng = Lcg(0x5EED_0001);
    let mut inbound = Vec::new();
    let per_track = [(1u32, 8usize), (2, 6), (3, 5), (4, 4), (5, 4), (6, 3)];
    for (track, n) in per_track {
        for _ in 0..n {
            inbound.push(car_for_track(track, &mut rng));
        }
    }
    rng.shuffle(&mut inbound);
    for i in [0usize, 9, 19, 29] {
        if let Some(c) = inbound.get_mut(i) {
            c.hand_brake = true;
        }
    }
    Scenario {
        name: "Yard Shift",
        kind: ScenarioKind::Sort,
        description: "A bled 30-car cut is on the main east of the lead. Sort every car to its track and tie it down. Track 1 is the departure track.",
        yard: YardParams::default(),
        inbound,
        inbound_tail_offset: 250.0,
        loco_offset: 200.0,
        inbound_charged: false,
        time_budget: 45.0 * 60.0,
    }
}

pub fn doubling() -> Scenario {
    let mut rng = Lcg(0x5EED_0002);
    let mut inbound = Vec::new();
    for i in 0..150usize {
        let track = (i / 30) as u32 + 1;
        inbound.push(car_for_track(track.min(6), &mut rng));
    }
    for i in (0..150).step_by(15) {
        inbound[i].hand_brake = true;
    }
    Scenario {
        name: "Doubling",
        kind: ScenarioKind::Double,
        description: "A charged 150-car road train is on the main. No yard track holds it. Cut it into blocks and double it into tracks 1 to 5, thirty cars each.",
        yard: YardParams::default(),
        inbound,
        inbound_tail_offset: 250.0,
        loco_offset: 200.0,
        inbound_charged: true,
        time_budget: 90.0 * 60.0,
    }
}

pub fn all_scenarios() -> Vec<Scenario> {
    vec![yard_shift(), doubling()]
}

/// The built world plus what the app needs to know about it.
pub struct Built {
    pub world: World,
    pub yard: Yard,
    pub loco: TrainId,
    pub inbound: TrainId,
}

pub fn build(sc: &Scenario) -> Built {
    let (graph, yard) = build_ladder_yard(&sc.yard);
    let mut world = World::new(graph);

    let mut cars = Vec::with_capacity(sc.inbound.len());
    for spec in &sc.inbound {
        let mut c = world.new_car(spec.type_id);
        c.m_payload = spec.payload;
        c.commodity = spec.commodity;
        c.dest = Some(spec.dest);
        c.hand_brake = if spec.hand_brake { 1.0 } else { 0.0 };
        c.brake = if sc.inbound_charged { Brake::charged() } else { Brake::bled() };
        c.knuckle_open = [true, true];
        cars.push(c);
    }
    let cut_len: f64 = cars.iter().map(|c| world.car_types[c.type_id as usize].length).sum();
    let head_s = sc.inbound_tail_offset + cut_len;
    let inbound = world.spawn_train(cars, yard.main_east, head_s, true).expect("inbound cut fits on the east main");
    if sc.inbound_charged {
        let tr = world.train_mut(inbound).unwrap();
        for h in &mut tr.hoses {
            *h = true;
        }
    }

    let mut l = world.new_car(TYPE_SWITCHER);
    l.brake = Brake::charged();
    l.knuckle_open = [true, true];
    let west_len = world.graph.edge(yard.main_west).length;
    let loco = world.spawn_train(vec![l], yard.main_west, west_len - sc.loco_offset, true).expect("locomotive fits on the west main");
    world.set_controls(loco, Controls { reverser: Reverser::Forward, independent: 1.0, ..Default::default() });

    Built { world, yard, loco, inbound }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenarios_build_and_fit() {
        for sc in all_scenarios() {
            let b = build(&sc);
            assert_eq!(b.world.trains.len(), 2, "{}", sc.name);
            let inbound = b.world.train(b.inbound).unwrap();
            assert_eq!(inbound.cars.len(), sc.inbound.len());
            let loco = b.world.train(b.loco).unwrap();
            assert!(b.world.car_types[loco.cars[0].type_id as usize].is_loco());
            let head = b.world.car_location(inbound, 0).unwrap();
            assert_eq!(b.world.graph.edge(head.edge).track, Some(TRACK_MAIN));
        }
    }

    #[test]
    fn scenario_generation_is_deterministic() {
        let a: Vec<(u16, u32)> = yard_shift().inbound.iter().map(|c| (c.type_id, c.dest)).collect();
        let b: Vec<(u16, u32)> = yard_shift().inbound.iter().map(|c| (c.type_id, c.dest)).collect();
        assert_eq!(a, b);
    }
}

#[cfg(test)]
mod playthrough {
    //! A scripted crew works the Yard Shift: couple to the cut, pull it clear of the lead,
    //! line the switches for track 1, shove, and kick the first car in.

    use super::*;

    fn step(w: &mut World, n: usize) {
        for _ in 0..n {
            w.step(DT);
        }
    }

    fn loco_train(w: &World) -> &Train {
        w.trains.iter().find(|t| t.control_car(&w.car_types).is_some()).unwrap()
    }

    fn drive(w: &mut World, id: TrainId, reverser: Reverser, throttle: u8, independent: f64) {
        w.set_controls(id, Controls { throttle, reverser, independent, ..Default::default() });
    }

    /// Head of the locomotive's train in world x, and the west end of the other train.
    fn east_x(w: &World, t: &Train) -> f64 {
        (0..t.cars.len()).filter_map(|i| w.car_pose(t, i)).map(|p| p.pos.x).fold(f64::MIN, f64::max)
    }

    fn west_x(w: &World, t: &Train) -> f64 {
        (0..t.cars.len()).filter_map(|i| w.car_pose(t, i)).map(|p| p.pos.x).fold(f64::MAX, f64::min)
    }

    #[test]
    fn crew_couples_pulls_clear_and_kicks_a_car_into_track_one() {
        let sc = yard_shift();
        let Built { mut world, yard, loco, inbound } = build(&sc);

        // 1. Ease east onto the cut and couple under 1.8 m/s.
        let mut coupled = false;
        for _ in 0..(600.0 / DT) as usize {
            let (gap, v) = {
                let l = world.train(loco).unwrap();
                let c = world.train(inbound).unwrap();
                (west_x(&world, c) - east_x(&world, l) - 15.0, l.speed(&world.car_types))
            };
            let target = (2.0 * 0.12 * gap.max(0.0)).sqrt().clamp(0.6, 5.0);
            if v < target {
                drive(&mut world, loco, Reverser::Forward, 2, 0.0);
            } else {
                drive(&mut world, loco, Reverser::Forward, 0, 0.6);
            }
            step(&mut world, 12);
            if world.trains.len() == 1 {
                coupled = true;
                break;
            }
        }
        assert!(coupled, "never coupled to the cut");
        assert!(world.events.iter().all(|e| !matches!(e, SimEvent::Coupled { hard: true, .. })), "coupling was hard");
        let id = world.trains[0].id;
        let n = world.trains[0].cars.len();
        assert_eq!(n, sc.inbound.len() + 1);

        // 2. Release every hand brake the road crew left.
        for i in 0..n {
            world.set_hand_brake(id, i, false).unwrap();
        }

        // 3. Pull west until the whole train is clear of the lead switch.
        let lead_x = world.graph.node(yard.lead_switch).pos.x;
        let mut clear = false;
        for _ in 0..(900.0 / DT) as usize {
            let (east, v) = {
                let t = world.train(id).unwrap();
                (east_x(&world, t), t.speed(&world.car_types).abs())
            };
            let remaining = east - (lead_x - 25.0);
            if remaining <= 0.0 && v < 0.05 {
                clear = true;
                break;
            }
            let target = if remaining <= 0.0 { 0.0 } else { (2.0 * 0.05 * remaining).sqrt().clamp(0.5, 4.0) };
            if v < target {
                drive(&mut world, id, Reverser::Reverse, 3, 0.0);
            } else {
                drive(&mut world, id, Reverser::Neutral, 0, 1.0);
            }
            step(&mut world, 12);
        }
        assert!(clear, "could not pull the cut clear of the lead");

        // 4. Line the ladder for track 1.
        for (node, route) in yard.route_to(1).unwrap() {
            if world.graph.turnout_setting(node) != Some(route) {
                world.throw_switch(node).expect("switch free to throw");
            }
        }

        // 5. Shove east at a steady 2.5 m/s. When the east-most car is fully on track 1,
        //    pull the pin behind it and stop the rest with the independent.
        let east_car_id = {
            let t = world.train(id).unwrap();
            (0..t.cars.len()).max_by(|&a, &b| world.car_pose(t, a).unwrap().pos.x.total_cmp(&world.car_pose(t, b).unwrap().pos.x)).map(|i| t.cars[i].id).unwrap()
        };
        let mut kicked: Option<TrainId> = None;
        for _ in 0..(600.0 / DT) as usize {
            let (v, kick_at) = {
                let t = world.train(id).unwrap();
                let i = t.cars.iter().position(|c| c.id == east_car_id).unwrap();
                let k = if i == 0 { 1 } else { i };
                let coupler_on_track_one = t.locate(&world.graph, t.coupler_x(&world.car_types, k)).map(|l| world.graph.edge(l.edge).track == Some(1)).unwrap_or(false);
                (t.speed(&world.car_types).abs(), coupler_on_track_one.then_some(k))
            };
            if let Some(k) = kick_at {
                let new = world.pull_pin(id, k).expect("pin pulls while shoving");
                // The head part keeps the old id, so the single car may be either side.
                let (car_id, rest_id) = if world.train(new).map(|t| t.cars.len()) == Some(1) { (new, id) } else { (id, new) };
                kicked = Some(car_id);
                drive(&mut world, rest_id, Reverser::Neutral, 0, 1.0);
                break;
            }
            if v < 2.5 {
                drive(&mut world, id, Reverser::Forward, 3, 0.0);
            } else {
                drive(&mut world, id, Reverser::Forward, 0, 0.0);
            }
            step(&mut world, 6);
        }
        let kicked = kicked.expect("never kicked a car");

        // 6. Let everything roll out.
        step(&mut world, (400.0 / DT) as usize);
        let car = world.train(kicked).expect("kicked car exists as its own train");
        assert_eq!(car.cars.len(), 1);
        assert!(!car.derailed);
        assert!(car.cars[0].v.abs() < 1e-6, "kicked car should have stopped, v = {}", car.cars[0].v);
        let loc = world.car_location(car, 0).unwrap();
        assert_eq!(world.graph.edge(loc.edge).track, Some(1), "kicked car should rest on track 1");
        assert!(world.events.iter().all(|e| !matches!(e, SimEvent::Derail { .. })), "no derailments expected: {:?}", world.events.iter().filter(|e| matches!(e, SimEvent::Derail { .. })).collect::<Vec<_>>());
        assert!(!loco_train(&world).derailed);

        // 7. Tie it down and see it count.
        let (dest, car_id, rest_s) = (car.cars[0].dest, car.cars[0].id, loc.s);
        world.set_hand_brake(kicked, 0, true).unwrap();
        let mut score = crate::Score::default();
        score.evaluate(&world, &yard, sc.time_budget);
        if dest == Some(1) {
            assert!(score.cars_secured >= 1);
        }
        eprintln!("kicked car #{car_id} (dest {dest:?}) rests on track 1 at s={rest_s:.0}; t={:.0}s; events={}", world.t, world.events.len());
    }
}
