use std::collections::HashMap;

use crate::base_graph::{Graph, InnerGraph, Neighbors};
use crate::common::{CarConfig, Edge, Error, Mode, Node, QueryInput, WorldState};
use crate::route::Route;

/**
 * Finds the best (lowest cost) route from `start` to `end` in
 * `base_graph`. Returns None if no route could be found.
 *
 * TODO: adjust the construction of the problem so that we can always
 * find a route.
 */
pub fn best_route<'a>(
    base_graph: &mut Graph,
    input: QueryInput,
    state: &'a WorldState,
) -> Result<Option<Route>, Error> {
    use cgmath::MetricSpace;
    use cgmath::Vector2;
    use itertools::Itertools;

    let inner = &base_graph.graph;

    let goal_vec = Vector2::from(
        inner
            .node_weight(base_graph.end_node)
            .unwrap()
            .location(&input),
    );

    let is_goal = |n| match inner.node_weight(n).unwrap() {
        Node::EndNode { .. } => true,
        _ => false,
    };
    let edge_cost = |e: petgraph::graph::EdgeReference<Edge>| {
        e.weight().cost(&input, state, base_graph.tile_size)
    };

    // This should be the fastest possible speed by any mode of transportation.
    // TODO: There is probably a more principled way to approach this.
    let top_speed = metro::timing::MAX_SPEED;
    let estimate_cost =
        |n| goal_vec.distance(inner.node_weight(n).unwrap().location(&input).into()) / top_speed;

    Ok(
        match petgraph::algo::astar(
            inner,
            base_graph.start_node,
            is_goal,
            edge_cost,
            estimate_cost,
        ) {
            Some((cost, path)) => Some(Route::new(
                path.iter()
                    .map(|n| inner.node_weight(*n).unwrap().clone())
                    .collect(),
                path.iter()
                    .tuple_windows()
                    .map(|(a, b)| {
                        inner
                            .edge_weight(inner.find_edge(*a, *b).unwrap())
                            .unwrap()
                            .clone()
                    })
                    .collect(),
                cost,
                input.clone(),
            )),
            None => None,
        },
    )
}
