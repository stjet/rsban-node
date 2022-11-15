use crate::{
    core::{Account, Amount, Block},
    ledger::{ledger_tests::LedgerContext, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let change = ctx.process_change(txn.as_mut(), &DEV_GENESIS_KEY, Account::from(1000));

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
    let change = ctx.process_change(txn.as_mut(), &DEV_GENESIS_KEY, Account::from(1000));

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
    let change = ctx.process_change(txn.as_mut(), &DEV_GENESIS_KEY, Account::from(1000));

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
    let change = ctx.process_change(txn.as_mut(), &DEV_GENESIS_KEY, Account::from(1000));

    let send = ctx.process_send_from_genesis(txn.as_mut(), &Account::from(1000), Amount::new(100));

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.store.block().get(txn.txn(), &send.hash()), None);

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}
