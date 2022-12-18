use crate::{
    ledger_tests::{setup_legacy_send_block, upgrade_genesis_to_epoch_v1, AccountBlockFactory},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Amount, Block, BlockDetails, BlockEnum, Epoch};

use super::LedgerContext;

#[test]
fn upgrade_from_v0_to_v2_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let epoch = genesis.epoch_v2(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch))
        .unwrap_err();

    // Trying to upgrade from epoch 0 to epoch 2. It is a requirement epoch upgrades are sequential unless the account is unopened
    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn upgrade_to_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let epoch2 = ctx.genesis_block_factory().epoch_v2(txn.txn()).build();
    let mut epoch2 = BlockEnum::State(epoch2);
    ctx.ledger.process(txn.as_mut(), &mut epoch2).unwrap();

    assert_eq!(
        epoch2.as_block().sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, false, true)
    );
    // source_epoch is not used for send blocks
    assert_eq!(
        epoch2.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch0
    );

    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(account_info.epoch, Epoch::Epoch2);
}

#[test]
fn upgrading_to_epoch_v2_twice_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let genesis = ctx.genesis_block_factory();

    let epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let epoch3 = genesis.epoch_v2(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch3))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn rollback_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());
    let genesis = ctx.genesis_block_factory();

    let epoch = genesis.epoch_v2(txn.txn()).build();
    let mut epoch = BlockEnum::State(epoch);
    ctx.ledger.process(txn.as_mut(), &mut epoch).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &epoch.as_block().hash())
        .unwrap();

    let genesis_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(genesis_info.epoch, Epoch::Epoch1);

    let legacy_change = genesis.legacy_change(txn.txn()).build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Change(legacy_change))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn upgrade_epoch_after_state_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(send))
        .unwrap();

    let state_open = destination.epoch_v1_open().build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(state_open))
        .unwrap();

    let epoch2 = destination.epoch_v2(txn.txn()).build();
    let mut epoch2 = BlockEnum::State(epoch2);
    ctx.ledger.process(txn.as_mut(), &mut epoch2).unwrap();

    assert_eq!(
        epoch2.as_block().sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, false, true)
    );
    // source_epoch is not used for send blocks
    assert_eq!(
        epoch2.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch0
    );
}

#[test]
fn legacy_receive_block_after_epoch_v2_upgrade_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let epoch1 = destination.epoch_v1_open().build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let epoch2 = destination.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let legacy_receive = destination
        .legacy_receive(txn.txn(), send.as_block().hash())
        .build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Receive(legacy_receive))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn cannot_use_legacy_open_block_with_epoch_v2_send() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    // Try to create an open block from an epoch 2 source block.
    let legacy_open = destination.legacy_open(send.as_block().hash()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(legacy_open))
        .unwrap_err();
    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn receive_after_epoch_v2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let send = genesis
        .send(txn.txn())
        .amount(Amount::new(50))
        .link(destination.account())
        .build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let epoch1 = destination.epoch_v1_open().build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let epoch2 = destination.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let receive = destination
        .receive(txn.txn(), send.as_block().hash())
        .representative(destination.account())
        .build();
    let mut receive = BlockEnum::State(receive);
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(
        receive.as_block().sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, true, false)
    );
    assert_eq!(
        receive.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch1
    );
    assert_eq!(ctx.ledger.weight(&destination.account()), Amount::new(50));
}

#[test]
fn receiving_from_epoch_2_block_upgrades_receiver_to_epoch2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let send1 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send1 = BlockEnum::State(send1);
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let open1 = destination.legacy_open(send1.as_block().hash()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(open1))
        .unwrap();

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let send2 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send2 = BlockEnum::State(send2);
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let receive2 = destination
        .receive(txn.txn(), send2.as_block().hash())
        .build();
    let mut receive2 = BlockEnum::State(receive2);
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    assert_eq!(
        receive2.as_block().sideband().unwrap().details.epoch,
        Epoch::Epoch2
    );
    assert_eq!(
        receive2.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch2
    );
    let destination_info = ctx
        .ledger
        .get_account_info(txn.txn(), &destination.account())
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch2);
}

#[test]
fn upgrade_new_account_straight_to_epoch_2() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = AccountBlockFactory::new(&ctx.ledger);

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let epoch2 = genesis.epoch_v2(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch2))
        .unwrap();

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(send))
        .unwrap();

    let open = destination.epoch_v2_open().build();
    let mut open = BlockEnum::State(open);
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    assert_eq!(
        open.as_block().sideband().unwrap().details.epoch,
        Epoch::Epoch2
    );
    assert_eq!(
        open.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch0
    ); // Not used for epoch blocks
}

#[test]
fn epoch_v2_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let epoch_fork = ctx
        .genesis_block_factory()
        .epoch_v2(txn.txn())
        .previous(send.send_block.previous())
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch_fork))
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn epoch_v2_fork_with_epoch_v1_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let epoch_v1 = upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let genesis = ctx.genesis_block_factory();
    let epoch_v2_fork = genesis
        .epoch_v2(txn.txn())
        .previous(epoch_v1.previous())
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch_v2_fork))
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}
