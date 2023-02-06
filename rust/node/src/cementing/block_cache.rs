use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use rsnano_core::{BlockEnum, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

pub(crate) struct BlockCache {
    //todo: Remove RwLock? `contains` is called by RPC!
    block_cache: RwLock<HashMap<BlockHash, Arc<BlockEnum>>>,
    ledger: Arc<Ledger>,
}

impl BlockCache {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            block_cache: RwLock::new(HashMap::new()),
            ledger,
        }
    }

    pub fn add(&self, block: Arc<BlockEnum>) {
        self.block_cache
            .write()
            .unwrap()
            .insert(block.hash(), block);
    }

    pub fn get_cached(&self, block_hash: &BlockHash) -> Option<Arc<BlockEnum>> {
        self.block_cache
            .read()
            .unwrap()
            .get(block_hash)
            .map(Arc::clone)
    }

    pub fn load_block(&self, hash: &BlockHash, txn: &dyn Transaction) -> Option<Arc<BlockEnum>> {
        let mut cache = self.block_cache.write().unwrap();
        match cache.get(hash) {
            Some(block) => Some(Arc::clone(block)),
            None => {
                let block = self.ledger.get_block(txn, hash)?; //todo: remove unwrap
                let block = Arc::new(block);
                cache.insert(*hash, Arc::clone(&block));
                Some(block)
            }
        }
    }

    pub fn contains(&self, hash: &BlockHash) -> bool {
        self.block_cache.read().unwrap().contains_key(hash)
    }

    pub fn len(&self) -> usize {
        self.block_cache.read().unwrap().len()
    }

    pub fn clear(&self) {
        self.block_cache.write().unwrap().clear();
    }
}
