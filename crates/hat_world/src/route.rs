//! Routing over the track graph: which switches to line to get from here to there, and
//! how far it is. Dijkstra over (edge, direction) states, so a route never reverses.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use hat_sim::*;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoutePlan {
    /// Switch settings in travel order.
    pub settings: Vec<(NodeId, Route)>,
    /// Edges in travel order, starting with the one the train is on.
    pub edges: Vec<EdgeId>,
    /// True when the target edge is entered at its `a` end.
    pub target_along_s: bool,
    /// Distance from the starting face to the start of the target edge, m.
    pub length: f64,
}

#[derive(Clone, Copy, PartialEq)]
struct Item {
    cost: f64,
    edge: EdgeId,
    along: bool,
}

impl Eq for Item {}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost).then_with(|| self.edge.cmp(&other.edge)).then_with(|| self.along.cmp(&other.along))
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Shortest route from `s` on `from_edge`, travelling along increasing `s` when `along`,
/// to any point on `target`. `penalty(edge)` adds cost to an edge, for example one that
/// another train stands on. The starting edge counts as reached only after leaving it.
pub fn route(graph: &TrackGraph, from_edge: EdgeId, along: bool, s: f64, target: EdgeId, penalty: &dyn Fn(EdgeId) -> f64) -> Option<RoutePlan> {
    if from_edge == target {
        return Some(RoutePlan { settings: vec![], edges: vec![from_edge], target_along_s: along, length: 0.0 });
    }
    let e0 = graph.edge(from_edge);
    let start_cost = if along { e0.length - s } else { s };
    let mut best: HashMap<(EdgeId, bool), f64> = HashMap::new();
    let mut prev: HashMap<(EdgeId, bool), ((EdgeId, bool), Option<(NodeId, Route)>)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    best.insert((from_edge, along), start_cost);
    heap.push(Item { cost: start_cost, edge: from_edge, along });
    while let Some(Item { cost, edge, along }) = heap.pop() {
        if best.get(&(edge, along)).map_or(false, |&b| cost > b + 1e-9) {
            continue;
        }
        if edge == target {
            // Rebuild.
            let mut settings = Vec::new();
            let mut edges = vec![edge];
            let mut cur = (edge, along);
            while let Some((p, setting)) = prev.get(&cur) {
                if let Some(st) = setting {
                    settings.push(*st);
                }
                edges.push(p.0);
                cur = *p;
            }
            settings.reverse();
            edges.reverse();
            let length = cost - graph.edge(target).length - penalty(target);
            return Some(RoutePlan { settings, edges, target_along_s: along, length: length.max(0.0) });
        }
        let e = graph.edge(edge);
        let node = if along { e.b } else { e.a };
        let mut nexts: Vec<(EdgeId, Option<(NodeId, Route)>)> = Vec::new();
        match &graph.node(node).kind {
            NodeKind::End => {}
            NodeKind::Plain => {
                if let Some(&other) = graph.node(node).edges.iter().find(|&&x| x != edge) {
                    nexts.push((other, None));
                }
            }
            NodeKind::Turnout { toe, normal, diverging, .. } => {
                if edge == *toe {
                    nexts.push((*normal, Some((node, Route::Normal))));
                    nexts.push((*diverging, Some((node, Route::Diverging))));
                } else if edge == *normal {
                    nexts.push((*toe, Some((node, Route::Normal))));
                } else if edge == *diverging {
                    nexts.push((*toe, Some((node, Route::Diverging))));
                }
            }
        }
        for (next, setting) in nexts {
            let ne = graph.edge(next);
            let next_along = ne.a == node;
            let c = cost + ne.length + penalty(next);
            let key = (next, next_along);
            if best.get(&key).map_or(true, |&b| c < b - 1e-9) {
                best.insert(key, c);
                prev.insert(key, ((edge, along), setting));
                heap.push(Item { cost: c, edge: next, along: next_along });
            }
        }
    }
    None
}

/// Route for a train from its leading face in train direction `forward`. Edges that other
/// trains stand on are avoided when there is a way around.
pub fn route_for_train(world: &World, train: &Train, forward: bool, target: EdgeId) -> Option<RoutePlan> {
    let types = &world.car_types;
    let face_x = if forward { train.head_face_x(types) } else { train.tail_face_x(types) };
    let loc = train.locate(&world.graph, face_x)?;
    let along = loc.forward == forward;
    let occupied: std::collections::HashSet<EdgeId> = world.edge_intervals().into_iter().filter(|(ti, _, _, _)| world.trains[*ti].id != train.id).map(|(_, e, _, _)| e).collect();
    let penalty = |e: EdgeId| if e != target && occupied.contains(&e) { 5_000.0 } else { 0.0 };
    route(&world.graph, loc.edge, along, loc.s, target, &penalty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::*;
    use crate::yard::*;

    #[test]
    fn routes_from_the_portal_into_a_yard_track_and_around_the_loop() {
        let (g, yard) = build_terminal(&TerminalParams::default());
        let pe = portal_edge(&g, &yard).unwrap();
        // Portal edge runs west; travelling along s means westbound, which is the loop direction.
        let t3 = yard.track(3).unwrap();
        let plan = route(&g, pe, true, 300.0, t3.edges[1], &|_| 0.0).expect("a route");
        assert!(plan.length > 2000.0, "portal to track 3 is a long way round: {}", plan.length);
        // The plan lines the lead switch diverging and the ladder to track 3.
        assert!(plan.settings.contains(&(yard.lead_switch, Route::Diverging)));
        let expected = yard.route_to(3).unwrap();
        for st in &expected {
            assert!(plan.settings.contains(st), "missing {st:?} in {:?}", plan.settings);
        }
        // Against the loop direction there is no way: the lead is a facing point only from the west.
        assert!(route(&g, pe, false, 300.0, t3.edges[1], &|_| 0.0).map(|p| p.length > 2000.0).unwrap_or(true));
    }

    #[test]
    fn a_train_on_a_body_track_routes_out_through_the_ladder() {
        let (g, yard) = build_ladder_yard(&YardParams::default());
        let t4 = yard.track(4).unwrap();
        // Standing on track 4 heading west (against s) toward the ladder.
        let plan = route(&g, t4.edges[1], false, 100.0, yard.main_west, &|_| 0.0).expect("a route");
        assert_eq!(plan.settings.iter().filter(|(n, _)| *n == yard.lead_switch).count(), 1);
        assert!(plan.settings.contains(&(t4.entry.0, t4.entry.1)));
        assert!(plan.edges.first() == Some(&t4.edges[1]) && plan.edges.last() == Some(&yard.main_west));
    }
}
