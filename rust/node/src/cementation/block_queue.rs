use std::{
    collections::{HashSet, VecDeque},
    mem::size_of,
    sync::Arc,
};

use rsnano_core::{BlockEnum, BlockHash};

pub(crate) struct BlockQueue {
    blocks: VecDeque<Arc<BlockEnum>>,
    hashes: HashSet<BlockHash>,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            blocks: VecDeque::new(),
            hashes: HashSet::new(),
        }
    }

    pub(crate) fn entry_size() -> usize {
        size_of::<Arc<BlockEnum>>() + size_of::<BlockHash>()
    }

    pub(crate) fn len(&self) -> usize {
        self.blocks.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub(crate) fn contains(&self, hash: &BlockHash) -> bool {
        self.hashes.contains(hash)
    }

    pub(crate) fn push_back(&mut self, block: Arc<BlockEnum>) {
        let hash = block.hash();
        if self.hashes.contains(&hash) {
            return;
        }

        self.blocks.push_back(block);
        self.hashes.insert(hash);
    }

    pub(crate) fn front(&self) -> Option<&Arc<BlockEnum>> {
        self.blocks.front()
    }

    pub(crate) fn pop_front(&mut self) -> Option<Arc<BlockEnum>> {
        let front = self.blocks.pop_front();
        if let Some(block) = &front {
            self.hashes.remove(&block.hash());
        }
        front
    }
}
