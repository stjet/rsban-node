use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};
use crate::{
    datastore::{block_store::BlockIterator, parallel_traversal, BlockStore, DbIterator},
    deserialize_block_enum,
    utils::{MemoryStream, Serialize, Stream, StreamAdapter},
    Account, Amount, Block, BlockEnum, BlockHash, BlockSideband, BlockType, BlockVisitor, Epoch,
};
use lmdb::{Database, DatabaseFlags, WriteFlags};
use num_traits::FromPrimitive;
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

    pub fn raw_put(&self, txn: &mut LmdbWriteTransaction, data: &[u8], hash: &BlockHash) {
        txn.rw_txn_mut()
            .put(self.database, hash.as_bytes(), &data, WriteFlags::empty())
            .unwrap();
    }

    pub fn block_raw_get<'txn>(
        &'txn self,
        txn: &'txn LmdbTransaction,
        hash: &BlockHash,
    ) -> Option<&[u8]> {
        match txn.get(self.database, hash.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => Some(bytes),
            Err(e) => panic!("Could not load block. {:?}", e),
        }
    }
}

impl<'a> BlockStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbBlockStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction<'a>, hash: &BlockHash, block: &dyn Block) {
        debug_assert!(
            block.sideband().unwrap().successor.is_zero()
                || self.exists(&txn.as_txn(), &block.sideband().unwrap().successor)
        );

        let mut stream = MemoryStream::new();
        stream.write_u8(block.block_type() as u8).unwrap();
        block.serialize(&mut stream).unwrap();
        block
            .sideband()
            .unwrap()
            .serialize(&mut stream, block.block_type())
            .unwrap();
        self.raw_put(txn, stream.as_bytes(), hash);
        {
            let mut predecessor = BlockPredecessorMdbSet::new(txn, self);
            block.visit(&mut predecessor);
        }

        debug_assert!(
            block.previous().is_zero() || self.successor(&txn.as_txn(), block.previous()) == *hash
        );
    }

    fn exists(&self, transaction: &LmdbTransaction, hash: &BlockHash) -> bool {
        self.block_raw_get(transaction, hash).is_some()
    }

    fn successor(&self, txn: &LmdbTransaction, hash: &BlockHash) -> BlockHash {
        match self.block_raw_get(txn, hash) {
            None => BlockHash::new(),
            Some(data) => {
                debug_assert!(data.len() >= 32);
                let block_type = BlockType::from_u8(data[0]).unwrap();
                let offset = block_successor_offset(data.len(), block_type);
                BlockHash::from_bytes(data[offset..offset + 32].try_into().unwrap())
            }
        }
    }

    fn successor_clear(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        let t = txn.as_txn();
        let value = self.block_raw_get(&t, hash).unwrap();
        let block_type = BlockType::from_u8(value[0]).unwrap();

        let mut data = value.to_vec();
        let offset = block_successor_offset(value.len(), block_type);
        data[offset..offset + BlockHash::serialized_size()].fill(0);
        self.raw_put(txn, &data, hash)
    }

    fn get(&self, txn: &LmdbTransaction, hash: &BlockHash) -> Option<BlockEnum> {
        match self.block_raw_get(txn, hash) {
            None => None,
            Some(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                let mut block = deserialize_block_enum(&mut stream).unwrap();
                let sideband = BlockSideband::from_stream(&mut stream, block.block_type()).unwrap();
                block.as_block_mut().set_sideband(sideband);
                Some(block)
            }
        }
    }

    fn get_no_sideband(&self, txn: &LmdbTransaction, hash: &BlockHash) -> Option<BlockEnum> {
        match self.block_raw_get(txn, hash) {
            None => None,
            Some(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                Some(deserialize_block_enum(&mut stream).unwrap())
            }
        }
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, hash: &BlockHash) {
        txn.rw_txn_mut()
            .del(self.database, hash.as_bytes(), None)
            .unwrap();
    }

    fn count(&self, txn: &LmdbTransaction) -> usize {
        txn.count(self.database)
    }

    fn account_calculated(&self, block: &dyn Block) -> Account {
        let result = if block.account().is_zero() {
            block.sideband().unwrap().account
        } else {
            *block.account()
        };

        debug_assert!(!result.is_zero());
        result
    }

    fn account(&self, txn: &LmdbTransaction, hash: &BlockHash) -> Account {
        let block = self.get(txn, hash).unwrap();
        self.account_calculated(block.as_block())
    }

    fn begin(&self, transaction: &LmdbTransaction) -> BlockIterator<LmdbIteratorImpl> {
        DbIterator::new(LmdbIteratorImpl::new(
            transaction,
            self.database,
            None,
            true,
        ))
    }

    fn begin_at_hash(
        &self,
        transaction: &LmdbTransaction,
        hash: &BlockHash,
    ) -> BlockIterator<LmdbIteratorImpl> {
        DbIterator::new(LmdbIteratorImpl::new(
            transaction,
            self.database,
            Some(hash.as_bytes()),
            true,
        ))
    }

    fn end(&self) -> BlockIterator<LmdbIteratorImpl> {
        DbIterator::new(LmdbIteratorImpl::null())
    }

    fn random(&self, transaction: &LmdbTransaction) -> Option<BlockEnum> {
        let hash = BlockHash::random();
        let mut existing = self.begin_at_hash(transaction, &hash);
        if existing.is_end() {
            existing = self.begin(transaction);
        }

        existing.current().map(|(_, v)| v.block.clone())
    }

    fn balance(&self, txn: &LmdbTransaction, hash: &BlockHash) -> Amount {
        match self.get(txn, hash) {
            Some(block) => self.balance_calculated(&block),
            None => Amount::zero(),
        }
    }

    fn balance_calculated(&self, block: &BlockEnum) -> Amount {
        match block {
            BlockEnum::Send(b) => b.balance(),
            BlockEnum::Receive(b) => b.sideband().unwrap().balance,
            BlockEnum::Open(b) => b.sideband().unwrap().balance,
            BlockEnum::Change(b) => b.sideband().unwrap().balance,
            BlockEnum::State(b) => b.balance(),
        }
    }

    fn version(&self, txn: &LmdbTransaction, hash: &BlockHash) -> crate::Epoch {
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
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            BlockIterator<LmdbIteratorImpl>,
            BlockIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_hash(&transaction.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it, end_it);
        });
    }

    fn account_height(&self, txn: &LmdbTransaction, hash: &BlockHash) -> u64 {
        match self.get(txn, hash) {
            Some(block) => block.as_block().sideband().unwrap().height,
            None => 0,
        }
    }
}

/// Fill in our predecessors
struct BlockPredecessorMdbSet<'a, 'b> {
    transaction: &'a mut LmdbWriteTransaction<'b>,
    block_store: &'a LmdbBlockStore,
}

impl<'a, 'b> BlockPredecessorMdbSet<'a, 'b> {
    fn new(transaction: &'a mut LmdbWriteTransaction<'b>, block_store: &'a LmdbBlockStore) -> Self {
        Self {
            transaction,
            block_store,
        }
    }

    fn fill_value(&mut self, block: &dyn Block) {
        let hash = block.hash();
        let t = self.transaction.as_txn();
        let value = self
            .block_store
            .block_raw_get(&t, block.previous())
            .unwrap();
        let mut data = value.to_vec();
        let block_type = BlockType::from_u8(data[0]).unwrap();

        let offset = block_successor_offset(data.len(), block_type);
        data[offset..offset + hash.as_bytes().len()].copy_from_slice(hash.as_bytes());

        self.block_store
            .raw_put(self.transaction, &data, block.previous());
    }
}

impl<'a, 'b> BlockVisitor for BlockPredecessorMdbSet<'a, 'b> {
    fn send_block(&mut self, block: &crate::SendBlock) {
        self.fill_value(block);
    }

    fn receive_block(&mut self, block: &crate::ReceiveBlock) {
        self.fill_value(block);
    }

    fn open_block(&mut self, _block: &crate::OpenBlock) {
        // Open blocks don't have a predecessor
    }

    fn change_block(&mut self, block: &crate::ChangeBlock) {
        self.fill_value(block);
    }

    fn state_block(&mut self, block: &crate::StateBlock) {
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
    use super::*;
    use crate::{datastore::lmdb::TestLmdbEnv, BlockBuilder};

    #[test]
    fn block_not_found() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert!(store.get(&txn.as_txn(), &BlockHash::from(1)).is_none());
        assert_eq!(store.exists(&txn.as_txn(), &BlockHash::from(1)), false);
        Ok(())
    }

    #[test]
    fn add_block() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let block = BlockBuilder::open().build()?;
        let block_hash = block.hash();

        store.put(&mut txn, &block_hash, &block);
        let loaded = store
            .get(&txn.as_txn(), &block.hash())
            .expect("block not found");

        assert_eq!(loaded, BlockEnum::Open(block));
        assert!(store.exists(&txn.as_txn(), &block_hash));
        Ok(())
    }

    #[test]
    fn clear_successor() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;

        let mut block1 = BlockBuilder::open()
            .account(Account::from(0))
            .representative(Account::from(1))
            .build()?;

        let block2 = BlockBuilder::open()
            .account(Account::from(0))
            .representative(Account::from(2))
            .build()?;

        let mut sideband = block1.sideband().unwrap().clone();
        sideband.successor = block2.hash();
        block1.set_sideband(sideband);

        store.put(&mut txn, &block2.hash(), &block2);
        store.put(&mut txn, &block1.hash(), &block1);

        store.successor_clear(&mut txn, &block1.hash());

        let loaded = store
            .get(&txn.as_txn(), &block1.hash())
            .expect("block not found");
        assert_eq!(
            loaded.as_block().sideband().unwrap().successor,
            *BlockHash::zero()
        );
        Ok(())
    }

    #[test]
    fn add_two_blocks() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbBlockStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let block1 = BlockBuilder::open().build()?;
        let block2 = BlockBuilder::open().build()?;

        store.put(&mut txn, &block1.hash(), &block1);
        store.put(&mut txn, &block2.hash(), &block2);
        let loaded1 = store
            .get(&txn.as_txn(), &block1.hash())
            .expect("block1 not found");
        let loaded2 = store
            .get(&txn.as_txn(), &block2.hash())
            .expect("block2 not found");

        assert_eq!(loaded1, BlockEnum::Open(block1));
        assert_eq!(loaded2, BlockEnum::Open(block2));
        Ok(())
    }
}
