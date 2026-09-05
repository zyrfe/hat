//! Track graph: nodes, edges with straight or circular-arc geometry, turnouts.

use std::f64::consts::{FRAC_PI_2, PI};

use glam::DVec2;

pub type NodeId = u32;
pub type EdgeId = u32;

/// A position and a direction of travel in the horizontal plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: DVec2,
    /// Radians, counter-clockwise from +x.
    pub heading: f64,
}

impl Pose {
    pub fn new(x: f64, y: f64, heading: f64) -> Self {
        Pose { pos: DVec2::new(x, y), heading }
    }
    pub fn dir(&self) -> DVec2 {
        DVec2::from_angle(self.heading)
    }
    pub fn left(&self) -> DVec2 {
        DVec2::new(-self.heading.sin(), self.heading.cos())
    }
    pub fn reversed(mut self) -> Self {
        self.heading += PI;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Straight { a: DVec2, b: DVec2 },
    /// Circular arc. `sweep` is signed: positive turns left (counter-clockwise).
    Arc { center: DVec2, radius: f64, start_angle: f64, sweep: f64 },
}

impl Geometry {
    pub fn straight_from(start: Pose, len: f64) -> Self {
        Geometry::Straight { a: start.pos, b: start.pos + start.dir() * len }
    }

    /// Arc starting at `start`, turning by `angle` radians (positive left) with `radius`.
    pub fn arc_from(start: Pose, radius: f64, angle: f64) -> Self {
        let center = start.pos + start.left() * radius * angle.signum();
        let start_angle = (start.pos - center).to_angle();
        Geometry::Arc { center, radius, start_angle, sweep: angle }
    }

    pub fn length(&self) -> f64 {
        match self {
            Geometry::Straight { a, b } => a.distance(*b),
            Geometry::Arc { radius, sweep, .. } => radius * sweep.abs(),
        }
    }

    pub fn curvature(&self) -> f64 {
        match self {
            Geometry::Straight { .. } => 0.0,
            Geometry::Arc { radius, .. } => 1.0 / radius,
        }
    }

    /// Pose at arc length `s` from the start, heading in the direction of increasing `s`.
    pub fn pose(&self, s: f64) -> Pose {
        match self {
            Geometry::Straight { a, b } => {
                let d = *b - *a;
                let len = d.length();
                let dir = if len > 0.0 { d / len } else { DVec2::X };
                Pose { pos: *a + dir * s, heading: dir.to_angle() }
            }
            Geometry::Arc { center, radius, start_angle, sweep } => {
                let sgn = sweep.signum();
                let ang = start_angle + sgn * (s / radius);
                Pose { pos: *center + DVec2::from_angle(ang) * *radius, heading: ang + sgn * FRAC_PI_2 }
            }
        }
    }

    pub fn start(&self) -> Pose {
        self.pose(0.0)
    }

    pub fn end(&self) -> Pose {
        self.pose(self.length())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Normal,
    Diverging,
}

impl Route {
    pub fn other(self) -> Route {
        match self {
            Route::Normal => Route::Diverging,
            Route::Diverging => Route::Normal,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// Bumper. Nothing beyond.
    End,
    /// Two edges meet; trains pass through.
    Plain,
    /// Points. Trains from the toe follow the setting; trains from a leg must match it.
    Turnout { toe: EdgeId, normal: EdgeId, diverging: EdgeId, setting: Route },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub pos: DVec2,
    pub kind: NodeKind,
    pub edges: Vec<EdgeId>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub a: NodeId,
    pub b: NodeId,
    pub geom: Geometry,
    pub length: f64,
    /// Rise over run in the direction of increasing `s`.
    pub grade: f64,
    /// m/s
    pub speed_limit: f64,
    /// Logical track this edge belongs to, for scoring and display.
    pub track: Option<u32>,
}

/// What lies beyond a node when leaving an edge through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Edge(EdgeId),
    End,
    /// Trailing move through points set for the other leg.
    SplitSwitch,
}

#[derive(Clone, Debug, Default)]
pub struct TrackGraph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl TrackGraph {
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn edge(&self, id: EdgeId) -> &Edge {
        &self.edges[id as usize]
    }

    pub fn add_node(&mut self, pos: DVec2, kind: NodeKind, name: impl Into<String>) -> NodeId {
        self.nodes.push(Node { pos, kind, edges: Vec::new(), name: name.into() });
        (self.nodes.len() - 1) as NodeId
    }

    pub fn add_edge(&mut self, a: NodeId, b: NodeId, geom: Geometry, grade: f64, speed_limit: f64, track: Option<u32>) -> EdgeId {
        let length = geom.length();
        self.edges.push(Edge { a, b, geom, length, grade, speed_limit, track });
        let id = (self.edges.len() - 1) as EdgeId;
        self.nodes[a as usize].edges.push(id);
        self.nodes[b as usize].edges.push(id);
        id
    }

    /// Turn a node into a turnout once its three edges exist.
    pub fn set_turnout(&mut self, node: NodeId, toe: EdgeId, normal: EdgeId, diverging: EdgeId) {
        self.nodes[node as usize].kind = NodeKind::Turnout { toe, normal, diverging, setting: Route::Normal };
    }

    pub fn other_node(&self, edge: EdgeId, node: NodeId) -> NodeId {
        let e = self.edge(edge);
        if e.a == node { e.b } else { e.a }
    }

    pub fn exit(&self, from: EdgeId, via: NodeId) -> Exit {
        let node = self.node(via);
        match &node.kind {
            NodeKind::End => Exit::End,
            NodeKind::Plain => match node.edges.iter().find(|&&e| e != from) {
                Some(&e) => Exit::Edge(e),
                None => Exit::End,
            },
            NodeKind::Turnout { toe, normal, diverging, setting } => {
                if from == *toe {
                    Exit::Edge(if *setting == Route::Normal { *normal } else { *diverging })
                } else if from == *normal {
                    if *setting == Route::Normal { Exit::Edge(*toe) } else { Exit::SplitSwitch }
                } else if from == *diverging {
                    if *setting == Route::Diverging { Exit::Edge(*toe) } else { Exit::SplitSwitch }
                } else {
                    Exit::End
                }
            }
        }
    }

    pub fn turnout_setting(&self, node: NodeId) -> Option<Route> {
        match &self.node(node).kind {
            NodeKind::Turnout { setting, .. } => Some(*setting),
            _ => None,
        }
    }

    pub fn set_route(&mut self, node: NodeId, route: Route) -> bool {
        match &mut self.nodes[node as usize].kind {
            NodeKind::Turnout { setting, .. } => {
                *setting = route;
                true
            }
            _ => false,
        }
    }

    pub fn pose_on_edge(&self, edge: EdgeId, s: f64) -> Pose {
        self.edge(edge).geom.pose(s)
    }

    /// Distance from `s` on `edge` to `node`, if the node is an end of that edge.
    pub fn distance_to_node(&self, edge: EdgeId, s: f64, node: NodeId) -> Option<f64> {
        let e = self.edge(edge);
        if e.a == node {
            Some(s)
        } else if e.b == node {
            Some(e.length - s)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_pose_turns_left_and_right() {
        let start = Pose::new(0.0, 0.0, 0.0);
        let left = Geometry::arc_from(start, 100.0, FRAC_PI_2);
        let end = left.end();
        assert!((end.pos - DVec2::new(100.0, 100.0)).length() < 1e-9, "{end:?}");
        assert!((end.heading - FRAC_PI_2).abs() < 1e-9);
        let right = Geometry::arc_from(start, 100.0, -FRAC_PI_2);
        let end = right.end();
        assert!((end.pos - DVec2::new(100.0, -100.0)).length() < 1e-9, "{end:?}");
        assert!((end.heading + FRAC_PI_2).abs() < 1e-9);
        assert!((left.length() - 100.0 * FRAC_PI_2).abs() < 1e-9);
    }
}
