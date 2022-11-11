use crate::{
    core::{Account, Amount, Block, BlockEnum},
    ledger::ledger_tests::LedgerWithSendBlock,
    DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

#[test]
fn saves_block() {
    let ctx = LedgerWithSendBlock::new();

    let loaded_send = ctx
        .ledger()
        .store
        .block()
        .get(ctx.txn.txn(), &ctx.send_block.hash())
        .unwrap();

    let BlockEnum::Send(loaded_send) = loaded_send else {panic!("not a send block")};
    assert_eq!(loaded_send, ctx.send_block);
    assert_eq!(
        loaded_send.sideband().unwrap(),
        ctx.send_block.sideband().unwrap()
    );
}

#[test]
fn updates_sideband() {
    let ctx = LedgerWithSendBlock::new();
    let sideband = ctx.send_block.sideband().unwrap();
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(sideband.height, 2);
    assert_eq!(sideband.balance, Amount::new(50));
}

#[test]
fn updates_block_amount() {
    let ctx = LedgerWithSendBlock::new();
    assert_eq!(
        ctx.ledger().amount(ctx.txn.txn(), &ctx.send_block.hash()),
        Some(ctx.amount_sent)
    );
}

#[test]
fn updates_receivable() {
    let ctx = LedgerWithSendBlock::new();
    assert_eq!(
        ctx.ledger()
            .account_receivable(ctx.txn.txn(), &ctx.receiver_account, false),
        ctx.amount_sent
    );
}

#[test]
fn updates_frontier_store() {
    let ctx = LedgerWithSendBlock::new();
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &DEV_GENESIS_HASH),
        Account::zero()
    );
    assert_eq!(
        ctx.ledger()
            .store
            .frontier()
            .get(ctx.txn.txn(), &ctx.send_block.hash()),
        *DEV_GENESIS_ACCOUNT
    );
}

#[test]
fn updates_account_info() {
    let ctx = LedgerWithSendBlock::new();
    let account_info = ctx
        .ledger()
        .store
        .account()
        .get(ctx.txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.block_count, 2);
    assert_eq!(account_info.head, ctx.send_block.hash());
}
