use crate::{
    core::{Account, Block},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerWithSendBlock;

#[test]
fn updates_vote_weight() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn updates_frontier_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.send_block.hash()),
        Account::zero()
    );
}

#[test]
fn updates_account_store() {
    let mut ctx = LedgerWithSendBlock::new();

    ctx.rollback();

    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.head, *DEV_GENESIS_HASH);
}
