use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
    #[error("Parking not found: {0:?}")]
    ParkingNotFound(quadtree::Address),
    #[error("No terminal node found: {0:?}")]
    NoTerminalNodeFound(quadtree::Address),
}

/**
 * The querying agent has a car available.
 */
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CarConfig {
    /// departure: user has car available and can park it anywhere, including the destination
    StartWithCar,
    /// return home: user must arrive home with car, and parked it somewhere on the departing trip
    CollectParkedCar { address: quadtree::Address },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QueryInput {
    pub start: quadtree::Address,
    pub end: quadtree::Address,
    pub car_config: Option<CarConfig>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Mode {
    Walking,
    Biking,
    Driving,
}

pub static MODES: &'static [Mode] = &[Mode::Walking, Mode::Biking, Mode::Driving];

impl Mode {
    /**
     * Average speed, in m/s.
     */
    pub fn linear_speed(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 1.5,  // normal walking speed
            Biking => 6.7,   // 15mph, average biking speed
            Driving => 13.4, // 30mph, a standard city driving speed limit
        }
    }

    /**
     * Max distance it is reasonable to travel on local routes, i.e. bridging the gap beteen
     * existing nodes. This number should be relatively small to avoid making the problem
     * intractable, as edges are added between all pairs of nodes within this radius.
     */
    pub fn bridge_radius(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 800.0, // about 0.5 miles
            Biking => 3200.0, // about 2 miles
            // TODO: this seems to make fast_paths intractable for some reason
            Driving => 1000.0,
            // Driving => 8000.0, // about 5 miles
        }
    }

    /**
     * Max distance it is reasonable to travel for the first or last segment of a trip, in meters.
     * This is used for adding inferred edges in the base graph and the augmented graph.
     */
    pub fn max_radius(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 3000.0,  // about 2 miles
            Biking => 16000.0,  // about 10 miles
            Driving => 80000.0, // about 50 miles
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Mode::*;
        match self {
            Walking => write!(f, "walking"),
            Biking => write!(f, "biking"),
            Driving => write!(f, "driving"),
        }
    }
}
