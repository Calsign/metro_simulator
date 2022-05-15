use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldState {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<u64, u64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    metro_segments: HashMap<(u64, quadtree::Address, quadtree::Address), u64>,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            highway_segments: HashMap::new(),
            metro_segments: HashMap::new(),
        }
    }

    fn edge_entry(&mut self, edge: &Edge, state: &state::State) -> Option<&mut u64> {
        match edge {
            Edge::Highway { segment, .. } => {
                Some(self.highway_segments.entry(*segment).or_insert(0))
            }
            Edge::MetroSegment {
                metro_line,
                start,
                stop,
                ..
            } => Some(
                self.metro_segments
                    .entry((*metro_line, *start, *stop))
                    .or_insert(0),
            ),
            _ => None,
        }
    }

    pub fn increment_edge(&mut self, edge: &Edge, state: &state::State) {
        self.edge_entry(edge, state).map(|e| *e += 1);
    }

    pub fn decrement_edge(&mut self, edge: &Edge, state: &state::State) {
        self.edge_entry(edge, state).map(|e| {
            assert!(*e > 0);
            *e -= 1;
        });
    }

    pub fn get_highway_segment_travelers(&self, segment: u64) -> u64 {
        *self.highway_segments.get(&segment).unwrap_or(&0)
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
}
