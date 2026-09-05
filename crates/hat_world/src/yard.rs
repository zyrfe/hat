//! Ladder yard generator. Straight and arc geometry only, standard turnouts.

use glam::DVec2;
use hat_sim::*;

pub const TRACK_MAIN: u32 = 0;
pub const TRACK_LEAD: u32 = 100;

#[derive(Clone, Debug)]
pub struct YardParams {
    /// Number of ladder turnouts. Body tracks = this + 1.
    pub n_turnouts: usize,
    pub main_west: f64,
    pub main_east: f64,
    /// x of the lead turnout on the main.
    pub lead_x: f64,
    /// Body tracks end at this x with bumpers.
    pub body_end_x: f64,
    /// Center spacing between body tracks, m.
    pub spacing: f64,
    /// Turnout diverging angle, rad. A #8 is atan(1/8).
    pub turnout_angle: f64,
    /// Radius of the turnout and ladder curves, m.
    pub radius: f64,
    /// Straight ladder length between the lead arc and the first turnout, m.
    pub ladder_lead: f64,
    pub yard_speed: f64,
    pub main_speed: f64,
}

impl Default for YardParams {
    fn default() -> Self {
        YardParams {
            n_turnouts: 5,
            main_west: -3200.0,
            main_east: 3200.0,
            lead_x: -100.0,
            body_end_x: 600.0,
            spacing: 4.5,
            turnout_angle: (1.0f64 / 8.0).atan(),
            radius: 150.0,
            ladder_lead: 30.0,
            yard_speed: 6.7,
            main_speed: 27.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct YardTrack {
    pub id: u32,
    pub name: String,
    /// Arc then straight, in travel order from the ladder.
    pub edges: Vec<EdgeId>,
    /// Straight, usable length, m.
    pub length: f64,
    /// Turnout on the ladder that leads here, and which route.
    pub entry: (NodeId, Route),
}

#[derive(Clone, Debug)]
pub struct Yard {
    pub main_west: EdgeId,
    pub main_east: EdgeId,
    pub lead_switch: NodeId,
    /// Ladder turnouts in order from the lead, `T1..`.
    pub ladder_switches: Vec<NodeId>,
    pub lead_edges: Vec<EdgeId>,
    pub tracks: Vec<YardTrack>,
    pub min: DVec2,
    pub max: DVec2,
}

impl Yard {
    pub fn track(&self, id: u32) -> Option<&YardTrack> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_name(&self, id: u32) -> String {
        match id {
            TRACK_MAIN => "Main".to_string(),
            TRACK_LEAD => "Lead".to_string(),
            _ => self.track(id).map(|t| t.name.clone()).unwrap_or_else(|| format!("Track {id}")),
        }
    }

    /// Switch settings that route a train from the lead into `track`.
    pub fn route_to(&self, track: u32) -> Option<Vec<(NodeId, Route)>> {
        let t = self.track(track)?;
        let mut out = vec![(self.lead_switch, Route::Diverging)];
        for &sw in &self.ladder_switches {
            if sw == t.entry.0 {
                out.push((sw, t.entry.1));
                break;
            }
            out.push((sw, Route::Normal));
        }
        Some(out)
    }
}

pub fn build_ladder_yard(p: &YardParams) -> (TrackGraph, Yard) {
    let mut g = TrackGraph::default();
    let th = p.turnout_angle;
    let r = p.radius;

    // Main line with the lead turnout on it.
    let w_end = g.add_node(DVec2::new(p.main_west, 0.0), NodeKind::End, "Main west end");
    let t0 = g.add_node(DVec2::new(p.lead_x, 0.0), NodeKind::Plain, "T0");
    let e_end = g.add_node(DVec2::new(p.main_east, 0.0), NodeKind::End, "Main east end");
    let main_west = g.add_edge(w_end, t0, Geometry::Straight { a: DVec2::new(p.main_west, 0.0), b: DVec2::new(p.lead_x, 0.0) }, 0.0, p.main_speed, Some(TRACK_MAIN));
    let main_east = g.add_edge(t0, e_end, Geometry::Straight { a: DVec2::new(p.lead_x, 0.0), b: DVec2::new(p.main_east, 0.0) }, 0.0, p.main_speed, Some(TRACK_MAIN));

    // Lead arc turning left onto the ladder heading.
    let lead_arc = Geometry::arc_from(Pose::new(p.lead_x, 0.0, 0.0), r, th);
    let l0_pose = lead_arc.end();
    let l0 = g.add_node(l0_pose.pos, NodeKind::Plain, "L0");
    let lead_arc_e = g.add_edge(t0, l0, lead_arc, 0.0, p.yard_speed, Some(TRACK_LEAD));
    g.set_turnout(t0, main_west, main_east, lead_arc_e);

    let mut lead_edges = vec![lead_arc_e];
    let mut ladder_switches = Vec::new();
    let mut tracks = Vec::new();
    let mut min = DVec2::new(p.main_west, -10.0);
    let mut max = DVec2::new(p.main_east, 10.0);

    // Ladder straight to the first turnout.
    let ladder_step = p.spacing / th.sin();
    let first = Geometry::straight_from(l0_pose, p.ladder_lead);
    let mut cur_pose = first.end();
    let mut cur_node = g.add_node(cur_pose.pos, NodeKind::Plain, "T1");
    let mut incoming = g.add_edge(l0, cur_node, first, 0.0, p.yard_speed, Some(TRACK_LEAD));
    lead_edges.push(incoming);

    let body_track = |g: &mut TrackGraph, from: NodeId, from_pose: Pose, id: u32, entry: (NodeId, Route)| -> YardTrack {
        let arc = Geometry::arc_from(from_pose, r, -th);
        let b_pose = arc.end();
        let b = g.add_node(b_pose.pos, NodeKind::Plain, format!("B{id}"));
        let arc_e = g.add_edge(from, b, arc, 0.0, p.yard_speed, Some(id));
        let end_pos = DVec2::new(p.body_end_x, b_pose.pos.y);
        let e = g.add_node(end_pos, NodeKind::End, format!("Track {id} bumper"));
        let straight = Geometry::Straight { a: b_pose.pos, b: end_pos };
        let length = straight.length();
        let straight_e = g.add_edge(b, e, straight, 0.0, p.yard_speed, Some(id));
        YardTrack { id, name: format!("Track {id}"), edges: vec![arc_e, straight_e], length, entry }
    };

    for k in 1..=p.n_turnouts {
        let id = k as u32;
        ladder_switches.push(cur_node);
        let track = body_track(&mut g, cur_node, cur_pose, id, (cur_node, Route::Diverging));
        max.y = max.y.max(g.node(track_end_node(&g, &track)).pos.y + 10.0);
        tracks.push(track);

        let next_geom = Geometry::straight_from(cur_pose, ladder_step);
        let next_pose = next_geom.end();
        let last = k == p.n_turnouts;
        let next_node = g.add_node(next_pose.pos, NodeKind::Plain, if last { "L end".to_string() } else { format!("T{}", k + 1) });
        let outgoing = g.add_edge(cur_node, next_node, next_geom, 0.0, p.yard_speed, Some(TRACK_LEAD));
        lead_edges.push(outgoing);
        g.set_turnout(cur_node, incoming, outgoing, track_arc(&tracks[tracks.len() - 1]));
        if last {
            // The ladder's end curves into the final body track.
            let id = p.n_turnouts as u32 + 1;
            let track = body_track(&mut g, next_node, next_pose, id, (cur_node, Route::Normal));
            max.y = max.y.max(g.node(track_end_node(&g, &track)).pos.y + 10.0);
            tracks.push(track);
        }
        cur_pose = next_pose;
        cur_node = next_node;
        incoming = outgoing;
    }
    min.y = -10.0;
    let yard = Yard { main_west, main_east, lead_switch: t0, ladder_switches, lead_edges, tracks, min, max };
    (g, yard)
}

fn track_arc(t: &YardTrack) -> EdgeId {
    t.edges[0]
}

fn track_end_node(g: &TrackGraph, t: &YardTrack) -> NodeId {
    g.edge(t.edges[1]).b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loco_on_main(w: &mut World, yard: &Yard, s: f64) -> TrainId {
        let mut c = w.new_car(TYPE_SWITCHER);
        c.brake = Brake::charged();
        w.spawn_train(vec![c], yard.main_west, s, true).unwrap()
    }

    #[test]
    fn body_tracks_are_parallel_and_spaced() {
        let (g, yard) = build_ladder_yard(&YardParams::default());
        assert_eq!(yard.tracks.len(), 6);
        let ys: Vec<f64> = yard.tracks.iter().map(|t| g.node(g.edge(t.edges[1]).b).pos.y).collect();
        for w in ys.windows(2) {
            assert!((w[1] - w[0] - 4.5).abs() < 1e-6, "spacing {:?}", ys);
        }
        for t in &yard.tracks {
            let e = g.edge(t.edges[1]);
            assert!((e.geom.start().heading).abs() < 1e-9, "body track must run east");
            assert!(t.length > 400.0, "track {} is only {} m", t.id, t.length);
        }
    }

    #[test]
    fn a_train_reaches_every_body_track() {
        for target in 1..=6u32 {
            let (g, yard) = build_ladder_yard(&YardParams::default());
            let mut w = World::new(g);
            for (node, route) in yard.route_to(target).unwrap() {
                w.graph.set_route(node, route);
            }
            let len = w.graph.edge(yard.main_west).length;
            let id = loco_on_main(&mut w, &yard, len - 150.0);
            w.train_mut(id).unwrap().cars[0].v = 5.0;
            w.set_controls(id, Controls { throttle: 0, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
            for _ in 0..(150.0 / DT) as usize {
                w.step(DT);
                let tr = w.train(id).unwrap();
                if let Some(l) = w.car_location(tr, 0) {
                    if w.graph.edge(l.edge).track == Some(target) && l.s > 50.0 {
                        break;
                    }
                }
            }
            let tr = w.train(id).unwrap();
            assert!(!tr.derailed, "derailed on the way to track {target}: {:?}", w.events.iter().filter(|e| matches!(e, SimEvent::Derail { .. })).collect::<Vec<_>>());
            let l = w.car_location(tr, 0).unwrap();
            assert_eq!(w.graph.edge(l.edge).track, Some(target), "ended on the wrong track");
        }
    }

    #[test]
    fn trailing_through_points_set_against_you_derails() {
        let (g, yard) = build_ladder_yard(&YardParams::default());
        let mut w = World::new(g);
        // T1 left Normal; a car coming out of track 1 trails through against the points.
        let t1 = yard.tracks[0].edges[1];
        let mut c = w.new_car(TYPE_SWITCHER);
        c.brake = Brake::charged();
        let id = w.spawn_train(vec![c], t1, 30.0, false).unwrap();
        w.set_controls(id, Controls { throttle: 3, reverser: Reverser::Forward, independent: 0.0, ..Default::default() });
        for _ in 0..(60.0 / DT) as usize {
            w.step(DT);
        }
        assert!(w.train(id).unwrap().derailed);
        assert!(w.events.iter().any(|e| matches!(e, SimEvent::Derail { cause: DerailCause::SplitSwitch, .. })));
    }

    #[test]
    fn occupied_switch_cannot_be_thrown() {
        let (g, yard) = build_ladder_yard(&YardParams::default());
        let mut w = World::new(g);
        let len = w.graph.edge(yard.main_west).length;
        let _ = loco_on_main(&mut w, &yard, len - 1.0);
        assert!(w.throw_switch(yard.lead_switch).is_err());
        let (g, yard) = build_ladder_yard(&YardParams::default());
        let mut w = World::new(g);
        let _ = loco_on_main(&mut w, &yard, len - 100.0);
        assert!(w.throw_switch(yard.lead_switch).is_ok());
    }
}
