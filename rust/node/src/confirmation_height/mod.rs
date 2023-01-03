use rsnano_core::{Account, BlockEnum, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::{Duration, Instant},
};

pub struct ConfirmationHeightUnbounded {
    ledger: Arc<Ledger>,
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
        batch_separate_pending_min_time: Duration,
        notify_observers_callback: Box<dyn Fn(&Vec<Arc<BlockEnum>>)>,
    ) -> Self {
        Self {
            ledger,
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
}

#[derive(Clone)]
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
