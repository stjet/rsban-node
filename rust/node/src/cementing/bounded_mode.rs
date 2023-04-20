use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    Account, BlockEnum, BlockHash, ConfirmationHeightInfo, ConfirmationHeightUpdate,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::{ReadTransaction, Transaction, WriteTransaction};

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{
    accounts_confirmed_map::{AccountsConfirmedMap, AccountsConfirmedMapContainerInfo},
    multi_account_cementer::CementationDataRequester,
    BatchWriteSizeManager, CementCallbackRefs, MultiAccountCementer, WriteDetails,
    WriteDetailsContainerInfo,
};

/** The maximum number of various containers to keep the memory bounded */
const MAX_ITEMS: usize = 131072;

/** The maximum number of blocks to be read in while iterating over a long account chain */
const BATCH_READ_SIZE: usize = 65536;

pub(crate) struct ConfirmedInfo {
    pub(crate) confirmed_height: u64,
    pub(crate) iterated_frontier: BlockHash,
}

pub(super) struct BoundedMode {
    stopped: Arc<AtomicBool>,
    batch_separate_pending_min_time: Duration,
    cementer: MultiAccountCementer,

    processing_timer: Instant,
    stats: Arc<Stats>,
    cemented_batch_timer: Instant,
    write_database_queue: Arc<WriteDatabaseQueue>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    ledger: Arc<Ledger>,
    helper: BoundedModeHelper,
}

impl BoundedMode {
    pub fn new(
        ledger: Arc<Ledger>,
        write_database_queue: Arc<WriteDatabaseQueue>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        batch_separate_pending_min_time: Duration,
        stopped: Arc<AtomicBool>,
        stats: Arc<Stats>,
    ) -> Self {
        let helper = BoundedModeHelper::new(ledger.clone(), logger.clone(), stopped.clone());

        Self {
            write_database_queue,
            logger,
            enable_timing_logging,
            ledger,
            stopped,
            stats,
            processing_timer: Instant::now(),
            batch_separate_pending_min_time,
            cemented_batch_timer: Instant::now(),
            cementer: MultiAccountCementer::new(),
            helper,
        }
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.cementer.batch_write_size
    }

    pub(crate) fn process2(
        &mut self,
        original_block: &BlockEnum,
        callbacks: &mut CementCallbackRefs,
    ) {
        if !self.has_pending_writes() {
            self.clear_process_vars();
            self.processing_timer = Instant::now();
        }

        self.helper.initialize(original_block.hash());

        let mut txn = self.ledger.store.tx_begin_read();

        loop {
            let next_step = self.helper.get_next_step(txn.as_mut());

            // This block was added to the confirmation height processor but is already confirmed
            if next_step.already_cemented {
                (callbacks.block_already_cemented)(original_block.hash());
            }

            for write in next_step.write_details.into_iter().flatten() {
                self.cementer.enqueue(write);
            }

            if self.should_flush(&self.helper, callbacks) {
                self.try_flush(callbacks);
            }

            if next_step.is_done || self.stopped.load(Ordering::SeqCst) {
                break;
            }
            txn.refresh();
        }
    }

    pub(crate) fn process(
        &mut self,
        original_block: &BlockEnum,
        callbacks: &mut CementCallbackRefs,
    ) {
        if !self.has_pending_writes() {
            self.clear_process_vars();
            self.processing_timer = Instant::now();
        }

        self.helper.initialize(original_block.hash());

        let mut txn = self.ledger.store.tx_begin_read();

        loop {
            if !self.helper.load_next_block(txn.txn()) {
                continue;
            }

            // This block was added to the confirmation height processor but is already confirmed
            if self.helper.should_notify_already_cemented() {
                (callbacks.block_already_cemented)(original_block.hash());
                break;
            }

            self.helper.goto_least_unconfirmed_hash(txn.txn());
            self.helper.top_most_non_receive_block_hash = self.helper.current_hash;

            let hit_receive = if !self.helper.is_already_cemented() {
                self.helper.iterate(txn.as_mut())
            } else {
                false
            };

            // Exit early when the processor has been stopped, otherwise this function may take a
            // while (and hence keep the process running) if updating a long chain.
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }

            // next_in_receive_chain can be modified when writing, so need to cache it here before resetting
            let is_set = self.helper.next_in_receive_chain.is_some();
            self.helper.next_in_receive_chain = None;

            // Need to also handle the case where we are hitting receives where the sends below should be confirmed
            if !hit_receive
                || (self.helper.receive_source_pairs.len() == 1
                    && self.helper.top_most_non_receive_block_hash != self.helper.current_hash)
            {
                for write in self
                    .helper
                    .write_next(txn.txn(), is_set)
                    .into_iter()
                    .flatten()
                {
                    self.cementer.enqueue(write);
                }

                if self.should_flush(&self.helper, callbacks) {
                    self.try_flush(callbacks);
                }
            }

            self.helper.first_iter = false;
            txn.refresh();

            if self.helper.is_done() || self.stopped.load(Ordering::SeqCst) {
                break;
            }
        }

        debug_assert!(self.helper.checkpoints.is_empty());
    }

    fn should_flush(&self, helper: &BoundedModeHelper, callbacks: &mut CementCallbackRefs) -> bool {
        let is_batch_full = self.cementer.max_batch_write_size_reached();

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let awaiting_processing = (callbacks.awaiting_processing_count)();
        let is_done_processing = helper.finished_iterating()
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded());

        let should_flush = is_done_processing || is_batch_full || self.is_write_queue_full();
        should_flush && !self.cementer.has_pending_writes()
    }

    fn is_write_queue_full(&self) -> bool {
        self.cementer.max_pending_writes_reached()
            || self.helper.accounts_confirmed_info.len() >= MAX_ITEMS
    }

    fn try_flush(&mut self, callbacks: &mut CementCallbackRefs) {
        // If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
        if self
            .write_database_queue
            .process(Writer::ConfirmationHeight)
        {
            // todo: this does not seem thread safe!
            let mut scoped_write_guard = self.write_database_queue.pop();
            self.write_pending_blocks_with_write_guard(&mut scoped_write_guard, callbacks);
        } else if self.is_write_queue_full() {
            // Block and wait until we have DB access. We must flush because the queue is full.
            let mut scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            self.write_pending_blocks_with_write_guard(&mut scoped_write_guard, callbacks);
        }
    }

    fn is_min_processing_time_exceeded(&self) -> bool {
        self.processing_timer.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if !self.cementer.has_pending_writes() {
            return;
        }

        let mut write_guard = self
            .write_database_queue
            .wait(rsnano_ledger::Writer::ConfirmationHeight);

        self.write_pending_blocks_with_write_guard(&mut write_guard, callbacks);
    }

    fn write_pending_blocks_with_write_guard(
        &mut self,
        scoped_write_guard: &mut WriteGuard,
        callbacks: &mut CementCallbackRefs,
    ) {
        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();

        self.start_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while let Some((update_command, account_done)) = self
            .cementer
            .cement_next(&CementationLedgerAdapter {
                ledger: &self.ledger,
                txn: txn.txn(),
            })
            .unwrap()
        {
            self.flush(txn.as_mut(), &update_command, scoped_write_guard, callbacks);
            if account_done {
                if let Some(found_info) = self
                    .helper
                    .accounts_confirmed_info
                    .get(&update_command.account)
                {
                    if found_info.confirmed_height == update_command.new_height {
                        self.helper
                            .accounts_confirmed_info
                            .remove(&update_command.account);
                    }
                }
            }
        }
        drop(txn);

        let unpublished_count = self.cementer.unpublished_cemented_blocks();
        self.stop_batch_timer(unpublished_count);

        if unpublished_count > 0 {
            scoped_write_guard.release();
            self.cementer
                .publish_cemented_blocks(callbacks.block_cemented);
        }

        self.processing_timer = Instant::now();
    }

    fn start_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    fn stop_batch_timer(&mut self, cemented_count: usize) {
        let time_spent_cementing = self.cemented_batch_timer.elapsed();

        if time_spent_cementing > Duration::from_millis(50) {
            self.log_cemented_blocks(time_spent_cementing, cemented_count);
        }

        self.cementer
            .batch_write_size
            .adjust_size(time_spent_cementing);
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration, cemented_count: usize) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
                cemented_count,
                time_spent_cementing.as_millis()
            ));
        }
    }

    fn flush(
        &mut self,
        txn: &mut dyn WriteTransaction,
        update: &ConfirmationHeightUpdate,
        scoped_write_guard: &mut WriteGuard,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.ledger.write_confirmation_height(txn, update);

        self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            update.num_blocks_cemented,
            false,
        );
        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        txn.commit();

        self.log_cemented_blocks(
            time_spent_cementing,
            self.cementer.unpublished_cemented_blocks(),
        );
        self.cementer
            .batch_write_size
            .adjust_size(time_spent_cementing);
        scoped_write_guard.release();
        self.cementer
            .publish_cemented_blocks(callbacks.block_cemented);

        // Only aquire transaction if there are blocks left
        if !self.cementer.is_done() {
            *scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.start_batch_timer();
    }

    pub fn clear_process_vars(&mut self) {
        self.helper.clear_accounts_cache();
    }

    pub fn has_pending_writes(&self) -> bool {
        self.cementer.has_pending_writes()
    }

    pub fn container_info(&self) -> BoundedModeContainerInfo {
        BoundedModeContainerInfo {
            pending_writes: self.cementer.container_info(),
            accounts_confirmed: self.helper.accounts_confirmed_info.container_info(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ReceiveChainDetails {
    pub account: Account,
    pub height: u64,
    pub hash: BlockHash,
    pub top_level: BlockHash,
    pub next: Option<BlockHash>,
    pub bottom_height: u64,
    pub bottom_most: BlockHash,
}

#[derive(Clone, Default)]
pub(crate) struct TopAndNextHash {
    pub top: BlockHash,
    pub next: Option<BlockHash>,
    pub next_height: u64,
}

pub(crate) struct ReceiveSourcePair {
    pub receive_details: ReceiveChainDetails,
    pub source_hash: BlockHash,
}

fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}

pub(super) struct BoundedModeContainerInfo {
    pending_writes: WriteDetailsContainerInfo,
    accounts_confirmed: AccountsConfirmedMapContainerInfo,
}

impl BoundedModeContainerInfo {
    pub fn collect(&self) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            "bounded_mode".to_owned(),
            vec![
                self.pending_writes.collect("pending_writes".to_owned()),
                self.accounts_confirmed
                    .collect("accounts_confirmed_info".to_owned()),
            ],
        )
    }
}

pub(crate) struct CementationLedgerAdapter<'a> {
    txn: &'a dyn Transaction,
    ledger: &'a Ledger,
}

impl<'a> CementationDataRequester for CementationLedgerAdapter<'a> {
    fn get_block(&self, block_hash: &BlockHash) -> Option<BlockEnum> {
        self.ledger.store.block().get(self.txn, block_hash)
    }

    fn get_current_confirmation_height(&self, account: &Account) -> ConfirmationHeightInfo {
        self.ledger
            .store
            .confirmation_height()
            .get(self.txn, account)
            .unwrap_or_default()
    }
}

struct BoundedModeHelper {
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    stopped: Arc<AtomicBool>,
    next_in_receive_chain: Option<TopAndNextHash>,
    checkpoints: BoundedVecDeque<BlockHash>,
    receive_source_pairs: BoundedVecDeque<ReceiveSourcePair>,
    accounts_confirmed_info: AccountsConfirmedMap,
    current_hash: BlockHash,
    first_iter: bool,
    original_block: BlockHash,
    receive_details: Option<ReceiveChainDetails>,
    hash_to_process: TopAndNextHash,
    top_level_hash: BlockHash,
    account: Account,
    block_height: u64,
    previous: BlockHash,
    current_confirmation_height: ConfirmationHeightInfo,
    top_most_non_receive_block_hash: BlockHash,
}

impl BoundedModeHelper {
    fn new(ledger: Arc<Ledger>, logger: Arc<dyn Logger>, stopped: Arc<AtomicBool>) -> Self {
        Self {
            ledger,
            logger,
            stopped,
            checkpoints: BoundedVecDeque::new(MAX_ITEMS),
            receive_source_pairs: BoundedVecDeque::new(MAX_ITEMS),
            accounts_confirmed_info: AccountsConfirmedMap::new(),
            next_in_receive_chain: None,
            current_hash: BlockHash::zero(),
            first_iter: true,
            original_block: BlockHash::zero(),
            receive_details: None,
            hash_to_process: Default::default(),
            top_level_hash: BlockHash::zero(),
            account: Account::zero(),
            block_height: 0,
            previous: BlockHash::zero(),
            current_confirmation_height: Default::default(),
            top_most_non_receive_block_hash: BlockHash::zero(),
        }
    }

    fn initialize(&mut self, original_block: BlockHash) {
        self.checkpoints.clear();
        self.receive_source_pairs.clear();
        self.next_in_receive_chain = None;
        self.current_hash = BlockHash::zero();
        self.first_iter = true;
        self.original_block = original_block;
        self.receive_details = None;
        self.hash_to_process = Default::default();
        self.top_level_hash = BlockHash::zero();
        self.account = Account::zero();
        self.block_height = 0;
        self.previous = BlockHash::zero();
        self.current_confirmation_height = Default::default();
        self.top_most_non_receive_block_hash = BlockHash::zero();
    }

    fn load_next_block(&mut self, txn: &dyn Transaction) -> bool {
        self.receive_details = None;
        self.hash_to_process = self.get_next_block_hash();
        self.current_hash = self.hash_to_process.top;
        self.top_level_hash = self.current_hash;

        let current_block = self.ledger.store.block().get(txn, &self.current_hash);

        let Some(current_block) = current_block else{
                if self.ledger.pruning_enabled() && self.ledger.store.pruned().exists(txn, &self.current_hash) {
                    if !self.receive_source_pairs.is_empty() {
                        self.receive_source_pairs.pop_back();
                    }
                    return false;
                } else {
                    let error_str = format!("Ledger mismatch trying to set confirmation height for block {} (bounded processor)", self.current_hash);
                    self.logger.always_log(&error_str);
                    eprintln!("{}", error_str);
                    panic!("{}", error_str);
                }
            };

        self.account = current_block.account_calculated();
        self.block_height = current_block.sideband().unwrap().height;
        self.previous = current_block.previous();
        self.current_confirmation_height =
            self.get_confirmation_height(self.account, txn, &self.accounts_confirmed_info);

        true
    }

    /// The next block hash to iterate over, the priority is as follows:
    /// 1 - The next block in the account chain for the last processed receive (if there is any)
    /// 2 - The next receive block which is closest to genesis
    /// 3 - The last checkpoint hit.
    /// 4 - The hash that was passed in originally. Either all checkpoints were exhausted (this can happen when there are many accounts to genesis)
    ///     or all other blocks have been processed.
    fn get_next_block_hash(&mut self) -> TopAndNextHash {
        if let Some(next_in_chain) = &self.next_in_receive_chain {
            next_in_chain.clone()
        } else if let Some(next_receive_source_pair) = self.receive_source_pairs.back() {
            self.receive_details = Some(next_receive_source_pair.receive_details.clone());
            TopAndNextHash {
                top: next_receive_source_pair.source_hash,
                next: next_receive_source_pair.receive_details.next,
                next_height: next_receive_source_pair.receive_details.height + 1,
            }
        } else if let Some(checkpoint) = self.checkpoints.back() {
            TopAndNextHash {
                top: *checkpoint,
                next: None,
                next_height: 0,
            }
        } else {
            TopAndNextHash {
                top: self.original_block,
                next: None,
                next_height: 0,
            }
        }
    }

    fn get_confirmation_height(
        &self,
        account: Account,
        txn: &dyn Transaction,
        accounts_confirmed_info: &AccountsConfirmedMap,
    ) -> ConfirmationHeightInfo {
        // Checks if we have encountered this account before but not commited changes yet, if so then update the cached confirmation height
        if let Some(found_info) = accounts_confirmed_info.get(&account) {
            ConfirmationHeightInfo::new(found_info.confirmed_height, found_info.iterated_frontier)
        } else {
            self.ledger
                .store
                .confirmation_height()
                .get(txn, &account)
                .unwrap_or_default()
        }
    }

    fn is_already_cemented(&self) -> bool {
        self.current_confirmation_height.height >= self.block_height
    }

    fn should_notify_already_cemented(&self) -> bool {
        self.first_iter && self.is_already_cemented() && self.current_hash == self.original_block
    }

    fn blocks_to_cement_for_this_account(&self) -> u64 {
        // If we are not already at the bottom of the account chain (1 above cemented frontier) then find it
        if self.is_already_cemented() {
            0
        } else {
            self.block_height - self.current_confirmation_height.height
        }
    }

    fn get_least_unconfirmed_hash_from_top_level(&mut self, txn: &dyn Transaction) -> BlockHash {
        let mut least_unconfirmed_hash = self.current_hash;
        if self.current_confirmation_height.height != 0 {
            if self.block_height > self.current_confirmation_height.height {
                let block = self
                    .ledger
                    .store
                    .block()
                    .get(txn, &self.current_confirmation_height.frontier)
                    .unwrap();
                least_unconfirmed_hash = block.sideband().unwrap().successor;
                self.block_height = block.sideband().unwrap().height + 1;
            }
        } else {
            // No blocks have been confirmed, so the first block will be the open block
            let info = self.ledger.account_info(txn, &self.account).unwrap();
            least_unconfirmed_hash = info.open_block;
            self.block_height = 1;
        }
        return least_unconfirmed_hash;
    }

    fn goto_least_unconfirmed_hash(&mut self, txn: &dyn Transaction) {
        if self.blocks_to_cement_for_this_account() > 1 {
            if self.blocks_to_cement_for_this_account() == 2 {
                // If there is 1 uncemented block in-between this block and the cemented frontier,
                // we can just use the previous block to get the least unconfirmed hash.
                self.current_hash = self.previous;
                self.block_height -= 1;
            } else if self.next_in_receive_chain.is_none() {
                self.current_hash = self.get_least_unconfirmed_hash_from_top_level(txn);
            } else {
                // Use the cached successor of the last receive which saves having to do more IO in get_least_unconfirmed_hash_from_top_level
                // as we already know what the next block we should process should be.
                self.current_hash = self.hash_to_process.next.unwrap();
                self.block_height = self.hash_to_process.next_height;
            }
        }
    }

    fn iterate(&mut self, txn: &mut dyn ReadTransaction) -> bool {
        let mut reached_target = false;
        let mut hit_receive = false;
        let mut hash = self.current_hash;
        let mut num_blocks = 0;
        while !hash.is_zero() && !reached_target && !self.stopped.load(Ordering::SeqCst) {
            // Keep iterating upwards until we either reach the desired block or the second receive.
            // Once a receive is cemented, we can cement all blocks above it until the next receive, so store those details for later.
            num_blocks += 1;
            let block = self.ledger.store.block().get(txn.txn(), &hash).unwrap();
            let source = block.source_or_link();
            //----------------------------------------
            if !source.is_zero()
                && !self.ledger.is_epoch_link(&source.into())
                && self.ledger.store.block().exists(txn.txn(), &source)
            {
                hit_receive = true;
                reached_target = true;
                let sideband = block.sideband().unwrap();
                let next =
                    if !sideband.successor.is_zero() && sideband.successor != self.top_level_hash {
                        Some(sideband.successor)
                    } else {
                        None
                    };
                self.receive_source_pairs.push_back(ReceiveSourcePair {
                    receive_details: ReceiveChainDetails {
                        account: self.account,
                        height: sideband.height,
                        hash,
                        top_level: self.top_level_hash,
                        next,
                        bottom_height: self.block_height,
                        bottom_most: self.current_hash,
                    },
                    source_hash: source,
                });

                // Store a checkpoint every max_items so that we can always traverse a long number of accounts to genesis
                if self.receive_source_pairs.len() % MAX_ITEMS == 0 {
                    self.checkpoints.push_back(self.top_level_hash);
                }
            } else {
                // Found a send/change/epoch block which isn't the desired top level
                self.top_most_non_receive_block_hash = hash;
                if hash == self.top_level_hash {
                    reached_target = true;
                } else {
                    hash = block.sideband().unwrap().successor;
                }
            }

            // We could be traversing a very large account so we don't want to open read transactions for too long.
            if (num_blocks > 0) && num_blocks % BATCH_READ_SIZE == 0 {
                txn.refresh();
            }
        }
        hit_receive
    }

    /// Add the non-receive blocks iterated for this account
    fn cement_non_receive_blocks_for_this_account(
        &mut self,
        txn: &dyn Transaction,
    ) -> Option<WriteDetails> {
        if self.is_already_cemented() {
            return None;
        }

        let height = self
            .ledger
            .store
            .block()
            .account_height(txn, &self.top_most_non_receive_block_hash);

        if height <= self.current_confirmation_height.height {
            return None;
        }

        let confirmed_info = ConfirmedInfo {
            confirmed_height: height,
            iterated_frontier: self.top_most_non_receive_block_hash,
        };

        self.accounts_confirmed_info
            .insert(self.account, confirmed_info);

        truncate_after(&mut self.checkpoints, &self.top_most_non_receive_block_hash);

        Some(WriteDetails {
            account: self.account,
            bottom_height: self.block_height,
            bottom_hash: self.current_hash,
            top_height: height,
            top_hash: self.top_most_non_receive_block_hash,
        })
    }

    /// Add the receive block and all non-receive blocks above that one
    fn cement_receive_block_and_all_non_receive_blocks_above(&mut self) -> Option<WriteDetails> {
        let Some(receive_details) = &self.receive_details else { return None; };
        self.accounts_confirmed_info.insert(
            receive_details.account,
            ConfirmedInfo {
                confirmed_height: receive_details.height,
                iterated_frontier: receive_details.hash,
            },
        );

        if receive_details.next.is_some() {
            self.next_in_receive_chain = Some(TopAndNextHash {
                top: receive_details.top_level,
                next: receive_details.next,
                next_height: receive_details.height + 1,
            });
        } else {
            truncate_after(&mut self.checkpoints, &receive_details.hash);
        }

        Some(WriteDetails {
            account: receive_details.account,
            bottom_height: receive_details.bottom_height,
            bottom_hash: receive_details.bottom_most,
            top_height: receive_details.height,
            top_hash: receive_details.hash,
        })
    }

    fn is_done(&self) -> bool {
        self.receive_source_pairs.is_empty() && self.current_hash == self.original_block
    }

    fn finished_iterating(&self) -> bool {
        self.current_hash == self.original_block
    }

    fn clear_accounts_cache(&mut self) {
        self.accounts_confirmed_info.clear();
    }

    fn get_next_step(&mut self, txn: &mut dyn ReadTransaction) -> BoundedCementationStep {
        let mut next_step = BoundedCementationStep {
            write_details: [None, None],
            already_cemented: false,
            is_done: false,
        };

        while !self.load_next_block(txn.txn()) {
            if self.is_done() || self.stopped.load(Ordering::SeqCst) {
                next_step.is_done = true;
                return next_step;
            }
        }

        // This block was added to the confirmation height processor but is already confirmed
        if self.should_notify_already_cemented() {
            next_step.already_cemented = true;
            next_step.is_done = true;
            return next_step;
        }

        self.goto_least_unconfirmed_hash(txn.txn());
        self.top_most_non_receive_block_hash = self.current_hash;

        let hit_receive = if !self.is_already_cemented() {
            self.iterate(txn)
        } else {
            false
        };

        // Exit early when the processor has been stopped, otherwise this function may take a
        // while (and hence keep the process running) if updating a long chain.
        if self.stopped.load(Ordering::SeqCst) {
            next_step.is_done = true;
            return next_step;
        }

        // next_in_receive_chain can be modified when writing, so need to cache it here before resetting
        let is_set = self.next_in_receive_chain.is_some();
        self.next_in_receive_chain = None;
        self.first_iter = false;

        // Need to also handle the case where we are hitting receives where the sends below should be confirmed
        if !hit_receive
            || (self.receive_source_pairs.len() == 1
                && self.top_most_non_receive_block_hash != self.current_hash)
        {
            next_step.write_details = self.write_next(txn.txn(), is_set);
        }

        next_step.is_done = self.is_done();
        next_step
    }

    fn write_next(&mut self, txn: &dyn Transaction, is_set: bool) -> [Option<WriteDetails>; 2] {
        // Once the path to genesis has been iterated to, we can begin to cement the lowest blocks in the accounts. This sets up
        // the non-receive blocks which have been iterated for an account, and the associated receive block.
        let cement_non_receives = self.cement_non_receive_blocks_for_this_account(txn);

        let cement_receive = self.cement_receive_block_and_all_non_receive_blocks_above();

        // If used the top level, don't pop off the receive source pair because it wasn't used
        if !is_set && !self.receive_source_pairs.is_empty() {
            self.receive_source_pairs.pop_back();
        }

        [cement_non_receives, cement_receive]
    }
}

struct BoundedCementationStep {
    write_details: [Option<WriteDetails>; 2],
    already_cemented: bool,
    is_done: bool,
}
