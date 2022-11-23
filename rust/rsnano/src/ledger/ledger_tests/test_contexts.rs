use crate::{
    core::{Account, Amount, Block, KeyPair, OpenBlock, ReceiveBlock, SendBlock},
    ledger::{datastore::WriteTransaction, Ledger},
    DEV_CONSTANTS,
};

use super::{AccountBlockFactory, LedgerContext};

pub(crate) struct LedgerWithOpenBlock {
    pub open_block: OpenBlock,
    pub send_block: SendBlock,
    pub txn: Box<dyn WriteTransaction>,
    pub receiver_key: KeyPair,
    pub receiver_account: Account,
    pub amount_sent: Amount,
    pub ledger_context: LedgerContext,
}

impl LedgerWithOpenBlock {
    pub fn new() -> Self {
        let ledger_context = LedgerContext::empty();
        let genesis = AccountBlockFactory::genesis(&ledger_context.ledger);
        let receiver = AccountBlockFactory::new(&ledger_context.ledger);
        let mut txn = ledger_context.ledger.rw_txn();
        let amount_sent = DEV_CONSTANTS.genesis_amount - Amount::new(50);

        let mut send_block = genesis
            .send(txn.txn(), receiver.account(), amount_sent)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send_block)
            .unwrap();

        let mut open_block = receiver.open(send_block.hash()).build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut open_block)
            .unwrap();
        let receiver_account = receiver.account();
        let receiver_key = receiver.key;

        Self {
            txn,
            open_block,
            send_block,
            ledger_context,
            receiver_key,
            amount_sent,
            receiver_account,
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger_context.ledger
    }

    pub fn rollback(&mut self) {
        self.ledger_context
            .ledger
            .rollback(self.txn.as_mut(), &self.open_block.hash(), &mut Vec::new())
            .unwrap();
    }
}

pub(crate) struct LedgerWithSendBlock {
    pub send_block: SendBlock,
    pub txn: Box<dyn WriteTransaction>,
    pub receiver_key: KeyPair,
    pub receiver_account: Account,
    pub old_genesis_balance: Amount,
    pub new_genesis_balance: Amount,
    pub amount_sent: Amount,
    pub ledger_context: LedgerContext,
}

impl LedgerWithSendBlock {
    pub fn new() -> Self {
        let receiver_key = KeyPair::new();
        let receiver_account = receiver_key.public_key().into();
        let old_genesis_balance = DEV_CONSTANTS.genesis_amount;
        let new_genesis_balance = Amount::new(50);
        let amount_sent = old_genesis_balance - new_genesis_balance;

        let ledger_context = LedgerContext::empty();
        let genesis = AccountBlockFactory::genesis(&ledger_context.ledger);
        let mut txn = ledger_context.ledger.rw_txn();

        let mut send_block = genesis
            .send(txn.txn(), receiver_account, amount_sent)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send_block)
            .unwrap();

        Self {
            txn,
            send_block,
            ledger_context,
            receiver_key,
            receiver_account,
            old_genesis_balance,
            new_genesis_balance,
            amount_sent,
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger_context.ledger
    }

    pub(crate) fn rollback(&mut self) {
        self.ledger_context
            .ledger
            .rollback(self.txn.as_mut(), &self.send_block.hash(), &mut Vec::new())
            .unwrap();
    }
}

pub(crate) struct LedgerWithReceiveBlock {
    pub open_block: OpenBlock,
    pub send_block: SendBlock,
    pub receive_block: ReceiveBlock,
    pub txn: Box<dyn WriteTransaction>,
    pub receiver_key: KeyPair,
    pub receiver_account: Account,
    pub amount_sent: Amount,
    pub expected_receiver_balance: Amount,
    pub ledger_context: LedgerContext,
}

impl LedgerWithReceiveBlock {
    pub fn new() -> Self {
        let ledger_context = LedgerContext::empty();
        let mut txn = ledger_context.ledger.rw_txn();
        let genesis = AccountBlockFactory::genesis(&ledger_context.ledger);
        let receiver = AccountBlockFactory::new(&ledger_context.ledger);

        let amount_sent1 = DEV_CONSTANTS.genesis_amount - Amount::new(50);

        let mut send1 = genesis
            .send(txn.txn(), receiver.account(), amount_sent1)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send1)
            .unwrap();

        let mut open_block = receiver.open(send1.hash()).build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut open_block)
            .unwrap();
        let amount_sent2 = Amount::new(25);

        let mut send2 = genesis
            .send(txn.txn(), receiver.account(), amount_sent2)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send2)
            .unwrap();

        let mut receive_block = receiver.receive(txn.txn(), send2.hash()).build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut receive_block)
            .unwrap();

        let expected_receiver_balance = DEV_CONSTANTS.genesis_amount - Amount::new(25);
        let receiver_account = receiver.account();
        let receiver_key = receiver.key;

        Self {
            txn,
            open_block,
            send_block: send2,
            receive_block,
            ledger_context,
            receiver_key,
            amount_sent: amount_sent2,
            receiver_account,
            expected_receiver_balance,
        }
    }

    pub fn ledger(&self) -> &Ledger {
        &self.ledger_context.ledger
    }

    pub(crate) fn rollback(&mut self) {
        self.ledger_context
            .ledger
            .rollback(
                self.txn.as_mut(),
                &self.receive_block.hash(),
                &mut Vec::new(),
            )
            .unwrap();
    }
}
