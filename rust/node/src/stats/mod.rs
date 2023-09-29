mod histogram;
mod message_parse_status;
mod socket_stats;
mod stats;
mod stats_config;
mod stats_log_sink;

mod ledger_stats;
pub use ledger_stats::LedgerStats;

use rsnano_ledger::ProcessResult;
pub use socket_stats::SocketStats;
pub use stats::{stat_type_as_str, DetailType, Direction, StatType, Stats};
pub use stats_config::StatsConfig;
pub use stats_log_sink::{FileWriter, JsonWriter, StatsLogSink};

impl From<ProcessResult> for DetailType {
    fn from(value: ProcessResult) -> Self {
        match value {
            ProcessResult::Progress => Self::Progress,
            ProcessResult::BadSignature => Self::BadSignature,
            ProcessResult::Old => Self::Old,
            ProcessResult::NegativeSpend => Self::NegativeSpend,
            ProcessResult::Fork => Self::Fork,
            ProcessResult::Unreceivable => Self::Unreceivable,
            ProcessResult::GapPrevious => Self::GapPrevious,
            ProcessResult::GapSource => Self::GapSource,
            ProcessResult::GapEpochOpenPending => Self::GapEpochOpenPending,
            ProcessResult::OpenedBurnAccount => Self::OpenedBurnAccount,
            ProcessResult::BalanceMismatch => Self::BalanceMismatch,
            ProcessResult::RepresentativeMismatch => Self::RepresentativeMismatch,
            ProcessResult::BlockPosition => Self::BlockPosition,
            ProcessResult::InsufficientWork => Self::InsufficientWork,
        }
    }
}
