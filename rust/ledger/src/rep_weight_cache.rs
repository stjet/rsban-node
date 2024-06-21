use rsnano_core::utils::{ContainerInfo, ContainerInfoComponent};
use rsnano_core::{Account, Amount};
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard};

use crate::LedgerCache;

/// Returns the cached vote weight for the given representative.
/// If the weight is below the cache limit it returns 0.
/// During bootstrap it returns the preconfigured bootstrap weights.
pub struct RepWeightCache {
    weights: Arc<RwLock<HashMap<Account, Amount>>>,
    pub bootstrap_weights: HashMap<Account, Amount>,
    max_blocks: u64,
    ledger_cache: Arc<LedgerCache>,
    check_bootstrap_weights: AtomicBool,
}

impl RepWeightCache {
    pub fn new() -> Self {
        Self {
            weights: Arc::new(RwLock::new(HashMap::new())),
            bootstrap_weights: HashMap::new(),
            max_blocks: 0,
            ledger_cache: Arc::new(LedgerCache::new()),
            check_bootstrap_weights: AtomicBool::new(false),
        }
    }

    pub fn with_bootstrap_weights(
        bootstrap_weights: HashMap<Account, Amount>,
        max_blocks: u64,
        ledger_cache: Arc<LedgerCache>,
    ) -> Self {
        Self {
            weights: Arc::new(RwLock::new(HashMap::new())),
            bootstrap_weights,
            max_blocks,
            ledger_cache,
            check_bootstrap_weights: AtomicBool::new(true),
        }
    }

    pub fn read(&self) -> RwLockReadGuard<HashMap<Account, Amount>> {
        self.weights.read().unwrap()
    }

    pub fn get_weight(&self, rep: &Account) -> Amount {
        if self.check_bootstrap_weights.load(Ordering::SeqCst) {
            if self.ledger_cache.block_count.load(Ordering::SeqCst) < self.max_blocks {
                if let Some(&weight) = self.bootstrap_weights.get(rep) {
                    return weight;
                }
            } else {
                self.check_bootstrap_weights.store(false, Ordering::SeqCst);
            }
        }

        self.weights
            .read()
            .unwrap()
            .get(rep)
            .cloned()
            .unwrap_or_default()
    }

    pub fn bootstrap_weight_max_blocks(&self) -> u64 {
        self.max_blocks
    }

    pub fn len(&self) -> usize {
        self.weights.read().unwrap().len()
    }

    pub(super) fn inner(&self) -> Arc<RwLock<HashMap<Account, Amount>>> {
        self.weights.clone()
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
