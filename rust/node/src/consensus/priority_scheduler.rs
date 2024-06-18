use super::{ActiveElections, Buckets, ElectionBehavior};
use crate::{
    consensus::ActiveElectionsExt,
    stats::{DetailType, StatType, Stats},
};
use rsnano_core::{
    utils::{seconds_since_epoch, ContainerInfoComponent},
    Account, BlockEnum, QualifiedRoot, Root,
};
use rsnano_ledger::Ledger;
use rsnano_store_lmdb::{LmdbReadTransaction, Transaction};
use std::{
    cmp::max,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};
use tracing::trace;

pub struct PriorityScheduler {
    thread: Mutex<Option<JoinHandle<()>>>,
    mutex: Mutex<PrioritySchedulerImpl>,
    condition: Condvar,
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    active: Arc<ActiveElections>,
}

impl PriorityScheduler {
    pub fn new(ledger: Arc<Ledger>, stats: Arc<Stats>, active: Arc<ActiveElections>) -> Self {
        Self {
            thread: Mutex::new(None),
            mutex: Mutex::new(PrioritySchedulerImpl {
                stopped: false,
                buckets: Buckets::default(),
            }),
            condition: Condvar::new(),
            ledger,
            stats,
            active,
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.notify();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }

    pub fn activate(&self, tx: &dyn Transaction, account: &Account) -> bool {
        debug_assert!(!account.is_zero());

        let head = self
            .ledger
            .confirmed()
            .account_head(tx, account)
            .unwrap_or_default();
        if self
            .ledger
            .any()
            .account_head(tx, account)
            .unwrap_or_default()
            == head
        {
            return false;
        }

        let root = if head.is_zero() {
            Root::from(account)
        } else {
            head.into()
        };

        let successor = self
            .ledger
            .any()
            .block_successor_by_qualified_root(tx, &QualifiedRoot::new(root, head))
            .unwrap();

        let block = self.ledger.any().get_block(tx, &successor).unwrap();

        if !self.ledger.dependents_confirmed(tx, &block) {
            return false;
        }

        let previous_balance = self
            .ledger
            .confirmed()
            .block_balance(tx, &head)
            .unwrap_or_default();
        let balance_priority = max(block.balance(), previous_balance);

        let time_priority = if !head.is_zero() {
            self.ledger
                .confirmed()
                .get_block(tx, &head)
                .unwrap()
                .sideband()
                .unwrap()
                .timestamp
        } else {
            // New accounts get current timestamp i.e. lowest priority
            seconds_since_epoch()
        };

        self.stats
            .inc(StatType::ElectionScheduler, DetailType::Activated);

        trace!(
            account = account.encode_account(),
            ?block,
            time = time_priority,
            priority = balance_priority.number(),
            "priority scheduler activated"
        );

        let mut guard = self.mutex.lock().unwrap();
        guard
            .buckets
            .push(time_priority, Arc::new(block), balance_priority);
        self.notify();

        true // Activated
    }

    pub fn notify(&self) {
        self.condition.notify_all();
    }

    pub fn len(&self) -> usize {
        self.mutex.lock().unwrap().buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mutex.lock().unwrap().buckets.is_empty()
    }

    fn predicate(&self, buckets: &Buckets) -> bool {
        self.active.vacancy(ElectionBehavior::Priority) > 0 && !buckets.is_empty()
    }

    pub fn run(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !guard.stopped {
            guard = self
                .condition
                .wait_while(guard, |i| !i.stopped && !self.predicate(&i.buckets))
                .unwrap();
            if !guard.stopped {
                self.stats
                    .inc(StatType::ElectionScheduler, DetailType::Loop);

                if self.predicate(&guard.buckets) {
                    let block = Arc::clone(guard.buckets.top());
                    guard.buckets.pop();
                    drop(guard);
                    self.stats
                        .inc(StatType::ElectionScheduler, DetailType::InsertPriority);
                    let (inserted, election) =
                        self.active.insert(&block, ElectionBehavior::Priority);
                    if inserted {
                        self.stats.inc(
                            StatType::ElectionScheduler,
                            DetailType::InsertPrioritySuccess,
                        );
                    }
                    if let Some(election) = election {
                        election.transition_active();
                    }
                } else {
                    drop(guard);
                }
                self.notify();
                guard = self.mutex.lock().unwrap();
            }
        }
    }

    pub fn activate_successors(&self, tx: &LmdbReadTransaction, block: &BlockEnum) {
        self.activate(tx, &block.account());

        // Start or vote for the next unconfirmed block in the destination account
        if let Some(destination) = block.destination() {
            if block.is_send() && !destination.is_zero() && destination != block.account() {
                self.activate(tx, &destination);
            }
        }
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        let guard = self.mutex.lock().unwrap();
        ContainerInfoComponent::Composite(
            name.into(),
            vec![guard.buckets.collect_container_info("buckets")],
        )
    }
}

impl Drop for PriorityScheduler {
    fn drop(&mut self) {
        // Thread must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
    }
}

pub trait PrioritySchedulerExt {
    fn start(&self);
}

impl PrioritySchedulerExt for Arc<PriorityScheduler> {
    fn start(&self) {
        debug_assert!(self.thread.lock().unwrap().is_none());
        let self_l = Arc::clone(&self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Sched Priority".to_string())
                .spawn(Box::new(move || {
                    self_l.run();
                }))
                .unwrap(),
        );
    }
}

struct PrioritySchedulerImpl {
    stopped: bool,
    buckets: Buckets,
}
