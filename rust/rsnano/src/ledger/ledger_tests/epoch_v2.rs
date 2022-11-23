use crate::{
    core::{Amount, Block, BlockDetails, Epoch, SignatureVerification},
    ledger::{ledger_tests::AccountBlockFactory, ProcessResult},
    DEV_GENESIS_ACCOUNT,
};

use super::LedgerContext;

#[test]
fn upgrade_from_v0_to_v2_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut epoch = genesis.epoch_v2(txn.txn()).build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch, SignatureVerification::Unknown);

    // Trying to upgrade from epoch 0 to epoch 2. It is a requirement epoch upgrades are sequential unless the account is unopened
    assert_eq!(result.code, ProcessResult::BlockPosition);
}

#[test]
fn upgrade_to_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch2, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::Progress);

    assert_eq!(
        epoch2.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, false, true)
    );
    // source_epoch is not used for send blocks
    assert_eq!(epoch2.sideband().unwrap().source_epoch, Epoch::Epoch0);

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.epoch, Epoch::Epoch2);
}

#[test]
fn upgrading_to_epoch_v2_twice_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut epoch3 = genesis.epoch_v2(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch3, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

#[test]
fn rollback_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    ctx.ledger
        .rollback(txn.as_mut(), &epoch2.hash(), &mut Vec::new())
        .unwrap();

    let genesis_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(genesis_info.epoch, Epoch::Epoch1);

    let mut legacy_change = genesis
        .change_representative(txn.txn(), *DEV_GENESIS_ACCOUNT)
        .build();

    let result = ctx.ledger.process(
        txn.as_mut(),
        &mut legacy_change,
        SignatureVerification::Unknown,
    );

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

#[test]
fn upgrade_epoch_after_state_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send);

    let mut state_open = destination.epoch_v1_open().build();
    ctx.process(txn.as_mut(), &mut state_open);

    let mut epoch2 = destination.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    assert_eq!(
        epoch2.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, false, true)
    );
    // source_epoch is not used for send blocks
    assert_eq!(epoch2.sideband().unwrap().source_epoch, Epoch::Epoch0);
}

#[test]
fn legacy_receive_block_after_epoch_v2_upgrade_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut send = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send);

    let mut epoch1 = destination.epoch_v1_open().build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = destination.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut legacy_receive = destination.receive(txn.txn(), send.hash()).build();

    let result = ctx.ledger.process(
        txn.as_mut(),
        &mut legacy_receive,
        SignatureVerification::Unknown,
    );

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

#[test]
fn cannot_use_legacy_open_block_with_epoch_v2_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut send = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send);

    // Try to create an open block from an epoch 2 source block.
    let mut legacy_open = destination.open(send.hash()).build();
    let result = ctx.ledger.process(
        txn.as_mut(),
        &mut legacy_open,
        SignatureVerification::Unknown,
    );
    assert_eq!(result.code, ProcessResult::Unreceivable);
}

#[test]
fn receive_after_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut send = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send);

    let mut epoch1 = destination.epoch_v1_open().build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = destination.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut receive = destination
        .state_receive(txn.txn(), send.hash())
        .representative(destination.account())
        .build();
    ctx.process(txn.as_mut(), &mut receive);

    assert_eq!(
        receive.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, true, false)
    );
    assert_eq!(receive.sideband().unwrap().source_epoch, Epoch::Epoch1);
    assert_eq!(ctx.ledger.weight(&destination.account()), Amount::new(50));
}

#[test]
fn receiving_from_epoch_2_block_upgrades_receiver_to_epoch2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut send1 = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send1);

    let mut open1 = destination.open(send1.hash()).build();
    ctx.process(txn.as_mut(), &mut open1);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut send2 = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send2);

    let mut receive2 = destination.state_receive(txn.txn(), send2.hash()).build();
    ctx.process(txn.as_mut(), &mut receive2);

    assert_eq!(receive2.sideband().unwrap().details.epoch, Epoch::Epoch2);
    assert_eq!(receive2.sideband().unwrap().source_epoch, Epoch::Epoch2);
    let destination_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &destination.account())
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch2);
}

#[test]
fn upgrade_new_account_straight_to_epoch_2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = AccountBlockFactory::genesis(&ctx.ledger);
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.process(txn.as_mut(), &mut epoch2);

    let mut send = genesis
        .state_send(txn.txn(), destination.account(), Amount::new(50))
        .build();
    ctx.process(txn.as_mut(), &mut send);

    let mut open = destination.epoch_v2_open().build();
    ctx.process(txn.as_mut(), &mut open);

    assert_eq!(open.sideband().unwrap().details.epoch, Epoch::Epoch2);
    assert_eq!(open.sideband().unwrap().source_epoch, Epoch::Epoch0); // Not used for epoch blocks
}
