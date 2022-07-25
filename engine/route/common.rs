use serde::{Deserialize, Serialize};

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
    #[error("Parking not found: {0:?}")]
    ParkingNotFound(quadtree::Address),
    #[error("No terminal node found: {0:?}")]
    NoTerminalNodeFound(quadtree::Address),
    #[error("Spade (Delaunay Triangulation) error: {0:?}")]
    SpadeError(#[from] spade::InsertionError),
    #[error("Edge counting inconsistency error: {0}")]
    EdgeCountingError(String),
    #[error("Parking error: {0}")]
    ParkingError(String),
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

pub static MODES: &[Mode] = &[Mode::Walking, Mode::Biking, Mode::Driving];

impl Mode {
    /**
     * Average speed, in m/s.
     */
    pub fn linear_speed(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 1.5,  // normal walking speed
            Biking => 6.7,   // 15mph, average biking speed
            Driving => 11.2, // 25mph, a standard city driving speed limit
        }
    }

    /**
     * Max distance it is reasonable to travel on local routes, i.e. bridging the gap beteen
     * existing nodes. Inferred edges are added for each edge in the Delaunay triangulation of
     * available nodes for each mode, as long as the edge length is within this bridge radius.
     */
    pub fn bridge_radius(&self) -> f64 {
        use Mode::*;
        match self {
            Walking => 800.0,  // about 0.5 miles
            Biking => 3200.0,  // about 2 miles
            Driving => 8000.0, // about 5 miles
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeMap<T> {
    data: [T; 3],
}

impl<T> ModeMap<T> {
    pub fn new<F>(mut init: F) -> Self
    where
        F: FnMut(Mode) -> T,
    {
        let data: Vec<T> = MODES.iter().map(|mode| init(*mode)).collect();
        Self {
            data: data
                .try_into()
                .unwrap_or_else(|_| panic!("should be impossible")),
        }
    }
}

impl<T> std::ops::Index<Mode> for ModeMap<T> {
    type Output = T;
    fn index(&self, mode: Mode) -> &T {
        &self.data[mode as usize]
    }
}

impl<T> std::ops::IndexMut<Mode> for ModeMap<T> {
    fn index_mut(&mut self, mode: Mode) -> &mut T {
        &mut self.data[mode as usize]
    }
}

#[cfg(test)]
mod tests {
    use crate::common::*;

    #[test]
    fn mode_map() {
        let map = ModeMap::new(|mode| match mode {
            Mode::Walking => 0,
            Mode::Biking => 1,
            Mode::Driving => 2,
        });
        assert_eq!(map[Mode::Walking], 0);
        assert_eq!(map[Mode::Biking], 1);
        assert_eq!(map[Mode::Driving], 2);
    }
}
