use std::cmp::Ordering;
use std::collections::binary_heap::BinaryHeap;

use serde::{Deserialize, Serialize};

// NOTE: Trigger, and all implementations, are defined in behavior.rs
use crate::behavior::{Trigger, TriggerType};
use crate::state::State;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TriggerEntry {
    trigger: Trigger,
    time: u64,
}

impl Ord for TriggerEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // always order by time first
        if self.time == other.time {
            self.trigger.cmp(&other.trigger)
        } else {
            self.time.cmp(&other.time)
        }
        // NOTE: reverse ordering so that we get a min queue
        .reverse()
    }
}

impl PartialOrd for TriggerEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// This is a sequential implementation.
// TODO: replace with a parallel implementation, perhaps using rayon.
// As long as a trigger can access all of State, this will be very difficult.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerQueue {
    /// invariant: for each trigger in heap, we must have trigger.time() >= current_time
    heap: BinaryHeap<TriggerEntry>,
    current_time: u64,
}

impl TriggerQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
            current_time: 0,
        }
    }

    /**
     * Add a new trigger to the queue.
     * The trigger time must be after the current time.
     */
    pub fn push<T: Into<Trigger>>(&mut self, trigger: T, time: u64) {
        assert!(time >= self.current_time);
        self.heap.push(TriggerEntry {
            trigger: trigger.into(),
            time,
        });
    }

    pub fn push_rel<T: Into<Trigger>>(&mut self, trigger: T, rel_time: u64) {
        self.heap.push(TriggerEntry {
            trigger: trigger.into(),
            time: self.current_time + rel_time,
        });
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }
}

impl crate::state::State {
    /**
     * Advance time forward to the current time, executing triggers in order until the given time.
     */
    pub fn advance_trigger_queue(&mut self) {
        let time = self.time_state.current_time;
        assert!(time >= self.trigger_queue.current_time);
        while self
            .trigger_queue
            .heap
            .peek()
            .map(|t| t.time <= time)
            .unwrap_or(false)
        {
            let entry = self.trigger_queue.heap.pop().unwrap();
            assert!(entry.time >= self.trigger_queue.current_time);
            self.trigger_queue.current_time = entry.time;
            entry.trigger.execute(self, self.trigger_queue.current_time);
        }
        self.trigger_queue.current_time = time;
    }
}
