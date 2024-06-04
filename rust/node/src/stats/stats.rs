use anyhow::Result;
use once_cell::sync::Lazy;
use rsnano_messages::MessageType;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{atomic::AtomicU64, Mutex, RwLock},
    time::{Duration, Instant, SystemTime},
};

use super::{DetailType, Direction, JsonWriter, Sample, StatType};
use super::{FileWriter, StatsConfig, StatsLogSink};

pub struct Stats {
    config: StatsConfig,
    mutables: RwLock<StatMutables>,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new(StatsConfig::default())
    }
}

impl Stats {
    pub fn new(config: StatsConfig) -> Self {
        Self {
            config,
            mutables: RwLock::new(StatMutables {
                stopped: false,
                counters: BTreeMap::new(),
                samplers: BTreeMap::new(),
                log_last_count_writeout: Instant::now(),
                log_last_sample_writeout: Instant::now(),
                timestamp: Instant::now(),
            }),
        }
    }

    /// Add `value` to given counter
    pub fn add(&self, stat_type: StatType, detail: DetailType, value: u64) {
        self.add_dir(stat_type, detail, Direction::In, value)
    }

    /// Add `value` to given counter
    pub fn add_dir(&self, stat_type: StatType, detail: DetailType, dir: Direction, value: u64) {
        if value == 0 {
            return;
        }

        let key = CounterKey::new(stat_type, detail, dir);
        let all_key = CounterKey::new(key.stat_type, DetailType::All, key.dir);

        // This is a two-step process to avoid exclusively locking the mutex in the common case
        {
            let lock = self.mutables.read().unwrap();

            if let Some(counter) = lock.counters.get(&key) {
                counter.add(value);

                if key != all_key {
                    // The `all` counter should always be created together
                    let all_counter = lock.counters.get(&all_key).unwrap();
                    all_counter.add(value);
                }

                return;
            }
        }
        // Not found, create a new entry
        {
            let mut lock = self.mutables.write().unwrap();

            let counter = lock.counters.entry(key).or_insert(CounterEntry::new());
            counter.add(value);

            let all_counter = lock.counters.entry(all_key).or_insert(CounterEntry::new());
            if key != all_key {
                all_counter.add(value);
            }
        }
    }

    pub fn inc(&self, stat_type: StatType, detail: DetailType) {
        self.add_dir(stat_type, detail, Direction::In, 1)
    }

    pub fn inc_dir(&self, stat_type: StatType, detail: DetailType, dir: Direction) {
        self.add_dir(stat_type, detail, dir, 1)
    }

    pub fn sample(&self, stat_type: StatType, sample: Sample, value: i64) {
        let key = SamplerKey::new(stat_type, sample);
        // This is a two-step process to avoid exclusively locking the mutex in the common case
        {
            let lock = self.mutables.read().unwrap();
            if let Some(sampler) = lock.samplers.get(&key) {
                sampler.add(value, self.config.max_samples);
                return;
            }
        }
        // Not found, create a new entry
        {
            let mut lock = self.mutables.write().unwrap();
            let sampler = lock.samplers.entry(key).or_insert(SamplerEntry::new());
            sampler.add(value, self.config.max_samples)
        }
    }

    pub fn samples(&self, stat_type: StatType, sample: Sample) -> Vec<i64> {
        let key = SamplerKey::new(stat_type, sample);
        let lock = self.mutables.read().unwrap();
        if let Some(sampler) = lock.samplers.get(&key) {
            sampler.collect()
        } else {
            Vec::new()
        }
    }

    fn update(&self) -> anyhow::Result<()> {
        let mut lock = self.mutables.write().unwrap();
        if !lock.stopped {
            // Counters
            if !self.config.log_counters_interval.is_zero()
                && lock.log_last_count_writeout.elapsed() > self.config.log_counters_interval
            {
                let mut log_count = LOG_COUNT.lock().unwrap();
                let writer = match log_count.as_mut() {
                    Some(x) => x,
                    None => {
                        let writer = FileWriter::new(&self.config.log_counters_filename)?;
                        log_count.get_or_insert(writer)
                    }
                };

                lock.log_counters_impl(writer, &self.config, SystemTime::now())?;
                lock.log_last_count_writeout = Instant::now();
            }

            // Samples
            if !self.config.log_samples_interval.is_zero()
                && lock.log_last_sample_writeout.elapsed() > self.config.log_samples_interval
            {
                let mut log_sample = LOG_SAMPLE.lock().unwrap();
                let writer = match log_sample.as_mut() {
                    Some(x) => x,
                    None => {
                        let writer = FileWriter::new(&self.config.log_samples_filename)?;
                        log_sample.get_or_insert(writer)
                    }
                };
                lock.log_samples_impl(writer, &self.config, SystemTime::now())?;
                lock.log_last_sample_writeout = Instant::now();
            }
        }

        Ok(())
    }

    /// Log counters to the given log link
    pub fn log_counters(&self, sink: &mut dyn StatsLogSink) -> Result<()> {
        let now = SystemTime::now();
        let lock = self.mutables.write().unwrap();
        lock.log_counters_impl(sink, &self.config, now)
    }

    /// Log samples to the given log sink
    pub fn log_samples(&self, sink: &mut dyn StatsLogSink) -> Result<()> {
        let now = SystemTime::now();
        let lock = self.mutables.write().unwrap();
        lock.log_samples_impl(sink, &self.config, now)
    }

    /// Returns the duration since `clear()` was last called, or node startup if it's never called.
    pub fn last_reset(&self) -> Duration {
        let lock = self.mutables.read().unwrap();
        lock.timestamp.elapsed()
    }

    /// Clear all stats
    pub fn clear(&self) {
        let mut lock = self.mutables.write().unwrap();
        lock.counters.clear();
        lock.samplers.clear();
        lock.timestamp = Instant::now();
    }

    /// Returns current value for the given counter at the type level
    pub fn count(&self, stat_type: StatType, detail: DetailType, dir: Direction) -> u64 {
        let key = CounterKey::new(stat_type, detail, dir);
        self.mutables
            .read()
            .unwrap()
            .counters
            .get(&key)
            .map(|i| i.into())
            .unwrap_or_default()
    }

    /// Stop stats being output
    pub fn stop(&self) {
        self.mutables.write().unwrap().stopped = true;
    }

    pub fn dump(&self, category: StatCategory) -> String {
        let mut sink = JsonWriter::new();
        match category {
            StatCategory::Counters => self.log_counters(&mut sink).unwrap(),
            StatCategory::Samples => self.log_samples(&mut sink).unwrap(),
        }
        sink.to_string()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct CounterKey {
    stat_type: StatType,
    detail: DetailType,
    dir: Direction,
}

impl CounterKey {
    fn new(stat_type: StatType, detail: DetailType, dir: Direction) -> Self {
        Self {
            stat_type,
            detail,
            dir,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct SamplerKey {
    stat_type: StatType,
    sample: Sample,
}

impl SamplerKey {
    fn new(stat_type: StatType, sample: Sample) -> Self {
        Self { stat_type, sample }
    }
}

pub enum StatCategory {
    Counters,
    Samples,
}

struct StatMutables {
    /// Whether stats should be output
    stopped: bool,

    /// Stat entries are sorted by key to simplify processing of log output
    counters: BTreeMap<CounterKey, CounterEntry>,
    samplers: BTreeMap<SamplerKey, SamplerEntry>,

    log_last_count_writeout: Instant,
    log_last_sample_writeout: Instant,

    /// Time of last clear() call
    timestamp: Instant,
}

impl StatMutables {
    /// Unlocked implementation of log_samples() to avoid using recursive locking
    fn log_samples_impl(
        &self,
        sink: &mut dyn StatsLogSink,
        config: &StatsConfig,
        time: SystemTime,
    ) -> Result<()> {
        sink.begin()?;
        if sink.entries() >= config.log_rotation_count {
            sink.rotate()?;
        }

        if config.log_headers {
            let walltime = SystemTime::now();
            sink.write_header("samples", walltime)?;
        }

        for (&key, value) in &self.samplers {
            let type_str = key.stat_type.as_str();
            let sample = key.sample.as_str();

            for datapoint in value.collect() {
                sink.write_sampler_entry(time, type_str, sample, datapoint)?;
            }
        }
        sink.inc_entries();
        sink.finalize();
        Ok(())
    }

    /// Unlocked implementation of log_counters() to avoid using recursive locking
    fn log_counters_impl(
        &self,
        sink: &mut dyn StatsLogSink,
        config: &StatsConfig,
        time: SystemTime,
    ) -> Result<()> {
        sink.begin()?;
        if sink.entries() >= config.log_rotation_count {
            sink.rotate()?;
        }

        if config.log_headers {
            let walltime = SystemTime::now();
            sink.write_header("counters", walltime)?;
        }

        for (&key, value) in &self.counters {
            let type_str = key.stat_type.as_str();
            let detail = key.detail.as_str();
            let dir = key.dir.as_str();
            sink.write_counter_entry(time, type_str, detail, dir, value.into())?;
        }
        sink.inc_entries();
        sink.finalize();
        Ok(())
    }
}

struct CounterEntry(AtomicU64);

impl CounterEntry {
    fn new() -> Self {
        Self(AtomicU64::new(0))
    }

    fn add(&self, value: u64) {
        self.0.fetch_add(value, std::sync::atomic::Ordering::SeqCst);
    }
}

impl From<&CounterEntry> for u64 {
    fn from(value: &CounterEntry) -> Self {
        value.0.load(std::sync::atomic::Ordering::SeqCst)
    }
}

struct SamplerEntry {
    samples: Mutex<VecDeque<i64>>,
}

impl SamplerEntry {
    pub fn new() -> Self {
        Self {
            samples: Mutex::new(VecDeque::new()),
        }
    }

    fn add(&self, value: i64, max_samples: usize) {
        let mut guard = self.samples.lock().unwrap();
        guard.push_back(value);
        while guard.len() > max_samples {
            guard.pop_front();
        }
    }

    fn collect(&self) -> Vec<i64> {
        let mut guard = self.samples.lock().unwrap();
        guard.drain(..).collect()
    }
}

impl From<MessageType> for DetailType {
    fn from(msg: MessageType) -> Self {
        match msg {
            MessageType::Invalid => DetailType::Invalid,
            MessageType::NotAType => DetailType::NotAType,
            MessageType::Keepalive => DetailType::Keepalive,
            MessageType::Publish => DetailType::Publish,
            MessageType::ConfirmReq => DetailType::ConfirmReq,
            MessageType::ConfirmAck => DetailType::ConfirmAck,
            MessageType::BulkPull => DetailType::BulkPull,
            MessageType::BulkPush => DetailType::BulkPush,
            MessageType::FrontierReq => DetailType::FrontierReq,
            MessageType::NodeIdHandshake => DetailType::NodeIdHandshake,
            MessageType::BulkPullAccount => DetailType::BulkPullAccount,
            MessageType::TelemetryReq => DetailType::TelemetryReq,
            MessageType::TelemetryAck => DetailType::TelemetryAck,
            MessageType::AscPullReq => DetailType::AscPullReq,
            MessageType::AscPullAck => DetailType::AscPullAck,
        }
    }
}

/// Value and wall time of measurement
#[derive(Clone)]
pub struct StatDatapoint {
    /// Value of the sample interval
    value: u64,
    /// When the sample was added. This is wall time (system_clock), suitable for display purposes.
    timestamp: SystemTime, //todo convert back to Instant
}

impl Default for StatDatapoint {
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
        self.value
    }

    pub(crate) fn set_value(&mut self, value: u64) {
        self.value = value;
    }

    pub(crate) fn get_timestamp(&self) -> SystemTime {
        self.timestamp
    }

    pub(crate) fn set_timestamp(&mut self, timestamp: SystemTime) {
        self.timestamp = timestamp;
    }

    pub(crate) fn add(&mut self, addend: u64, update_timestamp: bool) {
        self.value += addend;
        if update_timestamp {
            self.timestamp = SystemTime::now();
        }
    }
}

static LOG_COUNT: Lazy<Mutex<Option<FileWriter>>> = Lazy::new(|| Mutex::new(None));
static LOG_SAMPLE: Lazy<Mutex<Option<FileWriter>>> = Lazy::new(|| Mutex::new(None));

#[cfg(test)]
mod tests {
    use super::*;

    /// Test stat counting at both type and detail levels
    #[test]
    fn counting() {
        let stats = Stats::new(StatsConfig::new());
        stats.add_dir(StatType::Ledger, DetailType::All, Direction::In, 1);
        stats.add_dir(StatType::Ledger, DetailType::All, Direction::In, 5);
        stats.inc_dir(StatType::Ledger, DetailType::All, Direction::In);
        stats.inc_dir(StatType::Ledger, DetailType::Send, Direction::In);
        stats.inc_dir(StatType::Ledger, DetailType::Send, Direction::In);
        stats.inc_dir(StatType::Ledger, DetailType::Receive, Direction::In);
        assert_eq!(
            10,
            stats.count(StatType::Ledger, DetailType::All, Direction::In)
        );
        assert_eq!(
            2,
            stats.count(StatType::Ledger, DetailType::Send, Direction::In)
        );
        assert_eq!(
            1,
            stats.count(StatType::Ledger, DetailType::Receive, Direction::In)
        );

        stats.add_dir(StatType::Ledger, DetailType::All, Direction::In, 0);
        assert_eq!(
            10,
            stats.count(StatType::Ledger, DetailType::All, Direction::In)
        );
    }
}
