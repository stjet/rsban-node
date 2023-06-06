use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, BlockBuilder, BlockDetails, BlockEnum, Epoch,
    KeyPair,
};

use crate::{
    block_insertion::{BlockInsertInstructions, BlockValidator},
    ledger_constants::LEDGER_CONSTANTS_STUB,
    ProcessResult, DEV_GENESIS_KEY,
};

use super::create_account_info;

#[test]
fn updgrade_to_epoch_v1() {
    let open = BlockBuilder::legacy_open().with_sideband().build();
    let old_account_info = create_account_info(&open);
    let epoch = create_epoch_block(&open);

    let result = validate(epoch, open, old_account_info).unwrap();

    assert_eq!(result.set_account_info.epoch, Epoch::Epoch1);
    assert_eq!(
        result.set_sideband.details,
        BlockDetails::new(Epoch::Epoch1, false, false, true)
    );
    assert_eq!(result.set_sideband.source_epoch, Epoch::Epoch0); // source_epoch is not used for epoch blocks
}

#[test]
fn adding_epoch_twice_fails() {
    let (keypair, previous, old_account_info) = create_epoch1_previous_block();
    let epoch = create_epoch_block(&previous);

    let result = validate(epoch, previous, old_account_info);

    assert_eq!(result, Err(ProcessResult::BlockPosition))
}

#[test]
fn adding_legacy_change_block_after_epoch1_fails() {
    let (keypair, previous, old_account_info) = create_epoch1_previous_block();

    let change = create_legacy_change_block(keypair, &previous);

    let result = validate(change, previous, old_account_info);
    assert_eq!(result, Err(ProcessResult::BlockPosition));
}

#[test]
fn can_add_state_blocks_after_epoch1() {
    let (keypair, previous, old_account_info) = create_epoch1_previous_block();

    let state = BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .link(0)
        .sign(&keypair)
        .build();

    validate(state, previous, old_account_info).expect("block should be valid");
}

fn validate(
    block: BlockEnum,
    previous: BlockEnum,
    old_account_info: AccountInfo,
) -> Result<BlockInsertInstructions, ProcessResult> {
    let validator = BlockValidator {
        block: &block,
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account: previous.account(),
        frontier_missing: false,
        old_account_info: Some(old_account_info),
        previous_block: Some(previous),
        pending_receive_info: None,
        any_pending_exists: false,
        source_block_exists: false,
        seconds_since_epoch: 123456,
    };

    validator.validate()
}

fn create_epoch_block(open: &BlockEnum) -> BlockEnum {
    BlockBuilder::state()
        .account(open.account())
        .balance(open.balance_calculated())
        .representative(open.representative().unwrap())
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
        .previous(open.hash())
        .sign(&DEV_GENESIS_KEY)
        .build()
}

fn create_epoch1_previous_block() -> (KeyPair, BlockEnum, AccountInfo) {
    let keypair = KeyPair::new();
    let open = BlockBuilder::state()
        .account(keypair.public_key())
        .sign(&keypair)
        .with_sideband()
        .build();

    let account_info = AccountInfo {
        epoch: Epoch::Epoch1,
        ..create_account_info(&open)
    };
    (keypair, open, account_info)
}

fn create_legacy_change_block(keypair: KeyPair, previous: &BlockEnum) -> BlockEnum {
    BlockBuilder::legacy_change()
        .account(keypair.public_key())
        .representative(Account::from(12345))
        .previous(previous.hash())
        .sign(&keypair)
        .build()
}
