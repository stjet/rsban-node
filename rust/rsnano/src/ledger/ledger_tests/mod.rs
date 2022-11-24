mod ledger_context;
use std::{ops::Deref, sync::atomic::Ordering};

pub(crate) use ledger_context::LedgerContext;

mod test_contexts;
pub(crate) use test_contexts::*;

use crate::{
    core::{
        Account, Amount, Block, BlockBuilder, BlockEnum, BlockHash, KeyPair, QualifiedRoot, Root,
        GXRB_RATIO,
    },
    DEV_CONSTANTS, DEV_GENESIS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::DEV_GENESIS_KEY;

mod account_block_factory;
mod empty_ledger;
mod epoch_v1;
mod epoch_v2;
mod process_change;
mod process_open;
mod process_receive;
mod process_send;
mod process_state_change;
mod process_state_open;
mod process_state_receive;
mod process_state_send;
mod rollback_change;
mod rollback_open;
mod rollback_receive;
mod rollback_send;
mod rollback_state;
pub(crate) use account_block_factory::AccountBlockFactory;

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

    ctx.ledger
        .rollback(txn.as_mut(), &receive.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::new(50));
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger
        .rollback(txn.as_mut(), &open.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&rep_account),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger
        .rollback(txn.as_mut(), &change.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - Amount::new(100)
    );

    ctx.ledger
        .rollback(txn.as_mut(), &send2.hash(), &mut Vec::new())
        .unwrap();

    assert_eq!(ctx.ledger.weight(&receiver.account()), Amount::zero());
    assert_eq!(ctx.ledger.weight(&rep_account), Amount::zero());
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - Amount::new(50)
    );

    ctx.ledger
        .rollback(txn.as_mut(), &send1.hash(), &mut Vec::new())
        .unwrap();

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
