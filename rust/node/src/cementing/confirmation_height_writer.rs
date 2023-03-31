use std::{
    cmp::max,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, UpdateConfirmationHeight};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::write_details_queue::{WriteDetails, WriteDetailsQueue};

pub(crate) struct ConfirmationHeightWriter<'a> {
    pub cemented_batch_timer: Instant,
    pub pending_writes: &'a mut WriteDetailsQueue,
    ledger: &'a Ledger,
    stats: &'a Stats,
    batch_write_size: &'a AtomicUsize,
    write_database_queue: &'a WriteDatabaseQueue,

    /// Will contain all blocks that have been cemented (bounded by batch_write_size)
    /// and will get run through the cemented observer callback
    pub cemented_blocks: Vec<Arc<BlockEnum>>,

    logger: &'a dyn Logger,
    enable_timing_logging: bool,
}

impl<'a> ConfirmationHeightWriter<'a> {
    pub(crate) const MINIMUM_BATCH_WRITE_SIZE: usize = 16384;
    pub(crate) const MAXIMUM_BATCH_WRITE_TIME: Duration = Duration::from_millis(250);

    pub(crate) const MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF: Duration =
        eighty_percent_of(Self::MAXIMUM_BATCH_WRITE_TIME);

    pub fn new(
        pending_writes: &'a mut WriteDetailsQueue,
        ledger: &'a Ledger,
        stats: &'a Stats,
        batch_write_size: &'a AtomicUsize,
        write_database_queue: &'a WriteDatabaseQueue,
        logger: &'a dyn Logger,
        enable_timing_logging: bool,
    ) -> Self {
        Self {
            cemented_batch_timer: Instant::now(),
            pending_writes,
            ledger,
            stats,
            batch_write_size,
            write_database_queue,
            cemented_blocks: Vec::new(),
            logger,
            enable_timing_logging,
        }
    }

    pub(crate) fn do_it(
        &mut self,
        scoped_write_guard: &mut WriteGuard,
        txn: &mut dyn WriteTransaction,
        block_cemented: &mut dyn FnMut(&Arc<BlockEnum>),
        batch_size_amount_to_change: usize,
        account: Account,
        new_cemented_frontier: &mut BlockHash,
        start_height: u64,
        num_blocks_confirmed: u64,
        pending: &WriteDetails,
    ) {
        let mut total_blocks_cemented = 0;

        let mut block = self
            .ledger
            .store
            .block()
            .get(txn.txn(), new_cemented_frontier)
            .map(|b| Arc::new(b));

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
                panic!("{}", error_str);
            }

            let last_iteration = (num_blocks_confirmed - num_blocks_iterated) == 1;

            self.cemented_blocks.push(block.as_ref().unwrap().clone());

            // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
            // Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
            if self.cemented_blocks.len()
                > self.batch_write_size.load(Ordering::SeqCst)
                    + (self.batch_write_size.load(Ordering::SeqCst) / 10)
            {
                let time_spent_cementing = self.cemented_batch_timer.elapsed();

                let num_blocks_cemented = num_blocks_iterated - total_blocks_cemented + 1;
                total_blocks_cemented += num_blocks_cemented;

                self.write_confirmation_height(
                    txn,
                    &UpdateConfirmationHeight {
                        account,
                        new_cemented_frontier: *new_cemented_frontier,
                        new_height: start_height + total_blocks_cemented - 1,
                        num_blocks_cemented,
                    },
                );

                txn.commit();

                if self.enable_timing_logging {
                    self.logger.always_log(&format!(
                        "Cemented {} blocks in {} ms (bounded processor)",
                        self.cemented_blocks.len(),
                        time_spent_cementing.as_millis()
                    ));
                }

                // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
                if time_spent_cementing > Self::MAXIMUM_BATCH_WRITE_TIME {
                    self.reduce_batch_write_size(batch_size_amount_to_change);
                } else if time_spent_cementing < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF {
                    // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
                    self.increase_batch_write_size(batch_size_amount_to_change);
                }

                scoped_write_guard.release();

                for block in &self.cemented_blocks {
                    block_cemented(block);
                }

                self.cemented_blocks.clear();

                // Only aquire transaction if there are blocks left
                if !(last_iteration && self.pending_writes.len() == 1) {
                    *scoped_write_guard =
                        self.write_database_queue.wait(Writer::ConfirmationHeight);
                    txn.renew();
                }

                self.reset_batch_timer();
            }

            // Get the next block in the chain until we have reached the final desired one
            if !last_iteration {
                *new_cemented_frontier = block.as_ref().unwrap().sideband().unwrap().successor;
                block = self
                    .ledger
                    .store
                    .block()
                    .get(txn.txn(), &new_cemented_frontier)
                    .map(|b| Arc::new(b));
            } else {
                // Confirm it is indeed the last one
                debug_assert!(
                    *new_cemented_frontier == self.pending_writes.front().unwrap().top_hash
                );
            }
        }

        let num_blocks_cemented = num_blocks_confirmed - total_blocks_cemented;
        if num_blocks_cemented > 0 {
            self.write_confirmation_height(
                txn,
                &UpdateConfirmationHeight {
                    account,
                    new_cemented_frontier: *new_cemented_frontier,
                    new_height: pending.top_height,
                    num_blocks_cemented,
                },
            );
        }
    }

    pub fn reset_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    pub fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        update_confirmation_height: &UpdateConfirmationHeight,
    ) {
        self.ledger
            .write_confirmation_height(txn, &update_confirmation_height);

        self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            update_confirmation_height.num_blocks_cemented,
            false,
        );
    }

    pub fn increase_batch_write_size(&self, amount_to_change: usize) {
        self.batch_write_size
            .fetch_add(amount_to_change, Ordering::SeqCst);
    }

    pub fn reduce_batch_write_size(&self, amount_to_change: usize) {
        // Reduce (unless we have hit a floor)
        self.batch_write_size.store(
            max(
                ConfirmationHeightWriter::MINIMUM_BATCH_WRITE_SIZE,
                self.batch_write_size.load(Ordering::SeqCst) - amount_to_change,
            ),
            Ordering::SeqCst,
        );
    }
}

const fn eighty_percent_of(d: Duration) -> Duration {
    let millis = d.as_millis() as u64;
    Duration::from_millis(millis - (millis / 5))
}
