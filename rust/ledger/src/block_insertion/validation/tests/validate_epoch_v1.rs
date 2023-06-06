use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, BlockBuilder, BlockDetails, BlockEnum,
    BlockSideband, Epoch, KeyPair, PendingInfo, StateBlockBuilder,
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
    let epoch = create_epoch_block(&open).build();

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
    let epoch = create_epoch_block(&previous).build();

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

#[test]
fn epoch_block_with_changed_representative_fails() {
    let open = BlockBuilder::legacy_open().with_sideband().build();
    let old_account_info = create_account_info(&open);
    let epoch_with_invalid_rep = create_epoch_block(&open)
        .representative(Account::from(999999))
        .build();

    let result = validate(epoch_with_invalid_rep, open, old_account_info);

    assert_eq!(result, Err(ProcessResult::RepresentativeMismatch));
}

#[test]
fn cannot_use_legacy_open_block_after_epoch1() {
    let (keypair, previous, old_account_info) = create_epoch1_previous_block();

    let legacy_open = BlockBuilder::legacy_open().build();

    let mut validator = create_validator(&legacy_open, legacy_open.account());
    validator.source_block_exists = true;
    validator.pending_receive_info = Some(PendingInfo {
        epoch: Epoch::Epoch1,
        ..PendingInfo::create_test_instance()
    });

    let result = validator.validate();
    assert_eq!(result, Err(ProcessResult::Unreceivable));
}

#[test]
fn cannot_use_legacy_receive_block_after_epoch1_open() {
    let (keypair, previous, old_account_info) = create_epoch1_previous_block();
    let legacy_receive = BlockBuilder::legacy_receive().build();
    let mut validator = create_validator(&legacy_receive, keypair.public_key());
    validator.old_account_info = Some(old_account_info);
    validator.previous_block = Some(previous);
    validator.source_block_exists = true;
    validator.pending_receive_info = Some(PendingInfo {
        epoch: Epoch::Epoch0,
        ..PendingInfo::create_test_instance()
    });

    let result = validator.validate();

    assert_eq!(result, Err(ProcessResult::BlockPosition));
}

#[test]
fn cannot_use_legacy_receive_block_after_sender_upgraded_to_epoch1() {
    let keypair = KeyPair::new();
    let previous = BlockBuilder::legacy_open().build();

    let legacy_receive = BlockBuilder::legacy_receive().sign(&keypair).build();

    let mut validator = create_validator(&legacy_receive, keypair.public_key());
    validator.old_account_info = Some(AccountInfo {
        epoch: Epoch::Epoch0,
        ..AccountInfo::create_test_instance()
    });
    validator.previous_block = Some(previous);
    validator.source_block_exists = true;
    validator.pending_receive_info = Some(PendingInfo {
        epoch: Epoch::Epoch1,
        ..PendingInfo::create_test_instance()
    });

    let result = validator.validate();

    assert_eq!(result, Err(ProcessResult::Unreceivable));
}

fn validate(
    block: BlockEnum,
    previous: BlockEnum,
    old_account_info: AccountInfo,
) -> Result<BlockInsertInstructions, ProcessResult> {
    let mut validator = create_validator(&block, previous.account());
    validator.previous_block = Some(previous);
    validator.old_account_info = Some(old_account_info);
    validator.validate()
}

fn create_validator<'a>(block: &'a BlockEnum, account: Account) -> BlockValidator {
    BlockValidator {
        block: block,
        epochs: &LEDGER_CONSTANTS_STUB.epochs,
        work: &WORK_THRESHOLDS_STUB,
        block_exists: false,
        account,
        frontier_missing: false,
        old_account_info: None,
        previous_block: None,
        pending_receive_info: None,
        any_pending_exists: false,
        source_block_exists: false,
        seconds_since_epoch: 123456,
    }
}

fn create_epoch_block(open: &BlockEnum) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(open.account())
        .balance(open.balance_calculated())
        .representative(open.representative().unwrap())
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
        .previous(open.hash())
        .sign(&DEV_GENESIS_KEY)
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
