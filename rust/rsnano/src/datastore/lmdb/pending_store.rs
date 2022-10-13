use std::sync::Arc;

use lmdb::{Database, DatabaseFlags, WriteFlags};

use crate::{
    datastore::{parallel_traversal_u512, pending_store::PendingIterator, PendingStore},
    utils::{Deserialize, StreamAdapter},
    Account, BlockHash, PendingInfo, PendingKey,
};

use super::{
    LmdbEnv, LmdbIteratorImpl, LmdbReadTransaction, LmdbTransaction, LmdbWriteTransaction,
};

pub struct LmdbPendingStore {
    env: Arc<LmdbEnv>,
    database: Database,
}

impl LmdbPendingStore {
    pub fn new(env: Arc<LmdbEnv>) -> anyhow::Result<Self> {
        let database = env
            .environment
            .create_db(Some("pending"), DatabaseFlags::empty())?;

        Ok(Self { env, database })
    }

    pub fn database(&self) -> Database {
        self.database
    }
}

impl<'a> PendingStore<'a, LmdbReadTransaction<'a>, LmdbWriteTransaction<'a>, LmdbIteratorImpl>
    for LmdbPendingStore
{
    fn put(&self, txn: &mut LmdbWriteTransaction, key: &PendingKey, pending: &PendingInfo) {
        let key_bytes = key.to_bytes();
        let pending_bytes = pending.to_bytes();
        txn.rw_txn_mut()
            .put(
                self.database,
                &key_bytes,
                &pending_bytes,
                WriteFlags::empty(),
            )
            .unwrap();
    }

    fn del(&self, txn: &mut LmdbWriteTransaction, key: &PendingKey) {
        let key_bytes = key.to_bytes();
        txn.rw_txn_mut()
            .del(self.database, &key_bytes, None)
            .unwrap();
    }

    fn get(&self, txn: &LmdbTransaction, key: &PendingKey) -> Option<PendingInfo> {
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

    fn begin(&self, txn: &LmdbTransaction) -> PendingIterator<LmdbIteratorImpl> {
        PendingIterator::new(LmdbIteratorImpl::new(txn, self.database, None, true))
    }

    fn begin_at_key(
        &self,
        txn: &LmdbTransaction,
        key: &PendingKey,
    ) -> PendingIterator<LmdbIteratorImpl> {
        let key_bytes = key.to_bytes();
        PendingIterator::new(LmdbIteratorImpl::new(
            txn,
            self.database,
            Some(&key_bytes),
            true,
        ))
    }

    fn exists(&self, txn: &LmdbTransaction, key: &PendingKey) -> bool {
        let iterator = self.begin_at_key(txn, key);
        iterator.current().map(|(k, _)| k == key).unwrap_or(false)
    }

    fn any(&self, txn: &LmdbTransaction, account: &Account) -> bool {
        let key = PendingKey::new(*account, BlockHash::new());
        let iterator = self.begin_at_key(txn, &key);
        iterator
            .current()
            .map(|(k, _)| k.account == *account)
            .unwrap_or(false)
    }

    fn for_each_par(
        &'a self,
        action: &(dyn Fn(
            LmdbReadTransaction<'a>,
            PendingIterator<LmdbIteratorImpl>,
            PendingIterator<LmdbIteratorImpl>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let transaction = self.env.tx_begin_read().unwrap();
            let begin_it = self.begin_at_key(&transaction.as_txn(), &start.into());
            let end_it = if !is_last {
                self.begin_at_key(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(transaction, begin_it, end_it);
        });
    }

    fn end(&self) -> PendingIterator<LmdbIteratorImpl> {
        PendingIterator::new(LmdbIteratorImpl::null())
    }
}
