//! A train is a contiguous array of cars on a path over the track graph.
//!
//! Cars live at a scalar path coordinate `x` that increases toward the head (index 0).
//! The path is the list of edges the train occupies, tail-most first. See ADR-0003.

use std::collections::VecDeque;
use std::f64::consts::PI;

use crate::car::*;
use crate::params::*;
use crate::track::*;
use crate::TrainId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Reverser {
    Reverse,
    #[default]
    Neutral,
    Forward,
}

impl Reverser {
    pub fn sign(self) -> f64 {
        match self {
            Reverser::Reverse => -1.0,
            Reverser::Neutral => 0.0,
            Reverser::Forward => 1.0,
        }
    }
    pub fn flipped(self) -> Self {
        match self {
            Reverser::Reverse => Reverser::Forward,
            Reverser::Neutral => Reverser::Neutral,
            Reverser::Forward => Reverser::Reverse,
        }
    }
}

/// Engineer's controls. Applied to every locomotive in the train.
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    /// 0..=8
    pub throttle: u8,
    pub reverser: Reverser,
    /// Independent (locomotive) brake, 0..=1.
    pub independent: f64,
    /// Automatic brake: target brake pipe pressure at the locomotive, Pa.
    pub auto_target: f64,
    pub emergency: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, auto_target: PIPE_REF, emergency: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathSeg {
    pub edge: EdgeId,
    /// True when +x corresponds to increasing `s` on the edge.
    pub forward: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Buff,
    Free,
    Draft,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PipeCmd {
    pub t: f64,
    pub target: f64,
    pub emergency: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Location {
    pub seg: usize,
    pub edge: EdgeId,
    pub s: f64,
    pub forward: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Head,
    Tail,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EndResult {
    Ok,
    Bumper { overshoot: f64 },
    SplitSwitch { overshoot: f64 },
}

#[derive(Clone, Debug)]
pub struct Train {
    pub id: TrainId,
    /// Index 0 is the head.
    pub cars: Vec<CarState>,
    /// Occupied edges, tail-most first.
    pub path: Vec<PathSeg>,
    /// Path coordinate of the start of `path[0]`.
    pub path_origin: f64,
    pub controls: Controls,
    pub pipe_cmds: VecDeque<PipeCmd>,
    /// Coupler zones, `zones[k]` is the coupler between car k and k+1.
    pub zones: Vec<Zone>,
    /// Air hose connected at coupler k (between car k and k+1).
    pub hoses: Vec<bool>,
    pub derailed: bool,
}

impl Train {
    pub fn new(id: TrainId, cars: Vec<CarState>, path: Vec<PathSeg>, path_origin: f64) -> Self {
        let n = cars.len();
        let mut pipe_cmds = VecDeque::new();
        pipe_cmds.push_back(PipeCmd { t: f64::NEG_INFINITY, target: PIPE_REF, emergency: false });
        Train {
            id,
            cars,
            path,
            path_origin,
            controls: Controls::default(),
            pipe_cmds,
            zones: vec![Zone::Free; n.saturating_sub(1)],
            hoses: vec![false; n.saturating_sub(1)],
            derailed: false,
        }
    }

    pub fn control_car(&self, types: &[CarType]) -> Option<usize> {
        self.cars.iter().position(|c| types[c.type_id as usize].is_loco())
    }

    pub fn car_len(&self, types: &[CarType], i: usize) -> f64 {
        types[self.cars[i].type_id as usize].length
    }

    pub fn head_face_x(&self, types: &[CarType]) -> f64 {
        self.cars[0].x + 0.5 * self.car_len(types, 0)
    }

    pub fn tail_face_x(&self, types: &[CarType]) -> f64 {
        let n = self.cars.len() - 1;
        self.cars[n].x - 0.5 * self.car_len(types, n)
    }

    pub fn length(&self, types: &[CarType]) -> f64 {
        self.cars.iter().map(|c| types[c.type_id as usize].length).sum()
    }

    pub fn total_mass(&self, types: &[CarType]) -> f64 {
        self.cars.iter().map(|c| c.mass(&types[c.type_id as usize])).sum()
    }

    /// Velocity of the controlling locomotive, or the head car.
    pub fn speed(&self, types: &[CarType]) -> f64 {
        match self.control_car(types) {
            Some(i) => self.cars[i].v,
            None => self.cars[0].v,
        }
    }

    /// Path coordinate of coupler k, between car k-1 and car k.
    pub fn coupler_x(&self, types: &[CarType], k: usize) -> f64 {
        let front_face = self.cars[k - 1].x - 0.5 * self.car_len(types, k - 1);
        let rear_face = self.cars[k].x + 0.5 * self.car_len(types, k);
        0.5 * (front_face + rear_face)
    }

    /// Extension of coupler k: positive stretched, negative bunched.
    pub fn coupler_extension(&self, types: &[CarType], k: usize) -> f64 {
        (self.cars[k - 1].x - self.cars[k].x) - 0.5 * (self.car_len(types, k - 1) + self.car_len(types, k))
    }

    pub fn seg_start_x(&self, graph: &TrackGraph, k: usize) -> f64 {
        let mut x = self.path_origin;
        for seg in &self.path[..k] {
            x += graph.edge(seg.edge).length;
        }
        x
    }

    pub fn path_end_x(&self, graph: &TrackGraph) -> f64 {
        self.seg_start_x(graph, self.path.len())
    }

    pub fn locate(&self, graph: &TrackGraph, x: f64) -> Option<Location> {
        if x < self.path_origin - 1e-6 {
            return None;
        }
        let mut start = self.path_origin;
        let n = self.path.len();
        for (k, seg) in self.path.iter().enumerate() {
            let len = graph.edge(seg.edge).length;
            let last = k + 1 == n;
            if x < start + len || (last && x <= start + len + 1e-6) {
                let local = (x - start).clamp(0.0, len);
                let s = if seg.forward { local } else { len - local };
                return Some(Location { seg: k, edge: seg.edge, s, forward: seg.forward });
            }
            start += len;
        }
        None
    }

    /// World pose at path coordinate `x`, heading toward +x.
    pub fn pose_at(&self, graph: &TrackGraph, x: f64) -> Option<Pose> {
        let loc = self.locate(graph, x)?;
        let mut p = graph.pose_on_edge(loc.edge, loc.s);
        if !loc.forward {
            p.heading += PI;
        }
        Some(p)
    }

    /// Reverse the train's frame so the tail becomes the head. Physical state unchanged.
    pub fn flip(&mut self, graph: &TrackGraph) {
        let total: f64 = self.path.iter().map(|s| graph.edge(s.edge).length).sum();
        self.path.reverse();
        for s in &mut self.path {
            s.forward = !s.forward;
        }
        self.path_origin = -(self.path_origin + total);
        self.cars.reverse();
        for c in &mut self.cars {
            c.x = -c.x;
            c.v = -c.v;
            c.knuckle_open.swap(0, 1);
            c.facing_head = !c.facing_head;
            c.no_couple_until.swap(0, 1);
            c.no_couple_with.swap(0, 1);
        }
        self.zones.reverse();
        self.hoses.reverse();
        self.controls.reverser = self.controls.reverser.flipped();
    }

    /// Grow the path so it covers the head face. Reports what stops it.
    pub fn extend_head(&mut self, graph: &TrackGraph, types: &[CarType]) -> EndResult {
        loop {
            let head = self.head_face_x(types);
            let end = self.path_end_x(graph);
            if head <= end + 1e-9 {
                return EndResult::Ok;
            }
            let last = *self.path.last().unwrap();
            let e = graph.edge(last.edge);
            let node = if last.forward { e.b } else { e.a };
            match graph.exit(last.edge, node) {
                Exit::Edge(next) => {
                    let ne = graph.edge(next);
                    self.path.push(PathSeg { edge: next, forward: ne.a == node });
                }
                Exit::End => return EndResult::Bumper { overshoot: head - end },
                Exit::SplitSwitch => return EndResult::SplitSwitch { overshoot: head - end },
            }
        }
    }

    /// Grow the path so it covers the tail face.
    pub fn extend_tail(&mut self, graph: &TrackGraph, types: &[CarType]) -> EndResult {
        loop {
            let tail = self.tail_face_x(types);
            if tail >= self.path_origin - 1e-9 {
                return EndResult::Ok;
            }
            let first = self.path[0];
            let e = graph.edge(first.edge);
            let node = if first.forward { e.a } else { e.b };
            match graph.exit(first.edge, node) {
                Exit::Edge(next) => {
                    let ne = graph.edge(next);
                    self.path.insert(0, PathSeg { edge: next, forward: ne.b == node });
                    self.path_origin -= ne.length;
                }
                Exit::End => return EndResult::Bumper { overshoot: self.path_origin - tail },
                Exit::SplitSwitch => return EndResult::SplitSwitch { overshoot: self.path_origin - tail },
            }
        }
    }

    /// Drop path segments no car occupies.
    pub fn retract(&mut self, graph: &TrackGraph, types: &[CarType]) {
        let head = self.head_face_x(types);
        let tail = self.tail_face_x(types);
        while self.path.len() > 1 {
            let len0 = graph.edge(self.path[0].edge).length;
            if tail >= self.path_origin + len0 - 1e-9 {
                self.path.remove(0);
                self.path_origin += len0;
            } else {
                break;
            }
        }
        while self.path.len() > 1 {
            let n = self.path.len();
            let start_last = self.seg_start_x(graph, n - 1);
            if head <= start_last + 1e-9 {
                self.path.pop();
            } else {
                break;
            }
        }
    }

    /// Which cars share a continuous brake pipe with the control car.
    pub fn pipe_connectivity(&self, ctrl: Option<usize>) -> Vec<bool> {
        let n = self.cars.len();
        let mut out = vec![false; n];
        let Some(c) = ctrl else { return out };
        out[c] = true;
        for i in (0..c).rev() {
            if self.hoses[i] {
                out[i] = true;
            } else {
                break;
            }
        }
        for i in c + 1..n {
            if self.hoses[i - 1] {
                out[i] = true;
            } else {
                break;
            }
        }
        out
    }

    pub fn push_pipe_cmd(&mut self, t: f64, target: f64, emergency: bool) {
        if let Some(last) = self.pipe_cmds.back() {
            if last.target == target && last.emergency == emergency {
                return;
            }
        }
        self.pipe_cmds.push_back(PipeCmd { t, target, emergency });
        while self.pipe_cmds.len() > 2 && self.pipe_cmds[1].t < t - 60.0 {
            self.pipe_cmds.pop_front();
        }
    }

    /// Pipe target seen `dist` meters from the control car at time `t`.
    pub fn pipe_target_at(&self, t: f64, dist: f64) -> (f64, bool) {
        let latest_em = self.pipe_cmds.back().map(|c| c.emergency).unwrap_or(false);
        let c = if latest_em { C_EMERGENCY } else { C_SERVICE };
        let query = t - dist / c;
        let mut result = (PIPE_REF, false);
        for cmd in &self.pipe_cmds {
            if cmd.t <= query {
                result = (cmd.target, cmd.emergency);
            } else {
                break;
            }
        }
        result
    }
}
