use crate::HighwaySegment;

// if a highway doesn't have a known speed limit, we use an assumed speed
pub const DEFAULT_SPEED: u32 = 27; // ~60 mph

// if a highway doesn't have a known number of lanes, we use an assumed number (in each direction)
pub const DEFAULT_LANES: u32 = 2;

/// the number of cars that can pass through a 1m stretch at 1m/s before congestion passes the
/// critical threshold where a significant slowdown begins to occur
// TODO: figure out what the correct value should be
pub const K_CRITICAL_CAPACITY: f64 = 0.04;

/// factor of critical capacity at which traffic is at a virtual standstill, and no new cars can
/// enter the segment
pub const K_JAM_FACTOR: f64 = 4.0;

/// how much slower we travel at critical capacity compared to an empty highway
// this particular value means we go 10% slower
pub const K_LINEAR_FACTOR: f64 = 1.1;

/// controls how quickly traffic gets worse after the critical capacity
// this particular value means we go ~10x slower at double the critical capacity
pub const K_EXPONENTIAL_FACTOR: f64 = 3.22;

/// we need a bound on total time to keep things from breaking
pub const MAX_CONGESTED_TIME: f64 = 1200.0; // 20 minutes

pub trait HighwayTiming {
    fn travel_time(&self, tile_size: f64) -> f64;
    fn critical_capacity(&self, tile_size: u32, people_per_sim: u32) -> f64;
    fn congested_travel_factor(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> f64;
    fn congested_travel_time(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> f64;
    fn is_jammed(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> bool;
}

impl HighwayTiming for network::Segment<HighwaySegment> {
    fn travel_time(&self, tile_size: f64) -> f64 {
        let speed = self.data.speed_limit.unwrap_or(DEFAULT_SPEED) as f64;
        assert!(
            speed > 0.0,
            "speed for segment id {:?} is <= 0: {}",
            self.id,
            speed
        );
        self.length() * tile_size / speed
    }

    /**
     * The maxiumum number of passengers that can travel along this segment without passing the
     * inflection point after which overall flow gets exponentially worse.
     *
     * NOTE: This is a really primitive modeling of traffic flow. It is sufficient for now,
     * but could be worth investigating more sophisticated techniques in the future.
     */
    fn critical_capacity(&self, tile_size: u32, people_per_sim: u32) -> f64 {
        let length = self.length() * tile_size as f64; // meters
        let speed = self.data.speed_limit.unwrap_or(DEFAULT_SPEED) as f64; // meters per second
        let lanes = self.data.lanes.unwrap_or(DEFAULT_LANES) as f64;
        let car_factor = people_per_sim as f64;
        (length * speed * lanes / car_factor * K_CRITICAL_CAPACITY).ceil()
    }

    fn congested_travel_factor(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> f64 {
        let critical_capacity = self.critical_capacity(tile_size, people_per_sim);
        congested_travel_factor(critical_capacity, travelers)
    }

    fn congested_travel_time(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> f64 {
        let base_travel_time = self.travel_time(tile_size as f64);
        let factor = self.congested_travel_factor(tile_size, people_per_sim, travelers);
        assert!(factor >= 1.0);
        (base_travel_time * factor).min(MAX_CONGESTED_TIME)
    }

    fn is_jammed(&self, tile_size: u32, people_per_sim: u32, travelers: f64) -> bool {
        let critical_capacity = self.critical_capacity(tile_size, people_per_sim);
        is_jammed(critical_capacity, travelers)
    }
}

pub fn congested_travel_factor(critical_capacity: f64, travelers: f64) -> f64 {
    assert!(critical_capacity.is_normal(), "{}", critical_capacity);
    assert!(
        !travelers.is_nan() && travelers.is_finite(),
        "{}",
        travelers
    );

    if travelers <= critical_capacity {
        // we get linearly slower
        1.0 + travelers / critical_capacity * (K_LINEAR_FACTOR - 1.0)
    } else {
        // we get exponentially slower
        K_LINEAR_FACTOR
            + 2.0_f64.powf((travelers - critical_capacity) / critical_capacity)
                * K_EXPONENTIAL_FACTOR
    }
}

pub fn is_jammed(critical_capacity: f64, travelers: f64) -> bool {
    travelers > critical_capacity * K_JAM_FACTOR
}
