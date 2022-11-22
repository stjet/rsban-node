use crate::{
    core::{
        Account, Amount, Block, BlockBuilder, BlockDetails, Epoch, KeyPair, SignatureVerification,
        StateBlockBuilder,
    },
    ledger::{ProcessResult, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::{epoch_v1::epoch_v1_block_for_genesis_account, LedgerContext};

pub(crate) fn epoch_v2_block_for_genesis_account() -> StateBlockBuilder {
    BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount)
        .link(*DEV_CONSTANTS.epochs.link(Epoch::Epoch2).unwrap())
        .sign(&DEV_GENESIS_KEY)
}

#[test]
fn epoch_blocks_v2_general() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let destination = KeyPair::new();
    let destination_account = Account::from(destination.public_key());

    let mut epoch1 = epoch_v2_block_for_genesis_account().build().unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch1, SignatureVerification::Unknown);

    // Trying to upgrade from epoch 0 to epoch 2. It is a requirement epoch upgrades are sequential unless the account is unopened
    assert_eq!(result.code, ProcessResult::BlockPosition);

    // Set it to the first epoch and it should now succeed
    let mut epoch1 = epoch_v1_block_for_genesis_account().build().unwrap();
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut epoch2 = epoch_v2_block_for_genesis_account()
        .previous(epoch1.hash())
        .build()
        .unwrap();

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

    let mut epoch3 = epoch_v2_block_for_genesis_account()
        .previous(epoch2.hash())
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch3, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);

    let genesis_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(genesis_info.epoch, Epoch::Epoch2);

    ctx.ledger
        .rollback(txn.as_mut(), &epoch1.hash(), &mut Vec::new())
        .unwrap();

    let genesis_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();
    assert_eq!(genesis_info.epoch, Epoch::Epoch0);
    ctx.process(txn.as_mut(), &mut epoch1);

    let mut change1 = BlockBuilder::change()
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut change1, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);

    let mut send1 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(destination_account)
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send1);

    let mut epoch4 = BlockBuilder::state()
        .account(destination_account)
        .previous(0)
        .representative(0)
        .balance(0)
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut epoch4);

    let mut epoch6 = BlockBuilder::state()
        .account(destination_account)
        .previous(epoch4.hash())
        .representative(0)
        .balance(0)
        .link(ctx.ledger.epoch_link(Epoch::Epoch2).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut epoch6);
    assert_eq!(
        epoch6.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, false, true)
    );
    // source_epoch is not used for send blocks
    assert_eq!(epoch6.sideband().unwrap().source_epoch, Epoch::Epoch0);

    let mut receive1 = BlockBuilder::receive()
        .previous(epoch6.hash())
        .source(send1.hash())
        .sign(&destination)
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut receive1, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);

    let mut receive2 = BlockBuilder::state()
        .account(destination_account)
        .previous(epoch6.hash())
        .representative(destination_account)
        .balance(Amount::new(50))
        .link(send1.hash())
        .sign(&destination)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut receive2);
    assert_eq!(
        receive2.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch2, false, true, false)
    );
    assert_eq!(receive2.sideband().unwrap().source_epoch, Epoch::Epoch1);
    assert_eq!(ctx.ledger.weight(&destination_account), Amount::new(50));
}
