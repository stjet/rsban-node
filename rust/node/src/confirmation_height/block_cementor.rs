use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, Writer};
use rsnano_store_traits::{Table, WriteTransaction};

use crate::{
    config::Logging,
    stats::{DetailType, Direction, Stat, StatType},
};

use super::{cement_queue::CementQueue, ConfHeightDetails};

// Cements blocks. That means it increases the confirmation_height of the account
pub(crate) struct BlockCementor {
    timer: Instant,
    batch_separate_pending_min_time: Duration,
    pub write_database_queue: Arc<WriteDatabaseQueue>,
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    pub logging: Logging,
    stats: Arc<Stat>,
    notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
    block_cache: Mutex<HashMap<BlockHash, Arc<BlockEnum>>>,
}

impl BlockCementor {
    pub(crate) fn new(
        batch_separate_pending_min_time: Duration,
        write_database_queue: Arc<WriteDatabaseQueue>,
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        stats: Arc<Stat>,
        notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
    ) -> Self {
        Self {
            timer: Instant::now(),
            batch_separate_pending_min_time,
            write_database_queue,
            ledger,
            logger,
            logging,
            stats,
            notify_observers_callback,
            block_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn restart_timer(&mut self) {
        self.timer = Instant::now();
    }

    pub fn min_time_exceeded(&self) -> bool {
        self.timer.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn cement_pending_blocks(
        &mut self,
        cement_queue: &mut CementQueue,
        block_cache: &HashMap<BlockHash, Arc<BlockEnum>>,
    ) {
        let mut scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
        let cemented_batch_timer: Instant;
        let mut cemented_blocks: Vec<Arc<BlockEnum>> = Vec::new();
        {
            let mut txn = self
                .ledger
                .store
                .tx_begin_write_for(&[Table::ConfirmationHeight])
                .unwrap();

            cemented_batch_timer = Instant::now();

            while let Some(mut pending) = cement_queue.pop() {
                let old_conf_height = self
                    .ledger
                    .store
                    .confirmation_height()
                    .get(txn.txn(), &pending.account)
                    .unwrap_or_default();

                if pending.height > old_conf_height.height {
                    let block = self.ledger.store.block().get(txn.txn(), &pending.hash);

                    debug_assert!(self.ledger.pruning_enabled() || block.is_some());
                    debug_assert!(
                        self.ledger.pruning_enabled()
                            || block.as_ref().unwrap().sideband().unwrap().height == pending.height
                    );

                    if block.is_none() {
                        if self.ledger.pruning_enabled()
                            && self.ledger.store.pruned().exists(txn.txn(), &pending.hash)
                        {
                            continue;
                        } else {
                            let error_str = format!("Failed to write confirmation height for block {} (unbounded processor)", pending.hash);
                            self.logger.always_log(&error_str);
                            panic!("{}", error_str);
                        }
                    }

                    debug_assert!(
                        pending.num_blocks_confirmed == pending.height - old_conf_height.height
                    );

                    self.write_confirmation_height(txn.as_mut(), &pending);
                    self.notify_num_blocks_confirmed(&pending);

                    // Reverse it so that the callbacks start from the lowest newly cemented block and move upwards
                    pending.block_callback_data.reverse();

                    for hash in &pending.block_callback_data {
                        cemented_blocks.push(Arc::clone(block_cache.get(hash).unwrap()));
                    }
                }
            }
        }

        let time_spent_cementing = cemented_batch_timer.elapsed();
        if self.logging.timing_logging_value && time_spent_cementing > Duration::from_millis(50) {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (unbounded processor)",
                cemented_blocks.len(),
                time_spent_cementing.as_millis()
            ));
        }

        scoped_write_guard.release();
        (self.notify_observers_callback)(&cemented_blocks);

        debug_assert!(cement_queue.len() == 0);
        debug_assert!(cement_queue.atomic_len().load(Ordering::Relaxed) == 0);
        self.restart_timer();
    }

    fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        conf_height: &ConfHeightDetails,
    ) {
        self.ledger
            .cache
            .cemented_count
            .fetch_add(conf_height.num_blocks_confirmed, Ordering::SeqCst);

        self.ledger.store.confirmation_height().put(
            txn,
            &conf_height.account,
            &ConfirmationHeightInfo::new(conf_height.height, conf_height.hash),
        );
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
