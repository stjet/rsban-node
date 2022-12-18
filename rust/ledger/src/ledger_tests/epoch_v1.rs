use crate::{
    ledger_tests::{setup_legacy_send_block, upgrade_genesis_to_epoch_v1},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Account, BlockDetails, Epoch, PendingKey};

use super::LedgerContext;

#[test]
fn epoch_block_upgrades_epoch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let epoch = upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    assert_eq!(
        epoch.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, false, true)
    );
    // source_epoch is not used for epoch blocks
    assert_eq!(epoch.sideband().unwrap().source_epoch, Epoch::Epoch0);
    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.epoch, Epoch::Epoch1);
}

#[test]
fn adding_epoch_twice_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut epoch = ctx.genesis_block_factory().epoch_v1(txn.txn()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut epoch).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn adding_legacy_change_block_after_epoch1_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut change = ctx.genesis_block_factory().legacy_change(txn.txn()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut change).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn can_add_state_blocks_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut send = ctx.genesis_block_factory().send(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    assert_eq!(
        send.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, true, false, false)
    );
    // source_epoch is not used for send blocks
    assert_eq!(send.sideband().unwrap().source_epoch, Epoch::Epoch0);
}

#[test]
fn rollback_epoch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let epoch = upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    ctx.ledger.rollback(txn.as_mut(), &epoch.hash()).unwrap();

    let account_info = ctx
        .ledger
        .get_account_info(txn.txn(), &epoch.account())
        .unwrap();

    assert_eq!(account_info.epoch, Epoch::Epoch0);
}

#[test]
fn epoch_block_with_changed_representative_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();

    let mut epoch = genesis
        .epoch_v1(txn.txn())
        .representative(Account::from(1))
        .build();

    let result = ctx.ledger.process(txn.as_mut(), &mut epoch).unwrap_err();

    assert_eq!(result, ProcessResult::RepresentativeMismatch);
}

#[test]
fn cannot_use_legacy_open_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let destination = ctx.block_factory();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut send = ctx
        .genesis_block_factory()
        .send(txn.txn())
        .link(destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut open = destination.legacy_open(send.hash()).build();
    let result = ctx.ledger.process(txn.as_mut(), &mut open).unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn cannot_use_legacy_receive_block_after_epoch1_open() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut epoch_open = destination.epoch_v1_open().build();
    ctx.ledger.process(txn.as_mut(), &mut epoch_open).unwrap();

    let mut receive = destination.legacy_receive(txn.txn(), send.hash()).build();

    let result = ctx.ledger.process(txn.as_mut(), &mut receive).unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn cannot_use_legacy_receive_block_after_sender_upgraded_to_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut epoch1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open = destination.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut legacy_receive = destination.legacy_receive(txn.txn(), send2.hash()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut legacy_receive)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn can_add_state_receive_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let mut send = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let mut epoch_open = destination.epoch_v1_open().build();
    ctx.ledger.process(txn.as_mut(), &mut epoch_open).unwrap();

    let mut receive = destination.receive(txn.txn(), send.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(
        receive.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, true, false)
    );
    assert_eq!(receive.sideband().unwrap().source_epoch, Epoch::Epoch1);
}

#[test]
fn receiving_from_epoch1_sender_upgrades_receiver_to_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut epoch1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // open destination
    let mut open1 = destination.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();

    let mut receive2 = destination.receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    assert_eq!(receive2.sideband().unwrap().details.epoch, Epoch::Epoch1);
    assert_eq!(receive2.sideband().unwrap().source_epoch, Epoch::Epoch1);

    let destination_info = ctx
        .ledger
        .get_account_info(txn.txn(), &destination.account())
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch1);
}

#[test]
fn rollback_receive_block_which_performed_epoch_upgrade_undoes_epoch_upgrade() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut epoch1).unwrap();

    let mut send2 = genesis.send(txn.txn()).link(destination.account()).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut open1 = destination.legacy_open(send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open1).unwrap();

    let mut receive2 = destination.receive(txn.txn(), send2.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    ctx.ledger.rollback(txn.as_mut(), &receive2.hash()).unwrap();

    let destination_info = ctx
        .ledger
        .get_account_info(txn.txn(), &destination.account())
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch0);

    let pending_send2 = ctx
        .ledger
        .get_pending(
            txn.txn(),
            &PendingKey::new(destination.account(), send2.hash()),
        )
        .unwrap();
    assert_eq!(pending_send2.epoch, Epoch::Epoch1);
}

#[test]
fn epoch_v1_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let mut epoch_fork = ctx
        .genesis_block_factory()
        .epoch_v1(txn.txn())
        .previous(send.send_block.previous())
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch_fork)
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn successor_epoch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let mut send1 = genesis
        .legacy_send(txn.txn())
        .destination(destination.account())
        .build();
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let mut open = destination.open(txn.txn(), send1.hash()).build();
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let mut change = destination.change(txn.txn()).build();
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let account = Account::from_bytes(*open.hash().as_bytes());
    let mut send2 = genesis.legacy_send(txn.txn()).destination(account).build();
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let mut epoch_open = destination.epoch_v1_open().account(account).build();
    ctx.ledger.process(txn.as_mut(), &mut epoch_open).unwrap();

    assert_eq!(
        ctx.ledger.successor(txn.txn(), &change.qualified_root()),
        Some(change)
    );
    assert_eq!(
        ctx.ledger
            .successor(txn.txn(), &epoch_open.qualified_root()),
        Some(epoch_open)
    );
}
