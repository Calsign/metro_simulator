use serde::{Deserialize, Serialize};
use uom::si::time::hour;
use uom::si::u64::Time;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeState {
    /// current time in seconds since the beginning of the simulation
    pub current_time: u64,
    /// number of simulated seconds advanced per real-world second
    pub playback_rate: u64,
    /// whether the simulation is currently paused
    pub paused: bool,
    /// the number of seconds since the epoch for the beginning of the simulation
    pub engine_start_time: u64,
}

impl TimeState {
    pub fn new() -> Self {
        Self {
            current_time: 0,
            playback_rate: 300, // 5 minutes per second
            paused: true,
            // TODO: make this configurable in the map
            engine_start_time: chrono::NaiveDate::from_ymd(2020, 1, 1)
                .and_hms(0, 0, 0)
                .timestamp() as u64,
        }
    }

    pub fn current_date_time(&self) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::from_timestamp(
            (self.engine_start_time + self.current_time) as i64,
            0,
        )
    }

    pub fn time_from_datetime(&self, datetime: chrono::NaiveDateTime) {
        unimplemented!()
    }

    pub fn should_render_motion(&self) -> bool {
        // NOTE: could use some tweaking?
        self.playback_rate < Time::new::<hour>(2).value
    }
}