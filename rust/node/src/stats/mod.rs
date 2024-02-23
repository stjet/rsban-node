mod histogram;
mod parse_message_error;
mod socket_stats;
mod stats;
mod stats_config;
mod stats_log_sink;

mod ledger_stats;
pub use ledger_stats::LedgerStats;

use rsnano_ledger::BlockStatus;
use rsnano_messages::Message;
pub use socket_stats::SocketStats;
pub use stats::{stat_type_as_str, DetailType, Direction, StatType, Stats};
pub use stats_config::StatsConfig;
pub use stats_log_sink::{FileWriter, JsonWriter, StatsLogSink};

impl From<BlockStatus> for DetailType {
    fn from(value: BlockStatus) -> Self {
        match value {
            BlockStatus::Progress => Self::Progress,
            BlockStatus::BadSignature => Self::BadSignature,
            BlockStatus::Old => Self::Old,
            BlockStatus::NegativeSpend => Self::NegativeSpend,
            BlockStatus::Fork => Self::Fork,
            BlockStatus::Unreceivable => Self::Unreceivable,
            BlockStatus::GapPrevious => Self::GapPrevious,
            BlockStatus::GapSource => Self::GapSource,
            BlockStatus::GapEpochOpenPending => Self::GapEpochOpenPending,
            BlockStatus::OpenedBurnAccount => Self::OpenedBurnAccount,
            BlockStatus::BalanceMismatch => Self::BalanceMismatch,
            BlockStatus::RepresentativeMismatch => Self::RepresentativeMismatch,
            BlockStatus::BlockPosition => Self::BlockPosition,
            BlockStatus::InsufficientWork => Self::InsufficientWork,
        }
    }
}

impl From<&Message> for DetailType {
    fn from(value: &Message) -> Self {
        value.message_type().into()
    }
}
