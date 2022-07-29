use std::collections::{HashMap, HashSet};

use crate::common::{Error, Mode, ModeMap, MODES};
use crate::edge::Edge;
use crate::fast_graph_wrapper::FastGraphWrapper;
use crate::node::Node;

pub struct BaseGraphInput<'a, F: state::Fields> {
    pub state: &'a state::State<F>,

    pub filter_metro_lines: Option<HashSet<u64>>,
    pub filter_highway_segments: Option<HashSet<u64>>,

    pub add_inferred_edges: bool,
    pub validate_highways: bool,
}

pub type InnerGraph = FastGraphWrapper;
pub type NodeIndex = fast_paths::NodeId;

/**
 * We construct a pair of nodes for each parking area in the base
 * graph. The nodes are not connected in the base graph. Edges are
 * added as needed during graph augmentation.
 *
 * It is not correct in general to add edges between the parking and
 * driving modes; for example, if we added a bidirectional edge
 * between the pair of nodes for each parking area, then the route
 * planner would allow creating new cars out of thin air.
 */
#[derive(Debug, Clone)]
pub struct Parking {
    pub address: quadtree::Address,
    pub walking_node: NodeIndex,
    pub driving_node: NodeIndex,
}

#[derive(Debug, Clone)]
pub struct BaseGraphStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub terminal_node_counts: HashMap<Mode, usize>,
}

impl Default for BaseGraphStats {
    fn default() -> Self {
        Self {
            node_count: 0,
            edge_count: 0,
            terminal_node_counts: MODES.iter().map(|mode| (*mode, 0)).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub graph: FastGraphWrapper,
    pub terminal_nodes: Neighbors,
    pub parking: HashMap<quadtree::Address, Parking>,
    pub tile_size: f64,
    pub max_depth: u32,
}

impl Graph {
    pub fn get_stats(&self) -> BaseGraphStats {
        BaseGraphStats {
            node_count: self.graph.node_count(),
            edge_count: self.graph.edge_count(),
            terminal_node_counts: MODES
                .iter()
                .map(|mode| (*mode, self.terminal_nodes[*mode].count()))
                .collect(),
        }
    }
}

pub fn dump_graph<W>(_graph: &InnerGraph, _write: &mut W) -> Result<(), std::io::Error>
where
    W: std::io::Write,
{
    // TODO: Implement dumping to dot format. This is implemented for petgraph, but after switching
    // to fast_paths we no long have a provided implementation
    unimplemented!();
}

// put this into a mod so that we can't construct TriangulationVertex directly, for safety
mod triangulation_ext {
    use crate::base_graph::NodeIndex;
    use crate::common::Error;

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct TriangulationVertex {
        index: NodeIndex,
        x: f64,
        y: f64,
    }

    impl TriangulationVertex {
        pub fn index(&self) -> NodeIndex {
            self.index
        }

        pub fn coords(&self) -> (f64, f64) {
            (self.x, self.y)
        }
    }

    impl spade::HasPosition for TriangulationVertex {
        type Scalar = f64;
        fn position(&self) -> spade::Point2<f64> {
            spade::Point2::new(self.x, self.y)
        }
    }

    pub(crate) trait SafeTriangulationInsert {
        /**
         * By default, inserting a vertex at the position of an existing vertex replaces the
         * existing vertex. We do not want this! Instead, return an error in this case.
         */
        fn safe_insert(
            &mut self,
            node: NodeIndex,
            x: f64,
            y: f64,
        ) -> Result<spade::handles::FixedVertexHandle, Error>;
    }

    impl SafeTriangulationInsert for spade::DelaunayTriangulation<TriangulationVertex> {
        fn safe_insert(
            &mut self,
            index: NodeIndex,
            x: f64,
            y: f64,
        ) -> Result<spade::handles::FixedVertexHandle, Error> {
            use spade::HasPosition;
            use spade::Triangulation;

            let new_vertex = TriangulationVertex { index, x, y };
            if let Some(existing_vertex) = self.locate_vertex(new_vertex.position()) {
                // We could do an error instead, but I don't want to risk letting this slip through.
                panic!(
                    "Attempted to insert vertex {:?}, but existing vertex {:?} was found at that location!",
                    new_vertex, existing_vertex
                );
            } else {
                Ok(self.insert(new_vertex)?)
            }
        }
    }
}

type Neighbors = ModeMap<quadtree::NeighborsStore<NodeIndex>>;
type Triangulations = ModeMap<spade::DelaunayTriangulation<triangulation_ext::TriangulationVertex>>;

pub fn construct_base_graph<F: state::Fields>(
    input: BaseGraphInput<'_, F>,
) -> Result<Graph, Error> {
    use itertools::Itertools;
    use spade::Triangulation;
    use triangulation_ext::SafeTriangulationInsert;

    let tile_size = input.state.config.min_tile_size as f64;

    if input.validate_highways {
        input.state.highways.validate();
    }

    let mut graph = InnerGraph::new();
    // nodes from which routes can start and end
    let mut terminal_nodes =
        ModeMap::new(|_| quadtree::NeighborsStore::new(4, input.state.config.max_depth));
    // we use a Delaunay triangulation to infer edges based on proximity
    let mut inference_triangulation = ModeMap::new(|_| spade::DelaunayTriangulation::new());
    let mut parking = HashMap::new();

    let mut add_parking = |address,
                           graph: &mut InnerGraph,
                           terminal_nodes: &mut Neighbors,
                           inference_triangulation: &mut Triangulations|
     -> Result<(NodeIndex, NodeIndex), Error> {
        let walking_node = graph.add_node(Node::Parking { address });
        let driving_node = graph.add_node(Node::Parking { address });
        let (x, y) = address.to_xy_f64();

        // NOTE: Important that we don't add the driving node to the terminal nodes.
        // This would be invalid since it intentionally has no outgoing edges.
        terminal_nodes[Mode::Walking].insert(walking_node, x, y)?;

        inference_triangulation[Mode::Walking].safe_insert(walking_node, x, y)?;
        inference_triangulation[Mode::Driving].safe_insert(driving_node, x, y)?;

        parking.insert(
            address,
            Parking {
                address,
                walking_node,
                driving_node,
            },
        );
        graph.add_edge(
            driving_node,
            walking_node,
            Edge::ModeTransition {
                from: Mode::Driving,
                to: Mode::Walking,
                address,
            },
            input.state,
        );

        Ok((walking_node, driving_node))
    };

    let mut station_map = HashMap::new();
    for metro_line in input.state.metro_lines.values() {
        if let Some(ref filter) = input.filter_metro_lines {
            if !filter.contains(&metro_line.id) {
                continue;
            }
            // NOTE: print to stderr so that we can pipe dump output to xdot
            eprintln!(
                "Filtering metro lines selected {}: {}",
                &metro_line.id, &metro_line.name
            );
        }

        let mut stop_map = HashMap::new();

        let speed_keys = metro::timing::speed_keys(
            metro_line.get_keys(),
            tile_size,
            metro_line.speed_limit as f64,
        );
        let timetable = metro::timing::timetable(&speed_keys);
        for (station, _) in timetable.iter() {
            let station_id = *station_map.entry(station.address).or_insert_with(|| {
                let station_id = graph.add_node(Node::MetroStation {
                    station: station.clone(),
                });

                // for now, we assume that every station offers parking
                let (parking_walking, _) = add_parking(
                    station.address,
                    &mut graph,
                    &mut terminal_nodes,
                    &mut inference_triangulation,
                )
                .unwrap();

                let location = station.address.to_xy_f64();

                // NOTE: can't put this node into the inference triangulation because it
                // occupies the same point as the parking node.
                graph.add_edge(
                    station_id,
                    parking_walking,
                    Edge::ModeSegment {
                        mode: Mode::Walking,
                        distance: 0.0,
                        start: location,
                        stop: location,
                    },
                    input.state,
                );
                graph.add_edge(
                    parking_walking,
                    station_id,
                    Edge::ModeSegment {
                        mode: Mode::Walking,
                        distance: 0.0,
                        start: location,
                        stop: location,
                    },
                    input.state,
                );

                station_id
            });

            let stop_id = graph.add_node(Node::MetroStop {
                station: station.clone(),
                metro_line: metro_line.id,
            });
            stop_map.insert(station.address, stop_id);

            graph.add_edge(
                station_id,
                stop_id,
                Edge::MetroEmbark {
                    metro_line: metro_line.id,
                    station: station.clone(),
                },
                input.state,
            );

            graph.add_edge(
                stop_id,
                station_id,
                Edge::MetroDisembark {
                    metro_line: metro_line.id,
                    station: station.clone(),
                },
                input.state,
            );
        }
        for ((left, left_t), (right, right_t)) in timetable.iter().tuple_windows() {
            graph.add_edge(
                stop_map[&left.address],
                stop_map[&right.address],
                Edge::MetroSegment {
                    metro_line: metro_line.id,
                    time: right_t - left_t,
                    start: left.address,
                    stop: right.address,
                },
                input.state,
            );
        }
    }

    let mut junction_map = HashMap::new();
    let mut segment_map = HashMap::new();

    for junction in input.state.highways.get_junctions().values() {
        // TODO: filter on junctions

        let (x, y) = junction.location;
        let address = quadtree::Address::from_xy(x as u64, y as u64, input.state.config.max_depth);
        let node_id = if let Some(ramp) = &junction.ramp {
            let outer_id = graph.add_node(Node::HighwayRamp {
                junction: junction.id,
                position: (x, y),
                address,
            });
            let inner_id = graph.add_node(Node::HighwayRamp {
                junction: junction.id,
                position: (x, y),
                address,
            });
            let (first, second) = match ramp {
                highway::RampDirection::OnRamp => (outer_id, inner_id),
                highway::RampDirection::OffRamp => (inner_id, outer_id),
            };
            graph.add_edge(
                first,
                second,
                Edge::HighwayRamp { position: (x, y) },
                input.state,
            );
            terminal_nodes[Mode::Driving].insert(outer_id, x, y)?;
            inference_triangulation[Mode::Driving].safe_insert(outer_id, x, y)?;
            inner_id
        } else {
            graph.add_node(Node::HighwayJunction {
                junction: junction.id,
                position: (x, y),
                address,
            })
        };

        junction_map.insert(junction.id, node_id);
    }

    for segment in input.state.highways.get_segments().values() {
        if let Some(ref filter) = input.filter_highway_segments {
            if !filter.contains(&segment.id) {
                continue;
            }
            // NOTE: print to stderr so that we can pipe dump output to xdot
            eprintln!(
                "Filtering highway segments selected segment {}: {:?}",
                &segment.id, &segment.data
            );
        }

        let edge_id = graph.add_edge(
            *junction_map
                .get(&segment.start_junction())
                .expect("missing start junction"),
            *junction_map
                .get(&segment.end_junction())
                .expect("missing end junction"),
            Edge::Highway {
                segment: segment.id,
                data: segment.data.clone(),
                time: segment.travel_time(tile_size),
            },
            input.state,
        );

        segment_map.insert(segment.id, edge_id);
    }

    if input.add_inferred_edges {
        for mode in MODES {
            let max_radius_sq = (mode.bridge_radius() / tile_size).powi(2);
            // TODO: use bulk_load instead of a bunch of individual insertions
            for edge in inference_triangulation[*mode].undirected_edges() {
                if edge.length_2() <= max_radius_sq {
                    let [a, b] = edge.vertices();
                    for (start, end) in [(a, b), (b, a)] {
                        let start = start.data();
                        let end = end.data();

                        graph.add_edge(
                            start.index(),
                            end.index(),
                            Edge::ModeSegment {
                                mode: *mode,
                                distance: edge.length_2().sqrt() * tile_size,
                                start: start.coords(),
                                stop: start.coords(),
                            },
                            input.state,
                        );
                    }
                }
            }
        }
    }

    graph.prepare();

    Ok(Graph {
        graph,
        terminal_nodes,
        parking,
        tile_size,
        max_depth: input.state.config.max_depth,
    })
}

#[cfg(test)]
mod highway_tests {
    use crate::base_graph::*;
    use highway::*;

    #[derive(derive_more::Constructor)]
    struct JunctionData {
        location: (f64, f64),
    }

    // used only for tests
    #[derive(derive_more::Constructor)]
    struct SegmentData {
        start: u64,
        end: u64,
    }

    #[derive(Debug, Default, Clone)]
    struct DummyFields {}

    impl state::Fields for DummyFields {}

    fn setup_problem(junctions: Vec<JunctionData>, segments: Vec<SegmentData>) -> Graph {
        let data = HighwayData {
            name: None,
            refs: vec![],
            lanes: None,
            speed_limit: Some(1), // easy math
        };

        let mut state: state::State<DummyFields> = state::State::new(state::Config {
            max_depth: 5,
            people_per_sim: 1,
            min_tile_size: 1,
        });

        for junction in &junctions {
            state.highways.add_junction(junction.location, None);
        }
        for segment in &segments {
            state.highways.add_segment(
                data.clone(),
                segment.start,
                segment.end,
                Some(vec![
                    junctions[segment.start as usize].location.into(),
                    junctions[segment.end as usize].location.into(),
                ]),
            );
        }

        let input = BaseGraphInput {
            state: &state,
            filter_metro_lines: None,
            filter_highway_segments: None,
            add_inferred_edges: false,
            validate_highways: true,
        };

        construct_base_graph(input).unwrap()
    }

    #[test]
    fn empty() {
        let graph = setup_problem(vec![], vec![]).graph;
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn one() {
        let graph = setup_problem(
            vec![JunctionData::new((0.0, 0.0)), JunctionData::new((1.0, 0.0))],
            vec![SegmentData::new(0, 1)],
        )
        .graph;
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn simple() {
        let graph = setup_problem(
            vec![
                JunctionData::new((0.0, 0.0)),
                JunctionData::new((1.0, 0.0)),
                JunctionData::new((2.0, 0.0)),
            ],
            vec![SegmentData::new(0, 1), SegmentData::new(1, 2)],
        )
        .graph;
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn chain() {
        let graph = setup_problem(
            vec![
                JunctionData::new((0.0, 0.0)),
                JunctionData::new((1.0, 0.0)),
                JunctionData::new((2.0, 0.0)),
                JunctionData::new((3.0, 0.0)),
                JunctionData::new((4.0, 0.0)),
                JunctionData::new((5.0, 0.0)),
                JunctionData::new((6.0, 0.0)),
            ],
            vec![
                SegmentData::new(0, 1),
                SegmentData::new(1, 2),
                SegmentData::new(2, 3),
                SegmentData::new(3, 4),
                SegmentData::new(4, 5),
                SegmentData::new(5, 6),
            ],
        )
        .graph;
        assert_eq!(graph.node_count(), 7);
        assert_eq!(graph.edge_count(), 6);
    }

    #[test]
    fn branching() {
        let graph = setup_problem(
            vec![
                JunctionData::new((0.0, 0.0)),
                JunctionData::new((1.0, 0.0)),
                JunctionData::new((2.0, 1.0)),
                JunctionData::new((2.0, 2.0)),
                JunctionData::new((3.0, 0.0)),
                JunctionData::new((4.0, 0.0)),
            ],
            vec![
                SegmentData::new(0, 1),
                SegmentData::new(1, 2),
                SegmentData::new(1, 3),
                SegmentData::new(2, 4),
                SegmentData::new(3, 4),
                SegmentData::new(4, 5),
            ],
        )
        .graph;
        assert_eq!(graph.node_count(), 6);
        assert_eq!(graph.edge_count(), 6);
        // TODO: it would be great to verify the actual structure of the graphs.
    }
}
