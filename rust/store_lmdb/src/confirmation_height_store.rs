use crate::{
    iterator::DbIterator, lmdb_env::RwTransaction2, parallel_traversal, Environment,
    EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbWriteTransaction,
    Transaction,
};
use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{
    utils::{Deserialize, StreamAdapter},
    Account, ConfirmationHeightInfo,
};
use std::sync::Arc;

pub type ConfirmationHeightIterator = Box<dyn DbIterator<Account, ConfirmationHeightInfo>>;

pub struct LmdbConfirmationHeightStore<T: Environment = EnvironmentWrapper> {
    env: Arc<LmdbEnv<T>>,
    database: T::Database,
}

impl<T: Environment + 'static> LmdbConfirmationHeightStore<T> {
    pub fn new(env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("confirmation_height"), DatabaseFlags::empty())?;
        Ok(Self { env, database })
    }

    pub fn database(&self) -> T::Database {
        self.database
    }

    pub fn put(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        account: &Account,
        info: &ConfirmationHeightInfo,
    ) {
        txn.rw_txn_mut()
            .put(
                self.database,
                account.as_bytes(),
                &info.to_bytes(),
                WriteFlags::empty(),
            )
            .unwrap();
    }

    pub fn get(
        &self,
        txn: &dyn Transaction<Database = T::Database>,
        account: &Account,
    ) -> Option<ConfirmationHeightInfo> {
        match txn.get(self.database, account.as_bytes()) {
            Err(lmdb::Error::NotFound) => None,
            Ok(bytes) => {
                let mut stream = StreamAdapter::new(bytes);
                ConfirmationHeightInfo::deserialize(&mut stream).ok()
            }
            Err(e) => {
                panic!("Could not load confirmation height info: {:?}", e);
            }
        }
    }

    pub fn exists(&self, txn: &dyn Transaction<Database = T::Database>, account: &Account) -> bool {
        txn.exists(self.database, account.as_bytes())
    }

    pub fn del(&self, txn: &mut LmdbWriteTransaction<T>, account: &Account) {
        txn.rw_txn_mut()
            .del(self.database, account.as_bytes(), None)
            .unwrap();
    }

    pub fn count(&self, txn: &dyn Transaction<Database = T::Database>) -> u64 {
        txn.count(self.database)
    }

    pub fn clear(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.rw_txn_mut().clear_db(self.database).unwrap()
    }

    pub fn begin(
        &self,
        txn: &dyn Transaction<Database = T::Database>,
    ) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::new_iterator::<T, _, _>(txn, self.database, None, true)
    }

    pub fn begin_at_account(
        &self,
        txn: &dyn Transaction<Database = T::Database>,
        account: &Account,
    ) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::new_iterator::<T, _, _>(
            txn,
            self.database,
            Some(account.as_bytes()),
            true,
        )
    }

    pub fn end(&self) -> ConfirmationHeightIterator {
        LmdbIteratorImpl::null_iterator()
    }

    pub fn for_each_par(
        &self,
        action: &(dyn Fn(&LmdbReadTransaction<T>, ConfirmationHeightIterator, ConfirmationHeightIterator)
              + Send
              + Sync),
    ) {
        parallel_traversal(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_account(&transaction, &start.into());
            let end_it = if !is_last {
                self.begin_at_account(&transaction, &end.into())
            } else {
                self.end()
            };
            action(&transaction, begin_it, end_it);
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::TestLmdbEnv;
    use rsnano_core::BlockHash;

    use super::*;

    #[test]
    fn empty_store() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let txn = env.tx_begin_read()?;
        assert!(store.get(&txn, &Account::from(0)).is_none());
        assert_eq!(store.exists(&txn, &Account::from(0)), false);
        assert!(store.begin(&txn).is_end());
        assert!(store.begin_at_account(&txn, &Account::from(0)).is_end());
        Ok(())
    }

    #[test]
    fn add_account() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let account = Account::from(1);
        let info = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        store.put(&mut txn, &account, &info);
        let loaded = store.get(&txn, &account);
        assert_eq!(loaded, Some(info));
        Ok(())
    }

    #[test]
    fn iterate_one_account() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let account = Account::from(1);
        let info = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        store.put(&mut txn, &account, &info);

        let mut it = store.begin(&txn);
        assert_eq!(it.current(), Some((&account, &info)));

        it.next();
        assert!(it.is_end());
        Ok(())
    }

    #[test]
    fn iterate_two_accounts() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let account1 = Account::from(1);
        let account2 = Account::from(2);
        let info1 = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        let info2 = ConfirmationHeightInfo::new(3, BlockHash::from(4));
        store.put(&mut txn, &account1, &info1);
        store.put(&mut txn, &account2, &info2);

        let mut it = store.begin(&txn);
        assert_eq!(it.current(), Some((&account1, &info1)));
        it.next();
        assert_eq!(it.current(), Some((&account2, &info2)));
        it.next();
        assert!(it.is_end());
        Ok(())
    }

    #[test]
    fn clear() -> anyhow::Result<()> {
        let env = TestLmdbEnv::new();
        let store = LmdbConfirmationHeightStore::new(env.env())?;
        let mut txn = env.tx_begin_write()?;
        let account1 = Account::from(1);
        let account2 = Account::from(2);
        let info1 = ConfirmationHeightInfo::new(1, BlockHash::from(2));
        let info2 = ConfirmationHeightInfo::new(3, BlockHash::from(4));
        store.put(&mut txn, &account1, &info1);
        store.put(&mut txn, &account2, &info2);

        store.clear(&mut txn);

        assert_eq!(store.count(&txn), 0);
        Ok(())
    }
}
