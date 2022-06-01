use bounded_vec_deque::BoundedVecDeque;
use std::{sync::Mutex, time::SystemTime};

use crate::{StatConfig, StatHistogram};

/// Value and wall time of measurement
#[derive(Default)]
pub struct StatDatapoint {
    values: Mutex<StatDatapointValues>,
}

impl Clone for StatDatapoint {
    fn clone(&self) -> Self {
        let lock = self.values.lock().unwrap();
        Self {
            values: Mutex::new(lock.clone()),
        }
    }
}

#[derive(Clone)]
struct StatDatapointValues {
    /// Value of the sample interval
    value: u64,
    /// When the sample was added. This is wall time (system_clock), suitable for display purposes.
    timestamp: SystemTime, //todo convert back to Instant
}

impl Default for StatDatapointValues {
    fn default() -> Self {
        Self {
            value: 0,
            timestamp: SystemTime::now(),
        }
    }
}

impl StatDatapoint {
    pub fn new() -> Self {
        Default::default()
    }

    pub(crate) fn get_value(&self) -> u64 {
        self.values.lock().unwrap().value
    }

    pub(crate) fn set_value(&self, value: u64) {
        self.values.lock().unwrap().value = value;
    }

    pub(crate) fn get_timestamp(&self) -> SystemTime {
        self.values.lock().unwrap().timestamp
    }

    pub(crate) fn set_timestamp(&self, timestamp: SystemTime) {
        self.values.lock().unwrap().timestamp = timestamp;
    }

    pub(crate) fn add(&self, addend: u64, update_timestamp: bool) {
        let mut lock = self.values.lock().unwrap();
        lock.value += addend;
        if update_timestamp {
            lock.timestamp = SystemTime::now();
        }
    }
}

pub struct StatEntry {
    /// Sample interval in milliseconds. If 0, sampling is disabled.
    pub sample_interval: usize,

    /// Value within the current sample interval
    pub sample_current: StatDatapoint,

    /// Optional samples. Note that this doesn't allocate any memory unless sampling is configured, which sets the capacity.
    pub samples: Option<BoundedVecDeque<StatDatapoint>>,

    /// Counting value for this entry, including the time of last update. This is never reset and only increases.
    pub counter: StatDatapoint,

    /// Start time of current sample interval. This is a steady clock for measuring interval; the datapoint contains the wall time.
    pub sample_start_time: SystemTime,

    /// Optional histogram for this entry
    pub histogram: Option<StatHistogram>,
}

impl StatEntry {
    pub fn new(capacity: usize, interval: usize) -> Self {
        Self {
            sample_interval: interval,
            sample_current: StatDatapoint::new(),
            samples: if capacity > 0 {
                Some(BoundedVecDeque::new(capacity))
            } else {
                None
            },
            counter: StatDatapoint::new(),
            sample_start_time: SystemTime::now(),
            histogram: None,
        }
    }
}

pub struct Stat {
    config: StatConfig,
}

impl Stat {
    pub fn new(config: StatConfig) -> Self {
        Self { config }
    }
}
