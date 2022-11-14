use crate::{
    core::{Account, Amount, Block, BlockBuilder, BlockEnum, SignatureVerification},
    ledger::{ledger_tests::LedgerWithOpenBlock, ProcessResult},
    work::DEV_WORK_POOL,
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT,
};

#[test]
fn update_sideband() {
    let ctx = LedgerWithOpenBlock::new();
    let sideband = ctx.open_block.sideband().unwrap();
    assert_eq!(sideband.account, ctx.receiver_account);
    assert_eq!(sideband.balance, ctx.amount_sent);
    assert_eq!(sideband.height, 1);
}

#[test]
fn save_block() {
    let ctx = LedgerWithOpenBlock::new();

    let loaded_open = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.open_block.hash())
        .unwrap();

    let BlockEnum::Open(loaded_open) = loaded_open else{panic!("not an open block")};
    assert_eq!(loaded_open, ctx.open_block);
    assert_eq!(
        loaded_open.sideband().unwrap(),
        ctx.open_block.sideband().unwrap()
    );
}

#[test]
fn update_block_amount() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger().amount(ctx.txn.txn(), &ctx.open_block.hash()),
        Some(ctx.amount_sent)
    );
    assert_eq!(
        ctx.ledger()
            .store
            .block()
            .account_calculated(&ctx.open_block),
        ctx.receiver_account
    );
}

#[test]
fn update_frontier_store() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.open_block.hash()),
        ctx.receiver_account
    );
}

#[test]
fn update_account_balance() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_balance(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.amount_sent
    );
}

#[test]
fn update_account_receivable() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        Amount::zero()
    );
}

#[test]
fn update_vote_weight() {
    let ctx = LedgerWithOpenBlock::new();
    assert_eq!(
        ctx.ledger().weight(&DEV_GENESIS_ACCOUNT),
        DEV_CONSTANTS.genesis_amount - ctx.amount_sent
    );
    assert_eq!(ctx.ledger().weight(&ctx.receiver_account), ctx.amount_sent);
}

#[test]
fn update_sender_account_info() {
    let ctx = LedgerWithOpenBlock::new();
    let sender_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(sender_info.head, ctx.send_block.hash());
}

#[test]
fn update_receiver_account_info() {
    let ctx = LedgerWithOpenBlock::new();
    let receiver_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &ctx.receiver_account)
        .unwrap();
    assert_eq!(receiver_info.head, ctx.open_block.hash());
}

#[test]
fn open_fork() {
    let mut ctx = LedgerWithOpenBlock::new();
    let mut open_fork = BlockBuilder::open()
        .source(ctx.send_block.hash())
        .representative(Account::from(1000))
        .account(ctx.receiver_account)
        .sign(ctx.receiver_key)
        .work(
            DEV_WORK_POOL
                .generate_dev2(ctx.send_block.hash().into())
                .unwrap(),
        )
        .build()
        .unwrap();

    let result = ctx.ledger_context.ledger.process(
        ctx.txn.as_mut(),
        &mut open_fork,
        SignatureVerification::Unknown,
    );

    assert_eq!(result.code, ProcessResult::Fork);
}
