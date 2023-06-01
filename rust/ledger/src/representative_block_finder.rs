use rsnano_core::{BlockEnum, BlockHash};
use rsnano_store_lmdb::{Environment, LmdbStore, Transaction};

/// Goes back in the block history until it finds a block with representative information
pub struct RepresentativeBlockFinder<'a, T: Environment + 'static> {
    txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    store: &'a LmdbStore<T>,
}

impl<'a, T: Environment + 'static> RepresentativeBlockFinder<'a, T> {
    pub fn new(
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        store: &'a LmdbStore<T>,
    ) -> Self {
        Self { txn, store }
    }

    pub fn find_rep_block(&self, hash: BlockHash) -> BlockHash {
        let mut current = hash;
        let mut result = BlockHash::zero();
        while result.is_zero() {
            let Some(block) = self.store.block.get(self.txn, &current) else {return BlockHash::zero();};
            (current, result) = match &block {
                BlockEnum::LegacySend(_) => (block.previous(), BlockHash::zero()),
                BlockEnum::LegacyReceive(_) => (block.previous(), BlockHash::zero()),
                BlockEnum::LegacyOpen(_) => (BlockHash::zero(), block.hash()),
                BlockEnum::LegacyChange(_) => (BlockHash::zero(), block.hash()),
                BlockEnum::State(_) => (BlockHash::zero(), block.hash()),
            };
        }

        result
    }
}
