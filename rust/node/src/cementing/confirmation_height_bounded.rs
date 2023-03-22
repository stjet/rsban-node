use std::{
    cmp::max,
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::Transaction;

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
    batch_write_size: Arc<AtomicU64>,
    logger: Arc<dyn Logger>,
    logging: Logging,
    ledger: Arc<Ledger>,
    pub accounts_confirmed_info: HashMap<Account, ConfirmedInfo>,
    pub accounts_confirmed_info_size: AtomicUsize,
    pub pending_writes_size: AtomicUsize,
}

const MAXIMUM_BATCH_WRITE_TIME: u64 = 250; // milliseconds
const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: u64 =
    MAXIMUM_BATCH_WRITE_TIME - (MAXIMUM_BATCH_WRITE_TIME / 5);
const MINIMUM_BATCH_WRITE_SIZE: u64 = 16384;

impl ConfirmationHeightBounded {
    pub fn new(
        write_database_queue: Arc<WriteDatabaseQueue>,
        notify_observers_callback: NotifyObserversCallback,
        batch_write_size: Arc<AtomicU64>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        ledger: Arc<Ledger>,
    ) -> Self {
        Self {
            write_database_queue,
            pending_writes: VecDeque::new(),
            notify_observers_callback,
            batch_write_size,
            logger,
            logging,
            ledger,
            accounts_confirmed_info: HashMap::new(),
            accounts_confirmed_info_size: AtomicUsize::new(0),
            pending_writes_size: AtomicUsize::new(0),
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

pub struct ReceiveChainDetails {
    pub account: Account,
    pub height: u64,
    pub hash: BlockHash,
    pub top_level: BlockHash,
    pub next: Option<BlockHash>,
    pub bottom_height: u64,
    pub bottom_most: BlockHash,
}

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
