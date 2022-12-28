use crate::{as_write_txn, count, get, parallel_traversal, LmdbEnv, LmdbIteratorImpl};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use num_traits::FromPrimitive;
use rsnano_core::{
    deserialize_block_enum,
    utils::{MemoryStream, Serialize, Stream, StreamAdapter},
    Account, Amount, Block, BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockType,
    BlockVisitor, ChangeBlock, Epoch, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
};
use rsnano_store_traits::{
    BlockIterator, BlockStore, ReadTransaction, Transaction, WriteTransaction,
};
use std::sync::Arc;

pub struct LmdbBlockStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbBlockStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("blocks"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }

    pub fn raw_put(&self, txn: &mut dyn WriteTransaction, data: &[u8], hash: &BlockHash) {
        as_write_txn(txn)
            .put(self.database, hash.as_bytes(), &data, WriteFlags::empty())
            .unwrap();
    }

    pub fn block_raw_get<'a>(
        &self,
        txn: &'a dyn Transaction,
        hash: &BlockHash,
    ) -> Option<&'a [u8]> {
        match get(txn, self.database, hash.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => Some(bytes),
            Err(e) => panic!("Could not load block. {:?}", e),
        }
    }
}

impl BlockStore for LmdbBlockStore {
    fn put(&self, txn: &mut dyn WriteTransaction, block: &BlockEnum) {
        let hash = block.hash();
        debug_assert!(
            block.sideband().unwrap().successor.is_zero()
                || self.exists(txn.txn(), &block.sideband().unwrap().successor)
        );

        let mut stream = MemoryStream::new();
        stream.write_u8(block.block_type() as u8).unwrap();
        block.serialize(&mut stream).unwrap();
        block
            .sideband()
            .unwrap()
            .serialize(&mut stream, block.block_type())
            .unwrap();
        self.raw_put(txn, stream.as_bytes(), &hash);
        {
            let mut predecessor = BlockPredecessorMdbSet::new(txn, self);
            block.visit(&mut predecessor);
        }

        debug_assert!(
            block.previous().is_zero()
                || self
                    .successor(txn.txn(), &block.previous())
                    .unwrap_or_default()
                    == hash
        );
    }

    fn exists(&self, transaction: &dyn Transaction, hash: &BlockHash) -> bool {
        self.block_raw_get(transaction, hash).is_some()
    }

    fn successor(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockHash> {
        self.block_raw_get(txn, hash)
            .map(|data| {
                debug_assert!(data.len() >= 32);
                let block_type = BlockType::from_u8(data[0]).unwrap();
                let offset = block_successor_offset(data.len(), block_type);
                let successor =
                    BlockHash::from_bytes(data[offset..offset + 32].try_into().unwrap());
                if successor.is_zero() {
                    None
                } else {
                    Some(successor)
                }
            })
            .flatten()
    }

    fn successor_clear(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash) {
        let value = self.block_raw_get(txn.txn(), hash).unwrap();
        let block_type = BlockType::from_u8(value[0]).unwrap();

        let mut data = value.to_vec();
        let offset = block_successor_offset(value.len(), block_type);
        data[offset..offset + BlockHash::serialized_size()].fill(0);
        self.raw_put(txn, &data, hash)
    }

    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum> {
        match self.block_raw_get(txn, hash) {
            None => None,
            Some(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                let mut block = deserialize_block_enum(&mut stream).unwrap();
                let mut sideband =
                    BlockSideband::from_stream(&mut stream, block.block_type()).unwrap();
                // BlockSideband does not serialize all data depending on the block type.
                // That's why we fill in the missing data here:
                match &block {
                    BlockEnum::LegacySend(_) => {
                        sideband.balance = block.balance();
                        sideband.details = BlockDetails::new(Epoch::Epoch0, true, false, false)
                    }
                    BlockEnum::LegacyOpen(_) => {
                        sideband.account = block.account();
                        sideband.details = BlockDetails::new(Epoch::Epoch0, false, true, false)
                    }
                    BlockEnum::LegacyReceive(_) => {
                        sideband.details = BlockDetails::new(Epoch::Epoch0, false, true, false)
                    }
                    BlockEnum::LegacyChange(_) => {
                        sideband.details = BlockDetails::new(Epoch::Epoch0, false, false, false)
                    }
                    BlockEnum::State(_) => {
                        sideband.account = block.account();
                        sideband.balance = block.balance();
                    }
                }
                block.as_block_mut().set_sideband(sideband);
                Some(block)
            }
        }
    }

    fn get_no_sideband(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum> {
        match self.block_raw_get(txn, hash) {
            None => None,
            Some(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                Some(deserialize_block_enum(&mut stream).unwrap())
            }
        }
    }

    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash) {
        as_write_txn(txn)
            .del(self.database, hash.as_bytes(), None)
            .unwrap();
    }

    fn count(&self, txn: &dyn Transaction) -> u64 {
        count(txn, self.database)
    }

    fn account(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Account> {
        let block = self.get(txn, hash)?;
        Some(block.account_calculated())
    }

    fn begin(&self, transaction: &dyn Transaction) -> BlockIterator {
        LmdbIteratorImpl::new_iterator(transaction, self.database, None, true)
    }

    fn begin_at_hash(&self, transaction: &dyn Transaction, hash: &BlockHash) -> BlockIterator {
        LmdbIteratorImpl::new_iterator(transaction, self.database, Some(hash.as_bytes()), true)
    }

    fn end(&self) -> BlockIterator {
        LmdbIteratorImpl::null_iterator()
    }

    fn random(&self, transaction: &dyn Transaction) -> Option<BlockEnum> {
        let hash = BlockHash::random();
        let mut existing = self.begin_at_hash(transaction, &hash);
        if existing.is_end() {
            existing = self.begin(transaction);
        }

        existing.current().map(|(_, v)| v.block.clone())
    }

    fn balance(&self, txn: &dyn Transaction, hash: &BlockHash) -> Amount {
        match self.get(txn, hash) {
            Some(block) => block.balance_calculated(),
            None => Amount::zero(),
        }
    }

    fn version(&self, txn: &dyn Transaction, hash: &BlockHash) -> Epoch {
        match self.get(txn, hash) {
            Some(block) => {
                if let BlockEnum::State(b) = block {
                    b.sideband().unwrap().details.epoch
                } else {
                    Epoch::Epoch0
                }
            }
            None => Epoch::Epoch0,
        }
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, BlockIterator, BlockIterator) + Send + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_hash(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    fn account_height(&self, txn: &dyn Transaction, hash: &BlockHash) -> u64 {
        match self.get(txn, hash) {
            Some(block) => block.sideband().unwrap().height,
            None => 0,
        }
    }
}

/// Fill in our predecessors
struct BlockPredecessorMdbSet<'a> {
    transaction: &'a mut dyn WriteTransaction,
    block_store: &'a LmdbBlockStore,
}

impl<'a, 'b> BlockPredecessorMdbSet<'a> {
    fn new(transaction: &'a mut dyn WriteTransaction, block_store: &'a LmdbBlockStore) -> Self {
        Self {
            transaction,
            block_store,
        }
    }

    fn fill_value(&mut self, block: &dyn Block) {
        let hash = block.hash();
        let t = self.transaction.txn();
        let value = self
            .block_store
            .block_raw_get(t, &block.previous())
            .unwrap();
        let mut data = value.to_vec();
        let block_type = BlockType::from_u8(data[0]).unwrap();

        let offset = block_successor_offset(data.len(), block_type);
        data[offset..offset + hash.as_bytes().len()].copy_from_slice(hash.as_bytes());

        self.block_store
            .raw_put(self.transaction, &data, &block.previous());
    }
}

impl<'a> BlockVisitor for BlockPredecessorMdbSet<'a> {
    fn send_block(&mut self, block: &SendBlock) {
        self.fill_value(block);
    }

    fn receive_block(&mut self, block: &ReceiveBlock) {
        self.fill_value(block);
    }

    fn open_block(&mut self, _block: &OpenBlock) {
        // Open blocks don't have a predecessor
    }

    fn change_block(&mut self, block: &ChangeBlock) {
        self.fill_value(block);
    }

    fn state_block(&mut self, block: &StateBlock) {
        if !block.previous().is_zero() {
            self.fill_value(block);
        }
    }
}

fn block_successor_offset(entry_size: usize, block_type: BlockType) -> usize {
    entry_size - BlockSideband::serialized_size(block_type)
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;
    use rsnano_core::BlockBuilder;

    use super::*;

    #[test]
    fn empty() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert!(store.get(&txn, &BlockHash::from(1)).is_none());
        assert_eq!(store.exists(&txn, &BlockHash::from(1)), false);
        assert_eq!(store.count(&txn), 0);
        Ok(())
    }

    #[test]
    fn add_block() {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env()).unwrap();
        let mut txn = env.tx_begin_write().unwrap();
        let block = BlockBuilder::legacy_open().with_sideband().build();
        let block_hash = block.hash();

        store.put(&mut txn, &block);
        let loaded = store.get(&txn, &block.hash()).expect("block not found");

        assert_eq!(loaded, block);
        assert!(store.exists(&txn, &block_hash));
        assert_eq!(store.count(&txn), 1);
    }

    #[test]
    fn clear_successor() {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env()).unwrap();
        let mut txn = env.tx_begin_write().unwrap();

        let mut block1 = BlockBuilder::legacy_open()
            .account(Account::from(1))
            .representative(Account::from(2))
            .with_sideband()
            .build();

        let block2 = BlockBuilder::legacy_open()
            .account(Account::from(1))
            .representative(Account::from(3))
            .with_sideband()
            .build();

        let mut sideband = block1.sideband().unwrap().clone();
        sideband.successor = block2.hash();
        block1.as_block_mut().set_sideband(sideband);

        store.put(&mut txn, &block2);
        store.put(&mut txn, &block1);

        store.successor_clear(&mut txn, &block1.hash());

        let loaded = store.get(&txn, &block1.hash()).expect("block not found");
        assert_eq!(loaded.sideband().unwrap().successor, BlockHash::zero());
    }

    #[test]
    fn add_two_blocks() {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env()).unwrap();
        let mut txn = env.tx_begin_write().unwrap();
        let block1 = BlockBuilder::legacy_open().with_sideband().build();
        let block2 = BlockBuilder::legacy_open().with_sideband().build();

        store.put(&mut txn, &block1);
        store.put(&mut txn, &block2);
        let loaded1 = store.get(&txn, &block1.hash()).expect("block1 not found");
        let loaded2 = store.get(&txn, &block2.hash()).expect("block2 not found");

        assert_eq!(loaded1, block1);
        assert_eq!(loaded2, block2);
    }

    #[test]
    fn add_receive() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let block1 = BlockBuilder::legacy_open().with_sideband().build();
        let block2 = BlockBuilder::legacy_receive()
            .previous(block1.hash())
            .with_sideband()
            .build();
        let mut txn = env.tx_begin_write()?;
        store.put(&mut txn, &block1);
        store.put(&mut txn, &block2);
        let loaded = store.get(&txn, &block2.hash()).expect("block not found");
        assert_eq!(loaded, block2);
        Ok(())
    }

    #[test]
    fn add_state() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let block1 = BlockBuilder::legacy_open().with_sideband().build();
        let block2 = BlockBuilder::state()
            .previous(block1.hash())
            .with_sideband()
            .build();
        let mut txn = env.tx_begin_write()?;
        store.put(&mut txn, &block1);
        store.put(&mut txn, &block2);
        let loaded = store.get(&txn, &block2.hash()).expect("block not found");
        assert_eq!(loaded, block2);
        Ok(())
    }

    #[test]
    fn replace_block() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let open = BlockBuilder::legacy_open().with_sideband().build();
        let send1 = BlockBuilder::legacy_send()
            .previous(open.hash())
            .with_sideband()
            .build();
        let mut send2 = send1.clone();
        send2.as_block_mut().set_work(12345);

        store.put(&mut txn, &open);
        store.put(&mut txn, &send1);
        store.put(&mut txn, &send2);

        assert_eq!(store.count(&txn), 2);
        assert_eq!(store.get(&txn, &send1.hash()).unwrap().work(), 12345);
        Ok(())
    }

    #[test]
    fn random() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let block = BlockBuilder::legacy_open().with_sideband().build();

        store.put(&mut txn, &block);
        let random = store.random(&txn).expect("block not found");

        assert_eq!(random, block);
        Ok(())
    }

    #[test]
    fn reset_renew_existing_transaction() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let block = BlockBuilder::legacy_open().with_sideband().build();
        let block_hash = block.hash();

        let mut read_txn = env.tx_begin_read()?;
        read_txn.reset();
        {
            let mut txn = env.tx_begin_write()?;
            store.put(&mut txn, &block);
        }
        read_txn.renew();
        assert!(store.exists(&read_txn, &block_hash));
        Ok(())
    }
}
