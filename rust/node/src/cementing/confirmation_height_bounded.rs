use std::{
    cmp::max,
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::{Duration, Instant},
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::{ReadTransaction, Transaction};

use crate::config::Logging;

pub type NotifyObserversCallback = Box<dyn Fn(&Vec<Arc<RwLock<BlockEnum>>>)>;

pub struct ConfirmedInfo {
    pub confirmed_height: u64,
    pub iterated_frontier: BlockHash,
}

pub struct ConfirmationHeightBounded {
    write_database_queue: Arc<WriteDatabaseQueue>,
    pub pending_writes: VecDeque<WriteDetails>,
    notify_observers_callback: NotifyObserversCallback,
    notify_block_already_cemented_observers_callback: Box<dyn Fn(BlockHash)>,
    logger: Arc<dyn Logger>,
    logging: Logging,
    ledger: Arc<Ledger>,

    /* Holds confirmation height/cemented frontier in memory for accounts while iterating */
    pub accounts_confirmed_info: HashMap<Account, ConfirmedInfo>,

    stopped: Arc<AtomicBool>,
    timer: Instant,
    batch_separate_pending_min_time: Duration,
    awaiting_processing_size_callback: Box<dyn Fn() -> u64>,

    // All of the atomic variables here just track the size for use in collect_container_info.
    // This is so that no mutexes are needed during the algorithm itself, which would otherwise be needed
    // for the sake of a rarely used RPC call for debugging purposes. As such the sizes are not being acted
    // upon in any way (does not synchronize with any other data).
    // This allows the load and stores to use relaxed atomic memory ordering.
    batch_write_size: Arc<AtomicU64>,
    pub accounts_confirmed_info_size: AtomicUsize,
    pub pending_writes_size: AtomicUsize,
}

const MAXIMUM_BATCH_WRITE_TIME: u64 = 250; // milliseconds
const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: u64 =
    MAXIMUM_BATCH_WRITE_TIME - (MAXIMUM_BATCH_WRITE_TIME / 5);
const MINIMUM_BATCH_WRITE_SIZE: u64 = 16384;

/** The maximum number of various containers to keep the memory bounded */
const MAX_ITEMS: usize = 131072;

/** The maximum number of blocks to be read in while iterating over a long account chain */
const BATCH_READ_SIZE: u64 = 65536;

const PENDING_WRITES_MAX_SIZE: usize = MAX_ITEMS;

impl ConfirmationHeightBounded {
    pub fn new(
        write_database_queue: Arc<WriteDatabaseQueue>,
        notify_observers_callback: NotifyObserversCallback,
        notify_block_already_cemented_observers_callback: Box<dyn Fn(BlockHash)>,
        batch_write_size: Arc<AtomicU64>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        ledger: Arc<Ledger>,
        stopped: Arc<AtomicBool>,
        batch_separate_pending_min_time: Duration,
        awaiting_processing_size_callback: Box<dyn Fn() -> u64>,
    ) -> Self {
        Self {
            write_database_queue,
            pending_writes: VecDeque::new(),
            notify_observers_callback,
            notify_block_already_cemented_observers_callback,
            batch_write_size,
            logger,
            logging,
            ledger,
            accounts_confirmed_info: HashMap::new(),
            accounts_confirmed_info_size: AtomicUsize::new(0),
            pending_writes_size: AtomicUsize::new(0),
            stopped,
            timer: Instant::now(),
            batch_separate_pending_min_time,
            awaiting_processing_size_callback,
        }
    }

    pub fn cement_blocks(&mut self, scoped_write_guard: &mut WriteGuard) -> Option<WriteGuard> {
        let mut new_scoped_write_guard = None;
        let mut cemented_batch_timer: Instant;
        let mut error = false;
        let amount_to_change = self.batch_write_size.load(Ordering::SeqCst) / 10; // 10%

        // Will contain all blocks that have been cemented (bounded by batch_write_size)
        // and will get run through the cemented observer callback
        let mut cemented_blocks: Vec<Arc<RwLock<BlockEnum>>> = Vec::new();

        {
            // This only writes to the confirmation_height table and is the only place to do so in a single process
            let mut txn = self.ledger.store.tx_begin_write();
            cemented_batch_timer = Instant::now();

            // Cement all pending entries, each entry is specific to an account and contains the least amount
            // of blocks to retain consistent cementing across all account chains to genesis.
            while !error && !self.pending_writes.is_empty() {
                let pending = self.pending_writes.front().unwrap();
                let account = pending.account;
                let confirmation_height_info = self
                    .ledger
                    .store
                    .confirmation_height()
                    .get(txn.txn(), &pending.account)
                    .unwrap_or_default();

                // Some blocks need to be cemented at least
                if pending.top_height > confirmation_height_info.height {
                    // The highest hash which will be cemented
                    let mut new_cemented_frontier: BlockHash;
                    let num_blocks_confirmed: u64;
                    let start_height: u64;
                    if pending.bottom_height > confirmation_height_info.height {
                        new_cemented_frontier = pending.bottom_hash;
                        // If we are higher than the cemented frontier, we should be exactly 1 block above
                        debug_assert!(pending.bottom_height == confirmation_height_info.height + 1);
                        num_blocks_confirmed = pending.top_height - pending.bottom_height + 1;
                        start_height = pending.bottom_height;
                    } else {
                        let block = self
                            .ledger
                            .store
                            .block()
                            .get(txn.txn(), &confirmation_height_info.frontier)
                            .unwrap();
                        new_cemented_frontier = block.sideband().unwrap().successor;
                        num_blocks_confirmed = pending.top_height - confirmation_height_info.height;
                        start_height = confirmation_height_info.height + 1;
                    }

                    let mut total_blocks_cemented = 0;
                    let mut block = self
                        .ledger
                        .store
                        .block()
                        .get(txn.txn(), &new_cemented_frontier)
                        .map(|b| Arc::new(RwLock::new(b))); // todo remove RwLock???

                    // Cementing starts from the bottom of the chain and works upwards. This is because chains can have effectively
                    // an infinite number of send/change blocks in a row. We don't want to hold the write transaction open for too long.
                    for num_blocks_iterated in 0..num_blocks_confirmed {
                        if block.is_none() {
                            let error_str = format!(
                            "Failed to write confirmation height for block {} (bounded processor)",
                            new_cemented_frontier
                        );
                            self.logger.always_log(&error_str);
                            eprintln!("{}", error_str);
                            // Undo any blocks about to be cemented from this account for this pending write.
                            cemented_blocks
                                .truncate(cemented_blocks.len() - num_blocks_iterated as usize);
                            error = true;
                            break;
                        }

                        let last_iteration = (num_blocks_confirmed - num_blocks_iterated) == 1;

                        cemented_blocks.push(block.as_ref().unwrap().clone());

                        // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
                        // Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
                        if cemented_blocks.len() as u64
                            > self.batch_write_size.load(Ordering::SeqCst)
                                + (self.batch_write_size.load(Ordering::SeqCst) / 10)
                        {
                            let time_spent_cementing =
                                cemented_batch_timer.elapsed().as_millis() as u64;

                            let num_blocks_cemented =
                                num_blocks_iterated - total_blocks_cemented + 1;
                            total_blocks_cemented += num_blocks_cemented;

                            self.ledger.write_confirmation_height(
                                txn.as_mut(),
                                &account,
                                num_blocks_cemented,
                                start_height + total_blocks_cemented - 1,
                                &new_cemented_frontier,
                            );

                            txn.commit();

                            if self.logging.timing_logging_value {
                                self.logger.always_log(&format!(
                                    "Cemented {} blocks in {} ms (bounded processor)",
                                    cemented_blocks.len(),
                                    time_spent_cementing
                                ));
                            }

                            // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
                            if time_spent_cementing > MAXIMUM_BATCH_WRITE_TIME {
                                // Reduce (unless we have hit a floor)
                                self.batch_write_size.store(
                                    max(
                                        MINIMUM_BATCH_WRITE_SIZE,
                                        self.batch_write_size.load(Ordering::SeqCst)
                                            - amount_to_change,
                                    ),
                                    Ordering::SeqCst,
                                );
                            } else if time_spent_cementing
                                < MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF
                            {
                                // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
                                self.batch_write_size
                                    .fetch_add(amount_to_change, Ordering::SeqCst);
                            }

                            scoped_write_guard.release();

                            (self.notify_observers_callback)(&cemented_blocks);

                            cemented_blocks.clear();

                            // Only aquire transaction if there are blocks left
                            if !(last_iteration && self.pending_writes.len() == 1) {
                                new_scoped_write_guard = Some(
                                    self.write_database_queue.wait(Writer::ConfirmationHeight),
                                );
                                txn.renew();
                            }

                            cemented_batch_timer = Instant::now();
                        }

                        // Get the next block in the chain until we have reached the final desired one
                        if !last_iteration {
                            new_cemented_frontier = block
                                .as_ref()
                                .unwrap()
                                .read()
                                .unwrap()
                                .sideband()
                                .unwrap()
                                .successor;
                            block = self
                                .ledger
                                .store
                                .block()
                                .get(txn.txn(), &new_cemented_frontier)
                                .map(|b| Arc::new(RwLock::new(b)));
                        } else {
                            // Confirm it is indeed the last one
                            debug_assert!(
                                new_cemented_frontier
                                    == self.pending_writes.front().unwrap().top_hash
                            );
                        }
                    }

                    let num_blocks_cemented = num_blocks_confirmed - total_blocks_cemented;
                    if num_blocks_cemented > 0 {
                        self.ledger.write_confirmation_height(
                            txn.as_mut(),
                            &account,
                            num_blocks_cemented,
                            pending.top_height,
                            &new_cemented_frontier,
                        );
                    }
                }

                if let Some(found_info) = self.accounts_confirmed_info.get(&pending.account) {
                    if found_info.confirmed_height == pending.top_height {
                        self.accounts_confirmed_info.remove(&pending.account);
                        self.accounts_confirmed_info_size
                            .fetch_add(1, Ordering::Relaxed);
                    }
                }
                self.pending_writes.pop_front();
                self.pending_writes_size.fetch_sub(1, Ordering::Relaxed);
            }
        }

        let time_spent_cementing = cemented_batch_timer.elapsed().as_millis();
        if self.logging.timing_logging_value && time_spent_cementing > 50 {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
                cemented_blocks.len(),
                time_spent_cementing
            ));
        }

        // Scope guard could have been released earlier (0 cemented_blocks would indicate that)
        if scoped_write_guard.is_owned() && !cemented_blocks.is_empty() {
            scoped_write_guard.release();
            (self.notify_observers_callback)(&cemented_blocks);
        }

        // Bail if there was an error. This indicates that there was a fatal issue with the ledger
        // (the blocks probably got rolled back when they shouldn't have).
        assert!(!error);

        if time_spent_cementing as u64 > MAXIMUM_BATCH_WRITE_TIME {
            // Reduce (unless we have hit a floor)
            self.batch_write_size.store(
                max(
                    MINIMUM_BATCH_WRITE_SIZE,
                    self.batch_write_size.load(Ordering::SeqCst) - amount_to_change,
                ),
                Ordering::SeqCst,
            );
        }

        self.timer = Instant::now();

        debug_assert!(self.pending_writes.is_empty());
        debug_assert!(self.pending_writes_size.load(Ordering::Relaxed) == 0);

        new_scoped_write_guard
    }

    // Once the path to genesis has been iterated to, we can begin to cement the lowest blocks in the accounts. This sets up
    // the non-receive blocks which have been iterated for an account, and the associated receive block.
    pub fn prepare_iterated_blocks_for_cementing(
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

                let found_info = self.accounts_confirmed_info.get(account);
                if found_info.is_some() {
                    self.accounts_confirmed_info
                        .insert(*account, confirmed_info_l);
                } else {
                    self.accounts_confirmed_info
                        .insert(*account, confirmed_info_l);
                    self.accounts_confirmed_info_size
                        .fetch_add(1, Ordering::Relaxed);
                }

                truncate_after(checkpoints, top_most_non_receive_block_hash);

                let details = WriteDetails {
                    account: *account,
                    bottom_height: bottom_height,
                    bottom_hash: *bottom_most,
                    top_height: block_height,
                    top_hash: *top_most_non_receive_block_hash,
                };
                self.pending_writes.push_back(details);
                self.pending_writes_size.fetch_add(1, Ordering::Relaxed);
            }
        }

        // Add the receive block and all non-receive blocks above that one
        if let Some(receive_details) = receive_details {
            match self
                .accounts_confirmed_info
                .get_mut(&receive_details.account)
            {
                Some(found_info) => {
                    found_info.confirmed_height = receive_details.height;
                    found_info.iterated_frontier = receive_details.hash;
                }
                None => {
                    let receive_confirmed_info = ConfirmedInfo {
                        confirmed_height: receive_details.height,
                        iterated_frontier: receive_details.hash,
                    };
                    self.accounts_confirmed_info
                        .insert(receive_details.account, receive_confirmed_info);
                    self.accounts_confirmed_info_size
                        .fetch_add(1, Ordering::Relaxed);
                }
            }

            if receive_details.next.is_some() {
                *next_in_receive_chain = Some(TopAndNextHash {
                    top: receive_details.top_level,
                    next: receive_details.next,
                    next_height: receive_details.height + 1,
                });
            } else {
                truncate_after(checkpoints, &receive_details.hash);
            }

            let write_details = WriteDetails {
                account: receive_details.account,
                bottom_height: receive_details.bottom_height,
                bottom_hash: receive_details.bottom_most,
                top_height: receive_details.height,
                top_hash: receive_details.hash,
            };
            self.pending_writes.push_back(write_details);
            self.pending_writes_size.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn iterate(
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

    pub fn get_least_unconfirmed_hash_from_top_level(
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

    /// The next block hash to iterate over, the priority is as follows:
    /// 1 - The next block in the account chain for the last processed receive (if there is any)
    /// 2 - The next receive block which is closest to genesis
    /// 3 - The last checkpoint hit.
    /// 4 - The hash that was passed in originally. Either all checkpoints were exhausted (this can happen when there are many accounts to genesis)
    ///     or all other blocks have been processed.
    pub fn get_next_block(
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

    pub fn process(&mut self, original_block: &BlockEnum) {
        if self.pending_empty() {
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
            let block = if first_iter {
                debug_assert!(current == original_block.hash());
                Some(original_block.clone())
            } else {
                self.ledger.store.block().get(txn.txn(), &current)
            };

            let Some(block) = block else{
			if self.ledger.pruning_enabled () && self.ledger.store.pruned ().exists (txn.txn(), &current) {
				if !receive_source_pairs.is_empty () {
					receive_source_pairs.pop_back ();
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

            // Checks if we have encountered this account before but not commited changes yet, if so then update the cached confirmation height
            let confirmation_height_info = if let Some(found_info) =
                self.accounts_confirmed_info.get(&account)
            {
                ConfirmationHeightInfo::new(
                    found_info.confirmed_height,
                    found_info.iterated_frontier,
                )
            } else {
                let conf_info = self
                    .ledger
                    .store
                    .confirmation_height()
                    .get(txn.txn(), &account)
                    .unwrap_or_default();
                // This block was added to the confirmation height processor but is already confirmed
                if first_iter
                    && conf_info.height >= block.sideband().unwrap().height
                    && current == original_block.hash()
                {
                    (self.notify_block_already_cemented_observers_callback)(original_block.hash());
                }
                conf_info
            };

            let mut block_height = block.sideband().unwrap().height;
            let already_cemented = confirmation_height_info.height >= block_height;

            // If we are not already at the bottom of the account chain (1 above cemented frontier) then find it
            if !already_cemented && block_height - confirmation_height_info.height > 1 {
                if block_height - confirmation_height_info.height == 2 {
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

                let total_pending_write_block_count = self.total_pending_write_block_count();
                let max_batch_write_size_reached =
                    total_pending_write_block_count >= self.batch_write_size.load(Ordering::SeqCst);

                // When there are a lot of pending confirmation height blocks, it is more efficient to
                // bulk some of them up to enable better write performance which becomes the bottleneck.
                let min_time_exceeded =
                    self.timer.elapsed() >= self.batch_separate_pending_min_time;
                let finished_iterating = current == original_block.hash();
                let non_awaiting_processing = (self.awaiting_processing_size_callback)() == 0;
                let should_output =
                    finished_iterating && (non_awaiting_processing || min_time_exceeded);

                let force_write = self.pending_writes.len() >= PENDING_WRITES_MAX_SIZE
                    || self.accounts_confirmed_info.len() >= PENDING_WRITES_MAX_SIZE;

                if (max_batch_write_size_reached || should_output || force_write)
                    && !self.pending_writes.is_empty()
                {
                    // If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
                    if self
                        .write_database_queue
                        .process(Writer::ConfirmationHeight)
                    {
                        // todo: this does not seem thread safe!
                        let mut scoped_write_guard = self.write_database_queue.pop();
                        self.cement_blocks(&mut scoped_write_guard);
                    } else if force_write {
                        let mut scoped_write_guard =
                            self.write_database_queue.wait(Writer::ConfirmationHeight);
                        self.cement_blocks(&mut scoped_write_guard);
                    }
                }
            }

            first_iter = false;
            txn.refresh();

            if !((!receive_source_pairs.is_empty() || current != original_block.hash())
                && !self.stopped.load(Ordering::SeqCst))
            {
                break;
            }
        }

        debug_assert!(checkpoints.is_empty());
    }

    fn total_pending_write_block_count(&self) -> u64 {
        self.pending_writes
            .iter()
            .map(|i| i.top_height - i.bottom_height + 1)
            .sum()
    }

    pub fn clear_process_vars(&mut self) {
        self.accounts_confirmed_info.clear();
        self.accounts_confirmed_info_size
            .store(0, Ordering::Relaxed);
    }

    pub fn pending_empty(&self) -> bool {
        self.pending_writes.is_empty()
    }

    pub fn write_details_size() -> usize {
        std::mem::size_of::<WriteDetails>()
    }

    pub fn confirmed_info_entry_size() -> usize {
        std::mem::size_of::<ConfirmedInfo>() + std::mem::size_of::<Account>()
    }
}

pub struct WriteDetails {
    pub account: Account,
    // This is the first block hash (bottom most) which is not cemented
    pub bottom_height: u64,
    pub bottom_hash: BlockHash,
    // Desired cemented frontier
    pub top_height: u64,
    pub top_hash: BlockHash,
}

#[derive(Clone)]
pub struct ReceiveChainDetails {
    pub account: Account,
    pub height: u64,
    pub hash: BlockHash,
    pub top_level: BlockHash,
    pub next: Option<BlockHash>,
    pub bottom_height: u64,
    pub bottom_most: BlockHash,
}

#[derive(Clone)]
pub struct TopAndNextHash {
    pub top: BlockHash,
    pub next: Option<BlockHash>,
    pub next_height: u64,
}

pub struct ReceiveSourcePair {
    pub receive_details: ReceiveChainDetails,
    pub source_hash: BlockHash,
}

pub fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}
