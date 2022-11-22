use crate::{
    core::{
        Account, Amount, Block, BlockBuilder, BlockDetails, Epoch, KeyPair, PendingKey,
        SignatureVerification, StateBlockBuilder,
    },
    ledger::{ProcessResult, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerContext;

pub(crate) fn epoch_v1_block_for_genesis_account() -> StateBlockBuilder {
    BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount)
        .link(*DEV_CONSTANTS.epochs.link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
}

fn epoch_block_upgrades_epoch() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch = epoch_v1_block_for_genesis_account().build().unwrap();

    ctx.process(txn.as_mut(), &mut epoch);

    assert_eq!(
        epoch.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, false, true)
    );
    // source_epoch is not used for epoch blocks
    assert_eq!(epoch.sideband().unwrap().source_epoch, Epoch::Epoch0);
    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.epoch, Epoch::Epoch1);
}

fn adding_epoch_twice_fails() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch1 = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = epoch_v1_block_for_genesis_account()
        .previous(epoch1.hash())
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch2, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

fn cannot_add_old_change_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch);

    let mut change = BlockBuilder::change()
        .previous(epoch.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut change, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

fn can_add_state_blocks_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch);

    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(Account::from(1))
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send);

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
    let mut epoch = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch);

    ctx.ledger
        .rollback(txn.as_mut(), &epoch.hash(), &mut Vec::new())
        .unwrap();

    let account_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(account_info.epoch, Epoch::Epoch0);
}

#[test]
fn cannot_change_representative_with_epoch_block() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch = epoch_v1_block_for_genesis_account()
        .representative(Account::from(1))
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::RepresentativeMismatch);
}

#[test]
fn cannot_use_old_open_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let mut epoch = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch);

    let destination = KeyPair::new();
    let destination_account = destination.public_key().into();
    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send);

    let mut open = BlockBuilder::open()
        .source(send.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .account(destination_account)
        .sign(&destination)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::Unreceivable);
}

#[test]
fn cannot_use_old_receive_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let destination = KeyPair::new();
    let destination_account = Account::from(destination.public_key());

    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send);

    let mut epoch_open = BlockBuilder::state()
        .account(destination_account)
        .previous(0)
        .representative(0)
        .balance(0)
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut epoch_open);

    let mut receive = BlockBuilder::receive()
        .previous(epoch_open.hash())
        .source(send.hash())
        .sign(&destination)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);
}

fn can_add_state_receive_block_after_epoch1() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();

    let destination = KeyPair::new();
    let destination_account = Account::from(destination.public_key());

    let mut send = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send);

    let mut epoch_open = BlockBuilder::state()
        .account(destination_account)
        .previous(0)
        .representative(0)
        .balance(0)
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut epoch_open);

    let mut receive = BlockBuilder::state()
        .account(destination_account)
        .previous(epoch_open.hash())
        .representative(destination_account)
        .balance(Amount::new(50))
        .link(send.hash())
        .sign(&destination)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::Progress);
    assert_eq!(
        receive.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, true, false)
    );
    assert_eq!(receive.sideband().unwrap().source_epoch, Epoch::Epoch1);
}

#[test]
fn epoch_blocks_receive_upgrade() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let destination = KeyPair::new();
    let destination_account = Account::from(destination.public_key());

    let mut send1 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send1);

    let mut epoch1 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(send1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(send1.balance())
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut send2 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(100))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send2);

    let mut open1 = BlockBuilder::open()
        .source(send1.hash())
        .representative(destination_account)
        .account(destination_account)
        .sign(&destination)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut open1);

    let mut receive1 = BlockBuilder::receive()
        .previous(open1.hash())
        .source(send2.hash())
        .sign(&destination)
        .build()
        .unwrap();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive1, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::Unreceivable);

    let mut receive2 = BlockBuilder::state()
        .account(destination_account)
        .previous(open1.hash())
        .representative(destination_account)
        .balance(Amount::new(100))
        .link(send2.hash())
        .sign(&destination)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut receive2);

    assert_eq!(receive2.sideband().unwrap().details.epoch, Epoch::Epoch1);
    assert_eq!(receive2.sideband().unwrap().source_epoch, Epoch::Epoch1);

    let destination_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &destination_account)
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch1);

    ctx.ledger
        .rollback(txn.as_mut(), &receive2.hash(), &mut Vec::new())
        .unwrap();

    let destination_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &destination_account)
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch0);

    let pending_send2 = ctx
        .ledger
        .store
        .pending()
        .get(
            txn.txn(),
            &PendingKey::new(destination_account, send2.hash()),
        )
        .unwrap();
    assert_eq!(pending_send2.epoch, Epoch::Epoch1);

    ctx.process(txn.as_mut(), &mut receive2);

    let destination2 = KeyPair::new();
    let destination2_account = Account::from(destination2.public_key());
    let mut send3 = BlockBuilder::state()
        .account(destination_account)
        .previous(receive2.hash())
        .representative(destination_account)
        .balance(Amount::new(50))
        .link(destination2_account)
        .sign(&destination)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send3);

    let mut open2 = BlockBuilder::open()
        .source(send3.hash())
        .representative(destination2_account)
        .account(destination2_account)
        .sign(&destination2)
        .build()
        .unwrap();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open2, SignatureVerification::Unknown);
    assert_eq!(result.code, ProcessResult::Unreceivable);

    // Upgrade to epoch 2 and send to destination. Try to create an open block from an epoch 2 source block.
    let destination3 = KeyPair::new();
    let destination3_account = Account::from(destination3.public_key());
    let mut epoch2 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(send2.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(100))
        .link(ctx.ledger.epoch_link(Epoch::Epoch2).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut epoch2);
    let mut send4 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch2.hash())
        .account(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(150))
        .link(destination3_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send4);

    let mut open3 = BlockBuilder::open()
        .source(send4.hash())
        .representative(destination3_account)
        .account(destination3_account)
        .sign(&destination3)
        .build()
        .unwrap();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut open3, SignatureVerification::Unknown);
    assert_eq!(result.code, ProcessResult::Unreceivable);

    // Send it to an epoch 1 account
    let mut send5 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(send4.hash())
        .account(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(200))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send5);

    let destination_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &destination_account)
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch1);

    let mut receive3 = BlockBuilder::state()
        .account(destination_account)
        .previous(send3.hash())
        .representative(destination_account)
        .balance(100)
        .link(send5.hash())
        .sign(&destination)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut receive3);
    assert_eq!(receive3.sideband().unwrap().details.epoch, Epoch::Epoch2);
    assert_eq!(receive3.sideband().unwrap().source_epoch, Epoch::Epoch2);
    let destination_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &destination_account)
        .unwrap();
    assert_eq!(destination_info.epoch, Epoch::Epoch2);

    // Upgrade an unopened account straight to epoch 2
    let destination4 = KeyPair::new();
    let destination4_account = Account::from(destination4.public_key());
    let mut send6 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(send5.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(250))
        .link(destination4_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut send6);

    let mut epoch4 = BlockBuilder::state()
        .account(destination4_account)
        .previous(0)
        .representative(0)
        .balance(0)
        .link(ctx.ledger.epoch_link(Epoch::Epoch2).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    ctx.process(txn.as_mut(), &mut epoch4);
    assert_eq!(epoch4.sideband().unwrap().details.epoch, Epoch::Epoch2);
    assert_eq!(epoch4.sideband().unwrap().source_epoch, Epoch::Epoch0); // Not used for epoch blocks
}
