use crate::{
    core::{Account, Amount, Block, PendingKey},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

use super::LedgerWithReceiveBlock;

#[test]
fn clear_successor() {
    let mut ctx = LedgerWithReceiveBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .block()
            .successor(ctx.txn.txn(), &ctx.open_block.hash()),
        None
    );
}

#[test]
fn update_frontiers() {
    let mut ctx = LedgerWithReceiveBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        ctx.receiver_account
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.receive_block.hash()),
        Account::zero()
    );
}

#[test]
fn update_account_info() {
    let mut ctx = LedgerWithReceiveBlock::new();

    ctx.rollback();

    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &ctx.receiver_account)
        .unwrap();

    assert_eq!(account_info.head, ctx.open_block.hash());
    assert_eq!(account_info.block_count, 1);
    assert_eq!(
        account_info.balance,
        ctx.open_block.sideband().unwrap().balance
    );
}

#[test]
fn insert_pending_info() {
    let mut ctx = LedgerWithReceiveBlock::new();

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
fn update_vote_weight() {
    let mut ctx = LedgerWithReceiveBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger().weight(&ctx.receiver_account),
        DEV_CONSTANTS.genesis_amount - Amount::new(50),
    );
}
