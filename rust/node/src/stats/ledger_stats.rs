use std::sync::Arc;

use crate::ledger::LedgerObserver;

use super::{DetailType, Direction, Stat, StatType};

pub struct LedgerStats {
    stats: Arc<Stat>,
}

impl LedgerStats {
    pub fn new(stats: Arc<Stat>) -> Self {
        Self { stats }
    }
}

impl LedgerObserver for LedgerStats {
    fn blocks_cemented(&self, cemented_count: u64) {
        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In,
            cemented_count,
            false,
        );

        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            cemented_count,
            false,
        );
    }

    fn rollback_legacy_send(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Send, Direction::In);
    }

    fn rollback_legacy_receive(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Receive, Direction::In);
    }
    fn rollback_legacy_open(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Open, Direction::In);
    }
    fn rollback_legacy_change(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Change, Direction::In);
    }
    fn rollback_send(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Send, Direction::In);
    }
    fn rollback_receive(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Receive, Direction::In);
    }
    fn rollback_open(&self) {
        let _ = self
            .stats
            .inc(StatType::Rollback, DetailType::Open, Direction::In);
    }
}
