use serde::{Deserialize, Serialize};

// time it takes to wait for a train, on average
// TODO: replace this with correct accounting for train schedules
pub const EMBARK_TIME: f64 = 480.0;
// time it takes to enter or leave a highway
pub const RAMP_TIME: f64 = 30.0;

pub const MAX_COST: f64 = f64::INFINITY;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
    #[error("Parking not found: {0:?}")]
    ParkingNotFound(quadtree::Address),
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

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct WorldState {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct QueryInput {
    pub start: quadtree::Address,
    pub end: quadtree::Address,
    pub car_config: Option<CarConfig>,
    pub start_time: u64,
}

impl WorldState {
    pub fn new() -> Self {
        Self {}
    }
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
            Walking => 800.0,  // about 0.5 miles
            Biking => 3200.0,  // about 2 miles
            Driving => 8000.0, // about 5 miles
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Node {
    StartNode,
    EndNode,
    MetroStation {
        station: metro::Station,
    },
    MetroStop {
        station: metro::Station,
        metro_line: u64,
    },
    HighwayJunction {
        position: (f64, f64),
        address: quadtree::Address,
    },
    HighwayRamp {
        position: (f64, f64),
        address: quadtree::Address,
    },
    Parking {
        address: quadtree::Address,
    },
}

impl Node {
    pub fn address(&self, input: &QueryInput) -> quadtree::Address {
        use Node::*;
        match self {
            StartNode => input.start,
            EndNode => input.end,
            MetroStation {
                station: metro::Station { address, .. },
            }
            | MetroStop {
                station: metro::Station { address, .. },
                ..
            }
            | HighwayJunction { address, .. }
            | HighwayRamp { address, .. }
            | Parking { address } => *address,
        }
    }

    pub fn location(&self, input: &QueryInput) -> (f64, f64) {
        use Node::*;
        match self {
            StartNode => {
                let (x, y) = input.start.to_xy();
                (x as f64, y as f64)
            }
            EndNode => {
                let (x, y) = input.end.to_xy();
                (x as f64, y as f64)
            }
            MetroStation {
                station: metro::Station { address, .. },
            }
            | MetroStop {
                station: metro::Station { address, .. },
                ..
            }
            | Parking { address } => {
                let (x, y) = address.to_xy();
                (x as f64, y as f64)
            }
            HighwayJunction { position, .. } | HighwayRamp { position, .. } => *position,
        }
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Node::*;

        match self {
            StartNode => write!(f, "start"),
            EndNode => write!(f, "end"),
            MetroStation { station } => write!(f, "station:{}", station.name),
            MetroStop {
                station,
                metro_line,
            } => write!(f, "stop:{}:{}", metro_line, station.name),
            HighwayJunction {
                position: (x, y), ..
            } => write!(f, "junction:({:.1}, {:.1})", x, y),
            HighwayRamp {
                position: (x, y), ..
            } => write!(f, "ramp:({:.1}, {:.1})", x, y),
            Parking { .. } => write!(f, "parking"),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Edge {
    MetroSegment {
        metro_line: u64,
        time: f64,
        start: quadtree::Address,
        stop: quadtree::Address,
    },
    MetroEmbark {
        metro_line: u64,
        station: metro::Station,
    },
    MetroDisembark {
        metro_line: u64,
        station: metro::Station,
    },
    Highway {
        segment: u64,
        data: highway::HighwayData,
        time: f64,
    },
    HighwayRamp {
        position: (f64, f64),
    },
    ModeSegment {
        mode: Mode,
        distance: f64,
    },
    ModeTransition {
        from: Mode,
        to: Mode,
    },
    StartSegment {
        mode: Mode,
        location: (f64, f64),
    },
    EndSegment {
        mode: Mode,
        location: (f64, f64),
    },
    ParkCarSegment,
    CollectParkedCarSegment {
        address: quadtree::Address,
    },
}

fn u64_f64_point_dist(a: (f64, f64), (bx, by): (u64, u64)) -> f64 {
    use cgmath::MetricSpace;
    cgmath::Vector2::from(a).distance((bx as f64, by as f64).into())
}

impl Edge {
    pub fn cost(&self, input: &QueryInput, state: &WorldState, tile_size: f64) -> f64 {
        use Edge::*;
        match self {
            MetroSegment { time, .. } => *time,
            MetroEmbark {
                metro_line,
                station,
            } => {
                // TODO: properly account for train schedules
                EMBARK_TIME
            }
            MetroDisembark {
                metro_line,
                station,
            } => 0.0,
            Highway { time, .. } => *time,
            HighwayRamp { .. } => RAMP_TIME,
            ModeSegment { mode, distance } => distance / mode.linear_speed(),
            ModeTransition { .. } => 0.0,
            StartSegment { mode, location } => {
                let dist = u64_f64_point_dist(*location, input.start.to_xy()) * tile_size
                    / mode.linear_speed();
                match (input.car_config, mode) {
                    (Some(CarConfig::StartWithCar), Mode::Walking | Mode::Driving) => dist,
                    (_, Mode::Walking) => dist,
                    _ => MAX_COST,
                }
            }
            EndSegment { mode, location } => {
                let dist = u64_f64_point_dist(*location, input.end.to_xy()) * tile_size
                    / mode.linear_speed();
                match (input.car_config, mode) {
                    (Some(CarConfig::StartWithCar), Mode::Walking | Mode::Driving) => dist,
                    (Some(CarConfig::CollectParkedCar { .. }), Mode::Driving) => dist,
                    (None, Mode::Walking) => dist,
                    _ => MAX_COST,
                }
            }
            ParkCarSegment {} => match &input.car_config {
                Some(CarConfig::StartWithCar) => 0.0,
                _ => MAX_COST,
            },
            CollectParkedCarSegment { address } => match &input.car_config {
                Some(CarConfig::CollectParkedCar {
                    address: parked_address,
                }) if address == parked_address => 0.0,
                _ => MAX_COST,
            },
        }
    }
}

impl std::fmt::Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Edge::*;
        match self {
            MetroSegment {
                metro_line, time, ..
            } => write!(f, "metro:{}:{:.2}", metro_line, time),
            MetroEmbark {
                metro_line,
                station,
            } => write!(f, "embark:{}:{}", metro_line, station.name),
            MetroDisembark {
                metro_line,
                station,
            } => write!(f, "disembark:{}:{}", metro_line, station.name),
            Highway {
                segment,
                data,
                time,
            } => {
                let name = data.name.clone().unwrap_or("unknown".to_string());
                let refs = data.refs.join(";");
                write!(f, "highway:{}:{}:{}:{:.2}", segment, name, refs, time)
            }
            HighwayRamp { .. } => write!(f, "ramp"),
            ModeSegment { mode, distance } => {
                write!(
                    f,
                    "{}:{:.2}m:{:.2}s",
                    mode,
                    distance,
                    distance / mode.linear_speed(),
                )
            }
            ModeTransition { from, to } => write!(f, "{}->{}", from, to),
            StartSegment {
                mode,
                location: (x, y),
            } => write!(f, "start->({},{}):{}", x, y, mode),
            EndSegment {
                mode,
                location: (x, y),
            } => write!(f, "({},{})->end:{}", x, y, mode),
            ParkCarSegment {} => write!(f, "park_car"),
            CollectParkedCarSegment { address } => {
                let (x, y) = address.to_xy();
                write!(f, "collect_parked_car:({},{})", x, y)
            }
        }
    }
}
