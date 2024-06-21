use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};
use rsnano_core::{Account, Amount};
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, RwLock, RwLockReadGuard};

pub struct RepWeightCache(Arc<RwLock<HashMap<Account, Amount>>>);

impl RepWeightCache {
    pub fn new(cache: Arc<RwLock<HashMap<Account, Amount>>>) -> Self {
        Self(cache)
    }

    pub fn read(&self) -> RwLockReadGuard<HashMap<Account, Amount>> {
        self.0.read().unwrap()
    }

    pub fn get_weight(&self, rep: &Account) -> Amount {
        self.0.read().unwrap().get(rep).cloned().unwrap_or_default()
    }

    pub fn len(&self) -> usize {
        self.0.read().unwrap().len()
    }

    pub fn collect_container_info(&self, name: impl Into<String>) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            name.into(),
            vec![ContainerInfoComponent::Leaf(ContainerInfo {
                name: "rep_weights".to_string(),
                count: self.len(),
                sizeof_element: size_of::<(Account, Amount)>(),
            })],
        )
    }
}
