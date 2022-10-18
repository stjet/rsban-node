use super::{LmdbEnv, LmdbIteratorImpl, LmdbTransaction, LmdbWriteTransaction};
use crate::{
    core::{BlockHash, RawKey},
    datastore::DbIterator,
    NoValue, WalletId,
};
use lmdb::{Cursor, Database, DatabaseFlags, Transaction, WriteFlags};
pub type WalletsIterator = DbIterator<[u8; 64], NoValue, LmdbIteratorImpl>;

pub struct LmdbWallets {
    pub handle: Option<Database>,
    pub send_action_ids_handle: Option<Database>,
}

impl LmdbWallets {
    pub fn new() -> Self {
        Self {
            handle: None,
            send_action_ids_handle: None,
        }
    }

    pub fn initialize(
        &mut self,
        txn: &mut LmdbWriteTransaction,
        env: &LmdbEnv,
    ) -> anyhow::Result<()> {
        self.handle = Some(unsafe { txn.rw_txn_mut().create_db(None, DatabaseFlags::empty())? });
        self.split_if_needed(txn, env)?;
        self.send_action_ids_handle = Some(unsafe {
            txn.rw_txn_mut()
                .create_db(Some("send_action_ids"), DatabaseFlags::empty())?
        });
        Ok(())
    }

    pub fn get_store_it(&self, txn: &LmdbTransaction, hash: &str) -> WalletsIterator {
        let hash_bytes: [u8; 64] = hash.as_bytes().try_into().unwrap();
        WalletsIterator::new(LmdbIteratorImpl::new(
            txn,
            self.handle.unwrap(),
            Some(&hash_bytes),
            true,
        ))
    }

    pub fn move_table(
        &self,
        name: &str,
        txn_source: &mut LmdbWriteTransaction,
        txn_destination: &mut LmdbWriteTransaction,
    ) -> anyhow::Result<()> {
        let rw_txn_source = txn_source.rw_txn_mut();
        let rw_txn_dest = txn_destination.rw_txn_mut();
        let handle_source = unsafe { rw_txn_source.create_db(Some(name), DatabaseFlags::empty()) }?;
        let handle_destination =
            unsafe { rw_txn_dest.create_db(Some(name), DatabaseFlags::empty()) }?;

        {
            let mut cursor = rw_txn_source.open_ro_cursor(handle_source)?;
            for x in cursor.iter_start() {
                let (k, v) = x?;
                rw_txn_dest.put(handle_destination, &k, &v, WriteFlags::empty())?;
            }
        }

        unsafe { rw_txn_source.drop_db(handle_source) }?;
        Ok(())
    }

    pub fn split_if_needed(
        &self,
        txn_destination: &mut LmdbWriteTransaction,
        env: &LmdbEnv,
    ) -> anyhow::Result<()> {
        let beginning = RawKey::from(0).encode_hex();
        let end = RawKey::from_bytes([1; 32]).encode_hex();

        // First do a read pass to check if there are any wallets that need extracting (to save holding a write lock and potentially being blocked)
        let wallets_need_splitting = {
            let transaction_source = env.tx_begin_read()?;
            let i = self.get_store_it(&transaction_source.as_txn(), &beginning);
            let n = self.get_store_it(&transaction_source.as_txn(), &end);
            i.current().map(|(k, _)| *k) != n.current().map(|(k, _)| *k)
        };

        if wallets_need_splitting {
            let mut txn_source = env.tx_begin_write().unwrap();
            let mut i = self.get_store_it(&txn_source.as_txn(), &beginning);
            while let Some((k, _)) = i.current() {
                let text = std::str::from_utf8(k)?;
                let _id = WalletId::decode_hex(text)?;
                self.move_table(text, &mut txn_source, txn_destination)?;
                i.next();
            }
        }
        Ok(())
    }

    pub fn get_wallet_ids(&self, txn: &LmdbTransaction) -> Vec<WalletId> {
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

    pub fn get_block_hash(
        &self,
        txn: LmdbTransaction,
        id: &str,
    ) -> anyhow::Result<Option<BlockHash>> {
        match txn.get(self.send_action_ids_handle.unwrap(), &id.as_bytes()) {
            Ok(bytes) => Ok(Some(
                BlockHash::from_slice(bytes).ok_or_else(|| anyhow!("invalid block hash"))?,
            )),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_block_hash(
        &self,
        txn: &mut LmdbWriteTransaction,
        id: &str,
        hash: &BlockHash,
    ) -> anyhow::Result<()> {
        txn.rw_txn_mut().put(
            self.send_action_ids_handle.unwrap(),
            &id.as_bytes(),
            hash.as_bytes(),
            WriteFlags::empty(),
        )?;
        Ok(())
    }

    pub fn clear_send_ids(&self, txn: &mut LmdbWriteTransaction) {
        txn.rw_txn_mut()
            .clear_db(self.send_action_ids_handle.unwrap())
            .unwrap();
    }
}
