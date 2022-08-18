use serde::{Deserialize, Serialize};

use crate::network::Key;
use crate::segment::SegmentHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JunctionHandle(pub(crate) u64);

impl JunctionHandle {
    pub fn inner(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction<T> {
    pub id: JunctionHandle,
    pub data: T,
    pub location: Key,
    incoming: Vec<SegmentHandle>,
    outgoing: Vec<SegmentHandle>,
}

id_cmp::id_cmp!(Junction<T>, id, T);

impl<T> Junction<T> {
    pub(crate) fn new<K>(id: JunctionHandle, location: K, data: T) -> Self
    where
        K: Into<Key>,
    {
        Self {
            id,
            data,
            location: location.into(),
            incoming: Vec::new(),
            outgoing: Vec::new(),
        }
    }

    pub fn incoming_segments(&self) -> &[SegmentHandle] {
        &self.incoming
    }

    pub fn outgoing_segments(&self) -> &[SegmentHandle] {
        &self.outgoing
    }

    pub(crate) fn add_incoming(&mut self, id: SegmentHandle) {
        self.incoming.push(id);
    }

    pub(crate) fn add_outgoing(&mut self, id: SegmentHandle) {
        self.outgoing.push(id);
    }

    pub fn address(&self, max_depth: u32) -> quadtree::Address {
        let (x, y) = self.location.into();
        quadtree::Address::from_xy(x as u64, y as u64, max_depth)
    }
}
