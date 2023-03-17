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
        _timer: Instant,
        txn: &mut dyn WriteTransaction,
        last_iteration: bool,
        cemented_blocks: &mut Vec<Arc<RwLock<BlockEnum>>>,
        scoped_write_guard: &mut WriteGuard,
        amount_to_change: u64,
        time_spent_cementing: u64,
        num_blocks_iterated: u64,
        total_blocks_cemented: &mut u64,
        start_height: u64,
        new_cemented_frontier: &BlockHash,
        account: &Account,
    ) -> (Instant, Option<WriteGuard>) {
        let num_blocks_cemented = num_blocks_iterated - *total_blocks_cemented + 1;
        *total_blocks_cemented += num_blocks_cemented;

        self.ledger.write_confirmation_height(
            txn,
            account,
            num_blocks_cemented,
            start_height + *total_blocks_cemented - 1,
            new_cemented_frontier,
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

        let mut scoped_write_guard = None;
        // Only aquire transaction if there are blocks left
        if !(last_iteration && self.pending_writes.len() == 1) {
            scoped_write_guard = Some(self.write_database_queue.wait(Writer::ConfirmationHeight));
            txn.renew();
        }

        (Instant::now(), scoped_write_guard)
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
