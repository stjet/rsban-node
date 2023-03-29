use std::{
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, Writer};
use rsnano_store_traits::{Table, Transaction, WriteTransaction};

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::{block_cache::BlockCache, cement_queue::CementQueue, BlockCallback, ConfHeightDetails};

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
    block_cemented_callback: BlockCallback,
}

impl BlockCementor {
    pub fn new(
        batch_separate_pending_min_time: Duration,
        write_database_queue: Arc<WriteDatabaseQueue>,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        stats: Arc<Stats>,
        block_cemented_callback: BlockCallback,
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
            block_cemented_callback,
        }
    }

    pub fn set_last_cementation(&mut self) {
        self.last_cementation = Instant::now();
    }

    pub fn min_time_exceeded(&self) -> bool {
        self.last_cementation.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn cement_blocks(&mut self, cement_queue: &mut CementQueue, block_cache: &BlockCache) {
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
            (self.block_cemented_callback)(block);
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
        let old_conf_height = self.get_confirmation_height(txn.txn(), &pending.account);

        if pending.new_height <= old_conf_height {
            return;
        }

        match self.check_block_exists(txn.txn(), &pending.latest_confirmed_block) {
            BlockResult::BlockExists => {}
            BlockResult::BlockWasPruned => {}
            BlockResult::BlockNotFound => panic!(
                "Failed to write confirmation height for block {}",
                pending.latest_confirmed_block
            ),
        }

        debug_assert!(pending.num_blocks_confirmed == pending.new_height - old_conf_height);

        self.write_confirmation_height(txn, &pending);
        self.notify_num_blocks_confirmed(&pending);

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

    fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        conf_height: &ConfHeightDetails,
    ) {
        self.ledger.store.confirmation_height().put(
            txn,
            &conf_height.account,
            &ConfirmationHeightInfo::new(
                conf_height.new_height,
                conf_height.latest_confirmed_block,
            ),
        );

        self.ledger
            .cache
            .cemented_count
            .fetch_add(conf_height.num_blocks_confirmed, Ordering::SeqCst);
    }

    fn notify_num_blocks_confirmed(&self, pending: &super::ConfHeightDetails) {
        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmed,
            Direction::In,
            pending.num_blocks_confirmed,
            false,
        );
        let _ = self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedUnbounded,
            Direction::In,
            pending.num_blocks_confirmed,
            false,
        );
    }
}

enum BlockResult {
    BlockExists,
    BlockWasPruned,
    BlockNotFound,
}
