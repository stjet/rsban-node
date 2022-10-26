use crate::{
    core::{Account, Amount, BlockHash, PendingKey},
    stats::Stat,
    utils::create_property_tree,
};
use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use super::{
    datastore::{Store, Transaction},
    GenerateCache, LedgerCache, LedgerConstants, RepWeights,
};

pub struct Ledger {
    handle: *mut c_void,
    store: Arc<dyn Store>,
    pub cache: Arc<LedgerCache>,
    constants: LedgerConstants,
    stats: Arc<Stat>,
    pruning: AtomicBool,
    bootstrap_weight_max_blocks: AtomicU64,
    pub check_bootstrap_weights: AtomicBool,
    pub bootstrap_weights: Mutex<HashMap<Account, u128>>,
}

impl Ledger {
    pub fn new(
        handle: *mut c_void,
        store: Arc<dyn Store>,
        constants: LedgerConstants,
        stats: Arc<Stat>,
        generate_cache: &GenerateCache,
    ) -> anyhow::Result<Self> {
        let mut ledger = Self {
            handle,
            store,
            cache: Arc::new(LedgerCache::new()),
            constants,
            stats,
            pruning: AtomicBool::new(false),
            bootstrap_weight_max_blocks: AtomicU64::new(1),
            check_bootstrap_weights: AtomicBool::new(true),
            bootstrap_weights: Mutex::new(HashMap::new()),
        };

        ledger.initialize(generate_cache)?;

        Ok(ledger)
    }

    fn initialize(&mut self, generate_cache: &GenerateCache) -> anyhow::Result<()> {
        if generate_cache.reps || generate_cache.account_count || generate_cache.block_count {
            self.store.account().for_each_par(&|_txn, mut i, n| {
                let mut block_count = 0;
                let mut account_count = 0;
                let rep_weights = RepWeights::new();
                while !i.eq(n.as_ref()) {
                    let info = i.current().unwrap().1;
                    block_count += info.block_count;
                    account_count += 1;
                    rep_weights.representation_add(info.representative, info.balance.number());
                    i.next();
                }
                self.cache
                    .block_count
                    .fetch_add(block_count, Ordering::SeqCst);
                self.cache
                    .account_count
                    .fetch_add(account_count, Ordering::SeqCst);
                self.cache.rep_weights.copy_from(&rep_weights);
            });
        }

        if generate_cache.cemented_count {
            self.store
                .confirmation_height()
                .for_each_par(&|_txn, mut i, n| {
                    let mut cemented_count = 0;
                    while !i.eq(n.as_ref()) {
                        cemented_count += i.current().unwrap().1.height;
                        i.next();
                    }
                    self.cache
                        .cemented_count
                        .fetch_add(cemented_count, Ordering::SeqCst);
                });
        }

        let transaction = self.store.tx_begin_read()?;
        self.cache.pruned_count.fetch_add(
            self.store.pruned().count(transaction.txn()) as u64,
            Ordering::SeqCst,
        );

        // Final votes requirement for confirmation canary block
        if let Some(conf_height) = self.store.confirmation_height().get(
            transaction.txn(),
            &self.constants.final_votes_canary_account,
        ) {
            self.cache.final_votes_confirmation_canary.store(
                conf_height.height >= self.constants.final_votes_canary_height,
                Ordering::SeqCst,
            );
        }
        Ok(())
    }

    pub fn pruning_enabled(&self) -> bool {
        self.pruning.load(Ordering::SeqCst)
    }

    pub fn enable_pruning(&self) {
        self.pruning.store(true, Ordering::SeqCst);
    }

    pub fn bootstrap_weight_max_blocks(&self) -> u64 {
        self.bootstrap_weight_max_blocks.load(Ordering::SeqCst)
    }

    pub fn set_bootstrap_weight_max_blocks(&self, max: u64) {
        self.bootstrap_weight_max_blocks
            .store(max, Ordering::SeqCst)
    }

    pub fn block_or_pruned_exists(&self, block: &BlockHash) -> bool {
        let txn = self.store.tx_begin_read().unwrap();
        self.block_or_pruned_exists_txn(txn.txn(), block)
    }

    pub fn block_or_pruned_exists_txn(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool {
        self.store.pruned().exists(txn, hash) || self.store.block().exists(txn, hash)
    }

    /// Balance for account containing the given block at the time of the block.
    /// Returns 0 if the block was not found
    pub fn balance(&self, txn: &dyn Transaction, hash: &BlockHash) -> Amount {
        if hash.is_zero() {
            Amount::zero()
        } else {
            self.store.block().balance(txn, hash)
        }
    }

    /// Balance for account containing the given block at the time of the block.
    /// Returns Err if the pruning is enabled and the block was not found.
    pub fn balance_safe(&self, txn: &dyn Transaction, hash: &BlockHash) -> anyhow::Result<Amount> {
        if self.pruning_enabled() && !hash.is_zero() && !self.store.block().exists(txn, hash) {
            bail!("block not found");
        }

        Ok(self.balance(txn, hash))
    }

    /// Balance for account by account number
    pub fn account_balance(
        &self,
        txn: &dyn Transaction,
        account: &Account,
        only_confirmed: bool,
    ) -> Amount {
        if only_confirmed {
            match self.store.confirmation_height().get(txn, account) {
                Some(info) => self.balance(txn, &info.frontier),
                None => Amount::zero(),
            }
        } else {
            match self.store.account().get(txn, account) {
                Some(info) => info.balance,
                None => Amount::zero(),
            }
        }
    }

    pub fn account_receivable(
        &self,
        txn: &dyn Transaction,
        account: &Account,
        only_confirmed: bool,
    ) -> Amount {
        let mut result = Amount::zero();
        let end = Account::from(account.number() + 1);
        let mut i = self
            .store
            .pending()
            .begin_at_key(txn, &PendingKey::new(*account, BlockHash::zero()));
        let n = self
            .store
            .pending()
            .begin_at_key(txn, &PendingKey::new(end, BlockHash::zero()));
        while !i.eq(n.as_ref()) {
            if let Some((key, info)) = i.current() {
                if only_confirmed {
                    if self.block_confirmed(txn, &key.hash) {
                        result += info.amount;
                    }
                } else {
                    result += info.amount;
                }
            };
            i.next();
        }

        result
    }

    pub fn block_confirmed(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool {
        if self.store.pruned().exists(txn, hash) {
            return true;
        }

        match self.store.block().get(txn, hash) {
            Some(block) => {
                let mut account = block.as_block().account();
                let sideband = &block.as_block().sideband().unwrap();
                if account.is_zero() {
                    account = sideband.account;
                }
                let confirmed = match self.store.confirmation_height().get(txn, &account) {
                    Some(info) => info.height >= sideband.height,
                    None => false,
                };
                confirmed
            }
            None => false,
        }
    }

    pub fn block_text(&self, hash: &BlockHash) -> anyhow::Result<String> {
        let txn = self.store.tx_begin_read()?;
        match self.store.block().get(txn.txn(), hash) {
            Some(block) => {
                let mut writer = create_property_tree();
                block.as_block().serialize_json(writer.as_mut())?;
                Ok(writer.to_json())
            }
            None => Ok(String::new()),
        }
    }
}
