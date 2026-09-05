//! The simulation world: track graph, trains, stepping, contacts and crew actions.

use hat_units::G;

use crate::car::*;
use crate::events::*;
use crate::params::*;
use crate::track::*;
use crate::train::*;
use crate::TrainId;

pub struct World {
    pub graph: TrackGraph,
    pub car_types: Vec<CarType>,
    pub trains: Vec<Train>,
    pub t: f64,
    /// Events since the last `take_events`.
    pub events: Vec<SimEvent>,
    next_train: TrainId,
    next_car: CarId,
}

/// One end of a train, located on an edge.
#[derive(Clone, Copy, Debug)]
struct Face {
    train: usize,
    end: End,
    car: usize,
    edge: EdgeId,
    s: f64,
    forward: bool,
    /// +1 or -1 along `s`, pointing out of the train.
    out_dir: f64,
    /// Face velocity along its outward direction.
    v_out: f64,
    knuckle_open: bool,
    derailed: bool,
    mass: f64,
}

impl World {
    pub fn new(graph: TrackGraph) -> Self {
        World {
            graph,
            car_types: CarType::standard_library(),
            trains: Vec::new(),
            t: 0.0,
            events: Vec::new(),
            next_train: 1,
            next_car: 1,
        }
    }

    pub fn take_events(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn new_car(&mut self, type_id: CarTypeId) -> CarState {
        let id = self.next_car;
        self.next_car += 1;
        CarState::new(id, type_id)
    }

    /// Place a train with its head face at `s_head_face` on `edge`, extending backward.
    pub fn spawn_train(&mut self, cars: Vec<CarState>, edge: EdgeId, s_head_face: f64, forward: bool) -> Result<TrainId, &'static str> {
        if cars.is_empty() {
            return Err("empty train");
        }
        let id = self.next_train;
        let len = self.graph.edge(edge).length;
        let head_x = if forward { s_head_face } else { len - s_head_face };
        let mut train = Train::new(id, cars, vec![PathSeg { edge, forward }], 0.0);
        let mut x = head_x;
        for i in 0..train.cars.len() {
            let l = self.car_types[train.cars[i].type_id as usize].length;
            train.cars[i].x = x - 0.5 * l;
            x -= l;
        }
        if train.extend_tail(&self.graph, &self.car_types) != EndResult::Ok {
            return Err("train does not fit on the track behind that position");
        }
        if train.extend_head(&self.graph, &self.car_types) != EndResult::Ok {
            return Err("train does not fit on the track ahead of that position");
        }
        train.retract(&self.graph, &self.car_types);
        self.next_train += 1;
        self.trains.push(train);
        Ok(id)
    }

    pub fn train(&self, id: TrainId) -> Option<&Train> {
        self.trains.iter().find(|t| t.id == id)
    }

    pub fn train_mut(&mut self, id: TrainId) -> Option<&mut Train> {
        self.trains.iter_mut().find(|t| t.id == id)
    }

    pub fn train_index(&self, id: TrainId) -> Option<usize> {
        self.trains.iter().position(|t| t.id == id)
    }

    pub fn set_controls(&mut self, id: TrainId, controls: Controls) {
        let t = self.t;
        if let Some(tr) = self.train_mut(id) {
            let target = if controls.emergency { 0.0 } else { controls.auto_target };
            tr.push_pipe_cmd(t, target, controls.emergency);
            tr.controls = controls;
        }
    }

    pub fn car_pose(&self, train: &Train, i: usize) -> Option<Pose> {
        train.pose_at(&self.graph, train.cars[i].x)
    }

    pub fn coupler_pose(&self, train: &Train, k: usize) -> Option<Pose> {
        train.pose_at(&self.graph, train.coupler_x(&self.car_types, k))
    }

    pub fn car_location(&self, train: &Train, i: usize) -> Option<Location> {
        train.locate(&self.graph, train.cars[i].x)
    }

    /// A switch is occupied when a train straddles it or a train end is within 3 m.
    pub fn node_occupied(&self, node: NodeId) -> bool {
        for tr in &self.trains {
            for w in tr.path.windows(2) {
                let e0 = self.graph.edge(w[0].edge);
                let shared = if w[0].forward { e0.b } else { e0.a };
                if shared == node {
                    return true;
                }
            }
            for x in [tr.head_face_x(&self.car_types), tr.tail_face_x(&self.car_types)] {
                if let Some(loc) = tr.locate(&self.graph, x) {
                    if let Some(d) = self.graph.distance_to_node(loc.edge, loc.s, node) {
                        if d < 3.0 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn throw_switch(&mut self, node: NodeId) -> Result<Route, &'static str> {
        let Some(setting) = self.graph.turnout_setting(node) else {
            return Err("not a switch");
        };
        if self.node_occupied(node) {
            return Err("switch is occupied");
        }
        let new = setting.other();
        self.graph.set_route(node, new);
        let pos = self.graph.node(node).pos;
        self.events.push(SimEvent::SwitchThrown { node, pos });
        Ok(new)
    }

    /// Uncouple at coupler `k` (between car k-1 and k). The slack must be bunched.
    pub fn pull_pin(&mut self, id: TrainId, k: usize) -> Result<TrainId, &'static str> {
        let idx = self.train_index(id).ok_or("no such train")?;
        let tr = &self.trains[idx];
        if k == 0 || k >= tr.cars.len() {
            return Err("no coupler there");
        }
        if tr.derailed {
            return Err("train is derailed");
        }
        if tr.coupler_extension(&self.car_types, k) > 0.0 {
            return Err("slack is stretched: bunch the slack, then pull the pin");
        }
        let pos = self.coupler_pose(tr, k).map(|p| p.pos).unwrap_or_default();
        let new_id = self.split(idx, k);
        self.events.push(SimEvent::Uncoupled { pos, train: id, new_train: new_id });
        Ok(new_id)
    }

    pub fn set_hand_brake(&mut self, id: TrainId, i: usize, on: bool) -> Result<(), &'static str> {
        let idx = self.train_index(id).ok_or("no such train")?;
        let tr = &mut self.trains[idx];
        let car = tr.cars.get_mut(i).ok_or("no such car")?;
        car.hand_brake = if on { 1.0 } else { 0.0 };
        let car_id = car.id;
        let pos = tr.pose_at(&self.graph, tr.cars[i].x).map(|p| p.pos).unwrap_or_default();
        self.events.push(SimEvent::HandBrake { car: car_id, on, pos });
        Ok(())
    }

    /// Empty a car's reservoirs so it rolls free with no air brake.
    pub fn bleed(&mut self, id: TrainId, i: usize) -> Result<(), &'static str> {
        let idx = self.train_index(id).ok_or("no such train")?;
        let tr = &mut self.trains[idx];
        let car = tr.cars.get_mut(i).ok_or("no such car")?;
        car.brake.p_aux = 0.0;
        car.brake.p_cyl = 0.0;
        car.brake.emergency = false;
        let car_id = car.id;
        let pos = tr.pose_at(&self.graph, tr.cars[i].x).map(|p| p.pos).unwrap_or_default();
        self.events.push(SimEvent::Bled { car: car_id, pos });
        Ok(())
    }

    /// Lace every air hose in the train so the pipe runs end to end.
    pub fn connect_hoses(&mut self, id: TrainId) -> Result<(), &'static str> {
        let idx = self.train_index(id).ok_or("no such train")?;
        let tr = &mut self.trains[idx];
        for h in &mut tr.hoses {
            *h = true;
        }
        let pos = tr.pose_at(&self.graph, tr.cars[0].x).map(|p| p.pos).unwrap_or_default();
        self.events.push(SimEvent::HosesConnected { train: id, pos });
        Ok(())
    }

    /// Close the angle cocks at coupler `k` so a pin pull there does not vent the pipe.
    pub fn bottle_air(&mut self, id: TrainId, k: usize) -> Result<(), &'static str> {
        let idx = self.train_index(id).ok_or("no such train")?;
        let tr = &mut self.trains[idx];
        if k == 0 || k >= tr.cars.len() {
            return Err("no coupler there");
        }
        tr.hoses[k - 1] = false;
        let pos = tr.pose_at(&self.graph, tr.coupler_x(&self.car_types, k)).map(|p| p.pos).unwrap_or_default();
        self.events.push(SimEvent::AirBottled { train: id, pos });
        Ok(())
    }

    fn split(&mut self, idx: usize, k: usize) -> TrainId {
        let new_id = self.next_train;
        self.next_train += 1;
        let types = &self.car_types;
        let graph = &self.graph;
        let train = &mut self.trains[idx];
        let vent = train.hoses[k - 1];
        let tail_cars = train.cars.split_off(k);
        let mut tail_zones = train.zones.split_off(k - 1);
        tail_zones.remove(0);
        let mut tail_hoses = train.hoses.split_off(k - 1);
        tail_hoses.remove(0);
        let mut t2 = Train::new(new_id, tail_cars, train.path.clone(), train.path_origin);
        t2.zones = tail_zones;
        t2.hoses = tail_hoses;
        t2.derailed = train.derailed;
        let grace = self.t + UNCOUPLE_GRACE;
        if let Some(c) = train.cars.last_mut() {
            c.knuckle_open[1] = true;
            c.no_couple_until[1] = grace;
        }
        t2.cars[0].knuckle_open[0] = true;
        t2.cars[0].no_couple_until[0] = grace;
        if train.control_car(types).is_none() && t2.control_car(types).is_some() {
            t2.controls = std::mem::take(&mut train.controls);
            t2.pipe_cmds = train.pipe_cmds.clone();
        }
        if vent {
            for c in train.cars.iter_mut().chain(t2.cars.iter_mut()) {
                c.brake.p_pipe = 0.0;
                if !c.brake.is_bled() {
                    c.brake.emergency = true;
                }
            }
            let pos = train.pose_at(graph, train.tail_face_x(types)).map(|p| p.pos).unwrap_or_default();
            self.events.push(SimEvent::PipeVent { pos, train: train.id });
        }
        train.retract(graph, types);
        t2.retract(graph, types);
        self.trains.push(t2);
        new_id
    }

    /// Advance the world by `dt` seconds.
    pub fn step(&mut self, dt: f64) {
        let mut breaks: Vec<(TrainId, usize)> = Vec::new();
        {
            let World { graph, car_types, trains, events, t, .. } = self;
            for train in trains.iter_mut() {
                if train.derailed {
                    for c in &mut train.cars {
                        c.v = 0.0;
                    }
                    continue;
                }
                step_train(train, graph, car_types, *t, dt, events, &mut breaks);
            }
        }
        for (id, k) in breaks {
            if let Some(idx) = self.train_index(id) {
                if k > 0 && k < self.trains[idx].cars.len() {
                    let pos = self.coupler_pose(&self.trains[idx], k).map(|p| p.pos).unwrap_or_default();
                    let new_id = self.split(idx, k);
                    self.events.push(SimEvent::KnuckleBreak { pos, train: id, new_train: new_id });
                }
            }
        }
        self.resolve_contacts();
        self.check_overlaps();
        self.t += dt;
    }

    fn collect_faces(&self) -> Vec<Face> {
        let types = &self.car_types;
        let graph = &self.graph;
        let mut faces = Vec::with_capacity(self.trains.len() * 2);
        for (ti, tr) in self.trains.iter().enumerate() {
            if tr.cars.is_empty() {
                continue;
            }
            let n = tr.cars.len();
            if let Some(l) = tr.locate(graph, tr.head_face_x(types)) {
                let c = &tr.cars[0];
                faces.push(Face {
                    train: ti,
                    end: End::Head,
                    car: 0,
                    edge: l.edge,
                    s: l.s,
                    forward: l.forward,
                    out_dir: if l.forward { 1.0 } else { -1.0 },
                    v_out: c.v,
                    knuckle_open: c.knuckle_open[0] && self.t >= c.no_couple_until[0],
                    derailed: tr.derailed,
                    mass: c.mass(&types[c.type_id as usize]),
                });
            }
            if let Some(l) = tr.locate(graph, tr.tail_face_x(types)) {
                let c = &tr.cars[n - 1];
                faces.push(Face {
                    train: ti,
                    end: End::Tail,
                    car: n - 1,
                    edge: l.edge,
                    s: l.s,
                    forward: l.forward,
                    out_dir: if l.forward { -1.0 } else { 1.0 },
                    v_out: -c.v,
                    knuckle_open: c.knuckle_open[1] && self.t >= c.no_couple_until[1],
                    derailed: tr.derailed,
                    mass: c.mass(&types[c.type_id as usize]),
                });
            }
        }
        faces
    }

    fn resolve_contacts(&mut self) {
        for _ in 0..64 {
            let faces = self.collect_faces();
            let mut found = None;
            'search: for i in 0..faces.len() {
                for j in (i + 1)..faces.len() {
                    let (a, b) = (faces[i], faces[j]);
                    if a.train == b.train || a.edge != b.edge || a.out_dir == b.out_dir {
                        continue;
                    }
                    if a.derailed && b.derailed {
                        continue;
                    }
                    let gap = (b.s - a.s) * a.out_dir;
                    let closing = a.v_out + b.v_out;
                    if gap <= CONTACT_EPS && gap >= -MAX_PENETRATION && (closing > 0.0 || gap < -1e-6) {
                        found = Some((a, b, gap, closing));
                        break 'search;
                    }
                }
            }
            let Some((a, b, gap, closing)) = found else { return };
            let rel = closing.max(0.0);
            if rel > COLLISION_DERAIL_SPEED {
                self.derail_pair(a, b, DerailCause::Collision);
                continue;
            }
            if a.derailed || b.derailed {
                if rel > COUPLE_MAX_SPEED {
                    self.derail_pair(a, b, DerailCause::Collision);
                } else {
                    self.bump(a, b, gap, rel);
                }
                continue;
            }
            if (a.knuckle_open || b.knuckle_open) && rel >= MIN_COUPLE_SPEED {
                self.couple(a, b, rel);
            } else {
                self.bump(a, b, gap, rel);
            }
        }
    }

    fn derail_pair(&mut self, a: Face, b: Face, cause: DerailCause) {
        let World { graph, car_types, trains, events, .. } = self;
        for f in [a, b] {
            let tr = &mut trains[f.train];
            if !tr.derailed {
                derail_train(tr, graph, car_types, f.car, cause, events);
            }
        }
    }

    /// Join two trains at touching faces. `a` and `b` are on the same edge, facing.
    fn couple(&mut self, a: Face, b: Face, rel: f64) {
        let (mut a, mut b) = (a, b);
        if a.end == End::Head {
            std::mem::swap(&mut a, &mut b);
        }
        let World { graph, car_types: types, trains, events, .. } = self;
        let (ai, bi) = (a.train, b.train);
        let mut btrain = trains.remove(bi);
        let ai = if bi < ai { ai - 1 } else { ai };
        if b.end == End::Tail {
            btrain.flip(graph);
        }
        let atrain = &mut trains[ai];
        let a_seg = atrain.path[0];
        let b_seg = *btrain.path.last().unwrap();
        debug_assert_eq!(a_seg.edge, b_seg.edge, "coupling faces must share an edge");
        if a_seg.forward != b_seg.forward {
            btrain.flip(graph);
        }
        let b_last = btrain.path.len() - 1;
        let delta = atrain.seg_start_x(graph, 0) - btrain.seg_start_x(graph, b_last);
        for c in &mut btrain.cars {
            c.x += delta;
        }
        btrain.path_origin += delta;

        let na = atrain.cars.len();
        let la = types[atrain.cars[na - 1].type_id as usize].length;
        let lb = types[btrain.cars[0].type_id as usize].length;
        let desired = atrain.cars[na - 1].x - 0.5 * (la + lb) + SLACK_HALF;
        let shift = desired - btrain.cars[0].x;
        for c in &mut btrain.cars {
            c.x += shift;
        }

        let ma = atrain.cars[na - 1].mass(&types[atrain.cars[na - 1].type_id as usize]);
        let mb = btrain.cars[0].mass(&types[btrain.cars[0].type_id as usize]);
        let vc = (ma * atrain.cars[na - 1].v + mb * btrain.cars[0].v) / (ma + mb);
        atrain.cars[na - 1].v = vc;
        btrain.cars[0].v = vc;
        let hard = rel > COUPLE_MAX_SPEED;
        if hard {
            let d = 0.5 * (rel - COUPLE_MAX_SPEED);
            atrain.cars[na - 1].damage += d;
            btrain.cars[0].damage += d;
        }
        atrain.cars[na - 1].knuckle_open[1] = false;
        btrain.cars[0].knuckle_open[0] = false;

        let a_has_loco = atrain.control_car(types).is_some();
        let pos = atrain.pose_at(graph, atrain.tail_face_x(types)).map(|p| p.pos).unwrap_or_default();
        atrain.zones.push(Zone::Buff);
        atrain.zones.extend(btrain.zones.iter().copied());
        atrain.hoses.push(false);
        atrain.hoses.extend(btrain.hoses.iter().copied());
        atrain.cars.extend(btrain.cars.drain(..));
        let mut path = btrain.path.clone();
        path.extend(atrain.path.drain(1..));
        atrain.path = path;
        atrain.path_origin = btrain.path_origin;
        if !a_has_loco {
            atrain.controls = btrain.controls.clone();
            atrain.pipe_cmds = btrain.pipe_cmds.clone();
        }
        atrain.derailed |= btrain.derailed;
        atrain.retract(graph, types);
        events.push(SimEvent::Coupled { pos, rel_speed: rel, train: atrain.id, hard });
    }

    /// Two closed knuckles meet: exchange momentum, separate, no join.
    fn bump(&mut self, a: Face, b: Face, gap: f64, rel: f64) {
        let World { graph, car_types: types, trains, events, .. } = self;
        let sa = if a.forward { 1.0 } else { -1.0 };
        let sb = if b.forward { 1.0 } else { -1.0 };
        let va = trains[a.train].cars[a.car].v * sa;
        let vb = trains[b.train].cars[b.car].v * sb;
        let (ma, mb) = (a.mass, b.mass);
        let e = BUMP_RESTITUTION;
        let va2 = (ma * va + mb * vb - mb * e * (va - vb)) / (ma + mb);
        let vb2 = (ma * va + mb * vb + ma * e * (va - vb)) / (ma + mb);
        trains[a.train].cars[a.car].v = va2 * sa;
        trains[b.train].cars[b.car].v = vb2 * sb;
        if gap < 0.0 {
            let mover = if b.derailed { a } else { b };
            let dir = if mover.end == End::Head { -1.0 } else { 1.0 };
            let tr = &mut trains[mover.train];
            for c in &mut tr.cars {
                c.x += dir * (-gap);
            }
            let _ = tr.extend_head(graph, types);
            let _ = tr.extend_tail(graph, types);
            tr.retract(graph, types);
        }
        if rel > COUPLE_MAX_SPEED {
            let d = 0.5 * (rel - COUPLE_MAX_SPEED);
            trains[a.train].cars[a.car].damage += d;
            trains[b.train].cars[b.car].damage += d;
        }
        let pos = graph.pose_on_edge(a.edge, a.s).pos;
        events.push(SimEvent::Bumped { pos, rel_speed: rel });
    }

    /// Trains occupying the same stretch of an edge have collided side-on or run through.
    fn check_overlaps(&mut self) {
        struct Iv {
            train: usize,
            edge: EdgeId,
            s0: f64,
            s1: f64,
        }
        let World { graph, car_types: types, trains, events, .. } = self;
        let mut ivs: Vec<Iv> = Vec::new();
        for (ti, tr) in trains.iter().enumerate() {
            let head = tr.head_face_x(types);
            let tail = tr.tail_face_x(types);
            let mut start = tr.path_origin;
            for seg in &tr.path {
                let len = graph.edge(seg.edge).length;
                let x0 = tail.max(start);
                let x1 = head.min(start + len);
                if x1 > x0 {
                    let (l0, l1) = (x0 - start, x1 - start);
                    let (s0, s1) = if seg.forward { (l0, l1) } else { (len - l1, len - l0) };
                    ivs.push(Iv { train: ti, edge: seg.edge, s0, s1 });
                }
                start += len;
            }
        }
        ivs.sort_by(|a, b| a.edge.cmp(&b.edge).then(a.s0.total_cmp(&b.s0)));
        let mut hits: Vec<(usize, usize, EdgeId, f64)> = Vec::new();
        for i in 0..ivs.len() {
            for j in (i + 1)..ivs.len() {
                if ivs[j].edge != ivs[i].edge || ivs[j].s0 >= ivs[i].s1 {
                    break;
                }
                if ivs[i].train == ivs[j].train {
                    continue;
                }
                let overlap = ivs[i].s1.min(ivs[j].s1) - ivs[j].s0;
                if overlap > OVERLAP_DERAIL {
                    hits.push((ivs[i].train, ivs[j].train, ivs[i].edge, ivs[j].s0 + 0.5 * overlap));
                }
            }
        }
        for (ta, tb, edge, s) in hits {
            if trains[ta].derailed && trains[tb].derailed {
                continue;
            }
            for ti in [ta, tb] {
                let tr = &mut trains[ti];
                if tr.derailed {
                    continue;
                }
                let car = nearest_car(tr, graph, edge, s);
                derail_train(tr, graph, types, car, DerailCause::Collision, events);
            }
        }
    }
}

fn nearest_car(tr: &Train, graph: &TrackGraph, edge: EdgeId, s: f64) -> usize {
    let mut best = (0usize, f64::INFINITY);
    for (i, c) in tr.cars.iter().enumerate() {
        if let Some(l) = tr.locate(graph, c.x) {
            if l.edge == edge {
                let d = (l.s - s).abs();
                if d < best.1 {
                    best = (i, d);
                }
            }
        }
    }
    best.0
}

fn derail_train(train: &mut Train, graph: &TrackGraph, types: &[CarType], car_idx: usize, cause: DerailCause, events: &mut Vec<SimEvent>) {
    let _ = types;
    train.derailed = true;
    for c in &mut train.cars {
        c.v = 0.0;
    }
    let car_idx = car_idx.min(train.cars.len() - 1);
    train.cars[car_idx].derailed = true;
    let pos = train.pose_at(graph, train.cars[car_idx].x).map(|p| p.pos).unwrap_or_default();
    events.push(SimEvent::Derail { pos, cause, train: train.id });
}

/// Force through one coupler pair, positive in tension, and the zone it is in.
fn coupler_force(e: f64, de: f64) -> (f64, Zone) {
    if e > SLACK_HALF {
        let d = e - SLACK_HALF;
        let mut f = DRAFT_GEAR_K * d + DRAFT_GEAR_C * de;
        if d > DRAFT_GEAR_TRAVEL {
            f += STOP_K * (d - DRAFT_GEAR_TRAVEL);
        }
        (f.max(0.0), Zone::Draft)
    } else if e < -SLACK_HALF {
        let d = e + SLACK_HALF;
        let mut f = DRAFT_GEAR_K * d + DRAFT_GEAR_C * de;
        if d < -DRAFT_GEAR_TRAVEL {
            f += STOP_K * (d + DRAFT_GEAR_TRAVEL);
        }
        (f.min(0.0), Zone::Buff)
    } else {
        (0.0, Zone::Free)
    }
}

/// One fixed step for one train: couplers, forces, integration, brakes, path ends.
fn step_train(train: &mut Train, graph: &TrackGraph, types: &[CarType], t: f64, dt: f64, events: &mut Vec<SimEvent>, breaks: &mut Vec<(TrainId, usize)>) {
    let n = train.cars.len();
    let ctrl = train.control_car(types);

    // Coupler forces.
    let mut f = vec![0.0f64; n];
    let mut impacts: Vec<(usize, f64)> = Vec::new();
    for k in 1..n {
        let e = train.coupler_extension(types, k);
        let de = train.cars[k - 1].v - train.cars[k].v;
        let (fc, zone) = coupler_force(e, de);
        f[k] += fc;
        f[k - 1] -= fc;
        let prev = train.zones[k - 1];
        if zone != prev && zone != Zone::Free && de.abs() > 0.05 {
            impacts.push((k, de.abs()));
        }
        train.zones[k - 1] = zone;
        if fc > KNUCKLE_BREAK {
            breaks.push((train.id, k));
        }
    }

    // Track under each car.
    let locs: Vec<Option<Location>> = train.cars.iter().map(|c| train.locate(graph, c.x)).collect();

    // Forces and integration.
    let mut overspeed: Option<usize> = None;
    for i in 0..n {
        let ct = &types[train.cars[i].type_id as usize];
        let (grade, curv, limit) = match locs[i] {
            Some(l) => {
                let e = graph.edge(l.edge);
                (if l.forward { e.grade } else { -e.grade }, e.geom.curvature(), e.speed_limit)
            }
            None => (0.0, 0.0, f64::INFINITY),
        };
        let controls = &train.controls;
        let car = &mut train.cars[i];
        let m = ct.m_tare + car.m_payload;
        let mut drive = f[i] - m * G * grade;
        let mut ind = 0.0;
        if let Some(spec) = &ct.loco {
            let r = controls.reverser.sign();
            if r != 0.0 && controls.throttle > 0 {
                let notch = controls.throttle.min(8) as f64 / 8.0;
                let te = (spec.power_rail / car.v.abs().max(0.5)).min(spec.adhesion * m * G);
                drive += r * notch * te;
            }
            ind = controls.independent.clamp(0.0, 1.0) * spec.independent_max;
        }
        let vabs = car.v.abs();
        let axles = ct.n_axles as f64;
        let rolling = m * (DAVIS_A + DAVIS_B * vabs) + DAVIS_C * axles + DAVIS_D * vabs * vabs + m * CURVE_RESISTANCE * curv;
        let air = (ct.m_tare * G * BRAKE_RATIO * (car.brake.p_cyl / FULL_SERVICE_CYL)).min(BRAKE_ADHESION * m * G);
        let hb = car.hand_brake * HAND_BRAKE_RATIO * m * G;
        let fric = rolling + air + hb + ind;
        let fric_static = STARTING_RESISTANCE_FACTOR * (m * DAVIS_A + DAVIS_C * axles) + air + hb + ind;
        if vabs < V_EPS {
            if drive.abs() <= fric_static {
                car.v = 0.0;
            } else {
                car.v += (drive - fric_static * drive.signum()) / m * dt;
            }
        } else {
            let s = car.v.signum();
            let vn = car.v + (drive - fric * s) / m * dt;
            car.v = if vn * car.v < 0.0 { 0.0 } else { vn };
        }
        if curv > 0.0 && car.v.abs() > OVERSPEED_DERAIL_FACTOR * limit && overspeed.is_none() {
            overspeed = Some(i);
        }
    }
    for c in &mut train.cars {
        c.x += c.v * dt;
    }

    // Air brakes.
    let connected = train.pipe_connectivity(ctrl);
    let ctrl_x = ctrl.map(|c| train.cars[c].x);
    let mut emergencies = 0;
    for i in 0..n {
        let dist = ctrl_x.map(|cx| (train.cars[i].x - cx).abs()).unwrap_or(0.0);
        let target = if connected[i] { Some(train.pipe_target_at(t, dist)) } else { None };
        let b = &mut train.cars[i].brake;
        let p0 = b.p_pipe;
        match target {
            Some((tg, em)) => {
                if tg < p0 {
                    let rate = if em { PIPE_EMERGENCY_RATE } else { PIPE_SERVICE_RATE };
                    b.p_pipe = (p0 - rate * dt).max(tg);
                } else if tg > p0 {
                    let rate = PIPE_CHARGE_RATE / (1.0 + dist / 400.0);
                    b.p_pipe = (p0 + rate * dt).min(tg);
                }
            }
            None => {
                b.p_pipe = (p0 - LEAK_RATE * dt).max(0.0);
            }
        }
        let dp = b.p_pipe - p0;
        let rising = dp > 0.0;
        let bled = b.is_bled();
        if !bled && dp / dt < -EMERGENCY_TRIGGER_RATE && !b.emergency {
            b.emergency = true;
            emergencies += 1;
        }
        if b.emergency && rising && b.p_pipe > 0.6 * PIPE_REF {
            b.emergency = false;
        }
        if b.p_pipe > b.p_aux {
            b.p_aux = (b.p_aux + AUX_CHARGE_RATE * dt).min(b.p_pipe);
        }
        let cyl_target = if bled {
            0.0
        } else if b.emergency {
            EMERGENCY_CYL.min(b.p_aux)
        } else if rising {
            0.0
        } else {
            (CYL_PER_PIPE_REDUCTION * (PIPE_REF - b.p_pipe)).clamp(0.0, FULL_SERVICE_CYL).min(b.p_aux)
        };
        let tau = if cyl_target > b.p_cyl { CYL_APPLY_TAU } else { CYL_RELEASE_TAU };
        b.p_cyl += (cyl_target - b.p_cyl) * (1.0 - (-dt / tau).exp());
        b.p_aux = (b.p_aux - LEAK_RATE * dt).max(0.0);
        if b.p_cyl > b.p_aux {
            b.p_cyl = b.p_aux;
        }
    }

    if let Some(i) = overspeed {
        derail_train(train, graph, types, i, DerailCause::Overspeed, events);
    }

    // Path ends.
    match train.extend_head(graph, types) {
        EndResult::Ok => {}
        EndResult::Bumper { overshoot } => {
            for c in &mut train.cars {
                c.x -= overshoot;
            }
            let v = train.cars[0].v;
            if v > 0.0 {
                train.cars[0].v = 0.0;
                let pos = train.pose_at(graph, train.head_face_x(types)).map(|p| p.pos).unwrap_or_default();
                if v > BUMPER_DAMAGE_SPEED {
                    train.cars[0].damage += 0.1 * v;
                    events.push(SimEvent::BumperHit { pos, speed: v, train: train.id });
                }
                if v > COLLISION_DERAIL_SPEED {
                    derail_train(train, graph, types, 0, DerailCause::Bumper, events);
                }
            }
        }
        EndResult::SplitSwitch { overshoot } => {
            for c in &mut train.cars {
                c.x -= overshoot;
            }
            derail_train(train, graph, types, 0, DerailCause::SplitSwitch, events);
        }
    }
    match train.extend_tail(graph, types) {
        EndResult::Ok => {}
        EndResult::Bumper { overshoot } => {
            for c in &mut train.cars {
                c.x += overshoot;
            }
            let last = train.cars.len() - 1;
            let v = train.cars[last].v;
            if v < 0.0 {
                train.cars[last].v = 0.0;
                let pos = train.pose_at(graph, train.tail_face_x(types)).map(|p| p.pos).unwrap_or_default();
                if -v > BUMPER_DAMAGE_SPEED {
                    train.cars[last].damage += -0.1 * v;
                    events.push(SimEvent::BumperHit { pos, speed: -v, train: train.id });
                }
                if -v > COLLISION_DERAIL_SPEED {
                    derail_train(train, graph, types, last, DerailCause::Bumper, events);
                }
            }
        }
        EndResult::SplitSwitch { overshoot } => {
            for c in &mut train.cars {
                c.x += overshoot;
            }
            let last = train.cars.len() - 1;
            derail_train(train, graph, types, last, DerailCause::SplitSwitch, events);
        }
    }
    train.retract(graph, types);

    for (k, rel) in impacts {
        let pos = train.pose_at(graph, train.coupler_x(types, k)).map(|p| p.pos).unwrap_or_default();
        let mass = train.cars[k - 1].mass(&types[train.cars[k - 1].type_id as usize]) + train.cars[k].mass(&types[train.cars[k].type_id as usize]);
        events.push(SimEvent::CouplerImpact { pos, rel_speed: rel, mass });
    }
    if emergencies > 0 {
        let pos = train.pose_at(graph, train.cars[0].x).map(|p| p.pos).unwrap_or_default();
        events.push(SimEvent::Emergency { pos, train: train.id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DT;
    use glam::DVec2;

    fn straight_world(len: f64) -> World {
        let mut g = TrackGraph::default();
        let a = g.add_node(DVec2::new(0.0, 0.0), NodeKind::End, "W");
        let b = g.add_node(DVec2::new(len, 0.0), NodeKind::End, "E");
        g.add_edge(a, b, Geometry::Straight { a: DVec2::new(0.0, 0.0), b: DVec2::new(len, 0.0) }, 0.0, 30.0, Some(0));
        World::new(g)
    }

    fn hoppers(w: &mut World, n: usize, payload: f64, brake: Brake) -> Vec<CarState> {
        (0..n)
            .map(|_| {
                let mut c = w.new_car(TYPE_OPEN_HOPPER);
                c.m_payload = payload;
                c.brake = brake.clone();
                c.knuckle_open = [true, true];
                c
            })
            .collect()
    }

    fn loco(w: &mut World) -> CarState {
        let mut c = w.new_car(TYPE_SWITCHER);
        c.brake = Brake::charged();
        c.knuckle_open = [true, true];
        c
    }

    fn run(w: &mut World, seconds: f64) {
        for _ in 0..(seconds / DT).round() as usize {
            w.step(DT);
        }
    }

    fn stretch_scenario() -> (World, TrainId) {
        let mut w = straight_world(5000.0);
        let mut cars = vec![loco(&mut w)];
        cars.extend(hoppers(&mut w, 40, 100_000.0, Brake::bled()));
        let id = w.spawn_train(cars, 0, 2000.0, true).unwrap();
        w.set_controls(id, Controls { throttle: 8, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
        (w, id)
    }

    #[test]
    fn stretch_wave_runs_head_to_tail() {
        let (mut w, id) = stretch_scenario();
        let mut first_draft = vec![None; 40];
        for step in 0..(30.0 / DT) as usize {
            w.step(DT);
            let tr = w.train(id).unwrap();
            for k in 0..40 {
                if first_draft[k].is_none() && tr.zones[k] == Zone::Draft {
                    first_draft[k] = Some(step);
                }
            }
        }
        for k in 0..40 {
            assert!(first_draft[k].is_some(), "coupler {k} never went into draft");
        }
        for k in 1..40 {
            assert!(first_draft[k] >= first_draft[k - 1], "wave out of order at {k}: {:?} < {:?}", first_draft[k], first_draft[k - 1]);
        }
        assert!(first_draft[39].unwrap() > first_draft[0].unwrap() + 12, "wave should take time to reach the rear");
        assert!(w.train(id).unwrap().speed(&w.car_types) > 0.5, "train should be rolling");
        assert!(w.events.iter().any(|e| matches!(e, SimEvent::CouplerImpact { .. })), "slack running out should bang");
    }

    #[test]
    fn deterministic_across_runs() {
        let (mut a, ida) = stretch_scenario();
        let (mut b, idb) = stretch_scenario();
        run(&mut a, 20.0);
        run(&mut b, 20.0);
        let xa: Vec<(u64, u64)> = a.train(ida).unwrap().cars.iter().map(|c| (c.x.to_bits(), c.v.to_bits())).collect();
        let xb: Vec<(u64, u64)> = b.train(idb).unwrap().cars.iter().map(|c| (c.x.to_bits(), c.v.to_bits())).collect();
        assert_eq!(xa, xb);
    }

    #[test]
    fn rear_brakes_apply_seconds_after_head() {
        let mut w = straight_world(6000.0);
        let mut cars = vec![loco(&mut w)];
        cars.extend(hoppers(&mut w, 100, 100_000.0, Brake::charged()));
        let id = w.spawn_train(cars, 0, 4000.0, true).unwrap();
        {
            let tr = w.train_mut(id).unwrap();
            for h in &mut tr.hoses {
                *h = true;
            }
            for c in &mut tr.cars {
                c.v = 10.0;
            }
        }
        w.set_controls(id, Controls { throttle: 0, reverser: Reverser::Forward, independent: 0.0, auto_target: FULL_SERVICE_PIPE, ..Default::default() });
        let (mut t_head, mut t_rear) = (None, None);
        for _ in 0..(40.0 / DT) as usize {
            w.step(DT);
            let tr = w.train(id).unwrap();
            if t_head.is_none() && tr.cars[1].brake.p_cyl > 20_000.0 {
                t_head = Some(w.t);
            }
            if t_rear.is_none() && tr.cars[100].brake.p_cyl > 20_000.0 {
                t_rear = Some(w.t);
            }
            if t_head.is_some() && t_rear.is_some() {
                break;
            }
        }
        let (h, r) = (t_head.expect("head never braked"), t_rear.expect("rear never braked"));
        assert!(r - h > 8.0 && r - h < 16.0, "rear should lag the head by roughly ten seconds, got {}", r - h);
        assert!(!w.train(id).unwrap().cars.iter().any(|c| c.brake.emergency), "service application must not trigger emergency");
        assert!(w.train(id).unwrap().speed(&w.car_types) < 10.0, "train should be slowing");
    }

    #[test]
    fn open_knuckle_couples_and_pin_needs_bunched_slack() {
        let mut w = straight_world(3000.0);
        let l = loco(&mut w);
        let lid = w.spawn_train(vec![l], 0, 1000.0, true).unwrap();
        let cut = hoppers(&mut w, 3, 0.0, Brake::bled());
        w.spawn_train(cut, 0, 1100.0, true).unwrap();
        w.train_mut(lid).unwrap().cars[0].v = 1.5;
        w.set_controls(lid, Controls { independent: 0.0, reverser: Reverser::Forward, ..Default::default() });
        run(&mut w, 70.0);
        assert_eq!(w.trains.len(), 1, "should have coupled into one train");
        assert_eq!(w.trains[0].cars.len(), 4);
        assert!(w.events.iter().any(|e| matches!(e, SimEvent::Coupled { hard: false, .. })));
        let id = w.trains[0].id;
        let loco_idx = w.trains[0].control_car(&w.car_types).unwrap();
        let (k, pull) = if loco_idx == 0 { (1, Reverser::Forward) } else { (loco_idx, Reverser::Reverse) };

        w.set_controls(id, Controls { throttle: 4, reverser: pull, independent: 0.0, ..Default::default() });
        run(&mut w, 3.0);
        assert!(w.pull_pin(id, k).is_err(), "pin must not pull under draft");

        w.set_controls(id, Controls { throttle: 2, reverser: pull.flipped(), independent: 0.0, ..Default::default() });
        run(&mut w, 4.0);
        let new = w.pull_pin(id, k).expect("pin should pull with bunched slack");
        assert_eq!(w.trains.len(), 2);
        let sizes = { let mut s: Vec<usize> = w.trains.iter().map(|t| t.cars.len()).collect(); s.sort(); s };
        assert_eq!(sizes, vec![1, 3]);
        assert!(w.train(new).is_some());
    }

    #[test]
    fn closed_knuckles_bump_without_coupling() {
        let mut w = straight_world(3000.0);
        let mut l = loco(&mut w);
        l.knuckle_open = [false, false];
        let lid = w.spawn_train(vec![l], 0, 1000.0, true).unwrap();
        let mut cut = hoppers(&mut w, 2, 0.0, Brake::bled());
        for c in &mut cut {
            c.knuckle_open = [false, false];
        }
        let cid = w.spawn_train(cut, 0, 1100.0, true).unwrap();
        let x_before = w.train(cid).unwrap().cars[0].x;
        w.train_mut(lid).unwrap().cars[0].v = 1.5;
        w.set_controls(lid, Controls { independent: 0.0, reverser: Reverser::Forward, ..Default::default() });
        run(&mut w, 60.0);
        assert_eq!(w.trains.len(), 2, "closed knuckles must not couple");
        assert!(w.events.iter().any(|e| matches!(e, SimEvent::Bumped { .. })));
        assert!(w.train(cid).unwrap().cars[0].x > x_before + 0.5, "the cut should have been shoved");
        assert!(w.trains.iter().all(|t| !t.derailed));
    }

    #[test]
    fn hand_brakes_hold_on_grade_and_release_rolls() {
        let mut g = TrackGraph::default();
        let a = g.add_node(DVec2::new(0.0, 0.0), NodeKind::End, "W");
        let b = g.add_node(DVec2::new(2000.0, 0.0), NodeKind::End, "E");
        g.add_edge(a, b, Geometry::Straight { a: DVec2::new(0.0, 0.0), b: DVec2::new(2000.0, 0.0) }, 0.01, 30.0, Some(0));
        let mut w = World::new(g);
        let mut cut = hoppers(&mut w, 2, 100_000.0, Brake::bled());
        for c in &mut cut {
            c.hand_brake = 1.0;
        }
        let id = w.spawn_train(cut, 0, 1000.0, true).unwrap();
        run(&mut w, 10.0);
        assert!(w.train(id).unwrap().cars[0].v.abs() < 1e-9, "hand brakes should hold on 1 %");
        w.set_hand_brake(id, 0, false).unwrap();
        w.set_hand_brake(id, 1, false).unwrap();
        run(&mut w, 20.0);
        assert!(w.train(id).unwrap().cars[0].v < -0.5, "released cars should roll downhill, v = {}", w.train(id).unwrap().cars[0].v);
    }

    #[test]
    fn bumper_stops_a_slow_train_without_derailing() {
        let mut w = straight_world(1200.0);
        let l = loco(&mut w);
        let id = w.spawn_train(vec![l], 0, 1190.0, true).unwrap();
        w.train_mut(id).unwrap().cars[0].v = 0.5;
        w.set_controls(id, Controls { independent: 0.0, reverser: Reverser::Forward, ..Default::default() });
        run(&mut w, 30.0);
        let tr = w.train(id).unwrap();
        assert!(!tr.derailed);
        assert!(tr.cars[0].v.abs() < 1e-9);
        assert!(tr.head_face_x(&w.car_types) <= tr.path_end_x(&w.graph) + 1e-6);
    }

    #[test]
    #[ignore]
    fn hundred_thousand_cars_budget() {
        let mut w = straight_world(2_000_000.0);
        for k in 0..500 {
            let mut cars = vec![loco(&mut w)];
            cars.extend(hoppers(&mut w, 199, 100_000.0, Brake::charged()));
            let id = w.spawn_train(cars, 0, 3500.0 * (k as f64 + 1.0), true).unwrap();
            w.set_controls(id, Controls { throttle: 8, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
        }
        let start = std::time::Instant::now();
        for _ in 0..120 {
            w.step(DT);
        }
        let el = start.elapsed();
        eprintln!("100k cars, 120 steps: {:?} ({:.2} ms/step)", el, el.as_secs_f64() * 1000.0 / 120.0);
        assert!(el.as_secs_f64() < 2.0, "one second of sim took {el:?}");
    }
}

/// Something coming up along the track ahead of a train.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ahead {
    /// A lower speed limit starts `distance` meters ahead.
    Restriction { limit: f64, distance: f64, track: Option<u32> },
    /// Points set against a trailing move: a derailment if reached.
    SplitSwitch { node: NodeId, distance: f64 },
    /// A bumper.
    End { distance: f64 },
}

impl World {
    /// Walk the track from the train's leading face in its direction of travel and report
    /// the first thing that matters within `max_dist`. `forward` is the +x direction of the
    /// train; pass the sign of its velocity, or its facing when stopped.
    pub fn lookahead(&self, train: &Train, forward: bool, max_dist: f64) -> Option<Ahead> {
        let types = &self.car_types;
        let face_x = if forward { train.head_face_x(types) } else { train.tail_face_x(types) };
        let loc = train.locate(&self.graph, face_x)?;
        // Direction along `s` on the current edge.
        let mut along_s = loc.forward == forward;
        let mut edge = loc.edge;
        let e = self.graph.edge(edge);
        let mut dist = if along_s { e.length - loc.s } else { loc.s };
        let mut limit = e.speed_limit;
        let mut node = if along_s { e.b } else { e.a };
        while dist <= max_dist {
            match self.graph.exit(edge, node) {
                Exit::End => return Some(Ahead::End { distance: dist }),
                Exit::SplitSwitch => return Some(Ahead::SplitSwitch { node, distance: dist }),
                Exit::Edge(next) => {
                    let ne = self.graph.edge(next);
                    if ne.speed_limit < limit - 1e-9 {
                        return Some(Ahead::Restriction { limit: ne.speed_limit, distance: dist, track: ne.track });
                    }
                    limit = ne.speed_limit;
                    along_s = ne.a == node;
                    node = if along_s { ne.b } else { ne.a };
                    dist += ne.length;
                    edge = next;
                }
            }
        }
        None
    }

    /// Speed limit of the edge under the train's leading face.
    pub fn current_limit(&self, train: &Train) -> Option<f64> {
        let loc = train.locate(&self.graph, train.cars[0].x)?;
        Some(self.graph.edge(loc.edge).speed_limit)
    }
}

#[cfg(test)]
mod lookahead_tests {
    use super::*;
    use glam::DVec2;

    #[test]
    fn sees_a_slower_curve_then_a_bumper() {
        let mut g = TrackGraph::default();
        let a = g.add_node(DVec2::new(0.0, 0.0), NodeKind::End, "a");
        let b = g.add_node(DVec2::new(500.0, 0.0), NodeKind::Plain, "b");
        let c = g.add_node(DVec2::new(600.0, 0.0), NodeKind::End, "c");
        g.add_edge(a, b, Geometry::Straight { a: DVec2::new(0.0, 0.0), b: DVec2::new(500.0, 0.0) }, 0.0, 27.0, Some(0));
        g.add_edge(b, c, Geometry::Straight { a: DVec2::new(500.0, 0.0), b: DVec2::new(600.0, 0.0) }, 0.0, 6.7, Some(1));
        let mut w = World::new(g);
        let car = w.new_car(TYPE_SWITCHER);
        let id = w.spawn_train(vec![car], 0, 300.0, true).unwrap();
        let tr = w.train(id).unwrap();
        match w.lookahead(tr, true, 2000.0) {
            Some(Ahead::Restriction { limit, distance, .. }) => {
                assert!((limit - 6.7).abs() < 1e-9);
                assert!((distance - 200.0).abs() < 1e-6, "distance {distance}");
            }
            other => panic!("expected a restriction, got {other:?}"),
        }
        match w.lookahead(tr, false, 2000.0) {
            Some(Ahead::End { distance }) => assert!((distance - (300.0 - 13.7)).abs() < 1e-6, "distance {distance}"),
            other => panic!("expected the bumper, got {other:?}"),
        }
    }
}
