use std::sync::atomic::Ordering;

use crate::{
    ledger_tests::{setup_open_block, setup_send_block},
    ProcessResult,
};
use rsnano_core::{BlockBuilder, BlockDetails, BlockHash, Epoch, Link, PendingKey};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_open_block(&ctx, txn.as_mut());

    let loaded_open = ctx
        .ledger
        .get_block(txn.txn(), &open.open_block.hash())
        .unwrap();

    assert_eq!(loaded_open, open.open_block);
    assert_eq!(
        loaded_open.sideband().unwrap(),
        open.open_block.sideband().unwrap()
    );
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_open_block(&ctx, txn.as_mut());

    let sideband = open.open_block.sideband().unwrap();
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

    let open = setup_open_block(&ctx, txn.as_mut());

    let pending = ctx.ledger.get_pending(
        txn.txn(),
        &PendingKey::new(open.destination.account(), open.send_block.hash()),
    );
    assert_eq!(pending, None);
}

#[test]
fn add_account() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_open_block(&ctx, txn.as_mut());

    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &open.destination.account())
        .unwrap();
    assert_eq!(ctx.ledger.cache.account_count.load(Ordering::Relaxed), 2);
    assert_eq!(account_info.balance, open.open_block.balance());
    assert_eq!(account_info.block_count, 1);
    assert_eq!(account_info.head, open.open_block.hash());
    assert_eq!(account_info.open_block, open.open_block.hash());
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_open_block(&ctx, txn.as_mut());

    let weight = ctx
        .ledger
        .weight(&open.open_block.representative().unwrap());
    assert_eq!(weight, open.open_block.balance());
}

#[test]
fn open_fork_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_send_block(&ctx, txn.as_mut());
    let receiver = send.destination;

    let mut open1 = receiver.open(txn.txn(), send.send_block.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();

    let mut open2 = BlockBuilder::state()
        .account(receiver.account())
        .previous(BlockHash::zero())
        .balance(send.amount_sent)
        .link(send.send_block.hash())
        .sign(&receiver.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open2).unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn previous_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_send_block(&ctx, txn.as_mut());

    let invalid_previous = BlockHash::from(1);
    let mut open = send
        .destination
        .open(txn.txn(), send.send_block.hash())
        .previous(invalid_previous)
        .build();
    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn source_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_send_block(&ctx, txn.as_mut());

    let mut open = send
        .destination
        .open(txn.txn(), send.send_block.hash())
        .link(Link::zero())
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::GapSource);
}
