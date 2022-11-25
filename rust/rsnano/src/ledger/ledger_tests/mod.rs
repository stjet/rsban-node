use std::{ops::Deref, sync::atomic::Ordering};

mod helpers;
pub(crate) use helpers::*;

use crate::{
    core::{
        Account, Amount, Block, BlockBuilder, BlockEnum, BlockHash, Epoch, KeyPair, QualifiedRoot,
        Root, GXRB_RATIO,
    },
    DEV_CONSTANTS, DEV_GENESIS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::DEV_GENESIS_KEY;

mod empty_ledger;
mod epoch_v1;
mod epoch_v2;
mod process_change;
mod process_legacy_change;
mod process_legacy_open;
mod process_legacy_receive;
mod process_legacy_send;
mod process_open;
mod process_receive;
mod process_send;
mod rollback_legacy_change;
mod rollback_legacy_open;
mod rollback_legacy_receive;
mod rollback_legacy_send;
mod rollback_state;

#[test]
fn ledger_successor() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.successor(
            txn.txn(),
            &QualifiedRoot::new(Root::zero(), *DEV_GENESIS_HASH)
        ),
        Some(BlockEnum::Send(send.send_block))
    );
}

#[test]
fn ledger_successor_genesis() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    setup_legacy_send_block(&ctx, txn.as_mut());
    let genesis = DEV_GENESIS.read().unwrap().clone();

    assert_eq!(
        ctx.ledger.successor(
            txn.txn(),
            &QualifiedRoot::new(DEV_GENESIS_ACCOUNT.deref().into(), BlockHash::zero())
        ),
        Some(genesis)
    );
}

#[test]
fn latest_root_empty() {
    let ctx = LedgerContext::empty();
    let txn = ctx.ledger.read_txn();
    assert_eq!(
        ctx.ledger.latest_root(txn.txn(), &Account::from(1)),
        Root::from(1)
    );
}

#[test]
fn latest_root() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.latest_root(txn.txn(), &DEV_GENESIS_ACCOUNT),
        send.send_block.hash().into()
    );
}

#[test]
fn send_open_receive_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = ctx.block_factory();

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = receiver.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut receive = receiver.legacy_receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::new(100));
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );
}

#[test]
fn send_open_receive_rollback() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let receiver = AccountBlockFactory::new(&ctx.ledger);

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(receiver.account())
        .amount(Amount::new(50))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = receiver.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut receive = receiver.legacy_receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();
    let rep_account = Account::from(1);

    let mut change = genesis
        .legacy_change(txn.txn())
        .representative(rep_account)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &receive.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::new(50));
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &open.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &change.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger.rollback(txn.as_mut(), &send2.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - Amount::new(50)
    );

    ctx.ledger.rollback(txn.as_mut(), &send1.hash()).unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount
    );
}

#[test]
fn bootstrap_rep_weight() {
    let ctx = LedgerContext::empty();
    ctx.ledger.set_bootstrap_weight_max_blocks(3);
    let genesis = ctx.genesis_block_factory();
    let representative_key = KeyPair::new();
    let representative_account = representative_key.public_key().into();
    {
        let mut txn = ctx.ledger.rw_txn();
        let mut send = genesis
            .legacy_send(txn.txn())
            .destination(representative_account)
            .amount(Amount::new(50))
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    }
    {
        let mut weights = ctx.ledger.bootstrap_weights.lock().unwrap();
        weights.insert(representative_account, Amount::new(1000));
    }
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 2);
    assert_eq!(
        ctx.ledger.weight(&representative_account),
        Amount::new(1000)
    );
    {
        let mut txn = ctx.ledger.rw_txn();
        let mut send = genesis
            .legacy_send(txn.txn())
            .destination(representative_account)
            .amount(Amount::new(50))
            .build();
        ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    }
    assert_eq!(ctx.ledger.cache.block_count.load(Ordering::Relaxed), 3);
    assert_eq!(ctx.ledger.weight(&representative_account), Amount::zero());
}

#[test]
fn block_destination_source() {
    let ctx = LedgerContext::empty();
    let ledger = &ctx.ledger;
    let mut txn = ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let dest_account = Account::from(1000);

    let mut send_to_dest = genesis
        .legacy_send(txn.txn())
        .destination(dest_account)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send_to_dest).unwrap();

    let mut send_to_self = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send_to_self).unwrap();

    let mut receive = genesis
        .legacy_receive(txn.txn(), send_to_self.hash())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    let mut send_to_dest_2 = genesis.send(txn.txn()).link(dest_account).build();
    ctx.ledger
        .process(txn.as_mut(), &mut send_to_dest_2)
        .unwrap();

    let mut send_to_self_2 = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut send_to_self_2)
        .unwrap();

    let mut receive2 = genesis.receive(txn.txn(), send_to_self_2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    let block1 = BlockEnum::Send(send_to_dest);
    let block2 = BlockEnum::Send(send_to_self);
    let block3 = BlockEnum::Receive(receive);
    let block4 = BlockEnum::State(send_to_dest_2);
    let block5 = BlockEnum::State(send_to_self_2);
    let block6 = BlockEnum::State(receive2);

    assert_eq!(
        ledger.balance(txn.txn(), &block6.as_block().hash()),
        block6.as_block().balance()
    );
    assert_eq!(ledger.block_destination(txn.txn(), &block1), dest_account);
    assert_eq!(ledger.block_source(txn.txn(), &block1), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block2),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(ledger.block_source(txn.txn(), &block2), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block3),
        Account::zero()
    );
    assert_eq!(
        ledger.block_source(txn.txn(), &block3),
        block2.as_block().hash()
    );

    assert_eq!(ledger.block_destination(txn.txn(), &block4), dest_account);
    assert_eq!(ledger.block_source(txn.txn(), &block4), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block5),
        *DEV_GENESIS_ACCOUNT
    );
    assert_eq!(ledger.block_source(txn.txn(), &block5), BlockHash::zero());

    assert_eq!(
        ledger.block_destination(txn.txn(), &block6),
        Account::zero()
    );
    assert_eq!(
        ledger.block_source(txn.txn(), &block6),
        block5.as_block().hash()
    );
}

#[test]
fn state_account() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(*GXRB_RATIO))
        .link(*DEV_GENESIS_ACCOUNT)
        .sign(&DEV_GENESIS_KEY)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();
    assert_eq!(
        ctx.ledger.account(txn.txn(), &send.hash()),
        Some(*DEV_GENESIS_ACCOUNT)
    );
}

#[test]
fn could_fit() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    // Test legacy and state change blocks could_fit
    let mut change1 = genesis.legacy_change(txn.txn()).build();
    let change2 = genesis.change(txn.txn()).build();
    assert!(ctx.ledger.could_fit(txn.txn(), &change1));
    assert!(ctx.ledger.could_fit(txn.txn(), &change2));

    // Test legacy and state send
    let send1 = genesis
        .legacy_send(txn.txn())
        .previous(change1.hash())
        .destination(destination.account())
        .build();
    let mut send2 = genesis
        .send(txn.txn())
        .previous(change1.hash())
        .amount(Amount::new(1))
        .link(destination.account())
        .build();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &send1), false);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &send2), false);

    ctx.ledger.process(txn.as_mut(), &mut change1).unwrap();
    assert!(ctx.ledger.could_fit(txn.txn(), &change1));
    assert!(ctx.ledger.could_fit(txn.txn(), &change2));
    assert!(ctx.ledger.could_fit(txn.txn(), &send1));
    assert!(ctx.ledger.could_fit(txn.txn(), &send2));

    // Test legacy and state open
    let mut open1 = destination.legacy_open(send2.hash()).build();
    let open2 = BlockBuilder::state()
        .account(destination.account())
        .previous(0)
        .representative(genesis.account())
        .balance(Amount::new(1))
        .link(send2.hash())
        .sign(&destination.key)
        .without_sideband()
        .build();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &open1), false);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &open2), false);

    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();
    assert!(ctx.ledger.could_fit(txn.txn(), &send1));
    assert!(ctx.ledger.could_fit(txn.txn(), &send2));
    assert!(ctx.ledger.could_fit(txn.txn(), &open1));
    assert!(ctx.ledger.could_fit(txn.txn(), &open2));

    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();
    assert!(ctx.ledger.could_fit(txn.txn(), &open1));
    assert!(ctx.ledger.could_fit(txn.txn(), &open2));

    // Create another send to receive
    let mut send3 = genesis
        .send(txn.txn())
        .link(destination.account())
        .amount(Amount::new(1))
        .build();

    // Test legacy and state receive
    let mut receive1 = destination.legacy_receive(txn.txn(), send3.hash()).build();
    let receive2 = BlockBuilder::state()
        .account(destination.account())
        .previous(open1.hash())
        .balance(Amount::new(2))
        .link(send3.hash())
        .sign(&destination.key)
        .without_sideband()
        .build();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive1), false);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive2), false);

    ctx.ledger.process(txn.as_mut(), &mut send3).unwrap();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive1), true);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive2), true);

    // Test epoch (state)
    let mut epoch1 = BlockBuilder::state()
        .account(destination.account())
        .previous(receive1.hash())
        .representative(open1.representative())
        .balance(Amount::new(2))
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&genesis.key)
        .build();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &epoch1), false);
    ctx.ledger.process(txn.as_mut(), &mut receive1).unwrap();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive1), true);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &receive2), true);
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &epoch1), true);

    ctx.ledger.process(txn.as_mut(), &mut epoch1).unwrap();
    assert_eq!(ctx.ledger.could_fit(txn.txn(), &epoch1), true);
}
