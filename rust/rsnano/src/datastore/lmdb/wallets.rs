use std::ptr;

use crate::{
    datastore::{DbIterator, Transaction},
    utils::{Deserialize, Serialize, Stream},
    NoValue,
};

use super::{
    ensure_success, get_raw_lmdb_txn, mdb_cursor_get, mdb_cursor_open, mdb_dbi_open, mdb_drop,
    mdb_put, LmdbIterator, MdbCursorOp, MdbVal, MDB_CREATE, MDB_SUCCESS,
};

pub struct LmdbWallets {
    pub handle: u32,
}

impl LmdbWallets {
    pub fn new() -> Self {
        Self { handle: 0 }
    }

    pub fn initialize(&mut self, txn: &dyn Transaction) -> anyhow::Result<()> {
        let status =
            unsafe { mdb_dbi_open(get_raw_lmdb_txn(txn), None, MDB_CREATE, &mut self.handle) };
        ensure_success(status)
    }

    pub fn get_store_it(
        &self,
        txn: &dyn Transaction,
        hash: &str,
    ) -> Box<dyn DbIterator<[u8; 64], NoValue>> {
        let hash_bytes: [u8; 64] = hash.as_bytes().try_into().unwrap();
        Box::new(LmdbIterator::new(txn, self.handle, Some(&hash_bytes), true))
    }

    pub fn move_table(
        &self,
        name: &str,
        txn_source: &dyn Transaction,
        txn_destination: &dyn Transaction,
    ) -> anyhow::Result<()> {
        let mut handle_source = 0;
        let error2 = unsafe {
            mdb_dbi_open(
                get_raw_lmdb_txn(txn_source),
                Some(name),
                MDB_CREATE,
                &mut handle_source,
            )
        };
        ensure_success(error2)?;
        let mut handle_destination = 0;
        let error3 = unsafe {
            mdb_dbi_open(
                get_raw_lmdb_txn(txn_destination),
                Some(name),
                MDB_CREATE,
                &mut handle_destination,
            )
        };
        ensure_success(error3)?;
        let mut cursor = ptr::null_mut();
        let error4 =
            unsafe { mdb_cursor_open(get_raw_lmdb_txn(txn_source), handle_source, &mut cursor) };
        ensure_success(error4)?;
        let mut val_key = MdbVal::new();
        let mut val_value = MdbVal::new();
        let mut cursor_status =
            unsafe { mdb_cursor_get(cursor, &mut val_key, &mut val_value, MdbCursorOp::MdbFirst) };
        while cursor_status == MDB_SUCCESS {
            let error5 = unsafe {
                mdb_put(
                    get_raw_lmdb_txn(txn_destination),
                    handle_destination,
                    &mut val_key,
                    &mut val_value,
                    0,
                )
            };
            ensure_success(error5)?;
            cursor_status = unsafe {
                mdb_cursor_get(cursor, &mut val_key, &mut val_value, MdbCursorOp::MdbNext)
            };
        }
        let error6 = unsafe { mdb_drop(get_raw_lmdb_txn(txn_source), handle_source, 1) };
        ensure_success(error6)
    }
}

impl Serialize for [u8; 64] {
    fn serialized_size() -> usize {
        64
    }

    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        stream.write_bytes(self)
    }
}

impl Deserialize for [u8; 64] {
    type Target = Self;

    fn deserialize(stream: &mut dyn Stream) -> anyhow::Result<Self::Target> {
        let mut buffer = [0; 64];
        stream.read_bytes(&mut buffer, 64)?;
        Ok(buffer)
    }
}
