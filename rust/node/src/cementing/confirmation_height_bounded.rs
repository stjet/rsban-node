use std::{
    cmp::max,
    collections::VecDeque,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    time::Instant,
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

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
        }
    }

    pub fn cement_blocks(
        &self,
        cemented_batch_timer: Instant,
        txn: &mut dyn WriteTransaction,
        cemented_blocks: &mut Vec<Arc<RwLock<BlockEnum>>>,
        scoped_write_guard: &mut WriteGuard,
        amount_to_change: u64,
        error: &mut bool,
    ) -> (Instant, Option<WriteGuard>) {
        let mut new_scoped_write_guard = None;
        let mut new_timer = cemented_batch_timer;

        let pending = self.pending_writes.front().unwrap();
        let account = pending.account;
        let confirmation_height_info = self
            .ledger
            .store
            .confirmation_height()
            .get(txn.txn(), &pending.account)
            .unwrap_or_default();

        // -----

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
                    cemented_blocks.truncate(cemented_blocks.len() - num_blocks_iterated as usize);
                    *error = true;
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
                    let time_spent_cementing = cemented_batch_timer.elapsed().as_millis() as u64;

                    let num_blocks_cemented = num_blocks_iterated - total_blocks_cemented + 1;
                    total_blocks_cemented += num_blocks_cemented;

                    self.ledger.write_confirmation_height(
                        txn,
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
                                self.batch_write_size.load(Ordering::SeqCst) - amount_to_change,
                            ),
                            Ordering::SeqCst,
                        );
                    } else if time_spent_cementing < MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF {
                        // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
                        self.batch_write_size
                            .fetch_add(amount_to_change, Ordering::SeqCst);
                    }

                    scoped_write_guard.release();

                    (self.notify_observers_callback)(&cemented_blocks);

                    cemented_blocks.clear();

                    // Only aquire transaction if there are blocks left
                    if !(last_iteration && self.pending_writes.len() == 1) {
                        new_scoped_write_guard =
                            Some(self.write_database_queue.wait(Writer::ConfirmationHeight));
                        txn.renew();
                    }

                    new_timer = Instant::now();
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
                        new_cemented_frontier == self.pending_writes.front().unwrap().top_hash
                    );
                }
            }

            let num_blocks_cemented = num_blocks_confirmed - total_blocks_cemented;
            if num_blocks_cemented > 0 {
                self.ledger.write_confirmation_height(
                    txn,
                    &account,
                    num_blocks_cemented,
                    pending.top_height,
                    &new_cemented_frontier,
                );
            }
        }

        (new_timer, new_scoped_write_guard)
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

pub fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}
