use std::sync::atomic::Ordering;

use crate::{
    core::{Account, Amount, BlockType},
    utils::seconds_since_epoch,
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerContext;

#[test]
fn account_balance_is_zero_for_unknown_account() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    let balance = ctx
        .ledger
        .account_balance(txn.txn(), &Account::zero(), false);

    assert_eq!(balance, Amount::zero());
}

#[test]
fn genesis_block() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    let block = ctx
        .ledger
        .store
        .block()
        .get(txn.txn(), &DEV_GENESIS_HASH)
        .expect("genesis block not found");

    assert_eq!(block.block_type(), BlockType::Open);
}

#[test]
fn genesis_account_balance() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    let balance = ctx
        .ledger
        .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false);

    assert_eq!(balance, DEV_CONSTANTS.genesis_amount);
}

#[test]
fn genesis_account_info() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .expect("genesis account not found");

    // Frontier time should have been updated when genesis balance was added
    assert!(account_info.modified > 0 && account_info.modified <= seconds_since_epoch());
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.balance, DEV_CONSTANTS.genesis_amount);
}

#[test]
fn genesis_confirmation_height_info() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    // Genesis block should be confirmed by default
    let conf_info = ctx
        .ledger
        .store
        .confirmation_height()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .expect("conf height not found");

    assert_eq!(conf_info.height, 1);
    assert_eq!(conf_info.frontier, *DEV_GENESIS_HASH);
}

#[test]
fn genesis_frontier() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();

    assert_eq!(
        ctx.ledger
            .store
            .frontier()
            .get(txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_ACCOUNT,
    );
}

#[test]
fn cache() {
    let ctx = LedgerContext::empty();
    assert_eq!(ctx.ledger.cache.account_count.load(Ordering::SeqCst), 1);
    assert_eq!(ctx.ledger.cache.cemented_count.load(Ordering::SeqCst), 1);
}

#[test]
fn genesis_representative() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();
    assert_eq!(
        ctx.ledger.representative(txn.txn(), &DEV_GENESIS_HASH),
        *DEV_GENESIS_HASH
    );
}

#[test]
fn genesis_vote_weight() {
    let ctx = LedgerContext::empty();
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn latest_empty() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();
    assert_eq!(ctx.ledger.latest(txn.txn(), &Account::from(1)), None);
}
