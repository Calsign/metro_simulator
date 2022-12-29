use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uom::si::time::day;
use uom::si::u64::Time;

use crate::common::{Error, Mode};
use crate::edge::Edge;

/// The weight of each new congestion observation on the running estimate.
/// Larger values converge faster, but are less stable.
pub const OBSERVATION_WEIGHT: f64 = 0.3;

/// threshold for driver counts (fractional because some edges are split over multiple grid tiles)
const TOLERANCE: f64 = 0.0001;

pub trait WorldState {
    fn get_highway_segment_travelers(&self, segment: network::SegmentHandle) -> f64;
    fn get_metro_segment_travelers(&self, segment: network::SegmentHandle) -> f64;
    fn get_local_road_zone_travelers(&self, x: u64, y: u64) -> f64;
    fn get_local_road_travelers(&self, start: (f64, f64), end: (f64, f64), distance: f64) -> f64;
    fn get_parking(&self, x: f64, y: f64) -> f64;

    fn iter_highway_segments(&self) -> CongestionIterator<'_, network::SegmentHandle>;
    fn iter_metro_segments(&self) -> CongestionIterator<'_, network::SegmentHandle>;
    fn iter_local_road_zones(&self) -> CongestionIterator<'_, (u64, u64)>;
    fn iter_parking_zones(&self) -> CongestionIterator<'_, (u64, u64)>;
}

// We use serde_as to allow serializing non-string keys to json.
// TODO: use bincode or something else as the primary storage format instead of json.
#[serde_with::serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct WorldStateImpl {
    /// map from highway segment IDs to number of travelers
    highway_segments: HashMap<network::SegmentHandle, f64>,
    /// map from (metro line ID, start station address, end station address) pairs to number of
    /// travelers
    #[serde_as(as = "Vec<(_, _)>")]
    metro_segments: HashMap<network::SegmentHandle, f64>,
    /// flattened grid of local traffic zones, row major
    local_roads: Vec<f64>,
    /// flattened grid of parking zones, row major
    parking: Vec<f64>,

    pub grid_downsample: u32,
    grid_width: u32,
    min_tile_size: u32,

    /// map from addresses to the number of cars parked there
    #[serde_as(as = "Vec<(_, _)>")]
    parked_cars: HashMap<quadtree::Address, u64>,
}

impl WorldStateImpl {
    pub fn new(config: &state::Config) -> Self {
        let grid_downsample = crate::local_traffic::grid_downsample(config);
        let grid_width = config.tile_width() / grid_downsample;
        let grid_len = grid_width.pow(2) as usize;

        Self {
            highway_segments: HashMap::new(),
            metro_segments: HashMap::new(),
            local_roads: vec![0.0; grid_len],
            parking: vec![0.0; grid_len],
            grid_downsample,
            grid_width,
            min_tile_size: config.min_tile_size,
            parked_cars: HashMap::new(),
        }
    }

    fn apply_edge_entries<F>(&mut self, edge: &Edge, mut f: F) -> Result<(), Error>
    where
        F: FnMut(&mut f64, f64) -> Result<(), Error>,
    {
        match edge {
            Edge::Highway { segment, .. } => {
                f(self.highway_segments.entry(*segment).or_insert(0.0), 1.0)?;
            }
            Edge::MetroSegment {
                oriented_segment, ..
            } => {
                f(
                    self.metro_segments
                        .entry(oriented_segment.segment)
                        .or_insert(0.0),
                    1.0,
                )?;
            }
            Edge::ModeSegment {
                mode: Mode::Driving,
                distance,
                start,
                stop,
            } => {
                // avoid NaN
                if *distance > 0.0 {
                    let local_path: Vec<_> = self.local_path(*start, *stop).collect();
                    for ((x, y), value) in local_path {
                        let scaled_value = value / distance;
                        assert!(scaled_value.is_normal());
                        if scaled_value > TOLERANCE {
                            f(self.local_road_zone_mut(x, y), scaled_value)?;
                        }
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }

    pub fn local_path(
        &self,
        start: (f64, f64),
        stop: (f64, f64),
    ) -> impl Iterator<Item = ((u64, u64), f64)> + '_ {
        let start = self.local_zone_downscale(start);
        let stop = self.local_zone_downscale(stop);
        line_drawing::XiaolinWu::<f64, i64>::new(start, stop).filter_map(|((x, y), value)| {
            // NOTE: XiaolinWu will return coordinates outside the grid; we can just ignore them
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

    pub fn increment_edge_no_parking(&mut self, edge: &Edge) -> Result<(), Error> {
        self.apply_edge_entries(edge, |e, v| {
            *e += v;
            assert!(e.is_finite() && !e.is_nan());
            Ok(())
        })?;
        Ok(())
    }

    pub fn increment_edge(&mut self, edge: &Edge) -> Result<(), Error> {
        match edge {
            Edge::ModeTransition {
                from: Mode::Driving,
                to: Mode::Walking,
                address,
            } => self.increment_parking(*address)?,
            Edge::ModeTransition {
                from: Mode::Walking,
                to: Mode::Driving,
                address,
            } => self.decrement_parking(*address)?,
            _ => self.increment_edge_no_parking(edge)?,
        }

        Ok(())
    }

    pub fn decrement_edge(&mut self, edge: &Edge) -> Result<(), Error> {
        self.apply_edge_entries(edge, |e, v| {
            // small floating point rounding errors can accumulate here, so deal with them
            if v - *e > TOLERANCE {
                return Err(Error::EdgeCountingError(format!("e: {}, v: {}", e, v)));
            }
            *e -= v;
            if *e < -TOLERANCE {
                *e = 0.0;
            }
            assert!(e.is_finite() && !e.is_nan());
            Ok(())
        })?;

        Ok(())
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

    fn local_road_zone(&self, x: u64, y: u64) -> f64 {
        self.local_roads[self.local_zone_index(x, y)]
    }

    fn local_road_zone_mut(&mut self, x: u64, y: u64) -> &mut f64 {
        let index = self.local_zone_index(x, y);
        &mut self.local_roads[index]
    }

    fn parking_zone(&self, x: u64, y: u64) -> f64 {
        self.parking[self.local_zone_index(x, y)]
    }

    fn parking_zone_mut(&mut self, x: u64, y: u64) -> &mut f64 {
        let index = self.local_zone_index(x, y);
        &mut self.parking[index]
    }

    /**
     * Increment the total parking at the specified location. This is intended for use when creating
     * agents so that the initial parking levels are consistent with where agents' cars are parked.
     */
    pub fn increment_parking(&mut self, address: quadtree::Address) -> Result<(), Error> {
        let (x, y) = self.local_zone_downscale(address.to_xy_f64());
        *self.parking_zone_mut(x as u64, y as u64) += 1.0;

        *self.parked_cars.entry(address).or_insert(0) += 1;

        Ok(())
    }

    pub fn decrement_parking(&mut self, address: quadtree::Address) -> Result<(), Error> {
        let (x, y) = self.local_zone_downscale(address.to_xy_f64());
        let handle = self.parking_zone_mut(x as u64, y as u64);
        if *handle < 1.0 {
            let (x, y) = address.to_xy_f64();
            return Err(Error::ParkingError(format!(
                "parking handle {} < 1.0 at ({}, {})",
                *handle, x, y
            )));
        }
        *handle -= 1.0;

        let parked_cars = self.parked_cars.entry(address).or_insert(0);
        if *parked_cars < 1 {
            return Err(Error::ParkingError(format!(
                "parked cars {} < 1 at {:?}",
                *parked_cars, address
            )));
        }
        *parked_cars -= 1;

        Ok(())
    }

    pub fn move_parked_cars(
        &mut self,
        from: quadtree::Address,
        to: quadtree::Address,
    ) -> Result<(), Error> {
        let total = *self.parked_cars.get(&from).unwrap_or(&0);
        for _ in 0..total {
            self.decrement_parking(from)?;
            self.increment_parking(to)?;
        }
        Ok(())
    }

    /// Compare two HashMaps for equality, assuming a default value if either is missing a key.
    /// Invokes the callback function f for any key with unequal values.
    fn compare_hash_maps<K, V, F>(a: &HashMap<K, V>, b: &HashMap<K, V>, mut f: F)
    where
        K: Eq + std::hash::Hash,
        V: Default + std::cmp::PartialEq,
        F: FnMut(&K, &V, &V),
    {
        let default_v = V::default();

        for (key, a_value) in a {
            let b_value = b.get(key).unwrap_or(&default_v);
            if a_value != b_value {
                f(key, a_value, b_value);
            }
        }

        for (key, b_value) in b {
            let a_value = a.get(key).unwrap_or(&default_v);
            if a_value != b_value {
                f(key, a_value, b_value);
            }
        }
    }

    /// Compares the traffic stored in this world state to another world state. Used for testing.
    /// Returns a list of errors. The traffic is error-free iff there are no errors returned.
    pub fn check_same_traffic(&self, other: &Self) -> Vec<String> {
        let mut errors = Vec::new();

        // NOTE: Parking is checked separately. We could combine these together in the future.

        Self::compare_hash_maps(
            &self.highway_segments,
            &other.highway_segments,
            |id, self_v, other_v| {
                if (self_v - other_v).abs() > TOLERANCE {
                    errors.push(format!(
                        "highway segment mismatch for id {:?}: {} != {}",
                        id, self_v, other_v,
                    ))
                }
            },
        );

        Self::compare_hash_maps(
            &self.metro_segments,
            &other.metro_segments,
            |segment, self_v, other_v| {
                if (self_v - other_v).abs() > TOLERANCE {
                    errors.push(format!(
                        "metro segment mismatch for id {:?}; {} ! {}",
                        segment, self_v, other_v,
                    ))
                }
            },
        );

        for i in 0..self.local_roads.len() {
            if (self.local_roads[i] - other.local_roads[i]).abs() > TOLERANCE {
                let (x, y) = self.local_zone_upscale(self.local_zone_coords(i));
                errors.push(format!(
                    "mismatched local road traffic at ({}, {}): {} != {}",
                    x, y, self.local_roads[i], other.local_roads[i],
                ));
            }
        }

        errors
    }

    /// Compares the parking stored in this world state to another world state. Used for testing.
    /// Returns a list of errors. The parking is error-free iff there are no errors returned.
    pub fn check_same_parking(&self, other: &Self) -> Vec<String> {
        let mut errors = Vec::new();

        for i in 0..self.parking.len() {
            if self.parking[i] != other.parking[i] {
                let (x, y) = self.local_zone_upscale(self.local_zone_coords(i));
                errors.push(format!(
                    "mismatched parking at ({}, {}): {} != {}",
                    x, y, self.parking[i], other.parking[i]
                ));
            }
        }

        Self::compare_hash_maps(
            &self.parked_cars,
            &other.parked_cars,
            |address, self_v, other_v| {
                errors.push(format!(
                    "parked cars mismatch at {:?}: {} != {}",
                    address, self_v, other_v
                ))
            },
        );

        errors
    }
}

impl WorldState for WorldStateImpl {
    fn get_highway_segment_travelers(&self, segment: network::SegmentHandle) -> f64 {
        *self.highway_segments.get(&segment).unwrap_or(&0.0)
    }

    fn get_metro_segment_travelers(&self, segment: network::SegmentHandle) -> f64 {
        *self.metro_segments.get(&segment).unwrap_or(&0.0)
    }

    fn get_local_road_zone_travelers(&self, x: u64, y: u64) -> f64 {
        let (x, y) = self.local_zone_downscale((x as f64, y as f64));
        self.local_road_zone(x as u64, y as u64)
    }

    fn get_local_road_travelers(&self, start: (f64, f64), end: (f64, f64), distance: f64) -> f64 {
        // we pass in the distance to avoid having to do a sqrt. a little gross but maybe worthwhile?
        if distance > 0.0 {
            self.local_path(start, end)
                .map(|((x, y), value)| self.local_road_zone(x, y) * value)
                .sum::<f64>()
                / distance
        } else {
            0.0
        }
    }

    fn get_parking(&self, x: f64, y: f64) -> f64 {
        let (x, y) = self.local_zone_downscale((x, y));
        self.parking_zone(x as u64, y as u64)
    }

    fn iter_highway_segments(&self) -> CongestionIterator<'_, network::SegmentHandle> {
        CongestionIterator {
            iterator: Box::new(self.highway_segments.iter().map(|(k, v)| (*k, *v))),
            total: Some(self.highway_segments.len()),
        }
    }

    fn iter_metro_segments(&self) -> CongestionIterator<'_, network::SegmentHandle> {
        CongestionIterator {
            iterator: Box::new(self.metro_segments.iter().map(|(k, v)| (*k, *v))),
            total: Some(self.metro_segments.len()),
        }
    }

    fn iter_local_road_zones(&self) -> CongestionIterator<'_, (u64, u64)> {
        CongestionIterator {
            iterator: Box::new(
                self.local_roads
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (self.local_zone_upscale(self.local_zone_coords(i)), *v)),
            ),
            total: Some(self.local_roads.len()),
        }
    }

    fn iter_parking_zones(&self) -> CongestionIterator<'_, (u64, u64)> {
        CongestionIterator {
            iterator: Box::new(
                self.parking
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (self.local_zone_upscale(self.local_zone_coords(i)), *v)),
            ),
            total: Some(self.parking.len()),
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

        for (index, observation) in world_state.parking.iter().enumerate() {
            Self::update_prior(
                &mut self.snapshots[snapshot_index].parking[index],
                *observation,
            );
        }
    }

    pub fn get_current_snapshot_index(&self, prediction_time: u64, round_forward: bool) -> usize {
        let offset = u64::from(round_forward);
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
    pub fn get_predictor(&self, prediction_time: u64) -> WorldStatePredictor<'_> {
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
    fn get_highway_segment_travelers(&self, segment: network::SegmentHandle) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_highway_segment_travelers(segment)
            })
    }

    fn get_metro_segment_travelers(&self, segment: network::SegmentHandle) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_metro_segment_travelers(segment)
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

    fn get_parking(&self, x: f64, y: f64) -> f64 {
        self.history
            .interpolate(self.prediction_time, |world_state| {
                world_state.get_parking(x, y)
            })
    }

    fn iter_highway_segments(&self) -> CongestionIterator<'_, network::SegmentHandle> {
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
            total: Some(self.history.snapshots[snapshot].highway_segments.len()),
        }
    }

    fn iter_metro_segments(&self) -> CongestionIterator<'_, network::SegmentHandle> {
        let snapshot = self
            .history
            .get_current_snapshot_index(self.prediction_time, true);
        CongestionIterator {
            iterator: Box::new(
                self.history.snapshots[snapshot]
                    .metro_segments
                    .keys()
                    .map(|segment| ((*segment), self.get_metro_segment_travelers(*segment))),
            ),
            total: Some(self.history.snapshots[snapshot].metro_segments.len()),
        }
    }

    fn iter_local_road_zones(&self) -> CongestionIterator<'_, (u64, u64)> {
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
            total: Some(total),
        }
    }

    fn iter_parking_zones(&self) -> CongestionIterator<'_, (u64, u64)> {
        let snapshot_index = self
            .history
            .get_current_snapshot_index(self.prediction_time, true);
        let total = self.history.snapshots[snapshot_index].parking.len();
        CongestionIterator {
            iterator: Box::new((0..total).map(move |i| {
                let snapshot = &self.history.snapshots[snapshot_index];
                let (x, y) = snapshot.local_zone_upscale(snapshot.local_zone_coords(i));
                ((x, y), self.get_parking(x as f64, y as f64))
            })),
            total: Some(total),
        }
    }
}

pub struct CongestionIterator<'a, K> {
    iterator: Box<dyn Iterator<Item = (K, f64)> + 'a>,
    pub total: Option<usize>,
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
            total: None,
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
