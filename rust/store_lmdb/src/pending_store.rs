use std::sync::Arc;

use crate::{
    iterator::DbIterator, parallel_traversal_u512, Environment, EnvironmentWrapper, LmdbEnv,
    LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction, RwTransaction, Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::{Deserialize, StreamAdapter},
    Account, BlockHash, PendingInfo, PendingKey,
};

pub type PendingIterator = Box<dyn DbIterator<PendingKey, PendingInfo>>;

pub struct LmdbPendingStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbPendingStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pending"), DatabaseFlags::empty())?;

        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(&self, txn: &mut LmdbWriteTransaction<T>, key: &PendingKey, pending: &PendingInfo) {
        let key_bytes = key.to_bytes();
        let pending_bytes = pending.to_bytes();
        txn
            .put(
                self.database,
                &key_bytes,
                &pending_bytes,
                WriteFlags::empty(),
            )
            .unwrap();
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, key: &PendingKey) {
        let key_bytes = key.to_bytes();
        txn.rw_txn_mut()
            .del(self.database, &key_bytes, None)
            .unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> Option<PendingInfo> {
        let key_bytes = key.to_bytes();
        match txn.get(self.database, &key_bytes) {
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                PendingInfo::deserialize(&mut stream).ok()
            }
            Err(lmdb::Error::NotFound) => None,
            Err(e) => {
                panic!("Could not load pending info: {:?}", e);
            }
        }
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> PendingIterator {
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, None, true)
    }

    pub fn begin_at_key(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> PendingIterator {
        let key_bytes = key.to_bytes();
        LmdbIteratorImpl::<T>::new_iterator(txn, self.database, Some(&key_bytes), true)
    }

    pub fn exists(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        key: &PendingKey,
    ) -> bool {
        let iterator = self.begin_at_key(txn, key);
        iterator.current().map(|(k, _)| k == key).unwrap_or(false)
    }

    pub fn any(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        account: &Account,
    ) -> bool {
        let key = PendingKey::new(*account, BlockHash::zero());
        let iterator = self.begin_at_key(txn, &key);
        iterator
            .current()
            .map(|(k, _)| k.account == *account)
            .unwrap_or(false)
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, PendingIterator, PendingIterator) + Send + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_key(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_key(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }

    pub fn end(&self) -> PendingIterator {
        LmdbIteratorImpl::<T>::null_iterator()
    }
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;
    use rsnano_core::{Amount, Epoch};

    use super::*;

    #[test]
    fn not_found() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPendingStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        let result = store.get(&txn, &test_key());
        assert!(result.is_none());
        assert_eq!(store.exists(&txn, &test_key()), false);
        Ok(())
    }

    #[test]
    fn add_pending() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPendingStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let pending_key = test_key();
        let pending = test_pending_info();
        store.put(&mut txn, &pending_key, &pending);
        let result = store.get(&txn, &pending_key);
        assert_eq!(result, Some(pending));
        assert!(store.exists(&txn, &pending_key));
        Ok(())
    }

    #[test]
    fn delete() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPendingStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let pending_key = test_key();
        let pending = test_pending_info();
        store.put(&mut txn, &pending_key, &pending);
        store.del(&mut txn, &pending_key);
        let result = store.get(&txn, &pending_key);
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn iter_empty() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPendingStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert!(store.begin(&txn).is_end());
        Ok(())
    }

    #[test]
    fn iter() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbPendingStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let pending_key = test_key();
        let pending = test_pending_info();
        store.put(&mut txn, &pending_key, &pending);

        let mut it = store.begin(&txn);
        assert_eq!(it.is_end(), false);
        let (k, v) = it.current().unwrap();
        assert_eq!(k, &pending_key);
        assert_eq!(v, &pending);

        it.next();
        assert!(it.is_end());
        Ok(())
    }

    fn test_key() -> PendingKey {
        PendingKey::new(Account::from(1), BlockHash::from(2))
    }

    fn test_pending_info() -> PendingInfo {
        PendingInfo::new(Account::from(3), Amount::raw(4), Epoch::Epoch2)
    }
}
