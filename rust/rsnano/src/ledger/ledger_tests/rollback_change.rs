use crate::{
    core::{Account, Amount, Block},
    ledger::ledger_tests::LedgerContext,
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    let frontier = &ctx.ledger.store.frontier();
    assert_eq!(frontier.get(txn.txn(), &change.hash()), Account::zero());
    assert_eq!(
        frontier.get(txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT
    );
}

#[test]
fn update_account_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.head, *DEV_GENESIS_HASH);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.representative, *DEV_GENESIS_ACCOUNT);
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
    assert_eq!(ctx.ledger.weight(&change.representative()), Amount::zero(),);
}

#[test]
fn rollback_dependent_blocks_too() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut change = genesis.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let mut send = genesis.legacy_send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.store.block().get(txn.txn(), &send.hash()), None);

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}
