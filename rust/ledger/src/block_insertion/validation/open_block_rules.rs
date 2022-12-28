use super::BlockValidator;
use crate::ProcessResult;
use rsnano_core::{Block, BlockEnum};

impl<'a> BlockValidator<'a> {
    pub(crate) fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.account.is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_no_double_account_open(&self) -> Result<(), ProcessResult> {
        if self.account_exists() && self.block.is_open() {
            Err(ProcessResult::Fork)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_open_block_has_link(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.block {
            if self.block.is_open() && state.link().is_zero() {
                return Err(ProcessResult::GapSource);
            }
        }
        Ok(())
    }
}
