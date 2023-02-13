use std::sync::Arc;

use crate::stats::{DetailType, Direction, StatType, Stats};
use rsnano_core::{Account, AccountInfo, ConfirmationHeightInfo};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

pub struct BacklogPopulation {
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    activate_callback: Option<ActivateCallback>,
}

pub type ActivateCallback =
    Box<dyn Fn(&dyn Transaction, &Account, &AccountInfo, &ConfirmationHeightInfo)>;

impl BacklogPopulation {
    pub fn new(ledger: Arc<Ledger>, stats: Arc<Stats>) -> Self {
        Self {
            ledger,
            stats,
            activate_callback: None,
        }
    }

    pub fn set_activate_callback(&mut self, callback: ActivateCallback) {
        self.activate_callback = Some(callback);
    }

    pub fn activate(&self, txn: &dyn Transaction, account: &Account) {
        let account_info = match self.ledger.store.account().get(txn, account) {
            Some(info) => info,
            None => {
                return;
            }
        };

        let conf_info = self
            .ledger
            .store
            .confirmation_height()
            .get(txn, account)
            .unwrap_or_default();

        // If conf info is empty then it means then it means nothing is confirmed yet
        if conf_info.height < account_info.block_count {
            let _ = self
                .stats
                .inc(StatType::Backlog, DetailType::Activated, Direction::In);
            match &self.activate_callback {
                Some(callback) => callback(txn, account, &account_info, &conf_info),
                None => {
                    debug_assert!(false)
                }
            }
        }
    }
}
