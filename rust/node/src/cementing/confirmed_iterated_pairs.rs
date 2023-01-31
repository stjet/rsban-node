use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use rsnano_core::Account;

#[derive(Clone)]
pub struct ConfirmedIteratedPair {
    pub confirmed_height: u64,
    pub iterated_height: u64,
}

// The atomic variable here just tracks the size for use in collect_container_info.
// This is so that no mutexes are needed during the algorithm itself, which would otherwise be needed
// for the sake of a rarely used RPC call for debugging purposes. As such the sizes are not being acted
// upon in any way (does not synchronize with any other data).
// This allows the load and stores to use relaxed atomic memory ordering.
pub(crate) struct ConfirmedIteratedPairMap {
    map: HashMap<Account, ConfirmedIteratedPair>,
    atomic_size: AtomicUsize,
}

impl ConfirmedIteratedPairMap {
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::new(),
            atomic_size: AtomicUsize::new(0),
        }
    }

    pub(crate) fn insert(&mut self, account: Account, confirmed_height: u64, iterated_height: u64) {
        self.map.insert(
            account,
            ConfirmedIteratedPair {
                confirmed_height,
                iterated_height,
            },
        );
        self.atomic_size.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn update_iterated_height(
        &mut self,
        account: &Account,
        confirmed_height: u64,
        iterated_height: u64,
    ) {
        if self.map.contains_key(&account) {
            self.map.get_mut(&account).unwrap().iterated_height = iterated_height;
        } else {
            self.insert(*account, confirmed_height, iterated_height);
        }
    }

    pub(crate) fn get(&self, account: &Account) -> Option<&ConfirmedIteratedPair> {
        self.map.get(account)
    }

    pub(crate) fn get_mut(&mut self, account: &Account) -> Option<&mut ConfirmedIteratedPair> {
        self.map.get_mut(account)
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.atomic_size.store(0, Ordering::Relaxed);
    }

    pub(crate) fn size_atomic(&self) -> usize {
        self.atomic_size.load(Ordering::Relaxed)
    }
}
