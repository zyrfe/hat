//! The simulation as a Bevy resource, stepped on the fixed timestep.

use std::collections::HashMap;

use bevy::prelude::*;
use hat_sim::*;
use hat_world::*;

use crate::input::Sel;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Calm,
    Focused,
    Pleased,
    Worried,
    Alarmed,
    Panic,
}

/// The crew member in the cab. A reactive portrait today, a crew model later.
#[derive(Clone, Debug)]
pub struct Engineer {
    pub name: &'static str,
    pub mood: Mood,
    pub until: f64,
    pub line: String,
    pub sticky: bool,
}

impl Engineer {
    pub fn new() -> Self {
        Engineer { name: "R. Casey", mood: Mood::Calm, until: 0.0, line: "Ready when you are.".into(), sticky: false }
    }

    pub fn react(&mut self, t: f64, mood: Mood, secs: f64, line: &str, sticky: bool) {
        if self.sticky && !sticky {
            return;
        }
        self.mood = mood;
        self.until = t + secs;
        self.line = line.to_string();
        self.sticky = sticky;
    }

    fn settle(&mut self, t: f64, mood: Mood, line: &str) {
        if self.sticky || t < self.until {
            return;
        }
        if self.mood != mood {
            self.mood = mood;
            self.line = line.to_string();
        }
    }
}

#[derive(Clone, Debug)]
pub struct Banner {
    pub text: String,
    pub until: f64,
    pub color: [u8; 3],
}

pub struct SimPlugin;

impl Plugin for SimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Sim::load(std::env::var("HAT_SCENARIO").ok().and_then(|s| s.parse().ok()).unwrap_or(2), 0))
            .add_systems(FixedUpdate, step_sim)
            .add_systems(Update, handle_reload);
    }
}

#[derive(Resource)]
pub struct Sim {
    pub world: hat_sim::World,
    pub yard: Yard,
    pub scenario: Scenario,
    pub scenario_index: usize,
    pub score: Score,
    /// Engineer's controls in the locomotive's own frame.
    pub controls: Controls,
    /// Sim steps per fixed tick. 0 pauses.
    pub time_scale: u32,
    pub last_eval: f64,
    /// Events not yet consumed by audio.
    pub pending_events: Vec<SimEvent>,
    pub car_index: HashMap<CarId, (usize, usize)>,
    /// Short-lived HUD messages: (sim time, text).
    pub messages: Vec<(f64, String)>,
    pub reload: Option<usize>,
    /// Bumps on every reload so visuals from the old world get dropped.
    pub generation: u32,
    pub engineer: Engineer,
    pub banner: Option<Banner>,
    /// What the locomotive's train is heading into.
    pub ahead: Option<Ahead>,
    pub cur_limit: Option<f64>,
    /// Every crew on the railroad. The yard crew, when there is one, comes first.
    pub crews: Vec<Crew>,
    pub yard_crew: Option<usize>,
    /// The locomotive the cab window and the levers belong to.
    pub active_loco: Option<CarId>,
    /// When true the active crew drives; touching a control pauses it.
    pub auto: bool,
    pub manual_touch: bool,
    /// Road traffic and the yard master, for terminal scenarios.
    pub dispatcher: Option<Dispatcher>,
    /// Events from the most recent sim step, fed to the dispatcher on the next.
    pub last_events: Vec<SimEvent>,
    /// Money and industries, for open-country scenarios.
    pub economy: Option<Economy>,
    pub wrecker: Wrecker,
}

impl Sim {
    pub fn load(index: usize, generation: u32) -> Self {
        let scenarios = all_scenarios();
        let scenario_index = index % scenarios.len();
        let scenario = scenarios[scenario_index].clone();
        let Built { world, yard, loco, dispatcher, initial_program, economy, road_crews, .. } = build(&scenario);
        let controls = world.train(loco).map(|t| t.controls.clone()).unwrap_or_default();
        let auto = matches!(scenario.kind, ScenarioKind::Terminal | ScenarioKind::Branch);
        let mut s = Sim {
            world,
            yard,
            scenario,
            scenario_index,
            score: Score::default(),
            controls,
            time_scale: std::env::var("HAT_TIME_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
            last_eval: 0.0,
            pending_events: Vec::new(),
            car_index: HashMap::new(),
            messages: Vec::new(),
            reload: None,
            generation,
            engineer: Engineer::new(),
            banner: None,
            ahead: None,
            cur_limit: None,
            crews: Vec::new(),
            yard_crew: None,
            active_loco: None,
            auto,
            manual_touch: false,
            dispatcher,
            last_events: Vec::new(),
            economy,
            wrecker: Wrecker::default(),
        };
        let loco_car = s.world.train(loco).map(|t| t.cars[0].id);
        if let Some(car) = loco_car {
            s.crews.push(Crew::new("R. Casey", car, initial_program, 0.0));
            s.yard_crew = Some(0);
        }
        s.crews.extend(road_crews);
        // Scenarios that start on the levers keep the yard crew off them until asked.
        if !auto {
            for c in s.crews.iter_mut() {
                c.paused = true;
            }
        }
        // In open country the first road train is the one to watch.
        s.active_loco = if s.crews.len() > 1 && s.scenario.kind == ScenarioKind::Branch { Some(s.crews[1].loco_car) } else { loco_car };
        s.rebuild_index();
        s.score.evaluate(&s.world, &s.yard, s.scenario.time_budget);
        s
    }

    pub fn rebuild_index(&mut self) {
        self.car_index.clear();
        for (ti, tr) in self.world.trains.iter().enumerate() {
            for (ci, c) in tr.cars.iter().enumerate() {
                self.car_index.insert(c.id, (ti, ci));
            }
        }
    }

    /// Train index and car index of the active locomotive.
    pub fn loco_pos(&self) -> Option<(usize, usize)> {
        if let Some(id) = self.active_loco {
            if let Some(p) = self.world.train_of_car(id) {
                return Some(p);
            }
        }
        let types = &self.world.car_types;
        self.world.trains.iter().enumerate().find_map(|(ti, t)| t.control_car(types).map(|ci| (ti, ci)))
    }

    /// The crew riding the active locomotive.
    pub fn crew(&self) -> Option<&Crew> {
        let id = self.active_loco.or_else(|| self.loco_car().map(|c| c.id))?;
        self.crews.iter().find(|c| c.loco_car == id)
    }

    pub fn crew_mut(&mut self) -> Option<&mut Crew> {
        let id = self.active_loco.or_else(|| self.loco_car().map(|c| c.id))?;
        self.crews.iter_mut().find(|c| c.loco_car == id)
    }

    /// Make a locomotive the one the cab and the levers belong to.
    pub fn take_loco(&mut self, car: CarId) {
        if self.active_loco == Some(car) {
            return;
        }
        self.active_loco = Some(car);
        self.adopt_train_controls();
        self.auto = self.crew().map(|c| !c.paused).unwrap_or(false);
    }

    /// Call the wreck crew for the train holding `car`.
    pub fn action_rerail(&mut self, sel: Sel) {
        let car = match sel {
            Sel::Car(id) => id,
            Sel::Coupler(f, _) => f,
            Sel::None => match self.loco_car() {
                Some(c) => c.id,
                None => {
                    self.say("Select a car of the derailed train first");
                    return;
                }
            },
        };
        let Some((ti, _)) = self.world.train_of_car(car) else {
            self.say("Car not found");
            return;
        };
        let id = self.world.trains[ti].id;
        match self.wrecker.request(&self.world, id) {
            Ok(job) => {
                let msg = format!("Wreck crew called: about {} and {}", hat_units::fmt::duration(job.done_at - job.started), money(job.cost));
                self.say(msg);
                let t = self.world.t;
                self.engineer.react(t, Mood::Worried, 6.0, "Wreck crew's on the way.", true);
            }
            Err(e) => self.say(e),
        }
    }

    /// Crews with a job, for the payroll.
    pub fn crews_on_duty(&self) -> usize {
        self.crews.iter().filter(|c| c.status == Status::Running && !matches!(c.program, Program::Idle)).count()
    }

    pub fn loco_train(&self) -> Option<&Train> {
        self.loco_pos().map(|(ti, _)| &self.world.trains[ti])
    }

    pub fn loco_car(&self) -> Option<&CarState> {
        self.loco_pos().map(|(ti, ci)| &self.world.trains[ti].cars[ci])
    }

    /// Locomotive speed in its own frame, m/s. Positive is the way the cab points.
    pub fn loco_speed(&self) -> f64 {
        self.loco_car().map(|c| if c.facing_head { c.v } else { -c.v }).unwrap_or(0.0)
    }

    pub fn say(&mut self, msg: impl Into<String>) {
        let t = self.world.t;
        self.messages.push((t, msg.into()));
        if self.messages.len() > 5 {
            self.messages.remove(0);
        }
    }

    fn train_frame_controls(&self) -> Option<(TrainId, Controls)> {
        let (ti, ci) = self.loco_pos()?;
        let tr = &self.world.trains[ti];
        let mut c = self.controls.clone();
        if !tr.cars[ci].facing_head {
            c.reverser = c.reverser.flipped();
        }
        Some((tr.id, c))
    }

    pub fn apply_controls(&mut self) {
        if let Some((id, c)) = self.train_frame_controls() {
            if self.world.train(id).map(|t| t.controls != c).unwrap_or(false) {
                self.world.set_controls(id, c);
            }
        }
    }

    /// After a coupling the train frame may have flipped. Re-read into the loco frame.
    fn adopt_train_controls(&mut self) {
        if let Some((ti, ci)) = self.loco_pos() {
            let tr = &self.world.trains[ti];
            let mut c = tr.controls.clone();
            if !tr.cars[ci].facing_head {
                c.reverser = c.reverser.flipped();
            }
            self.controls = c;
        }
    }

    pub fn resolve_car(&self, id: CarId) -> Option<(TrainId, usize)> {
        self.car_index.get(&id).map(|&(ti, ci)| (self.world.trains[ti].id, ci))
    }

    pub fn resolve_coupler(&self, front: CarId, rear: CarId) -> Option<(TrainId, usize)> {
        let &(ta, ca) = self.car_index.get(&front)?;
        let &(tb, cb) = self.car_index.get(&rear)?;
        if ta == tb && cb == ca + 1 {
            Some((self.world.trains[ta].id, cb))
        } else {
            None
        }
    }

    // Crew actions. Each reports to the HUD.

    pub fn action_pull_pin(&mut self, sel: Sel) {
        match sel {
            Sel::Coupler(f, r) => match self.resolve_coupler(f, r) {
                Some((tid, k)) => match self.world.pull_pin(tid, k) {
                    Ok(_) => self.say("Pin pulled"),
                    Err(e) => self.say(e),
                },
                None => self.say("Those cars are no longer coupled"),
            },
            _ => self.say("Select a coupler first: click between two cars"),
        }
    }

    pub fn action_hand_brake(&mut self, sel: Sel) {
        match sel {
            Sel::Car(id) => match self.resolve_car(id) {
                Some((tid, ci)) => {
                    let on = self.world.train(tid).map(|t| t.cars[ci].hand_brake < 0.5).unwrap_or(true);
                    match self.world.set_hand_brake(tid, ci, on) {
                        Ok(_) => self.say(if on { "Hand brake set" } else { "Hand brake released" }),
                        Err(e) => self.say(e),
                    }
                }
                None => self.say("Car not found"),
            },
            _ => self.say("Select a car first"),
        }
    }

    pub fn action_bleed(&mut self, sel: Sel) {
        match sel {
            Sel::Car(id) => match self.resolve_car(id) {
                Some((tid, ci)) => match self.world.bleed(tid, ci) {
                    Ok(_) => self.say("Car bled: no air brake until recharged"),
                    Err(e) => self.say(e),
                },
                None => self.say("Car not found"),
            },
            _ => self.say("Select a car first"),
        }
    }

    pub fn action_connect_hoses(&mut self, sel: Sel) {
        let tid = match sel {
            Sel::Car(id) => self.resolve_car(id).map(|(t, _)| t),
            Sel::Coupler(f, _) => self.resolve_car(f).map(|(t, _)| t),
            Sel::None => self.loco_train().map(|t| t.id),
        };
        match tid {
            Some(tid) => match self.world.connect_hoses(tid) {
                Ok(_) => self.say("Hoses laced: pipe runs end to end, charging"),
                Err(e) => self.say(e),
            },
            None => self.say("Nothing selected"),
        }
    }

    pub fn action_bottle_air(&mut self, sel: Sel) {
        match sel {
            Sel::Coupler(f, r) => match self.resolve_coupler(f, r) {
                Some((tid, k)) => match self.world.bottle_air(tid, k) {
                    Ok(_) => self.say("Angle cocks closed: a pin pull here keeps the air"),
                    Err(e) => self.say(e),
                },
                None => self.say("Those cars are no longer coupled"),
            },
            _ => self.say("Select a coupler first"),
        }
    }

    pub fn set_dest(&mut self, car: CarId, dest: Option<u32>) {
        if let Some((ti, ci)) = self.world.train_of_car(car) {
            self.world.trains[ti].cars[ci].dest = dest;
        }
    }

    pub fn set_dest_for_kind(&mut self, kind: CarKind, dest: Option<u32>) {
        let types = self.world.car_types.clone();
        for tr in &mut self.world.trains {
            for c in &mut tr.cars {
                if types[c.type_id as usize].kind == kind && !types[c.type_id as usize].is_loco() {
                    c.dest = dest;
                }
            }
        }
    }

    pub fn resume_crew(&mut self) {
        let t = self.world.t;
        if let Some(c) = self.crew_mut() {
            c.paused = false;
            if c.status != Status::Running {
                c.status = Status::Running;
                c.current = None;
            }
            c.replan();
            c.say(t, "Crew has the engine.");
        }
        self.auto = true;
    }

    pub fn pause_crew(&mut self, why: &str) {
        let t = self.world.t;
        if let Some(c) = self.crew_mut() {
            if !c.paused {
                c.paused = true;
                c.say(t, format!("You have the engine ({why})."));
            }
        }
        self.auto = false;
    }

    pub fn action_throw_switch(&mut self, node: NodeId) {
        let name = self.world.graph.node(node).name.clone();
        match self.world.throw_switch(node) {
            Ok(r) => self.say(format!("{name} set {}", route_name(r))),
            Err(e) => self.say(format!("{name}: {e}")),
        }
    }
}

pub fn route_name(r: Route) -> &'static str {
    match r {
        Route::Normal => "straight",
        Route::Diverging => "diverging",
    }
}

fn step_sim(mut sim: ResMut<Sim>) {
    if sim.reload.is_some() {
        return;
    }
    let t0 = sim.world.t;
    if sim.manual_touch {
        if sim.auto {
            sim.pause_crew("you touched a control");
        }
        sim.manual_touch = false;
    }
    let crew_drives = sim.auto && sim.crew().map(|c| !c.paused && c.status == Status::Running).unwrap_or(false);
    if !crew_drives {
        sim.apply_controls();
    }
    let n = sim.time_scale;
    let on_duty = sim.crews_on_duty();
    let mut events: Vec<SimEvent> = Vec::new();
    for _ in 0..n {
        {
            let Sim { crews, yard_crew, world, dispatcher, last_events, economy, wrecker, .. } = &mut *sim;
            if let Some(e) = economy.as_ref() {
                e.before_step(world);
            }
            if let (Some(d), Some(yi)) = (dispatcher.as_mut(), *yard_crew) {
                d.step(world, &mut crews[yi], last_events, DT);
            }
            for c in crews.iter_mut() {
                if !c.paused {
                    c.step(world, DT);
                }
            }
            world.step(DT);
            *last_events = world.take_events();
            if let Some(e) = economy.as_mut() {
                e.after_step(world, last_events, DT, on_duty);
            }
            wrecker.step(world, economy.as_mut().map(|e| &mut e.ledger));
            events.extend(last_events.iter().cloned());
        }
    }
    if crew_drives {
        sim.adopt_train_controls();
        let line = sim.crew().and_then(|c| c.radio.back().cloned());
        if let Some((lt, line)) = line {
            if lt >= t0 {
                let now = sim.world.t;
                sim.engineer.react(now, Mood::Focused, 4.0, &line, false);
            }
        }
    }
    let wreck_lines: Vec<String> = sim.wrecker.log.iter().filter(|(lt, _)| *lt >= t0).map(|(_, l)| l.clone()).collect();
    for l in wreck_lines {
        sim.say(l);
    }
    let t = sim.world.t;
    let frame_changed = events.iter().any(|e| matches!(e, SimEvent::Coupled { .. } | SimEvent::Uncoupled { .. } | SimEvent::KnuckleBreak { .. }));
    {
        let Sim { score, yard, .. } = &mut *sim;
        score.ingest(t, &events, yard);
    }
    sim.pending_events.extend(events.iter().cloned());
    if sim.pending_events.len() > 256 {
        let drop = sim.pending_events.len() - 256;
        sim.pending_events.drain(0..drop);
    }
    if frame_changed {
        sim.adopt_train_controls();
    }
    react_to_events(&mut sim, &events, t);
    update_lookahead(&mut sim, t);
    if t - sim.last_eval >= 0.5 {
        {
            let Sim { score, world, yard, scenario, dispatcher, .. } = &mut *sim;
            if let Some(d) = dispatcher.as_ref() {
                score.cars_delivered = d.cars_delivered;
                score.cargo_delivered = d.cargo_delivered;
            }
            score.evaluate(world, yard, scenario.time_budget);
        }
        sim.last_eval = t;
    }
    sim.rebuild_index();
    sim.messages.retain(|(mt, _)| t - *mt < 8.0);
}

fn handle_reload(mut sim: ResMut<Sim>) {
    if let Some(i) = sim.reload {
        let gen = sim.generation + 1;
        *sim = Sim::load(i, gen);
    }
}

fn react_to_events(sim: &mut Sim, events: &[SimEvent], t: f64) {
    let speed_kph = sim.loco_speed().abs() * 3.6;
    for e in events {
        match e {
            SimEvent::Coupled { hard, rel_speed, .. } => {
                if *hard {
                    sim.engineer.react(t, Mood::Alarmed, 4.0, "Whoa, easy on the joint!", false);
                    sim.banner = Some(Banner { text: format!("HARD JOINT  {:.1} m/s", rel_speed), until: t + 4.0, color: [250, 140, 50] });
                } else {
                    sim.engineer.react(t, Mood::Pleased, 3.0, "Nice joint.", false);
                }
            }
            SimEvent::KnuckleBreak { .. } => {
                sim.engineer.react(t, Mood::Panic, 8.0, "That's a knuckle!", false);
                sim.banner = Some(Banner { text: "KNUCKLE BROKE".into(), until: t + 6.0, color: [240, 70, 60] });
            }
            SimEvent::Derail { cause, .. } => {
                let why = match cause {
                    DerailCause::Overspeed => format!("too fast for the curve at {:.0} km/h", speed_kph),
                    DerailCause::SplitSwitch => "ran through points set against us".to_string(),
                    DerailCause::Collision => "collision".to_string(),
                    DerailCause::Bumper => "hit the bumper".to_string(),
                };
                sim.engineer.react(t, Mood::Panic, 60.0, "We're on the ground.", true);
                sim.banner = Some(Banner { text: format!("DERAILED: {why}. Select a car and call the wreck crew."), until: f64::INFINITY, color: [240, 70, 60] });
            }
            SimEvent::Rerailed { cars, .. } => {
                sim.engineer.react(t, Mood::Pleased, 6.0, "Back on the rails.", true);
                sim.engineer.sticky = false;
                sim.banner = Some(Banner { text: format!("RERAILED {cars} car{}", if *cars == 1 { "" } else { "s" }), until: t + 5.0, color: [110, 200, 120] });
            }
            SimEvent::Emergency { .. } => sim.engineer.react(t, Mood::Alarmed, 5.0, "Big hole!", false),
            SimEvent::BumperHit { speed, .. } => {
                sim.engineer.react(t, Mood::Alarmed, 4.0, "That's the bumper!", false);
                sim.banner = Some(Banner { text: format!("BUMPER HIT  {:.1} m/s", speed), until: t + 3.0, color: [250, 140, 50] });
            }
            _ => {}
        }
    }
}

fn update_lookahead(sim: &mut Sim, t: f64) {
    let (ahead, cur_limit) = match sim.loco_pos() {
        Some((ti, ci)) => {
            let tr = &sim.world.trains[ti];
            let v = tr.speed(&sim.world.car_types);
            let forward = if v.abs() > 0.05 {
                v > 0.0
            } else {
                let s = tr.controls.reverser.sign();
                if s > 0.0 {
                    true
                } else if s < 0.0 {
                    false
                } else {
                    tr.cars[ci].facing_head
                }
            };
            (sim.world.lookahead(tr, forward, 800.0), sim.world.current_limit(tr))
        }
        None => (None, None),
    };
    sim.ahead = ahead;
    sim.cur_limit = cur_limit;

    let v = sim.loco_speed().abs();
    match sim.ahead {
        Some(Ahead::Restriction { limit, distance, .. }) if v > limit && distance < (v * v - limit * limit) / (2.0 * 0.6) * 1.3 => {
            sim.engineer.settle(t, Mood::Worried, "Too fast for the switch ahead!")
        }
        Some(Ahead::SplitSwitch { distance, .. }) if distance < 250.0 && v > 0.1 => sim.engineer.settle(t, Mood::Worried, "Points are against us!"),
        Some(Ahead::End { distance }) if v > 0.5 && distance < v * v / (2.0 * 0.6) * 1.3 => sim.engineer.settle(t, Mood::Worried, "Bumper coming up."),
        _ => {
            if v > 0.3 {
                sim.engineer.settle(t, Mood::Focused, "Rolling.")
            } else {
                sim.engineer.settle(t, Mood::Calm, "Standing by.")
            }
        }
    }
}
