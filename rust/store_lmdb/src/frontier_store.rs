use crate::{
    iterator::DbIterator, lmdb_env::RwTransaction, parallel_traversal, ConfiguredDatabase,
    DatabaseStub, Environment, EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction,
    LmdbWriteTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::utils::{OutputListenerMt, OutputTrackerMt};
use rsnano_core::{Account, BlockHash};
use std::sync::Arc;

pub type FrontierIterator = Box<dyn DbIterator<BlockHash, Account>>;

pub struct ConfiguredFrontierDatabaseBuilder {
    database: ConfiguredDatabase,
}

impl ConfiguredFrontierDatabaseBuilder {
    pub fn new() -> Self {
        Self {
            database: ConfiguredDatabase::new(DatabaseStub(2), "frontiers"),
        }
    }

    pub fn frontier(mut self, hash: &BlockHash, account: &Account) -> Self {
        self.database
            .entries
            .insert(hash.as_bytes().to_vec(), account.as_bytes().to_vec());
        self
    }

    pub fn build(self) -> ConfiguredDatabase {
        self.database
    }
}

pub struct LmdbFrontierStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
    #[cfg(feature = "output_tracking")]
    delete_listener: OutputListenerMt<BlockHash>,
}

impl<T: Environment + 'static> LmdbFrontierStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("frontiers"), DatabaseFlags::empty())?;
        Ok(Self {
            env,
            database,
            #[cfg(feature = "output_tracking")]
            delete_listener: OutputListenerMt::new(),
        })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn create_db(&self) -> anyhow::Result<()> {
        Ok(())
    }

    #[cfg(feature = "output_tracking")]
    pub fn track_deletions(&self) -> Arc<OutputTrackerMt<BlockHash>> {
        self.delete_listener.track()
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash, account: &Account) {
        txn.rw_txn_mut()
            .put(
                self.database,
                hash.as_bytes(),
                account.as_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> Option<Account> {
        match txn.get(self.database, hash.as_bytes()) {
            Ok(bytes) => Some(Account::from_slice(bytes).unwrap()),
            Err(lmdb::Error::NotFound) => None,
            Err(e) => panic!("Could not load frontier: {:?}", e),
        }
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, hash: &BlockHash) {
        #[cfg(feature = "output_tracking")]
        self.delete_listener.emit(hash.clone());
        txn.delete(self.database, hash.as_bytes(), None).unwrap();
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> FrontierIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &BlockHash,
    ) -> FrontierIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(hash.as_bytes()), true)
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, FrontierIterator, FrontierIterator)
              + Send
              + Sync),
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

    pub fn end(&self) -> FrontierIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestLmdbEnv;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbFrontierStore::new(env.env())?;
        let txn = env.tx_begin_read();
        assert_eq!(store.get(&txn, &BlockHash::from(1)), None);
        assert!(store.begin(&txn).is_end());
        Ok(())
    }

    #[test]
    fn put() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbFrontierStore::new(env.env())?;
        let mut txn = env.tx_begin_write();
        let block = BlockHash::from(1);
        let account = Account::from(2);

        store.put(&mut txn, &block, &account);
        let loaded = store.get(&txn, &block);

        assert_eq!(loaded, Some(account));
        Ok(())
    }

    #[test]
    fn delete() {
        let env = TestLmdbEnv::new();
        let store = LmdbFrontierStore::new(env.env()).unwrap();
        let mut txn = env.tx_begin_write();
        let block = BlockHash::from(1);
        store.put(&mut txn, &block, &Account::from(2));
        let delete_tracker = store.track_deletions();

        store.del(&mut txn, &block);

        let loaded = store.get(&txn, &block);
        assert_eq!(loaded, None);
        assert_eq!(delete_tracker.output(), vec![block]);
    }

    #[test]
    fn can_be_nulled() {
        let hash = BlockHash::from(1);
        let account = Account::from(2);
        let env = LmdbEnv::create_null_with()
            .configured_database(
                ConfiguredFrontierDatabaseBuilder::new()
                    .frontier(&hash, &account)
                    .build(),
            )
            .build();
        let txn = env.tx_begin_read();
        let store = LmdbFrontierStore::new(Arc::new(env)).unwrap();

        assert_eq!(store.get(&txn, &hash), Some(account));
    }
}
