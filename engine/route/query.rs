use uom::si::time::hour;
use uom::si::u64::Time;

use crate::base_graph::{Graph, InnerGraph, Neighbors, NodeIndex};
use crate::common::{CarConfig, Error, Mode, QueryInput};
use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;
use crate::traffic::WorldState;

fn perform_query<'a>(
    base_graph: &mut InnerGraph,
    start_id: NodeIndex,
    end_id: NodeIndex,
) -> Result<Option<(f64, Vec<NodeIndex>)>, Error> {
    use cgmath::MetricSpace;
    use cgmath::Vector2;

    let path = {
        let shortest_path = base_graph.query(start_id, end_id);
        match shortest_path {
            // TODO: remove this clone
            Some(p) if p.is_found() => Some((p.get_weight() as f64, p.get_nodes().clone())),
            _ => None,
        }
    };

    Ok(path)
}

fn construct_route(
    base_graph: &InnerGraph,
    input: QueryInput,
    cost: f64,
    path: &Vec<NodeIndex>,
    start: quadtree::Address,
    end: quadtree::Address,
    start_mode: Mode,
    end_mode: Mode,
    start_dist: f64,
    end_dist: f64,
) -> Route {
    use itertools::Itertools;
    use std::iter::once;

    Route::new(
        once(Node::Endpoint { address: start })
            .chain(
                path.iter()
                    .map(|n| base_graph.node_weight(*n).unwrap().clone()),
            )
            .chain(once(Node::Endpoint { address: end }))
            .collect(),
        once(Edge::ModeSegment {
            mode: start_mode,
            distance: start_dist,
        })
        .chain(path.iter().tuple_windows().map(|(a, b)| {
            base_graph
                .edge_weight(base_graph.find_edge(*a, *b).unwrap())
                .unwrap()
                .clone()
        }))
        .chain(once(Edge::ModeSegment {
            mode: end_mode,
            distance: end_dist,
        }))
        .collect(),
        cost as f32,
        input,
        start_mode,
        end_mode,
    )
}

/**
 * Finds the best (lowest cost) route from `start` to `end` in
 * `base_graph`. Returns None if no route could be found.
 *
 * TODO: adjust the construction of the problem so that we can always
 * find a route.
 */
pub fn best_route<'a>(
    mut base_graph: std::cell::RefMut<Graph>,
    input: QueryInput,
) -> Result<Option<Route>, Error> {
    use cgmath::MetricSpace;

    let mut fastest: Option<(f64, Vec<NodeIndex>, Mode, Mode, f64, f64)> = None;

    let mut attempt_route = |start: quadtree::Address,
                             start_mode,
                             end: quadtree::Address,
                             end_mode|
     -> Result<(), Error> {
        let (start_x, start_y) = start.to_xy();
        let (end_x, end_y) = end.to_xy();

        // TODO: precompute these values and store in the qtree
        let start_id =
            base_graph.neighbors[&start_mode].find_nearest(start_x as f64, start_y as f64);
        let end_id = base_graph.neighbors[&end_mode].find_nearest(end_x as f64, end_y as f64);

        if let (Some(start_id), Some(end_id)) = (start_id, end_id) {
            let path = perform_query(&mut base_graph.graph, start_id, end_id)?;
            if let Some((cost, nodes)) = path {
                // add in cost for reaching the start node and end node
                let start_vec = cgmath::Vector2::from(
                    base_graph
                        .graph
                        .node_weight(*nodes.first().unwrap())
                        .unwrap()
                        .location(),
                );
                let end_vec = cgmath::Vector2::from(
                    base_graph
                        .graph
                        .node_weight(*nodes.last().unwrap())
                        .unwrap()
                        .location(),
                );
                let start_dist = start_vec.distance((start_x as f64, start_y as f64).into())
                    * base_graph.tile_size;
                let start_cost = start_dist / start_mode.linear_speed();
                let end_dist =
                    end_vec.distance((end_x as f64, end_y as f64).into()) * base_graph.tile_size;
                let end_cost = end_dist / end_mode.linear_speed();

                let total_cost = cost + start_cost + end_cost;

                match &fastest {
                    None => {
                        fastest = Some((
                            total_cost, nodes, start_mode, end_mode, start_dist, end_dist,
                        ))
                    }
                    Some((old_cost, _, _, _, _, _)) if total_cost < *old_cost => {
                        fastest = Some((
                            total_cost, nodes, start_mode, end_mode, start_dist, end_dist,
                        ))
                    }
                    _ => (),
                }
            }
        }
        Ok(())
    };

    // TODO: use calc_path_multiple_sources_and_targets instead of invoking calc_path multiple times
    match &input.car_config {
        None => {
            attempt_route(input.start, Mode::Walking, input.end, Mode::Walking)?;
        }
        Some(CarConfig::StartWithCar) => {
            attempt_route(input.start, Mode::Driving, input.end, Mode::Walking)?;
            attempt_route(input.start, Mode::Driving, input.end, Mode::Driving)?;
        }
        Some(CarConfig::CollectParkedCar { address }) => {
            // TODO: basically just merge two routes together
            unimplemented!()
        }
    }

    Ok(
        fastest.and_then(|(cost, path, start_mode, end_mode, start_dist, end_dist)| {
            // only
            (cost < Time::new::<hour>(4).value as f64).then(|| {
                construct_route(
                    &mut base_graph.graph,
                    input,
                    cost,
                    &path,
                    input.start,
                    input.end,
                    start_mode,
                    end_mode,
                    start_dist,
                    end_dist,
                )
            })
        }),
    )
}
