use crate::{
    core::{Account, Amount, Block, BlockEnum, KeyPair, OpenBlock, ReceiveBlock, SendBlock},
    ledger::{datastore::WriteTransaction, Ledger},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

use super::LedgerContext;

#[test]
fn updates_sideband() {
    let ctx = LedgerWithReceiveBlock::new();
    let sideband = ctx.receive_block.sideband().unwrap();
    assert_eq!(sideband.account, ctx.receiver_account);
    assert_eq!(sideband.balance, ctx.expected_receiver_balance);
    assert_eq!(sideband.height, 2);
}

#[test]
fn saves_block() {
    let ctx = LedgerWithReceiveBlock::new();

    let loaded_block = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.receive_block.hash())
        .unwrap();

    let BlockEnum::Receive(loaded_block) = loaded_block else{panic!("not a receive block")};
    assert_eq!(loaded_block, ctx.receive_block);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        ctx.receive_block.sideband().unwrap()
    );
}

#[test]
fn updates_block_amount() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .amount(ctx.txn.txn(), &ctx.receive_block.hash()),
        Some(Amount::new(25))
    );
}

#[test]
fn updates_frontier_store() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        Account::zero()
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.receive_block.hash()),
        ctx.receiver_account
    );
}

#[test]
fn updates_balance() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        Amount::new(25)
    );
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.expected_receiver_balance
    );
}

#[test]
fn updates_vote_weight() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger().weight(&ctx.receiver_account),
        ctx.expected_receiver_balance
    );
}

#[test]
fn updates_account_receivable() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );
}

#[test]
fn updates_latest_block() {
    let ctx = LedgerWithReceiveBlock::new();
    assert_eq!(
        ctx.ledger().latest(ctx.txn.txn(), &ctx.receiver_account),
        Some(ctx.receive_block.hash())
    );
}

struct LedgerWithReceiveBlock {
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
    fn new() -> Self {
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

    fn ledger(&self) -> &Ledger {
        &self.ledger_context.ledger
    }
}
