use serde::{Deserialize, Serialize};

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

    pub fn update(&mut self, elapsed: f64) {
        if !self.paused {
            // NOTE: a small loss of precision, but shouldn't be noticeable
            self.current_time += (self.playback_rate as f64 * elapsed) as u64;
        }
    }

    pub fn current_date_time(&self) -> chrono::NaiveDateTime {
        chrono::NaiveDateTime::from_timestamp(
            (self.engine_start_time + self.current_time) as i64,
            0,
        )
    }
}
