use crate::{ProcessResult, DEV_GENESIS_ACCOUNT};
use rsnano_core::{
    Account, Amount, Block, BlockBuilder, BlockDetails, BlockEnum, BlockHash, Epoch, KeyPair, Link,
    PendingKey, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use super::LedgerContext;

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let (_, receive) = receive_50_raw_into_genesis(&ctx, txn.as_mut());

    let loaded_block = ctx.ledger.get_block(txn.txn(), &receive.hash()).unwrap();

    let BlockEnum::State(loaded_block) = loaded_block else { panic!("not a state block")};
    assert_eq!(loaded_block, receive);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        receive.sideband().unwrap()
    );
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let (_, receive) = receive_50_raw_into_genesis(&ctx, txn.as_mut());

    let sideband = receive.sideband().unwrap();
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(sideband.height, 3);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, false, true, false)
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let (_, receive) = receive_50_raw_into_genesis(&ctx, txn.as_mut());

    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), receive.balance());
}

#[test]
fn remove_pending_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let (send, _) = receive_50_raw_into_genesis(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.pending_info(
            txn.txn(),
            &PendingKey::new(*DEV_GENESIS_ACCOUNT, send.hash())
        ),
        None
    );
}

#[test]
fn receive_old_send_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis
        .legacy_send(txn.txn())
        .destination(genesis.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis.receive(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    let sideband = receive.sideband().unwrap();
    assert_eq!(sideband.account, genesis.account());
    assert_eq!(sideband.height, 3);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, false, true, false)
    );

    let loaded_block = ctx.ledger.get_block(txn.txn(), &receive.hash()).unwrap();
    assert_eq!(loaded_block, receive);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        receive.sideband().unwrap()
    );
}

#[test]
fn state_unreceivable_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis
        .receive(txn.txn(), send.hash())
        .link(Link::from(1))
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::GapSource);
}

#[test]
fn bad_amount_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis
        .receive(txn.txn(), send.hash())
        .balance(send.balance())
        .build();
    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::BalanceMismatch);
}

#[test]
fn no_link_amount_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis
        .receive(txn.txn(), send.hash())
        .link(Link::zero())
        .build();
    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::BalanceMismatch);
}

#[test]
fn receive_wrong_account_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let key = KeyPair::new();
    let mut receive = BlockBuilder::state()
        .account(key.public_key())
        .previous(BlockHash::zero())
        .balance(Amount::raw(1))
        .link(send.hash())
        .sign(&key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn receive_and_change_representative() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let amount_sent = Amount::raw(50);
    let mut send = genesis
        .send(txn.txn())
        .link(genesis.account())
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let representative = Account::from(1);
    let mut receive = genesis
        .receive(txn.txn(), send.hash())
        .representative(representative)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(
        ctx.ledger.balance(txn.txn(), &receive.hash()),
        receive.balance()
    );
    assert_eq!(
        ctx.ledger.amount(txn.txn(), &receive.hash()).unwrap(),
        amount_sent,
    );
    assert_eq!(ctx.ledger.weight(&DEV_GENESIS_ACCOUNT), Amount::zero());
    assert_eq!(ctx.ledger.weight(&representative), receive.balance());
    assert_eq!(
        receive.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch0, false, true, false)
    );
}

fn receive_50_raw_into_genesis(
    ctx: &LedgerContext,
    txn: &mut dyn WriteTransaction,
) -> (StateBlock, StateBlock) {
    let genesis = ctx.genesis_block_factory();
    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn, &mut send).unwrap();

    let mut receive = genesis.receive(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn, &mut receive).unwrap();

    let BlockEnum::State(send) = send else {unreachable!()};
    let BlockEnum::State(receive) = receive else {unreachable!()};
    (send, receive)
}
