use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
};

use rsnano_core::Amount;

use super::Wallet;

pub struct WalletActionThread {
    pub mutex: Mutex<BTreeMap<Amount, Vec<(Arc<Wallet>, Box<dyn Fn(Arc<Wallet>) + Send>)>>>,
    pub stopped: AtomicBool,
    pub condition: Condvar,
    observer: Box<dyn Fn(bool) + Send>,
}

impl WalletActionThread {
    pub fn new() -> Self {
        Self {
            mutex: Mutex::new(BTreeMap::new()),
            stopped: AtomicBool::new(false),
            condition: Condvar::new(),
            observer: Box::new(|_| {}),
        }
    }

    pub fn stop(&self) {
        {
            let mut guard = self.mutex.lock().unwrap();
            self.stopped.store(true, Ordering::SeqCst);
            guard.clear();
        }
        self.condition.notify_all();
        //TODO port more...
    }

    pub fn queue_wallet_action(
        &self,
        amount: Amount,
        wallet: Arc<Wallet>,
        action: Box<dyn Fn(Arc<Wallet>) + Send>,
    ) {
        {
            let mut guard = self.mutex.lock().unwrap();
            guard.entry(amount).or_default().push((wallet, action));
        }
        self.condition.notify_all();
    }

    pub fn len(&self) -> usize {
        self.mutex.lock().unwrap().len()
    }

    pub fn set_observer(&mut self, observer: Box<dyn Fn(bool) + Send>) {
        self.observer = observer;
    }

    pub fn do_wallet_actions(&self) {
        let mut guard = self.mutex.lock().unwrap();
        while !self.stopped.load(Ordering::SeqCst) {
            if let Some((_, wallets)) = guard.pop_first() {
                for (wallet, action) in wallets {
                    if self.stopped.load(Ordering::SeqCst) {
                        break;
                    }

                    if wallet.live() {
                        drop(guard);
                        (self.observer)(true);
                        action(wallet);
                        (self.observer)(false);
                        guard = self.mutex.lock().unwrap();
                    }
                }
            } else {
                guard = self.condition.wait(guard).unwrap();
            }
        }
    }
}
