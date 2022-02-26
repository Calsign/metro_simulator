#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Quadtree error: {0}")]
    QuadtreeError(#[from] quadtree::Error),
}

#[derive(Debug)]
#[non_exhaustive]
pub struct WorldState {}

impl WorldState {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[non_exhaustive]
pub enum Mode {
    Walking,
    Biking,
    Driving,
}

static MODES: &'static [Mode] = &[Mode::Walking, Mode::Biking, Mode::Driving];

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
     * Max distance it is reasonable to travel for the first or last
     * segment of a trip, in meters.
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

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Node {
    MetroStation {
        station: metro::Station,
    },
    MetroStop {
        station: metro::Station,
        metro_line: u64,
    },
    StartNode {
        address: quadtree::Address,
    },
    EndNode {
        address: quadtree::Address,
    },
}

impl Node {
    pub fn address(&self) -> &quadtree::Address {
        use Node::*;
        match self {
            MetroStation { station } => &station.address,
            MetroStop {
                station,
                metro_line,
            } => &station.address,
            StartNode { address } | EndNode { address } => address,
        }
    }

    pub fn location(&self) -> (f64, f64) {
        use Node::*;
        match self {
            MetroStation {
                station: metro::Station { address, .. },
            }
            | MetroStop {
                station: metro::Station { address, .. },
                ..
            }
            | StartNode { address }
            | EndNode { address } => {
                let (x, y) = address.to_xy();
                (x as f64, y as f64)
            }
        }
    }
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Node::*;

        match self {
            MetroStation { station } => write!(f, "station:{}", station.name),
            MetroStop {
                station,
                metro_line,
            } => write!(f, "stop:{}:{}", metro_line, station.name),
            StartNode { .. } => write!(f, "start"),
            EndNode { .. } => write!(f, "end"),
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Edge {
    MetroSegment {
        time: f64,
    },
    MetroEmbark {
        metro_line: u64,
        station: metro::Station,
    },
    MetroDisembark {
        metro_line: u64,
        station: metro::Station,
    },
    ModeSegment {
        mode: Mode,
        distance: f64,
    },
}

impl Edge {
    pub fn cost(&self, state: &WorldState) -> f64 {
        use Edge::*;
        match self {
            MetroSegment { time } => *time,
            MetroEmbark {
                metro_line,
                station,
            } => 0.0,
            MetroDisembark {
                metro_line,
                station,
            } => 0.0,
            ModeSegment { mode, distance } => mode.linear_speed() * distance,
        }
    }
}

impl std::fmt::Display for Edge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Edge::*;
        match self {
            MetroSegment { time } => write!(f, "metro:{:.2}", time),
            MetroEmbark {
                metro_line,
                station,
            } => write!(f, "embark:{}:{}", metro_line, station.name),
            MetroDisembark {
                metro_line,
                station,
            } => write!(f, "disembark:{}:{}", metro_line, station.name),
            ModeSegment { mode, distance } => write!(f, "{}:{:.2}", mode, distance),
        }
    }
}
