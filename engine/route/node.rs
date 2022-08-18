use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Node {
    MetroStation {
        station: metro::Station,
    },
    MetroStop {
        station: metro::Station,
        metro_line: metro::MetroLineHandle,
    },
    RailJunction {
        junction: network::JunctionHandle,
        address: quadtree::Address,
    },
    HighwayJunction {
        junction: network::JunctionHandle,
        position: (f64, f64),
        address: quadtree::Address,
    },
    HighwayRamp {
        junction: network::JunctionHandle,
        position: (f64, f64),
        address: quadtree::Address,
    },
    Parking {
        address: quadtree::Address,
    },
    Endpoint {
        address: quadtree::Address,
    },
}

impl Node {
    pub fn address(&self) -> quadtree::Address {
        use Node::*;
        match self {
            MetroStation {
                station: metro::Station { address, .. },
            }
            | MetroStop {
                station: metro::Station { address, .. },
                ..
            }
            | RailJunction { address, .. }
            | HighwayJunction { address, .. }
            | HighwayRamp { address, .. }
            | Parking { address }
            | Endpoint { address } => *address,
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
            | RailJunction { address, .. }
            | Parking { address }
            | Endpoint { address } => {
                let (x, y) = address.to_xy();
                (x as f64, y as f64)
            }
            HighwayJunction { position, .. } | HighwayRamp { position, .. } => *position,
        }
    }

    pub fn location_f32(&self) -> (f32, f32) {
        let (x, y) = self.location();
        (x as f32, y as f32)
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
            } => write!(f, "stop:{:?}:{}", metro_line, station.name),
            RailJunction { .. } => write!(f, "junction"),
            HighwayJunction {
                position: (x, y), ..
            } => write!(f, "junction:({:.1}, {:.1})", x, y),
            HighwayRamp {
                position: (x, y), ..
            } => write!(f, "ramp:({:.1}, {:.1})", x, y),
            Parking { address } => {
                let (x, y) = address.to_xy_f64();
                write!(f, "parking:({:.1}, {:.1})", x, y)
            }
            Endpoint { address } => {
                let (x, y) = address.to_xy_f64();
                write!(f, "endpoint:({:.1}, {:.1})", x, y)
            }
        }
    }
}
