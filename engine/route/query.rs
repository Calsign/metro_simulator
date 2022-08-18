use crate::base_graph::{Graph, InnerGraph, NodeIndex};
use crate::common::{CarConfig, Error, Mode, QueryInput};
use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;

fn perform_query(
    base_graph: &mut InnerGraph,
    start_id: NodeIndex,
    end_id: NodeIndex,
) -> Result<Option<(f64, Vec<NodeIndex>)>, Error> {
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

fn potential_route(
    base_graph: &mut std::cell::RefMut<Graph>,
    start: quadtree::Address,
    end: quadtree::Address,
    start_mode: Mode,
    end_mode: Mode,
) -> Result<Option<PotentialRoute>, Error> {
    use cgmath::MetricSpace;

    let (start_x, start_y) = start.to_xy_f64();
    let (end_x, end_y) = end.to_xy_f64();

    // TODO: pick N nearest terminal nodes and use calc_pat_multiple_sources_and_targets

    // TODO: precompute these values and store in the qtree?
    let start_id = base_graph.terminal_nodes[start_mode].find_nearest(start_x, start_y);
    let end_id = base_graph.terminal_nodes[end_mode].find_nearest(end_x, end_y);

    let potential_route = if let (Some(start_id), Some(end_id)) = (start_id, end_id) {
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
            let start_dist = start_vec.distance((start_x, start_y).into()) * base_graph.tile_size;
            let start_cost = start_dist / start_mode.linear_speed();
            let end_dist = end_vec.distance((end_x, end_y).into()) * base_graph.tile_size;
            let end_cost = end_dist / end_mode.linear_speed();

            let total_cost = cost + start_cost + end_cost;

            Some(PotentialRoute {
                cost: total_cost,
                path: nodes,
                start_mode,
                end_mode,
                start_dist,
                end_dist,
            })
        } else {
            None
        }
    } else {
        None
    };

    // compare with a "direct" route, i.e. a straight line
    let direct_route = if start_mode == end_mode {
        let direct_dist = cgmath::Vector2::from((start_x, start_y)).distance((end_x, end_y).into())
            * base_graph.tile_size;
        if direct_dist < start_mode.bridge_radius() {
            let direct_cost = direct_dist / start_mode.linear_speed();
            Some(PotentialRoute {
                cost: direct_cost,
                path: Vec::new(),
                start_mode,
                end_mode,
                start_dist: direct_dist,
                end_dist: 0.0,
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(fastest_route(
        potential_route.into_iter().chain(direct_route.into_iter()),
    ))
}

struct PotentialRoute {
    cost: f64,
    path: Vec<NodeIndex>,
    start_mode: Mode,
    end_mode: Mode,
    start_dist: f64,
    end_dist: f64,
}

fn construct_route(base_graph: &InnerGraph, input: QueryInput, route: &PotentialRoute) -> Route {
    use itertools::Itertools;
    use std::iter::once;

    let start_pos = input.start.to_xy_f64();
    let end_pos = input.end.to_xy_f64();

    let (extra_start_node, extra_start_edge, real_start_mode) = match route.start_mode {
        Mode::Driving => (
            Some(Node::Parking {
                address: input.start,
            }),
            Some(Edge::ModeTransition {
                from: Mode::Walking,
                to: Mode::Driving,
                address: input.start,
            }),
            Mode::Walking,
        ),
        _ => (None, None, route.start_mode),
    };

    let (extra_end_node, extra_end_edge, real_end_mode) = match route.end_mode {
        Mode::Driving => (
            Some(Node::Parking { address: input.end }),
            Some(Edge::ModeTransition {
                from: Mode::Driving,
                to: Mode::Walking,
                address: input.end,
            }),
            Mode::Walking,
        ),
        _ => (None, None, route.end_mode),
    };

    let start_edge = route
        .path
        .first()
        .map(|first| Edge::ModeSegment {
            mode: route.start_mode,
            distance: route.start_dist,
            start: start_pos,
            stop: base_graph.get_node_map().get(first).unwrap().location(),
        })
        .unwrap_or_else(|| {
            // if we have an empty route, then assume this is a "direct" route, i.e. one with only one edge
            assert_eq!(route.start_mode, route.end_mode);
            Edge::ModeSegment {
                mode: route.start_mode,
                distance: route.start_dist + route.end_dist,
                start: start_pos,
                stop: end_pos,
            }
        });

    let end_edge = route.path.last().map(|last| Edge::ModeSegment {
        mode: route.end_mode,
        distance: route.end_dist,
        start: base_graph.get_node_map().get(last).unwrap().location(),
        stop: end_pos,
    });

    let nodes = once(Node::Endpoint {
        address: input.start,
    })
    .chain(extra_start_node.into_iter())
    .chain(
        route
            .path
            .iter()
            .map(|n| base_graph.node_weight(*n).unwrap().clone()),
    )
    .chain(extra_end_node.into_iter())
    .chain(once(Node::Endpoint { address: input.end }))
    .collect();

    let edges = extra_start_edge
        .into_iter()
        .chain(once(start_edge))
        .chain(route.path.iter().tuple_windows().map(|(a, b)| {
            base_graph
                .edge_weight(base_graph.find_edge(*a, *b).unwrap())
                .unwrap()
                .clone()
        }))
        .chain(end_edge.into_iter())
        .chain(extra_end_edge.into_iter())
        .collect();

    Route::new(
        nodes,
        edges,
        route.cost as f32,
        input,
        real_start_mode,
        real_end_mode,
    )
}

fn fastest_route<I>(potential_routes: I) -> Option<PotentialRoute>
where
    I: std::iter::Iterator<Item = PotentialRoute>,
{
    potential_routes.min_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap())
}

/**
 * Finds the best (lowest cost) route from `start` to `end` in
 * `base_graph`. Returns None if no route could be found.
 *
 * TODO: adjust the construction of the problem so that we can always
 * find a route.
 */
pub fn best_route(
    mut base_graph: std::cell::RefMut<Graph>,
    input: QueryInput,
) -> Result<Option<Route>, Error> {
    Ok(match &input.car_config {
        None => potential_route(
            &mut base_graph,
            input.start,
            input.end,
            Mode::Walking,
            Mode::Walking,
        )?
        .map(|route| construct_route(&base_graph.graph, input, &route)),
        Some(CarConfig::StartWithCar) => fastest_route(
            potential_route(
                &mut base_graph,
                input.start,
                input.end,
                Mode::Driving,
                Mode::Walking,
            )?
            .into_iter()
            .chain(
                potential_route(
                    &mut base_graph,
                    input.start,
                    input.end,
                    Mode::Driving,
                    Mode::Driving,
                )?
                .into_iter(),
            )
            .chain(potential_route(
                &mut base_graph,
                input.start,
                input.end,
                Mode::Walking,
                Mode::Walking,
            )?),
        )
        .map(|route| construct_route(&base_graph.graph, input, &route)),
        Some(CarConfig::CollectParkedCar { address }) => {
            // basically just merge two routes together
            let walking_leg = potential_route(
                &mut base_graph,
                input.start,
                *address,
                Mode::Walking,
                Mode::Walking,
            )?
            .map(|route| {
                construct_route(
                    &base_graph.graph,
                    QueryInput {
                        start: input.start,
                        end: *address,
                        car_config: input.car_config,
                    },
                    &route,
                )
            });
            let driving_leg = potential_route(
                &mut base_graph,
                *address,
                input.end,
                Mode::Driving,
                Mode::Driving,
            )?
            .map(|route| {
                construct_route(
                    &base_graph.graph,
                    QueryInput {
                        start: *address,
                        end: input.end,
                        car_config: input.car_config,
                    },
                    &route,
                )
            });

            walking_leg
                .zip(driving_leg)
                .map(|(walking, driving)| Route::join(walking, driving))
        }
    })
}
