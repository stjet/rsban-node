use std::{any::Any, fs::File, io::Write, path::PathBuf, sync::Mutex, time::SystemTime};

use crate::{create_property_tree, PropertyTreeWriter, TomlWriter};
use anyhow::Result;
use bounded_vec_deque::BoundedVecDeque;
use chrono::{DateTime, Local};

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

pub trait StatLogSink {
    /// Called before logging starts
    fn begin(&mut self) -> Result<()>;

    /// Called after logging is completed
    fn finalize(&mut self);

    /// Write a header enrty to the log
    fn write_header(&mut self, header: &str, walltime: SystemTime) -> Result<()>;

    /// Write a counter or sampling entry to the log. Some log sinks may support writing histograms as well.
    fn write_entry(
        &mut self,
        time: SystemTime,
        entry_type: &str,
        detail: &str,
        dir: &str,
        value: u64,
        histogram: Option<&StatHistogram>,
    ) -> Result<()>;

    /// Rotates the log (e.g. empty file). This is a no-op for sinks where rotation is not supported.
    fn rotate(&mut self) -> Result<()>;

    /// Returns a reference to the log entry counter
    fn entries(&self) -> usize;

    fn inc_entries(&mut self);

    /// Returns the string representation of the log. If not supported, an empty string is returned.
    fn to_string(&self) -> String;

    /// Returns the object representation of the log result. The type depends on the sink used.
    /// returns Object, or nullptr if no object result is available.
    fn to_object(&self) -> Option<&dyn Any>;
}

/// File sink with rotation support. This writes one counter per line and does not include histogram values.
pub struct FileWriter {
    filename: PathBuf,
    file: File,
    log_entries: usize,
}

impl FileWriter {
    pub fn new(filename: impl Into<PathBuf>) -> Result<Self> {
        let filename = filename.into();
        let file = File::create(filename.clone())?;
        Ok(Self {
            filename,
            file,
            log_entries: 0,
        })
    }
}

impl StatLogSink for FileWriter {
    fn begin(&mut self) -> Result<()> {
        Ok(())
    }

    fn finalize(&mut self) {}

    fn write_header(&mut self, header: &str, walltime: SystemTime) -> Result<()> {
        let local = DateTime::<Local>::from(walltime);
        let local_fmt = local.format("%Y.%m.%d %H:%M:%S");
        writeln!(&mut self.file, "{header},{local_fmt}")?;
        Ok(())
    }

    fn write_entry(
        &mut self,
        time: SystemTime,
        entry_type: &str,
        detail: &str,
        dir: &str,
        value: u64,
        _histogram: Option<&StatHistogram>,
    ) -> Result<()> {
        let now = DateTime::<Local>::from(time).format("%H:%M:%S");
        writeln!(&mut self.file, "{now},{entry_type},{detail},{dir},{value}")?;
        Ok(())
    }

    fn rotate(&mut self) -> Result<()> {
        self.file = File::create(self.filename.clone())?;
        self.log_entries = 0;
        Ok(())
    }

    fn entries(&self) -> usize {
        self.log_entries
    }

    fn inc_entries(&mut self) {
        self.log_entries += 1;
    }

    fn to_string(&self) -> String {
        String::new()
    }

    fn to_object(&self) -> Option<&dyn Any> {
        None
    }
}

/// JSON sink. The resulting JSON object is provided as both a property_tree::ptree (to_object) and a string (to_string)
pub struct JsonWriter {
    tree: Box<dyn PropertyTreeWriter>,
    entries_tree: Box<dyn PropertyTreeWriter>,
    log_entries: usize,
}

impl JsonWriter {
    pub fn new() -> Self {
        Self {
            tree: create_property_tree(),
            entries_tree: create_property_tree(),
            log_entries: 0,
        }
    }
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl StatLogSink for JsonWriter {
    fn begin(&mut self) -> Result<()> {
        self.tree.clear()
    }

    fn finalize(&mut self) {
        self.tree.add_child("entries", self.entries_tree.as_ref());
    }

    fn write_header(&mut self, header: &str, walltime: SystemTime) -> Result<()> {
        let now = DateTime::<Local>::from(walltime);
        self.tree.put_string("type", header)?;
        self.tree
            .put_string("created", &now.format("%Y.%m.%d %H:%M:%S").to_string())?;
        Ok(())
    }

    fn write_entry(
        &mut self,
        time: SystemTime,
        entry_type: &str,
        detail: &str,
        dir: &str,
        value: u64,
        histogram: Option<&StatHistogram>,
    ) -> Result<()> {
        let mut entry = create_property_tree();
        entry.put_string(
            "time",
            &DateTime::<Local>::from(time).format("%H:%M:%S").to_string(),
        )?;
        entry.put_string("type", entry_type)?;
        entry.put_string("detail", detail)?;
        entry.put_string("dir", dir)?;
        entry.put_u64("value", value)?;
        if let Some(histogram) = histogram {
            let mut histogram_node = create_property_tree();
            for bin in &histogram.get_bins() {
                let mut bin_node = create_property_tree();
                bin_node.put_u64("start_inclusive", bin.start_inclusive)?;
                bin_node.put_u64("end_exclusive", bin.end_exclusive)?;
                bin_node.put_u64("value", bin.value)?;

                let local_time = DateTime::<Local>::from(bin.timestamp);
                bin_node.put_string("time", &local_time.format("%H:%M:%S").to_string())?;
                histogram_node.push_back("", bin_node.as_ref());
            }
            entry.put_child("histogram", histogram_node.as_ref());
        }
        self.entries_tree.push_back("", entry.as_ref());
        Ok(())
    }

    fn rotate(&mut self) -> Result<()> {
        Ok(())
    }

    fn entries(&self) -> usize {
        self.log_entries
    }

    fn inc_entries(&mut self) {
        self.log_entries += 1;
    }

    fn to_string(&self) -> String {
        self.tree.to_json()
    }

    fn to_object(&self) -> Option<&dyn Any> {
        Some(self.tree.as_ref().as_any())
    }
}
