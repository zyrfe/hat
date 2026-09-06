//! The Terminal: a main-line loop with a portal where road trains come and go, a receiving
//! siding, the flat ladder yard with a loader and a dumper track, and a hump with a bowl.
//!
//! South main runs east at y = 0. The loop closes over the north main at y = 2R running
//! west. Everything hangs off the south main. Coordinates in meters.

use std::f64::consts::PI;

use glam::DVec2;
use hat_sim::*;

use crate::yard::*;

pub const TRACK_RECEIVING: u32 = 20;
pub const TRACK_PORTAL: u32 = 30;
pub const TRACK_HUMP: u32 = 101;
pub const TRACK_LOADER: u32 = 5;
pub const TRACK_DUMPER: u32 = 6;
pub const TRACK_DEPARTURE: u32 = 1;
pub const BOWL_BASE: u32 = 10;

#[derive(Clone, Debug)]
pub struct TerminalParams {
    pub half_len: f64,
    pub loop_radius: f64,
    pub main_speed: f64,
    pub arc_speed: f64,
    pub yard_speed: f64,
    pub turnout_angle: f64,
    pub spacing: f64,
    pub ladder_radius: f64,
    /// Radius of the S-curves that put a siding parallel to the main.
    pub siding_radius: f64,
    pub flat_tracks: usize,
    pub bowl_tracks: usize,
    pub loader_rate: f64,
    pub dumper_rate: f64,
}

impl Default for TerminalParams {
    fn default() -> Self {
        TerminalParams {
            half_len: 2200.0,
            loop_radius: 350.0,
            main_speed: 20.0,
            arc_speed: 12.0,
            yard_speed: 6.7,
            turnout_angle: (1.0f64 / 8.0).atan(),
            spacing: 4.5,
            ladder_radius: 150.0,
            siding_radius: 292.0,
            flat_tracks: 6,
            bowl_tracks: 4,
            loader_rate: 400.0,
            dumper_rate: 600.0,
        }
    }
}

struct LadderSpec {
    track_base: u32,
    names: fn(u32) -> String,
    n_turnouts: usize,
    body_end_x: f64,
    retarder: Option<f64>,
    body_grade: f64,
    /// Bowl profile: after this many meters the grade changes to the second value.
    second_grade: Option<(f64, f64)>,
}

/// Build a ladder starting with a left-turning lead arc at `from` (a node with `from_pose`).
/// Returns the ladder, its tracks, the lead edges, and the first edge (the lead arc).
fn attach_ladder(g: &mut TrackGraph, from: NodeId, from_pose: Pose, p: &TerminalParams, spec: &LadderSpec) -> (Ladder, Vec<YardTrack>, Vec<EdgeId>, EdgeId) {
    let th = p.turnout_angle;
    let r = p.ladder_radius;
    let lead_arc = Geometry::arc_from(from_pose, r, th);
    let l0_pose = lead_arc.end();
    let l0 = g.add_node_z(l0_pose.pos, g.node(from).z, NodeKind::Plain, format!("L{}", spec.track_base));
    let lead_arc_e = g.add_edge(from, l0, lead_arc, 0.0, p.yard_speed, Some(TRACK_LEAD));
    let mut lead_edges = vec![lead_arc_e];
    let mut switches = Vec::new();
    let mut tracks = Vec::new();

    let ladder_step = p.spacing / th.sin();
    let first = Geometry::straight_from(l0_pose, 30.0);
    let mut cur_pose = first.end();
    let mut cur_node = g.add_node_z(cur_pose.pos, g.node(from).z, NodeKind::Plain, format!("S{}", spec.track_base + 1));
    let mut incoming = g.add_edge(l0, cur_node, first, 0.0, p.yard_speed, Some(TRACK_LEAD));
    lead_edges.push(incoming);

    let base_z = g.node(from).z;
    let body_track = |g: &mut TrackGraph, from: NodeId, from_pose: Pose, id: u32, entry: (NodeId, Route)| -> YardTrack {
        let arc = Geometry::arc_from(from_pose, r, -th);
        let b_pose = arc.end();
        let b = g.add_node_z(b_pose.pos, base_z, NodeKind::Plain, format!("B{id}"));
        let arc_e = g.add_edge(from, b, arc, 0.0, p.yard_speed, Some(id));
        if let Some(v) = spec.retarder {
            g.edges[arc_e as usize].retarder = Some(v);
        }
        let end_pos = DVec2::new(spec.body_end_x, b_pose.pos.y);
        let total = (end_pos - b_pose.pos).length();
        let mut edges = vec![arc_e];
        match spec.second_grade {
            Some((split, g2)) if split < total - 20.0 => {
                let mid_pos = b_pose.pos + DVec2::new(split, 0.0);
                let z_mid = base_z + spec.body_grade * split;
                let mid = g.add_node_z(mid_pos, z_mid, NodeKind::Plain, format!("{} mid", (spec.names)(id)));
                edges.push(g.add_edge_graded(b, mid, Geometry::Straight { a: b_pose.pos, b: mid_pos }, p.yard_speed, Some(id)));
                let e = g.add_node_z(end_pos, z_mid + g2 * (total - split), NodeKind::End, format!("{} bumper", (spec.names)(id)));
                edges.push(g.add_edge_graded(mid, e, Geometry::Straight { a: mid_pos, b: end_pos }, p.yard_speed, Some(id)));
            }
            _ => {
                let e = g.add_node_z(end_pos, base_z + spec.body_grade * total, NodeKind::End, format!("{} bumper", (spec.names)(id)));
                edges.push(g.add_edge_graded(b, e, Geometry::Straight { a: b_pose.pos, b: end_pos }, p.yard_speed, Some(id)));
            }
        }
        YardTrack { id, name: (spec.names)(id), edges, length: total, entry }
    };

    for k in 1..=spec.n_turnouts {
        let id = spec.track_base + k as u32;
        switches.push(cur_node);
        let track = body_track(g, cur_node, cur_pose, id, (cur_node, Route::Diverging));
        let arc_e = track.edges[0];
        tracks.push(track);
        let next_geom = Geometry::straight_from(cur_pose, ladder_step);
        let next_pose = next_geom.end();
        let last = k == spec.n_turnouts;
        let next_node = g.add_node_z(next_pose.pos, base_z, NodeKind::Plain, if last { format!("S{} end", spec.track_base) } else { format!("S{}", spec.track_base + k as u32 + 1) });
        let outgoing = g.add_edge(cur_node, next_node, next_geom, 0.0, p.yard_speed, Some(TRACK_LEAD));
        lead_edges.push(outgoing);
        g.set_turnout(cur_node, incoming, outgoing, arc_e);
        if last {
            let id = spec.track_base + spec.n_turnouts as u32 + 1;
            let track = body_track(g, next_node, next_pose, id, (cur_node, Route::Normal));
            tracks.push(track);
        }
        cur_pose = next_pose;
        cur_node = next_node;
        incoming = outgoing;
    }
    let ladder = Ladder { entry: None, switches, tracks: tracks.iter().map(|t| t.id).collect() };
    (ladder, tracks, lead_edges, lead_arc_e)
}

/// An S-curve: turn `sign` then back, ending parallel to the start, offset sideways.
pub(crate) fn s_curve(g: &mut TrackGraph, from: NodeId, from_pose: Pose, r: f64, th: f64, sign: f64, speed: f64, track: Option<u32>, name: &str) -> (NodeId, Pose, Vec<EdgeId>) {
    let a1 = Geometry::arc_from(from_pose, r, sign * th);
    let p1 = a1.end();
    let n1 = g.add_node(p1.pos, NodeKind::Plain, format!("{name} s1"));
    let e1 = g.add_edge(from, n1, a1, 0.0, speed, track);
    let a2 = Geometry::arc_from(p1, r, -sign * th);
    let p2 = a2.end();
    let n2 = g.add_node(p2.pos, NodeKind::Plain, format!("{name} s2"));
    let e2 = g.add_edge(n1, n2, a2, 0.0, speed, track);
    (n2, p2, vec![e1, e2])
}

pub fn build_terminal(p: &TerminalParams) -> (TrackGraph, Yard) {
    let mut g = TrackGraph::default();
    let th = p.turnout_angle;
    let h = p.half_len;
    let r = p.loop_radius;
    let east = |x: f64, y: f64| Pose::new(x, y, 0.0);

    // South main with its junction nodes, west to east.
    let xs = [(-h, "W arc end", false), (-1900.0, "Wr", true), (-700.0, "Er", true), (-100.0, "T0", true), (700.0, "Th", true), (h, "E arc start", false)];
    let nodes: Vec<NodeId> = xs.iter().map(|(x, name, _)| g.add_node(DVec2::new(*x, 0.0), NodeKind::Plain, *name)).collect();
    let mut main_edges = Vec::new();
    for w in nodes.windows(2) {
        let (a, b) = (w[0], w[1]);
        let geom = Geometry::Straight { a: g.node(a).pos, b: g.node(b).pos };
        main_edges.push(g.add_edge(a, b, geom, 0.0, p.main_speed, Some(TRACK_MAIN)));
    }
    let [w_arc_end, wr, er, t0, thump, e_arc_start] = [nodes[0], nodes[1], nodes[2], nodes[3], nodes[4], nodes[5]];
    let [main_w0, main_w1, main_w2, main_e0, main_e1] = [main_edges[0], main_edges[1], main_edges[2], main_edges[3], main_edges[4]];

    // East arc up to the north main, north main west with the portal, west arc back down.
    let east_arc = Geometry::arc_from(east(h, 0.0), r, PI);
    let n_e = g.add_node(east_arc.end().pos, NodeKind::Plain, "N east");
    g.add_edge(e_arc_start, n_e, east_arc, 0.0, p.arc_speed, Some(TRACK_MAIN));
    let ytop = 2.0 * r;
    let p_e = g.add_node(DVec2::new(300.0, ytop), NodeKind::Plain, "Portal east");
    let p_w = g.add_node(DVec2::new(-300.0, ytop), NodeKind::Plain, "Portal");
    let n_w = g.add_node(DVec2::new(-h, ytop), NodeKind::Plain, "N west");
    g.add_edge(n_e, p_e, Geometry::Straight { a: DVec2::new(h, ytop), b: DVec2::new(300.0, ytop) }, 0.0, p.main_speed, Some(TRACK_MAIN));
    let portal_edge = g.add_edge(p_e, p_w, Geometry::Straight { a: DVec2::new(300.0, ytop), b: DVec2::new(-300.0, ytop) }, 0.0, p.main_speed, Some(TRACK_PORTAL));
    g.add_edge(p_w, n_w, Geometry::Straight { a: DVec2::new(-300.0, ytop), b: DVec2::new(-h, ytop) }, 0.0, p.main_speed, Some(TRACK_MAIN));
    let west_arc = Geometry::arc_from(Pose::new(-h, ytop, PI), r, PI);
    debug_assert!((west_arc.end().pos - DVec2::new(-h, 0.0)).length() < 1e-6);
    g.add_edge(n_w, w_arc_end, west_arc, 0.0, p.arc_speed, Some(TRACK_MAIN));

    // Receiving siding on the south side between Wr and Er.
    let (rs_start, rs_pose, r_in) = s_curve(&mut g, wr, east(-1900.0, 0.0), p.siding_radius, th, -1.0, p.yard_speed, Some(TRACK_RECEIVING), "R west");
    let s_span = 2.0 * p.siding_radius * th.sin();
    let rs_end_pos = DVec2::new(-700.0 - s_span, rs_pose.pos.y);
    let rs_end = g.add_node(rs_end_pos, NodeKind::Plain, "R east s0");
    let r_straight = g.add_edge(rs_start, rs_end, Geometry::Straight { a: rs_pose.pos, b: rs_end_pos }, 0.0, p.yard_speed, Some(TRACK_RECEIVING));
    let (r_join, r_join_pose, r_out) = s_curve(&mut g, rs_end, Pose::new(rs_end_pos.x, rs_end_pos.y, 0.0), p.siding_radius, th, 1.0, p.yard_speed, Some(TRACK_RECEIVING), "R east");
    debug_assert!((r_join_pose.pos - DVec2::new(-700.0, 0.0)).length() < 1e-3, "{:?}", r_join_pose.pos);
    // Merge the S-curve's end node into Er: rewire its last edge to end at Er.
    let last = *r_out.last().unwrap();
    g.edges[last as usize].b = er;
    g.nodes[er as usize].edges.push(last);
    g.nodes[r_join as usize].edges.clear();
    g.set_turnout(wr, main_w0, main_w1, r_in[0]);
    g.set_turnout(er, main_w2, main_w1, last);
    let receiving_len = g.edge(r_straight).length;

    // Flat yard off T0, tracks 1..=flat_tracks, ending at x = 600.
    let flat_spec = LadderSpec { track_base: 0, names: |id| format!("Track {id}"), n_turnouts: p.flat_tracks - 1, body_end_x: 600.0, retarder: None, body_grade: 0.0, second_grade: None };
    let (mut flat_ladder, mut flat_tracks, lead_edges, flat_lead_arc) = attach_ladder(&mut g, t0, east(-100.0, 0.0), p, &flat_spec);
    flat_ladder.entry = Some((t0, Route::Diverging));
    g.set_turnout(t0, main_w2, main_e0, flat_lead_arc);
    for t in flat_tracks.iter_mut() {
        if t.id == TRACK_LOADER {
            t.name = "Loader (5)".into();
            g.edges[t.edges[1] as usize].facility = Some(Facility::load(Commodity::Coal, p.loader_rate, FacilityMode::Track { max_speed: 0.3 }));
        }
        if t.id == TRACK_DUMPER {
            t.name = "Dumper (6)".into();
            g.edges[t.edges[1] as usize].facility = Some(Facility::unload(None, p.dumper_rate, FacilityMode::Track { max_speed: 0.3 }));
        }
        if t.id == TRACK_DEPARTURE {
            t.name = "Departure (1)".into();
        }
    }

    // Hump off Th: S-curve to a parallel approach, climb, crest, descent, level, bowl ladder.
    let (h0, h0_pose, hump_in) = s_curve(&mut g, thump, east(700.0, 0.0), p.siding_radius, th, 1.0, p.yard_speed, Some(TRACK_HUMP), "Hump");
    let y_h = h0_pose.pos.y;
    let approach_end = DVec2::new(1150.0, y_h);
    let h1 = g.add_node(approach_end, NodeKind::Plain, "Hump approach end");
    g.add_edge(h0, h1, Geometry::Straight { a: h0_pose.pos, b: approach_end }, 0.0, p.yard_speed, Some(TRACK_HUMP));
    let crest_pos = DVec2::new(1270.0, y_h);
    let crest = g.add_node_z(crest_pos, 2.4, NodeKind::Plain, "Crest");
    let climb = g.add_edge_graded(h1, crest, Geometry::Straight { a: approach_end, b: crest_pos }, p.yard_speed, Some(TRACK_HUMP));
    let foot_pos = DVec2::new(1330.0, y_h);
    let foot = g.add_node_z(foot_pos, 0.0, NodeKind::Plain, "Hump foot");
    let descent = g.add_edge_graded(crest, foot, Geometry::Straight { a: crest_pos, b: foot_pos }, p.yard_speed, Some(TRACK_HUMP));
    let b0_pos = DVec2::new(1370.0, y_h);
    let b0 = g.add_node(b0_pos, NodeKind::Plain, "Bowl lead");
    g.add_edge(foot, b0, Geometry::Straight { a: foot_pos, b: b0_pos }, 0.0, p.yard_speed, Some(TRACK_HUMP));
    g.set_turnout(thump, main_e0, main_e1, hump_in[0]);
    let bowl_spec = LadderSpec { track_base: BOWL_BASE, names: |id| format!("Bowl {}", id - BOWL_BASE), n_turnouts: p.bowl_tracks - 1, body_end_x: 2100.0, retarder: Some(1.5), body_grade: -0.001, second_grade: Some((260.0, 0.0006)) };
    let (mut bowl_ladder, bowl_tracks, _bowl_lead, _) = attach_ladder(&mut g, b0, Pose::new(b0_pos.x, b0_pos.y, 0.0), p, &bowl_spec);
    bowl_ladder.entry = Some((thump, Route::Diverging));

    let mut tracks = flat_tracks;
    tracks.extend(bowl_tracks);
    tracks.push(YardTrack { id: TRACK_RECEIVING, name: "Receiving".into(), edges: vec![r_straight], length: receiving_len, entry: (wr, Route::Diverging) });
    let ladder_switches = flat_ladder.switches.clone();
    let yard = Yard {
        main_west: main_w2,
        main_east: main_e0,
        lead_switch: t0,
        ladder_switches,
        lead_edges,
        tracks,
        ladders: vec![flat_ladder, bowl_ladder],
        min: DVec2::new(-h - r - 20.0, -20.0),
        max: DVec2::new(h + r + 20.0, ytop + 20.0),
        portal: Some(p_w),
        receiving: Some((TRACK_RECEIVING, wr, er)),
        hump: Some(Hump { turnout: thump, climb, crest, descent, bowl: 1 }),
        departure: Some(TRACK_DEPARTURE),
    };
    let _ = portal_edge;
    (g, yard)
}

/// Edge trains spawn on: the portal.
pub fn portal_edge(g: &TrackGraph, yard: &Yard) -> Option<EdgeId> {
    let p = yard.portal?;
    g.node(p).edges.iter().copied().find(|&e| g.edge(e).track == Some(TRACK_PORTAL))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loco_at(w: &mut World, edge: EdgeId, s: f64, forward: bool) -> TrainId {
        let mut c = w.new_car(TYPE_SWITCHER);
        c.brake = Brake::charged();
        c.knuckle_open = [true, true];
        w.spawn_train(vec![c], edge, s, forward).unwrap()
    }

    /// Hold a light engine near `v_hold` m/s.
    fn govern(w: &mut World, id: TrainId, v_hold: f64) {
        let v = w.train(id).unwrap().speed(&w.car_types).abs();
        let c = if v < v_hold - 0.3 {
            Controls { throttle: 2, reverser: Reverser::Forward, independent: 0.0, ..Default::default() }
        } else if v > v_hold + 0.3 {
            Controls { throttle: 0, reverser: Reverser::Forward, independent: 0.4, ..Default::default() }
        } else {
            Controls { throttle: 1, reverser: Reverser::Forward, independent: 0.0, ..Default::default() }
        };
        w.set_controls(id, c);
    }

    #[test]
    fn geometry_closes_and_sidings_rejoin() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        // Every node with two or more edges has consistent endpoints.
        for (ni, n) in g.nodes.iter().enumerate() {
            for &e in &n.edges {
                let edge = g.edge(e);
                let end = if edge.a == ni as NodeId { edge.geom.start() } else { edge.geom.end() };
                assert!((end.pos - n.pos).length() < 0.05, "edge {e} does not meet node {} ({}) : {:?} vs {:?}", ni, n.name, end.pos, n.pos);
            }
        }
        assert_eq!(yard.tracks.len(), 6 + 4 + 1);
        assert!(yard.route_to(12).is_some());
        assert!(yard.route_to(3).is_some());
    }

    #[test]
    fn a_light_engine_runs_the_whole_loop_from_the_portal() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let pe = portal_edge(&w.graph, &yard).unwrap();
        let id = loco_at(&mut w, pe, 550.0, true);
        w.train_mut(id).unwrap().cars[0].v = 5.0;
        let mut left_portal = false;
        let mut back = false;
        for _ in 0..(2500.0 / DT) as usize {
            govern(&mut w, id, 10.0);
            w.step(DT);
            let tr = w.train(id).unwrap();
            if tr.derailed {
                break;
            }
            let on_portal = w.car_location(tr, 0).map(|l| w.graph.edge(l.edge).track == Some(TRACK_PORTAL)).unwrap_or(false);
            if !on_portal {
                left_portal = true;
            } else if left_portal {
                back = true;
                break;
            }
        }
        let tr = w.train(id).unwrap();
        assert!(!tr.derailed, "derailed: {:?}", w.events.iter().filter(|e| matches!(e, SimEvent::Derail { .. })).collect::<Vec<_>>());
        assert!(back, "engine should come back around to the portal");
    }

    #[test]
    fn road_train_can_enter_and_leave_receiving() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let (rt, wr, er) = yard.receiving.unwrap();
        w.graph.set_route(wr, Route::Diverging);
        w.graph.set_route(er, Route::Diverging);
        let pe = portal_edge(&w.graph, &yard).unwrap();
        let id = loco_at(&mut w, pe, 550.0, true);
        w.train_mut(id).unwrap().cars[0].v = 5.0;
        let mut seen_receiving = false;
        let mut past = false;
        for _ in 0..(2500.0 / DT) as usize {
            govern(&mut w, id, 6.0);
            w.step(DT);
            let tr = w.train(id).unwrap();
            if tr.derailed {
                break;
            }
            let track = w.car_location(tr, 0).and_then(|l| w.graph.edge(l.edge).track);
            if track == Some(rt) {
                seen_receiving = true;
            } else if seen_receiving && track == Some(TRACK_MAIN) {
                past = true;
                break;
            }
        }
        assert!(!w.train(id).unwrap().derailed);
        assert!(seen_receiving && past, "should run through the receiving track and rejoin the main");
    }

    #[test]
    fn a_car_cut_at_the_crest_rolls_into_its_bowl_track_and_couples_gently() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let hump = yard.hump.clone().unwrap();
        for (node, route) in yard.route_to(12).unwrap() {
            w.graph.set_route(node, route);
        }
        // A car already standing in Bowl 2, tied down.
        let bowl2 = yard.track(12).unwrap();
        let mut standing = w.new_car(TYPE_OPEN_HOPPER);
        standing.m_payload = 100_000.0;
        standing.hand_brake = 1.0;
        standing.knuckle_open = [true, true];
        let st = w.spawn_train(vec![standing], bowl2.edges[1], 200.0, true).unwrap();
        // Engine and two loaded hoppers on the hump, the leading coupler just past the crest.
        let mut cars = Vec::new();
        for _ in 0..2 {
            let mut c = w.new_car(TYPE_OPEN_HOPPER);
            c.m_payload = 100_000.0;
            c.knuckle_open = [true, true];
            cars.push(c);
        }
        let mut l = w.new_car(TYPE_SWITCHER);
        l.brake = Brake::charged();
        cars.push(l);
        let lead_len = w.car_types[TYPE_OPEN_HOPPER as usize].length;
        let id = w.spawn_train(cars, hump.descent, lead_len + 0.5, true).unwrap();
        for c in &mut w.train_mut(id).unwrap().cars {
            c.v = 1.0;
        }
        // The head part keeps `id`: that is the single rolling car. The engine gets the new id.
        let engine = w.pull_pin(id, 1).expect("pin pulls at the crest");
        let rolled = id;
        w.set_controls(engine, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
        let mut vmax = 0.0f64;
        for _ in 0..(400.0 / DT) as usize {
            w.step(DT);
            if let Some(t) = w.train(rolled) {
                vmax = vmax.max(t.cars[0].v.abs());
            } else {
                break;
            }
        }
        let coupled = w.events.iter().find(|e| matches!(e, SimEvent::Coupled { .. }));
        if let Some(t) = w.train(rolled) {
            let loc = w.car_location(t, 0);
            panic!("rolling car never joined: at {:?} v={:.2} vmax={:.2}", loc.map(|l| (w.graph.edge(l.edge).track, l.s)), t.cars[0].v, vmax);
        }
        match coupled {
            Some(SimEvent::Coupled { rel_speed, hard, .. }) => {
                eprintln!("joined at {:.2} m/s after peaking at {:.2} m/s", rel_speed, vmax);
                assert!(!hard, "retarders should make the joint gentle, got {rel_speed}");
            }
            _ => panic!("no coupling"),
        }
        let joined = w.train(st).or_else(|| w.trains.iter().find(|t| t.cars.len() == 2)).expect("standing cut");
        let loc = w.car_location(joined, 0).unwrap();
        assert_eq!(w.graph.edge(loc.edge).track, Some(12));
        assert!(w.events.iter().all(|e| !matches!(e, SimEvent::Derail { .. })));
    }

    #[test]
    fn loader_fills_standing_empties() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let t5 = yard.track(TRACK_LOADER).unwrap();
        let mut c = w.new_car(TYPE_OPEN_HOPPER);
        c.hand_brake = 1.0;
        let id = w.spawn_train(vec![c], t5.edges[1], 100.0, true).unwrap();
        for _ in 0..(300.0 / DT) as usize {
            w.step(DT);
        }
        let car = &w.train(id).unwrap().cars[0];
        assert!(car.m_payload > 90_000.0, "payload {}", car.m_payload);
        assert_eq!(car.commodity, Commodity::Coal);
        assert!(w.events.iter().any(|e| matches!(e, SimEvent::Loaded { .. })));
        assert!(w.cargo_loaded > 90_000.0);
    }
}

#[cfg(test)]
mod climb_tests {
    use super::*;

    /// A switcher shoving a mixed twenty-car cut up the hump climb must keep moving.
    #[test]
    fn switcher_pushes_twenty_cars_up_the_climb() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let hump = yard.hump.clone().unwrap();
        let mut cars = Vec::new();
        for i in 0..20 {
            let (ty, payload) = match i % 5 {
                0 | 1 => (TYPE_OPEN_HOPPER, 100_000.0),
                2 => (TYPE_BOXCAR, 30_000.0),
                3 => (TYPE_GONDOLA, 80_000.0),
                _ => (TYPE_OPEN_HOPPER, 0.0),
            };
            let mut c = w.new_car(ty);
            c.m_payload = payload;
            cars.push(c);
        }
        let mut l = w.new_car(TYPE_SWITCHER);
        l.brake = Brake::charged();
        cars.push(l);
        // Head car on the climb, 100 m up it.
        let id = w.spawn_train(cars, hump.climb, 108.0, true).unwrap();
        let mass = w.train(id).unwrap().total_mass(&w.car_types);
        w.set_controls(id, Controls { throttle: 8, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
        let mut vmax = 0.0f64;
        let mut trace = Vec::new();
        for i in 0..(40.0 / DT) as usize {
            w.step(DT);
            let tr = w.train(id).unwrap();
            let v = tr.speed(&w.car_types);
            vmax = vmax.max(v);
            if i % 240 == 0 {
                trace.push(format!("t={:>4.1} loco v={:+.3} head v={:+.3} head s={:.1} zone0={:?}", w.t, v, tr.cars[0].v, w.car_location(tr, 0).map(|l| l.s).unwrap_or(-1.0), tr.zones[0]));
            }
        }
        for l in &trace {
            eprintln!("{l}");
        }
        eprintln!("mass {:.0} t, vmax {:.3}", mass / 1000.0, vmax);
        assert!(vmax > 0.3, "the cut should climb, vmax {vmax}");
    }
}

#[cfg(test)]
mod hump_push_tests {
    use super::*;

    /// After a crest cut, the next shove pushes the loose car over without re-coupling.
    #[test]
    fn shoving_a_cut_car_over_the_crest_does_not_recouple() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let mut w = World::new(g);
        let hump = yard.hump.clone().unwrap();
        for (node, route) in yard.route_to(12).unwrap() {
            w.graph.set_route(node, route);
        }
        let mut cars = Vec::new();
        for _ in 0..3 {
            let mut c = w.new_car(TYPE_GONDOLA);
            c.m_payload = 80_000.0;
            c.knuckle_open = [true, true];
            cars.push(c);
        }
        let mut l = w.new_car(TYPE_SWITCHER);
        l.brake = Brake::charged();
        cars.push(l);
        // Lead car centre 16 m short of the crest, as the crew leaves it.
        let climb = w.graph.edge(hump.climb);
        let lead_len = w.car_types[TYPE_GONDOLA as usize].length;
        let id = w.spawn_train(cars, hump.climb, climb.length - 16.0 + lead_len / 2.0, true).unwrap();
        w.set_controls(id, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
        for _ in 0..(3.0 / DT) as usize {
            w.step(DT);
        }
        let e = w.train(id).unwrap().coupler_extension(&w.car_types, 1);
        eprintln!("before cut: coupler 1 extension {:+.4} m", e);
        let rest = w.pull_pin(id, 1).expect("pin pulls with the lead car's weight on the climb");
        let cut_car = id;
        let excl_lead = w.train(cut_car).unwrap().cars[0].no_couple_with;
        let excl_head = w.train(rest).unwrap().cars[0].no_couple_with;
        eprintln!("exclusions: cut car {:?}, new head {:?}", excl_lead, excl_head);
        // Shove the rest forward at notch 3.
        w.set_controls(rest, Controls { throttle: 3, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
        let mut log = Vec::new();
        let mut holding = false;
        for i in 0..(150.0 / DT) as usize {
            if !holding {
                if let Some(t) = w.train(cut_car) {
                    let over = w.car_location(t, 0).map(|l| l.edge == hump.descent && l.s > 2.0).unwrap_or(false);
                    if over {
                        holding = true;
                        w.set_controls(rest, Controls { throttle: 0, reverser: Reverser::Neutral, independent: 1.0, ..Default::default() });
                    }
                }
            }
            w.step(DT);
            for ev in w.take_events() {
                match ev {
                    SimEvent::Coupled { rel_speed, train, .. } => log.push(format!("t={:.1} COUPLED rel={:.3} train {train}", w.t, rel_speed)),
                    SimEvent::Bumped { rel_speed, .. } if rel_speed > 0.2 => log.push(format!("t={:.1} bump rel={:.2}", w.t, rel_speed)),
                    _ => {}
                }
            }
            if i % 1200 == 1199 {
                let cut = w.train(cut_car).map(|t| (w.car_location(t, 0).map(|l| (w.graph.edge(l.edge).track, l.s)), t.cars[0].v));
                let rest_t = w.train(rest).map(|t| (t.cars.len(), w.car_location(t, 0).map(|l| l.s), t.cars[0].v));
                log.push(format!("t={:.0} cut car {:?}  rest {:?}", w.t, cut, rest_t));
            }
        }
        for l in &log {
            eprintln!("{l}");
        }
        assert!(w.train(cut_car).is_some(), "the cut car must remain its own train");
        assert_eq!(w.train(rest).unwrap().cars.len(), 3, "the rest must not have picked the cut car back up");
        let loc = w.car_location(w.train(cut_car).unwrap(), 0).unwrap();
        assert_eq!(w.graph.edge(loc.edge).track, Some(12), "the cut car should have rolled into Bowl 2");
    }
}
