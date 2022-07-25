// NOTE: in the future we may extend this to support walking and biking as well

/// The width of a single block in the local zone grid, in meters.
pub const LOCAL_ZONE_BLOCK_SIZE: f32 = 500.0;

/// the number of cars that can pass through a 1x1 m square before congestion passes the critical
/// threshold where a significant slowdown begins to occur
pub const K_CRITICAL_CAPACITY: f64 = 0.05;

pub fn grid_downsample(config: &state::Config) -> u32 {
    config.even_downsample(LOCAL_ZONE_BLOCK_SIZE)
}

pub fn critical_capacity(config: &state::Config) -> f64 {
    ((config.min_tile_size * grid_downsample(config)).pow(2) / config.people_per_sim) as f64
        * K_CRITICAL_CAPACITY
}

pub fn congested_travel_factor(config: &state::Config, travelers: f64) -> f64 {
    highway::timing::congested_travel_factor(critical_capacity(config), travelers)
}

pub fn congested_travel_time(base_travel_time: f64, config: &state::Config, travelers: f64) -> f64 {
    let traffic_factor = congested_travel_factor(config, travelers);
    assert!(traffic_factor >= 0.999, "{}", traffic_factor);
    (base_travel_time * traffic_factor).min(highway::timing::MAX_CONGESTED_TIME)
}

pub fn is_jammed(config: &state::Config, travelers: f64) -> bool {
    let traffic_factor = congested_travel_factor(config, travelers);
    highway::timing::is_jammed(traffic_factor, travelers)
}
