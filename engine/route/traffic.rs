use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::day;
use uom::si::u64::Time;

use quadtree::Address;

use crate::common::Mode;
use crate::edge::Edge;
use crate::node::Node;
use crate::route::Route;

/// The weight of each new congestion observation on the running estimate.
/// Larger values converge faster, but are less stable.
pub const OBSERVATION_WEIGHT: f64 = 0.3;

pub trait WorldState {
    fn get_highway_segment_travelers(&self, segment: u64) -> f64;
    fn get_metro_segment_travelers(&self, segment: u64, start: Address, end: Address) -> f64;
    fn get_local_road_zone_travelers(&self, x: u64, y: u64) -> f64;
    fn get_local_road_travelers(&self, start: (f64, f64), end: (f64, f64), distance: f64) -> f64;

    fn iter_highway_segments<'a>(&'a self) -> CongestionIterator<'a, u64>;
    fn iter_metro_segments<'a>(&'a self) -> CongestionIterator<'a, (u64, Address, Address)>;
    fn iter_local_road_zones<'a>(&'a self) -> CongestionIterator<'a, (u64, u64)>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldStateImpl {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<u64, f64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    metro_segments: HashMap<(u64, Address, Address), f64>,
    /// flattened grid of local traffic zones, row major
    local_roads: Vec<f64>,

    pub grid_downsample: u32,
    grid_width: u32,
    min_tile_size: u32,
}

impl WorldStateImpl {
    pub fn new(config: &state::Config) -> Self {
        let grid_downsample = crate::local_traffic::grid_downsample(config);
        let grid_width = config.tile_width() / grid_downsample;

        Self {
            highway_segments: HashMap::new(),
            metro_segments: HashMap::new(),
            local_roads: vec![0.0; grid_width.pow(2) as usize],
            grid_downsample,
            grid_width,
            min_tile_size: config.min_tile_size,
        }
    }

    fn apply_edge_entries<F: state::Fields, G>(
        &mut self,
        edge: &Edge,
        state: &state::State<F>,
        mut f: G,
    ) where
        G: FnMut(&mut f64, f64),
    {
        match edge {
            Edge::Highway { segment, .. } => {
                f(self.highway_segments.entry(*segment).or_insert(0.0), 1.0)
            }
            Edge::MetroSegment {
                metro_line,
                start,
                stop,
                ..
            } => f(
                self.metro_segments
                    .entry((*metro_line, *start, *stop))
                    .or_insert(0.0),
                1.0,
            ),
            Edge::ModeSegment {
                mode: Mode::Driving,
                distance,
                start,
                stop,
            } => {
                let local_path: Vec<_> = self.local_path(*start, *stop).collect();
                for ((x, y), value) in local_path {
                    f(self.local_road_zone_mut(x, y), value / distance);
                }
            }
            _ => (),
        }
    }

    pub fn local_path<'a>(
        &'a self,
        start: (f64, f64),
        stop: (f64, f64),
    ) -> impl Iterator<Item = ((u64, u64), f64)> + 'a {
        let start = self.local_zone_downscale(start);
        let stop = self.local_zone_downscale(stop);
        line_drawing::XiaolinWu::<f64, i64>::new(start, stop).filter_map(|((x, y), value)| {
            // NOTE: XiaolinWu will return coordinates outside the grid, but only one
            // row/column past the grid; we can just ignore them
            assert!(
                x >= -1 && x <= self.grid_width as i64,
                "x: {}, grid_width: {}",
                x,
                self.grid_width
            );
            assert!(
                y >= -1 && y <= self.grid_width as i64,
                "y: {}, grid_width: {}",
                y,
                self.grid_width
            );
            if x >= 0 && x < self.grid_width as i64 && y >= 0 && y < self.grid_width as i64 {
                // scale number of people appropriately so that 1.0 is spread out across all
                // blocks
                let scaled_value = value * self.grid_downsample as f64 * self.min_tile_size as f64;
                Some(((x as u64, y as u64), scaled_value))
            } else {
                None
            }
        })
    }

    pub fn increment_edge<F: state::Fields>(&mut self, edge: &Edge, state: &state::State<F>) {
        self.apply_edge_entries(edge, state, |e, v| *e += v);
    }

    pub fn decrement_edge<F: state::Fields>(&mut self, edge: &Edge, state: &state::State<F>) {
        self.apply_edge_entries(edge, state, |e, v| {
            assert!(*e > 0.0);
            *e -= v;
        });
    }

    fn local_zone_downscale(&self, (x, y): (f64, f64)) -> (f64, f64) {
        (
            (x / self.grid_downsample as f64),
            (y / self.grid_downsample as f64),
        )
    }

    fn local_zone_upscale(&self, (x, y): (u64, u64)) -> (u64, u64) {
        (
            (x as f64 * self.grid_downsample as f64) as u64,
            (y as f64 * self.grid_downsample as f64) as u64,
        )
    }

    fn local_zone_index(&self, x: u64, y: u64) -> usize {
        assert!(x <= self.grid_width as u64);
        assert!(y <= self.grid_width as u64);
        (self.grid_width as u64 * y + x) as usize
    }

    fn local_zone_coords(&self, index: usize) -> (u64, u64) {
        assert!(index < self.local_roads.len());
        (
            (index as u64 % self.grid_width as u64),
            (index as u64 / self.grid_width as u64),
        )
    }

    fn local_road_zone(&self, x: u64, y: u64) -> &f64 {
        &self.local_roads[self.local_zone_index(x, y)]
    }

    fn local_road_zone_mut(&mut self, x: u64, y: u64) -> &mut f64 {
        let index = self.local_zone_index(x, y);
        &mut self.local_roads[index]
    }
}

impl WorldState for WorldStateImpl {
    fn get_highway_segment_travelers(&self, segment: u64) -> f64 {
        *self.highway_segments.get(&segment).unwrap_or(&0.0)
    }

    fn get_metro_segment_travelers(&self, metro_line: u64, start: Address, end: Address) -> f64 {
        *self
            .metro_segments
            .get(&(metro_line, start, end))
            .unwrap_or(&0.0)
    }

    fn get_local_road_zone_travelers(&self, x: u64, y: u64) -> f64 {
        let (x, y) = self.local_zone_downscale((x as f64, y as f64));
        self.local_roads[self.local_zone_index(x as u64, y as u64)]
    }

    fn get_local_road_travelers(&self, start: (f64, f64), end: (f64, f64), distance: f64) -> f64 {
        // we pass in the distance to avoid having to do a sqrt. a little gross but maybe worthwhile?
        self.local_path(start, end)
            .map(|((x, y), value)| self.local_road_zone(x, y))
            .sum::<f64>()
            / distance
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

    fn iter_local_road_zones<'a>(&'a self) -> CongestionIterator<'a, (u64, u64)> {
        CongestionIterator {
            iterator: Box::new(
                self.local_roads
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (self.local_zone_upscale(self.local_zone_coords(i)), *v)),
            ),
            total: self.local_roads.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldStateHistory {
    snapshots: Vec<WorldStateImpl>,
    period: u64,
}

impl WorldStateHistory {
    pub fn new(config: &state::Config, num_snapshots: usize) -> Self {
        let mut snapshots = Vec::with_capacity(num_snapshots);
        for _ in 0..num_snapshots {
            snapshots.push(WorldStateImpl::new(config));
        }
        Self {
            snapshots,
            period: Time::new::<day>(1).value / num_snapshots as u64,
        }
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

    fn update_prior(prior: &mut f64, observation: f64) {
        // TODO: use f64, store likelihood estimate, turn this into a real estimator.
        *prior = *prior * (1.0 - OBSERVATION_WEIGHT) + observation * OBSERVATION_WEIGHT;
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
                    .or_insert(0.0),
                *observation,
            );
        }

        for (metro_segment, observation) in world_state.metro_segments.iter() {
            Self::update_prior(
                self.snapshots[snapshot_index]
                    .metro_segments
                    .entry(*metro_segment)
                    .or_insert(0.0),
                *observation,
            );
        }

        for (index, observation) in world_state.local_roads.iter().enumerate() {
            Self::update_prior(
                &mut self.snapshots[snapshot_index].local_roads[index],
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

    fn interpolate<M>(&self, prediction_time: u64, measure: M) -> f64
    where
        M: Fn(&WorldStateImpl) -> f64,
    {
        let first_snapshot = self.get_current_snapshot_index(prediction_time, false);
        let second_snapshot = self.get_current_snapshot_index(prediction_time, true);
        let fraction =
            (prediction_time % self.snapshot_period()) as f64 / self.snapshot_period() as f64;

        // simple linear interpolation function
        // in the future we could potentially do something fancier
        measure(&self.snapshots[first_snapshot]) * (1.0 - fraction)
            + measure(&self.snapshots[second_snapshot]) * 1.0
    }
}

#[derive(Debug)]
pub struct WorldStatePredictor<'a> {
    history: &'a WorldStateHistory,
    prediction_time: u64,
}

impl<'a> WorldState for WorldStatePredictor<'a> {
    fn get_highway_segment_travelers(&self, segment: u64) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_highway_segment_travelers(segment)
            })
    }

    fn get_metro_segment_travelers(&self, metro_line: u64, start: Address, end: Address) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_metro_segment_travelers(metro_line, start, end)
            })
    }

    fn get_local_road_zone_travelers(&self, x: u64, y: u64) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_local_road_zone_travelers(x, y)
            })
    }

    fn get_local_road_travelers(&self, start: (f64, f64), end: (f64, f64), distance: f64) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_local_road_travelers(start, end, distance)
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

    fn iter_local_road_zones<'b>(&'b self) -> CongestionIterator<'b, (u64, u64)> {
        let snapshot_index = self
            .history
            .get_current_snapshot_index(self.prediction_time, true);
        let total = self.history.snapshots[snapshot_index].local_roads.len();
        CongestionIterator {
            iterator: Box::new((0..total).map(move |i| {
                let snapshot = &self.history.snapshots[snapshot_index];
                let (x, y) = snapshot.local_zone_upscale(snapshot.local_zone_coords(i));
                ((x, y), self.get_local_road_zone_travelers(x, y))
            })),
            total,
        }
    }
}

pub struct CongestionIterator<'a, K> {
    iterator: Box<dyn Iterator<Item = (K, f64)> + 'a>,
    total: usize,
}

impl<'a, K: 'a> CongestionIterator<'a, K> {
    pub fn keys(self) -> Box<dyn Iterator<Item = K> + 'a> {
        Box::new(self.iterator.map(|(k, _)| k))
    }

    pub fn values(self) -> Box<dyn Iterator<Item = f64> + 'a> {
        Box::new(self.iterator.map(|(_, v)| v))
    }
}

impl<'a, K: 'a + Copy> CongestionIterator<'a, K> {
    pub fn filter<F: 'a>(self, filter: F) -> Self
    where
        F: Fn(K, f64) -> bool,
    {
        Self {
            iterator: Box::new(self.iterator.filter(move |(k, v)| filter(*k, *v))),
            // TODO: it's not possible to calculate total unless we clone the iterator?
            total: 0,
        }
    }
}

pub trait CongestionStats {
    /// sum of all items
    fn sum(self) -> f64;

    /// calculate the mean of all items
    fn mean(self) -> f64;

    /// calculate the root mean square of all items
    fn rms(self) -> f64;

    /// constructs a histogram
    fn histogram(self, buckets: usize, max: f64) -> Vec<u64>;
}

impl<'a, K> CongestionStats for CongestionIterator<'a, K> {
    fn sum(self) -> f64 {
        self.values().sum()
    }

    fn mean(self) -> f64 {
        let mut sum = 0.0;
        let mut total = 0;
        for (_, value) in self.iterator {
            sum += value;
            total += 1;
        }
        if total > 0 {
            sum / total as f64
        } else {
            0.0
        }
    }

    fn rms(self) -> f64 {
        let mut sum = 0.0;
        let mut total = 0;
        for (_, value) in self.iterator {
            sum += value.powi(2);
            total += 1;
        }
        if total > 0 {
            (sum / total as f64).sqrt()
        } else {
            0.0
        }
    }

    fn histogram(self, buckets: usize, max: f64) -> Vec<u64> {
        assert!(max > 0.0);
        let mut histogram = vec![0; buckets];
        for (_, value) in self.iterator {
            let bucket = ((value / max) * buckets as f64) as usize;
            histogram[bucket.min(buckets - 1)] += 1;
        }
        histogram
    }
}
