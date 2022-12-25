use crate::{
    ledger_constants::LEDGER_CONSTANTS_STUB,
    ledger_tests::{setup_legacy_open_block, setup_legacy_send_block, LedgerContext},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Account, Amount, Block, BlockBuilder, BlockHash, KeyPair};

#[test]
fn update_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let sideband = open.open_block.sideband().unwrap();
    assert_eq!(sideband.account, open.destination.account());
    assert_eq!(sideband.balance, open.expected_balance);
    assert_eq!(sideband.height, 1);
}

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

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
fn update_block_amount() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.amount(txn.txn(), &open.open_block.hash()),
        Some(open.expected_balance)
    );
    assert_eq!(
        open.open_block.account_calculated(),
        open.destination.account()
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.get_frontier(txn.txn(), &open.open_block.hash()),
        Some(open.destination.account())
    );
}

#[test]
fn update_account_balance() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_balance(txn.txn(), &open.destination.account(), false),
        open.expected_balance
    );
}

#[test]
fn update_account_receivable() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger
            .account_receivable(txn.txn(), &open.destination.account(), false),
        Amount::zero()
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount - open.expected_balance
    );
    assert_eq!(
        ctx.ledger.weight(&open.destination.account()),
        open.expected_balance
    );
}

#[test]
fn update_sender_account_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let sender_info = ctx
        .ledger
        .get_account_info(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(sender_info.head, open.send_block.hash());
}

#[test]
fn update_receiver_account_info() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let receiver_info = ctx
        .ledger
        .get_account_info(txn.txn(), &open.destination.account())
        .unwrap();
    assert_eq!(receiver_info.head, open.open_block.hash());
}

#[test]
fn fail_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut open_fork = open
        .destination
        .legacy_open(open.send_block.hash())
        .representative(Account::from(1000))
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open_fork)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn fail_fork_previous() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut send2 = genesis
        .legacy_send(txn.txn())
        .destination(open.destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open_fork = BlockBuilder::legacy_open()
        .source(send2.hash())
        .account(open.destination.account())
        .sign(&open.destination.key)
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open_fork)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn process_duplicate_open_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut open = setup_legacy_open_block(&ctx, txn.as_mut());

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open.open_block)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Old);
}

#[test]
fn fail_gap_source() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let destination = ctx.block_factory();

    let mut open = destination.legacy_open(BlockHash::from(1)).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::GapSource);
}

#[test]
fn fail_bad_signature() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let bad_keys = KeyPair::new();

    let mut open = BlockBuilder::legacy_open()
        .source(send.send_block.hash())
        .account(send.destination.account())
        .sign(&bad_keys)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();
    assert_eq!(result, ProcessResult::BadSignature);
}

#[test]
fn fail_account_mismatch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());
    let bad_key = ctx.block_factory();

    let mut open = bad_key.legacy_open(send.send_block.hash()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn state_open_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let mut open2 = BlockBuilder::legacy_open()
        .source(open.send_block.hash())
        .account(open.destination.account())
        .sign(&open.destination.key)
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut open2).unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn open_from_state_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();
    let amount_sent = Amount::new(50);
    let mut send = genesis
        .send(txn.txn())
        .link(destination.account())
        .amount(amount_sent)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = destination
        .legacy_open(send.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    assert_eq!(ctx.ledger.balance(txn.txn(), &open.hash()), amount_sent);
    assert_eq!(
        ctx.ledger.weight(&DEV_GENESIS_ACCOUNT),
        LEDGER_CONSTANTS_STUB.genesis_amount
    );
}

#[test]
fn confirmation_height_not_updated() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let open = setup_legacy_open_block(&ctx, txn.as_mut());

    let confirmation_height_info = ctx
        .ledger
        .get_confirmation_height(txn.txn(), &open.open_block.account());
    assert_eq!(confirmation_height_info, None);
}

#[test]
fn fail_insufficient_work() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut open = send
        .destination
        .legacy_open(send.send_block.hash())
        .work(0)
        .build();

    {
        let block: &mut dyn Block = open.as_block_mut();
        block.set_work(0);
    };
    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::InsufficientWork);
}
