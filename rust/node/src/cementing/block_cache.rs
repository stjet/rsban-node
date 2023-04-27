use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{BlockEnum, BlockHash};
use rsnano_ledger::Ledger;
use rsnano_store_traits::Transaction;

use super::ledger_data_requester::LedgerDataRequester;

pub struct BlockCache {
    //todo: Remove RwLock? `contains` is called by RPC!
    block_cache: RwLock<HashMap<BlockHash, Arc<BlockEnum>>>,
    ledger: Arc<Ledger>,
    cache_size: Arc<AtomicUsize>,
}

impl BlockCache {
    pub fn new(ledger: Arc<Ledger>) -> Self {
        Self {
            block_cache: RwLock::new(HashMap::new()),
            ledger,
            cache_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn add(&self, block: Arc<BlockEnum>) {
        self.block_cache
            .write()
            .unwrap()
            .insert(block.hash(), block);
        self.cache_size.fetch_add(1, Ordering::Relaxed);
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

    pub fn atomic_len(&self) -> &Arc<AtomicUsize> {
        &self.cache_size
    }

    pub fn clear(&self) {
        self.block_cache.write().unwrap().clear();
        self.cache_size.store(0, Ordering::Relaxed);
    }
}

struct BlockCacheStorage {
    blocks: HashMap<BlockHash, BlockEnum>,
    sequential: BoundedVecDeque<BlockHash>,
}

pub(crate) struct BlockCacheV2 {
    //todo: Remove RwLock? `contains` is called by RPC!
    blocks: RwLock<BlockCacheStorage>,
    cache_size: Arc<AtomicUsize>,
}

impl BlockCacheV2 {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(BlockCacheStorage {
                blocks: HashMap::new(),
                sequential: BoundedVecDeque::new(0x4000),
            }),
            cache_size: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn add(&self, block: BlockEnum) {
        let mut lock = self.blocks.write().unwrap();
        if let Some(old) = lock.sequential.push_back(block.hash()) {
            lock.blocks.remove(&old);
        }
        lock.blocks.insert(block.hash(), block);
        self.cache_size.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_cached(&self, block_hash: &BlockHash) -> Option<BlockEnum> {
        self.blocks.read().unwrap().blocks.get(block_hash).cloned()
    }

    pub fn load_block<T: LedgerDataRequester>(
        &self,
        hash: &BlockHash,
        data_requester: &T,
    ) -> Option<BlockEnum> {
        let mut cache = self.blocks.write().unwrap();
        match cache.blocks.get(hash) {
            Some(block) => Some(block.clone()),
            None => {
                if let Some(block) = data_requester.get_block(hash) {
                    if let Some(old) = cache.sequential.push_back(block.hash()) {
                        cache.blocks.remove(&old);
                    }
                    cache.blocks.insert(*hash, block.clone());
                    Some(block)
                } else {
                    None
                }
            }
        }
    }

    pub fn contains(&self, hash: &BlockHash) -> bool {
        self.blocks.read().unwrap().blocks.contains_key(hash)
    }

    pub fn len(&self) -> usize {
        self.blocks.read().unwrap().blocks.len()
    }

    pub fn atomic_len(&self) -> &Arc<AtomicUsize> {
        &self.cache_size
    }

    pub fn clear(&self) {
        let mut lock = self.blocks.write().unwrap();
        lock.blocks.clear();
        lock.sequential.clear();
        self.cache_size.store(0, Ordering::Relaxed);
    }
}
