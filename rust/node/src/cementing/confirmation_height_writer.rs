use std::{
    cmp::max,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, BlockEnum, BlockHash, UpdateConfirmationHeight};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{accounts_confirmed_map::AccountsConfirmedMap, write_details_queue::WriteDetailsQueue};

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
    accounts_confirmed_info: &'a mut AccountsConfirmedMap,
    scoped_write_guard: &'a mut WriteGuard,
    block_cemented: &'a mut dyn FnMut(&Arc<BlockEnum>),
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
        accounts_confirmed_info: &'a mut AccountsConfirmedMap,
        scoped_write_guard: &'a mut WriteGuard,
        block_cemented: &'a mut dyn FnMut(&Arc<BlockEnum>),
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
            accounts_confirmed_info,
            scoped_write_guard,
            block_cemented,
        }
    }

    pub(crate) fn write(&mut self) {
        let batch_size_amount_to_change = self.batch_size_amount_to_change();
        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();
        self.reset_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while !self.pending_writes.is_empty() {
            let pending = self.pending_writes.front().unwrap().clone();
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
                            txn.as_mut(),
                            &UpdateConfirmationHeight {
                                account,
                                new_cemented_frontier: new_cemented_frontier,
                                new_height: start_height + total_blocks_cemented - 1,
                                num_blocks_cemented,
                            },
                        );

                        txn.commit();

                        self.log_cemented_blocks(time_spent_cementing);

                        // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
                        if time_spent_cementing > Self::MAXIMUM_BATCH_WRITE_TIME {
                            self.reduce_batch_write_size(batch_size_amount_to_change);
                        } else if time_spent_cementing
                            < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF
                        {
                            // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
                            self.increase_batch_write_size(batch_size_amount_to_change);
                        }

                        self.scoped_write_guard.release();

                        for block in &self.cemented_blocks {
                            (self.block_cemented)(block);
                        }

                        self.cemented_blocks.clear();

                        // Only aquire transaction if there are blocks left
                        if !(last_iteration && self.pending_writes.len() == 1) {
                            *self.scoped_write_guard =
                                self.write_database_queue.wait(Writer::ConfirmationHeight);
                            txn.renew();
                        }

                        self.reset_batch_timer();
                    }

                    // Get the next block in the chain until we have reached the final desired one
                    if !last_iteration {
                        new_cemented_frontier =
                            block.as_ref().unwrap().sideband().unwrap().successor;
                        block = self
                            .ledger
                            .store
                            .block()
                            .get(txn.txn(), &new_cemented_frontier)
                            .map(|b| Arc::new(b));
                    } else {
                        // Confirm it is indeed the last one
                        debug_assert!(
                            new_cemented_frontier == self.pending_writes.front().unwrap().top_hash
                        );
                    }
                }

                let num_blocks_cemented = num_blocks_confirmed - total_blocks_cemented;
                if num_blocks_cemented > 0 {
                    self.write_confirmation_height(
                        txn.as_mut(),
                        &UpdateConfirmationHeight {
                            account,
                            new_cemented_frontier: new_cemented_frontier,
                            new_height: pending.top_height,
                            num_blocks_cemented,
                        },
                    );
                }
            }

            if let Some(found_info) = self.accounts_confirmed_info.get(&pending.account) {
                if found_info.confirmed_height == pending.top_height {
                    self.accounts_confirmed_info.remove(&pending.account);
                }
            }
            self.pending_writes.pop_front();
        }

        drop(txn);

        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        if time_spent_cementing > Duration::from_millis(50) {
            self.log_cemented_blocks(time_spent_cementing);
        }

        // Scope guard could have been released earlier (0 cemented_blocks would indicate that)
        if self.scoped_write_guard.is_owned() && !self.cemented_blocks.is_empty() {
            self.scoped_write_guard.release();
            for block in &self.cemented_blocks {
                (self.block_cemented)(block);
            }
        }

        if time_spent_cementing > ConfirmationHeightWriter::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce_batch_write_size(batch_size_amount_to_change);
        }
        debug_assert!(self.pending_writes.is_empty());
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
                self.cemented_blocks.len(),
                time_spent_cementing.as_millis()
            ));
        }
    }

    fn batch_size_amount_to_change(&self) -> usize {
        // 10%
        let amount_to_change = self.batch_write_size.load(Ordering::SeqCst) / 10;
        amount_to_change
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
