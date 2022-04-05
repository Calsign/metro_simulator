use std::collections::HashMap;

use crate::base_graph::{Graph, InnerGraph, Neighbors};
use crate::common::{Edge, Error, Mode, Node, WorldState};
use crate::route::Route;

/**
 * The querying agent has a car available.
 */
#[derive(Debug, Clone)]
pub enum CarConfig {
    /// departure: user has car available and can park it anywhere, including the destination
    StartWithCar,
    /// return home: user must arrive home with car, and parked it somewhere on the departing trip
    CollectParkedCar { address: quadtree::Address },
}

pub struct QueryInput<'a, 'b> {
    pub base_graph: &'a mut Graph,
    pub start: quadtree::Address,
    pub end: quadtree::Address,
    pub state: &'b WorldState,
    pub car_config: Option<CarConfig>,
}

/**
 * A wrapper around graph that removes added nodes and edges when dropped.
 * Tracking added nodes and edges must be performed by the user.
 */
pub struct AugmentedGraph<'a> {
    pub graph: &'a mut InnerGraph,
    new_nodes: Vec<petgraph::graph::NodeIndex>,
    new_edges: Vec<petgraph::graph::EdgeIndex>,
    base_nodes: usize,
    base_edges: usize,
}

impl<'a> AugmentedGraph<'a> {
    fn new(graph: &'a mut InnerGraph) -> Self {
        Self {
            base_nodes: graph.node_count(),
            base_edges: graph.edge_count(),
            graph,
            new_nodes: Vec::new(),
            new_edges: Vec::new(),
        }
    }
}

impl<'a> Drop for AugmentedGraph<'a> {
    fn drop(&mut self) {
        // NOTE: iterating in reverse order should unwind the graph in
        // such a way that no index swapping needs to take place.

        // NOTE: remove edges first, since removing the nodes causes the edges to be removed.
        for edge in self.new_edges.iter().rev() {
            self.graph.remove_edge(*edge).unwrap();
        }
        for node in self.new_nodes.iter().rev() {
            self.graph.remove_node(*node).unwrap();
        }

        // make sure we have removed all of the nodes and edges
        assert_eq!(self.graph.node_count(), self.base_nodes);
        assert_eq!(self.graph.edge_count(), self.base_edges);
    }
}

struct AddEdgesVisitor<'a> {
    graph: &'a mut InnerGraph,
    base: petgraph::graph::NodeIndex,
    tile_size: f64,
    mode: Mode,
    new_edges: Vec<petgraph::graph::EdgeIndex>,
    direction: petgraph::Direction,
}

impl<'a> quadtree::NeighborsVisitor<petgraph::graph::NodeIndex, Error> for AddEdgesVisitor<'a> {
    fn visit(
        &mut self,
        entry: &petgraph::graph::NodeIndex,
        x: f64,
        y: f64,
        distance: f64,
    ) -> Result<(), Error> {
        let (first, second) = match self.direction {
            petgraph::Direction::Outgoing => (self.base, *entry),
            petgraph::Direction::Incoming => (*entry, self.base),
        };
        let edge_id = self.graph.add_edge(
            first,
            second,
            Edge::ModeSegment {
                mode: self.mode,
                distance: distance * self.tile_size,
            },
        );
        self.new_edges.push(edge_id);
        Ok(())
    }
}

fn augment_node(
    graph: &mut AugmentedGraph,
    tile_size: f64,
    neighbors: &Neighbors,
    node: Node,
    directions: HashMap<petgraph::Direction, Vec<Mode>>,
) -> Result<petgraph::graph::NodeIndex, Error> {
    let inner = &mut graph.graph;

    let (x, y) = node.location();

    // add node
    let node_id = inner.add_node(node);
    graph.new_nodes.push(node_id);

    for (direction, modes) in directions.iter() {
        for mode in modes {
            let radius = mode.max_radius() / tile_size;

            // add edges
            let mut visitor = AddEdgesVisitor {
                graph: inner,
                base: node_id,
                tile_size,
                mode: *mode,
                new_edges: Vec::new(),
                direction: *direction,
            };
            neighbors[&mode].visit_radius(&mut visitor, x, y, radius)?;
            graph.new_edges.append(&mut visitor.new_edges);
        }
    }

    Ok(node_id)
}

/**
 * Augment `graph` with a start node, end node, and walking edges
 * from each to the nodes in the base graph.
 */
pub fn augment_base_graph(
    base_graph: &mut Graph,
    start: quadtree::Address,
    end: quadtree::Address,
    car_config: Option<CarConfig>,
) -> Result<
    (
        AugmentedGraph,
        petgraph::graph::NodeIndex,
        petgraph::graph::NodeIndex,
    ),
    Error,
> {
    let mut graph = AugmentedGraph::new(&mut base_graph.graph);

    // add start and end nodes, and edges connecting them
    let start_node = augment_node(
        &mut graph,
        base_graph.tile_size,
        &base_graph.neighbors,
        Node::StartNode { address: start },
        HashMap::from([(
            petgraph::Direction::Outgoing,
            match car_config {
                Some(CarConfig::StartWithCar) => vec![Mode::Walking, Mode::Driving],
                _ => vec![Mode::Walking],
            },
        )]),
    )?;
    let end_node = augment_node(
        &mut graph,
        base_graph.tile_size,
        &base_graph.neighbors,
        Node::EndNode { address: end },
        HashMap::from([(
            petgraph::Direction::Incoming,
            match car_config {
                Some(CarConfig::StartWithCar) => vec![Mode::Walking, Mode::Driving],
                Some(CarConfig::CollectParkedCar { .. }) => vec![Mode::Driving],
                None => vec![Mode::Walking],
            },
        )]),
    )?;

    match car_config {
        Some(CarConfig::StartWithCar) => {
            // add driving->walking edges at all of the known parking locations
            for parking in base_graph.parking.values() {
                let edge_id = graph.graph.add_edge(
                    parking.driving_node,
                    parking.walking_node,
                    Edge::ModeTransition {
                        from: Mode::Driving,
                        to: Mode::Walking,
                    },
                );
                graph.new_edges.push(edge_id);
            }
        }
        Some(CarConfig::CollectParkedCar { address }) => {
            // add walking->driving edge where the car was parked
            match base_graph.parking.get(&address) {
                Some(parking) => {
                    let edge_id = graph.graph.add_edge(
                        parking.walking_node,
                        parking.driving_node,
                        Edge::ModeTransition {
                            from: Mode::Walking,
                            to: Mode::Driving,
                        },
                    );
                    graph.new_edges.push(edge_id);
                }
                None => return Err(Error::ParkingNotFound(address)),
            }
        }
        None => (),
    }

    Ok((graph, start_node, end_node))
}

/**
 * Finds the best (lowest cost) route from `start` to `end` in
 * `base_graph`. Returns None if no route could be found.
 *
 * TODO: adjust the construction of the problem so that we can always
 * find a route.
 */
pub fn best_route<'a, 'b>(input: QueryInput<'a, 'b>) -> Result<Option<Route>, Error> {
    use cgmath::MetricSpace;
    use cgmath::Vector2;
    use itertools::Itertools;

    let (graph, start_index, end_index) =
        augment_base_graph(input.base_graph, input.start, input.end, input.car_config)?;
    let inner = &graph.graph;

    let goal_vec = Vector2::from(inner.node_weight(end_index).unwrap().location());

    let is_goal = |n| match inner.node_weight(n).unwrap() {
        Node::EndNode { .. } => true,
        _ => false,
    };
    let edge_cost = |e: petgraph::graph::EdgeReference<Edge>| e.weight().cost(input.state);

    // This should be the fastest possible speed by any mode of transportation.
    // TODO: There is probably a more principled way to approach this.
    let top_speed = metro::timing::MAX_SPEED;
    let estimate_cost =
        |n| goal_vec.distance(inner.node_weight(n).unwrap().location().into()) / top_speed;

    Ok(
        match petgraph::algo::astar(&**inner, start_index, is_goal, edge_cost, estimate_cost) {
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
            )),
            None => None,
        },
    )
}
