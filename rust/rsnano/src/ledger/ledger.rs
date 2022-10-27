use rand::{thread_rng, Rng};

use crate::{
    core::{Account, Amount, Block, BlockEnum, BlockHash, BlockType, PendingKey},
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
    pub bootstrap_weights: Mutex<HashMap<Account, Amount>>,
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
                    rep_weights.representation_add(info.representative, info.balance);
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

    pub fn is_send(&self, txn: &dyn Transaction, block: &dyn Block) -> bool {
        if block.block_type() != BlockType::State {
            return block.block_type() == BlockType::Send;
        }
        let previous = block.previous();
        /*
         * if block_a does not have a sideband, then is_send()
         * requires that the previous block exists in the database.
         * This is because it must retrieve the balance of the previous block.
         */
        debug_assert!(
            block.sideband().is_some()
                || previous.is_zero()
                || self.store.block().exists(txn, &previous)
        );
        match block.sideband() {
            Some(sideband) => sideband.details.is_send,
            None => {
                if !previous.is_zero() {
                    block.balance() < self.balance(txn, &previous)
                } else {
                    false
                }
            }
        }
    }

    pub fn block_destination(&self, txn: &dyn Transaction, block: &BlockEnum) -> Account {
        match block {
            BlockEnum::Send(send) => send.hashables.destination,
            BlockEnum::State(state) => {
                if self.is_send(txn, state) {
                    state.link().into()
                } else {
                    Account::zero()
                }
            }
            _ => Account::zero(),
        }
    }

    pub fn block_source(&self, txn: &dyn Transaction, block: &BlockEnum) -> BlockHash {
        /*
         * block_source() requires that the previous block of the block
         * passed in exist in the database.  This is because it will try
         * to check account balances to determine if it is a send block.
         */
        debug_assert!(
            block.as_block().previous().is_zero()
                || self.store.block().exists(txn, &block.as_block().previous())
        );

        // If block_a.source () is nonzero, then we have our source.
        // However, universal blocks will always return zero.
        match block {
            BlockEnum::State(state) => {
                if !self.is_send(txn, state) {
                    state.link().into()
                } else {
                    state.source()
                }
            }
            _ => block.as_block().source(),
        }
    }

    pub fn hash_root_random(&self, txn: &dyn Transaction) -> Option<(BlockHash, BlockHash)> {
        if !self.pruning_enabled() {
            self.store
                .block()
                .random(txn)
                .map(|block| (block.as_block().hash(), block.as_block().root().into()))
        } else {
            let mut hash = BlockHash::zero();
            let count = self.cache.block_count.load(Ordering::SeqCst);
            let region = thread_rng().gen_range(0..count);
            // Pruned cache cannot guarantee that pruned blocks are already commited
            if region < self.cache.pruned_count.load(Ordering::SeqCst) {
                hash = self.store.pruned().random(txn).unwrap_or_default();
            }
            if hash.is_zero() {
                self.store
                    .block()
                    .random(txn)
                    .map(|block| (block.as_block().hash(), block.as_block().root().into()))
            } else {
                Some((hash, BlockHash::zero()))
            }
        }
    }

    /// Vote weight of an account
    pub fn weight(&self, account: &Account) -> Amount {
        if self.check_bootstrap_weights.load(Ordering::SeqCst) {
            if self.cache.block_count.load(Ordering::SeqCst) < self.bootstrap_weight_max_blocks() {
                let weights = self.bootstrap_weights.lock().unwrap();
                if let Some(&weight) = weights.get(account) {
                    return weight;
                }
            } else {
                self.check_bootstrap_weights.store(false, Ordering::SeqCst);
            }
        }

        self.cache.rep_weights.representation_get(account)
    }
}
