use std::sync::Arc;

use crate::stats::{DetailType, Direction, StatType, Stats};
use rsnano_core::{Account, AccountInfo, ConfirmationHeightInfo};
use rsnano_store_lmdb::LmdbStore;
use rsnano_store_traits::{Store, Transaction};

pub struct BacklogPopulation {
    store: Arc<LmdbStore>,
    stats: Arc<Stats>,
    activate_callback: Option<ActivateCallback>,
}

pub type ActivateCallback =
    Box<dyn Fn(&dyn Transaction, &Account, &AccountInfo, &ConfirmationHeightInfo)>;

impl BacklogPopulation {
    pub fn new(store: Arc<LmdbStore>, stats: Arc<Stats>) -> Self {
        Self {
            store,
            stats,
            activate_callback: None,
        }
    }

    pub fn set_activate_callback(&mut self, callback: ActivateCallback) {
        self.activate_callback = Some(callback);
    }

    pub fn activate(&self, txn: &dyn Transaction, account: &Account) {
        let account_info = match self.store.account().get(txn, account) {
            Some(info) => info,
            None => {
                return;
            }
        };

        let conf_info = self
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
