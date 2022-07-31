use serde::{Deserialize, Serialize};

#[derive(PartialEq, Eq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum RampDirection {
    OnRamp,
    OffRamp,
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct HighwayJunction {
    pub ramp: Option<RampDirection>,
}

impl HighwayJunction {
    pub fn new(ramp: Option<RampDirection>) -> Self {
        Self { ramp }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Serialize, Deserialize)]
pub struct HighwaySegment {
    pub name: Option<String>,
    pub refs: Vec<String>,
    pub lanes: Option<u32>,
    pub speed_limit: Option<u32>,
}

impl HighwaySegment {
    pub fn new(
        name: Option<String>,
        refs: Vec<String>,
        lanes: Option<u32>,
        speed_limit: Option<u32>,
    ) -> Self {
        Self {
            name,
            refs,
            lanes,
            speed_limit,
        }
    }
}

pub type Highways = network::Network<HighwayJunction, HighwaySegment>;
