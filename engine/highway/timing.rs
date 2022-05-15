use crate::segment::HighwaySegment;

// if a highway doesn't have a known speed limit, we use an assumed speed
pub const DEFAULT_SPEED: u32 = 27; // ~60 mph

// if a highway doesn't have a known number of lanes, we use an assumed number (in each direction)
pub const DEFAULT_LANES: u32 = 2;

/// the number of cars that can pass through a 1m stretch at 1m/s before congestion passes the
/// critical threshold where a significant slowdowns begins to occur
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
pub const K_EXPOENTIAL_FACTOR: f64 = 3.22;

/// we need a bound on total time to keep things from breaking
pub const MAX_CONGESTED_TIME: f64 = 3600.0 * 10.0; // 10 hours

impl HighwaySegment {
    pub fn travel_time(&self, tile_size: f64) -> f64 {
        let speed = self.data.speed_limit.unwrap_or(DEFAULT_SPEED) as f64;
        assert!(
            speed > 0.0,
            "speed for segment id {} is <= 0: {}",
            self.id,
            speed
        );
        return self.length() * tile_size / speed;
    }

    /**
     * The maxiumum number of passengers that can travel along this segment without passing the
     * inflection point after which overall flow gets exponentially worse.
     *
     * NOTE: This is a really primitive modeling of traffic flow. It is sufficient for now,
     * but could be worth investigating more sophisticated techniques in the future.
     */
    pub fn critical_capacity(&self, tile_size: u32, people_per_sim: u32) -> u32 {
        let length = self.length() * tile_size as f64; // meters
        let speed = self.data.speed_limit.unwrap_or(DEFAULT_SPEED) as f64; // meters per second
        let lanes = self.data.lanes.unwrap_or(DEFAULT_LANES) as f64;
        let car_factor = people_per_sim as f64;
        (length * speed * lanes / car_factor * K_CRITICAL_CAPACITY).ceil() as u32
    }

    pub fn congested_travel_factor(
        &self,
        tile_size: u32,
        people_per_sim: u32,
        travelers: u32,
    ) -> f64 {
        let travelers_f64 = travelers as f64;
        let critical_capacity = self.critical_capacity(tile_size, people_per_sim);
        let critical_capacity_f64 = critical_capacity as f64;

        if travelers <= critical_capacity {
            // we get linearly slower
            1.0 + travelers_f64 / critical_capacity_f64 * (K_LINEAR_FACTOR - 1.0)
        } else {
            // we get exponentially slower
            K_LINEAR_FACTOR
                + 2.0_f64.powf((travelers_f64 - critical_capacity_f64) / critical_capacity_f64)
                    * K_EXPOENTIAL_FACTOR
        }
    }

    pub fn congested_travel_time(
        &self,
        tile_size: u32,
        people_per_sim: u32,
        travelers: u32,
    ) -> f64 {
        let base_travel_time = self.travel_time(tile_size as f64);
        let factor = self.congested_travel_factor(tile_size, people_per_sim, travelers);
        assert!(factor >= 1.0);
        (base_travel_time * factor).min(MAX_CONGESTED_TIME)
    }

    pub fn is_jammed(&self, tile_size: u32, people_per_sim: u32, travelers: u32) -> bool {
        let critical_capacity = self.critical_capacity(tile_size, people_per_sim);
        travelers as f64 > critical_capacity as f64 * K_JAM_FACTOR
    }
}
