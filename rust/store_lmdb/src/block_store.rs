use crate::{
    iterator::DbIterator, parallel_traversal, ConfiguredDatabase, Environment, EnvironmentStub,
    EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction,
    Transaction, BLOCK_TEST_DATABASE,
};
use lmdb::{DatabaseFlags, WriteFlags};
use num_traits::FromPrimitive;
use rsnano_core::{
    deserialize_block_enum,
    utils::{OutputListenerMt, OutputTrackerMt, Serialize, StreamAdapter},
    Account, Amount, Block, BlockEnum, BlockHash, BlockSideband, BlockType, BlockVisitor,
    BlockWithSideband, ChangeBlock, Epoch, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
};
use std::sync::Arc;

pub type BlockIterator = Box<dyn DbIterator<BlockHash, BlockWithSideband>>;

pub struct LmdbBlockStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
    #[cfg(feature = "output_tracking")]
    put_listener: OutputListenerMt<BlockEnum>,
}

impl LmdbBlockStore<EnvironmentStub> {
    pub fn configured_responses() -> ConfiguredBlockDatabaseBuilder {
        ConfiguredBlockDatabaseBuilder::new()
    }
}

pub struct ConfiguredBlockDatabaseBuilder {
    database: ConfiguredDatabase,
}

impl ConfiguredBlockDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            database: ConfiguredDatabase::new(BLOCK_TEST_DATABASE, "blocks"),
        }
    }

    pub fn block(mut self, block: &BlockEnum) -> Self {
        self.database.entries.insert(
            block.hash().as_bytes().to_vec(),
            block.serialize_with_sideband(),
        );
        self
    }

    pub fn build(self) -> ConfiguredDatabase {
        self.database
    }
}

impl<T: Environment + 'static> LmdbBlockStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("blocks"), DatabaseFlags::empty())?;
        Ok(Self {
            env,
            database,
            #[cfg(feature = "output_tracking")]
            put_listener: OutputListenerMt::new(),
        })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_puts(&self) -> Arc<OutputTrackerMt<BlockEnum>> {
        self.put_listener.track()
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, block: &BlockEnum) {
        #[cfg(feature = "output_tracking")]
        self.put_listener.emit(block.clone());

        let hash = block.hash();
        debug_assert!(
            block.sideband().unwrap().successor.is_zero()
                || self.exists(txn, &block.sideband().unwrap().successor)
        );

        self.raw_put(txn, &block.serialize_with_sideband(), &hash);
        {
            let mut predecessor = BlockPredecessorMdbSet::new(txn, self);
            block.visit(&mut predecessor);
        }
    }

    pub fn exists(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> bool {
        self.block_raw_get(transaction, hash).is_some()
    }

    pub fn successor(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<BlockHash> {
        self.block_raw_get(txn, hash).and_then(|data| {
            debug_assert!(data.len() >= 32);
            let block_type = BlockType::from_u8(data[0]).unwrap();
            let offset = block_successor_offset(data.len(), block_type);
            let successor = BlockHash::from_bytes(data[offset..offset + 32].try_into().unwrap());
            if successor.is_zero() {
                None
            } else {
                Some(successor)
            }
        })
    }

    pub fn successor_clear(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        let value = self.block_raw_get(txn, hash).unwrap();
        let block_type = BlockType::from_u8(value[0]).unwrap();

        let mut data = value.to_vec();
        let offset = block_successor_offset(value.len(), block_type);
        data[offset..offset + BlockHash::serialized_size()].fill(0);
        self.raw_put(txn, &data, hash)
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<BlockEnum> {
        self.block_raw_get(txn, hash).map(|bytes| {
            BlockEnum::deserialize_with_sideband(bytes)
                .unwrap_or_else(|_| panic!("Could not deserialize block {}!", hash))
        })
    }

    pub fn get_no_sideband(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<BlockEnum> {
        match self.block_raw_get(txn, hash) {
            None => None,
            Some(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                Some(deserialize_block_enum(&mut stream).unwrap())
            }
        }
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        txn.delete(self.database, hash.as_bytes(), None).unwrap();
    }

    pub fn count(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> u64 {
        txn.count(self.database)
    }

    pub fn account(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Account> {
        let block = self.get(txn, hash)?;
        Some(block.account_calculated())
    }

    pub fn begin(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> BlockIterator {
        LmdbIteratorImpl::<T>::new_iterator(transaction, self.database, None, true)
    }

    pub fn begin_at_hash(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> BlockIterator {
        LmdbIteratorImpl::<T>::new_iterator(transaction, self.database, Some(hash.as_bytes()), true)
    }

    pub fn end(&self) -> BlockIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }

    pub fn random(
        &self,
        transaction: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Option<BlockEnum> {
        let hash = BlockHash::random();
        let mut existing = self.begin_at_hash(transaction, &hash);
        if existing.is_end() {
            existing = self.begin(transaction);
        }

        existing.current().map(|(_, v)| v.block.clone())
    }

    pub fn balance(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Amount {
        match self.get(txn, hash) {
            Some(block) => block.balance_calculated(),
            None => Amount::zero(),
        }
    }

    pub fn version(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Epoch {
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

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, BlockIterator, BlockIterator) + Send + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read();
            let begin_it = self.begin_at_hash(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_hash(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    pub fn account_height(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> u64 {
        match self.get(txn, hash) {
            Some(block) => block.sideband().unwrap().height,
            None => 0,
        }
    }

    pub fn raw_put(&self, txn: &mut LmdbWriteTransaction<T>, data: &[u8], hash: &BlockHash) {
        txn.put(self.database, hash.as_bytes(), data, WriteFlags::empty())
            .unwrap();
    }

    pub fn block_raw_get<'a>(
        &self,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<&'a [u8]> {
        match txn.get(self.database, hash.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => Some(bytes),
            Err(e) => panic!("Could not load block. {:?}", e),
        }
    }
}

/// Fill in our predecessors
struct BlockPredecessorMdbSet<'a, T: Environment + 'static> {
    transaction: &'a mut LmdbWriteTransaction<T>,
    block_store: &'a LmdbBlockStore<T>,
}

impl<'a, T: Environment + 'static> BlockPredecessorMdbSet<'a, T> {
    fn new(
        transaction: &'a mut LmdbWriteTransaction<T>,
        block_store: &'a LmdbBlockStore<T>,
    ) -> Self {
        Self {
            transaction,
            block_store,
        }
    }

    fn fill_value(&mut self, block: &dyn Block) {
        let hash = block.hash();
        let value = self
            .block_store
            .block_raw_get(self.transaction, &block.previous())
            .expect("block not found by fill_value");
        let mut data = value.to_vec();
        let block_type = BlockType::from_u8(data[0]).unwrap();

        let offset = block_successor_offset(data.len(), block_type);
        data[offset..offset + hash.as_bytes().len()].copy_from_slice(hash.as_bytes());

        self.block_store
            .raw_put(self.transaction, &data, &block.previous());
    }
}

impl<'a, T: Environment> BlockVisitor for BlockPredecessorMdbSet<'a, T> {
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
    use crate::lmdb_env::DatabaseStub;
    use crate::{EnvironmentStub, PutEvent};
    use rsnano_core::BlockBuilder;

    use super::*;

    struct Fixture {
        env: Arc<LmdbEnv<EnvironmentStub>>,
        store: LmdbBlockStore<EnvironmentStub>,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_env(LmdbEnv::create_null())
        }

        fn with_env(env: LmdbEnv<EnvironmentStub>) -> Self {
            let env = Arc::new(env);
            Self {
                env: env.clone(),
                store: LmdbBlockStore::new(env).unwrap(),
            }
        }
    }

    #[test]
    fn empty() {
        let fixture = Fixture::new();
        let store = &fixture.store;
        let txn = fixture.env.tx_begin_read();

        assert!(store.get(&txn, &BlockHash::from(1)).is_none());
        assert_eq!(store.exists(&txn, &BlockHash::from(1)), false);
        assert_eq!(store.count(&txn), 0);
    }

    #[test]
    fn load_block_by_hash() {
        let block = BlockBuilder::legacy_open().with_sideband().build();

        let env = LmdbEnv::create_null_with()
            .database("blocks", DatabaseStub(100))
            .entry(block.hash().as_bytes(), &block.serialize_with_sideband())
            .build()
            .build();
        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let result = fixture.store.get(&txn, &block.hash());
        assert_eq!(result, Some(block));
    }

    #[test]
    fn add_block() {
        let fixture = Fixture::new();
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();
        let block = BlockBuilder::legacy_open().with_sideband().build();

        fixture.store.put(&mut txn, &block);

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: Default::default(),
                key: block.hash().as_bytes().to_vec(),
                value: block.serialize_with_sideband(),
                flags: lmdb::WriteFlags::empty(),
            }]
        );
    }

    #[test]
    fn clear_successor() {
        let mut block = BlockBuilder::legacy_open().build();
        let sideband = BlockSideband {
            successor: BlockHash::from(123),
            ..BlockSideband::create_test_instance()
        };
        block.set_sideband(sideband.clone());

        let env = LmdbEnv::create_null_with()
            .database("blocks", DatabaseStub(100))
            .entry(block.hash().as_bytes(), &block.serialize_with_sideband())
            .build()
            .build();
        let fixture = Fixture::with_env(env);
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = txn.track_puts();

        fixture.store.successor_clear(&mut txn, &block.hash());

        let mut expected_block = block.clone();
        expected_block.set_sideband(BlockSideband {
            successor: BlockHash::zero(),
            ..sideband
        });

        assert_eq!(
            put_tracker.output(),
            vec![PutEvent {
                database: DatabaseStub(100),
                key: expected_block.hash().as_bytes().to_vec(),
                value: expected_block.serialize_with_sideband(),
                flags: WriteFlags::empty(),
            }]
        );
    }

    #[test]
    fn random() -> anyhow::Result<()> {
        let block = BlockBuilder::legacy_open().with_sideband().build();

        let env = LmdbEnv::create_null_with()
            .database("blocks", DatabaseStub(100))
            .entry(block.hash().as_bytes(), &block.serialize_with_sideband())
            .build()
            .build();

        let fixture = Fixture::with_env(env);
        let txn = fixture.env.tx_begin_read();

        let random = fixture.store.random(&txn).expect("block not found");

        assert_eq!(random, block);
        Ok(())
    }

    #[test]
    fn track_inserted_blocks() {
        let fixture = Fixture::new();
        let mut block = BlockBuilder::state().previous(BlockHash::zero()).build();
        block.set_sideband(BlockSideband {
            height: 1,
            successor: BlockHash::zero(),
            ..BlockSideband::create_test_instance()
        });
        let mut txn = fixture.env.tx_begin_write();
        let put_tracker = fixture.store.track_puts();

        fixture.store.put(&mut txn, &block);

        assert_eq!(put_tracker.output(), vec![block]);
    }

    #[test]
    fn can_be_nulled() {
        let block = BlockBuilder::state().with_sideband().build();
        let configured_responses = LmdbBlockStore::configured_responses().block(&block).build();
        let env = LmdbEnv::create_null_with()
            .configured_database(configured_responses)
            .build();
        let txn = env.tx_begin_read();
        let block_store = LmdbBlockStore::new(Arc::new(env)).unwrap();
        assert_eq!(block_store.get(&txn, &block.hash()), Some(block));
    }
}
