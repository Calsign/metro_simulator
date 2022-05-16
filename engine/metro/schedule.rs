use serde::{Deserialize, Serialize};

/**
 * Represents a metro schedule.
 *
 * For now this is very rudimentary, but in the future the intention is for this to be able to
 * support more complex schedules.
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    /// seconds between each departure, starting from the beginning of the simulation
    fixed_frequency: u64,
}

impl Schedule {
    pub fn fixed_frequency(fixed_frequency: u64) -> Self {
        assert!(fixed_frequency > 0);
        Self { fixed_frequency }
    }

    /**
     * Given the current timestamp (seconds since the start of the simulation), returns the
     * timestamp of the next departure. If the current time is a departure time, will return
     * the *next* departure, i.e. the returned value is always in the future.
     */
    pub fn next_departure(&self, current_time: u64) -> u64 {
        let remaining = current_time % self.fixed_frequency;
        current_time + (self.fixed_frequency - remaining)
    }

    /**
     * The expected value of the time to wait for the next train.
     */
    pub fn expected_waiting_time(&self) -> u64 {
        self.fixed_frequency / 2
    }
}

#[cfg(test)]
mod tests {
    use crate::schedule::*;

    #[test]
    fn fixed_frequency_test() {
        let schedule = Schedule::fixed_frequency(60);
        assert_eq!(schedule.next_departure(0), 60);
        assert_eq!(schedule.next_departure(1), 60);
        assert_eq!(schedule.next_departure(30), 60);
        assert_eq!(schedule.next_departure(59), 60);
        assert_eq!(schedule.next_departure(60), 120);
        assert_eq!(schedule.next_departure(100), 120);
        assert_eq!(schedule.next_departure(200), 240);
    }
}
