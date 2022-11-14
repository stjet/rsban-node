use crate::{
    core::{Account, Amount, Block},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerWithChangeBlock;

#[test]
fn update_frontier_store() {
    let mut ctx = LedgerWithChangeBlock::new();

    ctx.rollback();

    let frontier = &ctx.ledger().store.frontier();
    assert_eq!(
        frontier.get(ctx.txn.txn(), &ctx.change_block.hash()),
        Account::zero()
    );
    assert_eq!(
        frontier.get(ctx.txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT
    );
}

#[test]
fn update_account_info() {
    let mut ctx = LedgerWithChangeBlock::new();

    ctx.rollback();

    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.head, *DEV_GENESIS_HASH);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.representative, *DEV_GENESIS_ACCOUNT);
}

#[test]
fn update_vote_weight() {
    let mut ctx = LedgerWithChangeBlock::new();

    ctx.rollback();

    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
    assert_eq!(
        ctx.ledger().weight(&ctx.change_block.representative()),
        Amount::zero(),
    );
}

#[test]
fn rollback_dependent_blocks_too() {
    let mut ctx = LedgerWithChangeBlock::new();
    let send_block = ctx.ledger_context.process_send_from_genesis(
        ctx.txn.as_mut(),
        &Account::from(1000),
        Amount::new(100),
    );

    ctx.rollback();

    assert_eq!(
        ctx.ledger()
            .store
            .block()
            .get(ctx.txn.txn(), &send_block.hash()),
        None
    );

    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}
