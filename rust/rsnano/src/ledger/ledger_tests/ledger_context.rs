use std::{sync::Arc, time::Duration};

use crate::{
    config::TxnTrackingConfig,
    core::{
        Account, Amount, Block, BlockBuilder, ChangeBlock, KeyPair, OpenBlock, ReceiveBlock,
        SendBlock, SignatureVerification,
    },
    ledger::{
        datastore::{
            lmdb::{EnvOptions, LmdbStore, TestDbFile},
            WriteTransaction,
        },
        GenerateCache, Ledger, ProcessResult, DEV_GENESIS_KEY,
    },
    stats::{Stat, StatConfig},
    utils::NullLogger,
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

pub(crate) struct LedgerContext {
    pub(crate) ledger: Ledger,
    db_file: TestDbFile,
}

impl LedgerContext {
    pub fn empty() -> Self {
        let db_file = TestDbFile::random();
        let store = Arc::new(
            LmdbStore::new(
                &db_file.path,
                &EnvOptions::default(),
                TxnTrackingConfig::default(),
                Duration::from_millis(5000),
                Arc::new(NullLogger::new()),
                false,
            )
            .unwrap(),
        );

        let ledger = Ledger::new(
            store.clone(),
            DEV_CONSTANTS.clone(),
            Arc::new(Stat::new(StatConfig::default())),
            &GenerateCache::new(),
        )
        .unwrap();

        let mut txn = store.tx_begin_write().unwrap();
        store.initialize(&mut txn, &ledger.cache, &DEV_CONSTANTS);

        LedgerContext { ledger, db_file }
    }

    pub fn process(&self, txn: &mut dyn WriteTransaction, block: &mut dyn Block) -> ProcessResult {
        let result = self
            .ledger
            .process(txn, block, SignatureVerification::Unknown);
        result.code
    }

    pub fn process_send_from_genesis(
        &self,
        txn: &mut dyn WriteTransaction,
        receiver_account: &Account,
        amount: Amount,
    ) -> SendBlock {
        let account_info = self
            .ledger
            .store
            .account()
            .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
            .unwrap();

        let mut send = BlockBuilder::send()
            .previous(account_info.head)
            .destination(*receiver_account)
            .balance(account_info.balance - amount)
            .sign(DEV_GENESIS_KEY.clone())
            .without_sideband()
            .build()
            .unwrap();

        assert_eq!(self.process(txn, &mut send), ProcessResult::Progress);
        send
    }

    pub fn process_open(
        &self,
        txn: &mut dyn WriteTransaction,
        send: &SendBlock,
        receiver_key: &KeyPair,
    ) -> OpenBlock {
        let receiver_account = receiver_key.public_key().into();
        let mut open = BlockBuilder::open()
            .source(send.hash())
            .representative(receiver_account)
            .account(receiver_account)
            .sign(receiver_key.clone())
            .without_sideband()
            .build()
            .unwrap();

        assert_eq!(self.process(txn, &mut open), ProcessResult::Progress);
        open
    }

    pub fn process_receive(
        &self,
        txn: &mut dyn WriteTransaction,
        send: &SendBlock,
        receiver_key: &KeyPair,
    ) -> ReceiveBlock {
        let receiver_account = receiver_key.public_key().into();

        let account_info = self
            .ledger
            .store
            .account()
            .get(txn.txn(), &receiver_account)
            .unwrap();

        let mut receive = BlockBuilder::receive()
            .previous(account_info.head)
            .source(send.hash())
            .sign(receiver_key.clone())
            .without_sideband()
            .build()
            .unwrap();

        assert_eq!(self.process(txn, &mut receive), ProcessResult::Progress);
        receive
    }

    pub(crate) fn process_change(
        &self,
        txn: &mut dyn WriteTransaction,
        keypair: &KeyPair,
        representative: Account,
    ) -> ChangeBlock {
        let account = keypair.public_key().into();

        let account_info = self
            .ledger
            .store
            .account()
            .get(txn.txn(), &account)
            .unwrap();

        let mut change = BlockBuilder::change()
            .previous(account_info.head)
            .representative(representative)
            .sign(keypair.clone())
            .build()
            .unwrap();

        assert_eq!(self.process(txn, &mut change), ProcessResult::Progress);
        change
    }
}
