use crate::{
    core::{Amount, Block, PendingKey},
    ledger::DEV_GENESIS_KEY,
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerContext;

#[test]
fn rollback_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        *DEV_GENESIS_ACCOUNT,
        Amount::new(50),
    );

    ctx.ledger
        .rollback(txn.as_mut(), &send.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        DEV_CONSTANTS.genesis_amount
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
    assert_eq!(
        ctx.ledger.store.pending().get(
            txn.txn(),
            &PendingKey::new(*DEV_GENESIS_ACCOUNT, send.hash())
        ),
        None
    );
    assert_eq!(
        ctx.ledger
            .store
            .block()
            .successor(txn.txn(), &DEV_GENESIS_HASH),
        None
    );
}
