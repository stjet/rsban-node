use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash};
use rsnano_ledger::{Ledger, WriteDatabaseQueue};
use rsnano_store_traits::Transaction;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{config::Logging, stats::Stats};

use super::{
    block_cache::BlockCache,
    block_cementor::BlockCementor,
    cement_queue::CementQueue,
    confirmed_iterated_pairs::{ConfirmedIteratedPair, ConfirmedIteratedPairMap},
    implicit_receive_cemented_mapping::ImplictReceiveCementedMapping,
    unconfirmed_receive_and_sources_collector::UnconfirmedReceiveAndSourcesCollector,
    ConfHeightDetails,
};

/// When the uncemented count (block count - cemented count) is less than this use the unbounded processor
const UNBOUNDED_CUTOFF: usize = 16384;

pub struct ConfirmationHeightUnbounded {
    ledger: Arc<Ledger>,
    block_cache: BlockCache,
    logger: Arc<dyn Logger>,
    confirmed_iterated_pairs: ConfirmedIteratedPairMap,
    implicit_receive_cemented_mapping: ImplictReceiveCementedMapping,

    batch_write_size: Arc<AtomicU64>,
    notify_block_already_cemented_callback: Box<dyn Fn(&BlockHash)>,
    awaiting_processing_size_callback: Box<dyn Fn() -> u64>,
    stopped: AtomicBool,
    cement_queue: CementQueue,
    cementor: BlockCementor,
}

impl ConfirmationHeightUnbounded {
    pub fn new(
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        stats: Arc<Stats>,
        batch_separate_pending_min_time: Duration,
        batch_write_size: Arc<AtomicU64>,
        write_database_queue: Arc<WriteDatabaseQueue>,
        notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
        notify_block_already_cemented_callback: Box<dyn Fn(&BlockHash)>,
        awaiting_processing_size_callback: Box<dyn Fn() -> u64>,
    ) -> Self {
        Self {
            ledger: Arc::clone(&ledger),
            logger: Arc::clone(&logger),
            confirmed_iterated_pairs: ConfirmedIteratedPairMap::new(),
            implicit_receive_cemented_mapping: ImplictReceiveCementedMapping::new(),
            block_cache: BlockCache::new(Arc::clone(&ledger)),
            batch_write_size,
            notify_block_already_cemented_callback,
            awaiting_processing_size_callback,
            stopped: AtomicBool::new(false),
            cement_queue: CementQueue::new(),
            cementor: BlockCementor::new(
                batch_separate_pending_min_time,
                write_database_queue,
                ledger,
                logger,
                logging,
                stats,
                notify_observers_callback,
            ),
        }
    }

    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub fn pending_empty(&self) -> bool {
        self.cement_queue.is_empty()
    }

    pub fn pending_writes_size(&self) -> &AtomicUsize {
        &&self.cement_queue.atomic_len()
    }

    pub fn add_confirmed_iterated_pair(
        &mut self,
        account: Account,
        confirmed_height: u64,
        iterated_height: u64,
    ) {
        self.confirmed_iterated_pairs
            .insert(account, confirmed_height, iterated_height);
    }

    pub fn get_blocks(&self, details: &ConfHeightDetails) -> Vec<Arc<BlockEnum>> {
        details
            .cemented_in_current_account
            .iter()
            .map(|hash| self.block_cache.get_cached(hash).unwrap())
            .collect()
    }

    pub fn has_iterated_over_block(&self, hash: &BlockHash) -> bool {
        self.block_cache.contains(hash)
    }

    pub fn block_cache_size(&self) -> usize {
        self.block_cache.len()
    }

    pub fn clear_process_vars(&mut self) {
        // Separate blocks which are pending confirmation height can be batched by a minimum processing time (to improve lmdb disk write performance),
        // so make sure the slate is clean when a new batch is starting.
        self.confirmed_iterated_pairs.clear();
        self.implicit_receive_cemented_mapping.clear();
        self.block_cache.clear();
    }

    pub fn process(&mut self, original_block: Arc<BlockEnum>) {
        if self.pending_empty() {
            self.clear_process_vars();
            self.cementor.set_last_cementation();
        }
        // ConfHeightDetails for the source block of a receive/open.
        // The source is implicitly being cemented by cementing the receive.
        let mut receive_details: Option<Arc<Mutex<ConfHeightDetails>>> = None;

        let mut current_block_hash = original_block.hash();
        let mut cemented_by_original_block: Vec<BlockHash> = Vec::new();

        // List of all receive/open blocks and their corresponding source that are about to be cemented
        let mut receive_source_pairs: Vec<Arc<ReceiveSourcePair>> = Vec::new();

        let mut first_iter = true;
        let mut txn = self.ledger.read_txn();

        loop {
            match receive_source_pairs.last() {
                Some(pair) => {
                    receive_details = Some(Arc::clone(&pair.receive_details));
                    current_block_hash = pair.source_hash;
                }
                None => {
                    // If receive_details is set then this is the final iteration and we are back to the original chain.
                    // We need to confirm any blocks below the original hash (incl self) and the first receive block
                    // (if the original block is not already a receive)
                    if receive_details.is_some() {
                        current_block_hash = original_block.hash();
                        receive_details = None;
                    }
                }
            }

            let current_block = if first_iter {
                debug_assert!(current_block_hash == original_block.hash());
                // This is the original block passed so can use it directly
                self.block_cache.add(Arc::clone(&original_block));
                Some(Arc::clone(&original_block))
            } else {
                self.block_cache.load_block(&current_block_hash, txn.txn())
            };

            let Some(current_block) = current_block else{
            		let error_str = format!("Ledger mismatch trying to set confirmation height for block {} (unbounded processor)", current_block_hash);
                    self.logger.always_log(&error_str);
                    panic!("{}", error_str);
                };

            let current_account = current_block.account_calculated();
            let block_height = current_block.sideband().unwrap().height;
            let heights = self.get_confirmed_and_iterated_heights(&current_account, txn.txn());

            if first_iter && heights.confirmed_height >= block_height {
                // This block was added to the confirmation height processor but is already confirmed
                debug_assert!(current_block_hash == original_block.hash());
                (self.notify_block_already_cemented_callback)(&original_block.hash());
            }

            let count_before_receive = receive_source_pairs.len();
            let mut cemented_by_current_block = Vec::new();
            let already_traversed = heights.iterated_height >= block_height;
            if !already_traversed {
                {
                    let mut collector = UnconfirmedReceiveAndSourcesCollector::new(
                        txn.txn(),
                        current_block,
                        heights.iterated_height,
                        &mut receive_source_pairs,
                        &mut cemented_by_current_block,
                        &mut cemented_by_original_block,
                        &original_block,
                        &self.block_cache,
                        &self.ledger,
                        &mut self.implicit_receive_cemented_mapping,
                    );
                    collector.collect(&self.stopped);
                }
            }

            // Exit early when the processor has been stopped, otherwise this function may take a
            // while (and hence keep the process running) if updating a long chain.
            if self.stopped.load(Ordering::SeqCst) {
                break;
            }

            // No longer need the read transaction
            txn.reset();

            // If this adds no more open or receive blocks, then we can now confirm this account as well as the linked open/receive block
            // Collect as pending any writes to the database and do them in bulk after a certain time.
            let confirmed_receives_pending = count_before_receive != receive_source_pairs.len();
            if !confirmed_receives_pending {
                let mut preparation_data = PreparationData {
                    block_height,
                    confirmation_height: heights.confirmed_height,
                    iterated_height: heights.iterated_height,
                    account_it: self.confirmed_iterated_pairs.get(&current_account).cloned(),
                    account: current_account,
                    receive_details: receive_details.clone(),
                    already_traversed,
                    current: current_block_hash,
                    block_callback_data: &mut cemented_by_current_block,
                    orig_block_callback_data: &mut cemented_by_original_block,
                };
                self.prepare_iterated_blocks_for_cementing(&mut preparation_data);

                receive_source_pairs.pop();
            } else if block_height > heights.iterated_height {
                self.confirmed_iterated_pairs.update_iterated_height(
                    &current_account,
                    heights.confirmed_height,
                    block_height,
                );
            }

            // When there are a lot of pending confirmation height blocks, it is more efficient to
            // bulk some of them up to enable better write performance which becomes the bottleneck.
            let finished_iterating = receive_source_pairs.is_empty();
            let no_pending = (self.awaiting_processing_size_callback)() == 0;
            let should_output =
                finished_iterating && (no_pending || self.cementor.min_time_exceeded());

            let should_cement_pending_blocks =
                (self.max_write_size_reached() || should_output || self.should_force_write())
                    && self.cement_queue.len() > 0;

            if should_cement_pending_blocks {
                self.cement_pending_blocks();
            }

            first_iter = false;
            txn.renew();
            if (receive_source_pairs.is_empty() && current_block_hash == original_block.hash())
                || self.stopped.load(Ordering::SeqCst)
            {
                break;
            }
        }
    }

    fn get_confirmed_and_iterated_heights(
        &self,
        account: &Account,
        txn: &dyn Transaction,
    ) -> ConfirmedIteratedPair {
        let confirmed_and_iterated = match self.confirmed_iterated_pairs.get(account) {
            Some(conf_it) => ConfirmedIteratedPair {
                confirmed_height: conf_it.confirmed_height,
                iterated_height: std::cmp::max(conf_it.iterated_height, conf_it.confirmed_height),
            },
            None => {
                let height_info = self
                    .ledger
                    .store
                    .confirmation_height()
                    .get(txn, account)
                    .unwrap_or_default();

                ConfirmedIteratedPair {
                    confirmed_height: height_info.height,
                    iterated_height: height_info.height,
                }
            }
        };
        confirmed_and_iterated
    }

    fn should_force_write(&mut self) -> bool {
        self.cement_queue.total_cemented_blocks() > self.batch_write_size.load(Ordering::Relaxed)
    }

    fn max_write_size_reached(&self) -> bool {
        self.cement_queue.len() >= UNBOUNDED_CUTOFF
    }

    pub fn prepare_iterated_blocks_for_cementing(
        &mut self,
        preparation_data_a: &mut PreparationData,
    ) {
        let receive_details = &preparation_data_a.receive_details;
        let block_height = preparation_data_a.block_height;
        if block_height > preparation_data_a.confirmation_height {
            // Check whether the previous block has been seen. If so, the rest of sends below have already been seen so don't count them
            if let Some(_) = &preparation_data_a.account_it {
                let pair = self
                    .confirmed_iterated_pairs
                    .get_mut(&preparation_data_a.account)
                    .unwrap();
                pair.confirmed_height = block_height;
                if block_height > preparation_data_a.iterated_height {
                    pair.iterated_height = block_height;
                }
            } else {
                self.add_confirmed_iterated_pair(
                    preparation_data_a.account,
                    block_height,
                    block_height,
                );
            }

            let num_blocks_confirmed = block_height - preparation_data_a.confirmation_height;
            let mut block_callback_data = preparation_data_a.block_callback_data.clone();
            if block_callback_data.is_empty() {
                match receive_details {
                    Some(receive_details) => {
                        let mut receive_details_lock = receive_details.lock().unwrap();
                        if preparation_data_a.already_traversed
                            && receive_details_lock.cemented_in_source.is_empty()
                        {
                            drop(receive_details_lock);
                            // We are confirming a block which has already been traversed and found no associated receive details for it.
                            let above_receive_details_w = self
                                .implicit_receive_cemented_mapping
                                .get(&preparation_data_a.current)
                                .unwrap();
                            debug_assert!(above_receive_details_w.strong_count() > 0);
                            let above_receive_details = above_receive_details_w.upgrade().unwrap();
                            let above_receive_details_lock = above_receive_details.lock().unwrap();

                            let num_blocks_already_confirmed = above_receive_details_lock
                                .num_blocks_confirmed
                                - (above_receive_details_lock.new_height
                                    - preparation_data_a.confirmation_height);

                            let block_data = above_receive_details_lock
                                .cemented_in_current_account
                                .clone();
                            drop(above_receive_details_lock);
                            let end = block_data.len() - (num_blocks_already_confirmed as usize);
                            let start = end - num_blocks_confirmed as usize;

                            block_callback_data.clear();
                            block_callback_data.extend_from_slice(&block_data[start..end]);

                            let num_to_remove =
                                block_callback_data.len() - num_blocks_confirmed as usize;
                            block_callback_data.truncate(block_callback_data.len() - num_to_remove);
                            receive_details_lock = receive_details.lock().unwrap();
                            receive_details_lock.cemented_in_source.clear();
                        } else {
                            block_callback_data = receive_details_lock.cemented_in_source.clone();

                            let num_to_remove =
                                block_callback_data.len() - num_blocks_confirmed as usize;
                            block_callback_data.truncate(block_callback_data.len() - num_to_remove);
                            receive_details_lock.cemented_in_source.clear();
                        }
                    }
                    None => {
                        block_callback_data = preparation_data_a.orig_block_callback_data.clone();
                    }
                }
            }
            self.cement_queue.push(ConfHeightDetails {
                account: preparation_data_a.account,
                latest_confirmed_block: preparation_data_a.current,
                new_height: block_height,
                num_blocks_confirmed,
                cemented_in_current_account: block_callback_data,
                cemented_in_source: Vec::new(),
            });
        }

        if let Some(receive_details) = receive_details {
            let mut receive_details_lock = receive_details.lock().unwrap();
            // Check whether the previous block has been seen. If so, the rest of sends below have already been seen so don't count them
            let receive_account = receive_details_lock.account;
            let receive_account_it = self.confirmed_iterated_pairs.get(&receive_account);
            match receive_account_it {
                Some(receive_account_it) => {
                    // Get current height
                    let current_height = receive_account_it.confirmed_height;
                    let pair = self
                        .confirmed_iterated_pairs
                        .get_mut(&receive_account)
                        .unwrap();
                    pair.confirmed_height = receive_details_lock.new_height;
                    let orig_num_blocks_confirmed = receive_details_lock.num_blocks_confirmed;
                    receive_details_lock.num_blocks_confirmed =
                        receive_details_lock.new_height - current_height;

                    // Get the difference and remove the callbacks
                    let block_callbacks_to_remove =
                        orig_num_blocks_confirmed - receive_details_lock.num_blocks_confirmed;
                    let mut tmp_blocks = receive_details_lock.cemented_in_current_account.clone();
                    tmp_blocks.truncate(tmp_blocks.len() - block_callbacks_to_remove as usize);
                    receive_details_lock.cemented_in_current_account = tmp_blocks;
                    debug_assert!(
                        receive_details_lock.cemented_in_current_account.len()
                            == receive_details_lock.num_blocks_confirmed as usize
                    );
                }
                None => {
                    self.add_confirmed_iterated_pair(
                        receive_account,
                        receive_details_lock.new_height,
                        receive_details_lock.new_height,
                    );
                }
            }

            self.cement_queue.push(receive_details_lock.clone())
        }
    }
    pub fn cement_pending_blocks(&mut self) {
        self.cementor
            .cement_blocks(&mut self.cement_queue, &self.block_cache);
    }

    pub fn implicit_receive_cemented_mapping_size(&self) -> usize {
        self.implicit_receive_cemented_mapping.size_atomic()
    }

    pub fn confirmed_iterated_pairs_size_atomic(&self) -> usize {
        self.confirmed_iterated_pairs.size_atomic()
    }
}

#[derive(Clone)]
pub struct ReceiveSourcePair {
    pub receive_details: Arc<Mutex<ConfHeightDetails>>,
    pub source_hash: BlockHash,
}

impl ReceiveSourcePair {
    pub fn new(receive_details: Arc<Mutex<ConfHeightDetails>>, source_hash: BlockHash) -> Self {
        Self {
            receive_details,
            source_hash,
        }
    }
}

pub struct PreparationData<'a> {
    pub block_height: u64,
    pub confirmation_height: u64,
    pub iterated_height: u64,
    pub account_it: Option<ConfirmedIteratedPair>,
    pub account: Account,
    pub receive_details: Option<Arc<Mutex<ConfHeightDetails>>>,
    pub already_traversed: bool,
    pub current: BlockHash,
    pub block_callback_data: &'a mut Vec<BlockHash>,
    pub orig_block_callback_data: &'a mut Vec<BlockHash>,
}
