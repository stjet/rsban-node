use crate::{
    core::{Account, Amount, Block, BlockEnum},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerWithChangeBlock;

#[test]
fn update_sideband() {
    let ctx = LedgerWithChangeBlock::new();
    let sideband = ctx.change_block.sideband().unwrap();
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(sideband.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(sideband.height, 2);
}

#[test]
fn save_block() {
    let ctx = LedgerWithChangeBlock::new();

    let loaded_block = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.change_block.hash())
        .unwrap();

    let BlockEnum::Change(loaded_block) = loaded_block else{panic!("not a change block")};
    assert_eq!(loaded_block, ctx.change_block);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        ctx.change_block.sideband().unwrap()
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerWithChangeBlock::new();

    let account = ctx
        .ledger()
        .store
        .frontier()
        .get(ctx.txn.txn(), &ctx.change_block.hash());
    assert_eq!(account, *DEV_GENESIS_ACCOUNT);

    let account = ctx
        .ledger()
        .store
        .frontier()
        .get(ctx.txn.txn(), &DEV_GENESIS_HASH);
    assert_eq!(account, Account::zero());
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerWithChangeBlock::new();
    assert_eq!(ctx.ledger().weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger().weight(&ctx.change_block.representative()),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn update_account_info() {
    let ctx = LedgerWithChangeBlock::new();
    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.head, ctx.change_block.hash());
    assert_eq!(account_info.block_count, 2);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(
        account_info.representative,
        ctx.change_block.representative()
    );
}
