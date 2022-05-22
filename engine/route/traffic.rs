use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::day;
use uom::si::u64::Time;

use quadtree::Address;

use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;

/// The weight of each new congestion observation on the running estimate.
/// Larger values converge faster, but are less stable.
pub const OBSERVATION_WEIGHT: f64 = 0.3;

pub trait WorldState {
    fn get_highway_segment_travelers(&self, segment: u64) -> u64;
    fn get_metro_segment_travelers(&self, segment: u64, start: Address, end: Address) -> u64;

    fn iter_highway_segments<'a>(&'a self) -> CongestionIterator<'a, u64>;
    fn iter_metro_segments<'a>(&'a self) -> CongestionIterator<'a, (u64, Address, Address)>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldStateImpl {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<u64, u64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    metro_segments: HashMap<(u64, Address, Address), u64>,
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
}

impl WorldState for WorldStateImpl {
    fn get_highway_segment_travelers(&self, segment: u64) -> u64 {
        *self.highway_segments.get(&segment).unwrap_or(&0)
    }

    fn get_metro_segment_travelers(&self, metro_line: u64, start: Address, end: Address) -> u64 {
        *self
            .metro_segments
            .get(&(metro_line, start, end))
            .unwrap_or(&0)
    }

    fn iter_highway_segments<'a>(&'a self) -> CongestionIterator<'a, u64> {
        CongestionIterator {
            iterator: Box::new(self.highway_segments.iter().map(|(k, v)| (*k, *v))),
            total: self.highway_segments.len(),
        }
    }

    fn iter_metro_segments<'a>(&'a self) -> CongestionIterator<'a, (u64, Address, Address)> {
        CongestionIterator {
            iterator: Box::new(self.metro_segments.iter().map(|(k, v)| (*k, *v))),
            total: self.metro_segments.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateHistory {
    snapshots: Vec<WorldStateImpl>,
    period: u64,
}

impl WorldStateHistory {
    pub fn new(num_snapshots: usize) -> Self {
        let mut snapshots = Vec::with_capacity(num_snapshots);
        for _ in 0..num_snapshots {
            snapshots.push(WorldStateImpl::new());
        }
        Self { snapshots, period: Time::new::<day>(1).value / num_snapshots as u64 }
    }

    /**
     * The number of stored snapshots. Currently snapshots are on a daily cycle.
     */
    pub fn num_snapshots(&self) -> usize {
        self.snapshots.len()
    }

    pub fn get_snapshots(&self) -> &Vec<WorldStateImpl> {
        &self.snapshots
    }

    /**
     * Number of seconds between each snapshot.
     */
    pub fn snapshot_period(&self) -> u64 {
        self.period
    }

    fn update_prior(prior: &mut u64, observation: u64) {
        // TODO: use f64, store likelihood estimate, turn this into a real estimator.
        *prior = (*prior as f64 * (1.0 - OBSERVATION_WEIGHT)
            + observation as f64 * OBSERVATION_WEIGHT) as u64;
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

    pub fn get_current_snapshot_index(&self, prediction_time: u64, round_forward: bool) -> usize {
        let offset = if round_forward { 1 } else { 0 };
        let periods = (prediction_time + offset) as f64 / self.snapshot_period() as f64;
        let rounded = if round_forward {
            periods.ceil()
        } else {
            periods.floor()
        };
        rounded as usize % self.num_snapshots()
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
        let first_snapshot = self.get_current_snapshot_index(prediction_time, false);
        let second_snapshot = self.get_current_snapshot_index(prediction_time, true);
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

    fn get_metro_segment_travelers(&self, metro_line: u64, start: Address, end: Address) -> u64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_metro_segment_travelers(metro_line, start, end)
            })
    }

    fn iter_highway_segments<'b>(&'b self) -> CongestionIterator<'b, u64> {
        let snapshot = self
            .history
            .get_current_snapshot_index(self.prediction_time, true);
        CongestionIterator {
            iterator: Box::new(
                self.history.snapshots[snapshot]
                    .highway_segments
                    .keys()
                    .map(|segment| (*segment, self.get_highway_segment_travelers(*segment))),
            ),
            total: self.history.snapshots[snapshot].highway_segments.len(),
        }
    }

    fn iter_metro_segments<'b>(&'b self) -> CongestionIterator<'b, (u64, Address, Address)> {
        let snapshot = self
            .history
            .get_current_snapshot_index(self.prediction_time, true);
        CongestionIterator {
            iterator: Box::new(self.history.snapshots[snapshot].metro_segments.keys().map(
                |(segment, start, end)| {
                    (
                        (*segment, *start, *end),
                        self.get_metro_segment_travelers(*segment, *start, *end),
                    )
                },
            )),
            total: self.history.snapshots[snapshot].metro_segments.len(),
        }
    }
}

pub struct CongestionIterator<'a, K> {
    iterator: Box<dyn Iterator<Item = (K, u64)> + 'a>,
    total: usize,
}

impl<'a, K: 'a> CongestionIterator<'a, K> {
    pub fn keys(self) -> Box<dyn Iterator<Item = K> + 'a> {
        Box::new(self.iterator.map(|(k, _)| k))
    }

    pub fn values(self) -> Box<dyn Iterator<Item = u64> + 'a> {
        Box::new(self.iterator.map(|(_, v)| v))
    }
}

impl<'a, K: 'a + Copy> CongestionIterator<'a, K> {
    pub fn filter<F: 'a>(self, filter: F) -> Self
    where
        F: Fn(K, u64) -> bool,
    {
        Self {
            iterator: Box::new(self.iterator.filter(move |(k, v)| filter(*k, *v))),
            // TODO: it's not possible to calculate total unless we clone the iterator?
            total: 0,
        }
    }
}

pub trait CongestionStats<T> {
    /// sum of all items
    fn sum(self) -> T;

    /// calculate the mean of all items
    fn mean(self) -> f64;

    /// calculate the root mean square of all items
    fn rms(self) -> f64;

    /// constructs a histogram
    fn histogram(self, buckets: usize, max: T) -> Vec<u64>;
}

impl<'a, K> CongestionStats<u64> for CongestionIterator<'a, K> {
    fn sum(self) -> u64 {
        self.values().sum()
    }

    fn mean(self) -> f64 {
        let mut sum = 0;
        let mut total = 0;
        for (_, value) in self.iterator {
            sum += value;
            total += 1;
        }
        if total > 0 {
            sum as f64 / total as f64
        } else {
            0.0
        }
    }

    fn rms(self) -> f64 {
        let mut sum = 0;
        let mut total = 0;
        for (_, value) in self.iterator {
            sum += value.pow(2);
            total += 1;
        }
        if total > 0 {
            (sum as f64 / total as f64).sqrt()
        } else {
            0.0
        }
    }

    fn histogram(self, buckets: usize, max: u64) -> Vec<u64> {
        assert!(max > 0);
        let mut histogram = vec![0; buckets];
        for (_, value) in self.iterator {
            let bucket = ((value as f32 / max as f32) * buckets as f32) as usize;
            histogram[bucket.min(buckets - 1)] += 1;
        }
        histogram
    }
}
