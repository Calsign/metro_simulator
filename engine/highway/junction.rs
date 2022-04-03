use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighwayJunction {
    pub id: u64,
    pub location: (f64, f64),
    pub ramp: bool,
    incoming_segments: Vec<u64>,
    outgoing_segments: Vec<u64>,
}

impl PartialEq for HighwayJunction {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for HighwayJunction {}

impl Ord for HighwayJunction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PartialOrd for HighwayJunction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl HighwayJunction {
    pub(crate) fn new(id: u64, location: (f64, f64), ramp: bool) -> Self {
        Self {
            id,
            location,
            ramp,
            incoming_segments: Vec::new(),
            outgoing_segments: Vec::new(),
        }
    }

    pub fn incoming_segments(&self) -> &Vec<u64> {
        &self.incoming_segments
    }

    pub fn outgoing_segments(&self) -> &Vec<u64> {
        &self.outgoing_segments
    }

    pub(crate) fn add_incoming(&mut self, segment_id: u64) {
        self.incoming_segments.push(segment_id);
    }

    pub(crate) fn add_outgoing(&mut self, segment_id: u64) {
        self.outgoing_segments.push(segment_id);
    }
}
