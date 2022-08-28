use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::junction::{Junction, JunctionHandle};
use crate::network::{Handle, ManagedMap, Network, WithHandle};
use crate::segment::{Segment, SegmentHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeState {
    /// currently in use, unaffected by staged changes
    Active,
    /// staged for creation
    StagedActive,
    /// still in use, but staged for removal
    StagedTombstone,
    /// ready to be removed, but waiting for remaining routes to finish first
    /// countdown is the number of days until removal
    Tombstone { countdown: u32 },
}

impl ChangeState {
    /// Should this item be used for constructing the active base graph?
    pub fn is_active(&self) -> bool {
        match self {
            Self::Active | Self::StagedTombstone => true,
            Self::StagedActive | Self::Tombstone { .. } => false,
        }
    }

    /// Is this item part of staged changes?
    pub fn is_staged_change(&self) -> bool {
        match self {
            Self::StagedActive | Self::StagedTombstone => true,
            Self::Active | Self::Tombstone { .. } => false,
        }
    }

    /// Will this item be active if the change set is applied?
    pub fn is_staged_active(&self) -> bool {
        match self {
            Self::Active | Self::StagedActive => true,
            Self::StagedTombstone | Self::Tombstone { .. } => false,
        }
    }
}

pub(crate) trait WithChangeState {
    fn change_state(&self) -> &ChangeState;
    fn change_state_mut(&mut self) -> &mut ChangeState;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeSet<T: Eq + std::hash::Hash> {
    /// items with state StagedActive
    created: HashSet<T>,
    /// items with state StagedTombstone
    removed: HashSet<T>,
}

impl<T: Eq + std::hash::Hash> ChangeSet<T> {
    fn new() -> Self {
        Self {
            created: HashSet::new(),
            removed: HashSet::new(),
        }
    }

    pub fn created(&self) -> &HashSet<T> {
        &self.created
    }

    pub fn removed(&self) -> &HashSet<T> {
        &self.removed
    }
}

// TODO: I can't get the visibility right with this as a method
// (it complains about leaking the type Handle)
fn edit_changeset<'a, 'b, T: Handle, U: WithHandle<T> + WithChangeState>(
    change_set: &'a mut ChangeSet<T>,
    id: T,
    items: &'b mut ManagedMap<T, U>,
) -> T {
    assert!(!change_set.removed.contains(&id));
    assert!(items.get(id).change_state().is_staged_active());
    let ret_id = if change_set.created.contains(&id) {
        // this item is already part of the change set, so keep using it
        id
    } else {
        // duplicate it and switch things over
        let new_id = items.clone_item(id);
        change_set.created.insert(new_id);
        change_set.removed.insert(id);
        *items.get_mut(new_id).change_state_mut() = ChangeState::StagedActive;
        *items.get_mut(id).change_state_mut() = ChangeState::StagedTombstone;
        new_id
    };
    ret_id
}

fn apply_change_set<T: Handle, U: WithHandle<T> + WithChangeState>(
    change_set: &mut ChangeSet<T>,
    items: &mut ManagedMap<T, U>,
) {
    // create all the new items
    for created in change_set.created.drain() {
        let item = items.get_mut(created);
        assert_eq!(*item.change_state(), ChangeState::StagedActive);
        *item.change_state_mut() = ChangeState::Active;
    }

    // mark the old items as tombstones (will be removed later as part of AdvanceNetworktombstones)
    for removed in change_set.removed.drain() {
        let item = items.get_mut(removed);
        assert_eq!(*item.change_state(), ChangeState::StagedTombstone);
        // Delete after 2 days; a route may cross from one day to the next, but it will never cross
        // into the following day (i.e. last more than one full day).
        *item.change_state_mut() = ChangeState::Tombstone { countdown: 2 };
    }
}

fn clear_change_set<T: Handle, U: WithHandle<T> + WithChangeState>(
    change_set: &mut ChangeSet<T>,
    items: &mut ManagedMap<T, U>,
) -> Vec<T> {
    // remove all the to-be-created items
    let to_remove: Vec<T> = change_set.created.drain().collect();
    for item in &to_remove {
        assert_eq!(*items.get(*item).change_state(), ChangeState::StagedActive);
    }

    // revert all the to-be-removed items
    for removed in change_set.removed.drain() {
        let item = items.get_mut(removed);
        assert_eq!(*item.change_state(), ChangeState::StagedTombstone);
        *item.change_state_mut() = ChangeState::Active;
    }

    to_remove
}

/// Reduce each tombstone counter by one. This is intended to be called once per day. Returns list
/// of expired item handles, which should be removed.
fn advance_tombstones<T: Handle, U: WithHandle<T> + WithChangeState>(
    items: &mut ManagedMap<T, U>,
) -> Vec<T> {
    // advance tombstones
    for item in items.inner.values_mut() {
        match item.change_state_mut() {
            ChangeState::Tombstone { countdown } => *countdown -= 1,
            _ => (),
        }
    }

    // find expired tombstones
    let to_remove: Vec<T> = items
        .inner
        .iter()
        .filter_map(|(id, item)| match item.change_state() {
            ChangeState::Tombstone { countdown: 0 } => Some(*id),
            _ => None,
        })
        .collect();

    to_remove
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkChangeSet {
    pub junctions: ChangeSet<JunctionHandle>,
    pub segments: ChangeSet<SegmentHandle>,
}

impl NetworkChangeSet {
    pub fn new() -> Self {
        Self {
            junctions: ChangeSet::new(),
            segments: ChangeSet::new(),
        }
    }
}

impl<J: Clone, S: Clone> Network<J, S> {
    pub fn edit_junction(&mut self, id: JunctionHandle) -> &mut Junction<J> {
        let ret_id = edit_changeset(&mut self.change_set.junctions, id, &mut self.junctions);
        if ret_id != id {
            // if we just forked this junction, we also need to fork its adjacent segments

            // TODO: fix temp vector to satisfy borrow checker
            let incoming_segments: Vec<SegmentHandle> = self
                .junction(ret_id)
                .incoming_segments()
                .iter()
                .copied()
                .collect();
            for incoming in incoming_segments {
                // remove segment from the old junction
                self.junction_mut(ret_id).remove_incoming(incoming);

                if self.segments.get(incoming).change_state.is_staged_active() {
                    let segment_id =
                        edit_changeset(&mut self.change_set.segments, incoming, &mut self.segments);

                    if segment_id == incoming {
                        // if we have already edited this segment, need to detach it from the other
                        // old junction
                        self.junction_mut(self.segment(segment_id).end)
                            .remove_incoming(segment_id);
                    }

                    if self
                        .junction(ret_id)
                        .outgoing_segments()
                        .contains(&incoming)
                    {
                        // this is a loop - need to update the start as well
                        self.junction_mut(self.segment(segment_id).start)
                            .remove_outgoing(segment_id);
                        self.segment_mut(segment_id).start = ret_id;
                        self.junction_mut(ret_id).add_outgoing(segment_id);
                    }

                    // patch things up
                    self.segment_mut(segment_id).end = ret_id;
                    self.junction_mut(ret_id).add_incoming(segment_id);
                    self.junctions
                        .get_mut(self.segment(segment_id).start)
                        .add_outgoing(segment_id);
                }
            }

            let outgoing_segments: Vec<SegmentHandle> = self
                .junction(ret_id)
                .outgoing_segments()
                .iter()
                .copied()
                .collect();
            for outgoing in outgoing_segments {
                // remove segment from the old junction
                self.junction_mut(ret_id).remove_outgoing(outgoing);

                if self.segments.get(outgoing).change_state.is_staged_active() {
                    let segment_id =
                        edit_changeset(&mut self.change_set.segments, outgoing, &mut self.segments);

                    if segment_id == outgoing {
                        // if we have already edited this segment, need to detach it from the other
                        // old junction
                        self.junction_mut(self.segment(segment_id).start)
                            .remove_outgoing(segment_id);
                    }

                    // patch things up
                    self.segment_mut(segment_id).start = ret_id;
                    self.junction_mut(ret_id).add_outgoing(segment_id);
                    self.junctions
                        .get_mut(self.segment(segment_id).end)
                        .add_incoming(segment_id);
                }
            }
        }
        self.junctions.get_mut(ret_id)
    }

    pub fn edit_segment(&mut self, id: SegmentHandle) -> &mut Segment<S> {
        let ret_id = edit_changeset(&mut self.change_set.segments, id, &mut self.segments);
        let ret = self.segments.get_mut(ret_id);
        // patch up the endpoints
        self.junctions.get_mut(ret.start).add_outgoing(ret.id);
        self.junctions.get_mut(ret.end).add_incoming(ret.id);
        ret
    }

    pub fn apply_change_set(&mut self) {
        apply_change_set(&mut self.change_set.junctions, &mut self.junctions);
        apply_change_set(&mut self.change_set.segments, &mut self.segments);
    }

    pub fn clear_change_set(&mut self) {
        // NOTE: it is important to remove segments first
        for segment in clear_change_set(&mut self.change_set.segments, &mut self.segments) {
            self.remove_segment(segment);
        }
        for junction in clear_change_set(&mut self.change_set.junctions, &mut self.junctions) {
            self.remove_junction(junction);
        }
    }

    pub fn advance_tombstones(&mut self) {
        // NOTE: it is important to remove segments first
        for segment in advance_tombstones(&mut self.segments) {
            self.remove_segment(segment);
        }
        for junction in advance_tombstones(&mut self.junctions) {
            self.remove_junction(junction);
        }
    }
}
