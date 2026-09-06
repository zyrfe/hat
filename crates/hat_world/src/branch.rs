//! The Branch: an open-country loop with industries on it. Trains circulate one way, so
//! there are no meets to dispatch yet. A double-ended yard sits on the south main; every
//! industry is a through siding with a chute or a pit over the track.
//!
//! South main runs east at y = 0, north main runs west at y = 2R. Coordinates in meters.

use std::f64::consts::PI;

use glam::DVec2;
use hat_sim::*;

use crate::industry::*;
use crate::terminal::s_curve;
use crate::yard::*;

pub const TRACK_ELEVATOR_BASE: u32 = 40;
pub const TRACK_MINE: u32 = 50;
pub const TRACK_PLANT: u32 = 51;
pub const TRACK_GRAIN_TERMINAL: u32 = 52;
pub const TRACK_INTERCHANGE: u32 = 60;

pub const IND_MINE: u32 = 1;
pub const IND_PLANT: u32 = 2;
pub const IND_ELEVATOR_BASE: u32 = 3;
pub const IND_GRAIN_TERMINAL: u32 = 6;

#[derive(Clone, Debug)]
pub struct BranchParams {
    pub half_len: f64,
    pub loop_radius: f64,
    pub main_speed: f64,
    pub arc_speed: f64,
    pub yard_speed: f64,
    pub turnout_angle: f64,
    pub spacing: f64,
    pub ladder_radius: f64,
    pub siding_radius: f64,
    pub yard_tracks: usize,
    pub elevators: usize,
    /// kg/s
    pub mine_rate: f64,
    pub plant_rate: f64,
    pub elevator_rate: f64,
    pub terminal_rate: f64,
    pub flood_loader_rate: f64,
    pub pit_rate: f64,
    pub spout_rate: f64,
    pub grain_pit_rate: f64,
}

impl Default for BranchParams {
    fn default() -> Self {
        BranchParams {
            half_len: 4200.0,
            loop_radius: 350.0,
            main_speed: 20.0,
            arc_speed: 12.0,
            yard_speed: 6.7,
            turnout_angle: (1.0f64 / 8.0).atan(),
            spacing: 4.5,
            ladder_radius: 150.0,
            siding_radius: 292.0,
            yard_tracks: 4,
            elevators: 3,
            mine_rate: 40.0,
            plant_rate: 40.0,
            elevator_rate: 3.0,
            terminal_rate: 10.0,
            flood_loader_rate: 2500.0,
            pit_rate: 3000.0,
            spout_rate: 300.0,
            grain_pit_rate: 1500.0,
        }
    }
}

/// The world plus its industries.
pub struct BranchWorld {
    pub graph: TrackGraph,
    pub yard: Yard,
    pub industries: Vec<Industry>,
}

/// A ladder with open body-track starts, so two of them can be joined into a double-ended
/// yard. `side` is +1 for a ladder turning left of `from_pose`, -1 for right.
struct OpenLadder {
    switches: Vec<NodeId>,
    lead_edges: Vec<EdgeId>,
    lead_arc: EdgeId,
    /// Per track: (arc edge, body start node, entry switch and route).
    bodies: Vec<(EdgeId, NodeId, (NodeId, Route))>,
}

fn open_ladder(g: &mut TrackGraph, from: NodeId, from_pose: Pose, p: &BranchParams, side: f64, n_tracks: usize, tag: &str) -> OpenLadder {
    let th = p.turnout_angle;
    let r = p.ladder_radius;
    let lead_arc = Geometry::arc_from(from_pose, r, side * th);
    let l0_pose = lead_arc.end();
    let l0 = g.add_node(l0_pose.pos, NodeKind::Plain, format!("L{tag}"));
    let lead_arc_e = g.add_edge(from, l0, lead_arc, 0.0, p.yard_speed, Some(TRACK_LEAD));
    let mut lead_edges = vec![lead_arc_e];
    let ladder_step = p.spacing / th.sin();
    let first = Geometry::straight_from(l0_pose, 30.0);
    let mut cur_pose = first.end();
    let mut cur_node = g.add_node(cur_pose.pos, NodeKind::Plain, format!("{tag}1"));
    let mut incoming = g.add_edge(l0, cur_node, first, 0.0, p.yard_speed, Some(TRACK_LEAD));
    lead_edges.push(incoming);
    let mut switches = Vec::new();
    let mut bodies = Vec::new();
    let body = |g: &mut TrackGraph, from: NodeId, from_pose: Pose, id: u32| -> (EdgeId, NodeId) {
        let arc = Geometry::arc_from(from_pose, r, -side * th);
        let b_pose = arc.end();
        let b = g.add_node(b_pose.pos, NodeKind::Plain, format!("B{tag}{id}"));
        let arc_e = g.add_edge(from, b, arc, 0.0, p.yard_speed, Some(id));
        (arc_e, b)
    };
    let n_turnouts = n_tracks.saturating_sub(1);
    for k in 1..=n_turnouts {
        let id = k as u32;
        switches.push(cur_node);
        let (arc_e, b) = body(g, cur_node, cur_pose, id);
        bodies.push((arc_e, b, (cur_node, Route::Diverging)));
        let next_geom = Geometry::straight_from(cur_pose, ladder_step);
        let next_pose = next_geom.end();
        let last = k == n_turnouts;
        let next_node = g.add_node(next_pose.pos, NodeKind::Plain, if last { format!("{tag} end") } else { format!("{tag}{}", k + 1) });
        let outgoing = g.add_edge(cur_node, next_node, next_geom, 0.0, p.yard_speed, Some(TRACK_LEAD));
        lead_edges.push(outgoing);
        g.set_turnout(cur_node, incoming, outgoing, arc_e);
        if last {
            let (arc_e, b) = body(g, next_node, next_pose, n_tracks as u32);
            bodies.push((arc_e, b, (cur_node, Route::Normal)));
        }
        cur_pose = next_pose;
        cur_node = next_node;
        incoming = outgoing;
    }
    OpenLadder { switches, lead_edges, lead_arc: lead_arc_e, bodies }
}

/// A through siding off the main between `wr` and `er`, which must be 2 S-curve spans plus
/// `straight` apart along `wr_pose`. Returns (straight edge, first edge in, last edge out).
fn through_siding(g: &mut TrackGraph, wr: NodeId, wr_pose: Pose, er: NodeId, straight: f64, p: &BranchParams, sign: f64, track: u32, name: &str) -> (EdgeId, EdgeId, EdgeId) {
    let th = p.turnout_angle;
    let (s0, s0_pose, s_in) = s_curve(g, wr, wr_pose, p.siding_radius, th, sign, p.yard_speed, Some(track), &format!("{name} in"));
    let end_pose = Pose { pos: s0_pose.pos + s0_pose.dir() * straight, ..s0_pose };
    let s1 = g.add_node(end_pose.pos, NodeKind::Plain, format!("{name} s0"));
    let straight_e = g.add_edge(s0, s1, Geometry::Straight { a: s0_pose.pos, b: end_pose.pos }, 0.0, p.yard_speed, Some(track));
    let (join, join_pose, s_out) = s_curve(g, s1, end_pose, p.siding_radius, th, -sign, p.yard_speed, Some(track), &format!("{name} out"));
    debug_assert!((join_pose.pos - g.node(er).pos).length() < 0.05, "{name}: siding rejoins at {:?}, main node at {:?}", join_pose.pos, g.node(er).pos);
    let last = *s_out.last().unwrap();
    g.edges[last as usize].b = er;
    g.nodes[er as usize].edges.push(last);
    g.nodes[join as usize].edges.clear();
    (straight_e, s_in[0], last)
}

/// Longitudinal span of one S-curve.
fn s_span(p: &BranchParams) -> f64 {
    2.0 * p.siding_radius * p.turnout_angle.sin()
}

pub fn build_branch(p: &BranchParams) -> BranchWorld {
    let mut g = TrackGraph::default();
    let h = p.half_len;
    let r = p.loop_radius;
    let span = s_span(p);
    let ytop = 2.0 * r;

    // South main, west to east: arc end, elevator sidings, yard (T0 .. Te), mine siding, arc start.
    #[derive(Clone)]
    enum Stop {
        Plain(&'static str),
        Siding { straight: f64, track: u32, name: String },
        YardWest,
        YardEast,
    }
    let mut south: Vec<(f64, Stop)> = vec![(-h, Stop::Plain("W arc end"))];
    let mut x = -4000.0;
    for k in 0..p.elevators {
        let id = TRACK_ELEVATOR_BASE + 1 + k as u32;
        south.push((x, Stop::Siding { straight: 500.0, track: id, name: format!("Elevator {}", k + 1) }));
        x += 800.0;
    }
    south.push((-1200.0, Stop::YardWest));
    south.push((-200.0, Stop::YardEast));
    south.push((600.0, Stop::Siding { straight: 1200.0, track: TRACK_MINE, name: "Mine".into() }));
    south.push((h, Stop::Plain("E arc start")));

    // Lay the south main nodes and edges.
    struct Laid {
        node: NodeId,
        stop: Stop,
        /// Second node for sidings (the rejoin).
        node2: Option<NodeId>,
    }
    let mut laid: Vec<Laid> = Vec::new();
    let mut prev: Option<NodeId> = None;
    let mut main_edges: Vec<EdgeId> = Vec::new();
    let link = |g: &mut TrackGraph, prev: &mut Option<NodeId>, n: NodeId, main_edges: &mut Vec<EdgeId>, speed: f64| {
        if let Some(pn) = *prev {
            let geom = Geometry::Straight { a: g.node(pn).pos, b: g.node(n).pos };
            main_edges.push(g.add_edge(pn, n, geom, 0.0, speed, Some(TRACK_MAIN)));
        }
        *prev = Some(n);
    };
    for (x, stop) in south.iter().cloned() {
        match &stop {
            Stop::Siding { straight, name, .. } => {
                let wr = g.add_node(DVec2::new(x, 0.0), NodeKind::Plain, format!("{name} W"));
                link(&mut g, &mut prev, wr, &mut main_edges, p.main_speed);
                let er = g.add_node(DVec2::new(x + 2.0 * span + straight, 0.0), NodeKind::Plain, format!("{name} E"));
                link(&mut g, &mut prev, er, &mut main_edges, p.main_speed);
                laid.push(Laid { node: wr, stop: stop.clone(), node2: Some(er) });
            }
            Stop::Plain(name) => {
                let n = g.add_node(DVec2::new(x, 0.0), NodeKind::Plain, *name);
                link(&mut g, &mut prev, n, &mut main_edges, p.main_speed);
                laid.push(Laid { node: n, stop: stop.clone(), node2: None });
            }
            Stop::YardWest => {
                let n = g.add_node(DVec2::new(x, 0.0), NodeKind::Plain, "T0");
                link(&mut g, &mut prev, n, &mut main_edges, p.main_speed);
                laid.push(Laid { node: n, stop: stop.clone(), node2: None });
            }
            Stop::YardEast => {
                let n = g.add_node(DVec2::new(x, 0.0), NodeKind::Plain, "Te");
                link(&mut g, &mut prev, n, &mut main_edges, p.main_speed);
                laid.push(Laid { node: n, stop: stop.clone(), node2: None });
            }
        }
    }
    let e_arc_start = laid.last().unwrap().node;
    let w_arc_end = laid[0].node;

    // Main edges around each node, by node id.
    let edges_at = |g: &TrackGraph, n: NodeId| -> Vec<EdgeId> { g.node(n).edges.iter().copied().filter(|&e| g.edge(e).track == Some(TRACK_MAIN)).collect() };
    let main_before = |g: &TrackGraph, n: NodeId| -> EdgeId { *edges_at(g, n).iter().find(|&&e| g.edge(e).b == n).expect("main edge into node") };
    let main_after = |g: &TrackGraph, n: NodeId| -> EdgeId { *edges_at(g, n).iter().find(|&&e| g.edge(e).a == n).expect("main edge out of node") };

    let mut tracks: Vec<YardTrack> = Vec::new();
    let mut industries: Vec<Industry> = Vec::new();
    let mut ladders: Vec<Ladder> = Vec::new();
    let mut t0: Option<NodeId> = None;
    let mut te: Option<NodeId> = None;
    let mut west_lead: Vec<EdgeId> = Vec::new();
    let mut west_switches: Vec<NodeId> = Vec::new();

    // South-side sidings and their industries.
    for l in &laid {
        if let (Stop::Siding { straight, track, name }, Some(er)) = (&l.stop, l.node2) {
            let wr = l.node;
            let wr_pose = Pose { pos: g.node(wr).pos, heading: 0.0, z: 0.0 };
            let (straight_e, first_in, last_out) = through_siding(&mut g, wr, wr_pose, er, *straight, p, -1.0, *track, name);
            g.set_turnout(wr, main_before(&g, wr), main_after(&g, wr), first_in);
            g.set_turnout(er, main_after(&g, er), main_before(&g, er), last_out);
            let (fac, spot_s, ind) = if *track == TRACK_MINE {
                let fac = Facility::load(Commodity::Coal, p.flood_loader_rate, FacilityMode::Spot { s: 700.0, max_speed: 0.6 }).for_industry(IND_MINE);
                (fac, 700.0, Industry::producer(IND_MINE, "Black Creek Mine", IndustryKind::CoalMine, DVec2::ZERO, Commodity::Coal, p.mine_rate, 12.0e6, 6.0e6))
            } else {
                let k = track - TRACK_ELEVATOR_BASE;
                let id = IND_ELEVATOR_BASE + k - 1;
                let fac = Facility::load(Commodity::Grain, p.spout_rate, FacilityMode::Spot { s: 250.0, max_speed: 0.05 }).for_industry(id);
                (fac, 250.0, Industry::producer(id, &format!("{name} Co-op"), IndustryKind::Elevator, DVec2::ZERO, Commodity::Grain, p.elevator_rate, 2.5e6, 1.5e6))
            };
            g.edges[straight_e as usize].facility = Some(fac);
            let mut ind = ind;
            let spot = g.pose_on_edge(straight_e, spot_s);
            ind.pos = spot.pos - spot.left() * 22.0;
            ind.facilities = vec![straight_e];
            industries.push(ind);
            let in_edges: Vec<EdgeId> = g.node(wr).edges.iter().copied().filter(|&e| g.edge(e).track == Some(*track)).collect();
            let mut edges = in_edges.clone();
            edges.push(straight_e);
            let out_edges: Vec<EdgeId> = g.node(er).edges.iter().copied().filter(|&e| g.edge(e).track == Some(*track)).collect();
            edges.extend(out_edges);
            tracks.push(YardTrack { id: *track, name: name.clone(), edges, length: *straight, entry: (wr, Route::Diverging) });
        }
    }

    // The yard: west ladder off T0 turning left (north), east ladder off Te turning right
    // (north, since it faces west), body straights joining them.
    for l in &laid {
        match l.stop {
            Stop::YardWest => t0 = Some(l.node),
            Stop::YardEast => te = Some(l.node),
            _ => {}
        }
    }
    let (t0, te) = (t0.unwrap(), te.unwrap());
    let (x0, xe) = (g.node(t0).pos.x, g.node(te).pos.x);
    let west = open_ladder(&mut g, t0, Pose::new(x0, 0.0, 0.0), p, 1.0, p.yard_tracks, "S");
    let east = open_ladder(&mut g, te, Pose::new(xe, 0.0, PI), p, -1.0, p.yard_tracks, "E");
    g.set_turnout(t0, main_before(&g, t0), main_after(&g, t0), west.lead_arc);
    g.set_turnout(te, main_after(&g, te), main_before(&g, te), east.lead_arc);
    for k in 0..p.yard_tracks {
        let id = k as u32 + 1;
        let (arc_w, bw, entry_w) = west.bodies[k];
        let (arc_e, be, _entry_e) = east.bodies[k];
        let (pw, pe) = (g.node(bw).pos, g.node(be).pos);
        debug_assert!((pw.y - pe.y).abs() < 1e-6, "track {id} ends do not line up: {pw:?} vs {pe:?}");
        let straight = g.add_edge(bw, be, Geometry::Straight { a: pw, b: DVec2::new(pe.x, pw.y) }, 0.0, p.yard_speed, Some(id));
        tracks.push(YardTrack { id, name: format!("Track {id}"), edges: vec![arc_w, straight, arc_e], length: (pe.x - pw.x).abs(), entry: entry_w });
    }
    west_lead.extend(west.lead_edges.iter());
    west_switches.extend(west.switches.iter());
    ladders.push(Ladder { entry: Some((t0, Route::Diverging)), switches: west.switches.clone(), tracks: (1..=p.yard_tracks as u32).collect() });
    ladders.push(Ladder { entry: Some((te, Route::Diverging)), switches: east.switches.clone(), tracks: (1..=p.yard_tracks as u32).collect() });

    // East arc up, north main west with its sidings, west arc down.
    let east_arc = Geometry::arc_from(Pose::new(h, 0.0, 0.0), r, PI);
    let n_e = g.add_node(east_arc.end().pos, NodeKind::Plain, "N east");
    g.add_edge(e_arc_start, n_e, east_arc, 0.0, p.arc_speed, Some(TRACK_MAIN));
    let north: Vec<(f64, f64, u32, &str)> = vec![(2800.0, 1200.0, TRACK_PLANT, "Power plant"), (-400.0, 600.0, TRACK_GRAIN_TERMINAL, "Grain terminal"), (-2000.0, 500.0, TRACK_INTERCHANGE, "Interchange")];
    let mut prev_n = n_e;
    let mut north_nodes: Vec<(NodeId, NodeId, f64, u32, &str)> = Vec::new();
    for (x, straight, track, name) in north {
        let wr = g.add_node(DVec2::new(x, ytop), NodeKind::Plain, format!("{name} E"));
        g.add_edge(prev_n, wr, Geometry::Straight { a: g.node(prev_n).pos, b: DVec2::new(x, ytop) }, 0.0, p.main_speed, Some(TRACK_MAIN));
        let er = g.add_node(DVec2::new(x - 2.0 * span - straight, ytop), NodeKind::Plain, format!("{name} W"));
        g.add_edge(wr, er, Geometry::Straight { a: DVec2::new(x, ytop), b: g.node(er).pos }, 0.0, p.main_speed, Some(TRACK_MAIN));
        north_nodes.push((wr, er, straight, track, name));
        prev_n = er;
    }
    let n_w = g.add_node(DVec2::new(-h, ytop), NodeKind::Plain, "N west");
    g.add_edge(prev_n, n_w, Geometry::Straight { a: g.node(prev_n).pos, b: DVec2::new(-h, ytop) }, 0.0, p.main_speed, Some(TRACK_MAIN));
    let west_arc = Geometry::arc_from(Pose::new(-h, ytop, PI), r, PI);
    debug_assert!((west_arc.end().pos - DVec2::new(-h, 0.0)).length() < 1e-6);
    g.add_edge(n_w, w_arc_end, west_arc, 0.0, p.arc_speed, Some(TRACK_MAIN));
    for (wr, er, straight, track, name) in north_nodes {
        let wr_pose = Pose { pos: g.node(wr).pos, heading: PI, z: 0.0 };
        let (straight_e, first_in, last_out) = through_siding(&mut g, wr, wr_pose, er, straight, p, 1.0, track, name);
        g.set_turnout(wr, main_before(&g, wr), main_after(&g, wr), first_in);
        g.set_turnout(er, main_after(&g, er), main_before(&g, er), last_out);
        let (fac, spot_s, ind) = match track {
            TRACK_PLANT => (
                Some(Facility::unload(Some(Commodity::Coal), p.pit_rate, FacilityMode::Spot { s: 700.0, max_speed: 0.6 }).for_industry(IND_PLANT)),
                700.0,
                Some(Industry::consumer(IND_PLANT, "Riverside Power", IndustryKind::PowerPlant, DVec2::ZERO, Commodity::Coal, p.plant_rate, 15.0e6, 4.0e6)),
            ),
            TRACK_GRAIN_TERMINAL => (
                Some(Facility::unload(Some(Commodity::Grain), p.grain_pit_rate, FacilityMode::Spot { s: 300.0, max_speed: 0.5 }).for_industry(IND_GRAIN_TERMINAL)),
                300.0,
                Some(Industry::consumer(IND_GRAIN_TERMINAL, "Harbour Elevator", IndustryKind::GrainTerminal, DVec2::ZERO, Commodity::Grain, p.terminal_rate, 20.0e6, 0.0)),
            ),
            _ => (None, 0.0, None),
        };
        if let (Some(fac), Some(mut ind)) = (fac, ind) {
            g.edges[straight_e as usize].facility = Some(fac);
            let spot = g.pose_on_edge(straight_e, spot_s);
            ind.pos = spot.pos - spot.left() * 22.0;
            ind.facilities = vec![straight_e];
            industries.push(ind);
        }
        let in_edges: Vec<EdgeId> = g.node(wr).edges.iter().copied().filter(|&e| g.edge(e).track == Some(track)).collect();
        let mut edges = in_edges;
        edges.push(straight_e);
        let out_edges: Vec<EdgeId> = g.node(er).edges.iter().copied().filter(|&e| g.edge(e).track == Some(track)).collect();
        edges.extend(out_edges);
        tracks.push(YardTrack { id: track, name: name.to_string(), edges, length: straight, entry: (wr, Route::Diverging) });
    }
    industries.sort_by_key(|i| i.id);
    tracks.sort_by_key(|t| t.id);

    let yard = Yard {
        main_west: main_before(&g, t0),
        main_east: main_after(&g, t0),
        lead_switch: t0,
        ladder_switches: west_switches,
        lead_edges: west_lead,
        tracks,
        ladders,
        min: DVec2::new(-h - r - 20.0, -60.0),
        max: DVec2::new(h + r + 20.0, ytop + 60.0),
        portal: None,
        receiving: None,
        hump: None,
        departure: Some(1),
    };
    BranchWorld { graph: g, yard, industries }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::route;

    #[test]
    fn geometry_closes_and_every_siding_rejoins() {
        let b = build_branch(&BranchParams::default());
        let g = &b.graph;
        for (ni, n) in g.nodes.iter().enumerate() {
            for &e in &n.edges {
                let edge = g.edge(e);
                let end = if edge.a == ni as NodeId { edge.geom.start() } else { edge.geom.end() };
                assert!((end.pos - n.pos).length() < 0.05, "edge {e} does not meet node {} ({}) : {:?} vs {:?}", ni, n.name, end.pos, n.pos);
            }
            // Every turnout has exactly three edges, every plain node one or two.
            match n.kind {
                NodeKind::Turnout { .. } => assert_eq!(n.edges.len(), 3, "turnout {} has {} edges", n.name, n.edges.len()),
                NodeKind::Plain => assert!(n.edges.len() <= 2, "plain node {} has {} edges", n.name, n.edges.len()),
                NodeKind::End => {}
            }
        }
        assert_eq!(b.yard.tracks.len(), 4 + 3 + 1 + 3);
        for t in &b.yard.tracks {
            assert!(t.length > 300.0, "{} is only {} m", t.name, t.length);
        }
        assert_eq!(b.industries.len(), 6);
        // Everything reachable from everything, one way round.
        let mine = b.yard.track(TRACK_MINE).unwrap().edges[2];
        let plant = b.yard.track(TRACK_PLANT).unwrap().edges[2];
        let t1 = b.yard.track(1).unwrap().edges[1];
        let to_mine = route(g, t1, true, 100.0, mine, &|_| 0.0).expect("yard to mine");
        let to_plant = route(g, mine, true, 800.0, plant, &|_| 0.0).expect("mine to plant");
        let back = route(g, plant, true, 800.0, mine, &|_| 0.0).expect("plant to mine");
        eprintln!("yard->mine {:.0} m, mine->plant {:.0} m, plant->mine {:.0} m", to_mine.length, to_plant.length, back.length);
        assert!(to_mine.length < 3000.0 && to_plant.length < 6000.0 && back.length > 8000.0);
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use crate::crew::*;
    use crate::economy::*;
    use crate::scenario::*;

    /// Both schedules run unaided: coal loads at the mine and pays out at the plant, grain is
    /// gathered from the elevators and paid for at the harbour.
    #[test]
    fn branch_trains_run_their_schedules_and_get_paid() {
        let sc = branch();
        let Built { mut world, mut economy, mut road_crews, .. } = build(&sc);
        let mut economy = economy.take().unwrap();
        let mut events: Vec<SimEvent> = Vec::new();
        let mut derails = 0;
        let mut last_report = -1.0e9;
        let start = std::time::Instant::now();
        while world.t < 4.0 * 3600.0 {
            economy.before_step(&mut world);
            for c in road_crews.iter_mut() {
                c.step(&mut world, DT);
            }
            world.step(DT);
            events = world.take_events();
            let on_duty = road_crews.iter().filter(|c| c.status == Status::Running).count();
            economy.after_step(&world, &events, DT, on_duty);
            for e in &events {
                if let SimEvent::Derail { cause, pos, train } = e {
                    derails += 1;
                    eprintln!("  DERAIL train {train} {cause:?} at ({:.0},{:.0}) t={:.0} min", pos.x, pos.y, world.t / 60.0);
                }
            }
            if world.t - last_report > 600.0 {
                last_report = world.t;
                let descr: Vec<String> = road_crews.iter().map(|c| format!("{}: {}", c.name, c.describe())).collect();
                eprintln!("[{:5.0} min] {} | balance {} | loaded {:.0} t unloaded {:.0} t", world.t / 60.0, descr.join(" | "), money(economy.ledger.balance), world.cargo_loaded / 1000.0, world.cargo_unloaded / 1000.0);
            }
            if road_crews.iter().any(|c| matches!(c.status, Status::Failed(_))) {
                break;
            }
            if economy.ledger.total(Account::Freight) > 0 && world.cargo_unloaded > 3_000_000.0 && economy.industries.iter().find(|i| i.id == IND_GRAIN_TERMINAL).map(|i| i.rail > 0.0).unwrap_or(false) {
                break;
            }
        }
        let _ = events;
        for c in &road_crews {
            for (t, l) in c.radio.iter() {
                eprintln!("  {} [{:5.0} min] {l}", c.name, t / 60.0);
            }
            if matches!(c.status, Status::Failed(_)) {
                eprintln!("{}", c.debug_state(&world));
            }
        }
        for e in economy.ledger.entries.iter().rev().take(12).collect::<Vec<_>>().into_iter().rev() {
            eprintln!("  ledger [{:5.0} min] {:>10} {:?} {}", e.t / 60.0, money(e.amount), e.account, e.note);
        }
        for i in &economy.industries {
            eprintln!("  {}: stock {:.0}/{:.0} t, rail {:.0} t, trucked {:.0} t, service {:.2}", i.name, i.stock / 1000.0, i.capacity / 1000.0, i.rail / 1000.0, i.trucked / 1000.0, i.service);
        }
        eprintln!("sim {:.0} min in {:.1} s wall, derails {derails}, balance {}", world.t / 60.0, start.elapsed().as_secs_f64(), money(economy.ledger.balance));
        for c in &road_crews {
            assert!(!matches!(c.status, Status::Failed(_)), "{} stuck: {:?}", c.name, c.status);
        }
        assert_eq!(derails, 0);
        assert!(economy.ledger.total(Account::Freight) > 0, "someone should have paid for freight");
        assert!(world.cargo_unloaded > 3_000_000.0, "a coal train should have dumped at the plant");
        let harbour = economy.industries.iter().find(|i| i.id == IND_GRAIN_TERMINAL).unwrap();
        assert!(harbour.rail > 0.0, "grain should have reached the harbour");
    }
}

#[cfg(test)]
mod perf_probe {
    use super::*;
    use crate::scenario::*;

    /// Where the time goes: world alone vs. world with crews and economy, 5 sim minutes each.
    #[test]
    #[ignore]
    fn step_cost_breakdown() {
        let sc = branch();
        let steps = (std::env::var("HAT_PROBE_SECS").ok().and_then(|v| v.parse::<f64>().ok()).unwrap_or(300.0) / DT) as usize;
        let Built { mut world, .. } = build(&sc);
        let t0 = std::time::Instant::now();
        for _ in 0..steps {
            world.step(DT);
        }
        let world_only = t0.elapsed().as_secs_f64() / steps as f64 * 1e6;
        let Built { mut world, mut economy, mut road_crews, .. } = build(&sc);
        let mut economy = economy.take().unwrap();
        let t0 = std::time::Instant::now();
        let mut crew_t = 0.0;
        let mut econ_t = 0.0;
        let mut world_t = 0.0;
        for _ in 0..steps {
            let a = std::time::Instant::now();
            economy.before_step(&mut world);
            econ_t += a.elapsed().as_secs_f64();
            let a = std::time::Instant::now();
            for c in road_crews.iter_mut() {
                c.step(&mut world, DT);
            }
            crew_t += a.elapsed().as_secs_f64();
            let a = std::time::Instant::now();
            world.step(DT);
            world_t += a.elapsed().as_secs_f64();
            let ev = world.take_events();
            let a = std::time::Instant::now();
            economy.after_step(&world, &ev, DT, 2);
            econ_t += a.elapsed().as_secs_f64();
        }
        let all = t0.elapsed().as_secs_f64() / steps as f64 * 1e6;
        eprintln!("per step: world only {world_only:.1} us | all {all:.1} us (crews {:.1} us, world {:.1} us, economy {:.1} us)", crew_t / steps as f64 * 1e6, world_t / steps as f64 * 1e6, econ_t / steps as f64 * 1e6);
        for c in &road_crews {
            eprintln!("  {}: {:?} {}", c.name, c.status, c.describe());
        }
        let moving = world.trains.iter().filter(|t| t.speed(&world.car_types).abs() > 0.1).count();
        eprintln!("  moving trains {moving}, t={:.0}", world.t);

    }
}
