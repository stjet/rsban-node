use std::ops::Deref;

use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB,
    ledger_tests::{setup_legacy_open_block, setup_legacy_receive_block, setup_legacy_send_block},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Account, Amount, Block, BlockBuilder, BlockHash, KeyPair};

use super::LedgerContext;

#[test]
fn update_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    let sideband = result.receive_block.sideband().unwrap();
    assert_eq!(sideband.account, result.destination.account());
    assert_eq!(sideband.balance, result.expected_balance);
    assert_eq!(sideband.height, 2);
}

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    let loaded_block = ctx
        .ledger
        .get_block(txn.txn(), &result.receive_block.hash())
        .unwrap();

    assert_eq!(loaded_block, result.receive_block);
    assert_eq!(
        loaded_block.sideband().unwrap(),
        result.receive_block.sideband().unwrap()
    );
}

#[test]
fn update_block_amount() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.amount(txn.txn(), &result.receive_block.hash()),
        Some(result.amount_received)
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .get_frontier(txn.txn(), &result.open_block.hash()),
        None
    );
    assert_eq!(
        ctx.ledger
            .get_frontier(txn.txn(), &result.receive_block.hash()),
        Some(result.destination.account())
    );
}

#[test]
fn update_balance() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &DEV_GENESIS_ACCOUNT, false),
        LEDGER_CONSTANTS_STUB.genesis_amount - result.expected_balance
    );
    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &result.destination.account(), false),
        result.expected_balance
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.weight(&result.destination.account()),
        result.expected_balance
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - result.expected_balance
    );
}

#[test]
fn update_account_receivable() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_receivable(txn.txn(), &result.destination.account(), false),
        Amount::zero()
    );
}

#[test]
fn update_latest_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let result = setup_legacy_receive_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.latest(txn.txn(), &result.destination.account()),
        Some(result.receive_block.hash())
    );
}

#[test]
fn receive_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let result = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut send = ctx
        .genesis_block_factory()
        .legacy_send(txn.txn())
        .destination(result.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut change = result.destination.legacy_change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let mut receive_fork = BlockBuilder::legacy_receive()
        .previous(result.open_block.hash())
        .source(send.hash())
        .sign(&result.destination.key)
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive_fork)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn fail_double_receive() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut double_receive = BlockBuilder::legacy_receive()
        .previous(open.open_block.hash())
        .source(open.send_block.hash())
        .sign(&open.destination.key)
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut double_receive)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn fail_old() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut receive = setup_legacy_receive_block(&ctx, txn.as_mut());

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive.receive_block)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Old);
}

#[test]
fn fail_gap_source() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut receive = BlockBuilder::legacy_receive()
        .previous(open.open_block.hash())
        .source(BlockHash::from(1))
        .sign(&open.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::GapSource);
}

#[test]
fn fail_bad_signature() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut send = ctx
        .genesis_block_factory()
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = BlockBuilder::legacy_receive()
        .previous(open.open_block.hash())
        .source(send.hash())
        .sign(&KeyPair::new())
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::BadSignature);
}

#[test]
fn fail_gap_previous_unopened() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut receive = BlockBuilder::legacy_receive()
        .previous(BlockHash::from(1))
        .source(send.send_block.hash())
        .sign(&send.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn fail_gap_previous_opened() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut receive = BlockBuilder::legacy_receive()
        .previous(BlockHash::from(1))
        .source(send2.hash())
        .sign(&open.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn fail_fork_previous() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut receivable = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receivable).unwrap();

    let mut fork_send = BlockBuilder::legacy_send()
        .previous(open.open_block.hash())
        .destination(Account::from(1))
        .balance(Amount::zero())
        .sign(open.destination.key.clone())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut fork_send).unwrap();

    let mut fork_receive = BlockBuilder::legacy_receive()
        .previous(open.open_block.hash())
        .source(receivable.hash())
        .sign(&open.destination.key)
        .build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut fork_receive)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn fail_receive_received_source() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut receivable1 = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .amount(Amount::new(1))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receivable1).unwrap();

    let mut receivable2 = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .amount(Amount::new(1))
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receivable2).unwrap();

    let mut receive = open
        .destination
        .legacy_receive(txn.txn(), receivable1.deref().deref().hash())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    let mut fork_receive = BlockBuilder::legacy_receive()
        .previous(open.open_block.hash())
        .source(receivable2.hash())
        .sign(&open.destination.key)
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut fork_receive)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

// Make sure old block types can't be inserted after a state block.
#[test]
fn receive_after_state_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.send(txn.txn()).link(genesis.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive = genesis.legacy_receive(txn.txn(), send.hash()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn receive_from_state_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = destination.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut receive = destination
        .receive(txn.txn(), send2.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(
        ctx.ledger.balance(txn.txn(), &receive.hash()),
        Amount::new(100)
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    )
}

#[test]
fn fail_insufficient_work() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut send = ctx
        .genesis_block_factory()
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut receive_block = open
        .destination
        .legacy_receive(txn.txn(), send.hash())
        .build();

    {
        let block: &mut dyn Block = receive_block.as_block_mut();
        block.set_work(0);
    };

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive_block)
        .unwrap_err();

    assert_eq!(result, ProcessResult::InsufficientWork);
}
