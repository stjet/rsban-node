use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB, ledger_tests::setup_legacy_change_block,
    ProcessResult, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_KEY,
};
use rsnano_core::{Account, Amount, Block, BlockBuilder, BlockEnum, BlockHash, KeyPair};

use super::LedgerContext;

#[test]
fn update_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_legacy_change_block(&ctx, txn.as_mut());

    let sideband = change.sideband().unwrap();
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(sideband.balance, LEDGER_CONSTANTS_STUB.genesis_amount);
    assert_eq!(sideband.height, 2);
}

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_legacy_change_block(&ctx, txn.as_mut());

    let loaded_block = ctx.ledger.get_block(txn.txn(), &change.hash()).unwrap();

    assert_eq!(loaded_block, change);
    assert_eq!(loaded_block.sideband().unwrap(), change.sideband().unwrap());
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_legacy_change_block(&ctx, txn.as_mut());

    let account = ctx.ledger.get_frontier(txn.txn(), &change.hash());
    assert_eq!(account, Some(*DEV_GENESIS_ACCOUNT));

    let account = ctx.ledger.get_frontier(txn.txn(), &DEV_GENESIS_HASH);
    assert_eq!(account, None);
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let BlockEnum::LegacyChange(change) = setup_legacy_change_block(&ctx, txn.as_mut()) else {panic!("not a change block!")};

    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&change.mandatory_representative()),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
}

#[test]
fn update_account_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let BlockEnum::LegacyChange(change) = setup_legacy_change_block(&ctx, txn.as_mut()) else {panic!("not a change block!")};

    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.head, change.hash());
    assert_eq!(account_info.block_count, 2);
    assert_eq!(account_info.balance, LEDGER_CONSTANTS_STUB.genesis_amount);
    assert_eq!(
        account_info.representative,
        change.mandatory_representative()
    );
}

#[test]
fn fail_old() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut change = setup_legacy_change_block(&ctx, txn.as_mut());

    let result = ctx.ledger.process(txn.as_mut(), &mut change).unwrap_err();

    assert_eq!(result, ProcessResult::Old);
}

#[test]
fn fail_gap_previous() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let keypair = KeyPair::new();

    let mut block = BlockBuilder::legacy_change()
        .previous(BlockHash::from(1))
        .sign(&keypair)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut block).unwrap_err();

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn fail_bad_signature() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let wrong_keys = KeyPair::new();

    let mut block = BlockBuilder::legacy_change()
        .previous(*DEV_GENESIS_HASH)
        .sign(&wrong_keys)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut block).unwrap_err();

    assert_eq!(result, ProcessResult::BadSignature);
}

#[test]
fn fail_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let mut fork = BlockBuilder::legacy_change()
        .previous(*DEV_GENESIS_HASH)
        .representative(Account::from(12345))
        .sign(&DEV_GENESIS_KEY)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut fork).unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

// Make sure old block types can't be inserted after a state block.
#[test]
fn change_after_state_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut change = genesis.legacy_change(txn.txn()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut change).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn fail_insufficient_work() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut change = ctx
        .genesis_block_factory()
        .legacy_change(txn.txn())
        .work(0)
        .build();

    {
        let block: &mut dyn Block = change.as_block_mut();
        block.set_work(0);
    };
    let result = ctx.ledger.process(txn.as_mut(), &mut change).unwrap_err();
    assert_eq!(result, ProcessResult::InsufficientWork);
}
