use std::collections::{HashMap, HashSet};

use crate::common::{Edge, Error, Mode, Node};

pub struct BaseGraphInput<'a, I>
where
    I: Iterator<Item = &'a metro::MetroLine>,
{
    pub metro_lines: I,
    pub tile_size: f64,
    pub max_depth: u32,

    pub filter_metro_lines: Option<HashSet<u64>>,
}

pub type InnerGraph = petgraph::Graph<Node, Edge>;

pub struct Graph {
    pub graph: InnerGraph,
    pub walking_neighbors: quadtree::NeighborsStore<petgraph::graph::NodeIndex>,
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

pub fn construct_base_graph<'a, I>(input: BaseGraphInput<'a, I>) -> Result<Graph, Error>
where
    I: Iterator<Item = &'a metro::MetroLine>,
{
    use itertools::Itertools;

    let mut graph = InnerGraph::new();

    let mut station_map = HashMap::new();
    let mut walking_neighbors = quadtree::NeighborsStore::new(4, input.max_depth);

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
                    walking_neighbors
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

    walking_neighbors.visit_all_radius(
        &mut AddEdgesVisitor {
            graph: &mut graph,
            tile_size: input.tile_size,
            mode: Mode::Walking,
        },
        |_| Mode::Walking.max_radius() / input.tile_size,
    )?;

    Ok(Graph {
        graph,
        walking_neighbors,
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
