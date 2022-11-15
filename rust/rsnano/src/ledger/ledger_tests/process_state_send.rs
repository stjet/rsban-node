use crate::{
    core::{Account, Amount, Block, BlockDetails, BlockEnum, Epoch, PendingInfo, PendingKey},
    ledger::{ledger_tests::LedgerContext, DEV_GENESIS_KEY},
    DEV_GENESIS_ACCOUNT,
};

#[test]
fn save_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let receiver_account = Account::from(1);
    let amount_sent = Amount::new(1);
    let block = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        amount_sent,
    );

    let BlockEnum::State(loaded_block) = ctx.ledger.store.block().get(txn.txn(), &block.hash()).unwrap() else {panic!("not a state block")};
    assert_eq!(loaded_block.sideband().unwrap(), block.sideband().unwrap());
    assert_eq!(loaded_block, block);
    assert_eq!(
        ctx.ledger.amount(txn.txn(), &block.hash()),
        Some(amount_sent)
    );
}

#[test]
fn update_pending_store() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let receiver_account = Account::from(1);
    let amount_sent = Amount::new(1);
    let block = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        amount_sent,
    );

    let pending_info = ctx
        .ledger
        .store
        .pending()
        .get(txn.txn(), &PendingKey::new(receiver_account, block.hash()))
        .unwrap();

    assert_eq!(
        pending_info,
        PendingInfo {
            source: block.account(),
            amount: amount_sent,
            epoch: Epoch::Epoch0
        }
    );
}

#[test]
fn create_sideband() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let receiver_account = Account::from(1);
    let amount_sent = Amount::new(1);
    let block = ctx.process_state_send(
        txn.as_mut(),
        &DEV_GENESIS_KEY,
        receiver_account,
        amount_sent,
    );

    let sideband = block.sideband().unwrap();
    assert_eq!(sideband.height, 2);
    assert_eq!(sideband.account, *DEV_GENESIS_ACCOUNT);
    assert_eq!(
        sideband.details,
        BlockDetails::new(Epoch::Epoch0, true, false, false)
    );
}
