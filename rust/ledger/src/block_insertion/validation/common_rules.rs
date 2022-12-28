use super::BlockValidator;
use crate::ProcessResult;
use rsnano_core::validate_message;

impl<'a> BlockValidator<'a> {
    pub(crate) fn ensure_frontier_not_missing(&self) -> Result<(), ProcessResult> {
        if self.frontier_missing {
            Err(ProcessResult::Fork)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self.block_exists {
            Err(ProcessResult::Old)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_valid_signature(&self) -> Result<(), ProcessResult> {
        let result = if self.is_epoch_block() {
            self.epochs.validate_epoch_signature(self.block)
        } else {
            validate_message(
                &self.account,
                self.block.hash().as_bytes(),
                self.block.block_signature(),
            )
        };
        result.map_err(|_| ProcessResult::BadSignature)
    }

    pub(crate) fn ensure_account_exists_for_none_open_block(&self) -> Result<(), ProcessResult> {
        if !self.block.is_open() && self.is_new_account() {
            Err(ProcessResult::GapPrevious)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_previous_block_is_correct(&self) -> Result<(), ProcessResult> {
        self.ensure_previous_block_exists()?;
        self.ensure_previous_block_is_account_head()
    }

    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.account_exists() && self.previous_block.is_none() {
            return Err(ProcessResult::GapPrevious);
        }

        if self.is_new_account() && !self.block.previous().is_zero() {
            return Err(ProcessResult::GapPrevious);
        }

        Ok(())
    }

    fn ensure_previous_block_is_account_head(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if self.block.previous() != info.head {
                return Err(ProcessResult::Fork);
            }
        }

        Ok(())
    }

    pub(crate) fn ensure_sufficient_work(&self) -> Result<(), ProcessResult> {
        if !self.work.is_valid_pow(self.block, &self.block_details()) {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_valid_predecessor(&self) -> Result<(), ProcessResult> {
        if self.block.previous().is_zero() {
            return Ok(());
        }

        let previous = self
            .previous_block
            .as_ref()
            .ok_or(ProcessResult::GapPrevious)?;

        if !self.block.valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }
}
