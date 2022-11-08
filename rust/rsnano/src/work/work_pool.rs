use std::{
    cmp::min,
    mem::{size_of, size_of_val},
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc, Condvar, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use blake2::{
    digest::{Update, VariableOutput},
    VarBlake2b,
};

use crate::{
    config::NetworkConstants,
    core::{Root, WorkVersion},
    utils::get_cpu_count,
};

use super::XorShift1024Star;

static NEVER_EXPIRES: AtomicI32 = AtomicI32::new(0);

struct WorkItem {
    version: WorkVersion,
    item: Root,
    difficulty: u64,
    callback: Option<Box<dyn Fn(Option<u64>) + Send>>,
}

#[derive(Clone)]
pub struct WorkTicket<'a> {
    ticket: &'a AtomicI32,
    ticket_copy: i32,
}

impl<'a> WorkTicket<'a> {
    pub fn never_expires() -> Self {
        Self::new(&NEVER_EXPIRES)
    }

    pub fn new(ticket: &'a AtomicI32) -> Self {
        Self {
            ticket,
            ticket_copy: ticket.load(Ordering::SeqCst),
        }
    }

    pub fn expired(&self) -> bool {
        self.ticket_copy != self.ticket.load(Ordering::SeqCst)
    }
}

struct WorkPoolSharedState {
    opencl: Option<Box<dyn Fn(WorkVersion, Root, u64, &WorkTicket) -> Option<u64> + Send + Sync>>,
    network_constants: NetworkConstants,
    mutable_state: Mutex<WorkPoolMutableState>,
    producer_condition: Condvar,
    ticket: AtomicI32,
    pow_rate_limiter: Duration,
}

impl WorkPoolSharedState {
    pub fn create_work_ticket(&'_ self) -> WorkTicket<'_> {
        WorkTicket::new(&self.ticket)
    }

    pub fn has_opencl(&self) -> bool {
        self.opencl.is_some()
    }

    pub fn expire_tickets(&self) {
        self.ticket.fetch_add(1, Ordering::SeqCst);
    }
}

struct WorkPoolMutableState {
    pending: Vec<WorkItem>,
    done: bool,
}

pub struct WorkPool {
    max_threads: u32,
    threads: Vec<JoinHandle<()>>,
    shared_state: Arc<WorkPoolSharedState>,
}

impl WorkPool {
    pub fn new(
        network_constants: NetworkConstants,
        max_threads: u32,
        pow_rate_limiter: Duration,
        opencl: Option<
            Box<dyn Fn(WorkVersion, Root, u64, &WorkTicket) -> Option<u64> + Send + Sync>,
        >,
    ) -> Self {
        let mut count = if network_constants.is_dev_network() {
            min(max_threads, 1)
        } else {
            min(max_threads, get_cpu_count() as u32)
        };
        if opencl.is_some() {
            // One thread to handle OpenCL
            count += 1;
        }

        let mutable_state = WorkPoolMutableState {
            pending: Vec::new(),
            done: false,
        };

        let shared_state = Arc::new(WorkPoolSharedState {
            opencl,
            network_constants,
            mutable_state: Mutex::new(mutable_state),
            producer_condition: Condvar::new(),
            ticket: AtomicI32::new(0),
            pow_rate_limiter,
        });

        let mut threads = Vec::new();
        for i in 0..count {
            let state = Arc::clone(&shared_state);
            threads.push(
                thread::Builder::new()
                    .name("Work pool".to_string())
                    .spawn(move || {
                        work_loop(i, state);
                    })
                    .unwrap(),
            )
        }

        Self {
            max_threads,
            threads,
            shared_state,
        }
    }

    pub fn create_work_ticket(&'_ self) -> WorkTicket<'_> {
        self.shared_state.create_work_ticket()
    }

    pub fn expire_tickets(&self) {
        self.shared_state.expire_tickets();
    }

    pub fn call_open_cl(
        &self,
        version: WorkVersion,
        root: Root,
        difficulty: u64,
        ticket: &WorkTicket,
    ) -> Option<u64> {
        match &self.shared_state.opencl {
            Some(callback) => callback(version, root, difficulty, ticket),
            None => None,
        }
    }

    pub fn has_opencl(&self) -> bool {
        self.shared_state.opencl.is_some()
    }

    pub fn cancel(&self, root: &Root) {
        let mut lock = self.shared_state.mutable_state.lock().unwrap();
        if !lock.done {
            if let Some(front) = lock.pending.first() {
                if front.item == *root {
                    self.shared_state.expire_tickets();
                }
            }

            lock.pending.retain(|item| {
                let retain = item.item != *root;
                if !retain {
                    if let Some(callback) = &item.callback {
                        (callback)(None);
                    }
                }
                retain
            });
        }
    }

    pub fn stop(&self) {
        {
            let mut lock = self.shared_state.mutable_state.lock().unwrap();
            lock.done = true;
            self.shared_state.expire_tickets();
        }
        self.shared_state.producer_condition.notify_all();
    }

    pub fn generate_async(
        &self,
        version: WorkVersion,
        root: Root,
        difficulty: u64,
        done: Option<Box<dyn Fn(Option<u64>) + Send>>,
    ) {
        debug_assert!(!root.is_zero());
        if !self.threads.is_empty() {
            {
                let mut lock = self.shared_state.mutable_state.lock().unwrap();
                lock.pending.push(WorkItem {
                    version,
                    item: root,
                    difficulty,
                    callback: done,
                })
            }
            self.shared_state.producer_condition.notify_all();
        } else if let Some(callback) = done {
            callback(None);
        }
    }

    pub fn generate_dev(&self, root: Root, difficulty: u64) -> Option<u64> {
        debug_assert!(self.shared_state.network_constants.is_dev_network());
        self.generate(WorkVersion::Work1, root, difficulty)
    }

    pub fn generate_dev2(&self, root: Root) -> Option<u64> {
        debug_assert!(self.shared_state.network_constants.is_dev_network());
        self.generate(
            WorkVersion::Work1,
            root,
            self.shared_state.network_constants.work.base,
        )
    }

    pub fn generate(&self, version: WorkVersion, root: Root, difficulty: u64) -> Option<u64> {
        let done = Arc::new((Mutex::new((None, false)), Condvar::new()));
        let done_clone = Arc::clone(&done);
        if !self.threads.is_empty() {
            self.generate_async(
                version,
                root,
                difficulty,
                Some(Box::new(move |work| {
                    {
                        let mut lock = done_clone.0.lock().unwrap();
                        lock.0 = work;
                        lock.1 = true;
                    }
                    done_clone.1.notify_one();
                })),
            );

            let mut lock = done.0.lock().unwrap();
            loop {
                lock = done.1.wait(lock).unwrap();
                let done = lock.1;
                if done {
                    return lock.0;
                }
            }
        }

        None
    }

    pub fn size(&self) -> usize {
        let lock = self.shared_state.mutable_state.lock().unwrap();
        lock.pending.len()
    }

    pub fn pending_value_size() -> usize {
        size_of::<WorkItem>()
    }

    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    pub fn threshold_base(&self, version: WorkVersion) -> u64 {
        self.shared_state
            .network_constants
            .work
            .threshold_base(version)
    }

    pub fn difficulty(&self, version: WorkVersion, root: &Root, work: u64) -> u64 {
        self.shared_state
            .network_constants
            .work
            .difficulty(version, root, work)
    }
}

impl Drop for WorkPool {
    fn drop(&mut self) {
        self.stop();
        for handle in self.threads.drain(..) {
            handle.join().unwrap();
        }
    }
}

fn work_loop(thread: u32, state: Arc<WorkPoolSharedState>) {
    // Quick RNG for work attempts.
    let mut rng = XorShift1024Star::new();
    let mut work = 0;
    let mut output = 0;
    let mut hasher = VarBlake2b::new_keyed(&[], size_of_val(&output));
    let mut lock = state.mutable_state.lock().unwrap();
    while !lock.done {
        let empty = lock.pending.is_empty();
        if !empty {
            let current_l = lock.pending.first().unwrap().clone();
            let current_version = current_l.version;
            let current_item = current_l.item;
            let current_difficulty = current_l.difficulty;
            let ticket_l = state.create_work_ticket();
            drop(lock);
            output = 0;
            let mut opt_work = None;
            if thread == 0 && state.has_opencl() {
                opt_work = (state.opencl.as_ref().unwrap())(
                    current_version,
                    current_item,
                    current_difficulty,
                    &ticket_l,
                );
            }
            if let Some(w) = opt_work {
                work = w;
                output = state.network_constants.work.value(&current_item, work);
            } else {
                while !ticket_l.expired() && output < current_difficulty {
                    // Don't query main memory every iteration in order to reduce memory bus traffic
                    // All operations here operate on stack memory
                    // Count iterations down to zero since comparing to zero is easier than comparing to another number
                    let mut iteration = 256u32;
                    while iteration > 0 && output < current_difficulty {
                        work = rng.next();
                        hasher.update(&work.to_le_bytes());
                        hasher.update(current_item.as_bytes());
                        hasher.finalize_variable_reset(|result| {
                            output = u64::from_le_bytes(result.try_into().unwrap());
                        });
                        iteration -= 1;
                    }

                    // Add a rate limiter (if specified) to the pow calculation to save some CPUs which don't want to operate at full throttle
                    if !state.pow_rate_limiter.is_zero() {
                        thread::sleep(state.pow_rate_limiter);
                    }
                }
            }
            lock = state.mutable_state.lock().unwrap();
            if !ticket_l.expired() {
                // If the ticket matches what we started with, we're the ones that found the solution
                debug_assert!(output >= current_difficulty);
                debug_assert!(
                    current_difficulty == 0
                        || state.network_constants.work.value(&current_item, work) == output
                );
                // Signal other threads to stop their work next time they check ticket
                state.expire_tickets();
                let current_l = lock.pending.remove(0);
                drop(lock);
                if let Some(callback) = current_l.callback {
                    (callback)(Some(work));
                }
                lock = state.mutable_state.lock().unwrap();
            } else {
                // A different thread found a solution
            }
        } else {
            // Wait for a work request
            lock = state.producer_condition.wait(lock).unwrap();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use crate::{
        core::{Block, BlockBuilder},
        DEV_NETWORK_PARAMS,
    };

    use super::*;

    #[test]
    fn work_disabled() {
        let pool = WorkPool::new(DEV_NETWORK_PARAMS.network.clone(), 0, Duration::ZERO, None);
        let result = pool.generate_dev2(Root::from(1));
        assert_eq!(result, None);
    }

    #[test]
    fn work_one() {
        let pool = create_dev_work_pool();
        let mut block = BlockBuilder::state().build().unwrap();
        block.set_work(pool.generate_dev2(block.root()).unwrap());
        assert!(pool.threshold_base(block.work_version()) < difficulty(&block));
    }

    #[test]
    fn work_validate() {
        let pool = create_dev_work_pool();
        let mut block = BlockBuilder::send().work(6).build().unwrap();
        assert!(difficulty(&block) < pool.threshold_base(block.work_version()));
        block.set_work(pool.generate_dev2(block.root()).unwrap());
        assert!(difficulty(&block) > pool.threshold_base(block.work_version()));
    }

    #[test]
    fn work_cancel() {
        let pool = create_dev_work_pool();
        let (tx, rx) = mpsc::channel();
        let key = Root::from(1);
        pool.generate_async(
            WorkVersion::Work1,
            key,
            DEV_NETWORK_PARAMS.network.work.base,
            Some(Box::new(move |_done| {
                tx.send(()).unwrap();
            })),
        );
        pool.cancel(&key);
        assert_eq!(rx.recv_timeout(Duration::from_secs(2)), Ok(()))
    }

    #[test]
    fn cancel_many() {
        let pool = create_dev_work_pool();
        let key1 = Root::from(1);
        let key2 = Root::from(2);
        let key3 = Root::from(1);
        let key4 = Root::from(1);
        let key5 = Root::from(3);
        let key6 = Root::from(1);
        let base = DEV_NETWORK_PARAMS.network.work.base;
        pool.generate_async(WorkVersion::Work1, key1, base, None);
        pool.generate_async(WorkVersion::Work1, key2, base, None);
        pool.generate_async(WorkVersion::Work1, key3, base, None);
        pool.generate_async(WorkVersion::Work1, key4, base, None);
        pool.generate_async(WorkVersion::Work1, key5, base, None);
        pool.generate_async(WorkVersion::Work1, key6, base, None);
        pool.cancel(&key1);
    }

    #[test]
    fn work_difficulty() {
        let pool = create_dev_work_pool();
        let root = Root::from(1);
        let difficulty1 = 0xff00000000000000;
        let difficulty2 = 0xfff0000000000000;
        let difficulty3 = 0xffff000000000000;
        let mut result_difficulty = u64::MAX;

        while result_difficulty > difficulty2 {
            let work = pool
                .generate(WorkVersion::Work1, root, difficulty1)
                .unwrap();
            result_difficulty = DEV_NETWORK_PARAMS
                .work
                .difficulty(WorkVersion::Work1, &root, work);
        }
        assert!(result_difficulty > difficulty1);

        result_difficulty = u64::MAX;
        while result_difficulty > difficulty3 {
            let work = pool
                .generate(WorkVersion::Work1, root, difficulty2)
                .unwrap();
            result_difficulty = DEV_NETWORK_PARAMS
                .work
                .difficulty(WorkVersion::Work1, &root, work);
        }
        assert!(result_difficulty > difficulty2);
    }

    fn create_dev_work_pool() -> WorkPool {
        WorkPool::new(
            DEV_NETWORK_PARAMS.network.clone(),
            u32::MAX,
            Duration::ZERO,
            None,
        )
    }

    fn difficulty(block: &dyn Block) -> u64 {
        DEV_NETWORK_PARAMS.network.work.difficulty_block(block)
    }
}
