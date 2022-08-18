use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::color::Color;
use crate::railways::{Railways, Station};
use crate::schedule::Schedule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MetroLineHandle(u64);

impl MetroLineHandle {
    pub fn inner(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroLineData {
    pub color: Color,
    pub name: String,
    pub schedule: Schedule,
    pub speed_limit: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrientedSegment {
    pub segment: network::SegmentHandle,
    pub forward: bool,
}

impl OrientedSegment {
    pub fn start_junction(&self, railways: &Railways) -> network::JunctionHandle {
        let segment = railways.segment(self.segment);
        if self.forward {
            segment.start_junction()
        } else {
            segment.end_junction()
        }
    }

    pub fn end_junction(&self, railways: &Railways) -> network::JunctionHandle {
        let segment = railways.segment(self.segment);
        if self.forward {
            segment.end_junction()
        } else {
            segment.start_junction()
        }
    }

    pub fn maybe_reversed_fraction(&self, fraction: f32) -> f32 {
        if self.forward {
            fraction
        } else {
            1.0 - fraction
        }
    }

    pub fn maybe_reversed_iter<'a, T, I, F>(&self, iter: I, mut f: F)
    where
        I: Iterator<Item = T> + DoubleEndedIterator,
        F: FnMut(T),
    {
        // TODO: couldn't figure out a way to return an iterator here
        if self.forward {
            for x in iter {
                f(x);
            }
        } else {
            for x in iter.rev() {
                f(x);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetroLine {
    pub id: MetroLineHandle,
    pub data: MetroLineData,
    segments: Vec<OrientedSegment>,
}

id_cmp::id_cmp!(MetroLine, id);

impl MetroLine {
    fn new(id: MetroLineHandle, data: MetroLineData, segments: Vec<OrientedSegment>) -> Self {
        Self { id, data, segments }
    }

    pub fn segments(&self) -> &Vec<OrientedSegment> {
        &self.segments
    }

    pub fn junctions<'a>(
        &'a self,
        railways: &'a Railways,
    ) -> impl Iterator<Item = network::JunctionHandle> + 'a {
        self.segments
            .iter()
            .map(|segment| segment.start_junction(railways))
            .chain(self.segments.last().map(|last| last.end_junction(railways)))
    }

    pub fn stations<'a>(&'a self, railways: &'a Railways) -> impl Iterator<Item = &Station> + 'a {
        self.junctions(railways)
            .filter_map(|junction| railways.junction(junction).data.station.as_ref())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Metros {
    metro_lines: BTreeMap<MetroLineHandle, MetroLine>,
    metro_line_counter: u64,
    railway_segment_metro_lines: BTreeMap<network::SegmentHandle, HashSet<MetroLineHandle>>,
}

lazy_static::lazy_static! {
    static ref EMPTY_METRO_LINE_SET: HashSet<MetroLineHandle> = HashSet::new();
}

impl Metros {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn metro_line(&self, id: MetroLineHandle) -> &MetroLine {
        self.metro_lines
            .get(&id)
            .expect("invalid metro line handle")
    }

    pub fn metro_line_mut(&mut self, id: MetroLineHandle) -> &mut MetroLine {
        self.metro_lines
            .get_mut(&id)
            .expect("invalid metro line handle")
    }

    pub fn add_metro_line(
        &mut self,
        data: MetroLineData,
        segments: Vec<network::SegmentHandle>,
        railways: &Railways,
    ) -> MetroLineHandle {
        let id = MetroLineHandle(self.metro_line_counter);
        self.metro_line_counter += 1;

        for segment in &segments {
            self.railway_segment_metro_lines
                .entry(*segment)
                .or_insert_with(HashSet::new)
                .insert(id);
        }

        let oriented_segments = orient_segments(&segments, railways);

        self.metro_lines
            .insert(id, MetroLine::new(id, data, oriented_segments));
        id
    }

    pub fn metro_lines(&self) -> &BTreeMap<MetroLineHandle, MetroLine> {
        &self.metro_lines
    }

    /// Iterates through the metro lines that pass through a given railway segment
    pub fn railway_segment_metro_lines(
        &self,
        railway: network::SegmentHandle,
    ) -> &HashSet<MetroLineHandle> {
        self.railway_segment_metro_lines
            .get(&railway)
            .unwrap_or(&*EMPTY_METRO_LINE_SET)
    }

    /// Panics if any metro line has discontinuities.
    pub fn validate(&self, railways: &Railways) {
        use itertools::Itertools;
        for metro_line in self.metro_lines.values() {
            for (oriented_in_segment, oriented_out_segment) in
                metro_line.segments.iter().tuple_windows()
            {
                let in_segment = railways.segment(oriented_in_segment.segment);
                let out_segment = railways.segment(oriented_out_segment.segment);
                let in_end = oriented_in_segment.end_junction(railways);
                let out_start = oriented_out_segment.start_junction(railways);
                if in_end != out_start {
                    panic!(
                        "disconnected segments in metro line {:#?}: {:#?} and {:#?} end and start at {:#?} and {:#?}, respectively",
                        metro_line, in_segment, out_segment, in_end, out_start
                    );
                }
            }
        }
    }
}

/// Determine correct order for segments (since railways are bidirectional).
fn orient_segments(
    segments: &Vec<network::SegmentHandle>,
    railways: &Railways,
) -> Vec<OrientedSegment> {
    use itertools::Itertools;

    let mut oriented_segments = Vec::new();

    // we need to decide the orientation of the first segment first
    if let Some((first, second)) = segments.iter().tuple_windows().take(1).next() {
        // if there are at least two segments, grab the first two so that we can determine the
        // orientation
        let mut prev = {
            let first_segment = railways.segment(*first);
            let second_segment = railways.segment(*second);

            let forward = if first_segment.end_junction() == second_segment.start_junction()
                || first_segment.end_junction() == second_segment.end_junction()
            {
                true
            } else if first_segment.start_junction() == second_segment.start_junction()
                || first_segment.start_junction() == second_segment.end_junction()
            {
                false
            } else {
                panic!("gap in metro line (constructing first)");
            };

            let oriented_segment = OrientedSegment {
                segment: *first,
                forward,
            };

            oriented_segments.push(oriented_segment);
            oriented_segment
        };

        // determine the orientation of each subsequent segment based on the orientation of the
        // previous segment
        for segment_id in segments.iter().skip(1) {
            let segment = railways.segment(*segment_id);

            let forward = if segment.start_junction() == prev.end_junction(railways) {
                true
            } else if segment.end_junction() == prev.end_junction(railways) {
                false
            } else if segment.start_junction() == prev.start_junction(railways) {
                // prev is a dead-end turnaround
                oriented_segments.push(OrientedSegment {
                    segment: prev.segment,
                    forward: !prev.forward,
                });
                true
            } else if segment.end_junction() == prev.start_junction(railways) {
                // prev is a dead-end turnaround
                oriented_segments.push(OrientedSegment {
                    segment: prev.segment,
                    forward: !prev.forward,
                });
                false
            } else {
                panic!(
                    "gap in metro line (constructing rest of line); previous end: {:#?}, next segment: {:#?}",
                    railways.junction(prev.end_junction(railways)), segment
                );
            };

            let oriented_segment = OrientedSegment {
                segment: *segment_id,
                forward,
            };

            oriented_segments.push(oriented_segment);
            prev = oriented_segment;
        }
    } else {
        // there's one, or possibly zero segments, so we can't determine the orientation
        if let Some(first) = segments.first() {
            oriented_segments.push(OrientedSegment {
                segment: *first,
                forward: true,
            });
        }
    };

    oriented_segments
}
