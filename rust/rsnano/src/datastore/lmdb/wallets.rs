use std::ptr;

use crate::{datastore::DbIterator, BlockHash, NoValue, RawKey, WalletId};

use super::{
    assert_success, ensure_success, mdb_cursor_get, mdb_cursor_open, mdb_dbi_open, mdb_drop,
    mdb_get, mdb_put, LmdbIterator, LmdbStore, MdbCursorOp, MdbVal, Transaction, MDB_CREATE,
    MDB_NOTFOUND, MDB_SUCCESS,
};

pub struct LmdbWallets {
    pub handle: u32,
    pub send_action_ids_handle: u32,
}

impl LmdbWallets {
    pub fn new() -> Self {
        Self {
            handle: 0,
            send_action_ids_handle: 0,
        }
    }

    pub fn initialize(&mut self, txn: &Transaction, store: &LmdbStore) -> anyhow::Result<()> {
        let status = unsafe { mdb_dbi_open(txn.handle(), None, MDB_CREATE, &mut self.handle) };
        ensure_success(status)?;
        self.split_if_needed(txn, store)?;

        let status = unsafe {
            mdb_dbi_open(
                txn.handle(),
                Some("send_action_ids"),
                MDB_CREATE,
                &mut self.send_action_ids_handle,
            )
        };
        ensure_success(status)
    }

    pub fn get_store_it(
        &self,
        txn: &Transaction,
        hash: &str,
    ) -> Box<dyn DbIterator<[u8; 64], NoValue>> {
        let hash_bytes: [u8; 64] = hash.as_bytes().try_into().unwrap();
        Box::new(LmdbIterator::new(
            &txn,
            self.handle,
            Some(&hash_bytes),
            true,
        ))
    }

    pub fn move_table(
        &self,
        name: &str,
        txn_source: &Transaction,
        txn_destination: &Transaction,
    ) -> anyhow::Result<()> {
        let mut handle_source = 0;
        let error2 = unsafe {
            mdb_dbi_open(
                txn_source.handle(),
                Some(name),
                MDB_CREATE,
                &mut handle_source,
            )
        };
        ensure_success(error2)?;
        let mut handle_destination = 0;
        let error3 = unsafe {
            mdb_dbi_open(
                txn_destination.handle(),
                Some(name),
                MDB_CREATE,
                &mut handle_destination,
            )
        };
        ensure_success(error3)?;
        let mut cursor = ptr::null_mut();
        let error4 = unsafe { mdb_cursor_open(txn_source.handle(), handle_source, &mut cursor) };
        ensure_success(error4)?;
        let mut val_key = MdbVal::new();
        let mut val_value = MdbVal::new();
        let mut cursor_status =
            unsafe { mdb_cursor_get(cursor, &mut val_key, &mut val_value, MdbCursorOp::MdbFirst) };
        while cursor_status == MDB_SUCCESS {
            let error5 = unsafe {
                mdb_put(
                    txn_destination.handle(),
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
        let error6 = unsafe { mdb_drop(txn_source.handle(), handle_source, 1) };
        ensure_success(error6)
    }

    pub fn split_if_needed(
        &self,
        txn_destination: &Transaction,
        store: &LmdbStore,
    ) -> anyhow::Result<()> {
        let beginning = RawKey::from(0).encode_hex();
        let end = RawKey::from_bytes([1; 32]).encode_hex();

        // First do a read pass to check if there are any wallets that need extracting (to save holding a write lock and potentially being blocked)
        let wallets_need_splitting = {
            let transaction_source = store.env.tx_begin_read();
            let i = self.get_store_it(&transaction_source.as_txn(), &beginning);
            let n = self.get_store_it(&transaction_source.as_txn(), &end);
            i.current().map(|(k, _)| *k) != n.current().map(|(k, _)| *k)
        };

        if wallets_need_splitting {
            let txn_source = store.env.tx_begin_write();
            let mut i = self.get_store_it(&txn_source.as_txn(), &beginning);
            while let Some((k, _)) = i.current() {
                let text = std::str::from_utf8(k)?;
                let _id = WalletId::decode_hex(text)?;
                self.move_table(text, &txn_source.as_txn(), txn_destination)?;
                i.next();
            }
        }
        Ok(())
    }

    pub fn get_wallet_ids(&self, txn: &Transaction) -> Vec<WalletId> {
        let mut wallet_ids = Vec::new();
        let beginning = RawKey::from(0).encode_hex();
        let mut i = self.get_store_it(txn, &beginning);
        while let Some((k, _)) = i.current() {
            let text = std::str::from_utf8(k).unwrap();
            wallet_ids.push(WalletId::decode_hex(&text).unwrap());
            i.next();
        }
        wallet_ids
    }

    pub fn get_block_hash(&self, txn: Transaction, id: &str) -> anyhow::Result<Option<BlockHash>> {
        let mut id_mdb_val = MdbVal::from(id);
        let mut result = MdbVal::new();
        let status = unsafe {
            mdb_get(
                txn.handle(),
                self.send_action_ids_handle,
                &mut id_mdb_val,
                &mut result,
            )
        };
        if status == MDB_SUCCESS {
            Ok(Some(BlockHash::try_from(&result)?))
        } else if status == MDB_NOTFOUND {
            Ok(None)
        } else {
            Err(anyhow!("get block hash failed"))
        }
    }

    pub fn set_block_hash(
        &self,
        txn: Transaction,
        id: &str,
        hash: &BlockHash,
    ) -> anyhow::Result<()> {
        let mut id_mdb_val = MdbVal::from(id);
        let status = unsafe {
            mdb_put(
                txn.handle(),
                self.send_action_ids_handle,
                &mut id_mdb_val,
                &mut MdbVal::from(hash),
                0,
            )
        };
        ensure_success(status)
    }

    pub fn clear_send_ids(&self, txn: Transaction) {
        let status = unsafe { mdb_drop(txn.handle(), self.send_action_ids_handle, 0) };
        assert_success(status);
    }
}
