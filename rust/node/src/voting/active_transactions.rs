use rsnano_core::{BlockEnum, QualifiedRoot};
use std::{
    cmp::max,
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

use crate::NetworkParams;

use super::Election;

pub struct ActiveTransactions {
    pub mutex: Mutex<ActiveTransactionsData>,
    pub condition: Condvar,
    network: NetworkParams,
}

impl ActiveTransactions {
    pub fn new(network: NetworkParams) -> Self {
        Self {
            mutex: Mutex::new(ActiveTransactionsData {
                roots: OrderedRoots::default(),
                stopped: false,
            }),
            condition: Condvar::new(),
            network,
        }
    }

    pub fn erase(&self, _block: &BlockEnum) {}

    pub fn request_loop<'a>(
        &self,
        stamp: Instant,
        guard: MutexGuard<'a, ActiveTransactionsData>,
    ) -> MutexGuard<'a, ActiveTransactionsData> {
        if !guard.stopped {
            let loop_interval =
                Duration::from_millis(self.network.network.aec_loop_interval_ms as u64);
            let min_sleep = loop_interval / 2;

            let wait_duration = max(
                min_sleep,
                (stamp + loop_interval).saturating_duration_since(Instant::now()),
            );

            self.condition
                .wait_timeout_while(guard, wait_duration, |data| data.stopped)
                .unwrap()
                .0
        } else {
            guard
        }
    }
}

pub struct ActiveTransactionsData {
    pub roots: OrderedRoots,
    pub stopped: bool,
}

#[derive(Default)]
pub struct OrderedRoots {
    by_root: HashMap<QualifiedRoot, Arc<Election>>,
    sequenced: Vec<QualifiedRoot>,
}

impl OrderedRoots {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, root: QualifiedRoot, election: Arc<Election>) {
        if self.by_root.insert(root.clone(), election).is_none() {
            self.sequenced.push(root);
        }
    }

    pub fn get(&self, root: &QualifiedRoot) -> Option<&Arc<Election>> {
        self.by_root.get(root)
    }

    pub fn erase(&mut self, root: &QualifiedRoot) {
        if let Some(_) = self.by_root.remove(root) {
            self.sequenced.retain(|x| x != root)
        }
    }

    pub fn clear(&mut self) {
        self.sequenced.clear();
        self.by_root.clear();
    }

    pub fn len(&self) -> usize {
        self.sequenced.len()
    }

    pub fn iter_sequenced(&self) -> impl Iterator<Item = (&QualifiedRoot, &Arc<Election>)> {
        self.sequenced
            .iter()
            .map(|r| (r, self.by_root.get(r).unwrap()))
    }
}
