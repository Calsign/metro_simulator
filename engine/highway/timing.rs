use crate::segment::HighwaySegment;

// if a highway doesn't have a known speed limit, we use an assumed speed
const DEFAULT_SPEED: u32 = 27; // ~60 mph

pub fn travel_time(segment: &HighwaySegment, tile_size: f64) -> f64 {
    let speed = segment.data.speed_limit.unwrap_or(DEFAULT_SPEED) as f64;
    assert!(
        speed > 0.0,
        "speed for segment id {} is <= 0: {}",
        segment.id,
        speed
    );
    return segment.length() * tile_size / speed;
}
