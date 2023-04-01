use std::{
    cmp::max,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::Logger, BlockEnum, BlockHash, ConfirmationHeightInfo, UpdateConfirmationHeight,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{
    accounts_confirmed_map::AccountsConfirmedMap,
    write_details_queue::{WriteDetails, WriteDetailsQueue},
};

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
    batch_size_amount_to_change: usize,
    total_blocks_cemented: u64,
    num_blocks_confirmed: u64,
    num_blocks_iterated: u64,
    new_cemented_frontier: BlockHash,
    pending: WriteDetails,
    confirmation_height_info: ConfirmationHeightInfo,
    start_height: u64,
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
            batch_size_amount_to_change: batch_write_size.load(Ordering::SeqCst) / 10,
            batch_write_size,
            write_database_queue,
            cemented_blocks: Vec::new(),
            logger,
            enable_timing_logging,
            accounts_confirmed_info,
            scoped_write_guard,
            block_cemented,
            total_blocks_cemented: 0,
            num_blocks_confirmed: 0,
            num_blocks_iterated: 0,
            new_cemented_frontier: BlockHash::zero(),
            pending: Default::default(),
            confirmation_height_info: Default::default(),
            start_height: 0,
        }
    }

    pub(crate) fn write(&mut self) {
        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();
        self.reset_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while !self.pending_writes.is_empty() {
            self.total_blocks_cemented = 0;
            self.pending = self.pending_writes.front().unwrap().clone();
            self.confirmation_height_info = self
                .ledger
                .store
                .confirmation_height()
                .get(txn.txn(), &self.pending.account)
                .unwrap_or_default();

            // Some blocks need to be cemented at least
            if self.pending.top_height > self.confirmation_height_info.height {
                // The highest hash which will be cemented
                if self.pending.bottom_height > self.confirmation_height_info.height {
                    self.new_cemented_frontier = self.pending.bottom_hash;
                    // If we are higher than the cemented frontier, we should be exactly 1 block above
                    debug_assert!(
                        self.pending.bottom_height == self.confirmation_height_info.height + 1
                    );
                    self.num_blocks_confirmed =
                        self.pending.top_height - self.pending.bottom_height + 1;
                    self.start_height = self.pending.bottom_height;
                } else {
                    let frontier = self
                        .ledger
                        .store
                        .block()
                        .get(txn.txn(), &self.confirmation_height_info.frontier)
                        .unwrap();
                    self.new_cemented_frontier = frontier.sideband().unwrap().successor;
                    self.num_blocks_confirmed =
                        self.pending.top_height - self.confirmation_height_info.height;
                    self.start_height = self.confirmation_height_info.height + 1;
                }

                let mut block = self
                    .ledger
                    .store
                    .block()
                    .get(txn.txn(), &self.new_cemented_frontier)
                    .map(|b| Arc::new(b));

                // Cementing starts from the bottom of the chain and works upwards. This is because chains can have effectively
                // an infinite number of send/change blocks in a row. We don't want to hold the write transaction open for too long.
                for i in 0..self.num_blocks_confirmed {
                    self.num_blocks_iterated = i;
                    if block.is_none() {
                        let error_str = format!(
                            "Failed to write confirmation height for block {} (bounded processor)",
                            self.new_cemented_frontier
                        );
                        self.logger.always_log(&error_str);
                        eprintln!("{}", error_str);
                        panic!("{}", error_str);
                    }

                    self.cemented_blocks.push(block.as_ref().unwrap().clone());

                    // Flush these callbacks and continue as we write in batches (ideally maximum 250ms) to not hold write db transaction for too long.
                    if self.should_flush() {
                        self.flush(&mut txn);
                    }

                    // Get the next block in the chain until we have reached the final desired one
                    if !self.is_last_iteration() {
                        self.new_cemented_frontier =
                            block.as_ref().unwrap().sideband().unwrap().successor;
                        block = self
                            .ledger
                            .store
                            .block()
                            .get(txn.txn(), &self.new_cemented_frontier)
                            .map(|b| Arc::new(b));
                    } else {
                        // Confirm it is indeed the last one
                        debug_assert!(
                            self.new_cemented_frontier
                                == self.pending_writes.front().unwrap().top_hash
                        );
                    }
                }

                if self.num_blocks_cemented() > 0 {
                    self.write_confirmation_height(
                        txn.as_mut(),
                        &UpdateConfirmationHeight {
                            account: self.pending.account,
                            new_cemented_frontier: self.new_cemented_frontier,
                            new_height: self.pending.top_height,
                            num_blocks_cemented: self.num_blocks_cemented(),
                        },
                    );
                }
            }

            if let Some(found_info) = self.accounts_confirmed_info.get(&self.pending.account) {
                if found_info.confirmed_height == self.pending.top_height {
                    self.accounts_confirmed_info.remove(&self.pending.account);
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
            self.publish_cemented_blocks();
        }

        if time_spent_cementing > ConfirmationHeightWriter::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce_batch_write_size();
        }
        debug_assert!(self.pending_writes.is_empty());
    }

    fn flush(&mut self, txn: &mut Box<dyn WriteTransaction>) {
        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        self.write_confirmation_height(
            txn.as_mut(),
            &self.get_update_confirmation_height_command(),
        );

        self.total_blocks_cemented += self.num_blocks_cemented2();
        txn.commit();

        self.log_cemented_blocks(time_spent_cementing);
        self.adjust_batch_write_size(time_spent_cementing);
        self.scoped_write_guard.release();
        self.publish_cemented_blocks();

        // Only aquire transaction if there are blocks left
        if self.is_another_flush_needed() {
            *self.scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.reset_batch_timer();
    }

    fn is_another_flush_needed(&self) -> bool {
        !self.is_last_iteration() || self.pending_writes.len() != 1
    }

    fn get_update_confirmation_height_command(&self) -> UpdateConfirmationHeight {
        UpdateConfirmationHeight {
            account: self.pending.account,
            new_cemented_frontier: self.new_cemented_frontier,
            new_height: self.start_height
                + self.total_blocks_cemented
                + self.num_blocks_cemented2()
                - 1,
            num_blocks_cemented: self.num_blocks_cemented2(),
        }
    }

    fn publish_cemented_blocks(&mut self) {
        for block in &self.cemented_blocks {
            (self.block_cemented)(block);
        }

        self.cemented_blocks.clear();
    }

    fn num_blocks_cemented(&self) -> u64 {
        self.num_blocks_confirmed - self.total_blocks_cemented
    }

    // todo: Duplication!
    fn num_blocks_cemented2(&self) -> u64 {
        self.num_blocks_iterated - self.total_blocks_cemented + 1
    }

    fn is_last_iteration(&self) -> bool {
        self.num_blocks_confirmed - self.num_blocks_iterated == 1
    }

    fn should_flush(&self) -> bool {
        self.cemented_blocks.len() > self.min_block_count_for_flush()
    }

    fn min_block_count_for_flush(&self) -> usize {
        // Include a tolerance to save having to potentially wait on the block processor if the number of blocks to cement is only a bit higher than the max.
        let size = self.batch_write_size.load(Ordering::SeqCst);
        size + (size / 10)
    }

    fn adjust_batch_write_size(&self, time_spent_cementing: Duration) {
        // Update the maximum amount of blocks to write next time based on the time it took to cement this batch.
        if time_spent_cementing > Self::MAXIMUM_BATCH_WRITE_TIME {
            self.reduce_batch_write_size();
        } else if time_spent_cementing < Self::MAXIMUM_BATCH_WRITE_TIME_INCREASE_CUTOFF {
            // Increase amount of blocks written for next batch if the time for writing this one is sufficiently lower than the max time to warrant changing
            self.increase_batch_write_size();
        }
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

    pub fn increase_batch_write_size(&self) {
        self.batch_write_size
            .fetch_add(self.batch_size_amount_to_change, Ordering::SeqCst);
    }

    pub fn reduce_batch_write_size(&self) {
        // Reduce (unless we have hit a floor)
        self.batch_write_size.store(
            max(
                ConfirmationHeightWriter::MINIMUM_BATCH_WRITE_SIZE,
                self.batch_write_size.load(Ordering::SeqCst) - self.batch_size_amount_to_change,
            ),
            Ordering::SeqCst,
        );
    }
}

const fn eighty_percent_of(d: Duration) -> Duration {
    let millis = d.as_millis() as u64;
    Duration::from_millis(millis - (millis / 5))
}
