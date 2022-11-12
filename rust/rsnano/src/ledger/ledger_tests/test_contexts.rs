use crate::{
    core::{Account, Amount, Block, KeyPair, OpenBlock, ReceiveBlock, SendBlock},
    ledger::{datastore::WriteTransaction, Ledger},
    DEV_CONSTANTS,
};

use super::LedgerContext;

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
        let receiver_key = KeyPair::new();
        let receiver_account = receiver_key.public_key().into();
        let ledger_context = LedgerContext::empty();
        let mut txn = ledger_context.ledger.rw_txn();
        let amount_sent = DEV_CONSTANTS.genesis_amount - Amount::new(50);
        let send_block =
            ledger_context.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent);
        let open_block = ledger_context.process_open(txn.as_mut(), &send_block, &receiver_key);

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
        let mut txn = ledger_context.ledger.rw_txn();
        let send_block =
            ledger_context.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent);

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
    ledger_context: LedgerContext,
}

impl LedgerWithReceiveBlock {
    pub fn new() -> Self {
        let receiver_key = KeyPair::new();
        let receiver_account = receiver_key.public_key().into();
        let ledger_context = LedgerContext::empty();
        let mut txn = ledger_context.ledger.rw_txn();

        let amount_sent1 = DEV_CONSTANTS.genesis_amount - Amount::new(50);
        let send1 =
            ledger_context.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent1);
        let open_block = ledger_context.process_open(txn.as_mut(), &send1, &receiver_key);
        let amount_sent2 = Amount::new(25);
        let send2 =
            ledger_context.process_send_from_genesis(txn.as_mut(), &receiver_account, amount_sent2);
        let receive_block = ledger_context.process_receive(txn.as_mut(), &send2, &receiver_key);
        let expected_receiver_balance = DEV_CONSTANTS.genesis_amount - Amount::new(25);

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
