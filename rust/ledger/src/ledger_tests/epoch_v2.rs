use crate::{ledger_tests::upgrade_genesis_to_epoch_v1, ProcessResult, DEV_GENESIS_ACCOUNT};
use rsnano_core::Epoch;

use super::LedgerContext;

#[test]
fn rollback_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, &mut txn);
    let genesis = ctx.genesis_block_factory();

    let mut epoch = genesis.epoch_v2(&txn).build();
    ctx.ledger.process(&mut txn, &mut epoch).unwrap();

    ctx.ledger.rollback(&mut txn, &epoch.hash()).unwrap();

    let genesis_info = ctx.ledger.account_info(&txn, &DEV_GENESIS_ACCOUNT).unwrap();
    assert_eq!(genesis_info.epoch, Epoch::Epoch1);

    let mut legacy_change = genesis.legacy_change(&txn).build();

    let result = ctx
        .ledger
        .process(&mut txn, &mut legacy_change)
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}
