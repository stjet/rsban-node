use crate::{
    core::{Account, Amount, Block, BlockBuilder, BlockEnum, BlockHash, KeyPair},
    ledger::{ProcessResult, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::{LedgerContext, LedgerWithChangeBlock};

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

#[test]
fn fail_old() {
    let mut ctx = LedgerWithChangeBlock::new();
    let result = ctx
        .ledger_context
        .process(ctx.txn.as_mut(), &mut ctx.change_block);
    assert_eq!(result, ProcessResult::Old);
}

#[test]
fn fail_gap_previous() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let keypair = KeyPair::new();

    let mut block = BlockBuilder::change()
        .previous(BlockHash::from(1))
        .sign(keypair)
        .build()
        .unwrap();

    let result = ctx.process(txn.as_mut(), &mut block);

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn fail_bad_signature() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let wrong_keys = KeyPair::new();

    let mut block = BlockBuilder::change()
        .previous(*DEV_GENESIS_HASH)
        .sign(wrong_keys)
        .build()
        .unwrap();

    let result = ctx.process(txn.as_mut(), &mut block);

    assert_eq!(result, ProcessResult::BadSignature);
}

#[test]
fn fail_fork() {
    let mut ctx = LedgerWithChangeBlock::new();
    let mut block = BlockBuilder::change()
        .previous(*DEV_GENESIS_HASH)
        .representative(Account::from(12345))
        .sign(DEV_GENESIS_KEY.clone())
        .build()
        .unwrap();

    let result = ctx.ledger_context.process(ctx.txn.as_mut(), &mut block);

    assert_eq!(result, ProcessResult::Fork);
}
