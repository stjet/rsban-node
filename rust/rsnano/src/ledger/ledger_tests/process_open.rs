use crate::{
    core::{Account, Amount, Block, BlockEnum, KeyPair, OpenBlock, SendBlock},
    ledger::{datastore::WriteTransaction, Ledger},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

use super::LedgerContext;

#[test]
fn updates_sideband() {
    let ctx = LedgerWithOpenBlock::new();
    let sideband = ctx.open_block.sideband().unwrap();
    assert_eq!(sideband.account, ctx.receiver_account);
    assert_eq!(sideband.balance, ctx.amount_sent);
    assert_eq!(sideband.height, 1);
}

#[test]
fn saves_block() {
    let ctx = LedgerWithOpenBlock::new();

    let loaded_open = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.open_block.hash())
        .unwrap();

    let BlockEnum::Open(loaded_open) = loaded_open else{panic!("not an open block")};
    assert_eq!(loaded_open, ctx.open_block);
    assert_eq!(
        loaded_open.sideband().unwrap(),
        ctx.open_block.sideband().unwrap()
    );
}

#[test]
fn updates_block_amount() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger().amount(ctx.txn.txn(), &ctx.open_block.hash()),
        Some(ctx.amount_sent)
    );
    assert_eq!(
        ctx.ledger()
            .store
            .block()
            .account_calculated(&ctx.open_block),
        ctx.receiver_account
    );
}

#[test]
fn updates_frontier_store() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        ctx.receiver_account
    );
}

#[test]
fn updates_account_balance() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.amount_sent
    );
}

#[test]
fn updates_account_receivable() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );
}

#[test]
fn updates_vote_weight() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - ctx.amount_sent
    );
    assert_eq!(ctx.ledger().weight(&ctx.receiver_account), ctx.amount_sent);
}

#[test]
fn updates_sender_account_info() {
    let ctx = LedgerWithOpenBlock::new();
    let sender_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(sender_info.head, ctx.send_block.hash());
}

#[test]
fn updates_receiver_account_info() {
    let ctx = LedgerWithOpenBlock::new();
    let receiver_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &ctx.receiver_account)
        .unwrap();
    assert_eq!(receiver_info.head, ctx.open_block.hash());
}

struct LedgerWithOpenBlock {
    pub open_block: OpenBlock,
    pub send_block: SendBlock,
    pub txn: Box<dyn WriteTransaction>,
    pub receiver_key: KeyPair,
    pub receiver_account: Account,
    pub amount_sent: Amount,
    ledger_context: LedgerContext,
}

impl LedgerWithOpenBlock {
    fn new() -> Self {
        let receiver_key = KeyPair::new();
        let receiver_account = receiver_key.public_key().into();
        let ledger_context = LedgerContext::empty();
        let mut txn = ledger_context.ledger.rw_txn();
        let amount_sent = Amount::new(50);
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

    fn ledger(&self) -> &Ledger {
        &self.ledger_context.ledger
    }
}
