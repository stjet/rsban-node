use std::sync::{Arc, Mutex, RwLock, Weak};

use indexmap::IndexMap;
use rand::Rng;

use crate::{BlockEnum, BlockHash};

pub(crate) struct BlockUniquer {
    blocks: Mutex<IndexMap<BlockHash, Weak<RwLock<BlockEnum>>>>,
}

impl BlockUniquer {
    const CLEANUP_COUNT: usize = 2;

    pub(crate) fn new() -> Self {
        Self {
            blocks: Mutex::new(IndexMap::new()),
        }
    }

    pub(crate) fn unique(&self, block: &Arc<RwLock<BlockEnum>>) -> Arc<RwLock<BlockEnum>> {
        let key = block.read().unwrap().as_block().full_hash();
        let mut blocks = self.blocks.lock().unwrap();

        let result = match blocks.get(&key) {
            Some(weak) => match weak.upgrade() {
                Some(b) => b,
                None => {
                    blocks.insert(key, Arc::downgrade(block));
                    block.clone()
                }
            },
            None => {
                blocks.insert(key, Arc::downgrade(block));
                block.clone()
            }
        };

        cleanup(blocks);

        result
    }

    pub(crate) fn size(&self) -> usize {
        self.blocks.lock().unwrap().len()
    }
}

fn cleanup(mut blocks: std::sync::MutexGuard<IndexMap<BlockHash, Weak<RwLock<BlockEnum>>>>) {
    let mut i = 0;
    while i < BlockUniquer::CLEANUP_COUNT && !blocks.is_empty() {
        let random_offset = rand::thread_rng().gen_range(0..blocks.len());
        let mut hash_to_remove = None;
        if let Some((hash, weak)) = blocks.get_index(random_offset) {
            if weak.upgrade().is_none() {
                hash_to_remove = Some(*hash);
            }
        }
        if let Some(hash) = &hash_to_remove {
            blocks.remove(hash);
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StateBlockBuilder;

    #[test]
    fn new_block_gets_returned() {
        let uniquer = BlockUniquer::new();
        let block1 = create_block();
        let block2 = uniquer.unique(&block1);
        assert_eq!(Arc::as_ptr(&block1), Arc::as_ptr(&block2));
    }

    #[test]
    fn when_block_hashes_are_equal_return_original_block() {
        let uniquer = BlockUniquer::new();
        let block1 = create_block();
        let block2 = Arc::new(RwLock::new(block1.read().unwrap().clone()));
        uniquer.unique(&block1);
        let result = uniquer.unique(&block2);
        assert_eq!(Arc::as_ptr(&result), Arc::as_ptr(&block1));
    }

    #[test]
    fn uniquer_holds_weak_references() {
        let uniquer = BlockUniquer::new();
        let block = create_block();
        let weak = Arc::downgrade(&block);
        drop(uniquer.unique(&block));
        drop(block);
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn cleanup() {
        let uniquer = BlockUniquer::new();
        let block1 = create_block();
        uniquer.unique(&block1);
        {
            let block2 = create_block();
            uniquer.unique(&block2);
        }
        assert_eq!(uniquer.size(), 2);
        let mut iterations = 0;
        while uniquer.size() == 2 {
            uniquer.unique(&block1);
            iterations += 1;
            assert!(iterations < 200);
        }
    }

    fn create_block() -> Arc<RwLock<BlockEnum>> {
        Arc::new(RwLock::new(BlockEnum::State(
            StateBlockBuilder::new().build().unwrap(),
        )))
    }
}
