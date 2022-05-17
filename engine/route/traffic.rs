use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::day;
use uom::si::u64::Time;

use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;

pub trait WorldState {
    fn get_highway_segment_travelers(&self, segment: u64) -> u64;
    fn get_metro_segment_travelers(
        &self,
        segment: u64,
        start: quadtree::Address,
        end: quadtree::Address,
    ) -> u64;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldStateImpl {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<u64, u64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    metro_segments: HashMap<(u64, quadtree::Address, quadtree::Address), u64>,
}

impl WorldStateImpl {
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

    pub fn highway_segment_count(&self) -> usize {
        self.highway_segments.len()
    }

    pub fn metro_segment_count(&self) -> usize {
        self.metro_segments.len()
    }
}

impl WorldState for WorldStateImpl {
    fn get_highway_segment_travelers(&self, segment: u64) -> u64 {
        *self.highway_segments.get(&segment).unwrap_or(&0)
    }

    fn get_metro_segment_travelers(
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateHistory {
    snapshots: Vec<WorldStateImpl>,
}

impl WorldStateHistory {
    pub fn new(num_snapshots: usize) -> Self {
        let mut snapshots = Vec::with_capacity(num_snapshots);
        for _ in 0..num_snapshots {
            snapshots.push(WorldStateImpl::new());
        }
        Self { snapshots }
    }

    /**
     * The number of stored snapshots. Currently snapshots are on a daily cycle.
     */
    pub fn num_snapshots(&self) -> usize {
        self.snapshots.len()
    }

    /**
     * Number of seconds between each snapshot.
     */
    pub fn snapshot_period(&self) -> u64 {
        Time::new::<day>(1).value / self.num_snapshots() as u64
    }

    fn update_prior(prior: &mut u64, observation: u64) {
        const observation_weight: f64 = 0.1;
        // TODO: use f64, store likelihood estimate, turn this into a real estimator.
        *prior = (*prior as f64 * (1.0 - observation_weight)
            + observation as f64 * observation_weight) as u64;
    }

    /**
     * Update the history with a new snapshot. The new data will be used for future predictions.
     *
     * Panics if the given time is not a valid snapshot time; in other words, must be an exact match
     * for the time at which snapshots are saved.
     */
    pub fn take_snapshot(&mut self, world_state: &WorldStateImpl, current_time: u64) {
        assert!(current_time % self.snapshot_period() == 0);
        let snapshot_index =
            (current_time / self.snapshot_period()) as usize % self.num_snapshots();

        // TODO: if we start removing keys from the world state (e.g. if we delete them when they
        // hit zero as a way of cleaning up actual deleted edges), then we will need to also
        // traverse the keys that are in the snapshot but not in the new world state.

        for (highway_segment, observation) in world_state.highway_segments.iter() {
            Self::update_prior(
                self.snapshots[snapshot_index]
                    .highway_segments
                    .entry(*highway_segment)
                    .or_insert(0),
                *observation,
            );
        }

        for (metro_segment, observation) in world_state.metro_segments.iter() {
            Self::update_prior(
                self.snapshots[snapshot_index]
                    .metro_segments
                    .entry(*metro_segment)
                    .or_insert(0),
                *observation,
            );
        }
    }

    /**
     * Returns a predictor which can be used in place of WorldStateImpl to predict congestion at the
     * given prediction time.
     */
    pub fn get_predictor<'a>(&'a self, prediction_time: u64) -> WorldStatePredictor<'a> {
        WorldStatePredictor {
            history: self,
            prediction_time,
        }
    }

    fn interpolate<M>(&self, prediction_time: u64, measure: M) -> u64
    where
        M: Fn(&WorldStateImpl) -> u64,
    {
        let first_snapshot = (prediction_time as f64 / self.snapshot_period() as f64).floor()
            as usize
            % self.num_snapshots();
        let second_snapshot = ((prediction_time + 1) as f64 / self.snapshot_period() as f64).ceil()
            as usize
            % self.num_snapshots();
        let fraction =
            (prediction_time % self.snapshot_period()) as f64 / self.snapshot_period() as f64;

        // simple linear interpolation function
        // in the future we could potentially do something fancier
        (measure(&self.snapshots[first_snapshot]) as f64 * (1.0 - fraction)
            + measure(&self.snapshots[second_snapshot]) as f64 * 1.0) as u64
    }
}

#[derive(Debug)]
pub struct WorldStatePredictor<'a> {
    history: &'a WorldStateHistory,
    prediction_time: u64,
}

impl<'a> WorldState for WorldStatePredictor<'a> {
    fn get_highway_segment_travelers(&self, segment: u64) -> u64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_highway_segment_travelers(segment)
            })
    }

    fn get_metro_segment_travelers(
        &self,
        metro_line: u64,
        start: quadtree::Address,
        end: quadtree::Address,
    ) -> u64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_metro_segment_travelers(metro_line, start, end)
            })
    }
}
