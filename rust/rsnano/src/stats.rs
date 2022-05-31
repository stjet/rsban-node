use std::{sync::Mutex, time::SystemTime};

use crate::TomlWriter;
use anyhow::Result;
use bounded_vec_deque::BoundedVecDeque;

pub struct StatConfig {
    /** If true, sampling of counters is enabled */
    pub sampling_enabled: bool,

    /** How many sample intervals to keep in the ring buffer */
    pub capacity: usize,

    /** Sample interval in milliseconds */
    pub interval: usize,

    /** How often to log sample array, in milliseconds. Default is 0 (no logging) */
    pub log_interval_samples: usize,

    /** How often to log counters, in milliseconds. Default is 0 (no logging) */
    pub log_interval_counters: usize,

    /** Maximum number of log outputs before rotating the file */
    pub log_rotation_count: usize,

    /** If true, write headers on each counter or samples writeout. The header contains log type and the current wall time. */
    pub log_headers: bool,

    /** Filename for the counter log  */
    pub log_counters_filename: String,

    /** Filename for the sampling log */
    pub log_samples_filename: String,
}

impl Default for StatConfig {
    fn default() -> Self {
        Self {
            sampling_enabled: false,
            capacity: 0,
            interval: 0,
            log_interval_samples: 0,
            log_interval_counters: 0,
            log_rotation_count: 100,
            log_headers: true,
            log_counters_filename: "counters.stat".to_string(),
            log_samples_filename: "samples.stat".to_string(),
        }
    }
}

impl StatConfig {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn serialize_toml(&self, toml: &mut dyn TomlWriter) -> Result<()> {
        toml.put_child("sampling", &mut |sampling| {
            sampling.put_bool(
                "enable",
                self.sampling_enabled,
                "Enable or disable sampling.\ntype:bool",
            )?;
            sampling.put_usize(
                "capacity",
                self.capacity,
                "How many sample intervals to keep in the ring buffer.\ntype:uint64",
            )?;
            sampling.put_usize(
                "interval",
                self.interval,
                "Sample interval.\ntype:milliseconds",
            )?;
            Ok(())
        })?;

        toml.put_child("log", &mut |log|{
            log.put_bool("headers", self.log_headers, "If true, write headers on each counter or samples writeout.\nThe header contains log type and the current wall time.\ntype:bool")?;
            log.put_usize("interval_counters", self.log_interval_counters, "How often to log counters. 0 disables logging.\ntype:milliseconds")?;
            log.put_usize("interval_samples", self.log_interval_samples, "How often to log samples. 0 disables logging.\ntype:milliseconds")?;
            log.put_usize("rotation_count", self.log_rotation_count, "Maximum number of log outputs before rotating the file.\ntype:uint64")?;
            log.put_str("filename_counters", &self.log_counters_filename, "Log file name for counters.\ntype:string")?;
            log.put_str("filename_samples", &self.log_samples_filename, "Log file name for samples.\ntype:string")?;
            Ok(())
        })?;
        Ok(())
    }
}

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

/// Histogram bin with interval, current value and timestamp of last update
#[derive(Clone)]
pub struct HistogramBin {
    pub start_inclusive: u64,
    pub end_exclusive: u64,
    pub value: u64,
    pub timestamp: SystemTime,
}

impl HistogramBin {
    pub fn new(start_inclusive: u64, end_exclusive: u64) -> Self {
        Self {
            start_inclusive,
            end_exclusive,
            value: 0,
            timestamp: SystemTime::now(),
        }
    }
}

/// Histogram values
pub struct StatHistogram {
    bins: Mutex<Vec<HistogramBin>>,
}

impl StatHistogram {
    /// Create histogram given a set of intervals and an optional bin count
    /// # Arguments
    /// * `intervals` - Inclusive-exclusive intervals, e.g. {1,5,8,15} produces bins [1,4] [5,7] [8, 14]
    /// * `bin_count` -  If zero (default), \p intervals_a defines all the bins. If non-zero, \p intervals_a contains the total range, which is uniformly distributed into \p bin_count_a bins.
    pub fn new(intervals: &[u64], bin_count: u64) -> Self {
        let mut bins = Vec::new();
        if bin_count == 0 {
            debug_assert!(intervals.len() > 1);
            let mut start_inclusive = intervals[0];
            for &i in &intervals[1..] {
                let end_exclusive = i;
                bins.push(HistogramBin::new(start_inclusive, end_exclusive));
                start_inclusive = end_exclusive;
            }
        } else {
            debug_assert!(intervals.len() == 2);
            let min_inclusive = intervals[0];
            let max_exclusive = intervals[1];
            let domain = max_exclusive - min_inclusive;
            let bin_size = (domain + bin_count - 1) / bin_count;
            let last_bin_size = domain % bin_size;
            let mut next_start = min_inclusive;

            for _ in 0..bin_count {
                bins.push(HistogramBin::new(next_start, next_start + bin_size));
                next_start += bin_size;
            }

            if last_bin_size > 0 {
                bins.push(HistogramBin::new(next_start, next_start + last_bin_size));
            }
        }

        Self {
            bins: Mutex::new(bins),
        }
    }

    /// Add `addend` to the histogram bin into which `index` falls
    pub fn add(&self, index: u64, addend: u64) {
        let mut lock = self.bins.lock().unwrap();
        debug_assert!(!lock.is_empty());

        // The search for a bin is linear, but we're searching just a few
        // contiguous items which are likely to be in cache.
        let mut found = false;
        for bin in lock.iter_mut() {
            if index >= bin.start_inclusive && index < bin.end_exclusive {
                bin.value += addend;
                bin.timestamp = SystemTime::now();
                found = true;
                break;
            }
        }

        // Clamp into first or last bin if no suitable bin was found
        if !found {
            if index < lock[0].start_inclusive {
                lock[0].value += addend;
            } else {
                lock.last_mut().unwrap().value += addend;
            }
        }
    }

    pub fn get_bins(&self) -> Vec<HistogramBin> {
        self.bins.lock().unwrap().clone()
    }
}

impl Clone for StatHistogram {
    fn clone(&self) -> Self {
        let lock = self.bins.lock().unwrap();
        Self {
            bins: Mutex::new(lock.clone()),
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
