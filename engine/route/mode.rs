pub enum Mode {
    Walking,
    Biking,
    Driving,
}

impl Mode {
    /**
     * Average speed, in m/s.
     */
    fn linear_speed(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 1.5,  // normal walking speed
            Biking => 6.7,   // 15mph, average biking speed
            Driving => 13.4, // 30mph, a standard city driving speed limit
        }
    }

    /**
     * Max distance it is reasonable to travel for the first or last
     * segment of a trip, in meters.
     */
    fn max_radius(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 3000.0,  // about 2 miles
            Biking => 16000.0,  // about 10 miles
            Driving => 80000.0, // about 50 miles
        }
    }
}
