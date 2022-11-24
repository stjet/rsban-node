use crate::{
    core::{Account, Amount, Block, PendingKey},
    ledger::{
        datastore::WriteTransaction,
        ledger_tests::{setup_legacy_open_block, LedgerContext},
    },
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

use super::LegacyOpenBlockResult;

#[test]
fn remove_from_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .store
            .frontier()
            .get(txn.txn(), &open.open_block.hash()),
        Account::zero()
    );
}

#[test]
fn remove_from_account_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    let receiver_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &open.destination.account());
    assert_eq!(receiver_info, None);

    let sender_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(sender_info.head, open.send_block.hash());
}

#[test]
fn update_pending_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    let pending = ctx
        .ledger
        .store
        .pending()
        .get(
            txn.txn(),
            &PendingKey::new(open.destination.account(), open.send_block.hash()),
        )
        .unwrap();

    assert_eq!(pending.source, *DEV_GENESIS_ACCOUNT);
    assert_eq!(pending.amount, open.expected_balance);
}

#[test]
fn update_account_balance() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &open.destination.account(), false),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        DEV_CONSTANTS.genesis_amount - open.expected_balance
    );
}

#[test]
fn update_receivable() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_receivable(txn.txn(), &open.destination.account(), false),
        open.expected_balance
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - open.expected_balance
    );
    assert_eq!(
        ctx.ledger.weight(&open.destination.account()),
        Amount::zero()
    );
}

fn rollback_open_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> LegacyOpenBlockResult<'a> {
    let open = setup_legacy_open_block(ctx, txn);
    ctx.ledger
        .rollback(txn, &open.open_block.hash(), &mut Vec::new())
        .unwrap();
    open
}
