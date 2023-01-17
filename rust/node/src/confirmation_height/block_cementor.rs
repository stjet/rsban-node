use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, Writer};
use rsnano_store_traits::Table;

use crate::{
    config::Logging,
    stats::{DetailType, Direction, Stat, StatType},
};

use super::ConfHeightDetails;

// Cements blocks. That means it increases the confirmation_height of the account
pub(crate) struct BlockCementor {
    timer: Instant,
    batch_separate_pending_min_time: Duration,
    pub write_database_queue: Arc<WriteDatabaseQueue>,
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    pub logging: Logging,
    stats: Arc<Stat>,
    pub pending_writes: VecDeque<ConfHeightDetails>,
    pub pending_writes_size: AtomicUsize,
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
            pending_writes: VecDeque::new(),
            pending_writes_size: AtomicUsize::new(0),
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

    pub fn pending_empty(&self) -> bool {
        self.pending_writes.is_empty()
    }

    pub fn add_pending_write(&mut self, details: ConfHeightDetails) {
        self.pending_writes.push_back(details);
        self.pending_writes_size.fetch_add(1, Ordering::Relaxed);
    }

    pub fn erase_first_pending_write(&mut self) {
        self.pending_writes.pop_front();
        self.pending_writes_size.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn total_pending_write_block_count(&self) -> u64 {
        self.pending_writes
            .iter()
            .map(|x| x.num_blocks_confirmed)
            .sum()
    }

    pub fn cement_pending_blocks(
        &mut self,
        block_cache: &Mutex<HashMap<BlockHash, Arc<BlockEnum>>>,
    ) {
        let mut scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
        let cemented_batch_timer: Instant;
        let mut cemented_blocks: Vec<Arc<BlockEnum>> = Vec::new();
        let mut error = false;
        {
            let mut transaction = self
                .ledger
                .store
                .tx_begin_write_for(&[Table::ConfirmationHeight])
                .unwrap();
            cemented_batch_timer = Instant::now();
            while !self.pending_writes.is_empty() {
                let mut pending = self.pending_writes.front().unwrap().clone(); //todo: remove unwrap

                let confirmation_height_info = self
                    .ledger
                    .store
                    .confirmation_height()
                    .get(transaction.txn(), &pending.account)
                    .unwrap_or_default();
                let mut confirmation_height = confirmation_height_info.height;

                if pending.height > confirmation_height {
                    let block = self
                        .ledger
                        .store
                        .block()
                        .get(transaction.txn(), &pending.hash);

                    debug_assert!(self.ledger.pruning_enabled() || block.is_some());
                    debug_assert!(
                        self.ledger.pruning_enabled()
                            || block.as_ref().unwrap().sideband().unwrap().height == pending.height
                    );

                    if block.is_none() {
                        if self.ledger.pruning_enabled()
                            && self
                                .ledger
                                .store
                                .pruned()
                                .exists(transaction.txn(), &pending.hash)
                        {
                            self.erase_first_pending_write();
                            continue;
                        } else {
                            let error_str = format!("Failed to write confirmation height for block {} (unbounded processor)", pending.hash);
                            self.logger.always_log(&error_str);
                            eprintln!("{}", error_str);
                            error = true;
                            break;
                        }
                    }
                    let _ = self.stats.add(
                        StatType::ConfirmationHeight,
                        DetailType::BlocksConfirmed,
                        Direction::In,
                        pending.height - confirmation_height,
                        false,
                    );
                    let _ = self.stats.add(
                        StatType::ConfirmationHeight,
                        DetailType::BlocksConfirmedUnbounded,
                        Direction::In,
                        pending.height - confirmation_height,
                        false,
                    );

                    debug_assert!(
                        pending.num_blocks_confirmed == pending.height - confirmation_height
                    );
                    confirmation_height = pending.height;
                    self.ledger
                        .cache
                        .cemented_count
                        .fetch_add(pending.num_blocks_confirmed, Ordering::SeqCst);

                    self.ledger.store.confirmation_height().put(
                        transaction.as_mut(),
                        &pending.account,
                        &ConfirmationHeightInfo::new(confirmation_height, pending.hash),
                    );

                    // Reverse it so that the callbacks start from the lowest newly cemented block and move upwards
                    pending.block_callback_data.reverse();

                    let cache = block_cache.lock().unwrap();
                    for hash in &pending.block_callback_data {
                        cemented_blocks.push(Arc::clone(cache.get(hash).unwrap()));
                    }
                }
                self.erase_first_pending_write();
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
        assert!(!error);

        debug_assert!(self.pending_writes.len() == 0);
        debug_assert!(self.pending_writes_size.load(Ordering::Relaxed) == 0);
        self.restart_timer();
    }
}
