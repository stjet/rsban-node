use crate::{
    core::{Account, Amount, Block, KeyPair, OpenBlock, ReceiveBlock, SendBlock},
    ledger::{datastore::WriteTransaction, Ledger},
    DEV_CONSTANTS,
};

use super::{AccountBlockFactory, LedgerContext};

pub(crate) struct LegacySendBlockResult<'a> {
    pub destination: AccountBlockFactory<'a>,
    pub send_block: SendBlock,
    pub amount_sent: Amount,
}
pub(crate) fn setup_legacy_send_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> LegacySendBlockResult<'a> {
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let amount_sent = Amount::new(50);
    let mut send_block = genesis
        .legacy_send(txn.txn())
        .destination(destination.account())
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn, &mut send_block).unwrap();

    LegacySendBlockResult {
        destination,
        send_block,
        amount_sent,
    }
}

pub(crate) struct LegacyOpenBlockResult<'a> {
    pub destination: AccountBlockFactory<'a>,
    pub send_block: SendBlock,
    pub open_block: OpenBlock,
    pub expected_balance: Amount,
}
pub(crate) fn setup_legacy_open_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> LegacyOpenBlockResult<'a> {
    let send = setup_legacy_send_block(ctx, txn);

    let mut open_block = send.destination.legacy_open(send.send_block.hash()).build();
    ctx.ledger.process(txn, &mut open_block).unwrap();

    LegacyOpenBlockResult {
        destination: send.destination,
        send_block: send.send_block,
        open_block,
        expected_balance: send.amount_sent,
    }
}

pub(crate) struct LegacyReceiveBlockResult<'a> {
    pub destination: AccountBlockFactory<'a>,
    pub open_block: OpenBlock,
    pub receive_block: ReceiveBlock,
    pub expected_balance: Amount,
    pub amount_received: Amount,
}
pub(crate) fn setup_legacy_receive_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> LegacyReceiveBlockResult<'a> {
    let genesis = ctx.genesis_block_factory();

    let open = setup_legacy_open_block(ctx, txn);

    let amount_sent2 = Amount::new(25);
    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .amount(amount_sent2)
        .build();
    ctx.ledger.process(txn, &mut send2).unwrap();

    let mut receive_block = open
        .destination
        .legacy_receive(txn.txn(), send2.hash())
        .build();
    ctx.ledger.process(txn, &mut receive_block).unwrap();

    LegacyReceiveBlockResult {
        destination: open.destination,
        open_block: open.open_block,
        receive_block,
        expected_balance: open.expected_balance + amount_sent2,
        amount_received: amount_sent2,
    }
}

//todo delete:

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
            .legacy_send(txn.txn())
            .destination(receiver.account())
            .amount(amount_sent1)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send1)
            .unwrap();

        let mut open_block = receiver.legacy_open(send1.hash()).build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut open_block)
            .unwrap();
        let amount_sent2 = Amount::new(25);

        let mut send2 = genesis
            .legacy_send(txn.txn())
            .destination(receiver.account())
            .amount(amount_sent2)
            .build();
        ledger_context
            .ledger
            .process(txn.as_mut(), &mut send2)
            .unwrap();

        let mut receive_block = receiver.legacy_receive(txn.txn(), send2.hash()).build();
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
