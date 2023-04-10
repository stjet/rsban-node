use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, ConfirmationHeightUpdate};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, Writer};
use rsnano_store_traits::{Table, Transaction, WriteTransaction};

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{block_cache::BlockCache, cement_queue::CementQueue, ConfHeightDetails};

// Cements blocks. That means it increases the confirmation_height of the account
pub(super) struct BlockCementor {
    batch_start: Instant,
    last_cementation: Instant,
    batch_separate_pending_min_time: Duration,
    write_database_queue: Arc<WriteDatabaseQueue>,
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    stats: Arc<Stats>,
}

impl BlockCementor {
    pub fn new(
        batch_separate_pending_min_time: Duration,
        write_database_queue: Arc<WriteDatabaseQueue>,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        stats: Arc<Stats>,
    ) -> Self {
        Self {
            last_cementation: Instant::now(),
            batch_start: Instant::now(),
            batch_separate_pending_min_time,
            write_database_queue,
            ledger,
            logger,
            enable_timing_logging,
            stats,
        }
    }

    pub fn set_last_cementation(&mut self) {
        self.last_cementation = Instant::now();
    }

    pub fn min_time_exceeded(&self) -> bool {
        self.last_cementation.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn cement_blocks(
        &mut self,
        cement_queue: &mut CementQueue,
        block_cache: &BlockCache,
        cemented_callback: &mut dyn FnMut(&Arc<BlockEnum>),
    ) {
        let mut cemented_blocks = Vec::new();
        {
            let _write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            self.batch_start = Instant::now();
            let mut txn = self
                .ledger
                .store
                .tx_begin_write_for(&[Table::ConfirmationHeight]);

            while let Some(pending) = cement_queue.pop() {
                self.process_pending_entry(
                    txn.as_mut(),
                    pending,
                    block_cache,
                    &mut cemented_blocks,
                );
            }
        }

        self.log_cemented_count(&cemented_blocks);
        for block in &cemented_blocks {
            (cemented_callback)(block);
        }
        debug_assert!(cement_queue.len() == 0);
        debug_assert!(cement_queue.atomic_len().load(Ordering::Relaxed) == 0);
        self.set_last_cementation();
    }

    fn log_cemented_count(&self, cemented_blocks: &Vec<Arc<BlockEnum>>) {
        let time_spent_cementing = self.batch_start.elapsed();
        if self.enable_timing_logging && time_spent_cementing > Duration::from_millis(50) {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (unbounded processor)",
                cemented_blocks.len(),
                time_spent_cementing.as_millis()
            ));
        }
    }

    fn process_pending_entry(
        &self,
        txn: &mut dyn WriteTransaction,
        pending: ConfHeightDetails,
        block_cache: &BlockCache,
        cemented_blocks: &mut Vec<Arc<BlockEnum>>,
    ) {
        let old_conf_height =
            self.get_confirmation_height(txn.txn(), &pending.update_height.account);

        if pending.update_height.new_height <= old_conf_height {
            return;
        }

        match self.check_block_exists(txn.txn(), &pending.update_height.new_cemented_frontier) {
            BlockResult::BlockExists => {}
            BlockResult::BlockWasPruned => {}
            BlockResult::BlockNotFound => panic!(
                "Failed to write confirmation height for block {}",
                pending.update_height.new_cemented_frontier
            ),
        }

        debug_assert!(
            pending.update_height.num_blocks_cemented
                == pending.update_height.new_height - old_conf_height
        );

        self.ledger
            .write_confirmation_height(txn, &pending.update_height);
        self.notify_blocks_cemented(&pending.update_height);

        // Reverse it so that the callbacks start from the lowest newly cemented block and move upwards
        for hash in pending.cemented_in_current_account.iter().rev() {
            cemented_blocks.push(block_cache.get_cached(hash).unwrap());
        }
    }

    fn get_confirmation_height(&self, txn: &dyn Transaction, account: &Account) -> u64 {
        self.ledger
            .store
            .confirmation_height()
            .get(txn, account)
            .unwrap_or_default()
            .height
    }

    fn check_block_exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockResult {
        let block = self.ledger.store.block().get(txn, hash);
        match block {
            Some(_) => BlockResult::BlockExists,
            None => {
                if self.ledger.pruning_enabled() && self.ledger.store.pruned().exists(txn, &hash) {
                    BlockResult::BlockWasPruned
                } else {
                    BlockResult::BlockNotFound
                }
            }
        }
    }

    fn notify_blocks_cemented(&self, update_height: &ConfirmationHeightUpdate) {
        self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedUnbounded,
            Direction::In,
            update_height.num_blocks_cemented as u64,
            false,
        );
    }
}

enum BlockResult {
    BlockExists,
    BlockWasPruned,
    BlockNotFound,
}
