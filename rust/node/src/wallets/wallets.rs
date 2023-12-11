use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use lmdb::{DatabaseFlags, WriteFlags};
use rsnano_core::{Account, BlockHash, NoValue, RawKey, WalletId};
use rsnano_store_lmdb::{
    BinaryDbIterator, DbIterator, Environment, EnvironmentWrapper, LmdbEnv, LmdbIteratorImpl,
    LmdbWriteTransaction, RwTransaction, Transaction,
};

use super::Wallet;
pub type WalletsIterator<T> = BinaryDbIterator<[u8; 64], NoValue, LmdbIteratorImpl<T>>;

pub struct Wallets<T: Environment = EnvironmentWrapper> {
    pub handle: Option<T::Database>,
    pub send_action_ids_handle: Option<T::Database>,
    enable_voting: bool,
    env: Arc<LmdbEnv<T>>,
    pub mutex: Mutex<HashMap<WalletId, Arc<Wallet>>>,
}

impl<T: Environment + 'static> Wallets<T> {
    pub fn new(enable_voting: bool, env: Arc<LmdbEnv<T>>) -> anyhow::Result<Self> {
        let mut wallets = Self {
            handle: None,
            send_action_ids_handle: None,
            enable_voting,
            mutex: Mutex::new(HashMap::new()),
            env,
        };
        //let mut txn = wallets.env.tx_begin_write();
        //wallets.initialize(&mut txn)?;
        //let wallet_ids = wallets.get_wallet_ids(&txn);
        //for id in wallet_ids {
        //    assert!()
        //}

        Ok(wallets)
    }

    pub fn initialize(&mut self, txn: &mut LmdbWriteTransaction<T>) -> anyhow::Result<()> {
        self.handle = Some(unsafe { txn.rw_txn_mut().create_db(None, DatabaseFlags::empty())? });
        self.send_action_ids_handle = Some(unsafe {
            txn.rw_txn_mut()
                .create_db(Some("send_action_ids"), DatabaseFlags::empty())?
        });
        Ok(())
    }

    pub fn get_store_it(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        hash: &str,
    ) -> WalletsIterator<T> {
        let hash_bytes: [u8; 64] = hash.as_bytes().try_into().unwrap();
        WalletsIterator::new(LmdbIteratorImpl::<T>::new(
            txn,
            self.handle.unwrap(),
            Some(&hash_bytes),
            true,
        ))
    }

    pub fn get_wallet_ids(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    ) -> Vec<WalletId> {
        let mut wallet_ids = Vec::new();
        let beginning = RawKey::from(0).encode_hex();
        let mut i = self.get_store_it(txn, &beginning);
        while let Some((k, _)) = i.current() {
            let text = std::str::from_utf8(k).unwrap();
            wallet_ids.push(WalletId::decode_hex(text).unwrap());
            i.next();
        }
        wallet_ids
    }

    pub fn get_block_hash(
        &self,
        txn: &dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        id: &str,
    ) -> anyhow::Result<Option<BlockHash>> {
        match txn.get(self.send_action_ids_handle.unwrap(), id.as_bytes()) {
            Ok(bytes) => Ok(Some(
                BlockHash::from_slice(bytes).ok_or_else(|| anyhow!("invalid block hash"))?,
            )),
            Err(lmdb::Error::NotFound) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn set_block_hash(
        &self,
        txn: &mut LmdbWriteTransaction<T>,
        id: &str,
        hash: &BlockHash,
    ) -> anyhow::Result<()> {
        txn.rw_txn_mut().put(
            self.send_action_ids_handle.unwrap(),
            id.as_bytes(),
            hash.as_bytes(),
            WriteFlags::empty(),
        )?;
        Ok(())
    }

    pub fn clear_send_ids(&self, txn: &mut LmdbWriteTransaction<T>) {
        txn.clear_db(self.send_action_ids_handle.unwrap()).unwrap();
    }

    pub fn foreach_representative<F>(&self, _f: F)
    where
        F: Fn(&Account, &RawKey),
    {
        if !self.enable_voting {
            return;
        }

        //let mut action_accounts = Vec::new();
        //{
        //    let txn = self.env.tx_begin_read();
        //}
        //TODO
        todo!()
    }
}
