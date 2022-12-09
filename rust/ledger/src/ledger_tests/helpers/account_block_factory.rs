use crate::{ledger_constants::LEDGER_CONSTANTS_STUB, Ledger, DEV_GENESIS_KEY};
use rsnano_core::{Account, AccountInfo, Amount, BlockHash, Epoch, KeyPair, Link};
use rsnano_store_traits::Transaction;

use rsnano_core::{
    BlockBuilder, LegacyChangeBlockBuilder, LegacyOpenBlockBuilder, LegacyReceiveBlockBuilder,
    LegacySendBlockBuilder, StateBlockBuilder,
};

/// Test helper that creates blocks for a single account
pub(crate) struct AccountBlockFactory<'a> {
    pub key: KeyPair,
    ledger: &'a Ledger,
}

impl<'a> AccountBlockFactory<'a> {
    pub(crate) fn new(ledger: &'a Ledger) -> Self {
        Self {
            key: KeyPair::new(),
            ledger,
        }
    }

    pub(crate) fn genesis(ledger: &'a Ledger) -> Self {
        Self {
            key: DEV_GENESIS_KEY.clone(),
            ledger,
        }
    }

    pub(crate) fn account(&self) -> Account {
        self.key.public_key().into()
    }

    pub(crate) fn info(&self, txn: &dyn Transaction) -> Option<AccountInfo> {
        self.ledger.store.account().get(txn, &self.account())
    }

    pub(crate) fn legacy_open(&self, source: BlockHash) -> LegacyOpenBlockBuilder {
        BlockBuilder::legacy_open()
            .source(source)
            .representative(self.account())
            .account(self.account())
            .sign(&self.key)
    }

    pub(crate) fn epoch_v1(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(info.representative)
            .balance(info.balance)
            .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
            .sign(&DEV_GENESIS_KEY)
    }

    pub(crate) fn epoch_v1_open(&self) -> StateBlockBuilder {
        BlockBuilder::state()
            .account(self.account())
            .previous(0)
            .representative(0)
            .balance(0)
            .link(self.ledger.epoch_link(Epoch::Epoch1).unwrap())
            .sign(&DEV_GENESIS_KEY)
    }

    pub(crate) fn epoch_v2(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(info.representative)
            .balance(info.balance)
            .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch2).unwrap())
            .sign(&DEV_GENESIS_KEY)
    }

    pub(crate) fn epoch_v2_open(&self) -> StateBlockBuilder {
        BlockBuilder::state()
            .account(self.account())
            .previous(0)
            .representative(0)
            .balance(0)
            .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch2).unwrap())
            .sign(&DEV_GENESIS_KEY)
    }

    pub(crate) fn legacy_change(&self, txn: &dyn Transaction) -> LegacyChangeBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::legacy_change()
            .previous(info.head)
            .representative(Account::from(1))
            .sign(&self.key)
    }

    pub(crate) fn legacy_send(&self, txn: &dyn Transaction) -> LegacySendBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::legacy_send()
            .previous(info.head)
            .destination(Account::from(1))
            .previous_balance(info.balance)
            .amount(Amount::new(1))
            .sign(self.key.clone())
    }

    pub(crate) fn legacy_receive(
        &self,
        txn: &dyn Transaction,
        send_hash: BlockHash,
    ) -> LegacyReceiveBlockBuilder {
        let receiver_info = self.info(txn).unwrap();
        BlockBuilder::legacy_receive()
            .previous(receiver_info.head)
            .source(send_hash)
            .sign(&self.key)
    }

    pub(crate) fn send(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .previous_balance(info.balance)
            .representative(info.representative)
            .amount(Amount::new(50))
            .link(Account::from(1))
            .sign(&self.key)
    }

    pub(crate) fn receive(&self, txn: &dyn Transaction, send_hash: BlockHash) -> StateBlockBuilder {
        let receiver_info = self.info(txn).unwrap();
        let amount_sent = self.ledger.amount(txn, &send_hash).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(receiver_info.head)
            .representative(receiver_info.representative)
            .balance(receiver_info.balance + amount_sent)
            .link(send_hash)
            .sign(&self.key)
    }

    pub(crate) fn change(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(Account::from(1))
            .balance(info.balance)
            .link(Link::zero())
            .sign(&self.key)
    }

    pub(crate) fn open(&self, txn: &dyn Transaction, send_hash: BlockHash) -> StateBlockBuilder {
        let amount_sent = self.ledger.amount(txn, &send_hash).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(0)
            .representative(self.account())
            .balance(amount_sent)
            .link(send_hash)
            .sign(&self.key)
    }
}
