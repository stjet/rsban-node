use crate::{
    core::{Block, BlockDetails, BlockEnum, Epoch},
    ledger::ledger_tests::setup_change_block,
};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let change = setup_change_block(&ctx, txn.as_mut());

    let BlockEnum::State(loaded_block) = ctx.ledger.store.block().get(txn.txn(), &change.hash()).unwrap() else { panic!("not a state block!")};
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

    let weight = ctx.ledger.weight(&change.representative());
    assert_eq!(weight, change.balance());
}
