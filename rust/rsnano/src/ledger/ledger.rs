use rand::{thread_rng, Rng};

use crate::{
    core::{
        Account, AccountInfo, Amount, Block, BlockEnum, BlockHash, BlockType,
        ConfirmationHeightInfo, Epoch, Link, PendingKey, QualifiedRoot, Root,
        SignatureVerification,
    },
    ffi::ledger::DependentBlockVisitor,
    ledger::{LedgerProcessor, RollbackVisitor},
    stats::{DetailType, Direction, Stat, StatType},
    utils::create_property_tree,
    DEV_GENESIS,
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, RwLock,
    },
};

use super::{
    datastore::{Store, Transaction, WriteTransaction},
    GenerateCache, LedgerCache, LedgerConstants, RepWeights, RepresentativeVisitor,
};

pub struct UncementedInfo {
    pub cemented_frontier: BlockHash,
    pub frontier: BlockHash,
    pub account: Account,
}

#[derive(PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum ProcessResult {
    Progress,               // Hasn't been seen before, signed correctly
    BadSignature,           // Signature was bad, forged or transmission error
    Old,                    // Already seen and was valid
    NegativeSpend,          // Malicious attempt to spend a negative amount
    Fork,                   // Malicious fork based on previous
    Unreceivable, // Source block doesn't exist, has already been received, or requires an account upgrade (epoch blocks)
    GapPrevious,  // Block marked as previous is unknown
    GapSource,    // Block marked as source is unknown
    GapEpochOpenPending, // Block marked as pending blocks required for epoch open block are unknown
    OpenedBurnAccount, // Block attempts to open the burn account
    BalanceMismatch, // Balance and amount delta don't match
    RepresentativeMismatch, // Representative is changed when it is not allowed
    BlockPosition, // This block cannot follow the previous block
    InsufficientWork, // Insufficient work for this block, even though it passed the minimal validation
}

pub struct ProcessReturn {
    pub code: ProcessResult,
    pub verified: SignatureVerification,
    pub previous_balance: Amount,
}

pub struct Ledger {
    pub store: Arc<dyn Store>,
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
        store: Arc<dyn Store>,
        constants: LedgerConstants,
        stats: Arc<Stat>,
        generate_cache: &GenerateCache,
    ) -> anyhow::Result<Self> {
        let mut ledger = Self {
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

    /// Return account containing block hash
    pub fn account(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Account> {
        self.store.block().account(txn, hash)
    }

    /// Return account containing block hash
    pub fn account_safe(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Account> {
        if !self.pruning_enabled() {
            self.store.block().account(txn, hash)
        } else {
            self.store
                .block()
                .get(txn, hash)
                .map(|block| self.store.block().account_calculated(block.as_block()))
        }
    }

    /// Return absolute amount decrease or increase for block
    pub fn amount(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Amount> {
        self.store.block().get(txn, hash).map(|block| {
            let block_balance = self.balance(txn, hash);
            let previous_balance = self.balance(txn, &block.as_block().previous());
            if block_balance > previous_balance {
                block_balance - previous_balance
            } else {
                previous_balance - block_balance
            }
        })
    }

    /// Return absolute amount decrease or increase for block
    pub fn amount_safe(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Amount> {
        self.store
            .block()
            .get(txn, hash)
            .map(|block| {
                let block_balance = self.balance(txn, hash);
                let previous_balance = self.balance_safe(txn, &block.as_block().previous());
                match previous_balance {
                    Ok(previous) => {
                        if block_balance > previous {
                            Some(block_balance - previous)
                        } else {
                            Some(previous - block_balance)
                        }
                    }
                    Err(_) => None,
                }
            })
            .flatten()
    }

    /// Return latest block for account
    pub fn latest(&self, txn: &dyn Transaction, account: &Account) -> Option<BlockHash> {
        self.store.account().get(txn, account).map(|info| info.head)
    }

    /// Return latest root for account, account number if there are no blocks for this account
    pub fn latest_root(&self, txn: &dyn Transaction, account: &Account) -> Root {
        match self.store.account().get(txn, account) {
            Some(info) => info.head.into(),
            None => account.into(),
        }
    }

    pub fn is_epoch_link(&self, link: &Link) -> bool {
        self.constants.epochs.is_epoch_link(link)
    }

    /// Given the block hash of a send block, find the associated receive block that receives that send.
    /// The send block hash is not checked in any way, it is assumed to be correct.
    /// Return the receive block on success and None on failure
    pub fn find_receive_block_by_send_hash(
        &self,
        txn: &dyn Transaction,
        destination: &Account,
        send_block_hash: &BlockHash,
    ) -> Option<BlockEnum> {
        // get the cemented frontier
        let info = self.store.confirmation_height().get(txn, destination)?;
        let mut possible_receive_block = self.store.block().get(txn, &info.frontier);

        // walk down the chain until the source field of a receive block matches the send block hash
        while let Some(current) = possible_receive_block {
            // if source is non-zero then it is a legacy receive or open block
            let mut source = current.as_block().source();

            // if source is zero then it could be a state block, which needs a different kind of access
            if let BlockEnum::State(state_block) = &current {
                // we read the block from the database, so we expect it to have sideband
                if state_block.sideband().unwrap().details.is_receive {
                    source = state_block.link().into();
                }
            }

            if *send_block_hash == source {
                // we have a match
                return Some(current);
            }

            possible_receive_block = self.store.block().get(txn, &current.as_block().previous());
        }

        None
    }

    pub fn epoch_signer(&self, link: &Link) -> Option<Account> {
        self.constants
            .epochs
            .signer(self.constants.epochs.epoch(link)?)
            .map(|key| key.into())
    }

    pub fn epoch_link(&self, epoch: Epoch) -> Option<Link> {
        self.constants.epochs.link(epoch).cloned()
    }

    pub fn update_account(
        &self,
        txn: &mut dyn WriteTransaction,
        account: &Account,
        old_info: &AccountInfo,
        new_info: &AccountInfo,
    ) {
        if !new_info.head.is_zero() {
            if old_info.head.is_zero() && new_info.open_block == new_info.head {
                self.cache.account_count.fetch_add(1, Ordering::SeqCst);
            }
            if !old_info.head.is_zero() && old_info.epoch != new_info.epoch {
                // store.account ().put won't erase existing entries if they're in different tables
                self.store.account().del(txn, account);
            }
            self.store.account().put(txn, account, new_info);
        } else {
            debug_assert!(!self.store.confirmation_height().exists(txn.txn(), account));
            self.store.account().del(txn, account);
            debug_assert!(self.cache.account_count.load(Ordering::SeqCst) > 0);
            self.cache.account_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn successor(&self, txn: &dyn Transaction, root: &QualifiedRoot) -> Option<BlockEnum> {
        let (mut successor, get_from_previous) = if root.previous.is_zero() {
            match self.store.account().get(txn, &root.root.into()) {
                Some(info) => (Some(info.open_block), false),
                None => (None, true),
            }
        } else {
            (None, true)
        };

        if get_from_previous {
            successor = self.store.block().successor(txn, &root.previous);
        }

        successor
            .map(|hash| self.store.block().get(txn, &hash))
            .flatten()
    }

    pub fn pruning_action(
        &self,
        txn: &mut dyn WriteTransaction,
        hash: &BlockHash,
        batch_size: u64,
    ) -> u64 {
        let mut pruned_count = 0;
        let mut hash = *hash;
        let genesis_hash = { self.constants.genesis.read().unwrap().as_block().hash() };

        while !hash.is_zero() && hash != genesis_hash {
            if let Some(block) = self.store.block().get(txn.txn(), &hash) {
                self.store.block().del(txn, &hash);
                self.store.pruned().put(txn, &hash);
                hash = block.as_block().previous();
                pruned_count += 1;
                self.cache.pruned_count.fetch_add(1, Ordering::SeqCst);
                if pruned_count % batch_size == 0 {
                    txn.commit();
                    txn.renew();
                }
            } else if self.store.pruned().exists(txn.txn(), &hash) {
                hash = BlockHash::zero();
            } else {
                panic!("Error finding block for pruning");
            }
        }

        pruned_count
    }

    /// **Warning:** In C++ the result is sorted in reverse order!
    pub fn unconfirmed_frontiers(&self) -> BTreeMap<u64, Vec<UncementedInfo>> {
        let result = Mutex::new(BTreeMap::<u64, Vec<UncementedInfo>>::new());
        self.store.account().for_each_par(&|txn, mut i, n| {
            let mut unconfirmed_frontiers = Vec::new();
            while !i.eq(n.as_ref()) {
                if let Some((&account, account_info)) = i.current() {
                    if let Some(conf_height_info) =
                        self.store.confirmation_height().get(txn.txn(), &account)
                    {
                        if account_info.block_count != conf_height_info.height {
                            // Always output as no confirmation height has been set on the account yet
                            let height_delta = account_info.block_count - conf_height_info.height;
                            let frontier = account_info.head;
                            let cemented_frontier = conf_height_info.frontier;
                            unconfirmed_frontiers.push((
                                height_delta,
                                UncementedInfo {
                                    cemented_frontier,
                                    frontier,
                                    account,
                                },
                            ))
                        }
                    }
                }
                i.next()
            }

            // Merge results
            let mut guard = result.lock().unwrap();
            for (delta, info) in unconfirmed_frontiers {
                guard.entry(delta).or_default().push(info);
            }
        });

        result.into_inner().unwrap()
    }

    pub fn bootstrap_weight_reached(&self) -> bool {
        self.cache.block_count.load(Ordering::SeqCst) >= self.bootstrap_weight_max_blocks()
    }

    pub fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        account: &Account,
        num_blocks_cemented: u64,
        confirmation_height: u64,
        confirmed_frontier: &BlockHash,
    ) {
        #[cfg(debug_assertions)]
        {
            let conf_height = self
                .store
                .confirmation_height()
                .get(txn.txn(), account)
                .map(|i| i.height)
                .unwrap_or_default();
            let block = self
                .store
                .block()
                .get(txn.txn(), confirmed_frontier)
                .unwrap();
            debug_assert!(
                block.as_block().sideband().unwrap().height == conf_height + num_blocks_cemented
            );
        }

        self.store.confirmation_height().put(
            txn,
            account,
            &ConfirmationHeightInfo::new(confirmation_height, *confirmed_frontier),
        );

        self.cache
            .cemented_count
            .fetch_add(num_blocks_cemented, Ordering::SeqCst);

        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In,
            num_blocks_cemented,
            false,
        );

        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            num_blocks_cemented,
            false,
        );
    }

    pub fn dependent_blocks(&self, txn: &dyn Transaction, block: &dyn Block) -> [BlockHash; 2] {
        let mut visitor = DependentBlockVisitor::new(self, &self.constants, txn);
        block.visit(&mut visitor);
        [visitor.result[0], visitor.result[1]]
    }

    pub fn could_fit(&self, txn: &dyn Transaction, block: &dyn Block) -> bool {
        let dependents = self.dependent_blocks(txn, block);
        dependents
            .iter()
            .all(|dep| dep.is_zero() || self.store.block().exists(txn, dep))
    }

    pub fn dependents_confirmed(&self, txn: &dyn Transaction, block: &dyn Block) -> bool {
        let dependencies = self.dependent_blocks(txn, block);
        dependencies.iter().all(|dep| {
            if !dep.is_zero() {
                self.block_confirmed(txn, dep)
            } else {
                true
            }
        })
    }

    /// Rollback blocks until `block' doesn't exist or it tries to penetrate the confirmation height
    pub fn rollback(
        &self,
        txn: &mut dyn WriteTransaction,
        block: &BlockHash,
        list: &mut Vec<Arc<RwLock<BlockEnum>>>,
    ) -> anyhow::Result<()> {
        debug_assert!(self.store.block().exists(txn.txn(), block));
        let account = self.account(txn.txn(), block).unwrap();
        let block_account_height = self.store.block().account_height(txn.txn(), block);
        let mut rollback = RollbackVisitor::new(txn, self, self.stats.as_ref(), list);
        while self.store.block().exists(rollback.txn.txn(), block) {
            let conf_height = self
                .store
                .confirmation_height()
                .get(rollback.txn.txn(), &account)
                .unwrap_or_default();
            if block_account_height > conf_height.height {
                let account_info = self
                    .store
                    .account()
                    .get(rollback.txn.txn(), &account)
                    .unwrap();
                let block = self
                    .store
                    .block()
                    .get(rollback.txn.txn(), &account_info.head)
                    .unwrap();
                rollback.list.push(Arc::new(RwLock::new(block.clone())));
                block.as_block().visit(&mut rollback);
                if rollback.result.is_err() {
                    return rollback.result;
                }
                self.cache.block_count.fetch_sub(1, Ordering::SeqCst);
            } else {
                bail!("account height was bigger than conf height")
            }
        }

        Ok(())
    }

    pub fn representative_calculated(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockHash {
        let mut visitor = RepresentativeVisitor::new(txn, self.store.as_ref());
        visitor.compute(*hash);
        visitor.result
    }

    pub fn representative(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockHash {
        let result = self.representative_calculated(txn, hash);
        debug_assert!(result.is_zero() || self.store.block().exists(txn, &result));
        result
    }

    pub fn process(
        &self,
        txn: &mut dyn WriteTransaction,
        block: &mut dyn Block,
        verification: SignatureVerification,
    ) -> ProcessReturn {
        debug_assert!(
            !self.constants.work.validate_entry_block(block)
                || self.constants.genesis.read().unwrap().deref()
                    == DEV_GENESIS.read().unwrap().deref()
        );
        let mut processor =
            LedgerProcessor::new(self, &self.stats, &self.constants, txn, verification);
        block.visit_mut(&mut processor);
        if processor.result.code == ProcessResult::Progress {
            self.cache.block_count.fetch_add(1, Ordering::SeqCst);
        }
        processor.result
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        config::TxnTrackingConfig,
        core::{BlockBuilder, KeyPair, OpenBlock, SendBlock, GXRB_RATIO},
        ledger::{
            datastore::lmdb::{EnvOptions, LmdbStore, TestDbFile},
            DEV_GENESIS_KEY,
        },
        stats::StatConfig,
        utils::{seconds_since_epoch, NullLogger},
        work::DEV_WORK_POOL,
        DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
    };

    use super::*;

    struct LedgerContext {
        pub(crate) ledger: Ledger,
        db_file: TestDbFile,
    }

    impl LedgerContext {
        pub fn empty() -> anyhow::Result<Self> {
            let db_file = TestDbFile::random();
            let store = Arc::new(LmdbStore::new(
                &db_file.path,
                &EnvOptions::default(),
                TxnTrackingConfig::default(),
                Duration::from_millis(5000),
                Arc::new(NullLogger::new()),
                false,
            )?);

            let ledger = Ledger::new(
                store.clone(),
                DEV_CONSTANTS.clone(),
                Arc::new(Stat::new(StatConfig::default())),
                &GenerateCache::new(),
            )?;

            let mut txn = store.tx_begin_write()?;
            store.initialize(&mut txn, &ledger.cache, &DEV_CONSTANTS);

            Ok(LedgerContext { ledger, db_file })
        }

        pub fn process_send_from_genesis(
            &self,
            txn: &mut dyn WriteTransaction,
            receiver_account: &Account,
            amount: Amount,
        ) -> anyhow::Result<SendBlock> {
            let orig_genesis_info = self
                .ledger
                .store
                .account()
                .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
                .unwrap();

            let mut send = BlockBuilder::send()
                .previous(orig_genesis_info.head)
                .destination(*receiver_account)
                .balance(orig_genesis_info.balance - amount)
                .sign(DEV_GENESIS_KEY.clone())
                .work(
                    DEV_WORK_POOL
                        .generate_dev2(orig_genesis_info.head.into())
                        .unwrap(),
                )
                .without_sideband()
                .build()?;

            let result = self
                .ledger
                .process(txn, &mut send, SignatureVerification::Unknown);

            assert_eq!(result.code, ProcessResult::Progress);
            Ok(send)
        }

        pub fn process_open(
            &self,
            txn: &mut dyn WriteTransaction,
            send: &SendBlock,
            receiver_key: &KeyPair,
        ) -> anyhow::Result<OpenBlock> {
            let receiver_account = receiver_key.public_key().into();
            let mut open = BlockBuilder::open()
                .source(send.hash())
                .representative(receiver_account)
                .account(receiver_account)
                .sign(receiver_key.clone())
                .work(
                    DEV_WORK_POOL
                        .generate_dev2(receiver_key.public_key().into())
                        .unwrap(),
                )
                .without_sideband()
                .build()?;

            let result = self
                .ledger
                .process(txn, &mut open, SignatureVerification::Unknown);
            assert_eq!(result.code, ProcessResult::Progress);
            Ok(open)
        }
    }

    // Ledger can be initialized and returns a basic query for an empty account
    #[test]
    fn empty_ledger() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let txn = ctx.ledger.store.tx_begin_read()?;
        let balance = ctx
            .ledger
            .account_balance(txn.txn(), &Account::zero(), false);
        assert_eq!(balance, Amount::zero());
        Ok(())
    }

    // Genesis account should have the max balance on empty initialization
    #[test]
    fn genesis_balance() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let txn = ctx.ledger.store.tx_begin_write()?;

        let balance = ctx
            .ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false);
        assert_eq!(balance, DEV_CONSTANTS.genesis_amount);

        let account_info = ctx
            .ledger
            .store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .expect("genesis account not found");
        assert_eq!(ctx.ledger.cache.account_count.load(Ordering::SeqCst), 1);
        // Frontier time should have been updated when genesis balance was added
        assert!(account_info.modified > 0 && account_info.modified <= seconds_since_epoch());
        assert_eq!(account_info.block_count, 1);

        // Genesis block should be confirmed by default
        let conf_info = ctx
            .ledger
            .store
            .confirmation_height()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .expect("conf height not found");
        assert_eq!(conf_info.height, 1);
        assert_eq!(conf_info.frontier, *DEV_GENESIS_HASH);

        let block = ctx
            .ledger
            .store
            .block()
            .get(txn.txn(), &account_info.head)
            .expect("genesis block not found");
        assert_eq!(block.block_type(), BlockType::Open);

        assert_eq!(
            ctx.ledger
                .store
                .frontier()
                .get(txn.txn(), &account_info.head),
            *DEV_GENESIS_ACCOUNT,
        );
        Ok(())
    }

    #[test]
    fn cemented_count_cache() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        assert_eq!(ctx.ledger.cache.cemented_count.load(Ordering::SeqCst), 1);
        Ok(())
    }

    #[test]
    fn process_modifies_sideband() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let pool = &DEV_WORK_POOL;
        let mut send = BlockBuilder::state()
            .account(*DEV_GENESIS_ACCOUNT)
            .previous(*DEV_GENESIS_HASH)
            .representative(*DEV_GENESIS_ACCOUNT)
            .balance(DEV_CONSTANTS.genesis_amount - Amount::new(*GXRB_RATIO))
            .link(DEV_GENESIS_ACCOUNT.deref())
            .sign(&DEV_GENESIS_KEY)
            .work(pool.generate_dev2(DEV_GENESIS_HASH.deref().into()).unwrap())
            .build()?;

        let mut txn = ctx.ledger.store.tx_begin_write()?;
        let result = ctx
            .ledger
            .process(txn.as_mut(), &mut send, SignatureVerification::Unknown);

        assert_eq!(result.code, ProcessResult::Progress);
        assert_eq!(
            send.sideband().unwrap().timestamp,
            ctx.ledger
                .store
                .block()
                .get(txn.txn(), &send.hash())
                .unwrap()
                .as_block()
                .sideband()
                .unwrap()
                .timestamp
        );

        Ok(())
    }

    #[test]
    fn process_send() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let ledger = &ctx.ledger;
        let store = &ledger.store;
        let mut txn = store.tx_begin_write()?;

        let orig_genesis_info = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();

        let receiver_key = KeyPair::new();
        let receiver_account = Account::from(receiver_key.public_key());
        let new_genesis_balance = Amount::new(50);
        let amount_sent = orig_genesis_info.balance - new_genesis_balance;
        let send = ctx.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent)?;

        // Check sideband
        let send_sideband = send.sideband().unwrap();
        assert_eq!(send_sideband.account, *DEV_GENESIS_ACCOUNT);
        assert_eq!(send_sideband.height, 2);
        assert_eq!(ledger.amount(txn.txn(), &send.hash()), Some(amount_sent));

        // Check block was saved
        let loaded_send = store.block().get(txn.txn(), &send.hash()).unwrap();
        let BlockEnum::Send(loaded_send) = loaded_send else {panic!("not a send block")};
        assert_eq!(loaded_send, send);

        // Check stores were updated
        assert_eq!(
            store.frontier().get(txn.txn(), &orig_genesis_info.head),
            Account::zero()
        );
        assert_eq!(
            store.frontier().get(txn.txn(), &send.hash()),
            *DEV_GENESIS_ACCOUNT
        );
        assert_eq!(
            store.block().account_calculated(&send),
            *DEV_GENESIS_ACCOUNT
        );
        assert_eq!(
            ledger.account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
            new_genesis_balance
        );
        assert_eq!(
            ledger.account_receivable(txn.txn(), &receiver_account, false),
            amount_sent
        );

        let new_genesis_info = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();
        assert_eq!(new_genesis_info.block_count, 2);
        assert_eq!(new_genesis_info.head, send.hash());
        Ok(())
    }

    #[test]
    fn process_open() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let ledger = &ctx.ledger;
        let store = &ledger.store;
        let mut txn = store.tx_begin_write()?;

        let genesis_account_info = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();

        let receiver_key = KeyPair::new();
        let receiver_account = Account::from(receiver_key.public_key());
        let new_genesis_balance = Amount::new(50);
        let amount_sent = genesis_account_info.balance - new_genesis_balance;
        let send = ctx.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent)?;

        // Create an open block opening an account accepting the send we just created
        let open = ctx.process_open(txn.as_mut(), &send, &receiver_key)?;

        // Check sideband
        let open_sideband = open.sideband().unwrap();
        assert_eq!(open_sideband.account, receiver_account);
        assert_eq!(open_sideband.balance, amount_sent);
        assert_eq!(open_sideband.height, 1);

        // Check block was saved
        let loaded_open = store.block().get(txn.txn(), &open.hash()).unwrap();
        let BlockEnum::Open(loaded_open) = loaded_open else{panic!("not an open block")};
        assert_eq!(loaded_open, open);

        //Check stores were updated
        assert_eq!(ledger.amount(txn.txn(), &open.hash()), Some(amount_sent));
        assert_eq!(store.block().account_calculated(&open), receiver_account);
        assert_eq!(
            store.frontier().get(txn.txn(), &open.hash()),
            receiver_account
        );
        assert_eq!(
            ledger.account_balance(txn.txn(), &receiver_account, false),
            amount_sent
        );
        assert_eq!(
            ledger.account_receivable(txn.txn(), &receiver_account, false),
            Amount::zero()
        );
        assert_eq!(ledger.weight(&DEV_GENESIS_ACCOUNT), new_genesis_balance);
        assert_eq!(ledger.weight(&receiver_account), amount_sent);

        let new_genesis_info = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();
        assert_eq!(new_genesis_info.head, send.hash());

        let receiver_info = store.account().get(txn.txn(), &receiver_account).unwrap();
        assert_eq!(receiver_info.head, open.hash());
        Ok(())
    }

    #[test]
    fn rollback_open() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let ledger = &ctx.ledger;
        let store = &ledger.store;
        let mut txn = store.tx_begin_write()?;

        let receiver_key = KeyPair::new();
        let receiver_account = Account::from(receiver_key.public_key());
        let new_genesis_balance = Amount::new(50);
        let amount_sent = DEV_CONSTANTS.genesis_amount - new_genesis_balance;
        let send = ctx.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent)?;
        let open = ctx.process_open(txn.as_mut(), &send, &receiver_key)?;

        // --------------------------------
        // Rollback Open Block
        ledger.rollback(txn.as_mut(), &open.hash(), &mut Vec::new())?;

        assert_eq!(
            store.frontier().get(txn.txn(), &open.hash()),
            Account::zero()
        );
        let info5 = ledger
            .store
            .account()
            .get(txn.txn(), &receiver_key.public_key().into());
        assert_eq!(info5, None);

        let pending1 = ledger
            .store
            .pending()
            .get(
                txn.txn(),
                &PendingKey::new(receiver_key.public_key().into(), send.hash()),
            )
            .unwrap();

        assert_eq!(pending1.source, *DEV_GENESIS_ACCOUNT);
        assert_eq!(
            pending1.amount,
            DEV_CONSTANTS.genesis_amount - Amount::new(50)
        );
        assert_eq!(
            ledger.account_balance(txn.txn(), &receiver_key.public_key().into(), false),
            Amount::zero()
        );
        assert_eq!(
            ledger.account_receivable(txn.txn(), &receiver_key.public_key().into(), false),
            DEV_CONSTANTS.genesis_amount - Amount::new(50)
        );
        assert_eq!(
            ledger.account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
            Amount::new(50)
        );
        assert_eq!(ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::new(50));
        assert_eq!(
            ledger.weight(&receiver_key.public_key().into()),
            Amount::zero()
        );

        let info6 = ledger
            .store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();
        assert_eq!(info6.head, send.hash());
        Ok(())
    }

    #[test]
    fn rollback_send_block() -> anyhow::Result<()> {
        let ctx = LedgerContext::empty()?;
        let ledger = &ctx.ledger;
        let store = &ledger.store;
        let mut txn = store.tx_begin_write()?;

        let genesis_account_info = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();

        let receiver_key = KeyPair::new();
        let receiver_account = Account::from(receiver_key.public_key());
        let new_genesis_balance = Amount::new(50);
        let amount_sent = DEV_CONSTANTS.genesis_amount - new_genesis_balance;
        let send = ctx.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent)?;

        // --------------------------------
        // Rollback Send Block
        ledger.rollback(txn.as_mut(), &send.hash(), &mut Vec::new())?;

        assert_eq!(
            ledger.weight(&DEV_GENESIS_ACCOUNT),
            DEV_CONSTANTS.genesis_amount
        );
        assert_eq!(
            store.frontier().get(txn.txn(), &genesis_account_info.head),
            *DEV_GENESIS_ACCOUNT
        );
        assert_eq!(
            store.frontier().get(txn.txn(), &send.hash()),
            Account::zero()
        );
        let info7 = store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();
        assert_eq!(info7.block_count, 1);
        assert_eq!(info7.head, genesis_account_info.head);
        let pending2 = store.pending().get(
            txn.txn(),
            &PendingKey::new(receiver_key.public_key().into(), send.hash()),
        );
        assert_eq!(pending2, None);
        assert_eq!(
            ledger.account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
            DEV_CONSTANTS.genesis_amount
        );
        assert_eq!(
            ledger.account_receivable(txn.txn(), &receiver_key.public_key().into(), false),
            Amount::zero()
        );
        assert_eq!(
            ledger.cache.account_count.load(Ordering::Relaxed),
            store.account().count(txn.txn())
        );
        Ok(())
    }
}
