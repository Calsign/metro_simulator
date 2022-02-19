mod mode;

use std::collections::{HashMap, HashSet};

pub struct BaseGraphInput<'a, I>
where
    I: Iterator<Item = &'a metro::MetroLine>,
{
    pub metro_lines: I,
    pub tile_size: f64,

    pub filter_metro_lines: Option<HashSet<u64>>,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct WorldState {}

#[derive(Debug)]
#[non_exhaustive]
enum Node {
    MetroStation {
        station: metro::Station,
    },
    MetroStop {
        station: metro::Station,
        metro_line: u64,
    },
}

impl Node {
    fn address(&self) -> &quadtree::Address {
        use Node::*;
        match self {
            MetroStation { station } => &station.address,
            MetroStop {
                station,
                metro_line,
            } => &station.address,
        }
    }

    fn location(&self) -> (f64, f64) {
        use Node::*;
        match self {
            MetroStation { station } => unimplemented!(),
            MetroStop {
                station,
                metro_line,
            } => unimplemented!(),
        }
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Node::*;

        match self {
            MetroStation { station } => write!(f, "station:{}", station.name),
            MetroStop {
                station,
                metro_line,
            } => write!(f, "stop:{}:{}", metro_line, station.name),
        }
    }
}

#[non_exhaustive]
#[derive(Debug)]
enum Edge {
    MetroSegment {
        time: f64,
    },
    MetroEmbark {
        metro_line: u64,
        station: metro::Station,
    },
    MetroDisembark {
        metro_line: u64,
        station: metro::Station,
    },
}

impl Edge {
    fn cost(&self, state: &WorldState) -> f64 {
        use Edge::*;
        match self {
            MetroSegment { time } => *time,
            MetroEmbark {
                metro_line,
                station,
            } => 0.0,
            MetroDisembark {
                metro_line,
                station,
            } => 0.0,
        }
    }
}

impl std::fmt::Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Edge::*;
        match self {
            MetroSegment { time } => write!(f, "{:.2}", time),
            MetroEmbark {
                metro_line,
                station,
            } => write!(f, "embark:{}:{}", metro_line, station.name),
            MetroDisembark {
                metro_line,
                station,
            } => write!(f, "disembark:{}:{}", metro_line, station.name),
        }
    }
}

type InnerGraph = petgraph::Graph<Node, Edge>;
pub struct Graph(InnerGraph);

impl Graph {
    pub fn dump<W>(&self, write: &mut W) -> Result<(), std::io::Error>
    where
        W: std::io::Write,
    {
        let dot = petgraph::dot::Dot::new(&self.0);
        write!(write, "{}", dot)?;
        Ok(())
    }
}

pub fn construct_base_graph<'a, I>(input: BaseGraphInput<'a, I>) -> Graph
where
    I: Iterator<Item = &'a metro::MetroLine>,
{
    use itertools::Itertools;

    let mut graph = InnerGraph::new();

    let mut station_map = HashMap::new();

    for metro_line in input.metro_lines {
        if let Some(ref filter) = input.filter_metro_lines {
            if !filter.contains(&metro_line.id) {
                continue;
            }
        }

        let mut stop_map = HashMap::new();

        let speed_keys = metro::timing::speed_keys(metro_line.get_keys(), input.tile_size);
        let timetable = metro::timing::timetable(&speed_keys);
        for (station, _) in timetable.iter() {
            let station_id = *station_map
                .entry(station.address.clone())
                .or_insert_with(|| {
                    graph.add_node(Node::MetroStation {
                        station: station.clone(),
                    })
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

    Graph(graph)
}
