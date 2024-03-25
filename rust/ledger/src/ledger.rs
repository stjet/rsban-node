use super::DependentBlocksFinder;
use crate::{
    block_insertion::{BlockInserter, BlockValidatorFactory},
    BlockRollbackPerformer, GenerateCacheFlags, LedgerCache, LedgerConstants, RepWeights,
    RepresentativeBlockFinder,
};
use rand::{thread_rng, Rng};
use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, BlockChainSection, BlockEnum,
    BlockHash, BlockSubType, ConfirmationHeightInfo, Epoch, Link, PendingInfo, PendingKey,
    QualifiedRoot, Root,
};
use rsnano_store_lmdb::{
    ConfiguredAccountDatabaseBuilder, ConfiguredBlockDatabaseBuilder,
    ConfiguredFrontierDatabaseBuilder, ConfiguredPendingDatabaseBuilder,
    ConfiguredPrunedDatabaseBuilder, Environment, EnvironmentStub, EnvironmentWrapper,
    LmdbAccountStore, LmdbBlockStore, LmdbConfirmationHeightStore, LmdbEnv, LmdbFinalVoteStore,
    LmdbFrontierStore, LmdbOnlineWeightStore, LmdbPeerStore, LmdbPendingStore, LmdbPrunedStore,
    LmdbReadTransaction, LmdbRepWeightStore, LmdbStore, LmdbVersionStore, LmdbWriteTransaction,
    Transaction,
};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

#[derive(Debug, PartialEq, Eq)]
pub struct UncementedInfo {
    pub cemented_frontier: BlockHash,
    pub frontier: BlockHash,
    pub account: Account,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy, FromPrimitive)]
#[repr(u8)]
pub enum BlockStatus {
    Progress,      // Hasn't been seen before, signed correctly
    BadSignature,  // Signature was bad, forged or transmission error
    Old,           // Already seen and was valid
    NegativeSpend, // Malicious attempt to spend a negative amount
    Fork,          // Malicious fork based on previous
    /// Source block doesn't exist, has already been received, or requires an account upgrade (epoch blocks)
    Unreceivable,
    GapPrevious,         // Block marked as previous is unknown
    GapSource,           // Block marked as source is unknown
    GapEpochOpenPending, // Block marked as pending blocks required for epoch open block are unknown
    OpenedBurnAccount,   // Block attempts to open the burn account
    /// Balance and amount delta don't match
    BalanceMismatch,
    RepresentativeMismatch, // Representative is changed when it is not allowed
    BlockPosition,          // This block cannot follow the previous block
    InsufficientWork, // Insufficient work for this block, even though it passed the minimal validation
}

pub trait LedgerObserver: Send + Sync {
    fn blocks_cemented(&self, _cemented_count: u64) {}
    fn block_rolled_back(&self, _block_type: BlockSubType) {}
    fn block_rolled_back2(&self, _block: &BlockEnum, _is_epoch: bool) {}
    fn block_added(&self, _block: &BlockEnum, _is_epoch: bool) {}
}

pub struct NullLedgerObserver {}

impl NullLedgerObserver {
    pub fn new() -> Self {
        Self {}
    }
}

impl LedgerObserver for NullLedgerObserver {}

pub struct Ledger<T: Environment + 'static = EnvironmentWrapper> {
    pub store: Arc<LmdbStore<T>>,
    pub cache: Arc<LedgerCache>,
    pub constants: LedgerConstants,
    pub observer: Arc<dyn LedgerObserver>,
    pruning: AtomicBool,
    bootstrap_weight_max_blocks: AtomicU64,
    pub check_bootstrap_weights: AtomicBool,
    pub bootstrap_weights: Mutex<HashMap<Account, Amount>>,
}

impl Ledger<EnvironmentStub> {
    pub fn create_null() -> Self {
        Self::new(
            Arc::new(LmdbStore::create_null()),
            LedgerConstants::unit_test(),
        )
        .unwrap()
    }

    pub fn create_null_with() -> NullLedgerBuilder {
        NullLedgerBuilder::new()
    }
}

pub struct NullLedgerBuilder {
    blocks: ConfiguredBlockDatabaseBuilder,
    frontiers: ConfiguredFrontierDatabaseBuilder,
    accounts: ConfiguredAccountDatabaseBuilder,
    pending: ConfiguredPendingDatabaseBuilder,
    pruned: ConfiguredPrunedDatabaseBuilder,
}

impl NullLedgerBuilder {
    fn new() -> Self {
        Self {
            blocks: ConfiguredBlockDatabaseBuilder::new(),
            frontiers: ConfiguredFrontierDatabaseBuilder::new(),
            accounts: ConfiguredAccountDatabaseBuilder::new(),
            pending: ConfiguredPendingDatabaseBuilder::new(),
            pruned: ConfiguredPrunedDatabaseBuilder::new(),
        }
    }

    pub fn block(mut self, block: &BlockEnum) -> Self {
        self.blocks = self.blocks.block(block);
        self
    }

    pub fn blocks<'a>(mut self, blocks: impl IntoIterator<Item = &'a BlockEnum>) -> Self {
        for b in blocks.into_iter() {
            self.blocks = self.blocks.block(b);
        }
        self
    }

    pub fn frontier(mut self, hash: &BlockHash, account: &Account) -> Self {
        self.frontiers = self.frontiers.frontier(hash, account);
        self
    }

    pub fn account_info(mut self, account: &Account, info: &AccountInfo) -> Self {
        self.accounts = self.accounts.account(account, info);
        self
    }

    pub fn pending(mut self, key: &PendingKey, info: &PendingInfo) -> Self {
        self.pending = self.pending.pending(key, info);
        self
    }

    pub fn pruned(mut self, hash: &BlockHash) -> Self {
        self.pruned = self.pruned.pruned(hash);
        self
    }

    pub fn build(self) -> Ledger<EnvironmentStub> {
        let env = Arc::new(
            LmdbEnv::create_null_with()
                .configured_database(self.blocks.build())
                .configured_database(self.frontiers.build())
                .configured_database(self.accounts.build())
                .configured_database(self.pending.build())
                .configured_database(self.pruned.build())
                .build(),
        );

        let store = LmdbStore {
            env: env.clone(),
            account: Arc::new(LmdbAccountStore::new(env.clone()).unwrap()),
            block: Arc::new(LmdbBlockStore::new(env.clone()).unwrap()),
            confirmation_height: Arc::new(LmdbConfirmationHeightStore::new(env.clone()).unwrap()),
            final_vote: Arc::new(LmdbFinalVoteStore::new(env.clone()).unwrap()),
            frontier: Arc::new(LmdbFrontierStore::new(env.clone()).unwrap()),
            online_weight: Arc::new(LmdbOnlineWeightStore::new(env.clone()).unwrap()),
            peer: Arc::new(LmdbPeerStore::new(env.clone()).unwrap()),
            pending: Arc::new(LmdbPendingStore::new(env.clone()).unwrap()),
            pruned: Arc::new(LmdbPrunedStore::new(env.clone()).unwrap()),
            rep_weight: Arc::new(LmdbRepWeightStore::new(env.clone()).unwrap()),
            version: Arc::new(LmdbVersionStore::new(env.clone()).unwrap()),
        };
        Ledger::new(Arc::new(store), LedgerConstants::unit_test()).unwrap()
    }
}

impl<T: Environment + 'static> Ledger<T> {
    pub fn new(store: Arc<LmdbStore<T>>, constants: LedgerConstants) -> anyhow::Result<Self> {
        Self::with_cache(store, constants, &GenerateCacheFlags::new())
    }

    pub fn with_cache(
        store: Arc<LmdbStore<T>>,
        constants: LedgerConstants,
        generate_cache: &GenerateCacheFlags,
    ) -> anyhow::Result<Self> {
        let mut ledger = Self {
            store,
            cache: Arc::new(LedgerCache::new()),
            constants,
            observer: Arc::new(NullLedgerObserver::new()),
            pruning: AtomicBool::new(false),
            bootstrap_weight_max_blocks: AtomicU64::new(1),
            check_bootstrap_weights: AtomicBool::new(true),
            bootstrap_weights: Mutex::new(HashMap::new()),
        };

        ledger.initialize(generate_cache)?;

        Ok(ledger)
    }

    pub fn set_observer(&mut self, observer: Arc<dyn LedgerObserver>) {
        self.observer = observer;
    }

    pub fn read_txn(&self) -> LmdbReadTransaction<T> {
        self.store.tx_begin_read()
    }

    pub fn rw_txn(&self) -> LmdbWriteTransaction<T> {
        self.store.tx_begin_write()
    }

    fn initialize(&mut self, generate_cache: &GenerateCacheFlags) -> anyhow::Result<()> {
        if self.store.account.begin(&self.read_txn()).is_end() {
            self.add_genesis_block(&mut self.rw_txn());
        }

        if generate_cache.reps || generate_cache.account_count || generate_cache.block_count {
            self.store.account.for_each_par(&|_txn, mut i, n| {
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
                .confirmation_height
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

        let transaction = self.store.tx_begin_read();
        self.cache
            .pruned_count
            .fetch_add(self.store.pruned.count(&transaction), Ordering::SeqCst);

        Ok(())
    }

    fn add_genesis_block(&self, txn: &mut LmdbWriteTransaction<T>) {
        let genesis_block = self.constants.genesis.deref();
        let genesis_hash = genesis_block.hash();
        let genesis_account = self.constants.genesis_account;
        self.store.block.put(txn, genesis_block);

        self.store.confirmation_height.put(
            txn,
            &genesis_account,
            &ConfirmationHeightInfo::new(1, genesis_hash),
        );

        self.store.account.put(
            txn,
            &genesis_account,
            &AccountInfo {
                head: genesis_hash,
                representative: genesis_account,
                open_block: genesis_hash,
                balance: u128::MAX.into(),
                modified: seconds_since_epoch(),
                block_count: 1,
                epoch: Epoch::Epoch0,
            },
        );
        self.store
            .frontier
            .put(txn, &genesis_hash, &genesis_account);
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
        let txn = self.store.tx_begin_read();
        self.block_or_pruned_exists_txn(&txn, block)
    }

    pub fn block_exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        block: &BlockHash,
    ) -> bool {
        self.store.block.exists(txn, block)
    }

    pub fn block_or_pruned_exists_txn(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> bool {
        self.store.pruned.exists(txn, hash) || self.block_exists(txn, hash)
    }

    /// Balance for account containing the given block at the time of the block.
    pub fn balance(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Amount> {
        if hash.is_zero() {
            None
        } else {
            self.get_block(txn, hash).map(|block| block.balance())
        }
    }

    /// Balance for account by account number
    pub fn account_balance(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
        only_confirmed: bool,
    ) -> Amount {
        if only_confirmed {
            match self.store.confirmation_height.get(txn, account) {
                Some(info) => self.balance(txn, &info.frontier).unwrap_or_default(),
                None => Amount::zero(),
            }
        } else {
            match self.account_info(txn, account) {
                Some(info) => info.balance,
                None => Amount::zero(),
            }
        }
    }

    pub fn account_receivable(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
        only_confirmed: bool,
    ) -> Amount {
        let mut result = Amount::zero();

        for (key, info) in self.account_receivable_upper_bound(txn, *account, BlockHash::zero()) {
            if !only_confirmed || self.block_confirmed(txn, &key.hash) {
                result += info.amount;
            }
        }

        result
    }

    pub fn block_confirmed(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> bool {
        if self.store.pruned.exists(txn, hash) {
            return true;
        }

        match self.get_block(txn, hash) {
            Some(block) => {
                let mut account = block.account();
                let sideband = &block.sideband().unwrap();
                if account.is_zero() {
                    account = sideband.account;
                }
                match self.store.confirmation_height.get(txn, &account) {
                    Some(info) => info.height >= sideband.height,
                    None => false,
                }
            }
            None => false,
        }
    }

    pub fn block_text(&self, hash: &BlockHash) -> anyhow::Result<String> {
        let txn = self.store.tx_begin_read();
        match self.get_block(&txn, hash) {
            Some(block) => block.to_json(),
            None => Ok(String::new()),
        }
    }

    pub fn hash_root_random(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Option<(BlockHash, BlockHash)> {
        if !self.pruning_enabled() {
            self.store
                .block
                .random(txn)
                .map(|block| (block.hash(), block.root().into()))
        } else {
            let mut hash = BlockHash::zero();
            let count = self.cache.block_count.load(Ordering::SeqCst);
            let region = thread_rng().gen_range(0..count);
            // Pruned cache cannot guarantee that pruned blocks are already commited
            if region < self.cache.pruned_count.load(Ordering::SeqCst) {
                hash = self.store.pruned.random(txn).unwrap_or_default();
            }
            if hash.is_zero() {
                self.store
                    .block
                    .random(txn)
                    .map(|block| (block.hash(), block.root().into()))
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
    pub fn account(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Account> {
        self.get_block(txn, hash).map(|block| block.account())
    }

    /// Return absolute amount decrease or increase for block
    pub fn amount(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Amount> {
        let block = self.get_block(txn, hash)?;
        let block_balance = self.balance(txn, hash)?;
        let previous_balance = self.balance(txn, &block.previous())?;
        if block_balance > previous_balance {
            Some(block_balance - previous_balance)
        } else {
            Some(previous_balance - block_balance)
        }
    }

    /// Return latest block for account
    pub fn latest(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Option<BlockHash> {
        self.account_info(txn, account).map(|info| info.head)
    }

    /// Return latest root for account, account number if there are no blocks for this account
    pub fn latest_root(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Root {
        match self.account_info(txn, account) {
            Some(info) => info.head.into(),
            None => account.into(),
        }
    }

    pub fn version(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Epoch {
        self.get_block(txn, hash)
            .map(|block| block.epoch())
            .unwrap_or(Epoch::Epoch0)
    }

    pub fn account_height(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> u64 {
        self.get_block(txn, hash)
            .map(|block| block.sideband().unwrap().height)
            .unwrap_or_default()
    }

    pub fn is_epoch_link(&self, link: &Link) -> bool {
        self.constants.epochs.is_epoch_link(link)
    }

    /// Given the block hash of a send block, find the associated receive block that receives that send.
    /// The send block hash is not checked in any way, it is assumed to be correct.
    /// Return the receive block on success and None on failure
    pub fn find_receive_block_by_send_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        destination: &Account,
        send_block_hash: &BlockHash,
    ) -> Option<BlockEnum> {
        // get the cemented frontier
        let info = self.store.confirmation_height.get(txn, destination)?;
        let mut possible_receive_block = self.get_block(txn, &info.frontier);

        // walk down the chain until the source field of a receive block matches the send block hash
        while let Some(current) = possible_receive_block {
            if current.is_receive() && Some(*send_block_hash) == current.source() {
                // we have a match
                return Some(current);
            }

            possible_receive_block = self.get_block(txn, &current.previous());
        }

        None
    }

    pub fn epoch_link(&self, epoch: Epoch) -> Option<Link> {
        self.constants.epochs.link(epoch).cloned()
    }

    pub fn update_account(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
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
                self.store.account.del(txn, account);
            }
            self.store.account.put(txn, account, new_info);
        } else {
            debug_assert!(!self.store.confirmation_height.exists(txn, account));
            self.store.account.del(txn, account);
            debug_assert!(self.cache.account_count.load(Ordering::SeqCst) > 0);
            self.cache.account_count.fetch_sub(1, Ordering::SeqCst);
        }
    }

    pub fn successor(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<BlockHash> {
        self.store.block.successor(txn, hash)
    }

    pub fn successor_by_root(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        root: &QualifiedRoot,
    ) -> Option<BlockHash> {
        if !root.previous.is_zero() {
            self.store.block.successor(txn, &root.previous)
        } else {
            self.account_info(txn, &root.root.into())
                .map(|info| info.open_block)
        }
    }

    pub fn pruning_action(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        hash: &BlockHash,
        batch_size: u64,
    ) -> u64 {
        let mut pruned_count = 0;
        let mut hash = *hash;
        let genesis_hash = self.constants.genesis.hash();

        while !hash.is_zero() && hash != genesis_hash {
            if let Some(block) = self.get_block(txn, &hash) {
                self.store.block.del(txn, &hash);
                self.store.pruned.put(txn, &hash);
                hash = block.previous();
                pruned_count += 1;
                self.cache.pruned_count.fetch_add(1, Ordering::SeqCst);
                if pruned_count % batch_size == 0 {
                    txn.commit();
                    txn.renew();
                }
            } else if self.store.pruned.exists(txn, &hash) {
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
        self.store.account.for_each_par(&|txn, mut i, n| {
            let mut unconfirmed_frontiers = Vec::new();
            while !i.eq(n.as_ref()) {
                if let Some((&account, account_info)) = i.current() {
                    if let Some(conf_height_info) =
                        self.store.confirmation_height.get(txn, &account)
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
        txn: &mut LmdbWriteTransaction<T>,
        section: &BlockChainSection,
    ) {
        #[cfg(debug_assertions)]
        {
            let conf_height = self
                .store
                .confirmation_height
                .get(txn, &section.account)
                .map(|i| i.height)
                .unwrap_or_default();
            let block = self.get_block(txn, &section.top_hash).unwrap();
            debug_assert_eq!(
                block.sideband().unwrap().height,
                conf_height + section.block_count()
            );
        }

        self.store.confirmation_height.put(
            txn,
            &section.account,
            &ConfirmationHeightInfo::new(section.top_height, section.top_hash),
        );

        self.cache
            .cemented_count
            .fetch_add(section.block_count(), Ordering::SeqCst);

        self.observer.blocks_cemented(section.block_count());
    }

    pub fn dependent_blocks(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        block: &BlockEnum,
    ) -> (BlockHash, BlockHash) {
        DependentBlocksFinder::new(self, txn).find_dependent_blocks(block)
    }

    pub fn dependents_confirmed(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        block: &BlockEnum,
    ) -> bool {
        let (first, second) = self.dependent_blocks(txn, block);
        self.is_dependency_confirmed(txn, &first) && self.is_dependency_confirmed(txn, &second)
    }

    fn is_dependency_confirmed(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        dependency: &BlockHash,
    ) -> bool {
        if !dependency.is_zero() {
            self.block_confirmed(txn, dependency)
        } else {
            true
        }
    }

    /// Rollback blocks until `block' doesn't exist or it tries to penetrate the confirmation height
    pub fn rollback(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        block: &BlockHash,
    ) -> anyhow::Result<Vec<BlockEnum>> {
        BlockRollbackPerformer::new(self, txn).roll_back(block)
    }

    /// Returns the latest block with representative information
    pub fn representative_block_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> BlockHash {
        let hash = RepresentativeBlockFinder::new(txn, self.store.as_ref()).find_rep_block(*hash);
        debug_assert!(hash.is_zero() || self.store.block.exists(txn, &hash));
        hash
    }

    pub fn process(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        block: &mut BlockEnum,
    ) -> Result<(), BlockStatus> {
        let validator = BlockValidatorFactory::new(self, txn, block).create_validator();
        let instructions = validator.validate()?;
        BlockInserter::new(self, txn, block, &instructions).insert();
        Ok(())
    }

    pub fn get_block(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<BlockEnum> {
        self.store.block.get(txn, hash)
    }

    pub fn account_info(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Option<AccountInfo> {
        self.store.account.get(transaction, account)
    }

    pub fn get_confirmation_height(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> Option<ConfirmationHeightInfo> {
        self.store.confirmation_height.get(txn, account)
    }

    pub fn get_frontier(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Account> {
        self.store.frontier.get(txn, hash)
    }

    pub fn pending_info(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,

        key: &PendingKey,
    ) -> Option<PendingInfo> {
        self.store.pending.get(txn, key)
    }

    /// Returns the next receivable entry for the account 'account' with hash greater than 'hash'
    pub fn account_receivable_upper_bound<'a>(
        &'a self,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: Account,
        hash: BlockHash,
    ) -> ReceivableIterator<'a, T> {
        ReceivableIterator::<'a, T> {
            txn,
            pending: self.store.pending.deref(),
            requested_account: account,
            actual_account: Some(account),
            next_hash: hash.inc(),
        }
    }

    /// Returns the next receivable entry for an account greater than 'account'
    pub fn receivable_upper_bound<'a>(
        &'a self,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: Account,
    ) -> ReceivableIterator<'a, T> {
        ReceivableIterator::<'a, T> {
            txn,
            pending: self.store.pending.deref(),
            requested_account: account.inc().unwrap_or_default(),
            actual_account: None,
            next_hash: Some(BlockHash::zero()),
        }
    }

    /// Returns whether there are any receivable entries for 'account'
    pub fn receivable_any(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: Account,
    ) -> bool {
        self.account_receivable_upper_bound(txn, account, BlockHash::zero())
            .next()
            .is_some()
    }
}

pub struct ReceivableIterator<'a, T: Environment + 'static> {
    txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    pending: &'a LmdbPendingStore<T>,
    requested_account: Account,
    actual_account: Option<Account>,
    next_hash: Option<BlockHash>,
}

impl<'a, T: Environment + 'static> Iterator for ReceivableIterator<'a, T> {
    type Item = (PendingKey, PendingInfo);

    fn next(&mut self) -> Option<Self::Item> {
        let hash = self.next_hash?;
        let it = self.pending.begin_at_key(
            self.txn,
            &PendingKey::new(self.actual_account.unwrap_or(self.requested_account), hash),
        );

        let (key, info) = it.current()?;
        match self.actual_account {
            Some(account) => {
                if key.account == account {
                    self.next_hash = key.hash.inc();
                    Some((key.clone(), info.clone()))
                } else {
                    None
                }
            }
            None => {
                self.actual_account = Some(key.account);
                self.next_hash = key.hash.inc();
                Some((key.clone(), info.clone()))
            }
        }
    }
}
