use crate::{
    core::{
        Account, Amount, Block, BlockBuilder, BlockDetails, Epoch, KeyPair, SignatureVerification,
    },
    ledger::{ProcessResult, DEV_GENESIS_KEY},
    DEV_CONSTANTS, DEV_GENESIS_ACCOUNT, DEV_GENESIS_HASH,
};

use super::LedgerContext;

#[test]
fn epoch_blocks_v1_general() {
    let ctx = LedgerContext::empty();
    let mut txn = ctx.ledger.rw_txn();
    let mut epoch1 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(*DEV_GENESIS_HASH)
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount)
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut epoch1);
    assert_eq!(
        epoch1.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, false, false, true)
    );

    // source_epoch is not used for epoch blocks
    assert_eq!(epoch1.sideband().unwrap().source_epoch, Epoch::Epoch0);

    let mut epoch2 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount)
        .link(ctx.ledger.epoch_link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();
    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut epoch2, SignatureVerification::Unknown);
    assert_eq!(result.code, ProcessResult::BlockPosition);

    let genesis_info = ctx
        .ledger
        .store
        .account()
        .get(txn.txn(), &DEV_GENESIS_ACCOUNT)
        .unwrap();

    assert_eq!(genesis_info.epoch, Epoch::Epoch1);

    // Rollback epoch1
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

    // reapply epoch1
    ctx.process(txn.as_mut(), &mut epoch1);

    // test that old blocks cannot be appended anymore
    let mut change1 = BlockBuilder::change()
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .sign(DEV_GENESIS_KEY.clone())
        .build()
        .unwrap();

    let result = ctx
        .ledger
        .process(txn.as_mut(), &mut change1, SignatureVerification::Unknown);

    assert_eq!(result.code, ProcessResult::BlockPosition);

    // test that state blocks can be appended
    let destination = KeyPair::new();
    let mut send1 = BlockBuilder::state()
        .account(*DEV_GENESIS_ACCOUNT)
        .previous(epoch1.hash())
        .representative(*DEV_GENESIS_ACCOUNT)
        .balance(DEV_CONSTANTS.genesis_amount - Amount::new(50))
        .link(Account::from(destination.public_key()))
        .sign(&DEV_GENESIS_KEY)
        .build()
        .unwrap();

    ctx.process(txn.as_mut(), &mut send1);

    assert_eq!(
        send1.sideband().unwrap().details,
        BlockDetails::new(Epoch::Epoch1, true, false, false)
    );
    // source_epoch is not used for send blocks
    assert_eq!(send1.sideband().unwrap().source_epoch, Epoch::Epoch0);
}
