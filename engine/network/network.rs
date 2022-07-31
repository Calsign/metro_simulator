use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::junction::{Junction, JunctionHandle};
use crate::segment::{Segment, SegmentHandle};

pub type Key = cgmath::Vector2<f64>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network<J, S> {
    junctions: BTreeMap<JunctionHandle, Junction<J>>,
    segments: BTreeMap<SegmentHandle, Segment<S>>,
    junction_counter: u64,
    segment_counter: u64,
}

impl<J, S> Network<J, S> {
    pub fn new() -> Self {
        Self {
            junctions: BTreeMap::new(),
            segments: BTreeMap::new(),
            junction_counter: 0,
            segment_counter: 0,
        }
    }

    pub fn junction(&self, id: JunctionHandle) -> &Junction<J> {
        self.junctions.get(&id).expect("invalid junction handle")
    }

    pub fn junction_mut(&mut self, id: JunctionHandle) -> &mut Junction<J> {
        self.junctions
            .get_mut(&id)
            .expect("invalid junction handle")
    }

    pub fn segment(&self, id: SegmentHandle) -> &Segment<S> {
        self.segments.get(&id).expect("invalid segment handle")
    }

    pub fn segment_mut(&mut self, id: SegmentHandle) -> &mut Segment<S> {
        self.segments.get_mut(&id).expect("invalid segment handle")
    }

    pub fn add_junction<K>(&mut self, location: K, data: J) -> JunctionHandle
    where
        K: Into<Key>,
    {
        let id = JunctionHandle(self.junction_counter);
        self.junction_counter += 1;
        self.junctions.insert(id, Junction::new(id, location, data));
        id
    }

    pub fn add_segment(
        &mut self,
        data: S,
        start_junction: JunctionHandle,
        end_junction: JunctionHandle,
        keys: Option<Vec<Key>>,
    ) -> SegmentHandle {
        let id = SegmentHandle(self.segment_counter);
        self.segment_counter += 1;

        self.junction_mut(start_junction).add_outgoing(id);
        self.junction_mut(end_junction).add_incoming(id);

        let mut segment = Segment::new(id, data, start_junction, end_junction);

        if let Some(keys) = keys {
            segment.set_keys(keys);
        }

        self.segments.insert(id, segment);

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
        &self.junctions
    }

    pub fn segments(&self) -> &BTreeMap<SegmentHandle, Segment<S>> {
        &self.segments
    }
}

impl<J, S> Network<J, S> {
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
        for (id, junction) in self.junctions.iter() {
            assert!(
                id == &junction.id,
                "Mismatched ID, junction {} maps to {}",
                id.0,
                junction.id.0
            );
        }
        for (id, segment) in self.segments.iter() {
            assert!(
                id == &segment.id,
                "Mismatched ID, segment {} maps to {}",
                id.0,
                segment.id.0
            );
        }

        let mut issue_count = 0;

        for junction in self.junctions.values() {
            for incoming in self.junction_incoming(junction) {
                if self.segment_end(incoming).id != junction.id {
                    eprintln!(
                        "junction {} lists incoming segment {}, but segment doesn't agree",
                        junction.id.0, incoming.id.0
                    );
                    issue_count += 1;
                }
            }
            for outgoing in self.junction_outgoing(junction) {
                if self.segment_start(outgoing).id != junction.id {
                    eprintln!(
                        "junction {} lists outgoing segment {}, but segment doesn't agree",
                        junction.id.0, outgoing.id.0
                    );
                    issue_count += 1;
                }
            }
        }

        for segment in self.segments.values() {
            let start = self.segment_start(segment);
            if !start.outgoing_segments().contains(&segment.id) {
                eprintln!(
                    "segment {} lists start junction {}, but junction doesn't agree",
                    segment.id.0, start.id.0
                );
                issue_count += 1;
            }
            let end = self.segment_end(segment);
            if !end.incoming_segments().contains(&segment.id) {
                eprintln!(
                    "segment {} lists end junction {}, but junction doesn't agree",
                    segment.id.0, end.id.0
                );
                issue_count += 1;
            }
        }

        if issue_count > 0 {
            panic!("Found {} issues", issue_count);
        }
    }
}
