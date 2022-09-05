use std::cmp::Ordering;
use std::collections::binary_heap::BinaryHeap;

use enum_iterator::IntoEnumIterator;
use serde::{Deserialize, Serialize};
use tabled::Tabled;

// NOTE: Trigger, and all implementations, are defined in behavior.rs
use crate::behavior::{Trigger, TriggerKind, TriggerType};
use crate::engine::Error;

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
// As long as a trigger can access all of Engine, this will be very difficult.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerQueue {
    /// invariant: for each trigger in heap, we must have trigger.time() >= current_time
    heap: BinaryHeap<TriggerEntry>,
    current_time: u64,
}

impl TriggerQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /**
     * Add a new trigger to the queue.
     * The trigger time must be after the current time.
     */
    pub fn push<T: Into<Trigger>>(&mut self, trigger: T, time: u64) {
        let trigger = trigger.into();
        assert!(
            time >= self.current_time,
            "time: {}, current time: {}, trigger: {:?}",
            time,
            self.current_time,
            trigger
        );
        self.heap.push(TriggerEntry { trigger, time });
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

impl crate::engine::Engine {
    /**
     * Advance time forward to the current time, executing triggers in order until the given time.
     */
    pub fn advance_trigger_queue(&mut self, time_step: u64, time_budget: f64) -> Result<(), Error> {
        let target_time = self.time_state.current_time + time_step;
        let budget_start = std::time::Instant::now();

        while budget_start.elapsed().as_secs_f64() < time_budget {
            if self
                .trigger_queue
                .heap
                .peek()
                .map(|t| t.time > target_time)
                .unwrap_or(true)
            {
                self.trigger_queue.current_time = target_time;
                self.time_state.current_time = target_time;
                break;
            }
            self.single_step()?;
        }

        Ok(())
    }

    pub fn single_step(&mut self) -> Result<(), Error> {
        let entry = self.trigger_queue.heap.pop().unwrap();
        assert!(entry.time >= self.trigger_queue.current_time);
        self.trigger_queue.current_time = entry.time;
        self.time_state.current_time = entry.time;

        let start = self
            .trigger_stats
            .profiling_enabled
            .then(|| cpu_time::ThreadTime::try_now().ok())
            .flatten();
        let kind = TriggerKind::from(&entry.trigger);

        entry
            .trigger
            .execute(self, self.trigger_queue.current_time)?;

        if let Some(start) = start {
            self.trigger_stats.record_trigger(kind, start.elapsed());
        }
        Ok(())
    }

    pub fn peek_trigger(&self) -> Option<&Trigger> {
        self.trigger_queue.heap.peek().map(|entry| &entry.trigger)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerMap<T> {
    data: [T; TriggerKind::ITEM_COUNT],
}

impl<T> TriggerMap<T> {
    pub fn each<F>(f: F) -> Self
    where
        F: Fn(TriggerKind) -> T,
    {
        Self {
            data: TriggerKind::into_enum_iter()
                .map(f)
                .collect::<Vec<T>>()
                .try_into()
                .unwrap_or_else(|_| panic!("should be impossible")),
        }
    }

    pub fn values(&self) -> &[T; TriggerKind::ITEM_COUNT] {
        &self.data
    }

    pub fn iter(&self) -> impl Iterator<Item = (TriggerKind, &T)> {
        TriggerKind::into_enum_iter()
            .enumerate()
            .map(|(i, kind)| (kind, &self.data[i]))
    }
}

impl<T, K> std::ops::Index<K> for TriggerMap<T>
where
    K: Into<TriggerKind>,
{
    type Output = T;
    fn index(&self, kind: K) -> &T {
        let kind: TriggerKind = kind.into();
        &self.data[kind as usize]
    }
}

impl<T, K> std::ops::IndexMut<K> for TriggerMap<T>
where
    K: Into<TriggerKind>,
{
    fn index_mut(&mut self, kind: K) -> &mut T {
        let kind: TriggerKind = kind.into();
        &mut self.data[kind as usize]
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct IncrementalStats {
    pub count: u128,
    pub sum: u128,
    pub sum_sq: u128,
    pub min: Option<u128>,
    pub max: Option<u128>,
}

impl IncrementalStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn observe(&mut self, val: u128) {
        self.count = self.count.checked_add(1).expect("overflow");
        self.sum = self.sum.checked_add(val).expect("overflow");
        self.sum_sq = self
            .sum_sq
            .checked_add(val.checked_pow(2).expect("overflow"))
            .expect("overflow");
        self.min = Some(self.min.map_or(val, |v| v.min(val)));
        self.max = Some(self.max.map_or(val, |v| v.max(val)));
    }

    pub fn mean(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.sum as f64 / self.count as f64)
        } else {
            None
        }
    }

    pub fn variance(&self) -> Option<f64> {
        if self.count > 1 {
            Some(
                (self.sum_sq as f64 / self.count as f64)
                    - (self.sum as f64 / self.count as f64).powi(2),
            )
        } else {
            None
        }
    }

    pub fn std_dev(&self) -> Option<f64> {
        self.variance().map(|var| var.sqrt())
    }
}

#[derive(tabled::Tabled)]
pub struct StatsEntry {
    pub name: String,
    pub count: u128,
    pub sum: u128,
    pub mean: String,
    pub std_dev: String,
    pub min: String,
    pub max: String,
}

impl StatsEntry {
    fn new(name: String, stats: &IncrementalStats) -> Self {
        Self {
            name,
            count: stats.count,
            sum: stats.sum,
            mean: stats
                .mean()
                .map(|mean| format!("{:.2}", mean))
                .unwrap_or_else(|| "-".to_string()),
            std_dev: stats
                .std_dev()
                .map(|std_dev| format!("{:.2}", std_dev))
                .unwrap_or_else(|| "-".to_string()),
            min: stats
                .min
                .map(|min| min.to_string())
                .unwrap_or_else(|| "-".to_string()),
            max: stats
                .max
                .map(|max| max.to_string())
                .unwrap_or_else(|| "-".to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TriggerStats {
    pub profiling_enabled: bool,
    pub stats: TriggerMap<IncrementalStats>,
}

impl Default for TriggerStats {
    fn default() -> Self {
        Self::new(std::env::var("DEBUG_TRIGGER_PROFILE").is_ok())
    }
}

impl TriggerStats {
    pub fn new(profiling_enabled: bool) -> Self {
        Self {
            profiling_enabled,
            stats: TriggerMap::each(|_| IncrementalStats::new()),
        }
    }

    pub fn enable_profiling(&mut self) {
        self.profiling_enabled = true;
    }

    pub fn record_trigger(&mut self, kind: TriggerKind, duration: std::time::Duration) {
        if self.profiling_enabled {
            self.stats[kind].observe(duration.as_nanos());
        }
    }

    pub fn print(&self) {
        use tabled::object::{Columns, Rows, Segment};
        use tabled::{Alignment, Modify, Style, Table};

        let table = Table::new(
            self.stats
                .iter()
                .map(|(kind, stats)| StatsEntry::new(format!("{:?}", kind), stats)),
        )
        .with(Style::modern())
        .with(Modify::new(Segment::all()).with(Alignment::right()))
        .with(Modify::new(Columns::first()).with(Alignment::left()))
        .with(Modify::new(Rows::first()).with(tabled::Alignment::left()));

        println!("{}", table);
    }
}

#[cfg(test)]
mod incremental_stats_tests {
    use crate::trigger::IncrementalStats;
    use float_cmp::assert_approx_eq;

    fn mean(vals: &[u128]) -> Option<f64> {
        if !vals.is_empty() {
            Some(vals.iter().sum::<u128>() as f64 / vals.len() as f64)
        } else {
            None
        }
    }

    fn variance(vals: &[u128]) -> Option<f64> {
        if vals.len() > 1 {
            let mean = mean(vals).unwrap();
            Some(vals.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / (vals.len()) as f64)
        } else {
            None
        }
    }

    const TESTS: &[&[u128]] = &[
        &[3, 4, 5],
        &[100, 13131],
        &[4, 7, 0, 5, 102],
        &[7126378213781, 6123612, 1731],
    ];

    const ULPS: i64 = 20;

    #[test]
    fn test_empty() {
        assert_eq!(IncrementalStats::new().mean(), None);
        assert_eq!(IncrementalStats::new().variance(), None);
    }

    #[test]
    fn test_one() {
        let mut stats = IncrementalStats::new();
        stats.observe(1);

        assert_eq!(stats.mean(), Some(1.0));
        assert_eq!(stats.variance(), None);
    }

    #[test]
    fn incremental_stats_test() {
        for test in TESTS {
            println!("Testing: {:?}", test);
            let mut stats = IncrementalStats::new();
            for val in *test {
                stats.observe(*val);
            }
            assert_approx_eq!(f64, stats.mean().unwrap(), mean(test).unwrap(), ulps = ULPS);
            assert_approx_eq!(
                f64,
                stats.variance().unwrap(),
                variance(test).unwrap(),
                ulps = ULPS
            );
        }
    }
}
