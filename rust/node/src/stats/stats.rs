use anyhow::Result;
use bounded_vec_deque::BoundedVecDeque;
use num::FromPrimitive;
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_variant::to_variant_name;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

use crate::messages::MessageType;

use super::histogram::StatHistogram;
use super::{FileWriter, StatsConfig, StatsLogSink};

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
    pub sample_start_time: Instant,

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
            sample_start_time: Instant::now(),
            histogram: None,
        }
    }
}

/// Primary statistics type
#[repr(u8)]
#[derive(FromPrimitive, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatType {
    TrafficTcp,
    Error,
    Message,
    Block,
    Ledger,
    Rollback,
    Bootstrap,
    TcpServer,
    Vote,
    Election,
    HttpCallback,
    Peering,
    Ipc,
    Tcp,
    ConfirmationHeight,
    ConfirmationObserver,
    Drop,
    Aggregator,
    Requests,
    Filter,
    Telemetry,
    VoteGenerator,
    VoteCache,
    Hinting,
    BlockProcessor,
    BootstrapServer,
    Active,
    ActiveStarted,
    ActiveConfirmed,
    ActiveDropped,
    ActiveTimeout,
    Backlog,
    Unchecked,
}

impl StatType {
    pub fn as_str(&self) -> &'static str {
        to_variant_name(self).unwrap_or_default()
    }
}

// Optional detail type
#[repr(u8)]
#[derive(FromPrimitive, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailType {
    All = 0,

    // common
    Loop,
    Total,
    Process,
    Update,
    Request,
    Broadcast,

    // processing queue
    Queue,
    Overfill,
    Batch,

    // error specific
    BadSender,
    InsufficientWork,
    HttpCallback,
    UnreachableHost,
    InvalidNetwork,

    // confirmation_observer specific
    ActiveQuorum,
    ActiveConfHeight,
    InactiveConfHeight,

    // ledger, block, bootstrap
    Send,
    Receive,
    Open,
    Change,
    StateBlock,
    EpochBlock,
    Fork,
    Old,
    GapPrevious,
    GapSource,
    RollbackFailed,
    Progress,
    BadSignature,
    NegativeSpend,
    Unreceivable,
    GapEpochOpenPending,
    OpenedBurnAccount,
    BalanceMismatch,
    RepresentativeMismatch,
    BlockPosition,

    // message specific
    NotAType,
    Invalid,
    Keepalive,
    Publish,
    RepublishVote,
    ConfirmReq,
    ConfirmAck,
    NodeIdHandshake,
    TelemetryReq,
    TelemetryAck,
    AscPullReq,
    AscPullAck,

    // bootstrap, callback
    Initiate,
    InitiateLegacyAge,
    InitiateLazy,
    InitiateWalletLazy,

    // bootstrap specific
    BulkPull,
    BulkPullAccount,
    BulkPullDeserializeReceiveBlock,
    BulkPullErrorStartingRequest,
    BulkPullFailedAccount,
    BulkPullReceiveBlockFailure,
    BulkPullRequestFailure,
    BulkPush,
    FrontierReq,
    FrontierConfirmationFailed,
    FrontierConfirmationSuccessful,
    ErrorSocketClose,
    RequestUnderflow,

    // vote specific
    VoteValid,
    VoteReplay,
    VoteIndeterminate,
    VoteInvalid,
    VoteOverflow,

    // election specific
    VoteNew,
    VoteProcessed,
    VoteCached,
    LateBlock,
    LateBlockSeconds,
    ElectionBlockConflict,
    ElectionRestart,
    ElectionNotConfirmed,
    ElectionHintedOverflow,
    ElectionHintedConfirmed,
    ElectionHintedDrop,
    GenerateVote,
    GenerateVoteNormal,
    GenerateVoteFinal,

    // election types
    Normal,
    Hinted,

    // received messages
    InvalidHeader,
    InvalidMessageType,
    InvalidKeepaliveMessage,
    InvalidPublishMessage,
    InvalidConfirmReqMessage,
    InvalidConfirmAckMessage,
    InvalidNodeIdHandshakeMessage,
    InvalidTelemetryReqMessage,
    InvalidTelemetryAckMessage,
    InvalidBulkPullMessage,
    InvalidBulkPullAccountMessage,
    InvalidFrontierReqMessage,
    InvalidAscPullReqMessage,
    InvalidAscPullAckMessage,
    MessageTooBig,
    OutdatedVersion,

    // tcp
    TcpAcceptSuccess,
    TcpAcceptFailure,
    TcpWriteDrop,
    TcpWriteNoSocketDrop,
    TcpExcluded,
    TcpMaxPerIp,
    TcpMaxPerSubnetwork,
    TcpSilentConnectionDrop,
    TcpIoTimeoutDrop,
    TcpConnectError,
    TcpReadError,
    TcpWriteError,

    // ipc
    Invocations,

    // peering
    Handshake,

    // confirmation height
    BlocksConfirmed,
    BlocksConfirmedUnbounded,
    BlocksConfirmedBounded,

    // [request] aggregator
    AggregatorAccepted,
    AggregatorDropped,

    // requests
    RequestsCachedHashes,
    RequestsGeneratedHashes,
    RequestsCachedVotes,
    RequestsGeneratedVotes,
    RequestsCachedLateHashes,
    RequestsCachedLateVotes,
    RequestsCannotVote,
    RequestsUnknown,

    // duplicate
    DuplicatePublish,

    // telemetry
    InvalidSignature,
    DifferentGenesisHash,
    NodeIdMismatch,
    GenesisMismatch,
    RequestWithinProtectionCacheZone,
    NoResponseReceived,
    UnsolicitedTelemetryAck,
    FailedSendTelemetryReq,
    EmptyPayload,
    CleanupOutdated,
    CleanupDead,

    // vote generator
    GeneratorBroadcasts,
    GeneratorReplies,
    GeneratorRepliesDiscarded,
    GeneratorSpacing,

    // hinting
    MissingBlock,

    // bootstrap server
    Response,
    WriteDrop,
    WriteError,
    Blocks,
    Drop,
    BadCount,
    ResponseBlocks,
    ResponseAccountInfo,
    ChannelFull,

    // backlog
    Activated,

    // active
    Insert,
    InsertFailed,

    // unchecked
    Put,
    Satisfied,
    Trigger,
}

impl DetailType {
    pub fn as_str(&self) -> &'static str {
        to_variant_name(self).unwrap_or_default()
    }
}

/// Direction of the stat. If the direction is irrelevant, use In
#[derive(FromPrimitive)]
#[repr(u8)]
pub enum Direction {
    In,
    Out,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::In => "in",
            Direction::Out => "out",
        }
    }
}

static LOG_COUNT: Lazy<Mutex<Option<FileWriter>>> = Lazy::new(|| Mutex::new(None));
static LOG_SAMPLE: Lazy<Mutex<Option<FileWriter>>> = Lazy::new(|| Mutex::new(None));

pub struct Stats {
    config: StatsConfig,
    mutables: Mutex<StatMutables>,
}

impl Stats {
    pub fn new(config: StatsConfig) -> Self {
        let default_interval = config.interval;
        let default_capacity = config.capacity;
        Self {
            config,
            mutables: Mutex::new(StatMutables {
                stopped: false,
                entries: HashMap::new(),
                log_last_count_writeout: Instant::now(),
                log_last_sample_writeout: Instant::now(),
                timestamp: Instant::now(),
                default_interval,
                default_capacity,
            }),
        }
    }

    /// Add `value` to stat. If sampling is configured, this will update the current sample.
    ///
    /// # Arguments
    /// * `type` Main statistics type
    /// * `detail` Detail type, or detail::none to register on type-level only
    /// * `dir` Direction
    /// * `value` The amount to add
    /// * `detail_only` If `true`, only update the detail-level counter
    pub fn add(
        &self,
        stat_type: StatType,
        detail: DetailType,
        dir: Direction,
        value: u64,
        detail_only: bool,
    ) {
        if value == 0 {
            return;
        }

        const NO_DETAIL_MASK: u32 = 0xffff00ff;
        let key = key_of(stat_type, detail, dir);
        let _ = self.update(key, value);

        // Optionally update at type-level as well
        if !detail_only && (key & NO_DETAIL_MASK) != key {
            let _ = self.update(key & NO_DETAIL_MASK, value);
        }
    }

    pub fn inc(&self, stat_type: StatType, detail: DetailType, dir: Direction) {
        self.add(stat_type, detail, dir, 1, false)
    }

    pub fn inc_detail_only(&self, stat_type: StatType, detail: DetailType, dir: Direction) {
        self.add(stat_type, detail, dir, 1, true)
    }

    /// Update count and sample
    ///
    /// # Arguments
    /// * `key` a key constructor from `StatType`, `DetailType` and `Direction`
    /// * `value` Amount to add to the counter
    fn update(&self, key: u32, value: u64) -> anyhow::Result<()> {
        let now = Instant::now();

        let mut lock = self.mutables.lock().unwrap();
        if !lock.stopped {
            {
                let entry = lock.get_entry_default(key);
                entry.counter.add(value, true);
            }

            let duration = now - lock.log_last_count_writeout;
            if self.config.log_interval_counters > 0
                && duration.as_millis() > self.config.log_interval_counters as u128
            {
                let mut log_count = LOG_COUNT.lock().unwrap();
                let writer = match log_count.as_mut() {
                    Some(x) => x,
                    None => {
                        let writer = FileWriter::new(&self.config.log_counters_filename)?;
                        log_count.get_or_insert(writer)
                    }
                };

                lock.log_counters_impl(writer, &self.config)?;
                lock.log_last_count_writeout = now;
            }

            let entry = lock.get_entry_default(key);
            // Samples
            if self.config.sampling_enabled && entry.sample_interval > 0 {
                entry.sample_current.add(value, false);

                let duration = now - entry.sample_start_time;
                if duration.as_millis() > entry.sample_interval as u128 {
                    entry.sample_start_time = now;

                    // Make a snapshot of samples for thread safety and to get a stable container
                    entry.sample_current.set_timestamp(SystemTime::now());
                    if let Some(samples) = entry.samples.as_mut() {
                        samples.push_back(entry.sample_current.clone());
                    }
                    entry.sample_current.set_value(0);

                    // Log sink
                    let duration = now - lock.log_last_sample_writeout;
                    if self.config.log_interval_samples > 0
                        && duration.as_millis() > self.config.log_interval_samples as u128
                    {
                        let mut log_sample = LOG_SAMPLE.lock().unwrap();
                        let writer = match log_sample.as_mut() {
                            Some(x) => x,
                            None => {
                                let writer = FileWriter::new(&self.config.log_samples_filename)?;
                                log_sample.get_or_insert(writer)
                            }
                        };

                        lock.log_samples_impl(writer, &self.config)?;
                        lock.log_last_sample_writeout = now;
                    }
                }
            }
        }

        Ok(())
    }

    /// Log counters to the given log link
    pub fn log_counters(&self, sink: &mut dyn StatsLogSink) -> Result<()> {
        let lock = self.mutables.lock().unwrap();
        lock.log_counters_impl(sink, &self.config)
    }

    /// Log samples to the given log sink
    pub fn log_samples(&self, sink: &mut dyn StatsLogSink) -> Result<()> {
        let lock = self.mutables.lock().unwrap();
        lock.log_samples_impl(sink, &self.config)
    }

    /// Define histogram bins. Values are clamped into the first and last bins, but a catch-all bin on one or both
    /// ends can be defined.
    ///
    /// # Examples:
    ///
    ///  // Uniform histogram, total range 12, and 12 bins (each bin has width 1)
    ///  define_histogram (type::vote, detail::confirm_ack, dir::in, {1,13}, 12);
    ///
    ///  // Specific bins matching closed intervals [1,4] [5,19] [20,99]
    ///  define_histogram (type::vote, detail::something, dir::out, {1,5,20,100});
    ///
    ///  // Logarithmic bins matching half-open intervals [1..10) [10..100) [100 1000)
    ///  define_histogram(type::vote, detail::log, dir::out, {1,10,100,1000});
    pub fn define_histogram(
        &self,
        stat_type: StatType,
        detail: DetailType,
        dir: Direction,
        intervals: &[u64],
        bin_count: u64,
    ) {
        let mut lock = self.mutables.lock().unwrap();
        let entry = lock.get_entry_default(key_of(stat_type, detail, dir));
        entry.histogram = Some(StatHistogram::new(intervals, bin_count));
    }

    /// Update histogram
    ///
    /// # Examples:
    ///
    /// // Add 1 to the bin representing a 4-item vbh
    ///  stats.update_histogram(type::vote, detail::confirm_ack, dir::in, 4, 1)
    ///
    ///  // Add 5 to the second bin where 17 falls
    ///  stats.update_histogram(type::vote, detail::something, dir::in, 17, 5)
    ///
    ///  // Add 3 to the last bin as the histogram clamps. You can also add a final bin with maximum end value to effectively prevent this.
    ///  stats.update_histogram(type::vote, detail::log, dir::out, 1001, 3)
    pub fn update_histogram(
        &self,
        stat_type: StatType,
        detail: DetailType,
        dir: Direction,
        index: u64,
        addend: u64,
    ) {
        let mut lock = self.mutables.lock().unwrap();
        let entry = lock.get_entry_default(key_of(stat_type, detail, dir));
        if let Some(histogram) = entry.histogram.as_mut() {
            histogram.add(index, addend);
        }
    }

    pub fn get_histogram(
        &self,
        stat_type: StatType,
        detail: DetailType,
        dir: Direction,
    ) -> Option<StatHistogram> {
        let mut lock = self.mutables.lock().unwrap();
        let entry = lock.get_entry_default(key_of(stat_type, detail, dir));
        entry.histogram.clone()
    }

    /// Returns the duration since `clear()` was last called, or node startup if it's never called.
    pub fn last_reset(&self) -> Duration {
        let lock = self.mutables.lock().unwrap();
        lock.timestamp.elapsed()
    }

    /// Clear all stats
    pub fn clear(&self) {
        let mut lock = self.mutables.lock().unwrap();
        lock.entries.clear();
        lock.timestamp = Instant::now();
    }

    /// Call this to override the default sample interval and capacity, for a specific stat entry.
    /// This must be called before any stat entries are added, as part of the node initialiation.
    pub fn configure(
        &self,
        stat_type: StatType,
        detail: DetailType,
        dir: Direction,
        interval: usize,
        capacity: usize,
    ) {
        self.mutables
            .lock()
            .unwrap()
            .get_entry(key_of(stat_type, detail, dir), interval, capacity);
    }

    /// Disables sampling for a given type/detail/dir combination
    pub fn disable_sampling(&self, stat_type: StatType, detail: DetailType, dir: Direction) {
        self.mutables
            .lock()
            .unwrap()
            .get_entry_default(key_of(stat_type, detail, dir))
            .sample_interval = 0;
    }

    /// Returns current value for the given counter at the type level
    pub fn count(&self, stat_type: StatType, detail: DetailType, dir: Direction) -> u64 {
        self.mutables
            .lock()
            .unwrap()
            .get_entry_default(key_of(stat_type, detail, dir))
            .counter
            .get_value()
    }

    /// Stop stats being output
    pub fn stop(&self) {
        self.mutables.lock().unwrap().stopped = true;
    }
}

/// Constructs a key given type, detail and direction. This is used as input to update(...) and get_entry(...)
fn key_of(stat_type: StatType, detail: DetailType, dir: Direction) -> u32 {
    (stat_type as u32) << 16 | (detail as u32) << 8 | dir as u32
}

pub fn stat_type_as_str(key: u32) -> Result<&'static str> {
    let stat_type: StatType =
        FromPrimitive::from_u32(key >> 16 & 0x000000ff).ok_or_else(|| anyhow!("invalid key"))?;
    Ok(stat_type.as_str())
}

pub fn stat_detail_as_str(key: u32) -> Result<&'static str> {
    let detail: DetailType =
        FromPrimitive::from_u32(key >> 8 & 0x000000ff).ok_or_else(|| anyhow!("invalid key"))?;
    Ok(detail.as_str())
}

pub fn stat_dir_as_str(key: u32) -> Result<&'static str> {
    let stat_dir: Direction =
        FromPrimitive::from_u32(key & 0x000000ff).ok_or_else(|| anyhow!("invalid key"))?;
    Ok(stat_dir.as_str())
}

struct StatMutables {
    /// Whether stats should be output
    stopped: bool,

    /// Stat entries are sorted by key to simplify processing of log output
    entries: HashMap<u32, StatEntry>,

    log_last_count_writeout: Instant,
    log_last_sample_writeout: Instant,

    /// Time of last clear() call
    timestamp: Instant,

    default_interval: usize,
    default_capacity: usize,
}

impl StatMutables {
    fn get_entry_default(&mut self, key: u32) -> &'_ mut StatEntry {
        self.get_entry(key, self.default_interval, self.default_capacity)
    }

    fn get_entry(&mut self, key: u32, interval: usize, capacity: usize) -> &'_ mut StatEntry {
        self.entries
            .entry(key)
            .or_insert_with(|| StatEntry::new(capacity, interval))
    }

    /// Unlocked implementation of log_samples() to avoid using recursive locking
    fn log_samples_impl(&self, sink: &mut dyn StatsLogSink, config: &StatsConfig) -> Result<()> {
        sink.begin()?;
        if sink.entries() >= config.log_rotation_count {
            sink.rotate()?;
        }

        if config.log_headers {
            let walltime = SystemTime::now();
            sink.write_header("samples", walltime)?;
        }

        for (&key, value) in &self.entries {
            let type_str = stat_type_as_str(key)?;
            let detail = stat_detail_as_str(key)?;
            let dir = stat_dir_as_str(key)?;

            if let Some(samples) = &value.samples {
                for datapoint in samples {
                    let time = datapoint.get_timestamp();
                    sink.write_entry(time, type_str, detail, dir, datapoint.get_value(), None)?;
                }
            }
        }
        sink.inc_entries();
        sink.finalize();
        Ok(())
    }

    /// Unlocked implementation of log_counters() to avoid using recursive locking
    fn log_counters_impl(&self, sink: &mut dyn StatsLogSink, config: &StatsConfig) -> Result<()> {
        sink.begin()?;
        if sink.entries() >= config.log_rotation_count {
            sink.rotate()?;
        }

        if config.log_headers {
            let walltime = SystemTime::now();
            sink.write_header("counters", walltime)?;
        }

        for (&key, value) in &self.entries {
            let time = value.counter.get_timestamp();
            let type_str = stat_type_as_str(key)?;
            let detail = stat_detail_as_str(key)?;
            let dir = stat_dir_as_str(key)?;
            let histogram = value.histogram.as_ref();
            sink.write_entry(
                time,
                type_str,
                detail,
                dir,
                value.counter.get_value(),
                histogram,
            )?;
        }
        sink.inc_entries();
        sink.finalize();
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specific_bins() {
        let stats = Stats::new(StatsConfig::new());
        stats.define_histogram(
            StatType::Vote,
            DetailType::ConfirmReq,
            Direction::In,
            &[1, 6, 10, 16],
            0,
        );
        stats.update_histogram(StatType::Vote, DetailType::ConfirmReq, Direction::In, 1, 50);
        let histogram_req = stats
            .get_histogram(StatType::Vote, DetailType::ConfirmReq, Direction::In)
            .unwrap();
        assert_eq!(histogram_req.get_bins()[0].value, 50);
    }

    #[test]
    fn uniform_distribution_and_clamping() {
        // Uniform distribution (12 bins, width 1); also test clamping 100 to the last bin
        let stats = Stats::new(StatsConfig::new());
        stats.define_histogram(
            StatType::Vote,
            DetailType::ConfirmAck,
            Direction::In,
            &[1, 13],
            12,
        );
        stats.update_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::In, 1, 1);
        stats.update_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::In, 8, 10);
        stats.update_histogram(
            StatType::Vote,
            DetailType::ConfirmAck,
            Direction::In,
            100,
            1,
        );

        let histogram_ack = stats
            .get_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::In)
            .unwrap();
        assert_eq!(histogram_ack.get_bins()[0].value, 1);
        assert_eq!(histogram_ack.get_bins()[7].value, 10);
        assert_eq!(histogram_ack.get_bins()[11].value, 1);
    }

    #[test]
    fn uniform_distribution() {
        // Uniform distribution (2 bins, width 5); add 1 to each bin
        let stats = Stats::new(StatsConfig::new());
        stats.define_histogram(
            StatType::Vote,
            DetailType::ConfirmAck,
            Direction::Out,
            &[1, 11],
            2,
        );
        stats.update_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::Out, 1, 1);
        stats.update_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::Out, 6, 1);

        let histogram_ack_out = stats
            .get_histogram(StatType::Vote, DetailType::ConfirmAck, Direction::Out)
            .unwrap();
        assert_eq!(histogram_ack_out.get_bins()[0].value, 1);
        assert_eq!(histogram_ack_out.get_bins()[1].value, 1);
    }
}
