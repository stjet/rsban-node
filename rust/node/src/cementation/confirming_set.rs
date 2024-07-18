use super::{BatchCementedCallback, BlockCallback, BlockHashCallback};
use crate::{
    stats::{DetailType, StatType, Stats},
    utils::{ThreadPool, ThreadPoolImpl},
};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockEnum, BlockHash,
};
use rsnano_ledger::{Ledger, Writer};
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

#[derive(Clone)]
pub struct ConfirmingSetConfig {
    /// Maximum number of dependent blocks to be stored in memory during processing
    pub max_blocks: usize,
    pub max_queued_notifications: usize,
}

impl Default for ConfirmingSetConfig {
    fn default() -> Self {
        Self {
            max_blocks: 64 * 128,
            max_queued_notifications: 8,
        }
    }
}

/// Set of blocks to be durably confirmed
pub struct ConfirmingSet {
    thread: Arc<ConfirmingSetThread>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl ConfirmingSet {
    pub fn new(ledger: Arc<Ledger>, stats: Arc<Stats>) -> Self {
        Self {
            join_handle: Mutex::new(None),
            thread: Arc::new(ConfirmingSetThread {
                mutex: Mutex::new(ConfirmingSetImpl {
                    stopped: false,
                    set: HashSet::new(),
                }),
                condition: Condvar::new(),
                ledger,
                stats,
                observers: Arc::new(Mutex::new(Observers::default())),
                notification_workers: ThreadPoolImpl::create(1, "Conf notif"),
            }),
        }
    }

    pub(crate) fn add_batch_cemented_observer(&self, callback: BatchCementedCallback) {
        self.thread
            .observers
            .lock()
            .unwrap()
            .batch_cemented
            .push(callback);
    }

    pub fn add_cemented_observer(&self, callback: BlockCallback) {
        self.thread
            .observers
            .lock()
            .unwrap()
            .cemented
            .push(callback);
    }

    pub fn add_already_cemented_observer(&self, callback: BlockHashCallback) {
        self.thread
            .observers
            .lock()
            .unwrap()
            .already_cemented
            .push(callback);
    }

    /// Adds a block to the set of blocks to be confirmed
    pub fn add(&self, hash: BlockHash) {
        self.thread.add(hash);
    }

    pub fn start(&self) {
        debug_assert!(self.join_handle.lock().unwrap().is_none());

        let thread = Arc::clone(&self.thread);
        *self.join_handle.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Conf height".to_string())
                .spawn(move || thread.run())
                .unwrap(),
        );
    }

    pub fn stop(&self) {
        self.thread.stop();
        if let Some(handle) = self.join_handle.lock().unwrap().take() {
            handle.join().unwrap();
        }
        self.thread.notification_workers.stop();
    }

    /// Added blocks will remain in this set until after ledger has them marked as confirmed.
    pub fn exists(&self, hash: &BlockHash) -> bool {
        self.thread.exists(hash)
    }

    pub fn len(&self) -> usize {
        self.thread.len()
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.thread.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "set".to_string(),
                count: guard.set.len(),
                sizeof_element: std::mem::size_of::<BlockHash>(),
            })],
        )
    }
}

impl Drop for ConfirmingSet {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct ConfirmingSetThread {
    mutex: Mutex<ConfirmingSetImpl>,
    condition: Condvar,
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    notification_workers: ThreadPoolImpl,
    observers: Arc<Mutex<Observers>>,
}

impl ConfirmingSetThread {
    fn stop(&self) {
        {
            let mut guard = self.mutex.lock().unwrap();
            guard.stopped = true;
        }
        self.condition.notify_all();
    }

    fn add(&self, hash: BlockHash) {
        let added = {
            let mut guard = self.mutex.lock().unwrap();
            guard.set.insert(hash)
        };

        if added {
            self.condition.notify_all();
            self.stats.inc(StatType::ConfirmingSet, DetailType::Insert);
        } else {
            self.stats
                .inc(StatType::ConfirmingSet, DetailType::Duplicate);
        }
    }

    fn exists(&self, hash: &BlockHash) -> bool {
        self.mutex.lock().unwrap().set.contains(hash)
    }

    fn len(&self) -> usize {
        self.mutex.lock().unwrap().set.len()
    }

    fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            if !guard.set.is_empty() {
                let batch = guard.next_batch(256);
                drop(guard);
                self.run_batch(batch);
                guard = self.mutex.lock().unwrap();
            } else {
                guard = self
                    .condition
                    .wait_while(guard, |i| i.set.is_empty() && !i.stopped)
                    .unwrap();
            }
        }
    }

    fn run_batch(&self, batch: VecDeque<BlockHash>) {
        let mut cemented = VecDeque::new();
        let mut already_cemented = VecDeque::new();

        {
            let mut write_guard = self.ledger.write_queue.wait(Writer::ConfirmationHeight);
            let mut tx = self.ledger.rw_txn();

            for hash in batch {
                (write_guard, tx) = self.ledger.refresh_if_needed(write_guard, tx);
                let added = self.ledger.confirm(&mut tx, hash);
                let added_len = added.len();
                if !added.is_empty() {
                    // Confirming this block may implicitly confirm more
                    for block in added {
                        cemented.push_back((block, hash));
                    }

                    self.stats.add(
                        StatType::ConfirmingSet,
                        DetailType::Cemented,
                        added_len as u64,
                    );
                } else {
                    already_cemented.push_back(hash);
                    self.stats
                        .inc(StatType::ConfirmingSet, DetailType::AlreadyCemented);
                }
            }
        }

        let notification = CementedNotification {
            cemented,
            already_cemented,
        };

        let observers = self.observers.clone();
        self.notification_workers.push_task(Box::new(move || {
            observers.lock().unwrap().notify_batch(notification);
        }));
    }
}

struct ConfirmingSetImpl {
    stopped: bool,
    set: HashSet<BlockHash>,
}

impl ConfirmingSetImpl {
    fn next_batch(&mut self, max_count: usize) -> VecDeque<BlockHash> {
        let mut results = VecDeque::new();
        // TODO: use extract_if once it is stablized
        while let Some(&hash) = self.set.iter().next() {
            if results.len() >= max_count {
                break;
            }
            results.push_back(hash);
            self.set.remove(&hash);
        }
        results
    }
}

pub(crate) struct CementedNotification {
    pub cemented: VecDeque<(BlockEnum, BlockHash)>, // block + confirmation root
    pub already_cemented: VecDeque<BlockHash>,
}

#[derive(Default)]
struct Observers {
    cemented: Vec<BlockCallback>,
    already_cemented: Vec<BlockHashCallback>,
    batch_cemented: Vec<BatchCementedCallback>,
}

impl Observers {
    fn notify_batch(&mut self, notification: CementedNotification) {
        for (block, _) in &notification.cemented {
            for observer in &mut self.cemented {
                observer(&Arc::new(block.clone()));
            }
        }

        for hash in &notification.already_cemented {
            for observer in &mut self.already_cemented {
                observer(*hash);
            }
        }

        for observer in &mut self.batch_cemented {
            observer(&notification);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use rsnano_core::{ConfirmationHeightInfo, TestAccountChain};

    #[test]
    fn add_exists() {
        let ledger = Arc::new(Ledger::new_null());
        let confirming_set = ConfirmingSet::new(ledger, Arc::new(Stats::default()));
        let hash = BlockHash::from(1);
        confirming_set.add(hash);
        assert!(confirming_set.exists(&hash));
    }

    #[test]
    fn process_one() {
        let mut chain = TestAccountChain::genesis();
        let block_hash = chain.add_state().hash();
        let ledger = Arc::new(
            Ledger::new_null_builder()
                .blocks(chain.blocks())
                .confirmation_height(
                    &chain.account(),
                    &ConfirmationHeightInfo {
                        height: 1,
                        frontier: chain.open(),
                    },
                )
                .finish(),
        );
        let confirming_set = ConfirmingSet::new(ledger, Arc::new(Stats::default()));
        confirming_set.start();
        let count = Arc::new(Mutex::new(0));
        let condition = Arc::new(Condvar::new());
        let count_clone = Arc::clone(&count);
        let condition_clone = Arc::clone(&condition);
        confirming_set.add_cemented_observer(Box::new(move |_block| {
            {
                *count_clone.lock().unwrap() += 1;
            }
            condition_clone.notify_all();
        }));

        confirming_set.add(block_hash);

        let guard = count.lock().unwrap();
        let result = condition
            .wait_timeout_while(guard, Duration::from_secs(5), |i| *i != 1)
            .unwrap()
            .1;
        assert_eq!(result.timed_out(), false);
    }
}
