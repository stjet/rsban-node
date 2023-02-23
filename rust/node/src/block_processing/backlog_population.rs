use std::{
    ops::Deref,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::stats::{DetailType, Direction, StatType, Stats};
use primitive_types::U256;
use rsnano_core::{Account, AccountInfo, ConfirmationHeightInfo};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

#[derive(Clone)]
pub struct BacklogPopulationConfig {
    /** Control if ongoing backlog population is enabled. If not, backlog population can still be triggered by RPC */
    pub enabled: bool,

    /** Number of accounts per second to process. Number of accounts per single batch is this value divided by `frequency` */
    pub batch_size: u32,

    /** Number of batches to run per second. Batches run in 1 second / `frequency` intervals */
    pub frequency: u32,
}

struct BacklogPopulationFlags {
    stopped: bool,
    /** This is a manual trigger, the ongoing backlog population does not use this.
     *  It can be triggered even when backlog population (frontiers confirmation) is disabled. */
    triggered: bool,
}

pub struct BacklogPopulation {
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    /**
     * Callback called for each backlogged account
     */
    activate_callback: Arc<Mutex<Option<ActivateCallback>>>,
    config: BacklogPopulationConfig,
    mutex: Arc<Mutex<BacklogPopulationFlags>>,
    condition: Arc<Condvar>,
    /** Thread that runs the backlog implementation logic. The thread always runs, even if
     *  backlog population is disabled, so that it can service a manual trigger (e.g. via RPC). */
    thread: Option<JoinHandle<()>>,
}

pub type ActivateCallback =
    Box<dyn Fn(&dyn Transaction, &Account, &AccountInfo, &ConfirmationHeightInfo) + Send + Sync>;

impl BacklogPopulation {
    pub fn new(config: BacklogPopulationConfig, ledger: Arc<Ledger>, stats: Arc<Stats>) -> Self {
        Self {
            config,
            ledger,
            stats,
            activate_callback: Arc::new(Mutex::new(None)),
            mutex: Arc::new(Mutex::new(BacklogPopulationFlags {
                stopped: false,
                triggered: false,
            })),
            condition: Arc::new(Condvar::new()),
            thread: None,
        }
    }

    pub fn set_activate_callback(&mut self, callback: ActivateCallback) {
        let mut lock = self.activate_callback.lock().unwrap();
        *lock = Some(callback);
    }

    pub fn start(&mut self) {
        debug_assert!(self.thread.is_none());

        let thread = BacklogPopulationThread {
            ledger: Arc::clone(&self.ledger),
            stats: Arc::clone(&self.stats),
            activate_callback: Arc::clone(&self.activate_callback),
            config: self.config.clone(),
            mutex: Arc::clone(&self.mutex),
            condition: Arc::clone(&self.condition),
        };

        self.thread = Some(
            thread::Builder::new()
                .name("Backlog".to_owned())
                .spawn(move || {
                    thread.run();
                })
                .unwrap(),
        );
    }

    pub fn stop(&mut self) {
        let mut lock = self.mutex.lock().unwrap();
        lock.stopped = true;
        drop(lock);
        self.notify();
        if let Some(handle) = self.thread.take() {
            handle.join().unwrap()
        }
    }

    /** Manually trigger backlog population */
    pub fn trigger(&self) {
        {
            let mut lock = self.mutex.lock().unwrap();
            lock.triggered = true;
        }
        self.notify();
    }

    /** Notify about AEC vacancy */
    pub fn notify(&self) {
        self.condition.notify_all();
    }
}

impl Drop for BacklogPopulation {
    fn drop(&mut self) {
        self.stop();
    }
}

struct BacklogPopulationThread {
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    activate_callback: Arc<Mutex<Option<ActivateCallback>>>,
    config: BacklogPopulationConfig,
    mutex: Arc<Mutex<BacklogPopulationFlags>>,
    condition: Arc<Condvar>,
}

impl BacklogPopulationThread {
    fn run(&self) {
        let mut lock = self.mutex.lock().unwrap();
        while !lock.stopped {
            if self.predicate(&lock) {
                let _ = self
                    .stats
                    .inc(StatType::Backlog, DetailType::Loop, Direction::In);

                lock.triggered = false;
                drop(lock);
                self.populate_backlog();
                lock = self.mutex.lock().unwrap();
            }

            lock = self
                .condition
                .wait_while(lock, |l| !l.stopped && !self.predicate(&l))
                .unwrap();
        }
    }

    fn predicate(&self, lock: &BacklogPopulationFlags) -> bool {
        lock.triggered || self.config.enabled
    }

    fn populate_backlog(&self) {
        debug_assert!(self.config.frequency > 0);
        let mut lock = self.mutex.lock().unwrap();

        let chunk_size = self.config.batch_size / self.config.frequency;
        let mut done = false;
        let mut next = Account::zero();
        while !lock.stopped && !done {
            drop(lock);
            {
                let transaction = self.ledger.store.tx_begin_read();

                let mut count = 0u32;
                let mut i = self
                    .ledger
                    .store
                    .account()
                    .begin_account(transaction.txn(), &next);
                // 			auto const end = ledger.store.account ().end ();
                while let Some((account, _)) = i.current() {
                    if count >= chunk_size {
                        break;
                    }

                    let _ = self
                        .stats
                        .inc(StatType::Backlog, DetailType::Total, Direction::In);

                    self.activate(transaction.txn(), account);
                    next = (account.number().overflowing_add(U256::from(1)).0).into();

                    i.next();
                    count += 1;
                }
                done = next == Account::zero()
                    || self
                        .ledger
                        .store
                        .account()
                        .begin_account(transaction.txn(), &next)
                        .is_end();
            }
            lock = self.mutex.lock().unwrap();
            // Give the rest of the node time to progress without holding database lock
            lock = self
                .condition
                .wait_timeout(
                    lock,
                    Duration::from_millis(1000 / self.config.frequency as u64),
                )
                .unwrap()
                .0;
        }
    }

    pub fn activate(&self, txn: &dyn Transaction, account: &Account) {
        let account_info = match self.ledger.store.account().get(txn, account) {
            Some(info) => info,
            None => {
                return;
            }
        };

        let conf_info = self
            .ledger
            .store
            .confirmation_height()
            .get(txn, account)
            .unwrap_or_default();

        // If conf info is empty then it means then it means nothing is confirmed yet
        if conf_info.height < account_info.block_count {
            let _ = self
                .stats
                .inc(StatType::Backlog, DetailType::Activated, Direction::In);

            let callback_lock = self.activate_callback.lock().unwrap();
            match callback_lock.deref() {
                Some(callback) => callback(txn, account, &account_info, &conf_info),
                None => {
                    debug_assert!(false)
                }
            }
        }
    }
}
