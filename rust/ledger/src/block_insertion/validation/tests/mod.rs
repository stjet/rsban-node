mod validate_epoch_v1;
mod validate_legacy_change;
mod validate_legacy_open;
mod validate_legacy_receive;
mod validate_legacy_send;
mod validate_state_change;
mod validate_state_open;
mod validate_state_receive;
mod validate_state_send;

use crate::{block_insertion::BlockInsertInstructions, ProcessResult};
use rsnano_core::{Account, AccountInfo, BlockEnum, Epoch, StateBlockBuilder};

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
        epoch: Epoch::Epoch0,
        representative: block.representative().unwrap_or(Account::from(2)),
        ..AccountInfo::create_test_instance()
    }
}
