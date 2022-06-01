use anyhow::Result;
use bounded_vec_deque::BoundedVecDeque;
use num::FromPrimitive;
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

/// Primary statistics type
#[repr(u8)]
#[derive(FromPrimitive)]
enum StatType {
    TrafficUdp,
    TrafficTcp,
    Error,
    Message,
    Block,
    Ledger,
    Rollback,
    Bootstrap,
    Vote,
    Election,
    HttpCallback,
    Peering,
    Ipc,
    Tcp,
    Udp,
    ConfirmationHeight,
    ConfirmationObserver,
    Drop,
    Aggregator,
    Requests,
    Filter,
    Telemetry,
    VoteGenerator,
}

// Optional detail type
#[repr(u8)]
#[derive(FromPrimitive)]
pub enum DetailType {
    All = 0,

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

    // message specific
    Keepalive,
    Publish,
    RepublishVote,
    ConfirmReq,
    ConfirmAck,
    NodeIdHandshake,
    TelemetryReq,
    TelemetryAck,

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
    VoteCached,
    LateBlock,
    LateBlockSeconds,
    ElectionStart,
    ElectionBlockConflict,
    ElectionDifficultyUpdate,
    ElectionDropExpired,
    ElectionDropOverflow,
    ElectionDropAll,
    ElectionRestart,
    ElectionConfirmed,
    ElectionNotConfirmed,

    // udp
    Blocking,
    Overflow,
    InvalidHeader,
    InvalidMessageType,
    InvalidKeepaliveMessage,
    InvalidPublishMessage,
    InvalidConfirmReqMessage,
    InvalidConfirmAckMessage,
    InvalidNodeIdHandshakeMessage,
    InvalidTelemetryReqMessage,
    InvalidTelemetryAckMessage,
    OutdatedVersion,
    UdpMaxPerIp,
    UdpMaxPerSubnetwork,

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
    RequestWithinProtectionCacheZone,
    NoResponseReceived,
    UnsolicitedTelemetryAck,
    FailedSendTelemetryReq,

    // vote generator
    GeneratorBroadcasts,
    GeneratorReplies,
    GeneratorRepliesDiscarded,
    GeneratorSpacing,
}

impl DetailType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DetailType::All => "all",
            DetailType::BadSender => "bad_sender",
            DetailType::BulkPull => "bulk_pull",
            DetailType::BulkPullAccount => "bulk_pull_account",
            DetailType::BulkPullDeserializeReceiveBlock => "bulk_pull_deserialize_receive_block",
            DetailType::BulkPullErrorStartingRequest => "bulk_pull_error_starting_request",
            DetailType::BulkPullFailedAccount => "bulk_pull_failed_account",
            DetailType::BulkPullReceiveBlockFailure => "bulk_pull_receive_block_failure",
            DetailType::BulkPullRequestFailure => "bulk_pull_request_failure",
            DetailType::BulkPush => "bulk_push",
            DetailType::ActiveQuorum => "observer_confirmation_active_quorum",
            DetailType::ActiveConfHeight => "observer_confirmation_active_conf_height",
            DetailType::InactiveConfHeight => "observer_confirmation_inactive",
            DetailType::ErrorSocketClose => "error_socket_close",
            DetailType::RequestUnderflow => "request_underflow",
            DetailType::Change => "change",
            DetailType::ConfirmAck => "confirm_ack",
            DetailType::NodeIdHandshake => "node_id_handshake",
            DetailType::ConfirmReq => "confirm_req",
            DetailType::Fork => "fork",
            DetailType::Old => "old",
            DetailType::GapPrevious => "gap_previous",
            DetailType::GapSource => "gap_source",
            DetailType::RollbackFailed => "rollback_failed",
            DetailType::FrontierConfirmationFailed => "frontier_confirmation_failed",
            DetailType::FrontierConfirmationSuccessful => "frontier_confirmation_successful",
            DetailType::FrontierReq => "frontier_req",
            DetailType::Handshake => "handshake",
            DetailType::HttpCallback => "http_callback",
            DetailType::Initiate => "initiate",
            DetailType::InitiateLegacyAge => "initiate_legacy_age",
            DetailType::InitiateLazy => "initiate_lazy",
            DetailType::InitiateWalletLazy => "initiate_wallet_lazy",
            DetailType::InsufficientWork => "insufficient_work",
            DetailType::Invocations => "invocations",
            DetailType::Keepalive => "keepalive",
            DetailType::Open => "open",
            DetailType::Publish => "publish",
            DetailType::Receive => "receive",
            DetailType::RepublishVote => "republish_vote",
            DetailType::Send => "send",
            DetailType::TelemetryReq => "telemetry_req",
            DetailType::TelemetryAck => "telemetry_ack",
            DetailType::StateBlock => "state_block",
            DetailType::EpochBlock => "epoch_block",
            DetailType::VoteValid => "vote_valid",
            DetailType::VoteReplay => "vote_replay",
            DetailType::VoteIndeterminate => "vote_indeterminate",
            DetailType::VoteInvalid => "vote_invalid",
            DetailType::VoteOverflow => "vote_overflow",
            DetailType::VoteNew => "vote_new",
            DetailType::VoteCached => "vote_cached",
            DetailType::LateBlock => "late_block",
            DetailType::LateBlockSeconds => "late_block_seconds",
            DetailType::ElectionStart => "election_start",
            DetailType::ElectionBlockConflict => "election_block_conflict",
            DetailType::ElectionDifficultyUpdate => "election_difficulty_update",
            DetailType::ElectionDropExpired => "election_drop_expired",
            DetailType::ElectionDropOverflow => "election_drop_overflow",
            DetailType::ElectionDropAll => "election_drop_all",
            DetailType::ElectionRestart => "election_restart",
            DetailType::ElectionConfirmed => "election_confirmed",
            DetailType::ElectionNotConfirmed => "election_not_confirmed",
            DetailType::Blocking => "blocking",
            DetailType::Overflow => "overflow",
            DetailType::TcpAcceptSuccess => "accept_success",
            DetailType::TcpAcceptFailure => "accept_failure",
            DetailType::TcpWriteDrop => "tcp_write_drop",
            DetailType::TcpWriteNoSocketDrop => "tcp_write_no_socket_drop",
            DetailType::TcpExcluded => "tcp_excluded",
            DetailType::TcpMaxPerIp => "tcp_max_per_ip",
            DetailType::TcpMaxPerSubnetwork => "tcp_max_per_subnetwork",
            DetailType::TcpSilentConnectionDrop => "tcp_silent_connection_drop",
            DetailType::TcpIoTimeoutDrop => "tcp_io_timeout_drop",
            DetailType::TcpConnectError => "tcp_connect_error",
            DetailType::TcpReadError => "tcp_read_error",
            DetailType::TcpWriteError => "tcp_write_error",
            DetailType::UnreachableHost => "unreachable_host",
            DetailType::InvalidHeader => "invalid_header",
            DetailType::InvalidMessageType => "invalid_message_type",
            DetailType::InvalidKeepaliveMessage => "invalid_keepalive_message",
            DetailType::InvalidPublishMessage => "invalid_publish_message",
            DetailType::InvalidConfirmReqMessage => "invalid_confirm_req_message",
            DetailType::InvalidConfirmAckMessage => "invalid_confirm_ack_message",
            DetailType::InvalidNodeIdHandshakeMessage => "invalid_node_id_handshake_message",
            DetailType::InvalidTelemetryReqMessage => "invalid_telemetry_req_message",
            DetailType::InvalidTelemetryAckMessage => "invalid_telemetry_ack_message",
            DetailType::OutdatedVersion => "outdated_version",
            DetailType::UdpMaxPerIp => "udp_max_per_ip",
            DetailType::UdpMaxPerSubnetwork => "udp_max_per_subnetwork",
            DetailType::BlocksConfirmed => "blocks_confirmed",
            DetailType::BlocksConfirmedUnbounded => "blocks_confirmed_unbounded",
            DetailType::BlocksConfirmedBounded => "blocks_confirmed_bounded",
            DetailType::AggregatorAccepted => "aggregator_accepted",
            DetailType::AggregatorDropped => "aggregator_dropped",
            DetailType::RequestsCachedHashes => "requests_cached_hashes",
            DetailType::RequestsGeneratedHashes => "requests_generated_hashes",
            DetailType::RequestsCachedVotes => "requests_cached_votes",
            DetailType::RequestsGeneratedVotes => "requests_generated_votes",
            DetailType::RequestsCachedLateHashes => "requests_cached_late_hashes",
            DetailType::RequestsCachedLateVotes => "requests_cached_late_votes",
            DetailType::RequestsCannotVote => "requests_cannot_vote",
            DetailType::RequestsUnknown => "requests_unknown",
            DetailType::DuplicatePublish => "duplicate_publish",
            DetailType::DifferentGenesisHash => "different_genesis_hash",
            DetailType::InvalidSignature => "invalid_signature",
            DetailType::NodeIdMismatch => "node_id_mismatch",
            DetailType::RequestWithinProtectionCacheZone => "request_within_protection_cache_zone",
            DetailType::NoResponseReceived => "no_response_received",
            DetailType::UnsolicitedTelemetryAck => "unsolicited_telemetry_ack",
            DetailType::FailedSendTelemetryReq => "failed_send_telemetry_req",
            DetailType::GeneratorBroadcasts => "generator_broadcasts",
            DetailType::GeneratorReplies => "generator_replies",
            DetailType::GeneratorRepliesDiscarded => "generator_replies_discarded",
            DetailType::GeneratorSpacing => "generator_spacing",
            DetailType::InvalidNetwork => "invalid_network",
        }
    }
}

/// Direction of the stat. If the direction is irrelevant, use In
#[derive(FromPrimitive)]
#[repr(u8)]
enum Direction {
    In,
    Out,
}

pub struct Stat {
    config: StatConfig,
}

impl Stat {
    pub fn new(config: StatConfig) -> Self {
        Self { config }
    }
}

pub fn stat_type_as_str(key: u32) -> Result<&'static str> {
    let stat_type: StatType =
        FromPrimitive::from_u32(key >> 16 & 0x000000ff).ok_or_else(|| anyhow!("invalid key"))?;
    let str = match stat_type {
        StatType::Ipc => "ipc",
        StatType::Block => "block",
        StatType::Bootstrap => "bootstrap",
        StatType::Error => "error",
        StatType::HttpCallback => "http_callback",
        StatType::Ledger => "ledger",
        StatType::Tcp => "tcp",
        StatType::Udp => "udp",
        StatType::Peering => "peering",
        StatType::Rollback => "rollback",
        StatType::TrafficUdp => "traffic_udp",
        StatType::TrafficTcp => "traffic_tcp",
        StatType::Vote => "vote",
        StatType::Election => "election",
        StatType::Message => "message",
        StatType::ConfirmationObserver => "observer",
        StatType::ConfirmationHeight => "confirmation_height",
        StatType::Drop => "drop",
        StatType::Aggregator => "aggregator",
        StatType::Requests => "requests",
        StatType::Filter => "filter",
        StatType::Telemetry => "telemetry",
        StatType::VoteGenerator => "vote_generator",
    };
    Ok(str)
}

pub fn stat_detail_as_str(key: u32) -> Result<&'static str> {
    let detail: DetailType =
        FromPrimitive::from_u32(key >> 8 & 0x000000ff).ok_or_else(|| anyhow!("invalid key"))?;
    Ok(detail.as_str())
}
