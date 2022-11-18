use std::sync::atomic::Ordering;

use crate::{
    core::{Amount, Block, BlockDetails, BlockEnum, Epoch, KeyPair, PendingKey},
    ledger::DEV_GENESIS_KEY,
};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let receiver_key = KeyPair::new();
    let receiver_account = receiver_key.public_key().into();

    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        Amount::new(1),
    );

    let open = ctx.process_state_open(txn.as_mut(), &send, &receiver_key);

    let BlockEnum::State(loaded_open) = ctx
        .ledger
        .store
        .block()
        .get(txn.txn(), &open.hash())
        .unwrap() else { panic!("not a state block!")};

    assert_eq!(loaded_open, open);
    assert_eq!(loaded_open.sideband().unwrap(), open.sideband().unwrap());
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let receiver_key = KeyPair::new();
    let receiver_account = receiver_key.public_key().into();

    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        Amount::new(1),
    );

    let open = ctx.process_state_open(txn.as_mut(), &send, &receiver_key);
    let sideband = open.sideband().unwrap();

    assert_eq!(sideband.height, 1);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, false, true, false)
    );
}

#[test]
fn clear_pending() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let receiver_key = KeyPair::new();
    let receiver_account = receiver_key.public_key().into();

    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        Amount::new(1),
    );

    ctx.process_state_open(txn.as_mut(), &send, &receiver_key);

    let pending = ctx
        .ledger
        .store
        .pending()
        .get(txn.txn(), &PendingKey::new(receiver_account, send.hash()));
    assert_eq!(pending, None);
}

#[test]
fn add_account() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let receiver_key = KeyPair::new();
    let receiver_account = receiver_key.public_key().into();

    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        Amount::new(1),
    );

    let open = ctx.process_state_open(txn.as_mut(), &send, &receiver_key);

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &receiver_account)
        .unwrap();
    assert_eq!(ctx.ledger.cache.account_count.load(Ordering::Relaxed), 2);
    assert_eq!(account_info.balance, open.balance());
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.head, open.hash());
    assert_eq!(account_info.open_block, open.hash());
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let receiver_key = KeyPair::new();
    let receiver_account = receiver_key.public_key().into();

    let amount_sent = Amount::new(1);
    let send = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        amount_sent,
    );

    ctx.process_state_open(txn.as_mut(), &send, &receiver_key);

    let weight = ctx.ledger.weight(&receiver_account);
    assert_eq!(weight, amount_sent);
}
