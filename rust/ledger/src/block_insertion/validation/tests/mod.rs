mod validate_epoch_v1;
mod validate_epoch_v2;
mod validate_legacy_change;
mod validate_legacy_open;
mod validate_legacy_receive;
mod validate_legacy_send;
mod validate_state_change;
mod validate_state_open;
mod validate_state_receive;
mod validate_state_send;

use crate::{
    block_insertion::BlockInsertInstructions, ledger_constants::LEDGER_CONSTANTS_STUB,
    ProcessResult, DEV_GENESIS_KEY,
};
use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockBuilder, BlockDetails,
    BlockEnum, BlockSideband, Epoch, KeyPair, LegacyChangeBlockBuilder, LegacyReceiveBlockBuilder,
    PendingInfo, StateBlockBuilder,
};

use super::BlockValidator;

pub(crate) struct ValidateOutput {
    pub block: BlockEnum,
    pub result: Result<BlockInsertInstructions, ProcessResult>,
    pub old_account_info: AccountInfo,
    pub seconds_since_epoch: u64,
    pub account: Account,
}

pub(crate) struct ValidateStateBlockOptions<'a> {
    pub setup_block: Option<&'a dyn Fn(StateBlockBuilder) -> StateBlockBuilder>,
    pub setup_validator: Option<&'a mut dyn FnMut(&mut BlockValidator)>,
}

impl<'a> Default for ValidateStateBlockOptions<'a> {
    fn default() -> Self {
        Self {
            setup_block: None,
            setup_validator: None,
        }
    }
}

pub(crate) fn create_account_info(block: &BlockEnum) -> AccountInfo {
    AccountInfo {
        balance: block.balance_calculated(),
        head: block.hash(),
        epoch: block
            .sideband()
            .map(|sb| sb.details.epoch)
            .unwrap_or(Epoch::Epoch0),
        representative: block.representative().unwrap_or(Account::from(2)),
        ..AccountInfo::create_test_instance()
    }
}

pub(crate) fn create_test_validator<'a>(block: &'a BlockEnum, account: Account) -> BlockValidator {
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

pub(crate) fn assert_validation_fails_with(
    expected_error: ProcessResult,
    block: &BlockEnum,
    previous: Option<BlockEnum>,
) {
    let validator = match previous {
        Some(previous) => create_validator_for_existing_account(block, previous),
        None => create_test_validator(block, block.account()),
    };
    let result = validator.validate();
    assert_eq!(result, Err(expected_error))
}

pub(crate) fn assert_block_is_valid(
    block: &BlockEnum,
    previous: Option<BlockEnum>,
) -> BlockInsertInstructions {
    let validator = match previous {
        Some(previous) => create_validator_for_existing_account(block, previous),
        None => create_test_validator(block, block.account()),
    };
    validator.validate().expect("block should be valid!")
}

pub(crate) fn create_validator_for_existing_account(
    block: &BlockEnum,
    previous: BlockEnum,
) -> BlockValidator {
    let mut validator = create_test_validator(&block, previous.account_calculated());
    validator.old_account_info = Some(create_account_info(&previous));
    validator.previous_block = Some(previous);
    validator
}

pub(crate) fn setup_pending_receive(validator: &mut BlockValidator, epoch: Epoch, amount: Amount) {
    validator.source_block_exists = true;
    validator.pending_receive_info = Some(PendingInfo {
        epoch,
        amount,
        ..PendingInfo::create_test_instance()
    });
}

pub(crate) fn create_state_block(epoch: Epoch) -> (KeyPair, BlockEnum) {
    let keypair = KeyPair::new();
    let mut state = BlockBuilder::state()
        .account(keypair.public_key())
        .balance(1000)
        .sign(&keypair)
        .build();
    state.set_sideband(BlockSideband {
        account: keypair.public_key(),
        details: BlockDetails::new(epoch, false, true, false),
        ..BlockSideband::create_test_instance()
    });

    (keypair, state)
}

pub(crate) fn create_epoch1_open_block() -> BlockEnum {
    BlockBuilder::state()
        .previous(0)
        .balance(0)
        .representative(0)
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(Epoch::Epoch1).unwrap())
        .sign(&DEV_GENESIS_KEY)
        .build()
}

pub(crate) fn create_legacy_open_block() -> (KeyPair, BlockEnum) {
    let keypair = KeyPair::new();
    let mut open = BlockBuilder::legacy_open()
        .account(keypair.public_key())
        .sign(&keypair)
        .build();
    open.set_sideband(BlockSideband {
        details: BlockDetails::new(Epoch::Epoch0, false, true, false),
        ..BlockSideband::create_test_instance()
    });
    (keypair, open)
}

pub(crate) fn epoch_successor(previous: &BlockEnum, epoch: Epoch) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(previous.account_calculated())
        .balance(previous.balance_calculated())
        .representative(previous.representative().unwrap())
        .link(*LEDGER_CONSTANTS_STUB.epochs.link(epoch).unwrap())
        .previous(previous.hash())
        .sign(&DEV_GENESIS_KEY)
}

pub(crate) fn legacy_change_successor(
    keypair: KeyPair,
    previous: &BlockEnum,
) -> LegacyChangeBlockBuilder {
    BlockBuilder::legacy_change()
        .account(keypair.public_key())
        .representative(Account::from(12345))
        .previous(previous.hash())
        .sign(&keypair)
}

pub(crate) fn state_successor(keypair: KeyPair, previous: &BlockEnum) -> StateBlockBuilder {
    BlockBuilder::state()
        .account(keypair.public_key())
        .previous(previous.hash())
        .link(0)
        .sign(&keypair)
}

pub(crate) fn legacy_receive_successor(
    keypair: KeyPair,
    previous: &BlockEnum,
) -> LegacyReceiveBlockBuilder {
    BlockBuilder::legacy_receive()
        .previous(previous.hash())
        .sign(&keypair)
}
