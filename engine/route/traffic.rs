use std::collections::HashMap;

use crate::common::{Edge, Node};
use crate::route::Route;

#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct WorldState {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<u64, u64>,
    /// map from highway junction IDs to number of travelers
    highway_junctions: HashMap<u64, u64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    metro_segments: HashMap<(u64, quadtree::Address, quadtree::Address), u64>,
    /// map from station address to number to number of travelers
    metro_stations: HashMap<quadtree::Address, u64>,
}

impl WorldState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_routes<'a, I>(routes: I) -> Self
    where
        I: IntoIterator<Item = &'a Route>,
    {
        let mut highway_segments = HashMap::new();
        let mut highway_junctions = HashMap::new();
        let mut metro_segments = HashMap::new();
        let mut metro_stations = HashMap::new();

        for route in routes.into_iter() {
            for ((start, end), edge) in route.iter() {
                match edge {
                    Edge::Highway { segment, .. } => {
                        *highway_segments.entry(*segment).or_insert(0) += 1;
                    }
                    Edge::MetroSegment {
                        metro_line,
                        start,
                        stop,
                        ..
                    } => {
                        *metro_segments
                            .entry((*metro_line, *start, *stop))
                            .or_insert(0) += 1;
                    }
                    _ => (),
                }
            }
            for node in &route.nodes {
                match node {
                    Node::HighwayJunction { junction, .. } => {
                        *highway_junctions.entry(*junction).or_insert(0) += 1;
                    }
                    Node::HighwayRamp { junction, .. } => {
                        *highway_junctions.entry(*junction).or_insert(0) += 1;
                    }
                    Node::MetroStation { station } => {
                        *metro_stations.entry(station.address).or_insert(0) += 1;
                    }
                    _ => (),
                }
            }
        }

        Self {
            highway_segments,
            highway_junctions,
            metro_segments,
            metro_stations,
        }
    }

    pub fn get_highway_segment_travelers(&self, segment: u64) -> u64 {
        *self.highway_segments.get(&segment).unwrap_or(&0)
    }

    pub fn get_highway_junction_travelers(&self, junction: u64) -> u64 {
        *self.highway_junctions.get(&junction).unwrap_or(&0)
    }

    pub fn get_metro_segment_travelers(
        &self,
        metro_line: u64,
        start: quadtree::Address,
        end: quadtree::Address,
    ) -> u64 {
        *self
            .metro_segments
            .get(&(metro_line, start, end))
            .unwrap_or(&0)
    }

    pub fn get_metro_station_travelers(&self, station: quadtree::Address) -> u64 {
        *self.metro_stations.get(&station).unwrap_or(&0)
    }
}
