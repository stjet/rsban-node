use std::sync::atomic::Ordering;

use crate::{
    core::{
        Amount, Block, BlockBuilder, BlockDetails, BlockEnum, BlockHash, Epoch, Link, PendingKey,
    },
    ledger::{
        ledger_tests::{setup_legacy_send_block, AccountBlockFactory},
        ProcessResult,
    },
};

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis.send(txn.txn()).link(receiver.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = receiver.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

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
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis.send(txn.txn()).link(receiver.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = receiver.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

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
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis.send(txn.txn()).link(receiver.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = receiver.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let pending = ctx
        .ledger
        .store
        .pending()
        .get(txn.txn(), &PendingKey::new(receiver.account(), send.hash()));
    assert_eq!(pending, None);
}

#[test]
fn add_account() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis.send(txn.txn()).link(receiver.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = receiver.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &receiver.account())
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
    let genesis = ctx.genesis_block_factory();
    let receiver = ctx.block_factory();

    let amount_sent = Amount::new(1);
    let mut send = genesis
        .send(txn.txn())
        .link(receiver.account())
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = receiver.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let weight = ctx.ledger.weight(&receiver.account());
    assert_eq!(weight, amount_sent);
}

#[test]
fn open_fork_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());
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

    assert_eq!(result.code, ProcessResult::Fork);
}

#[test]
fn previous_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let invalid_previous = BlockHash::from(1);
    let mut open = BlockBuilder::state()
        .account(send.destination.account())
        .previous(invalid_previous)
        .balance(send.amount_sent)
        .link(send.send_block.hash())
        .sign(&send.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result.code, ProcessResult::GapPrevious);
}

#[test]
fn source_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut open = BlockBuilder::state()
        .account(send.destination.account())
        .previous(BlockHash::zero())
        .balance(Amount::zero())
        .link(Link::zero())
        .sign(&send.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result.code, ProcessResult::GapSource);
}
