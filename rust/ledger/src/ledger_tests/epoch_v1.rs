use crate::{
    ledger_tests::{setup_legacy_send_block, upgrade_genesis_to_epoch_v1},
    ProcessResult, DEV_GENESIS_ACCOUNT,
};
use rsnano_core::{Account, Block, BlockDetails, BlockEnum, Epoch, PendingKey};

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

    let epoch = ctx.genesis_block_factory().epoch_v1(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn adding_legacy_change_block_after_epoch1_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let change = ctx.genesis_block_factory().legacy_change(txn.txn()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Change(change))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn can_add_state_blocks_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let send = ctx.genesis_block_factory().send(txn.txn()).build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    assert_eq!(
        send.as_block().sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, true, false, false)
    );
    // source_epoch is not used for send blocks
    assert_eq!(
        send.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch0
    );
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

    let epoch = genesis
        .epoch_v1(txn.txn())
        .representative(Account::from(1))
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch))
        .unwrap_err();

    assert_eq!(result, ProcessResult::RepresentativeMismatch);
}

#[test]
fn cannot_use_legacy_open_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let destination = ctx.block_factory();
    upgrade_genesis_to_epoch_v1(&ctx, txn.as_mut());

    let send = ctx
        .genesis_block_factory()
        .send(txn.txn())
        .link(destination.account())
        .build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let open = destination.legacy_open(send.as_block().hash()).build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(open))
        .unwrap_err();

    assert_eq!(result, ProcessResult::Unreceivable);
}

#[test]
fn cannot_use_legacy_receive_block_after_epoch1_open() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let epoch_open = destination.epoch_v1_open().build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch_open))
        .unwrap();

    let receive = destination
        .legacy_receive(txn.txn(), send.as_block().hash())
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Receive(receive))
        .unwrap_err();

    assert_eq!(result, ProcessResult::BlockPosition);
}

#[test]
fn cannot_use_legacy_receive_block_after_sender_upgraded_to_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let send1 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send1 = BlockEnum::State(send1);
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let send2 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send2 = BlockEnum::State(send2);
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let open = destination.legacy_open(send1.as_block().hash()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(open))
        .unwrap();

    let legacy_receive = destination
        .legacy_receive(txn.txn(), send2.as_block().hash())
        .build();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::Receive(legacy_receive))
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

    let send = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send = BlockEnum::State(send);
    ctx.ledger.process(txn.as_mut(), &mut send).unwrap();

    let epoch_open = destination.epoch_v1_open().build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch_open))
        .unwrap();

    let receive = destination
        .receive(txn.txn(), send.as_block().hash())
        .build();
    let mut receive = BlockEnum::State(receive);
    ctx.ledger.process(txn.as_mut(), &mut receive).unwrap();

    assert_eq!(
        receive.as_block().sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, true, false)
    );
    assert_eq!(
        receive.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch1
    );
}

#[test]
fn receiving_from_epoch1_sender_upgrades_receiver_to_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let send1 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send1 = BlockEnum::State(send1);
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let send2 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send2 = BlockEnum::State(send2);
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    // open destination
    let open1 = destination.legacy_open(send1.as_block().hash()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(open1))
        .unwrap();

    let receive2 = destination
        .receive(txn.txn(), send2.as_block().hash())
        .build();
    let mut receive2 = BlockEnum::State(receive2);
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    assert_eq!(
        receive2.as_block().sideband().unwrap().details.epoch,
        Epoch::Epoch1
    );
    assert_eq!(
        receive2.as_block().sideband().unwrap().source_epoch,
        Epoch::Epoch1
    );

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

    let send1 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send1 = BlockEnum::State(send1);
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let epoch1 = genesis.epoch_v1(txn.txn()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch1))
        .unwrap();

    let send2 = genesis.send(txn.txn()).link(destination.account()).build();
    let mut send2 = BlockEnum::State(send2);
    ctx.ledger.process(txn.as_mut(), &mut send2).unwrap();

    let open1 = destination.legacy_open(send1.as_block().hash()).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::Open(open1))
        .unwrap();

    let receive2 = destination
        .receive(txn.txn(), send2.as_block().hash())
        .build();
    let mut receive2 = BlockEnum::State(receive2);
    ctx.ledger.process(txn.as_mut(), &mut receive2).unwrap();

    ctx.ledger
        .rollback(txn.as_mut(), &receive2.as_block().hash())
        .unwrap();

    let destination_info = ctx
        .ledger
        .get_account_info(txn.txn(), &destination.account())
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch0);

    let pending_send2 = ctx
        .ledger
        .get_pending(
            txn.txn(),
            &PendingKey::new(destination.account(), send2.as_block().hash()),
        )
        .unwrap();
    assert_eq!(pending_send2.epoch, Epoch::Epoch1);
}

#[test]
fn epoch_v1_fork() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let send = setup_legacy_send_block(&ctx, txn.as_mut());

    let epoch_fork = ctx
        .genesis_block_factory()
        .epoch_v1(txn.txn())
        .previous(send.send_block.previous())
        .build();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut BlockEnum::State(epoch_fork))
        .unwrap_err();

    assert_eq!(result, ProcessResult::Fork);
}

#[test]
fn successor_epoch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let genesis = ctx.genesis_block_factory();
    let destination = ctx.block_factory();

    let send1 = genesis
        .legacy_send(txn.txn())
        .destination(destination.account())
        .build();
    let mut send1 = BlockEnum::Send(send1);
    ctx.ledger.process(txn.as_mut(), &mut send1).unwrap();

    let open = destination.open(txn.txn(), send1.as_block().hash()).build();
    let mut open = BlockEnum::State(open);
    ctx.ledger.process(txn.as_mut(), &mut open).unwrap();

    let change = destination.change(txn.txn()).build();
    let mut change = BlockEnum::State(change);
    ctx.ledger.process(txn.as_mut(), &mut change).unwrap();

    let account = Account::from_bytes(*open.as_block().hash().as_bytes());
    let send2 = genesis.legacy_send(txn.txn()).destination(account).build();
    ctx.ledger
        .process(txn.as_mut(), &mut BlockEnum::Send(send2))
        .unwrap();

    let epoch_open = destination.epoch_v1_open().account(account).build();
    let mut epoch_open = BlockEnum::State(epoch_open);
    ctx.ledger.process(txn.as_mut(), &mut epoch_open).unwrap();

    assert_eq!(
        ctx.ledger
            .successor(txn.txn(), &change.as_block().qualified_root()),
        Some(change)
    );
    assert_eq!(
        ctx.ledger
            .successor(txn.txn(), &epoch_open.as_block().qualified_root()),
        Some(epoch_open)
    );
}
