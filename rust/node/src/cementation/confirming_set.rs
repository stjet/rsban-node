use super::{BlockCallback, BlockHashCallback};
use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    BlockHash,
};
use rsnano_ledger::{Ledger, Writer};
use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Set of blocks to be durably confirmed
pub struct ConfirmingSet {
    thread: Arc<ConfirmingSetThread>,
    join_handle: Mutex<Option<JoinHandle<()>>>,
}

impl ConfirmingSet {
    pub fn new(ledger: Arc<Ledger>, batch_time: Duration) -> Self {
        Self {
            join_handle: Mutex::new(None),
            thread: Arc::new(ConfirmingSetThread {
                mutex: Mutex::new(ConfirmingSetImpl {
                    stopped: false,
                    set: HashSet::new(),
                    processing: HashSet::new(),
                }),
                condition: Condvar::new(),
                ledger,
                batch_time,
                cemented_observers: Mutex::new(Vec::new()),
                already_cemented_observers: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn add_cemented_observer(&self, callback: BlockCallback) {
        self.thread
            .cemented_observers
            .lock()
            .unwrap()
            .push(callback);
    }

    pub fn add_already_cemented_observer(&self, callback: BlockHashCallback) {
        self.thread
            .already_cemented_observers
            .lock()
            .unwrap()
            .push(callback);
    }

    /// Adds a block to the set of blocks to be confirmed
    pub fn add(&self, hash: BlockHash) {
        self.thread.add(hash);
    }

    pub fn start(&self) {
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
            vec![
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "set".to_string(),
                    count: guard.set.len(),
                    sizeof_element: std::mem::size_of::<BlockHash>(),
                }),
                ContainerInfoComponent::Leaf(ContainerInfo {
                    name: "processing".to_string(),
                    count: guard.processing.len(),
                    sizeof_element: std::mem::size_of::<BlockHash>(),
                }),
            ],
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
    batch_time: Duration,
    cemented_observers: Mutex<Vec<BlockCallback>>,
    already_cemented_observers: Mutex<Vec<BlockHashCallback>>,
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
        {
            let mut guard = self.mutex.lock().unwrap();
            guard.set.insert(hash);
        }
        self.condition.notify_all();
    }

    fn exists(&self, hash: &BlockHash) -> bool {
        let guard = self.mutex.lock().unwrap();
        guard.set.contains(hash) || guard.processing.contains(hash)
    }

    fn len(&self) -> usize {
        let guard = self.mutex.lock().unwrap();
        guard.set.len() + guard.processing.len()
    }

    fn run(&self) {
        let mut processing = Vec::new();
        let mut guard = self.mutex.lock().unwrap();
        // Run the confirmation loop until stopped
        while !guard.stopped {
            guard = self
                .condition
                .wait_while(guard, |i| i.set.is_empty() && !i.stopped)
                .unwrap();
            // Loop if there are items to process
            if !guard.stopped && !guard.set.is_empty() {
                let mut cemented = VecDeque::new();
                let mut already = VecDeque::new();
                // Move items in to back buffer and release lock so more items can be added to the front buffer
                guard.swap_processing_and_set();
                // Process all items in the back buffer
                processing.reserve(guard.processing.len());
                for i in &guard.processing {
                    processing.push(*i);
                }

                while !processing.is_empty() && !guard.stopped {
                    drop(guard); // Waiting for db write is potentially slow
                    let _write_guard = self.ledger.write_queue.wait(Writer::ConfirmationHeight);
                    let mut tx = self.ledger.rw_txn();
                    guard = self.mutex.lock().unwrap();
                    // Process items in the back buffer within a single transaction for a limited amount of time
                    let start = Instant::now();
                    while let Some(item) = processing.pop() {
                        if start.elapsed() >= self.batch_time || guard.stopped {
                            break;
                        }
                        drop(guard);
                        let added = self.ledger.confirm(&mut tx, item);
                        if !added.is_empty() {
                            // Confirming this block may implicitly confirm more
                            cemented.extend(added);
                        } else {
                            already.push_back(item);
                        }
                        guard = self.mutex.lock().unwrap();
                    }
                }

                drop(guard);
                for i in cemented {
                    let mut observers = self.cemented_observers.lock().unwrap();
                    for observer in observers.iter_mut() {
                        observer(&Arc::new(i.clone()));
                    }
                }
                for i in already {
                    let mut observers = self.already_cemented_observers.lock().unwrap();
                    for observer in observers.iter_mut() {
                        observer(i);
                    }
                }
                guard = self.mutex.lock().unwrap();
                // Clear and free back buffer by re-initializing
                processing.clear();
                guard.processing.clear();
            }
        }
    }
}

struct ConfirmingSetImpl {
    stopped: bool,
    set: HashSet<BlockHash>,
    processing: HashSet<BlockHash>,
}

impl ConfirmingSetImpl {
    fn swap_processing_and_set(&mut self) {
        std::mem::swap(&mut self.set, &mut self.processing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{ConfirmationHeightInfo, TestAccountChain};

    #[test]
    fn add_exists() {
        let ledger = Arc::new(Ledger::new_null());
        let confirming_set = ConfirmingSet::new(ledger, Duration::from_millis(500));
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
        let confirming_set = ConfirmingSet::new(ledger, Duration::from_millis(500));
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
