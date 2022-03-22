use crate::types::HighwaySegment;
use std::collections::HashMap;

/**
 * Validates a set of highway segments.
 *
 * Specifically, makes sure that:
 *  - there are no duplicate IDs
 *  - segments referred to in pred and succ actually exist
 *  - each pred entry has a corresponding succ entry, and vice versa
 *
 * Panics if an issue is found. This is also not very performant, so should
 * only be used in tests and things like that.
 */
pub fn validate_highway_segments<'a, I>(highway_segments: I)
where
    I: Clone + Iterator<Item = &'a HighwaySegment>,
{
    let mut map = HashMap::new();
    for segment in highway_segments.clone() {
        assert!(
            !map.contains_key(&segment.id),
            "Duplicate key: {}",
            segment.id
        );
        map.insert(segment.id, segment);
    }

    let mut issue_count = 0;

    for segment in highway_segments {
        for pred in segment.pred() {
            if !map
                .get(pred)
                .expect("missing id")
                .succ()
                .contains(&segment.id)
            {
                eprintln!(
                    "{} has pred {}, which doesn't have a corresponding succ",
                    segment.id, pred
                );
                issue_count += 1;
            }
        }
        for succ in segment.succ() {
            if !map
                .get(succ)
                .expect("missing id")
                .pred()
                .contains(&segment.id)
            {
                eprintln!(
                    "{} has succ {}, which doesn't have a corresponding pred",
                    segment.id, succ
                );
                issue_count += 1;
            }
        }
    }

    if issue_count > 0 {
        panic!("Found {} issues", issue_count);
    }
}
