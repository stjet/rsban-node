use crate::{ledger_constants::LEDGER_CONSTANTS_STUB, DEV_GENESIS_ACCOUNT};
use rsnano_core::{Amount, PendingKey};
use rsnano_store_lmdb::LmdbWriteTransaction;

use crate::ledger_tests::{setup_legacy_open_block, LedgerContext};

use super::LegacyOpenBlockResult;

#[test]
fn remove_from_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, &mut txn);

    assert_eq!(ctx.ledger.get_frontier(&txn, &open.open_block.hash()), None);
}

#[test]
fn remove_from_account_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, &mut txn);

    let receiver_info = ctx.ledger.account_info(&txn, &open.destination.account());
    assert_eq!(receiver_info, None);

    let sender_info = ctx.ledger.account_info(&txn, &DEV_GENESIS_ACCOUNT).unwrap();
    assert_eq!(sender_info.head, open.send_block.hash());
}

#[test]
fn update_pending_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, &mut txn);

    let pending = ctx
        .ledger
        .pending_info(
            &txn,
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

    let open = rollback_open_block(&ctx, &mut txn);

    assert_eq!(
        ctx.ledger
            .account_balance(&txn, &open.destination.account(), false),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger
            .account_balance(&txn, &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount - open.expected_balance
    );
}

#[test]
fn update_receivable() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, &mut txn);

    assert_eq!(
        ctx.ledger
            .account_receivable(&txn, &open.destination.account(), false),
        open.expected_balance
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = rollback_open_block(&ctx, &mut txn);

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - open.expected_balance
    );
    assert_eq!(
        ctx.ledger.weight(&open.destination.account()),
        Amount::zero()
    );
}

fn rollback_open_block<'a>(
    ctx: &'a LedgerContext,
    txn: &mut LmdbWriteTransaction,
) -> LegacyOpenBlockResult<'a> {
    let open = setup_legacy_open_block(ctx, txn);
    ctx.ledger.rollback(txn, &open.open_block.hash()).unwrap();
    open
}
