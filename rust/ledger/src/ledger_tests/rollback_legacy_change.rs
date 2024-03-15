use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB, ledger_tests::LedgerContext, DEV_GENESIS_ACCOUNT,
};

#[test]
fn rollback_dependent_blocks_too() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(&txn).build();
    ctx.ledger.process(&mut txn, &mut change).unwrap();

    let mut send = genesis.legacy_send(&txn).build();
    ctx.ledger.process(&mut txn, &mut send).unwrap();

    ctx.ledger.rollback(&mut txn, &change.hash()).unwrap();

    assert_eq!(ctx.ledger.get_block(&txn, &send.hash()), None);

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
}
