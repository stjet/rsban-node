use crate::{
    core::{Account, Amount, Block, PendingKey},
    DEV_GENESIS_ACCOUNT,
};

use super::LedgerWithOpenBlock;

#[test]
fn clears_frontier_store() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        Account::zero()
    );
}

#[test]
fn clears_account_store() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    let receiver_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &ctx.receiver_account);
    assert_eq!(receiver_info, None);

    let sender_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(sender_info.head, ctx.send_block.hash());
}

#[test]
fn updates_pending_store() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    let pending = ctx
        .ledger()
        .store
        .pending()
        .get(
            ctx.txn.txn(),
            &PendingKey::new(ctx.receiver_account, ctx.send_block.hash()),
        )
        .unwrap();

    assert_eq!(pending.source, *DEV_GENESIS_ACCOUNT);
    assert_eq!(pending.amount, ctx.amount_sent);
}

#[test]
fn updates_account_balance() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        Amount::new(50)
    );
}

#[test]
fn updates_receivable() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.amount_sent
    );
}

#[test]
fn updates_vote_weight() {
    let mut ctx = LedgerWithOpenBlock::new();

    ctx.rollback();

    assert_eq!(ctx.ledger().weight(&DEV_GENESIS_ACCOUNT), Amount::new(50));
    assert_eq!(ctx.ledger().weight(&ctx.receiver_account), Amount::zero());
}
