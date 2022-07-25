use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::junction::{HighwayJunction, RampDirection};
use crate::segment::{HighwayData, HighwayKey, HighwaySegment};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Highways {
    junctions: BTreeMap<u64, HighwayJunction>,
    segments: BTreeMap<u64, HighwaySegment>,
    junction_counter: u64,
    segment_counter: u64,
}

impl Highways {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_junction(&mut self, location: (f64, f64), ramp: Option<RampDirection>) -> u64 {
        let id = self.junction_counter;

        self.junctions
            .insert(id, HighwayJunction::new(id, location, ramp));
        self.junction_counter += 1;

        id
    }

    pub fn add_segment(
        &mut self,
        data: HighwayData,
        start_junction: u64,
        end_junction: u64,
        keys: Option<Vec<HighwayKey>>,
    ) -> u64 {
        let id = self.segment_counter;

        self.junctions
            .get_mut(&start_junction)
            .expect("specified start not found")
            .add_outgoing(id);
        self.junctions
            .get_mut(&end_junction)
            .expect("specified end not found")
            .add_incoming(id);

        let mut segment = HighwaySegment::new(id, data, start_junction, end_junction);

        if let Some(keys) = keys {
            segment.set_keys(keys);
        }

        self.segments.insert(id, segment);
        self.segment_counter += 1;

        id
    }

    pub fn get_junction(&self, id: u64) -> Option<&HighwayJunction> {
        self.junctions.get(&id)
    }

    pub fn get_segment(&self, id: u64) -> Option<&HighwaySegment> {
        self.segments.get(&id)
    }

    pub fn get_junction_incoming(&self, junction: &HighwayJunction) -> Vec<&HighwaySegment> {
        junction
            .incoming_segments()
            .iter()
            .map(|id| self.get_segment(*id).expect("missing incoming segment"))
            .collect()
    }

    pub fn get_junction_outgoing(&self, junction: &HighwayJunction) -> Vec<&HighwaySegment> {
        junction
            .outgoing_segments()
            .iter()
            .map(|id| self.get_segment(*id).expect("missing outgoing segment"))
            .collect()
    }

    pub fn get_segment_start(&self, segment: &HighwaySegment) -> &HighwayJunction {
        self.get_junction(segment.start_junction())
            .expect("missing start junction")
    }

    pub fn get_segment_end(&self, segment: &HighwaySegment) -> &HighwayJunction {
        self.get_junction(segment.end_junction())
            .expect("missing end junction")
    }

    pub fn get_junctions(&self) -> &BTreeMap<u64, HighwayJunction> {
        &self.junctions
    }

    pub fn get_segments(&self) -> &BTreeMap<u64, HighwaySegment> {
        &self.segments
    }

    /**
     * Validates a Highways data structure.
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
                id,
                junction.id
            );
        }
        for (id, segment) in self.segments.iter() {
            assert!(
                id == &segment.id,
                "Mismatched ID, segment {} maps to {}",
                id,
                segment.id
            );
        }

        let mut issue_count = 0;

        for junction in self.junctions.values() {
            for incoming in self.get_junction_incoming(junction) {
                if !self.get_segment_end(incoming).id == junction.id {
                    eprintln!(
                        "junction {} lists incoming segment {}, but segment doesn't agree",
                        junction.id, incoming.id
                    );
                    issue_count += 1;
                }
            }
            for outgoing in self.get_junction_outgoing(junction) {
                if !self.get_segment_start(outgoing).id == junction.id {
                    eprintln!(
                        "junction {} lists outgoing segment {}, but segment doesn't agree",
                        junction.id, outgoing.id
                    );
                    issue_count += 1;
                }
            }
        }

        for segment in self.segments.values() {
            let start = self.get_segment_start(segment);
            if !start.outgoing_segments().contains(&segment.id) {
                eprintln!(
                    "segment {} lists start junction {}, but junction doesn't agree",
                    segment.id, start.id
                );
                issue_count += 1;
            }
            let end = self.get_segment_end(segment);
            if !end.incoming_segments().contains(&segment.id) {
                eprintln!(
                    "segment {} lists end junction {}, but junction doesn't agree",
                    segment.id, end.id
                );
                issue_count += 1;
            }
        }

        if issue_count > 0 {
            panic!("Found {} issues", issue_count);
        }
    }
}
