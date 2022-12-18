use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB, ledger_tests::AccountBlockFactory,
    DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};
use rsnano_core::{Account, Amount, Epoch, PendingInfo, PendingKey};

use super::LedgerContext;

#[test]
fn rollback_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &send.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
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

#[test]
fn rollback_receive() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let amount_sent = Amount::new(50);
    let mut send = genesis
        .send(txn.txn())
        .amount(amount_sent)
        .link(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis.receive(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &receive.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &receive.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount - amount_sent
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - amount_sent
    );
    assert_eq!(
        ctx.ledger.store.pending().get(
            txn.txn(),
            &PendingKey::new(*DEV_GENESIS_ACCOUNT, send.hash())
        ),
        Some(PendingInfo {
            source: *DEV_GENESIS_ACCOUNT,
            amount: amount_sent,
            epoch: Epoch::Epoch0
        })
    );
    assert_eq!(
        ctx.ledger.store.block().successor(txn.txn(), &send.hash()),
        None
    );
}

#[test]
fn rollback_received_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = destination.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &send.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.pending().exists(
            txn.txn(),
            &PendingKey::new(*DEV_GENESIS_ACCOUNT, send.hash())
        ),
        false
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send.hash()),
        false
    );
    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &open.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &destination.account(), false),
        Amount::zero()
    );
    assert_eq!(ctx.ledger.store.account().count(txn.txn()), 1);
}

#[test]
fn rollback_rep_change() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let representative = Account::from(1);

    let mut change = genesis
        .change(txn.txn())
        .representative(representative)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &change.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &change.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(ctx.ledger.weight(&representative), Amount::zero());
}

#[test]
fn rollback_open() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let amount_sent = Amount::new(50);
    let mut send = genesis
        .send(txn.txn())
        .link(destination.account())
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = destination.open(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &open.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &open.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &destination.account(), false),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - amount_sent
    );
    assert_eq!(
        ctx.ledger
            .get_pending(
                txn.txn(),
                &PendingKey::new(destination.account(), send.hash())
            )
            .unwrap(),
        PendingInfo {
            source: *DEV_GENESIS_ACCOUNT,
            amount: Amount::new(50),
            epoch: Epoch::Epoch0
        }
    );
    assert_eq!(ctx.ledger.store.account().count(txn.txn()), 1);
}

#[test]
fn rollback_send_with_rep_change() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let representative = Account::from(1);
    let mut send = genesis
        .send(txn.txn())
        .representative(representative)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &send.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &send.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
    assert_eq!(ctx.ledger.weight(&representative), Amount::zero());
}

#[test]
fn rollback_receive_with_rep_change() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let representative = Account::from(1);
    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis
        .receive(txn.txn(), send.hash())
        .representative(representative)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &receive.hash()).unwrap();

    assert_eq!(
        ctx.ledger.store.block().exists(txn.txn(), &receive.hash()),
        false
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        send.balance()
    );
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), send.balance());
    assert_eq!(ctx.ledger.weight(&representative), Amount::zero());
}
