use std::cmp::Ordering;
use std::collections::binary_heap::BinaryHeap;

use serde::{Deserialize, Serialize};

use crate::state::State;

#[enum_dispatch::enum_dispatch]
pub trait TriggerType: PartialEq + Eq + PartialOrd + Ord {
    fn execute(self, state: &mut State, time: u64);
}

// NOTE: all implementations of TriggerType must be listed here
#[enum_dispatch::enum_dispatch(TriggerType)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum Trigger {
    DummyTrigger,
    DoublingTrigger,
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

// Sample trigger implementation, demonstrates a simple recurring trigger
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DummyTrigger {}

impl TriggerType for DummyTrigger {
    fn execute(self, state: &mut State, time: u64) {
        println!("executing {}", time);
        state.trigger_queue.push(self, time + 1);
    }
}

// Used for testing. Must be defined here since enum_dispatch doesn't support crossing crate
// boundaries.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DoublingTrigger {}

impl TriggerType for DoublingTrigger {
    fn execute(self, state: &mut State, time: u64) {
        state.trigger_queue.push(DoublingTrigger {}, time + 1);
        state.trigger_queue.push(DoublingTrigger {}, time + 1);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TriggerEntry {
    trigger: Trigger,
    time: u64,
}
