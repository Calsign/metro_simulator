use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub address: quadtree::Address,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct RailwayJunction {
    pub station: Option<Station>,
}

impl RailwayJunction {
    pub fn new(station: Option<Station>) -> Self {
        Self { station }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct RailwaySegment {
    pub speed_limit: Option<u32>,
}

impl RailwaySegment {
    pub fn new(speed_limit: Option<u32>) -> Self {
        Self { speed_limit }
    }
}

pub type Railways = network::Network<RailwayJunction, RailwaySegment>;

pub trait RailwayTiming {
    fn timing_config(
        &self,
        speed_limit: u32,
        tile_size: f64,
        railways: &Railways,
    ) -> network::TimingConfig;
    fn railway_travel_time(&self, speed_limit: u32, tile_size: f64, railways: &Railways) -> f64;
    fn railway_dist_spline(
        &self,
        speed_limit: u32,
        tile_size: f64,
        railways: &Railways,
    ) -> &splines::Spline<f64, f64>;
}

impl RailwayTiming for network::Segment<RailwaySegment> {
    fn timing_config(
        &self,
        speed_limit: u32,
        tile_size: f64,
        railways: &Railways,
    ) -> network::TimingConfig {
        let max_speed = speed_limit.min(self.data.speed_limit.unwrap_or(u32::MAX)) as f64;
        // TODO: account for travel time around corners between segments?
        // TODO: this is probably less good than the old approach
        network::TimingConfig {
            tile_size,
            max_speed,
            max_acceleration: 1.5,
            start_speed: railways
                .junction(self.start_junction())
                .data
                .station
                .as_ref()
                .map(|_| 0.0)
                .unwrap_or(max_speed),
            end_speed: railways
                .junction(self.end_junction())
                .data
                .station
                .as_ref()
                .map(|_| 0.0)
                .unwrap_or(max_speed),
        }
    }

    fn railway_travel_time(&self, speed_limit: u32, tile_size: f64, railways: &Railways) -> f64 {
        self.travel_time(self.timing_config(speed_limit, tile_size, railways))
    }

    fn railway_dist_spline(
        &self,
        speed_limit: u32,
        tile_size: f64,
        railways: &Railways,
    ) -> &splines::Spline<f64, f64> {
        self.dist_spline(self.timing_config(speed_limit, tile_size, railways))
    }
}
