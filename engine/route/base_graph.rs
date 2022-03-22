use std::collections::{HashMap, HashSet};

use crate::common::{Edge, Error, Mode, Node, MODES};

pub struct BaseGraphInput<'a, 'b, M, H>
where
    M: Iterator<Item = &'a metro::MetroLine>,
    H: Clone + Iterator<Item = &'b highway::HighwaySegment>,
{
    pub metro_lines: M,
    pub highway_segments: H,
    pub tile_size: f64,
    pub max_depth: u32,

    pub filter_metro_lines: Option<HashSet<u64>>,
    pub filter_highway_segments: Option<HashSet<u64>>,

    pub add_inferred_edges: bool,
    pub validate_highways: bool,
}

pub type InnerGraph = petgraph::Graph<Node, Edge>;

pub struct Graph {
    pub graph: InnerGraph,
    pub neighbors: HashMap<Mode, quadtree::NeighborsStore<petgraph::graph::NodeIndex>>,
    pub tile_size: f64,
    pub max_depth: u32,
}

impl Graph {
    pub fn dump<W>(&self, write: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        let dot = petgraph::dot::Dot::new(&self.graph);
        write!(write, "{}", dot)?;
        Ok(())
    }
}

pub fn construct_base_graph<'a, 'b, M, H>(
    input: BaseGraphInput<'a, 'b, M, H>,
) -> Result<Graph, Error>
where
    M: Iterator<Item = &'a metro::MetroLine>,
    H: Clone + Iterator<Item = &'b highway::HighwaySegment>,
{
    use itertools::Itertools;

    if input.validate_highways {
        highway::validate::validate_highway_segments(input.highway_segments.clone());
    }

    let mut graph = InnerGraph::new();
    let mut neighbors = HashMap::new();
    for mode in MODES {
        neighbors.insert(*mode, quadtree::NeighborsStore::new(4, input.max_depth));
    }

    let mut station_map = HashMap::new();
    for metro_line in input.metro_lines {
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
                    time: right_t - left_t,
                },
            );
        }
    }

    let mut segment_in_map = HashMap::new();
    let mut segment_out_map = HashMap::new();
    for highway_segment in input.highway_segments {
        if let Some(ref filter) = input.filter_highway_segments {
            if !filter.contains(&highway_segment.id) {
                continue;
            }
            // NOTE: print to stderr so that we can pipe dump output to xdot
            eprintln!(
                "Filtering highway segments selected {}: {:?}",
                &highway_segment.id, &highway_segment.data
            );
        }

        // find the existing end node, or create one if needed
        let end_id = highway_segment
            .succ()
            .iter()
            .find_map(|succ| segment_in_map.get(succ).map(|node_id| *node_id))
            .unwrap_or_else(|| {
                let vec = highway_segment
                    .get_keys()
                    .last()
                    .expect("empty highway segment");
                let address =
                    quadtree::Address::from_xy(vec.x as u64, vec.y as u64, input.max_depth);
                let node_id = graph.add_node(Node::HighwayJunction {
                    position: (vec.x, vec.y),
                    address,
                });
                neighbors
                    .get_mut(&Mode::Driving)
                    .unwrap()
                    .insert(node_id, vec.x, vec.y)
                    .unwrap();
                // update the successors so that other segments with the same
                // successors can find the same node
                for succ in highway_segment.succ() {
                    segment_in_map.insert(succ, node_id);
                }
                // update this segment so that other segments that have this
                // segment as a predecessor can find the same node
                segment_out_map.insert(highway_segment.id, node_id);
                node_id
            });

        // find the existing start node, or create one if needed
        let start_id = highway_segment
            .pred()
            .iter()
            .find_map(|pred| segment_out_map.get(pred).map(|node_id| *node_id))
            .unwrap_or_else(|| {
                let vec = highway_segment
                    .get_keys()
                    .first()
                    .expect("empty highway segment");
                let address =
                    quadtree::Address::from_xy(vec.x as u64, vec.y as u64, input.max_depth);
                let node_id = graph.add_node(Node::HighwayJunction {
                    position: (vec.x, vec.y),
                    address,
                });
                neighbors
                    .get_mut(&Mode::Driving)
                    .unwrap()
                    .insert(node_id, vec.x, vec.y)
                    .unwrap();
                // see above
                for pred in highway_segment.pred() {
                    segment_out_map.insert(*pred, node_id);
                }
                segment_in_map.insert(&highway_segment.id, node_id);
                node_id
            });

        graph.add_edge(
            start_id,
            end_id,
            Edge::Highway {
                segment: highway_segment.id,
                time: highway::timing::travel_time(highway_segment, input.tile_size),
            },
        );
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
                |_| mode.max_radius() / input.tile_size,
            )?;
        }
    }

    Ok(Graph {
        graph,
        neighbors,
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
    use crate::{base_graph::*, common::*};
    use highway::*;

    // used only for tests
    #[derive(derive_more::Constructor)]
    struct SegmentData {
        id: u64,
        start: (f64, f64),
        end: (f64, f64),
        pred: Vec<u64>,
        succ: Vec<u64>,
    }

    fn setup_problem(segments: Vec<SegmentData>) -> Graph {
        let data = HighwayData {
            name: None,
            refs: vec![],
            lanes: None,
            speed_limit: Some(1), // easy math
        };

        let metro_lines = vec![];
        let highway_segments: Vec<HighwaySegment> = segments
            .iter()
            .map(|segment_data| {
                let mut segment = HighwaySegment::new(
                    segment_data.id,
                    data.clone(),
                    segment_data.pred.clone(),
                    segment_data.succ.clone(),
                );
                segment.set_keys(vec![segment_data.start.into(), segment_data.end.into()]);
                segment
            })
            .collect();

        let input = BaseGraphInput {
            metro_lines: metro_lines.iter(),
            highway_segments: highway_segments.iter(),
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
        let graph = setup_problem(vec![]).graph;
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn one() {
        let graph = setup_problem(vec![SegmentData::new(
            0,
            (0.0, 0.0),
            (1.0, 0.0),
            vec![],
            vec![],
        )])
        .graph;
        assert_eq!(graph.node_count(), 2);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn simple() {
        let graph = setup_problem(vec![
            SegmentData::new(0, (0.0, 0.0), (1.0, 0.0), vec![], vec![1]),
            SegmentData::new(1, (1.0, 0.0), (2.0, 0.0), vec![0], vec![]),
        ])
        .graph;
        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.edge_count(), 2);
    }

    #[test]
    fn chain() {
        let graph = setup_problem(vec![
            SegmentData::new(0, (0.0, 0.0), (1.0, 0.0), vec![], vec![1]),
            SegmentData::new(1, (1.0, 0.0), (2.0, 0.0), vec![0], vec![2]),
            SegmentData::new(2, (2.0, 0.0), (3.0, 0.0), vec![1], vec![3]),
            SegmentData::new(3, (3.0, 0.0), (4.0, 0.0), vec![2], vec![4]),
            SegmentData::new(4, (4.0, 0.0), (5.0, 0.0), vec![3], vec![5]),
            SegmentData::new(5, (5.0, 0.0), (6.0, 0.0), vec![4], vec![]),
        ])
        .graph;
        assert_eq!(graph.node_count(), 7);
        assert_eq!(graph.edge_count(), 6);
    }

    #[test]
    fn branching() {
        let graph = setup_problem(vec![
            SegmentData::new(0, (0.0, 0.0), (1.0, 0.0), vec![], vec![1, 3]),
            SegmentData::new(1, (1.0, 0.0), (2.0, 1.0), vec![0], vec![2]),
            SegmentData::new(2, (2.0, 1.0), (3.0, 0.0), vec![1], vec![5]),
            SegmentData::new(3, (1.0, 0.0), (2.0, -1.0), vec![0], vec![4]),
            SegmentData::new(4, (2.0, -1.0), (3.0, 0.0), vec![3], vec![5]),
            SegmentData::new(5, (3.0, 0.0), (4.0, 0.0), vec![2, 4], vec![]),
        ])
        .graph;
        assert_eq!(graph.node_count(), 6);
        assert_eq!(graph.edge_count(), 6);
        // TODO: it would be great to verify the actual structure of the graphs.
    }
}
