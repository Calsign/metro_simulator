use serde::{Deserialize, Serialize};

use crate::change_state::{ChangeState, WithChangeState};
use crate::network::{Handle, Key, WithHandle};
use crate::segment::SegmentHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JunctionHandle(pub(crate) u64);

impl JunctionHandle {
    pub fn inner(&self) -> u64 {
        self.0
    }
}

impl Handle for JunctionHandle {
    fn create(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Junction<T> {
    pub id: JunctionHandle,
    pub data: T,
    pub location: Key,
    incoming: Vec<SegmentHandle>,
    outgoing: Vec<SegmentHandle>,
    pub change_state: ChangeState,
}

id_cmp::id_cmp!(Junction<T>, id, T);

impl<T: Clone> WithHandle<JunctionHandle> for Junction<T> {
    fn get_id(&self) -> JunctionHandle {
        self.id
    }

    fn clone_new_id(&self, id: JunctionHandle) -> Self {
        let mut ret = (*self).clone();
        ret.id = id;
        ret
    }
}

impl<T> WithChangeState for Junction<T> {
    fn change_state(&self) -> &ChangeState {
        &self.change_state
    }

    fn change_state_mut(&mut self) -> &mut ChangeState {
        &mut self.change_state
    }
}

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
            change_state: ChangeState::Active,
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

    /// O(n), but the lists should be very small
    pub(crate) fn remove_incoming(&mut self, id: SegmentHandle) {
        self.incoming.retain(|incoming| *incoming != id);
    }

    /// O(n), but the lists should be very small
    pub(crate) fn remove_outgoing(&mut self, id: SegmentHandle) {
        self.outgoing.retain(|outgoing| *outgoing != id);
    }

    pub fn address(&self, max_depth: u32) -> quadtree::Address {
        let (x, y) = self.location.into();
        quadtree::Address::from_xy(x as u64, y as u64, max_depth)
    }
}
