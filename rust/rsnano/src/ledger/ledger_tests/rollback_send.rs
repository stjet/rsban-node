use std::sync::atomic::Ordering;

use crate::{
    core::{Account, Amount, Block, PendingKey},
    ledger::{datastore::WriteTransaction, ledger_tests::setup_legacy_open_block},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::{setup_legacy_send_block, LedgerContext, LegacySendBlockResult};

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    rollback_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = rollback_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .store
            .frontier()
            .get(txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(
        ctx.ledger
            .store
            .frontier()
            .get(txn.txn(), &send.send_block.hash()),
        Account::zero()
    );
}

#[test]
fn update_account_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    rollback_send_block(&ctx, txn.as_mut());

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.head, *DEV_GENESIS_HASH);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(ctx.ledger.cache.account_count.load(Ordering::Relaxed), 1);
}

#[test]
fn remove_from_pending_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = rollback_send_block(&ctx, txn.as_mut());

    let pending = ctx.ledger.store.pending().get(
        txn.txn(),
        &PendingKey::new(send.destination.account(), send.send_block.hash()),
    );
    assert_eq!(pending, None);
}

#[test]
fn update_confirmation_height_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    rollback_send_block(&ctx, txn.as_mut());

    let conf_height = ctx
        .ledger
        .store
        .confirmation_height()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(conf_height.frontier, *DEV_GENESIS_HASH);
    assert_eq!(conf_height.height, 1);
}

#[test]
fn rollback_dependent_blocks_too() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    // Rollback send block. This requires the rollback of the open block first.
    ctx.ledger
        .rollback(txn.as_mut(), &open.send_block.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        DEV_CONSTANTS.genesis_amount
    );

    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &open.destination.account(), false),
        Amount::zero()
    );

    assert!(ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &open.destination.account())
        .is_none());

    let pending = ctx.ledger.store.pending().get(
        txn.txn(),
        &PendingKey::new(open.destination.account(), *DEV_GENESIS_HASH),
    );
    assert_eq!(pending, None);
}

fn rollback_send_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> LegacySendBlockResult<'a> {
    let send = setup_legacy_send_block(ctx, txn);
    ctx.ledger
        .rollback(txn, &send.send_block.hash(), &mut Vec::new())
        .unwrap();
    send
}
