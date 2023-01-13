use rsnano_core::{utils::Logger, Account, BlockEnum, BlockHash, ConfirmationHeightInfo};
use rsnano_ledger::{Ledger, WriteGuard};
use rsnano_store_traits::{ReadTransaction, Table, Transaction};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

use crate::{
    config::Logging,
    stats::{DetailType, Direction, Stat, StatType},
};

pub struct ConfirmationHeightUnbounded {
    ledger: Arc<Ledger>,
    logger: Arc<dyn Logger>,
    logging: Logging,
    stats: Arc<Stat>,
    pub pending_writes: VecDeque<ConfHeightDetails>,
    pub confirmed_iterated_pairs: HashMap<Account, ConfirmedIteratedPair>,

    //todo: Remove Mutex
    pub implicit_receive_cemented_mapping: HashMap<BlockHash, Weak<Mutex<ConfHeightDetails>>>,
    pub block_cache: Mutex<HashMap<BlockHash, Arc<BlockEnum>>>,

    // All of the atomic variables here just track the size for use in collect_container_info.
    // This is so that no mutexes are needed during the algorithm itself, which would otherwise be needed
    // for the sake of a rarely used RPC call for debugging purposes. As such the sizes are not being acted
    // upon in any way (does not synchronize with any other data).
    // This allows the load and stores to use relaxed atomic memory ordering.
    pub confirmed_iterated_pairs_size: AtomicUsize,
    pub pending_writes_size: AtomicUsize,
    pub implicit_receive_cemented_mapping_size: AtomicUsize,
    timer: Instant,
    batch_separate_pending_min_time: Duration,
    notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
}

impl ConfirmationHeightUnbounded {
    pub fn new(
        ledger: Arc<Ledger>,
        logger: Arc<dyn Logger>,
        logging: Logging,
        stats: Arc<Stat>,
        batch_separate_pending_min_time: Duration,
        notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
    ) -> Self {
        Self {
            ledger,
            logger,
            logging,
            stats,
            pending_writes: VecDeque::new(),
            confirmed_iterated_pairs: HashMap::new(),
            implicit_receive_cemented_mapping: HashMap::new(),
            block_cache: Mutex::new(HashMap::new()),
            confirmed_iterated_pairs_size: AtomicUsize::new(0),
            pending_writes_size: AtomicUsize::new(0),
            implicit_receive_cemented_mapping_size: AtomicUsize::new(0),
            timer: Instant::now(),
            batch_separate_pending_min_time,
            notify_observers_callback,
        }
    }

    pub fn pending_empty(&self) -> bool {
        self.pending_writes.is_empty()
    }

    pub fn add_confirmed_iterated_pair(
        &mut self,
        account: Account,
        confirmed_height: u64,
        iterated_height: u64,
    ) {
        self.confirmed_iterated_pairs.insert(
            account,
            ConfirmedIteratedPair {
                confirmed_height,
                iterated_height,
            },
        );
        self.confirmed_iterated_pairs_size
            .fetch_add(1, Ordering::Relaxed);
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

    pub fn add_implicit_receive_cemented(
        &mut self,
        hash: BlockHash,
        details: &Arc<Mutex<ConfHeightDetails>>,
    ) {
        let details = Arc::downgrade(&details);
        self.implicit_receive_cemented_mapping.insert(hash, details);
        self.implicit_receive_cemented_mapping_size.store(
            self.implicit_receive_cemented_mapping.len(),
            Ordering::Relaxed,
        );
    }

    pub fn get_implicit_receive_cemented(
        &self,
        hash: &BlockHash,
    ) -> Option<&Weak<Mutex<ConfHeightDetails>>> {
        self.implicit_receive_cemented_mapping.get(hash)
    }

    pub fn cache_block(&self, block: Arc<BlockEnum>) {
        self.block_cache.lock().unwrap().insert(block.hash(), block);
    }

    pub fn get_blocks(&self, details: &ConfHeightDetails) -> Vec<Arc<BlockEnum>> {
        let cache = self.block_cache.lock().unwrap();
        details
            .block_callback_data
            .iter()
            .map(|hash| Arc::clone(cache.get(hash).unwrap()))
            .collect()
    }

    pub fn get_block_and_sideband(
        &self,
        hash: &BlockHash,
        txn: &dyn Transaction,
    ) -> Arc<BlockEnum> {
        let mut cache = self.block_cache.lock().unwrap();
        match cache.get(hash) {
            Some(block) => Arc::clone(block),
            None => {
                let block = self.ledger.get_block(txn, hash).unwrap(); //todo: remove unwrap
                let block = Arc::new(block);
                cache.insert(*hash, Arc::clone(&block));
                block
            }
        }
    }

    pub fn has_iterated_over_block(&self, hash: &BlockHash) -> bool {
        self.block_cache.lock().unwrap().contains_key(hash)
    }

    pub fn block_cache_size(&self) -> usize {
        self.block_cache.lock().unwrap().len()
    }

    pub fn restart_timer(&mut self) {
        self.timer = Instant::now();
    }

    pub fn min_time_exceeded(&self) -> bool {
        self.timer.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn clear_process_vars(&mut self) {
        // Separate blocks which are pending confirmation height can be batched by a minimum processing time (to improve lmdb disk write performance),
        // so make sure the slate is clean when a new batch is starting.
        self.confirmed_iterated_pairs.clear();
        self.confirmed_iterated_pairs_size
            .store(0, Ordering::Relaxed);

        self.implicit_receive_cemented_mapping.clear();
        self.implicit_receive_cemented_mapping_size
            .store(0, Ordering::Relaxed);

        self.block_cache.lock().unwrap().clear();
    }

    pub fn prepare_iterated_blocks_for_cementing(
        &mut self,
        preparation_data_a: &mut PreparationData,
    ) {
        let receive_details = &preparation_data_a.receive_details;
        let block_height = preparation_data_a.block_height;
        if block_height > preparation_data_a.confirmation_height {
            // Check whether the previous block has been seen. If so, the rest of sends below have already been seen so don't count them
            if let Some((account, _)) = &preparation_data_a.account_it {
                let pair = self.confirmed_iterated_pairs.get_mut(account).unwrap();
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
                            && receive_details_lock.source_block_callback_data.is_empty()
                        {
                            drop(receive_details_lock);
                            // We are confirming a block which has already been traversed and found no associated receive details for it.
                            let above_receive_details_w = self
                                .get_implicit_receive_cemented(&preparation_data_a.current)
                                .unwrap();
                            debug_assert!(above_receive_details_w.strong_count() > 0);
                            let above_receive_details = above_receive_details_w.upgrade().unwrap();
                            let above_receive_details_lock = above_receive_details.lock().unwrap();

                            let num_blocks_already_confirmed = above_receive_details_lock
                                .num_blocks_confirmed
                                - (above_receive_details_lock.height
                                    - preparation_data_a.confirmation_height);

                            let block_data = above_receive_details_lock.block_callback_data.clone();
                            drop(above_receive_details_lock);
                            let end = block_data.len() - (num_blocks_already_confirmed as usize);
                            let start = end - num_blocks_confirmed as usize;

                            block_callback_data.clear();
                            block_callback_data.extend_from_slice(&block_data[start..end]);

                            let num_to_remove =
                                block_callback_data.len() - num_blocks_confirmed as usize;
                            block_callback_data.truncate(block_callback_data.len() - num_to_remove);
                            receive_details_lock = receive_details.lock().unwrap();
                            receive_details_lock.source_block_callback_data.clear();

                            let details = ConfHeightDetails {
                                account: preparation_data_a.account,
                                hash: preparation_data_a.current,
                                height: block_height,
                                num_blocks_confirmed,
                                block_callback_data,
                                source_block_callback_data: Vec::new(),
                            };
                            self.add_pending_write(details);
                        } else {
                            block_callback_data =
                                receive_details_lock.source_block_callback_data.clone();

                            let num_to_remove =
                                block_callback_data.len() - num_blocks_confirmed as usize;
                            block_callback_data.truncate(block_callback_data.len() - num_to_remove);
                            // receive_details_lock = receive_details.lock().unwrap();
                            receive_details_lock.source_block_callback_data.clear();

                            let details = ConfHeightDetails {
                                account: preparation_data_a.account,
                                hash: preparation_data_a.current,
                                height: block_height,
                                num_blocks_confirmed,
                                block_callback_data,
                                source_block_callback_data: Vec::new(),
                            };
                            self.add_pending_write(details);
                        }
                    }
                    None => {
                        block_callback_data = preparation_data_a.orig_block_callback_data.clone();

                        let details = ConfHeightDetails {
                            account: preparation_data_a.account,
                            hash: preparation_data_a.current,
                            height: block_height,
                            num_blocks_confirmed,
                            block_callback_data,
                            source_block_callback_data: Vec::new(),
                        };
                        self.add_pending_write(details);
                    }
                }
            } else {
                let details = ConfHeightDetails {
                    account: preparation_data_a.account,
                    hash: preparation_data_a.current,
                    height: block_height,
                    num_blocks_confirmed,
                    block_callback_data,
                    source_block_callback_data: Vec::new(),
                };
                self.add_pending_write(details);
            }
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
                    pair.confirmed_height = receive_details_lock.height;
                    let orig_num_blocks_confirmed = receive_details_lock.num_blocks_confirmed;
                    receive_details_lock.num_blocks_confirmed =
                        receive_details_lock.height - current_height;

                    // Get the difference and remove the callbacks
                    let block_callbacks_to_remove =
                        orig_num_blocks_confirmed - receive_details_lock.num_blocks_confirmed;
                    let mut tmp_blocks = receive_details_lock.block_callback_data.clone();
                    tmp_blocks.truncate(tmp_blocks.len() - block_callbacks_to_remove as usize);
                    receive_details_lock.block_callback_data = tmp_blocks;
                    debug_assert!(
                        receive_details_lock.block_callback_data.len()
                            == receive_details_lock.num_blocks_confirmed as usize
                    );
                }
                None => {
                    self.add_confirmed_iterated_pair(
                        receive_account,
                        receive_details_lock.height,
                        receive_details_lock.height,
                    );
                }
            }

            self.add_pending_write(receive_details_lock.clone())
        }
    }

    pub fn collect_unconfirmed_receive_and_sources_for_account(
        &self,
        block_height_a: u64,
        confirmation_height_a: u64,
        block_a: &BlockEnum,
        hash_a: &BlockHash,
        account_a: &Account,
        transaction_a: &dyn ReadTransaction,
        receive_source_pairs_a: &mut Vec<Arc<ReceiveSourcePair>>,
        block_callback_data_a: &mut Vec<BlockHash>,
        orig_block_callback_data_a: &mut Vec<BlockHash>,
        original_block: &BlockEnum,
    ) {
        // debug_assert (block_a->hash () == hash_a);
        // auto hash (hash_a);
        // auto num_to_confirm = block_height_a - confirmation_height_a;

        // // Handle any sends above a receive
        // auto is_original_block = (hash == original_block->hash ());
        // auto hit_receive = false;
        // auto first_iter = true;
        // while ((num_to_confirm > 0) && !hash.is_zero () && !stopped)
        // {
        // 	std::shared_ptr<nano::block> block;
        // 	if (first_iter)
        // 	{
        // 		debug_assert (hash == hash_a);
        // 		block = block_a;
        // 		rsnano::rsn_conf_height_unbounded_cache_block (handle, block_a->get_handle ());
        // 	}
        // 	else
        // 	{
        // 		block = get_block_and_sideband (hash, transaction_a);
        // 	}

        // 	if (block)
        // 	{
        // 		auto source (block->source ());
        // 		if (source.is_zero ())
        // 		{
        // 			source = block->link ().as_block_hash ();
        // 		}

        // 		if (!source.is_zero () && !ledger.is_epoch_link (source) && ledger.store.block ().exists (transaction_a, source))
        // 		{
        // 			if (!hit_receive && !block_callback_data_a.empty ())
        // 			{
        // 				// Add the callbacks to the associated receive to retrieve later
        // 				debug_assert (!receive_source_pairs_a.empty ());
        // 				auto last_receive_details = receive_source_pairs_a.back ().receive_details ();
        // 				last_receive_details.set_source_block_callback_data (block_callback_data_a);
        // 				block_callback_data_a.clear ();
        // 			}

        // 			is_original_block = false;
        // 			hit_receive = true;

        // 			auto block_height = confirmation_height_a + num_to_confirm;
        // 			nano::block_hash_vec callback_data{};
        // 			callback_data.push_back (hash);
        // 			conf_height_details details (account_a, hash, block_height, 1, callback_data);
        // 			auto shared_details = rsnano::rsn_conf_height_details_shared_ptr_create (details.handle);
        // 			receive_source_pairs_a.push (nano::confirmation_height_unbounded::receive_source_pair{ shared_details, source });
        // 		}
        // 		else if (is_original_block)
        // 		{
        // 			orig_block_callback_data_a.push_back (hash);
        // 		}
        // 		else
        // 		{
        // 			if (!hit_receive)
        // 			{
        // 				// This block is cemented via a recieve, as opposed to below a receive being cemented
        // 				block_callback_data_a.push_back (hash);
        // 			}
        // 			else
        // 			{
        // 				// We have hit a receive before, add the block to it
        // 				auto last_receive_details = receive_source_pairs_a.back ().receive_details ();
        // 				last_receive_details.set_num_blocks_confirmed (last_receive_details.get_num_blocks_confirmed () + 1);
        // 				last_receive_details.add_block_callback_data (hash);

        // 				rsnano::rsn_conf_height_unbounded_implicit_receive_cemented_mapping_add (handle, hash.bytes.data (), last_receive_details.handle);
        // 			}
        // 		}

        // 		hash = block->previous ();
        // 	}

        // 	--num_to_confirm;
        // 	first_iter = false;
        // }
    }

    pub fn cement_blocks(&mut self, scoped_write_guard_a: &mut WriteGuard) {
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

                    cemented_blocks.append(&mut self.get_blocks(&pending));
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

        scoped_write_guard_a.release();
        (self.notify_observers_callback)(&cemented_blocks);
        assert!(!error);

        debug_assert!(self.pending_writes.len() == 0);
        debug_assert!(self.pending_writes_size.load(Ordering::Relaxed) == 0);
        self.restart_timer();
    }
}

#[derive(Clone, Debug)]
pub struct ConfHeightDetails {
    pub account: Account,
    pub hash: BlockHash,
    pub height: u64,
    pub num_blocks_confirmed: u64,
    pub block_callback_data: Vec<BlockHash>,
    pub source_block_callback_data: Vec<BlockHash>,
}

pub struct ConfirmedIteratedPair {
    pub confirmed_height: u64,
    pub iterated_height: u64,
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
    pub account_it: Option<(Account, ConfirmedIteratedPair)>,
    pub account: Account,
    pub receive_details: Option<Arc<Mutex<ConfHeightDetails>>>,
    pub already_traversed: bool,
    pub current: BlockHash,
    pub block_callback_data: &'a mut Vec<BlockHash>,
    pub orig_block_callback_data: &'a mut Vec<BlockHash>,
}
