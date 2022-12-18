use rsnano_core::{Account, Amount, Block, BlockBuilder, BlockHash, KeyPair};

use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB,
    ledger_tests::{setup_legacy_send_block, LedgerContext},
    ProcessResult, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH, DEV_GENESIS_KEY,
};

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let loaded_send = ctx
        .ledger
        .store
        .block()
        .get(txn.txn(), &send.send_block.hash())
        .unwrap();

    assert_eq!(loaded_send, send.send_block);
    assert_eq!(
        loaded_send.sideband().unwrap(),
        send.send_block.sideband().unwrap()
    );
}

#[test]
fn update_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let sideband = send.send_block.sideband().unwrap();
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(sideband.height, 2);
    assert_eq!(
        sideband.balance,
        LEDGER_CONSTANTS_STUB.genesis_amount - send.amount_sent
    );
}

#[test]
fn update_block_amount() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.amount(txn.txn(), &send.send_block.hash()),
        Some(send.amount_sent)
    );
}

#[test]
fn update_receivable() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_receivable(txn.txn(), &send.destination.account(), false),
        send.amount_sent
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(ctx.ledger.get_frontier(txn.txn(), &DEV_GENESIS_HASH), None);
    assert_eq!(
        ctx.ledger.get_frontier(txn.txn(), &send.send_block.hash()),
        Some(*DEV_GENESIS_ACCOUNT)
    );
}

#[test]
fn update_account_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.block_count, 2);
    assert_eq!(account_info.head, send.send_block.hash());
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.weight(&send.destination.account()),
        Amount::zero()
    );
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - send.amount_sent
    );
}

#[test]
fn fail_duplicate_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut send = setup_legacy_send_block(&ctx, txn.as_mut());

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut send.send_block)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Old);
}

#[test]
fn fail_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut fork = BlockBuilder::legacy_send()
        .previous(*DEV_GENESIS_HASH)
        .destination(Account::from(1000))
        .sign(send.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut fork).unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn fail_gap_previous() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut block = BlockBuilder::legacy_send()
        .previous(BlockHash::from(1))
        .destination(Account::from(2))
        .sign(DEV_GENESIS_KEY.clone())
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut block).unwrap_err();

    assert_eq!(result, ProcessResult::GapPrevious);
}

#[test]
fn fail_bad_signature() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let wrong_keys = KeyPair::new();
    let mut block = BlockBuilder::legacy_send()
        .previous(*DEV_GENESIS_HASH)
        .destination(Account::from(2))
        .sign(wrong_keys)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut block).unwrap_err();

    assert_eq!(result, ProcessResult::BadSignature);
}

#[test]
fn fail_negative_spend() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut negative_spend = genesis
        .legacy_send(txn.txn())
        .balance(send.send_block.balance() + Amount::new(1))
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut negative_spend)
        .unwrap_err();
    assert_eq!(result, ProcessResult::NegativeSpend);
}

// Make sure old block types can't be inserted after a state block.
#[test]
fn send_after_state_fail() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut send1 = genesis.send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut send2 = genesis.legacy_send(txn.txn()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut send2).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn fail_insufficient_work() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let genesis = ctx.genesis_block_factory();

    let mut send = genesis.legacy_send(txn.txn()).work(0).build();
    {
        let block: &mut dyn Block = send.as_block_mut();
        block.set_work(0);
    };
    let result = ctx.ledger.process(txn.as_mut(), &mut send).unwrap_err();
    assert_eq!(result, ProcessResult::InsufficientWork);
}
