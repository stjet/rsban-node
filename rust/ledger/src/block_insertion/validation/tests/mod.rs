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
    test_helpers::create_test_account_info, ProcessResult,
};
use rsnano_core::{
    work::WORK_THRESHOLDS_STUB, Account, AccountInfo, Amount, BlockEnum, Epoch, PendingInfo,
    StateBlockBuilder, TestAccountChain,
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
    previous: &BlockEnum,
) {
    let validator = create_validator_for_existing_account(block, previous);
    let result = validator.validate();
    assert_eq!(result, Err(expected_error))
}

pub(crate) fn assert_block_is_valid(
    block: &BlockEnum,
    previous: &BlockEnum,
) -> BlockInsertInstructions {
    let validator = create_validator_for_existing_account(block, previous);
    validator.validate().expect("block should be valid!")
}

pub(crate) fn create_validator_for_existing_account<'a>(
    block: &'a BlockEnum,
    previous: &'a BlockEnum,
) -> BlockValidator<'a> {
    let mut validator = create_test_validator(&block, previous.account_calculated());
    validator.old_account_info = Some(create_test_account_info(previous));
    validator.previous_block = Some(previous.clone());
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

pub(crate) struct BlockValidationTest {
    pub seconds_since_epoch: u64,
    chain: TestAccountChain,
    block: Option<BlockEnum>,
    pending_receive: Option<PendingInfo>,
}
impl BlockValidationTest {
    pub fn for_epoch0_account() -> Self {
        let mut result = Self::for_unopened_account();
        result.chain.add_random_open_block();
        result
    }

    pub fn for_epoch1_account() -> Self {
        let mut result = Self::for_unopened_account();
        result.chain.add_random_open_block();
        result.setup_account(|chain| {
            chain.add_epoch_v1();
        })
    }

    pub fn for_unopened_account() -> Self {
        Self {
            chain: TestAccountChain::new(),
            block: None,
            pending_receive: None,
            seconds_since_epoch: 123456
        }
    }

    pub fn setup_account(mut self, mut setup: impl FnMut(&mut TestAccountChain)) -> Self {
        setup(&mut self.chain);
        self
    }

    pub fn block_to_validate(
        mut self,
        create_block: impl FnOnce(&TestAccountChain) -> BlockEnum,
    ) -> Self {
        self.block = Some(create_block(&self.chain));
        self
    }

    pub fn with_pending_receive(mut self, amount: Amount, source_epoch: Epoch) -> Self {
        self.pending_receive = Some(PendingInfo {
            source: Account::from(42),
            amount,
            epoch: source_epoch,
        });
        self
    }

    pub fn block(&self) -> &BlockEnum{
        self.block.as_ref().unwrap()
    }

    pub fn assert_validation_fails_with(&self, expected: ProcessResult) {
        assert_eq!(self.validate(), Err(expected));
    }

    pub fn assert_is_valid(&self) -> BlockInsertInstructions {
        self.validate().expect("block should be valid!")
    }

    fn validate(&self) -> Result<BlockInsertInstructions, ProcessResult> {
        let block = self.block.as_ref().unwrap();
        let mut validator = if self.chain.height() == 0 {
            create_test_validator(block, self.chain.account())
        } else {
            create_validator_for_existing_account(block, self.chain.latest_block())
        };
        validator.seconds_since_epoch = self.seconds_since_epoch;
        if self.pending_receive.is_some() {
            validator.any_pending_exists = true;
            validator.source_block_exists = true;
            validator.pending_receive_info = self.pending_receive.clone();
        }
        validator.validate()
    }
}
