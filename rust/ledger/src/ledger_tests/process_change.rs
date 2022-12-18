use crate::{
    ledger_tests::{setup_change_block, upgrade_genesis_to_epoch_v1},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Account, Amount, Block, BlockDetails, Epoch};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_change_block(&ctx, txn.as_mut());

    let loaded_block = ctx
        .ledger
        .store
        .block()
        .get(txn.txn(), &change.hash())
        .unwrap();
    assert_eq!(loaded_block, change);
    assert_eq!(loaded_block.sideband().unwrap(), change.sideband().unwrap());
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_change_block(&ctx, txn.as_mut());

    let sideband = change.sideband().unwrap();
    assert_eq!(sideband.height, 2);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, false, false, false)
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_change_block(&ctx, txn.as_mut());

    let weight = ctx.ledger.weight(&change.representative().unwrap());
    assert_eq!(weight, change.balance());
}

#[test]
fn change_to_zero_rep() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut change = ctx
        .genesis_block_factory()
        .change(txn.txn())
        .representative(0)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    assert_eq!(
        ctx.ledger
            .cache
            .rep_weights
            .representation_get(&DEV_GENESIS_ACCOUNT),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger
            .cache
            .rep_weights
            .representation_get(&Account::zero()),
        change.balance()
    );
}

#[test]
fn change_from_zero_rep_to_real_rep() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut change_to_zero_rep = ctx
        .genesis_block_factory()
        .change(txn.txn())
        .representative(0)
        .build();
    ctx.ledger
        .process(txn.as_mut(), &mut change_to_zero_rep)
        .unwrap();

    let mut change_to_genesis = ctx
        .genesis_block_factory()
        .change(txn.txn())
        .representative(*DEV_GENESIS_ACCOUNT)
        .build();
    ctx.ledger
        .process(txn.as_mut(), &mut change_to_genesis)
        .unwrap();

    assert_eq!(
        ctx.ledger
            .cache
            .rep_weights
            .representation_get(&DEV_GENESIS_ACCOUNT),
        change_to_genesis.balance()
    );
    assert_eq!(
        ctx.ledger
            .cache
            .rep_weights
            .representation_get(&Account::zero()),
        Amount::zero()
    );
}

#[test]
fn fail_insufficient_work_epoch_0() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut send = ctx.genesis_block_factory().send(txn.txn()).work(0).build();
    {
        let block: &mut dyn Block = send.as_block_mut();
        block.set_work(0);
    };
    let result = ctx.ledger.process(txn.as_mut(), &mut send).unwrap_err();
    assert_eq!(result, ProcessResult::InsufficientWork);
}

#[test]
fn fail_insufficient_work_epoch_1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());
    let mut send = ctx.genesis_block_factory().send(txn.txn()).work(0).build();
    {
        let block: &mut dyn Block = send.as_block_mut();
        block.set_work(0);
    };
    let result = ctx.ledger.process(txn.as_mut(), &mut send).unwrap_err();
    assert_eq!(result, ProcessResult::InsufficientWork);
}
