use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, RwLock,
    },
};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{BlockEnum, BlockHash};

use super::ledger_data_requester::LedgerDataRequester;

struct BlockCacheStorage {
    blocks: HashMap<BlockHash, BlockEnum>,
    sequential: BoundedVecDeque<BlockHash>,
}

pub struct BlockCache {
    //todo: Remove RwLock? `contains` is called by RPC!
    blocks: RwLock<BlockCacheStorage>,
    cache_size: Arc<AtomicUsize>,
}

impl BlockCache {
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
