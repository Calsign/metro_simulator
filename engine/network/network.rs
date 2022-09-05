use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::change_state::{NetworkChangeSet, WithChangeState};
use crate::junction::{Junction, JunctionHandle};
use crate::segment::{Segment, SegmentHandle};

pub type Key = cgmath::Vector2<f64>;

pub(crate) trait Handle: Copy + std::cmp::Ord + std::hash::Hash + Eq {
    fn create(id: u64) -> Self;
}

pub(crate) trait WithHandle<H: Handle> {
    fn get_id(&self) -> H;
    fn clone_new_id(&self, id: H) -> Self;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ManagedMap<K: Handle, V: WithHandle<K>> {
    pub(crate) inner: BTreeMap<K, V>,
    counter: u64,
}

impl<K: Handle, V: WithHandle<K>> ManagedMap<K, V> {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
            counter: 0,
        }
    }

    pub fn add<F>(&mut self, value_f: F) -> K
    where
        F: FnOnce(K) -> V,
    {
        let id = Handle::create(self.counter);
        self.counter += 1;
        self.inner.insert(id, value_f(id));
        id
    }

    fn remove(&mut self, id: K) -> V {
        // TODO: in the future, it could be useful to add some tracking here for debugging issues
        // with removed items
        match self.inner.remove(&id) {
            Some(removed) => removed,
            None => panic!("attempt to remove invalid handle"),
        }
    }

    pub fn clone_item(&mut self, id: K) -> K {
        let new_id = Handle::create(self.counter);
        self.counter += 1;
        self.inner.insert(new_id, self.get(id).clone_new_id(new_id));
        new_id
    }

    pub fn get(&self, id: K) -> &V {
        self.inner.get(&id).expect("invalid handle")
    }

    pub fn get_mut(&mut self, id: K) -> &mut V {
        self.inner.get_mut(&id).expect("invalid handle")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network<J: Clone, S: Clone> {
    pub(crate) junctions: ManagedMap<JunctionHandle, Junction<J>>,
    pub(crate) segments: ManagedMap<SegmentHandle, Segment<S>>,
    pub(crate) change_set: NetworkChangeSet,
}

impl<J: Clone, S: Clone> Default for Network<J, S> {
    fn default() -> Self {
        Self {
            junctions: ManagedMap::new(),
            segments: ManagedMap::new(),
            change_set: NetworkChangeSet::new(),
        }
    }
}

impl<J: Clone, S: Clone> Network<J, S> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn junction(&self, id: JunctionHandle) -> &Junction<J> {
        self.junctions.get(id)
    }

    pub fn junction_mut(&mut self, id: JunctionHandle) -> &mut Junction<J> {
        self.junctions.get_mut(id)
    }

    pub fn segment(&self, id: SegmentHandle) -> &Segment<S> {
        self.segments.get(id)
    }

    pub fn segment_mut(&mut self, id: SegmentHandle) -> &mut Segment<S> {
        self.segments.get_mut(id)
    }

    pub fn add_junction<K>(&mut self, location: K, data: J) -> JunctionHandle
    where
        K: Into<Key>,
    {
        self.junctions.add(|id| Junction::new(id, location, data))
    }

    pub fn add_segment(
        &mut self,
        data: S,
        start_junction: JunctionHandle,
        end_junction: JunctionHandle,
        keys: Option<Vec<Key>>,
    ) -> SegmentHandle {
        let id = self.segments.add(|id| {
            let mut segment = Segment::new(id, data, start_junction, end_junction);

            if let Some(keys) = keys {
                segment.set_keys(keys);
            }

            segment
        });

        self.junction_mut(start_junction).add_outgoing(id);
        self.junction_mut(end_junction).add_incoming(id);

        id
    }

    pub fn junction_incoming<'a>(
        &'a self,
        junction: &'a Junction<J>,
    ) -> impl Iterator<Item = &'a Segment<S>> + 'a {
        junction
            .incoming_segments()
            .iter()
            .map(|segment| self.segment(*segment))
    }

    pub fn junction_outgoing<'a>(
        &'a self,
        junction: &'a Junction<J>,
    ) -> impl Iterator<Item = &'a Segment<S>> + 'a {
        junction
            .outgoing_segments()
            .iter()
            .map(|segment| self.segment(*segment))
    }

    pub fn segment_start(&self, segment: &Segment<S>) -> &Junction<J> {
        self.junction(segment.start_junction())
    }

    pub fn segment_end(&self, segment: &Segment<S>) -> &Junction<J> {
        self.junction(segment.end_junction())
    }

    pub fn junctions(&self) -> &BTreeMap<JunctionHandle, Junction<J>> {
        &self.junctions.inner
    }

    pub fn segments(&self) -> &BTreeMap<SegmentHandle, Segment<S>> {
        &self.segments.inner
    }

    pub fn remove_junction(&mut self, id: JunctionHandle) {
        let junction = self.junctions.remove(id);
        for incoming in junction.incoming_segments() {
            assert!(!self.segments.inner.contains_key(incoming));
        }
        for outgoing in junction.outgoing_segments() {
            assert!(!self.segments.inner.contains_key(outgoing));
        }
    }

    pub fn remove_segment(&mut self, id: SegmentHandle) {
        let segment = self.segments.remove(id);
        // also remove segment from the junctions it is connected to
        self.junction_mut(segment.start_junction())
            .remove_outgoing(id);
        self.junction_mut(segment.end_junction())
            .remove_incoming(id);
    }
}

impl<J: Clone, S: Clone> Network<J, S> {
    /**
     * Validates a network.
     *
     * Specifically, makes sure that:
     *  - segment and junction maps map to the entries with the correct ID
     *  - junction incoming/outgoing segments exist and have corresponding end/start junctions set
     *  - segment start/end junctions exist and have corresponding incoming/outgoing iter
     *
     * Panics if an issue is found. This is also not very performant, so should
     * only be used in tests and things like that.
     */
    pub fn validate(&self) {
        for (id, junction) in self.junctions.inner.iter() {
            assert!(
                id == &junction.id,
                "Mismatched ID, junction {} maps to {}",
                id.0,
                junction.id.0
            );
        }
        for (id, segment) in self.segments.inner.iter() {
            assert!(
                id == &segment.id,
                "Mismatched ID, segment {} maps to {}",
                id.0,
                segment.id.0
            );
        }

        let mut issue_count = 0;

        for junction in self.junctions.inner.values() {
            for incoming in junction.incoming_segments() {
                match self.segments.inner.get(incoming) {
                    Some(segment) => {
                        if segment.end != junction.id {
                            eprintln!(
                                "junction {} lists incoming segment {}, but segment doesn't agree",
                                junction.id.0, incoming.0
                            );
                            issue_count += 1;
                        }
                    }
                    None => {
                        eprintln!(
                            "junction {} lists incoming segment {}, but that segment doesn't exist",
                            junction.id.0, incoming.0,
                        );
                        issue_count += 1;
                    }
                }
            }
            for outgoing in junction.outgoing_segments() {
                match self.segments.inner.get(outgoing) {
                    Some(segment) => {
                        if segment.start != junction.id {
                            eprintln!(
                                "junction {} lists outgoing segment {}, but segment doesn't agree",
                                junction.id.0, outgoing.0
                            );
                            issue_count += 1;
                        }
                    }
                    None => {
                        eprintln!(
                            "junction {} lists outging segment {}, but that segment doesn't exist",
                            junction.id.0, outgoing.0,
                        );
                        issue_count += 1;
                    }
                }
            }
        }

        for segment in self.segments.inner.values() {
            match self.junctions.inner.get(&segment.start) {
                Some(start) => {
                    if !start.outgoing_segments().contains(&segment.id) {
                        eprintln!(
                            "segment {} lists start junction {}, but junction doesn't agree",
                            segment.id.0, start.id.0
                        );
                        issue_count += 1;
                    } else if segment.change_state().is_active()
                        && !start.change_state().is_active()
                    {
                        eprintln!(
                            "active segment {} lists start junction {}, but junction isn't active",
                            segment.id.0, start.id.0
                        );
                        issue_count += 1;
                    }
                }
                None => {
                    eprintln!(
                        "segment {} lists start junction {}, but that junction doesn't exist",
                        segment.id.0, segment.start.0,
                    );
                    issue_count += 1;
                }
            }
            match self.junctions.inner.get(&segment.end) {
                Some(end) => {
                    if !end.incoming_segments().contains(&segment.id) {
                        eprintln!(
                            "segment {} lists end junction {}, but junction doesn't agree",
                            segment.id.0, end.id.0
                        );
                        issue_count += 1;
                    } else if segment.change_state().is_active() && !end.change_state().is_active()
                    {
                        eprintln!(
                            "active segment {} lists end junction {}, but junction isn't active",
                            segment.id.0, end.id.0
                        );
                        issue_count += 1;
                    }
                }
                None => {
                    eprintln!(
                        "segment {} lists end junction {}, but that junction doesn't exist",
                        segment.id.0, segment.end.0,
                    );
                    issue_count += 1;
                }
            }
        }

        if issue_count > 0 {
            panic!("Found {} issues", issue_count);
        }
    }
}
