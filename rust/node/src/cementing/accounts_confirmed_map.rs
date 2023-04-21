use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use rsnano_core::{
    utils::{ContainerInfo, ContainerInfoComponent},
    Account, BlockHash,
};

pub(crate) struct ConfirmedInfo {
    pub(crate) confirmed_height: u64,
    pub(crate) iterated_frontier: BlockHash,
}

/// Holds confirmation height/cemented frontier in memory for accounts while iterating
pub(crate) struct AccountsConfirmedMap {
    confirmed_map: HashMap<Account, ConfirmedInfo>,
    confirmed_map_len: Arc<AtomicUsize>,
}

impl AccountsConfirmedMap {
    pub fn new() -> Self {
        Self {
            confirmed_map: HashMap::new(),
            confirmed_map_len: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn get(&self, account: &Account) -> Option<&ConfirmedInfo> {
        self.confirmed_map.get(account)
    }

    pub fn insert(&mut self, account: Account, info: ConfirmedInfo) {
        let old = self.confirmed_map.insert(account, info);
        if old.is_none() {
            self.confirmed_map_len.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn remove(&mut self, account: &Account) {
        let removed = self.confirmed_map.remove(account);
        if removed.is_some() {
            self.confirmed_map_len.fetch_sub(1, Ordering::Relaxed);
        }
    }

    pub fn clear(&mut self) {
        self.confirmed_map.clear();
        self.confirmed_map_len.store(0, Ordering::Relaxed);
    }

    pub fn len(&self) -> usize {
        self.confirmed_map.len()
    }

    pub fn container_info(&self) -> AccountsConfirmedMapContainerInfo {
        AccountsConfirmedMapContainerInfo {
            confirmed_map_len: self.confirmed_map_len.clone(),
        }
    }
}

pub(crate) struct AccountsConfirmedMapContainerInfo {
    confirmed_map_len: Arc<AtomicUsize>,
}

impl AccountsConfirmedMapContainerInfo {
    pub fn collect(&self, name: String) -> ContainerInfoComponent {
        ContainerInfoComponent::Leaf(ContainerInfo {
            name,
            count: self.confirmed_map_len.load(Ordering::Relaxed),
            sizeof_element: std::mem::size_of::<ConfirmedInfo>() + std::mem::size_of::<Account>(),
        })
    }
}
