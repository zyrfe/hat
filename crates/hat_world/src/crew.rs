//! Crews: a program of maneuvers that drives one locomotive through the sim.
//!
//! The player writes the switch list (car destinations); the crew's program turns it into
//! maneuvers and the executor drives. Every rule the player obeys, the crew obeys.
//! See docs/crew.md.

use std::collections::VecDeque;

use hat_sim::params::*;
use hat_sim::*;
use hat_units::G;

use crate::terminal::TRACK_HUMP as TRACK_HUMP_ID;
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
    /// Ignore whatever stands ahead: we are shoving it (humping).
    pub push_through: bool,
    cars_at_start: Option<usize>,
    started: Option<f64>,
    last_x: Option<f64>,
    /// Extra notches accumulated while the train fails to pick up speed (grades, tonnage).
    notch_bias: f64,
    last_v: Option<f64>,
    /// Sim time the train last made headway toward the target.
    last_progress: Option<f64>,
    /// Recent controller decisions for debugging: (t, v_dir, v_target, remaining, notch, independent).
    trace: VecDeque<(f64, f64, f64, f64, u8, f64)>,
}

impl Drive {
    pub fn new(forward: bool, target: Target, v_end: f64, vmax: f64) -> Self {
        Drive { forward, target, v_end, vmax, join: false, stop_on_couple: false, push_through: false, cars_at_start: None, started: None, last_x: None, notch_bias: 0.0, last_v: None, last_progress: None, trace: VecDeque::new() }
    }
    /// Shove whatever is touching us ahead instead of stopping short of it.
    pub fn pushing(mut self) -> Self {
        self.push_through = true;
        self
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
    /// `nudge_back`: bunch the slack by easing toward the locomotive instead of shoving.
    /// `moving`: we are shoving through the cut point, so nudge harder and retry at once.
    PullPin { k: usize, tries: u32, bunching_until: Option<f64>, retry_at: Option<f64>, nudge_back: bool, moving: bool },
    HandBrakes { indices: Vec<usize>, on: bool },
    Wait { until: f64 },
    LaceHoses,
    BleedAll,
    /// Wait for the pipe to charge through to the rear.
    WaitCharged { since: Option<f64> },
}

/// What a crew is trying to accomplish over many maneuvers.
#[derive(Clone, Debug)]
pub enum Program {
    Idle,
    /// Flat-switch a standing cut onto its destination tracks, then park.
    Sort(SortJob),
    /// Shove the receiving cut over the hump, one car at a time, into the bowl.
    Hump(HumpJob),
    /// Road crew: bring a train from the portal to the receiving track, cut off, leave.
    RoadIn(RoadJob),
    /// Road crew: from the portal to the departure track, couple, charge, leave.
    RoadOut(RoadJob),
}

#[derive(Clone, Debug)]
pub struct SortJob {
    pub yard: Yard,
    pub phase: SortPhase,
    /// Track holding the cut to work. None: the nearest loco-less train.
    pub source_track: Option<u32>,
    /// Pull clear of this switch before lining a delivery.
    pub clear_switch: NodeId,
    /// Where to leave the engine.
    pub park_switch: NodeId,
    pub park_route: Vec<(NodeId, Route)>,
}

#[derive(Clone, Debug)]
pub struct HumpJob {
    pub yard: Yard,
    pub phase: HumpPhase,
    /// Destination track to bowl track.
    pub bowl_for: Vec<(u32, u32)>,
    pub default_bowl: u32,
    /// The car cut most recently, so the next cut waits until it is off the hump.
    pub last_cut: Option<CarId>,
    /// Its length: the follow-through shove after the cut pushes it that far.
    pub last_cut_len: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HumpPhase {
    /// Line the way west to the receiving track's west switch.
    LiningWest,
    /// Run west until clear of the west switch.
    Positioning,
    /// Line into the receiving track and go couple.
    Approach,
    Coupling,
    Releasing,
    Bleeding,
    /// Line east to the hump and the first bowl track.
    LiningHump,
    Line,
    /// Waiting for the last cut car to clear the hump before lining the next.
    Spacing,
    Shove,
    Creep,
    Cut,
    /// Keep shoving one car length so the loose car goes over the crest.
    Follow,
    Parking,
    Done,
}

#[derive(Clone, Debug)]
pub struct RoadJob {
    pub yard: Yard,
    pub phase: RoadPhase,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RoadPhase {
    Lining,
    Running,
    Coupling,
    Lacing,
    Charging,
    Releasing,
    Tying,
    Cutting,
    LiningOut,
    Leaving,
    Done,
}

#[derive(Clone, Debug)]
pub enum SortPhase {
    Lining,
    Start,
    /// The cut was not reachable from here: line the entry switch straight and pull clear
    /// of it, then line again.
    RepositionLine,
    RepositionDrive,
    Coupling { cut: TrainId },
    Releasing,
    Pick,
    Clearing { track: u32, block: usize },
    Routing { track: u32, block: usize },
    Shoving { track: u32, block: usize },
    Tying { track: u32 },
    Cutting { track: u32 },
    /// Pull clear of the yard before lining the way to the parking spot.
    ParkClear,
    Parking,
    ParkDrive,
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
    /// A maneuver queued to run right after the current Route completes.
    pending_after_route: Option<Maneuver>,
    /// When the crew last had nothing to do while on a job.
    stalled_since: Option<f64>,
    /// A Sort job repositions at most once before giving up.
    repositioned: bool,
    /// Traces of the last drives that completed, for debugging.
    pub last_drive: Option<Drive>,
    pub prev_drive: Option<Drive>,
}

impl Crew {
    pub fn new(name: &'static str, loco_car: CarId, program: Program, t: f64) -> Self {
        Crew { name, loco_car, program, current: None, paused: false, status: Status::Running, radio: VecDeque::new(), deliveries: 0, started_at: t, finished_at: None, pending_after_route: None, stalled_since: None, repositioned: false, last_drive: None, prev_drive: None }
    }

    /// A Sort job for a yard with a single ladder off `lead_switch`.
    pub fn sort_job(yard: &Yard, source_track: Option<u32>) -> Program {
        let park_switch = yard.hump.as_ref().map(|h| h.turnout).unwrap_or(yard.lead_switch);
        let park_route = yard.hump.as_ref().map(|h| vec![(h.turnout, Route::Diverging)]).unwrap_or_default();
        let phase = if source_track.is_some() { SortPhase::Lining } else { SortPhase::Start };
        Program::Sort(SortJob { yard: yard.clone(), phase, source_track, clear_switch: yard.lead_switch, park_switch, park_route })
    }

    pub fn hump_job(yard: &Yard) -> Program {
        Program::Hump(HumpJob { yard: yard.clone(), phase: HumpPhase::LiningWest, bowl_for: vec![(5, 11), (6, 12), (1, 13)], default_bowl: 14, last_cut: None, last_cut_len: 0.0 })
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
                Program::Sort(j) => format!("{:?}", j.phase),
                Program::Hump(j) => format!("hump: {:?}", j.phase),
                Program::RoadIn(j) | Program::RoadOut(j) => format!("road: {:?}", j.phase),
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
            Some(Maneuver::BleedAll) => "bleeding the cut".into(),
            Some(Maneuver::WaitCharged { .. }) => "charging the air".into(),
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
                if !matches!(self.program, Program::Idle) && self.status == Status::Running {
                    let since = *self.stalled_since.get_or_insert(t);
                    if t - since > 60.0 {
                        let why = format!("stalled in {}", self.describe());
                        self.say(t, format!("I'm stuck: {why}"));
                        self.status = Status::Failed(why);
                    }
                }
                return;
            }
            self.stalled_since = None;
        }
        let mut maneuver = self.current.take().unwrap();
        let status = self.run_maneuver(&mut maneuver, world, id, dt);
        match status {
            Status::Running => self.current = Some(maneuver),
            Status::Done => {
                self.current = None;
                if let Maneuver::Drive(d) = &maneuver {
                    self.prev_drive = self.last_drive.take();
                    self.last_drive = Some(d.clone());
                }
                if matches!(maneuver, Maneuver::Route { .. }) {
                    if let Some(next) = self.pending_after_route.take() {
                        self.current = Some(next);
                        return;
                    }
                }
                self.advance(world, t);
            }
            Status::Failed(why) => {
                self.say(t, format!("Can't do it: {why}"));
                self.current = Some(maneuver);
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
            Maneuver::PullPin { k, tries, bunching_until, retry_at, nudge_back, moving } => {
                if let Some(until) = *bunching_until {
                    if t < until {
                        return Status::Running;
                    }
                    *bunching_until = None;
                    if *nudge_back {
                        hold(world, id);
                    }
                }
                if let Some(at) = *retry_at {
                    if t < at {
                        if *nudge_back {
                            hold(world, id);
                        }
                        return Status::Running;
                    }
                    *retry_at = None;
                }
                match world.pull_pin(id, *k) {
                    Ok(_) => {
                        if !*moving {
                            hold(world, id);
                        }
                        Status::Done
                    }
                    Err(_) if *tries < 8 => {
                        *tries += 1;
                        {
                            let tr = world.train(id).unwrap();
                            let e = tr.coupler_extension(&world.car_types, *k);
                            let li = tr.control_car(&world.car_types).unwrap_or(0);
                            let lead = if li == 0 { tr.cars.len() - 1 } else { 0 };
                            let loc = world.car_location(tr, lead).map(|l| (world.graph.edge(l.edge).track, l.s, world.graph.edge(l.edge).grade));
                            if *tries == 1 {
                                let _ = (e, loc, lead);
                                self.say(t, "Pin won't pull yet, bunching the slack.");
                            }
                        }
                        // Bunch the slack: a gentle nudge, then let it settle before trying again.
                        let tr = world.train(id).unwrap();
                        let li = tr.control_car(&world.car_types).unwrap_or(0);
                        let toward_far = (li == 0) != *nudge_back;
                        let rev = if toward_far { Reverser::Reverse } else { Reverser::Forward };
                        if *moving {
                            world.set_controls(id, Controls { throttle: 3, reverser: rev, independent: 0.0, ..Default::default() });
                            *bunching_until = Some(t + 0.5);
                            *retry_at = Some(t + 0.5);
                        } else if *nudge_back {
                            world.set_controls(id, Controls { throttle: 1, reverser: rev, independent: 0.0, ..Default::default() });
                            *bunching_until = Some(t + 1.2);
                            *retry_at = Some(t + 4.0);
                        } else {
                            // Shove to bunch and pull while still shoving, before the cars run out.
                            world.set_controls(id, Controls { throttle: 1, reverser: rev, independent: 0.0, ..Default::default() });
                            *bunching_until = Some(t + 1.5);
                            *retry_at = Some(t + 1.5);
                        }
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
            Maneuver::BleedAll => {
                let n = world.train(id).map(|t| t.cars.len()).unwrap_or(0);
                let types = world.car_types.clone();
                for i in 0..n {
                    let is_loco = world.train(id).map(|t| types[t.cars[i].type_id as usize].is_loco()).unwrap_or(true);
                    if !is_loco {
                        let _ = world.bleed(id, i);
                    }
                }
                Status::Done
            }
            Maneuver::WaitCharged { since } => {
                let started = *since.get_or_insert(t);
                hold(world, id);
                let tr = world.train(id).unwrap();
                let li = tr.control_car(&world.car_types).unwrap_or(0);
                let far = if li == 0 { tr.cars.len() - 1 } else { 0 };
                if tr.cars[far].brake.p_pipe > 450_000.0 || t - started > 900.0 { Status::Done } else { Status::Running }
            }
        }
    }

    /// Called when a maneuver completes: move the program forward.
    fn advance(&mut self, world: &World, t: f64) {
        let n = self.position(world).map(|(ti, _)| world.trains[ti].cars.len()).unwrap_or(0);
        let mut lines: Vec<String> = Vec::new();
        let mut finished = false;
        match &mut self.program {
            Program::Idle => {}
            Program::Sort(j) => {
                j.phase = match j.phase.clone() {
                    SortPhase::Lining => SortPhase::Start,
                    SortPhase::Start => SortPhase::Start,
                    SortPhase::RepositionLine => SortPhase::RepositionDrive,
                    SortPhase::RepositionDrive => SortPhase::Lining,
                    SortPhase::Coupling { .. } => {
                        lines.push(format!("Coupled, {} cars.", n.saturating_sub(1)));
                        SortPhase::Releasing
                    }
                    SortPhase::Releasing => {
                        lines.push("Brakes off. Let's sort them.".into());
                        SortPhase::Pick
                    }
                    SortPhase::Pick => SortPhase::Pick,
                    SortPhase::Clearing { track, block } => SortPhase::Routing { track, block },
                    SortPhase::Routing { track, block } => {
                        lines.push(format!("Lined for {}. Shoving {block}.", j.yard.track_name(track)));
                        SortPhase::Shoving { track, block }
                    }
                    SortPhase::Shoving { track, .. } => SortPhase::Tying { track },
                    SortPhase::Tying { track } => SortPhase::Cutting { track },
                    SortPhase::Cutting { track } => {
                        self.deliveries += 1;
                        lines.push(format!("Cut. {} done for now.", j.yard.track_name(track)));
                        SortPhase::Pick
                    }
                    SortPhase::ParkClear => {
                        if j.park_switch == j.clear_switch {
                            lines.push("Job done. Engine clear of the yard.".into());
                            finished = true;
                            SortPhase::Done
                        } else {
                            SortPhase::Parking
                        }
                    }
                    SortPhase::Parking => SortPhase::ParkDrive,
                    SortPhase::ParkDrive => {
                        lines.push("Job done. Engine parked.".into());
                        finished = true;
                        SortPhase::Done
                    }
                    SortPhase::Done => SortPhase::Done,
                };
            }
            Program::Hump(j) => {
                j.phase = match j.phase.clone() {
                    HumpPhase::LiningWest => HumpPhase::Positioning,
                    HumpPhase::Positioning => HumpPhase::Approach,
                    HumpPhase::Approach => HumpPhase::Coupling,
                    HumpPhase::Coupling => {
                        lines.push(format!("Coupled to the receiving cut, {} cars.", n.saturating_sub(1)));
                        HumpPhase::Releasing
                    }
                    HumpPhase::Releasing => HumpPhase::Bleeding,
                    HumpPhase::Bleeding => {
                        lines.push("Cut is bled. Taking it to the hump.".into());
                        HumpPhase::LiningHump
                    }
                    HumpPhase::LiningHump => HumpPhase::Line,
                    HumpPhase::Line => HumpPhase::Shove,
                    HumpPhase::Spacing => HumpPhase::Line,
                    HumpPhase::Shove => HumpPhase::Creep,
                    HumpPhase::Creep => HumpPhase::Cut,
                    HumpPhase::Cut => {
                        self.deliveries += 1;
                        HumpPhase::Follow
                    }
                    HumpPhase::Follow => HumpPhase::Line,
                    HumpPhase::Parking => {
                        lines.push("Hump done. Engine on the approach.".into());
                        finished = true;
                        HumpPhase::Done
                    }
                    HumpPhase::Done => HumpPhase::Done,
                };
            }
            Program::RoadIn(j) => {
                j.phase = match j.phase.clone() {
                    RoadPhase::Lining => RoadPhase::Running,
                    RoadPhase::Running => {
                        lines.push("In the receiving track. Tying down.".into());
                        RoadPhase::Tying
                    }
                    RoadPhase::Tying => RoadPhase::Cutting,
                    RoadPhase::Cutting => RoadPhase::LiningOut,
                    RoadPhase::LiningOut => {
                        lines.push("Cut off, heading out.".into());
                        RoadPhase::Leaving
                    }
                    RoadPhase::Leaving => {
                        finished = true;
                        RoadPhase::Done
                    }
                    other => other,
                };
            }
            Program::RoadOut(j) => {
                j.phase = match j.phase.clone() {
                    RoadPhase::Lining => RoadPhase::Running,
                    RoadPhase::Running => RoadPhase::Coupling,
                    RoadPhase::Coupling => {
                        lines.push(format!("On the departure cut, {} cars. Lacing.", n.saturating_sub(1)));
                        RoadPhase::Lacing
                    }
                    RoadPhase::Lacing => RoadPhase::Charging,
                    RoadPhase::Charging => {
                        lines.push("Air's up. Releasing hand brakes.".into());
                        RoadPhase::Releasing
                    }
                    RoadPhase::Releasing => RoadPhase::LiningOut,
                    RoadPhase::LiningOut => {
                        lines.push("Highball.".into());
                        RoadPhase::Leaving
                    }
                    RoadPhase::Leaving => {
                        finished = true;
                        RoadPhase::Done
                    }
                    other => other,
                };
            }
        }
        for l in lines {
            self.say(t, l);
        }
        if finished {
            self.finished_at = Some(t);
            self.status = Status::Done;
        }
    }

    /// Train-frame direction in which `node` lies, if reachable either way.
    fn dir_to(world: &World, tr: &Train, node: NodeId) -> Option<bool> {
        if world.distance_to_node(tr, End::Head, true, node, 30_000.0).is_some() {
            Some(true)
        } else if world.distance_to_node(tr, End::Tail, false, node, 30_000.0).is_some() {
            Some(false)
        } else {
            None
        }
    }

    /// Which train-frame direction reaches `target`, if any.
    fn dir_to_train(world: &World, tr: &Train, target: TrainId) -> Option<bool> {
        if world.distance_to_train_ahead(tr, true, 30_000.0).map(|(_, id)| id == target).unwrap_or(false) {
            Some(true)
        } else if world.distance_to_train_ahead(tr, false, 30_000.0).map(|(_, id)| id == target).unwrap_or(false) {
            Some(false)
        } else {
            None
        }
    }

    /// The far-end run of cars, from the end away from the locomotive.
    fn far_run(tr: &Train, li: usize, pred: impl Fn(&CarState) -> bool) -> Vec<usize> {
        let n = tr.cars.len();
        let order: Vec<usize> = if li == 0 { (0..n).rev().collect() } else { (0..n).collect() };
        let mut out = Vec::new();
        for i in order {
            if i != li && pred(&tr.cars[i]) {
                out.push(i);
            } else {
                break;
            }
        }
        out
    }

    /// Reassign this crew to a new job.
    pub fn assign(&mut self, program: Program, t: f64) {
        self.program = program;
        self.repositioned = false;
        self.current = None;
        self.status = Status::Running;
        self.paused = false;
        self.finished_at = None;
        self.started_at = t;
    }

    /// One line per car for debugging a stuck crew.
    pub fn debug_state(&self, world: &World) -> String {
        let Some((ti, _)) = self.position(world) else { return "no train".into() };
        let tr = &world.trains[ti];
        let mut out = format!("controls {:?}\n", tr.controls);
        for (i, c) in tr.cars.iter().enumerate() {
            let ct = &world.car_types[c.type_id as usize];
            let loc = world.car_location(tr, i);
            out += &format!(
                "  {:>2} #{:<3} {:<14} track {:?} s={:>7.1} grade={:+.3} v={:+.3} cyl={:>4.0} aux={:>4.0} hb={:.0} zone={:?}\n",
                i,
                ct.name,
                c.id,
                loc.and_then(|l| world.graph.edge(l.edge).track),
                loc.map(|l| l.s).unwrap_or(f64::NAN),
                loc.map(|l| world.graph.edge(l.edge).grade).unwrap_or(0.0),
                c.v,
                c.brake.p_cyl / 1000.0,
                c.brake.p_aux / 1000.0,
                c.hand_brake,
                if i + 1 < tr.cars.len() { Some(tr.zones[i]) } else { None }
            );
        }
        let mut drives: Vec<(&str, &Drive)> = Vec::new();
        if let Some(d) = self.prev_drive.as_ref() {
            drives.push(("previous", d));
        }
        if let Some(d) = self.last_drive.as_ref() {
            drives.push(("last", d));
        }
        if let Some(Maneuver::Drive(d)) = &self.current {
            drives.push(("current", d));
        }
        for (which, d) in drives {
            out += &format!("  {which} drive: forward={} target={:?} vmax={} v_end={} bias={:.1} trace_len={} started={:?}\n", d.forward, d.target, d.vmax, d.v_end, d.notch_bias, d.trace.len(), d.started);
            for (t, v, vt, rem, notch, ind) in d.trace.iter() {
                out += &format!("    t={:>7.1} v={:+.3} v_target={:.3} remaining={:>8.2} notch={} ind={:.2}\n", t, v, vt, rem, notch, ind);
            }
        }
        out
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.program, Program::Idle) || self.status != Status::Running
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
            Program::Sort(j) => self.next_sort(world, tr, li, n, types, j),
            Program::Hump(j) => self.next_hump(world, tr, li, n, j),
            Program::RoadIn(j) => self.next_road_in(world, tr, li, n, j),
            Program::RoadOut(j) => self.next_road_out(world, tr, li, n, j),
        }
        .or_else(|| {
            let _ = t;
            None
        })
    }

    fn next_sort(&mut self, world: &World, tr: &Train, li: usize, n: usize, types: &[CarType], j: SortJob) -> Option<Maneuver> {
        let t = world.t;
        let yard = &j.yard;
        match j.phase {
            SortPhase::Lining => {
                let route = j.source_track.and_then(|tk| yard.route_to(tk)).unwrap_or_default();
                Some(Maneuver::Route { settings: route, since: None })
            }
            SortPhase::Start => {
                let candidates: Vec<TrainId> = world
                    .trains
                    .iter()
                    .filter(|o| o.id != tr.id && o.control_car(types).is_none() && !o.cars.is_empty())
                    .filter(|o| match j.source_track {
                        Some(tk) => world.car_location(o, 0).and_then(|l| world.graph.edge(l.edge).track) == Some(tk),
                        None => true,
                    })
                    .map(|o| o.id)
                    .collect();
                for &target in &candidates {
                    if let Some(forward) = Self::dir_to_train(world, tr, target) {
                        self.say(t, "Heading for the cut.");
                        self.set_phase(SortPhase::Coupling { cut: target });
                        return Some(Maneuver::Drive(Drive::new(forward, Target::ShortOfTrain { gap: 0.0 }, 0.9, 5.0).coupling()));
                    }
                }
                if candidates.is_empty() {
                    self.status = Status::Failed("there is no cut to work".into());
                    return None;
                }
                // Not reachable from here. Try once from the toe side of the ladder entry.
                let entry = j.source_track.and_then(|tk| yard.ladder_of(tk)).and_then(|li| yard.ladders[li].entry.map(|(n, _)| n)).unwrap_or(j.clear_switch);
                if self.repositioned {
                    let ahead = world.distance_to_train_ahead(tr, true, 30_000.0);
                    let behind = world.distance_to_train_ahead(tr, false, 30_000.0);
                    let here = world.car_location(tr, 0).map(|l| (world.graph.edge(l.edge).track, l.s));
                    self.status = Status::Failed(format!("no clear path to the cut: candidates {candidates:?}, ahead {ahead:?}, behind {behind:?}, engine at {here:?}"));
                    return None;
                }
                self.repositioned = true;
                self.say(t, "Can't reach it from here. Getting on the right side of the switch.");
                self.set_phase(SortPhase::RepositionLine);
                Some(Maneuver::Route { settings: vec![(entry, Route::Normal)], since: None })
            }
            SortPhase::RepositionLine => None,
            SortPhase::RepositionDrive => {
                let entry = j.source_track.and_then(|tk| yard.ladder_of(tk)).and_then(|li| yard.ladders[li].entry.map(|(n, _)| n)).unwrap_or(j.clear_switch);
                Self::clear_drive(world, tr, entry)
            }
            SortPhase::Coupling { .. } => None,
            SortPhase::Releasing => Some(Maneuver::HandBrakes { indices: (0..n).collect(), on: false }),
            SortPhase::Pick => {
                let blocks = self.plan_preview(world);
                match blocks.first() {
                    None => {
                        // Nothing left to deliver: get clear of the yard first.
                        self.set_phase(SortPhase::ParkClear);
                        Self::clear_drive(world, tr, j.clear_switch)
                    }
                    Some(&(track, block)) => {
                        self.say(t, format!("Next: {block} for {}. Pulling clear.", yard.track_name(track)));
                        self.set_phase(SortPhase::Clearing { track, block });
                        Self::clear_drive(world, tr, j.clear_switch)
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
                let block = Self::far_run(tr, li, |c| c.dest == Some(track));
                let need = (block.len() + 9) / 10;
                let have = block.iter().filter(|&&i| tr.cars[i].hand_brake > 0.5).count();
                let to_set: Vec<usize> = block.iter().copied().filter(|&i| tr.cars[i].hand_brake < 0.5).take(need.saturating_sub(have)).collect();
                Some(Maneuver::HandBrakes { indices: to_set, on: true })
            }
            SortPhase::Cutting { track } => {
                let len = Self::far_run(tr, li, |c| c.dest == Some(track)).len();
                if len == 0 {
                    self.set_phase(SortPhase::Pick);
                    return None;
                }
                let k = if li == 0 { n - len } else { len };
                Some(Maneuver::PullPin { k, tries: 0, bunching_until: None, retry_at: None, nudge_back: false, moving: false })
            }
            SortPhase::ParkClear => None,
            SortPhase::Parking => {
                let mut settings = vec![(j.clear_switch, Route::Normal)];
                settings.extend(j.park_route.iter().copied());
                Some(Maneuver::Route { settings, since: None })
            }
            SortPhase::ParkDrive => {
                let forward = match Self::dir_to(world, tr, j.park_switch) {
                    Some(f) => f,
                    None => {
                        self.status = Status::Failed("no way to the parking spot".into());
                        return None;
                    }
                };
                let trailing = if forward { End::Tail } else { End::Head };
                Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: j.park_switch, face: trailing, beyond: 60.0 }, 0.0, 6.0)))
            }
            SortPhase::Done => None,
        }
    }

    pub fn road_phase(&self) -> Option<RoadPhase> {
        match &self.program {
            Program::RoadIn(j) | Program::RoadOut(j) => Some(j.phase.clone()),
            _ => None,
        }
    }

    /// Drive toward `switch` until the whole train is 25 m beyond it. Works for a lone engine
    /// and for a train whichever end the locomotive is on, because the direction comes from
    /// where the switch lies.
    fn clear_drive(world: &World, tr: &Train, switch: NodeId) -> Option<Maneuver> {
        // Straddling the switch, neither end can see it: pull toward the locomotive's end.
        let li = tr.control_car(&world.car_types).unwrap_or(0);
        let forward = Self::dir_to(world, tr, switch).unwrap_or(li == 0);
        let trailing = if forward { End::Tail } else { End::Head };
        Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: switch, face: trailing, beyond: 25.0 }, 0.0, 6.0)))
    }

    fn set_hump_phase(&mut self, p: HumpPhase) {
        if let Program::Hump(j) = &mut self.program {
            j.phase = p;
        }
    }

    fn next_hump(&mut self, world: &World, tr: &Train, li: usize, n: usize, j: HumpJob) -> Option<Maneuver> {
        let t = world.t;
        let yard = &j.yard;
        let hump = yard.hump.clone()?;
        let (rt, wr, er) = yard.receiving?;
        match j.phase {
            HumpPhase::LiningWest => Some(Maneuver::Route { settings: vec![(hump.turnout, Route::Diverging), (yard.lead_switch, Route::Normal), (er, Route::Normal), (wr, Route::Normal)], since: None }),
            HumpPhase::Positioning => {
                let forward = Self::dir_to(world, tr, wr)?;
                let trailing = if forward { End::Tail } else { End::Head };
                self.say(t, "Running west to get behind the receiving cut.");
                Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: wr, face: trailing, beyond: 30.0 }, 0.0, 12.0)))
            }
            HumpPhase::Approach => {
                self.set_hump_phase(HumpPhase::Coupling);
                self.pending_after_route = {
                    let target = world.trains.iter().find(|o| o.id != tr.id && o.control_car(&world.car_types).is_none() && world.car_location(o, 0).and_then(|l| world.graph.edge(l.edge).track) == Some(rt)).map(|o| o.id);
                    target.map(|_| Maneuver::Drive(Drive::new(true, Target::ShortOfTrain { gap: 0.0 }, 0.9, 5.0).coupling()))
                };
                Some(Maneuver::Route { settings: vec![(wr, Route::Diverging)], since: None })
            }
            HumpPhase::Coupling => None,
            HumpPhase::Releasing => Some(Maneuver::HandBrakes { indices: (0..n).collect(), on: false }),
            HumpPhase::Bleeding => Some(Maneuver::BleedAll),
            HumpPhase::LiningHump => Some(Maneuver::Route { settings: vec![(er, Route::Diverging), (yard.lead_switch, Route::Normal), (hump.turnout, Route::Diverging)], since: None }),
            HumpPhase::Line => {
                if let Some(prev) = j.last_cut {
                    if let Some((pti, pci)) = world.train_of_car(prev) {
                        let ptr = &world.trains[pti];
                        // Still on the hump lead, the ladder, or in a retarder: give it room.
                        let on_hump = world
                            .car_location(ptr, pci)
                            .map(|l| {
                                let e = world.graph.edge(l.edge);
                                e.track == Some(TRACK_HUMP_ID) || e.track == Some(TRACK_LEAD) || e.retarder.is_some()
                            })
                            .unwrap_or(false);
                        if on_hump {
                            self.set_hump_phase(HumpPhase::Spacing);
                            return Some(Maneuver::Wait { until: t + 2.0 });
                        }
                    }
                }
                let run = Self::far_run(tr, li, |_| true);
                let Some(&lead) = run.first() else {
                    self.set_hump_phase(HumpPhase::Parking);
                    let forward = Self::dir_to(world, tr, hump.turnout)?;
                    let trailing = if forward { End::Tail } else { End::Head };
                    return Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: hump.crest, face: trailing, beyond: 90.0 }, 0.0, 4.0)));
                };
                let dest = tr.cars[lead].dest;
                let bowl = dest.and_then(|d| j.bowl_for.iter().find(|(from, _)| *from == d).map(|(_, to)| *to)).unwrap_or(j.default_bowl);
                let mut settings = yard.route_to(bowl)?;
                settings.retain(|(node, _)| *node != hump.turnout || world.graph.turnout_setting(*node) != Some(Route::Diverging));
                Some(Maneuver::Route { settings, since: None })
            }
            HumpPhase::Shove => {
                let run = Self::far_run(tr, li, |_| true);
                let &lead = run.first()?;
                let k = if li == 0 { lead } else { lead + 1 };
                if k == 0 || k >= n {
                    return None;
                }
                let (a, b) = (tr.cars[k - 1].id, tr.cars[k].id);
                let climb = world.graph.edge(hump.climb);
                let forward = Self::dir_to(world, tr, hump.crest).unwrap_or(li != 0);
                // Reach the cut point still moving at about 1 m/s with the leading car's centre
                // just short of the crest: its weight keeps the coupler bunched so the pin pulls,
                // and its momentum carries it over the moment it is loose. Real pin pullers cut
                // on the move for the same reason.
                let lead_len = world.car_types[tr.cars[lead].type_id as usize].length;
                let cut_depth = (climb.length - (lead_len / 2.0 + 3.0)).max(1.0);
                Some(Maneuver::Drive(Drive::new(forward, Target::CouplerOnEdge { a, b, edge: hump.climb, from_node: climb.a, depth: (cut_depth - 25.0).max(0.5) }, 1.2, 6.0).pushing()))
            }
            HumpPhase::Creep => {
                let run = Self::far_run(tr, li, |_| true);
                let &lead = run.first()?;
                let k = if li == 0 { lead } else { lead + 1 };
                if k == 0 || k >= n {
                    return None;
                }
                let (a, b) = (tr.cars[k - 1].id, tr.cars[k].id);
                let climb = world.graph.edge(hump.climb);
                let forward = Self::dir_to(world, tr, hump.crest).unwrap_or(li != 0);
                let lead_len = world.car_types[tr.cars[lead].type_id as usize].length;
                let cut_depth = (climb.length - (lead_len / 2.0 + 3.0)).max(1.0);
                // Steady shove: no braking into the cut point, so the slack stays bunched.
                Some(Maneuver::Drive(Drive::new(forward, Target::CouplerOnEdge { a, b, edge: hump.climb, from_node: climb.a, depth: cut_depth }, 1.0, 1.2).pushing()))
            }
            HumpPhase::Cut => {
                let run = Self::far_run(tr, li, |_| true);
                let &lead = run.first()?;
                let k = if li == 0 { lead } else { lead + 1 };
                let lead_len = world.car_types[tr.cars[lead].type_id as usize].length;
                if let Program::Hump(job) = &mut self.program {
                    job.last_cut = Some(tr.cars[lead].id);
                    job.last_cut_len = lead_len;
                }
                Some(Maneuver::PullPin { k, tries: 0, bunching_until: None, retry_at: None, nudge_back: false, moving: true })
            }
            HumpPhase::Follow => {
                // The loose car was cut with its centre 3 m short of the crest. Push it a little
                // past, no further, so the next car stops short of its own cut point.
                let forward = Self::dir_to(world, tr, hump.crest).unwrap_or(li != 0);
                Some(Maneuver::Drive(Drive::new(forward, Target::Distance { total: 4.5, done: 0.0 }, 0.0, 1.5).pushing()))
            }
            HumpPhase::Spacing => None,
            HumpPhase::Parking | HumpPhase::Done => None,
        }
    }

    fn set_road_phase(&mut self, p: RoadPhase) {
        match &mut self.program {
            Program::RoadIn(j) | Program::RoadOut(j) => j.phase = p,
            _ => {}
        }
    }

    fn next_road_in(&mut self, world: &World, tr: &Train, li: usize, n: usize, j: RoadJob) -> Option<Maneuver> {
        let yard = &j.yard;
        let (_, wr, er) = yard.receiving?;
        let portal = yard.portal?;
        match j.phase {
            RoadPhase::Lining => Some(Maneuver::Route { settings: vec![(wr, Route::Diverging), (er, Route::Diverging), (yard.lead_switch, Route::Normal)], since: None }),
            RoadPhase::LiningOut if yard.hump.is_some() => Some(Maneuver::Route { settings: vec![(er, Route::Diverging), (yard.lead_switch, Route::Normal), (yard.hump.as_ref().unwrap().turnout, Route::Normal)], since: None }),
            RoadPhase::Running => {
                let forward = Self::dir_to(world, tr, er)?;
                let leading = if forward { End::Head } else { End::Tail };
                Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: er, face: leading, beyond: -40.0 }, 0.0, 15.0)))
            }
            RoadPhase::Tying => {
                let cars: Vec<usize> = (0..n).filter(|&i| i != li).collect();
                let need = (cars.len() + 9) / 10;
                let step = (cars.len() / need.max(1)).max(1);
                Some(Maneuver::HandBrakes { indices: cars.iter().copied().step_by(step).take(need).collect(), on: true })
            }
            RoadPhase::Cutting => {
                let k = if li == 0 { 1 } else { n - 1 };
                Some(Maneuver::PullPin { k, tries: 0, bunching_until: None, retry_at: None, nudge_back: false, moving: false })
            }
            RoadPhase::LiningOut => Some(Maneuver::Route { settings: vec![(er, Route::Diverging), (yard.lead_switch, Route::Normal)], since: None }),
            RoadPhase::Leaving => {
                let forward = Self::dir_to(world, tr, portal)?;
                let leading = if forward { End::Head } else { End::Tail };
                Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: portal, face: leading, beyond: 40.0 }, 0.0, 15.0)))
            }
            _ => None,
        }
    }

    fn next_road_out(&mut self, world: &World, tr: &Train, li: usize, n: usize, j: RoadJob) -> Option<Maneuver> {
        let yard = &j.yard;
        let (_, wr, er) = yard.receiving?;
        let portal = yard.portal?;
        let dep = yard.departure?;
        match j.phase {
            RoadPhase::Lining => {
                let mut settings = vec![(wr, Route::Normal), (er, Route::Normal)];
                settings.extend(yard.route_to(dep)?);
                Some(Maneuver::Route { settings, since: None })
            }
            RoadPhase::Running => {
                let target = world.trains.iter().find(|o| o.id != tr.id && o.control_car(&world.car_types).is_none() && world.car_location(o, 0).and_then(|l| world.graph.edge(l.edge).track) == Some(dep)).map(|o| o.id)?;
                let forward = Self::dir_to_train(world, tr, target)?;
                self.set_road_phase(RoadPhase::Coupling);
                Some(Maneuver::Drive(Drive::new(forward, Target::ShortOfTrain { gap: 0.0 }, 0.9, 15.0).coupling()))
            }
            RoadPhase::Coupling => None,
            RoadPhase::Lacing => Some(Maneuver::LaceHoses),
            RoadPhase::Charging => Some(Maneuver::WaitCharged { since: None }),
            RoadPhase::Releasing => Some(Maneuver::HandBrakes { indices: (0..n).filter(|&i| i != li).collect(), on: false }),
            RoadPhase::LiningOut => Some(Maneuver::Route { settings: vec![(wr, Route::Normal), (er, Route::Normal)], since: None }),
            RoadPhase::Leaving => {
                let forward = Self::dir_to(world, tr, portal)?;
                let leading = if forward { End::Head } else { End::Tail };
                Some(Maneuver::Drive(Drive::new(forward, Target::PastNode { node: portal, face: leading, beyond: 40.0 }, 0.0, 15.0)))
            }
            _ => None,
        }
    }

    fn set_phase(&mut self, p: SortPhase) {
        if let Program::Sort(j) = &mut self.program {
            j.phase = p;
        }
    }
}

fn hold(world: &mut World, id: TrainId) {
    world.set_controls(id, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
}

/// Trains standing on `track` that have no locomotive.
pub fn cuts_on_track(world: &World, track: u32) -> Vec<TrainId> {
    let types = &world.car_types;
    world
        .trains
        .iter()
        .filter(|t| t.control_car(types).is_none() && !t.cars.is_empty())
        .filter(|t| world.car_location(t, 0).and_then(|l| world.graph.edge(l.edge).track) == Some(track))
        .map(|t| t.id)
        .collect()
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
    let with_air = tr.cars.iter().enumerate().any(|(i, c)| connected[i] && !c.brake.is_bled() && types[c.type_id as usize].loco.is_none());
    (force / tr.total_mass(types) * if with_air { 0.4 } else { 0.6 }).max(0.02)
}

/// Does this train have charged cars on the pipe beyond the locomotive?
fn has_air(world: &World, tr: &Train) -> bool {
    let types = &world.car_types;
    let connected = tr.pipe_connectivity(tr.control_car(types));
    tr.cars.iter().enumerate().any(|(i, c)| connected[i] && !c.brake.is_bled() && types[c.type_id as usize].loco.is_none())
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
                Some(dn) => (dn + *beyond).max(0.0),
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
    if !d.push_through {
        let gap_ahead = if matches!(d.target, Target::ShortOfTrain { .. }) { Some(remaining) } else { world.distance_to_train_ahead(tr, d.forward, 600.0).map(|(g, _)| g) };
        if let Some(gap) = gap_ahead {
            let v_join = if d.join { 0.8 } else { 0.0 };
            let margin = if d.join { 0.0 } else { 6.0 };
            vmax = vmax.min((v_join * v_join + 2.0 * (a * 0.5) * (gap - margin).max(0.0)).sqrt());
            if d.join {
                if gap < 30.0 {
                    vmax = vmax.min(1.2);
                }
                if gap < 10.0 {
                    vmax = vmax.min(0.9);
                }
            }
        }
    }
    let mut v_target = (d.v_end * d.v_end + 2.0 * a * remaining).sqrt().min(vmax);
    // Creep into a stop: slack run-out past the locomotive is what overshoots a spot.
    if d.v_end < 0.05 && !matches!(d.target, Target::ShortOfTrain { .. }) {
        if remaining < 12.0 {
            v_target = v_target.min(0.6);
        }
        if remaining < 3.0 {
            v_target = v_target.min(0.3);
        }
    }
    // Approaching a joint we also want the head itself slow, not just the loco.
    let v_dir = if d.join { v_dir } else { tr.speed(types) * sign };

    if d.v_end > 0.3 && remaining < 0.3 && !d.stop_on_couple && !matches!(d.target, Target::ShortOfTrain { .. }) {
        // A moving target: hand over at speed with the throttle where it is.
        return Status::Done;
    }
    let arrived = (remaining < 0.3 || at_bumper) && v_dir.abs() < 0.15;
    if arrived {
        hold(world, id);
        return if v_dir.abs() < 0.05 { Status::Done } else { Status::Running };
    }

    let rev = if d.forward { Reverser::Forward } else { Reverser::Reverse };
    let err = v_target - v_dir;
    let air = has_air(world, tr);
    // Integral action on the throttle: a heavy cut on a grade needs more than the speed
    // error alone suggests. Wind the bias up while we want speed and are not gaining it.
    let accel = d.last_v.map(|lv| (v_dir - lv) / dt).unwrap_or(0.0);
    d.last_v = Some(v_dir);
    if err > 0.15 && accel < 0.03 {
        d.notch_bias = (d.notch_bias + 1.2 * dt).min(8.0);
    } else if err < 0.0 {
        d.notch_bias = (d.notch_bias - 3.0 * dt).max(0.0);
    }
    // Headway watchdog: stuck for two minutes means we cannot move it.
    if v_dir.abs() > 0.05 || err <= 0.15 {
        d.last_progress = Some(t);
    }
    if let Some(lp) = d.last_progress {
        if t - lp > 120.0 && d.notch_bias >= 7.9 {
            return Status::Failed("can't get it moving".into());
        }
    } else {
        d.last_progress = Some(t);
    }
    let controls = if v_dir < -0.05 {
        Controls { throttle: 0, reverser: rev, independent: 1.0, auto_target: if air { FULL_SERVICE_PIPE } else { PIPE_REF }, ..Default::default() }
    } else if err > 0.15 {
        let notch = (((err * 3.0).ceil() + d.notch_bias.floor()) as u8).clamp(1, 8);
        Controls { throttle: notch, reverser: rev, independent: 0.0, ..Default::default() }
    } else if err < -0.1 {
        // Deceleration needed to hit the target speed at the target, against what the whole
        // train can deliver at full application.
        let a_full = a / if air { 0.4 } else { 0.6 };
        let need = if remaining > 0.5 { ((v_dir * v_dir - d.v_end * d.v_end) / (2.0 * remaining)).max(0.0) } else { a_full };
        let frac = (need / a_full).max((-err) * 2.0).clamp(0.35, 1.0);
        let reduction = if air { (frac * (PIPE_REF - FULL_SERVICE_PIPE)).clamp(30_000.0, PIPE_REF - FULL_SERVICE_PIPE) } else { 0.0 };
        Controls { throttle: 0, reverser: rev, independent: frac, auto_target: PIPE_REF - reduction, ..Default::default() }
    } else {
        let hold_notch = if v_dir > 0.5 && remaining > 30.0 { 1 } else { 0 };
        Controls { throttle: hold_notch, reverser: rev, independent: 0.0, ..Default::default() }
    };
    let period = if remaining < 60.0 { 1.0 } else { 5.0 };
    if d.trace.back().map(|b| t - b.0 >= period).unwrap_or(true) {
        d.trace.push_back((t, v_dir, v_target, remaining, controls.throttle, controls.independent));
        while d.trace.len() > 60 {
            d.trace.pop_front();
        }
    }
    world.set_controls(id, controls);
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
        let mut crew = Crew::new("R. Casey", loco_car, Crew::sort_job(&yard, None), 0.0);
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
