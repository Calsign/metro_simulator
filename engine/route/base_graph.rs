use std::collections::{HashMap, HashSet};

use crate::common::{Edge, Error, Mode, Node, MODES};

pub struct BaseGraphInput<'a, 'b> {
    pub metro_lines: &'a HashMap<u64, metro::MetroLine>,
    pub highways: &'b highway::Highways,
    pub tile_size: f64,
    pub max_depth: u32,

    pub filter_metro_lines: Option<HashSet<u64>>,
    pub filter_highway_segments: Option<HashSet<u64>>,

    pub add_inferred_edges: bool,
    pub validate_highways: bool,
}

pub type InnerGraph = petgraph::Graph<Node, Edge>;

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
pub struct Parking {
    pub address: quadtree::Address,
    pub walking_node: petgraph::graph::NodeIndex,
    pub driving_node: petgraph::graph::NodeIndex,
}

pub type Neighbors = HashMap<Mode, quadtree::NeighborsStore<petgraph::graph::NodeIndex>>;

pub struct Graph {
    pub graph: InnerGraph,
    pub neighbors: Neighbors,
    pub parking: HashMap<quadtree::Address, Parking>,
    pub tile_size: f64,
    pub max_depth: u32,
}

pub fn dump_graph<W>(
    graph: &petgraph::Graph<Node, Edge>,
    write: &mut W,
) -> Result<(), std::io::Error>
where
    W: std::io::Write,
{
    let dot = petgraph::dot::Dot::new(graph);
    write!(write, "{}", dot)?;
    Ok(())
}

pub fn construct_base_graph<'a, 'b>(input: BaseGraphInput<'a, 'b>) -> Result<Graph, Error> {
    use itertools::Itertools;

    if input.validate_highways {
        input.highways.validate();
    }

    let mut graph = InnerGraph::new();
    let mut neighbors = HashMap::new();
    for mode in MODES {
        neighbors.insert(*mode, quadtree::NeighborsStore::new(4, input.max_depth));
    }
    let mut parking = HashMap::new();

    let mut add_parking = |address, graph: &mut InnerGraph, neighbors: &mut Neighbors| {
        let walking_node = graph.add_node(Node::Parking { address });
        let driving_node = graph.add_node(Node::Parking { address });
        let (x, y) = address.to_xy();
        let (x, y) = (x as f64, y as f64);
        neighbors
            .get_mut(&Mode::Walking)
            .unwrap()
            .insert(walking_node, x, y)
            .unwrap();
        neighbors
            .get_mut(&Mode::Driving)
            .unwrap()
            .insert(driving_node, x, y)
            .unwrap();
        parking.insert(
            address,
            Parking {
                address,
                walking_node,
                driving_node,
            },
        );
    };

    let mut station_map = HashMap::new();
    for metro_line in input.metro_lines.values() {
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

        let speed_keys = metro::timing::speed_keys(metro_line.get_keys(), input.tile_size);
        let timetable = metro::timing::timetable(&speed_keys);
        for (station, _) in timetable.iter() {
            let station_id = *station_map
                .entry(station.address.clone())
                .or_insert_with(|| {
                    let station_id = graph.add_node(Node::MetroStation {
                        station: station.clone(),
                    });

                    let (x, y) = station.address.to_xy();
                    neighbors
                        .get_mut(&Mode::Walking)
                        .unwrap()
                        .insert(station_id, x as f64, y as f64)
                        .unwrap();

                    // for now, we assume that every station offers parking
                    add_parking(station.address, &mut graph, &mut neighbors);

                    station_id
                });

            let stop_id = graph.add_node(Node::MetroStop {
                station: station.clone(),
                metro_line: metro_line.id,
            });
            stop_map.insert(station.address.clone(), stop_id);

            graph.add_edge(
                station_id,
                stop_id,
                Edge::MetroEmbark {
                    metro_line: metro_line.id,
                    station: station.clone(),
                },
            );

            graph.add_edge(
                stop_id,
                station_id,
                Edge::MetroDisembark {
                    metro_line: metro_line.id,
                    station: station.clone(),
                },
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
            );
        }
    }

    let mut junction_map = HashMap::new();
    let mut segment_map = HashMap::new();

    for junction in input.highways.get_junctions().values() {
        if let Some(ref filter) = input.filter_highway_segments {
            // TODO: filter on junctions
            // NOTE: print to stderr so that we can pipe dump output to xdot
            eprintln!(
                "Filtering highway segments selected junction {}",
                &junction.id
            );
        }

        let (x, y) = junction.location;
        let address = quadtree::Address::from_xy(x as u64, y as u64, input.max_depth);
        let node_id = if let Some(ramp) = &junction.ramp {
            let outer_id = graph.add_node(Node::HighwayRamp {
                position: (x, y),
                address,
            });
            let inner_id = graph.add_node(Node::HighwayRamp {
                position: (x, y),
                address,
            });
            let (first, second) = match ramp {
                highway::RampDirection::OnRamp => (outer_id, inner_id),
                highway::RampDirection::OffRamp => (inner_id, outer_id),
            };
            graph.add_edge(first, second, Edge::HighwayRamp { position: (x, y) });
            neighbors
                .get_mut(&Mode::Driving)
                .unwrap()
                .insert(outer_id, x, y)
                .unwrap();
            inner_id
        } else {
            graph.add_node(Node::HighwayJunction {
                position: (x, y),
                address,
            })
        };

        junction_map.insert(junction.id, node_id);
    }

    for segment in input.highways.get_segments().values() {
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
                time: highway::timing::travel_time(segment, input.tile_size),
            },
        );

        segment_map.insert(segment.id, edge_id);
    }

    if input.add_inferred_edges {
        // TODO: cost to walk/drive should depend on the local density.
        // For example, it should take longer to drive across San Francisco
        // than across Palo Alto.
        // But maybe this should just be based on local traffic?

        for mode in MODES {
            neighbors[mode].visit_all_radius(
                &mut AddEdgesVisitor {
                    graph: &mut graph,
                    tile_size: input.tile_size,
                    mode: *mode,
                },
                |_| mode.bridge_radius() / input.tile_size,
            )?;
        }
    }

    Ok(Graph {
        graph,
        neighbors,
        parking,
        tile_size: input.tile_size,
        max_depth: input.max_depth,
    })
}

struct AddEdgesVisitor<'a> {
    graph: &'a mut InnerGraph,
    tile_size: f64,
    mode: Mode,
}

impl<'a> quadtree::AllNeighborsVisitor<petgraph::graph::NodeIndex, Error> for AddEdgesVisitor<'a> {
    fn visit(
        &mut self,
        base: &petgraph::graph::NodeIndex,
        entry: &petgraph::graph::NodeIndex,
        distance: f64,
    ) -> Result<(), Error> {
        if base != entry {
            self.graph.add_edge(
                *base,
                *entry,
                Edge::ModeSegment {
                    mode: self.mode,
                    distance: distance * self.tile_size,
                },
            );
        }
        Ok(())
    }
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

    fn setup_problem(junctions: Vec<JunctionData>, segments: Vec<SegmentData>) -> Graph {
        let data = HighwayData {
            name: None,
            refs: vec![],
            lanes: None,
            speed_limit: Some(1), // easy math
        };

        let metro_lines = HashMap::new();
        let mut highways = Highways::new();
        for junction in &junctions {
            highways.add_junction(junction.location, None);
        }
        for segment in &segments {
            highways.add_segment(
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
            metro_lines: &metro_lines,
            highways: &highways,
            tile_size: 1.0, // easy math
            max_depth: 5,
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
