use std::sync::{Arc, Mutex};

use primitive_types::U512;

use crate::{
    datastore::{parallel_traversal_u512, DbIterator, NullIterator, PendingStore},
    utils::{Deserialize, StreamAdapter},
    Account, BlockHash, PendingInfo, PendingKey,
};

use super::{
    assert_success, ensure_success, mdb_dbi_open, mdb_del, mdb_get, mdb_put, LmdbEnv, LmdbIterator,
    LmdbReadTransaction, LmdbWriteTransaction, MdbVal, Transaction, MDB_NOTFOUND, MDB_SUCCESS,
};

pub struct LmdbPendingStore {
    env: Arc<LmdbEnv>,
    db_handle: Mutex<u32>,
}

impl LmdbPendingStore {
    pub fn new(env: Arc<LmdbEnv>) -> Self {
        Self {
            env,
            db_handle: Mutex::new(0),
        }
    }

    pub fn db_handle(&self) -> u32 {
        *self.db_handle.lock().unwrap()
    }

    pub fn open_db(&self, txn: &Transaction, flags: u32) -> anyhow::Result<()> {
        let mut handle = 0;
        let status = unsafe { mdb_dbi_open(txn.handle(), Some("pending"), flags, &mut handle) };
        *self.db_handle.lock().unwrap() = handle;
        ensure_success(status)
    }
}

impl PendingStore<LmdbReadTransaction, LmdbWriteTransaction> for LmdbPendingStore {
    fn put(&self, txn: &LmdbWriteTransaction, key: &PendingKey, pending: &PendingInfo) {
        let key_bytes = key.to_bytes();
        let pending_bytes = pending.to_bytes();
        let status = unsafe {
            mdb_put(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                &mut MdbVal::from_slice(&pending_bytes),
                0,
            )
        };
        assert_success(status);
    }

    fn del(&self, txn: &LmdbWriteTransaction, key: &PendingKey) {
        let key_bytes = key.to_bytes();
        let status = unsafe {
            mdb_del(
                txn.handle,
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                None,
            )
        };
        assert_success(status);
    }

    fn get(&self, txn: &Transaction, key: &PendingKey) -> Option<PendingInfo> {
        let key_bytes = key.to_bytes();
        let mut value = MdbVal::new();
        let status = unsafe {
            mdb_get(
                txn.handle(),
                self.db_handle(),
                &mut MdbVal::from_slice(&key_bytes),
                &mut value,
            )
        };
        assert!(status == MDB_SUCCESS || status == MDB_NOTFOUND);
        if status == MDB_SUCCESS {
            let mut stream = StreamAdapter::new(value.as_slice());
            PendingInfo::deserialize(&mut stream).ok()
        } else {
            None
        }
    }

    fn begin(&self, txn: &Transaction) -> Box<dyn DbIterator<PendingKey, PendingInfo>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), None, true))
    }

    fn begin_at_key(
        &self,
        txn: &Transaction,
        key: &PendingKey,
    ) -> Box<dyn DbIterator<PendingKey, PendingInfo>> {
        Box::new(LmdbIterator::new(txn, self.db_handle(), Some(key), true))
    }

    fn exists(&self, txn: &Transaction, key: &PendingKey) -> bool {
        let iterator = self.begin_at_key(txn, key);
        iterator.current().map(|(k, _)| k == key).unwrap_or(false)
    }

    fn any(&self, txn: &Transaction, account: &Account) -> bool {
        let key = PendingKey::new(*account, BlockHash::new());
        let iterator = self.begin_at_key(txn, &key);
        iterator
            .current()
            .map(|(k, _)| k.account == *account)
            .unwrap_or(false)
    }

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &LmdbReadTransaction,
            &mut dyn DbIterator<PendingKey, PendingInfo>,
            &mut dyn DbIterator<PendingKey, PendingInfo>,
        ) + Send
              + Sync),
    ) {
        parallel_traversal_u512(&|start, end, is_last| {
            let mut transaction = self.env.tx_begin_read();
            let mut begin_it = self.begin_at_key(&transaction.as_txn(), &start.into());
            let mut end_it = if !is_last {
                self.begin_at_key(&transaction.as_txn(), &end.into())
            } else {
                self.end()
            };
            action(&mut transaction, begin_it.as_mut(), end_it.as_mut());
        });
    }

    fn end(&self) -> Box<dyn DbIterator<PendingKey, PendingInfo>> {
        Box::new(NullIterator::new())
    }
}

impl From<U512> for PendingKey {
    fn from(value: U512) -> Self {
        let mut buffer = [0; 64];
        value.to_big_endian(&mut buffer);
        PendingKey::new(
            Account::from_slice(&buffer[..32]).unwrap(),
            BlockHash::from_slice(&buffer[32..]).unwrap(),
        )
    }
}
