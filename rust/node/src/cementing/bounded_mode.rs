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
    write_database_queue: Arc<WriteDatabaseQueue>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    ledger: Arc<Ledger>,
    accounts_confirmed_info: AccountsConfirmedMap,

    stopped: Arc<AtomicBool>,
    timer: Instant,
    batch_separate_pending_min_time: Duration,

    stats: Arc<Stats>,
    cemented_batch_timer: Instant,
    cementer: MultiAccountCementer,
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
        Self {
            write_database_queue,
            logger,
            enable_timing_logging,
            ledger,
            accounts_confirmed_info: AccountsConfirmedMap::new(),
            stopped,
            stats,
            timer: Instant::now(),
            batch_separate_pending_min_time,
            cemented_batch_timer: Instant::now(),
            cementer: MultiAccountCementer::new(),
        }
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.cementer.batch_write_size
    }

    pub(crate) fn process(
        &mut self,
        original_block: &BlockEnum,
        callbacks: &mut CementCallbackRefs,
    ) {
        if !self.has_pending_writes() {
            self.clear_process_vars();
            self.timer = Instant::now();
        }

        let mut next_in_receive_chain: Option<TopAndNextHash> = None;
        let mut checkpoints = BoundedVecDeque::new(MAX_ITEMS);
        let mut receive_source_pairs = BoundedVecDeque::new(MAX_ITEMS);
        let mut current: BlockHash;
        let mut first_iter = true;
        let mut txn = self.ledger.store.tx_begin_read();

        loop {
            let mut receive_details = None;
            let hash_to_process = self.get_next_block(
                &next_in_receive_chain,
                &checkpoints,
                &receive_source_pairs,
                &mut receive_details,
                original_block,
            );
            current = hash_to_process.top;

            let top_level_hash = current;
            let block = if current == original_block.hash() {
                Some(original_block.clone())
            } else {
                self.ledger.store.block().get(txn.txn(), &current)
            };

            let Some(block) = block else{
                if self.ledger.pruning_enabled() && self.ledger.store.pruned().exists(txn.txn(), &current) {
                    if !receive_source_pairs.is_empty() {
                        receive_source_pairs.pop_back();
                    }
                    continue;
                } else {
                    let error_str = format!("Ledger mismatch trying to set confirmation height for block {} (bounded processor)", current);
                    self.logger.always_log(&error_str);
                    eprintln!("{}", error_str);
                    panic!("{}", error_str);
                }
            };

            let account = block.account_calculated();
            let confirmation_height_info = self.get_confirmation_height(account, txn.txn());

            let mut block_height = block.sideband().unwrap().height;
            let already_cemented = confirmation_height_info.height >= block_height;

            // This block was added to the confirmation height processor but is already confirmed
            if first_iter && already_cemented && current == original_block.hash() {
                (callbacks.block_already_cemented)(original_block.hash());
            }

            // If we are not already at the bottom of the account chain (1 above cemented frontier) then find it
            let blocks_to_cement_for_this_account = if already_cemented {
                0
            } else {
                block_height - confirmation_height_info.height
            };
            if blocks_to_cement_for_this_account > 1 {
                if blocks_to_cement_for_this_account == 2 {
                    // If there is 1 uncemented block in-between this block and the cemented frontier,
                    // we can just use the previous block to get the least unconfirmed hash.
                    current = block.previous();
                    block_height -= 1;
                } else if next_in_receive_chain.is_none() {
                    current = self.get_least_unconfirmed_hash_from_top_level(
                        txn.txn(),
                        &current,
                        &account,
                        &confirmation_height_info,
                        &mut block_height,
                    );
                } else {
                    // Use the cached successor of the last receive which saves having to do more IO in get_least_unconfirmed_hash_from_top_level
                    // as we already know what the next block we should process should be.
                    current = hash_to_process.next.unwrap();
                    block_height = hash_to_process.next_height;
                }
            }

            let mut top_most_non_receive_block_hash = current;

            let mut hit_receive = false;
            if !already_cemented {
                hit_receive = self.iterate(
                    &mut receive_source_pairs,
                    &mut checkpoints,
                    top_level_hash,
                    account,
                    block_height,
                    current,
                    &mut top_most_non_receive_block_hash,
                    txn.as_mut(),
                );
            }

            // Exit early when the processor has been stopped, otherwise this function may take a
            // while (and hence keep the process running) if updating a long chain.
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }

            // next_in_receive_chain can be modified when writing, so need to cache it here before resetting
            let is_set = next_in_receive_chain.is_some();
            next_in_receive_chain = None;

            // Need to also handle the case where we are hitting receives where the sends below should be confirmed
            if !hit_receive
                || (receive_source_pairs.len() == 1 && top_most_non_receive_block_hash != current)
            {
                self.prepare_iterated_blocks_for_cementing(
                    &receive_details,
                    &mut checkpoints,
                    &mut next_in_receive_chain,
                    already_cemented,
                    txn.txn(),
                    &top_most_non_receive_block_hash,
                    &confirmation_height_info,
                    &account,
                    block_height,
                    &current,
                );

                // If used the top level, don't pop off the receive source pair because it wasn't used
                if !is_set && !receive_source_pairs.is_empty() {
                    receive_source_pairs.pop_back();
                }

                let max_batch_write_size_reached = self.cementer.max_batch_write_size_reached();

                // When there are a lot of pending confirmation height blocks, it is more efficient to
                // bulk some of them up to enable better write performance which becomes the bottleneck.
                let min_time_exceeded =
                    self.timer.elapsed() >= self.batch_separate_pending_min_time;
                let finished_iterating = current == original_block.hash();
                let non_awaiting_processing = (callbacks.awaiting_processing_count)() == 0;
                let should_output =
                    finished_iterating && (non_awaiting_processing || min_time_exceeded);

                let force_write = self.cementer.max_pending_writes_reached()
                    || self.accounts_confirmed_info.len() >= MAX_ITEMS;

                if (max_batch_write_size_reached || should_output || force_write)
                    && !self.cementer.has_pending_writes()
                {
                    // If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
                    if self
                        .write_database_queue
                        .process(Writer::ConfirmationHeight)
                    {
                        // todo: this does not seem thread safe!
                        let mut scoped_write_guard = self.write_database_queue.pop();
                        self.write_pending_blocks_with_write_guard(
                            &mut scoped_write_guard,
                            callbacks,
                        );
                    } else if force_write {
                        let mut scoped_write_guard =
                            self.write_database_queue.wait(Writer::ConfirmationHeight);
                        self.write_pending_blocks_with_write_guard(
                            &mut scoped_write_guard,
                            callbacks,
                        );
                    }
                }
            }

            first_iter = false;
            txn.refresh();

            let is_done = receive_source_pairs.is_empty() && current == original_block.hash();
            if is_done || self.stopped.load(Ordering::SeqCst) {
                break;
            }
        }

        debug_assert!(checkpoints.is_empty());
    }

    fn get_confirmation_height(
        &self,
        account: Account,
        txn: &dyn Transaction,
    ) -> ConfirmationHeightInfo {
        // Checks if we have encountered this account before but not commited changes yet, if so then update the cached confirmation height
        if let Some(found_info) = self.accounts_confirmed_info.get(&account) {
            ConfirmationHeightInfo::new(found_info.confirmed_height, found_info.iterated_frontier)
        } else {
            self.ledger
                .store
                .confirmation_height()
                .get(txn, &account)
                .unwrap_or_default()
        }
    }

    /// The next block hash to iterate over, the priority is as follows:
    /// 1 - The next block in the account chain for the last processed receive (if there is any)
    /// 2 - The next receive block which is closest to genesis
    /// 3 - The last checkpoint hit.
    /// 4 - The hash that was passed in originally. Either all checkpoints were exhausted (this can happen when there are many accounts to genesis)
    ///     or all other blocks have been processed.
    fn get_next_block(
        &self,
        next_in_receive_chain: &Option<TopAndNextHash>,
        checkpoints: &BoundedVecDeque<BlockHash>,
        receive_source_pairs: &BoundedVecDeque<ReceiveSourcePair>,
        receive_details: &mut Option<ReceiveChainDetails>,
        original_block: &BlockEnum,
    ) -> TopAndNextHash {
        let next: TopAndNextHash;
        if let Some(next_in_chain) = next_in_receive_chain {
            next = next_in_chain.clone();
        } else if let Some(next_receive_source_pair) = receive_source_pairs.back() {
            *receive_details = Some(next_receive_source_pair.receive_details.clone());
            next = TopAndNextHash {
                top: next_receive_source_pair.source_hash,
                next: next_receive_source_pair.receive_details.next,
                next_height: next_receive_source_pair.receive_details.height + 1,
            };
        } else if let Some(checkpoint) = checkpoints.back() {
            next = TopAndNextHash {
                top: *checkpoint,
                next: None,
                next_height: 0,
            }
        } else {
            next = TopAndNextHash {
                top: original_block.hash(),
                next: None,
                next_height: 0,
            };
        }

        next
    }

    fn iterate(
        &self,
        receive_source_pairs: &mut BoundedVecDeque<ReceiveSourcePair>,
        checkpoints: &mut BoundedVecDeque<BlockHash>,
        top_level_hash: BlockHash,
        account: Account,
        bottom_height: u64,
        bottom_hash: BlockHash,
        top_most_non_receive_block_hash: &mut BlockHash,
        txn: &mut dyn ReadTransaction,
    ) -> bool {
        let mut reached_target = false;
        let mut hit_receive = false;
        let mut hash = bottom_hash;
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
                let next = if !sideband.successor.is_zero() && sideband.successor != top_level_hash
                {
                    Some(sideband.successor)
                } else {
                    None
                };
                receive_source_pairs.push_back(ReceiveSourcePair {
                    receive_details: ReceiveChainDetails {
                        account,
                        height: sideband.height,
                        hash,
                        top_level: top_level_hash,
                        next,
                        bottom_height,
                        bottom_most: bottom_hash,
                    },
                    source_hash: source,
                });

                // Store a checkpoint every max_items so that we can always traverse a long number of accounts to genesis
                if receive_source_pairs.len() % MAX_ITEMS == 0 {
                    checkpoints.push_back(top_level_hash);
                }
            } else {
                // Found a send/change/epoch block which isn't the desired top level
                *top_most_non_receive_block_hash = hash;
                if hash == top_level_hash {
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

    // Once the path to genesis has been iterated to, we can begin to cement the lowest blocks in the accounts. This sets up
    // the non-receive blocks which have been iterated for an account, and the associated receive block.
    fn prepare_iterated_blocks_for_cementing(
        &mut self,
        receive_details: &Option<ReceiveChainDetails>,
        checkpoints: &mut BoundedVecDeque<BlockHash>,
        next_in_receive_chain: &mut Option<TopAndNextHash>,
        already_cemented: bool,
        txn: &dyn Transaction,
        top_most_non_receive_block_hash: &BlockHash,
        confirmation_height_info: &ConfirmationHeightInfo,
        account: &Account,
        bottom_height: u64,
        bottom_most: &BlockHash,
    ) {
        if !already_cemented {
            // Add the non-receive blocks iterated for this account
            let block_height = self
                .ledger
                .store
                .block()
                .account_height(txn, top_most_non_receive_block_hash);
            if block_height > confirmation_height_info.height {
                let confirmed_info_l = ConfirmedInfo {
                    confirmed_height: block_height,
                    iterated_frontier: *top_most_non_receive_block_hash,
                };

                self.accounts_confirmed_info
                    .insert(*account, confirmed_info_l);

                truncate_after(checkpoints, top_most_non_receive_block_hash);

                self.cementer.enqueue(WriteDetails {
                    account: *account,
                    bottom_height: bottom_height,
                    bottom_hash: *bottom_most,
                    top_height: block_height,
                    top_hash: *top_most_non_receive_block_hash,
                });
            }
        }

        // Add the receive block and all non-receive blocks above that one
        if let Some(receive_details) = receive_details {
            self.accounts_confirmed_info.insert(
                receive_details.account,
                ConfirmedInfo {
                    confirmed_height: receive_details.height,
                    iterated_frontier: receive_details.hash,
                },
            );

            if receive_details.next.is_some() {
                *next_in_receive_chain = Some(TopAndNextHash {
                    top: receive_details.top_level,
                    next: receive_details.next,
                    next_height: receive_details.height + 1,
                });
            } else {
                truncate_after(checkpoints, &receive_details.hash);
            }

            self.cementer.enqueue(WriteDetails {
                account: receive_details.account,
                bottom_height: receive_details.bottom_height,
                bottom_hash: receive_details.bottom_most,
                top_height: receive_details.height,
                top_hash: receive_details.hash,
            });
        }
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
                if let Some(found_info) = self.accounts_confirmed_info.get(&update_command.account)
                {
                    if found_info.confirmed_height == update_command.new_height {
                        self.accounts_confirmed_info.remove(&update_command.account);
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

        self.timer = Instant::now();
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

    fn get_least_unconfirmed_hash_from_top_level(
        &self,
        txn: &dyn Transaction,
        hash: &BlockHash,
        account: &Account,
        confirmation_height_info: &ConfirmationHeightInfo,
        block_height: &mut u64,
    ) -> BlockHash {
        let mut least_unconfirmed_hash = *hash;
        if confirmation_height_info.height != 0 {
            if *block_height > confirmation_height_info.height {
                let block = self
                    .ledger
                    .store
                    .block()
                    .get(txn, &confirmation_height_info.frontier)
                    .unwrap();
                least_unconfirmed_hash = block.sideband().unwrap().successor;
                *block_height = block.sideband().unwrap().height + 1;
            }
        } else {
            // No blocks have been confirmed, so the first block will be the open block
            let info = self.ledger.account_info(txn, account).unwrap();
            least_unconfirmed_hash = info.open_block;
            *block_height = 1;
        }
        return least_unconfirmed_hash;
    }

    pub fn clear_process_vars(&mut self) {
        self.accounts_confirmed_info.clear();
    }

    pub fn has_pending_writes(&self) -> bool {
        self.cementer.has_pending_writes()
    }

    pub fn container_info(&self) -> BoundedModeContainerInfo {
        BoundedModeContainerInfo {
            pending_writes: self.cementer.pending_writes.container_info(),
            accounts_confirmed: self.accounts_confirmed_info.container_info(),
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

#[derive(Clone)]
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
