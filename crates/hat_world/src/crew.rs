//! Crews: a program of maneuvers that drives one locomotive through the sim.
//!
//! The player writes the switch list (car destinations); the crew's program turns it into
//! maneuvers and the executor drives. Every rule the player obeys, the crew obeys.
//! See docs/crew.md.

use std::collections::VecDeque;

use hat_sim::params::*;
use hat_sim::*;
use hat_units::G;

use crate::yard::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Running,
    Done,
    Failed(String),
}

/// Where a drive should stop.
#[derive(Clone, Debug)]
pub enum Target {
    /// `face` ends up `beyond` meters past `node` in the direction of travel.
    PastNode { node: NodeId, face: End, beyond: f64 },
    /// Leading face stops `gap` meters short of the next train ahead (0 to couple).
    ShortOfTrain { gap: f64 },
    /// The coupler between cars `a` and `b` ends up `depth` meters along `edge` from `from_node`.
    CouplerOnEdge { a: CarId, b: CarId, edge: EdgeId, from_node: NodeId, depth: f64 },
    /// Travel a fixed distance.
    Distance { total: f64, done: f64 },
}

#[derive(Clone, Debug)]
pub struct Drive {
    /// Direction of travel in the train frame.
    pub forward: bool,
    pub target: Target,
    pub v_end: f64,
    pub vmax: f64,
    /// Approach anything ahead gently enough to couple to it.
    pub join: bool,
    /// Finish when the train couples to something.
    pub stop_on_couple: bool,
    cars_at_start: Option<usize>,
    started: Option<f64>,
    last_x: Option<f64>,
}

impl Drive {
    pub fn new(forward: bool, target: Target, v_end: f64, vmax: f64) -> Self {
        Drive { forward, target, v_end, vmax, join: false, stop_on_couple: false, cars_at_start: None, started: None, last_x: None }
    }
    /// Approach to couple and finish on the joint.
    pub fn coupling(mut self) -> Self {
        self.join = true;
        self.stop_on_couple = true;
        self
    }
    /// Approach to couple, then keep going to the target.
    pub fn joining(mut self) -> Self {
        self.join = true;
        self
    }
}

#[derive(Clone, Debug)]
pub enum Maneuver {
    Route { settings: Vec<(NodeId, Route)>, since: Option<f64> },
    Drive(Drive),
    PullPin { k: usize, tries: u32, bunching_until: Option<f64> },
    HandBrakes { indices: Vec<usize>, on: bool },
    Wait { until: f64 },
    LaceHoses,
}

/// What a crew is trying to accomplish over many maneuvers.
#[derive(Clone, Debug)]
pub enum Program {
    Idle,
    /// Flat-switch the standing cut onto its destination tracks, then park.
    Sort { yard: Yard, phase: SortPhase },
}

#[derive(Clone, Debug)]
pub enum SortPhase {
    Start,
    Coupling { cut: TrainId },
    Releasing,
    Pick,
    Clearing { track: u32, block: usize },
    Routing { track: u32, block: usize },
    Shoving { track: u32, block: usize },
    Tying { track: u32 },
    Cutting { track: u32 },
    Parking,
    Done,
}

#[derive(Clone, Debug)]
pub struct Crew {
    pub name: &'static str,
    pub loco_car: CarId,
    pub program: Program,
    pub current: Option<Maneuver>,
    pub paused: bool,
    pub status: Status,
    pub radio: VecDeque<(f64, String)>,
    pub deliveries: u32,
    pub started_at: f64,
    pub finished_at: Option<f64>,
}

impl Crew {
    pub fn new(name: &'static str, loco_car: CarId, program: Program, t: f64) -> Self {
        Crew { name, loco_car, program, current: None, paused: false, status: Status::Running, radio: VecDeque::new(), deliveries: 0, started_at: t, finished_at: None }
    }

    pub fn say(&mut self, t: f64, line: impl Into<String>) {
        self.radio.push_back((t, line.into()));
        while self.radio.len() > 40 {
            self.radio.pop_front();
        }
    }

    /// Train index and car index of the locomotive this crew rides.
    pub fn position(&self, world: &World) -> Option<(usize, usize)> {
        world.train_of_car(self.loco_car)
    }

    pub fn train_id(&self, world: &World) -> Option<TrainId> {
        self.position(world).map(|(ti, _)| world.trains[ti].id)
    }

    pub fn describe(&self) -> String {
        match &self.current {
            None => match &self.program {
                Program::Idle => "idle".into(),
                Program::Sort { phase, .. } => format!("{phase:?}"),
            },
            Some(Maneuver::Route { .. }) => "lining switches".into(),
            Some(Maneuver::Drive(d)) => match &d.target {
                Target::PastNode { .. } => if d.v_end > 0.5 { "running".into() } else { "moving to clear".into() },
                Target::ShortOfTrain { .. } => "approaching to couple".into(),
                Target::CouplerOnEdge { .. } => "shoving in".into(),
                Target::Distance { .. } => "moving".into(),
            },
            Some(Maneuver::PullPin { .. }) => "pulling the pin".into(),
            Some(Maneuver::HandBrakes { on, .. }) => if *on { "tying down".into() } else { "releasing hand brakes".into() },
            Some(Maneuver::Wait { .. }) => "waiting".into(),
            Some(Maneuver::LaceHoses) => "lacing hoses".into(),
        }
    }

    /// Blocks the sort will deliver, in order, from the current consist.
    pub fn plan_preview(&self, world: &World) -> Vec<(u32, usize)> {
        let Some((ti, li)) = self.position(world) else { return vec![] };
        let tr = &world.trains[ti];
        let mut out = Vec::new();
        let n = tr.cars.len();
        if n <= 1 {
            return out;
        }
        // Cars from the far end toward the locomotive.
        let order: Vec<&CarState> = if li == 0 { tr.cars.iter().rev().collect() } else { tr.cars.iter().collect() };
        let mut i = 0;
        while i < order.len() {
            let Some(d) = order[i].dest else { i += 1; continue };
            let mut j = i;
            while j < order.len() && order[j].dest == Some(d) {
                j += 1;
            }
            out.push((d, j - i));
            i = j;
        }
        out
    }

    /// Advance the crew by one sim step. Sets the controls of the crew's train.
    pub fn step(&mut self, world: &mut World, dt: f64) {
        if self.paused || self.status != Status::Running {
            return;
        }
        let t = world.t;
        let Some(id) = self.train_id(world) else {
            self.status = Status::Failed("locomotive is gone".into());
            return;
        };
        if self.current.is_none() {
            self.current = self.next_maneuver(world);
            if self.current.is_none() {
                return;
            }
        }
        let mut maneuver = self.current.take().unwrap();
        let status = self.run_maneuver(&mut maneuver, world, id, dt);
        match status {
            Status::Running => self.current = Some(maneuver),
            Status::Done => {
                self.current = None;
                self.advance(world, t);
            }
            Status::Failed(why) => {
                self.say(t, format!("Can't do it: {why}"));
                self.status = Status::Failed(why);
                let _ = world.set_controls(id, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
            }
        }
    }

    fn run_maneuver(&mut self, m: &mut Maneuver, world: &mut World, id: TrainId, dt: f64) -> Status {
        let t = world.t;
        match m {
            Maneuver::Route { settings, since } => {
                let started = *since.get_or_insert(t);
                for (node, route) in settings.iter() {
                    if world.graph.turnout_setting(*node) != Some(*route) {
                        match world.throw_switch(*node) {
                            Ok(_) => {}
                            Err(_) => {
                                if t - started > 180.0 {
                                    return Status::Failed(format!("{} stays occupied", world.graph.node(*node).name));
                                }
                                hold(world, id);
                                return Status::Running;
                            }
                        }
                    }
                }
                Status::Done
            }
            Maneuver::Drive(d) => drive(d, world, id, dt),
            Maneuver::PullPin { k, tries, bunching_until } => {
                if let Some(until) = *bunching_until {
                    if t < until {
                        return Status::Running;
                    }
                    *bunching_until = None;
                    hold(world, id);
                }
                match world.pull_pin(id, *k) {
                    Ok(_) => Status::Done,
                    Err(_) if *tries < 6 => {
                        *tries += 1;
                        // Bunch the slack: a gentle shove toward the far end for a moment.
                        let tr = world.train(id).unwrap();
                        let li = tr.control_car(&world.car_types).unwrap_or(0);
                        let toward_far = li == 0;
                        let rev = if toward_far { Reverser::Reverse } else { Reverser::Forward };
                        world.set_controls(id, Controls { throttle: 1, reverser: rev, independent: 0.0, ..Default::default() });
                        *bunching_until = Some(t + 1.5);
                        Status::Running
                    }
                    Err(e) => Status::Failed(e.to_string()),
                }
            }
            Maneuver::HandBrakes { indices, on } => {
                for &i in indices.iter() {
                    let _ = world.set_hand_brake(id, i, *on);
                }
                Status::Done
            }
            Maneuver::Wait { until } => {
                hold(world, id);
                if t >= *until { Status::Done } else { Status::Running }
            }
            Maneuver::LaceHoses => {
                let _ = world.connect_hoses(id);
                Status::Done
            }
        }
    }

    /// Called when a maneuver completes: move the program forward.
    fn advance(&mut self, world: &World, t: f64) {
        let Program::Sort { phase, .. } = &mut self.program else { return };
        let next = match phase.clone() {
            SortPhase::Start => SortPhase::Start,
            SortPhase::Coupling { .. } => {
                let n = self.position(world).map(|(ti, _)| world.trains[ti].cars.len()).unwrap_or(0);
                self.say(t, format!("Coupled, {} cars.", n.saturating_sub(1)));
                SortPhase::Releasing
            }
            SortPhase::Releasing => {
                self.say(t, "Brakes off. Let's sort them.");
                SortPhase::Pick
            }
            SortPhase::Pick => SortPhase::Pick,
            SortPhase::Clearing { track, block } => SortPhase::Routing { track, block },
            SortPhase::Routing { track, block } => {
                self.say(t, format!("Lined for Track {track}. Shoving {block}."));
                SortPhase::Shoving { track, block }
            }
            SortPhase::Shoving { track, .. } => SortPhase::Tying { track },
            SortPhase::Tying { track } => SortPhase::Cutting { track },
            SortPhase::Cutting { track } => {
                self.deliveries += 1;
                self.say(t, format!("Cut. Track {track} done for now."));
                SortPhase::Pick
            }
            SortPhase::Parking => {
                self.say(t, "Shift complete. Engine parked.");
                self.finished_at = Some(t);
                self.status = Status::Done;
                SortPhase::Done
            }
            SortPhase::Done => SortPhase::Done,
        };
        if let Program::Sort { phase, .. } = &mut self.program {
            *phase = next;
        }
    }

    /// Produce the next maneuver for the current program phase.
    fn next_maneuver(&mut self, world: &World) -> Option<Maneuver> {
        let t = world.t;
        let (ti, li) = self.position(world)?;
        let tr = &world.trains[ti];
        let n = tr.cars.len();
        let types = &world.car_types;
        let program = self.program.clone();
        match program {
            Program::Idle => None,
            Program::Sort { yard, phase } => match phase {
                SortPhase::Start => {
                    // Find the cut: the nearest train without a locomotive.
                    let target = world.trains.iter().filter(|o| o.id != tr.id && o.control_car(types).is_none()).map(|o| o.id).next()?;
                    let forward = world.distance_to_train_ahead(tr, true, 5000.0).map(|(_, id)| id == target).unwrap_or(false);
                    let backward = world.distance_to_train_ahead(tr, false, 5000.0).map(|(_, id)| id == target).unwrap_or(false);
                    if !forward && !backward {
                        self.status = Status::Failed("no clear path to the cut".into());
                        return None;
                    }
                    self.say(t, "Heading for the cut.");
                    self.set_phase(SortPhase::Coupling { cut: target });
                    Some(Maneuver::Drive(Drive::new(forward, Target::ShortOfTrain { gap: 0.0 }, 1.2, 5.0).coupling()))
                }
                SortPhase::Coupling { .. } => None,
                SortPhase::Releasing => Some(Maneuver::HandBrakes { indices: (0..n).collect(), on: false }),
                SortPhase::Pick => {
                    let blocks = self.plan_preview(world);
                    match blocks.first() {
                        None => {
                            self.set_phase(SortPhase::Parking);
                            let far = if li == 0 { End::Tail } else { End::Head };
                            let toward_loco = li == 0;
                            Some(Maneuver::Drive(Drive::new(toward_loco, Target::PastNode { node: yard.lead_switch, face: far, beyond: 60.0 }, 0.0, 5.0)))
                        }
                        Some(&(track, block)) => {
                            self.say(t, format!("Next: {block} for Track {track}. Pulling clear."));
                            self.set_phase(SortPhase::Clearing { track, block });
                            // Pull toward the locomotive end until the far face is clear of the lead switch.
                            let far = if li == 0 { End::Tail } else { End::Head };
                            let toward_loco = li == 0;
                            Some(Maneuver::Drive(Drive::new(toward_loco, Target::PastNode { node: yard.lead_switch, face: far, beyond: 25.0 }, 0.0, 5.0)))
                        }
                    }
                }
                SortPhase::Clearing { .. } => None,
                SortPhase::Routing { track, .. } => Some(Maneuver::Route { settings: yard.route_to(track)?, since: None }),
                SortPhase::Shoving { track, block } => {
                    let yt = yard.track(track)?;
                    let straight = yt.edges[1];
                    let from_node = world.graph.edge(straight).a;
                    let k = if li == 0 { n - block } else { block };
                    let (a, b) = (tr.cars[k - 1].id, tr.cars[k].id);
                    let away = li != 0;
                    Some(Maneuver::Drive(Drive::new(away, Target::CouplerOnEdge { a, b, edge: straight, from_node, depth: 20.0 }, 0.0, 3.0).joining()))
                }
                SortPhase::Tying { track } => {
                    // The block is the far-end run of cars destined here (grown by any cars we joined).
                    let far_indices: Vec<usize> = if li == 0 { (0..n).rev().collect() } else { (0..n).collect() };
                    let mut block: Vec<usize> = Vec::new();
                    for &i in &far_indices {
                        if tr.cars[i].dest == Some(track) && i != li {
                            block.push(i);
                        } else {
                            break;
                        }
                    }
                    let need = (block.len() + 9) / 10;
                    let have = block.iter().filter(|&&i| tr.cars[i].hand_brake > 0.5).count();
                    let to_set: Vec<usize> = block.iter().copied().filter(|&i| tr.cars[i].hand_brake < 0.5).take(need.saturating_sub(have)).collect();
                    Some(Maneuver::HandBrakes { indices: to_set, on: true })
                }
                SortPhase::Cutting { track } => {
                    let far_indices: Vec<usize> = if li == 0 { (0..n).rev().collect() } else { (0..n).collect() };
                    let mut len = 0;
                    for &i in &far_indices {
                        if tr.cars[i].dest == Some(track) && i != li {
                            len += 1;
                        } else {
                            break;
                        }
                    }
                    if len == 0 {
                        self.set_phase(SortPhase::Pick);
                        return None;
                    }
                    let k = if li == 0 { n - len } else { len };
                    Some(Maneuver::PullPin { k, tries: 0, bunching_until: None })
                }
                SortPhase::Parking | SortPhase::Done => None,
            },
        }
    }

    fn set_phase(&mut self, p: SortPhase) {
        if let Program::Sort { phase, .. } = &mut self.program {
            *phase = p;
        }
    }
}

fn hold(world: &mut World, id: TrainId) {
    world.set_controls(id, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
}

/// Braking deceleration this train can count on, m/s².
fn brake_decel(world: &World, tr: &Train) -> f64 {
    let types = &world.car_types;
    let ctrl = tr.control_car(types);
    let connected = tr.pipe_connectivity(ctrl);
    let mut force = 0.0;
    for (i, c) in tr.cars.iter().enumerate() {
        let ct = &types[c.type_id as usize];
        if let Some(spec) = &ct.loco {
            force += spec.independent_max;
        }
        if connected[i] && !c.brake.is_bled() {
            force += ct.m_tare * G * BRAKE_RATIO;
        }
    }
    (force / tr.total_mass(types) * 0.6).max(0.02)
}

fn drive(d: &mut Drive, world: &mut World, id: TrainId, dt: f64) -> Status {
    let t = world.t;
    let tr = world.train(id).unwrap();
    let types = &world.car_types;
    let n = tr.cars.len();
    if d.cars_at_start.is_none() {
        d.cars_at_start = Some(n);
        d.started = Some(t);
    }
    if d.stop_on_couple && n > d.cars_at_start.unwrap() {
        hold(world, id);
        return Status::Done;
    }
    if t - d.started.unwrap() > 1800.0 {
        return Status::Failed("this move is taking too long".into());
    }
    let sign = if d.forward { 1.0 } else { -1.0 };
    let lead = if d.forward { 0 } else { n - 1 };
    // Judge speed by whichever is faster in the direction of travel: the locomotive or the
    // leading car. On a long bled shove the head keeps coasting after the loco brakes.
    let v_dir = (tr.speed(types) * sign).max(tr.cars[lead].v * sign);

    // Remaining distance to the target.
    let remaining = match &mut d.target {
        Target::PastNode { node, face, beyond } => {
            match world.distance_to_node(tr, *face, d.forward, *node, 20_000.0) {
                Some(dn) => dn + *beyond,
                None => match world.distance_to_node(tr, *face, !d.forward, *node, 20_000.0) {
                    Some(db) => (*beyond - db).max(0.0),
                    None => return Status::Failed("can't find that switch from here".into()),
                },
            }
        }
        Target::ShortOfTrain { gap } => match world.distance_to_train_ahead(tr, d.forward, 20_000.0) {
            Some((dist, _)) => (dist - *gap).max(0.0),
            None => return Status::Failed("nothing ahead to couple to".into()),
        },
        Target::CouplerOnEdge { a, b, edge, from_node, depth } => {
            let ia = tr.cars.iter().position(|c| c.id == *a);
            let ib = tr.cars.iter().position(|c| c.id == *b);
            let (Some(ia), Some(ib)) = (ia, ib) else { return Status::Failed("the block came apart".into()) };
            let k = ia.max(ib);
            let x = tr.coupler_x(types, k);
            match tr.locate(&world.graph, x) {
                Some(loc) if loc.edge == *edge => {
                    let e = world.graph.edge(*edge);
                    let progress = if e.a == *from_node { loc.s } else { e.length - loc.s };
                    (*depth - progress).max(0.0)
                }
                _ => match world.distance_from_x_to_node(tr, x, d.forward, *from_node, 20_000.0) {
                    Some(dn) => dn + *depth,
                    None => return Status::Failed("that track is not ahead of us".into()),
                },
            }
        }
        Target::Distance { total, done } => {
            if let Some(lx) = d.last_x {
                *done += (tr.cars[0].x - lx).abs();
            }
            d.last_x = Some(tr.cars[0].x);
            (*total - *done).max(0.0)
        }
    };

    // Speed limits: current edge, upcoming restriction, bumper, and anything standing ahead.
    let a = brake_decel(world, tr);
    let mut vmax = d.vmax.min(world.current_limit(tr).unwrap_or(d.vmax));
    match world.lookahead(tr, d.forward, 600.0) {
        Some(Ahead::Restriction { limit, distance, .. }) => vmax = vmax.min((limit * limit + 2.0 * a * distance).sqrt()),
        Some(Ahead::End { distance }) => vmax = vmax.min((2.0 * a * (distance - 8.0).max(0.0)).sqrt()),
        Some(Ahead::SplitSwitch { distance, .. }) => vmax = vmax.min((2.0 * a * (distance - 15.0).max(0.0)).sqrt()),
        None => {}
    }
    let mut at_bumper = false;
    if let Some(Ahead::End { distance }) = world.lookahead(tr, d.forward, 60.0) {
        at_bumper = distance < 10.0;
    }
    if !matches!(d.target, Target::ShortOfTrain { .. }) {
        if let Some((gap, _)) = world.distance_to_train_ahead(tr, d.forward, 600.0) {
            let v_join = if d.join { 0.9 } else { 0.0 };
            let margin = if d.join { 0.0 } else { 6.0 };
            vmax = vmax.min((v_join * v_join + 2.0 * (a * 0.5) * (gap - margin).max(0.0)).sqrt());
            if d.join {
                if gap < 25.0 {
                    vmax = vmax.min(1.3);
                }
                if gap < 8.0 {
                    vmax = vmax.min(1.0);
                }
            }
        }
    }
    let v_target = (d.v_end * d.v_end + 2.0 * a * remaining).sqrt().min(vmax);
    // Approaching a joint we also want the head itself slow, not just the loco.
    let v_dir = if d.join { v_dir } else { tr.speed(types) * sign };

    let arrived = (remaining < 0.3 || at_bumper) && v_dir.abs() < 0.15;
    if arrived {
        hold(world, id);
        return if v_dir.abs() < 0.05 { Status::Done } else { Status::Running };
    }

    let rev = if d.forward { Reverser::Forward } else { Reverser::Reverse };
    let err = v_target - v_dir;
    let controls = if v_dir < -0.05 {
        Controls { throttle: 0, reverser: rev, independent: 1.0, ..Default::default() }
    } else if err > 0.15 {
        let notch = ((err * 3.0).ceil() as u8).clamp(1, 8);
        Controls { throttle: notch, reverser: rev, independent: 0.0, ..Default::default() }
    } else if err < -0.1 {
        Controls { throttle: 0, reverser: rev, independent: ((-err) * 1.5).clamp(0.25, 1.0), ..Default::default() }
    } else {
        let hold_notch = if v_dir > 0.5 && remaining > 30.0 { 1 } else { 0 };
        Controls { throttle: hold_notch, reverser: rev, independent: 0.0, ..Default::default() }
    };
    world.set_controls(id, controls);
    let _ = dt;
    Status::Running
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::*;
    use crate::score::Score;

    #[test]
    fn crew_sorts_the_yard_shift_unaided() {
        let sc = yard_shift();
        let Built { mut world, yard, loco, .. } = build(&sc);
        let loco_car = world.train(loco).unwrap().cars[0].id;
        let mut crew = Crew::new("R. Casey", loco_car, Program::Sort { yard: yard.clone(), phase: SortPhase::Start }, 0.0);
        let mut score = Score::default();
        let mut steps = 0usize;
        while crew.status == Status::Running && world.t < 5.0 * 3600.0 {
            crew.step(&mut world, DT);
            world.step(DT);
            steps += 1;
            if steps % 600 == 0 {
                let ev = world.take_events();
                score.ingest(world.t, &ev, &yard);
            }
        }
        let ev = world.take_events();
        score.ingest(world.t, &ev, &yard);
        score.evaluate(&world, &yard, sc.time_budget);
        for (t, line) in crew.radio.iter().take(12) {
            eprintln!("  [{:6.0}s] {line}", t);
        }
        eprintln!("crew status {:?}; t={:.0}s ({:.1} min); deliveries={}; placed {}/{} secured {}; hard={} derails={} bumps={} points={}", crew.status, world.t, world.t / 60.0, crew.deliveries, score.cars_placed, score.cars_total, score.cars_secured, score.hard_couplings, score.derailments, score.bumps, score.points);
        assert_eq!(crew.status, Status::Done, "crew did not finish: {:?}", crew.radio.back());
        assert_eq!(score.derailments, 0);
        assert!(score.complete, "all cars should be placed and secured");
        assert!(score.hard_couplings <= 1, "hard couplings: {}", score.hard_couplings);
    }
}
