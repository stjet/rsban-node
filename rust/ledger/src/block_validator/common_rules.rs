use super::BlockValidator;
use crate::ProcessResult;
use rsnano_core::{validate_message, Account, Block, BlockEnum, BlockHash};

impl<'a> BlockValidator<'a> {
    pub(crate) fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn, &self.block.hash())
        {
            return Err(ProcessResult::Old);
        }
        Ok(())
    }

    pub(crate) fn ensure_valid_signature(&self) -> Result<(), ProcessResult> {
        let result = if self.is_epoch_block() {
            self.ledger.validate_epoch_signature(self.block)
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
        if !self.block.is_open() && self.old_account_info.is_none() {
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
        if !self
            .ledger
            .constants
            .work
            .is_valid_pow(self.block, &self.block_details())
        {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_account(&self) -> Result<Account, ProcessResult> {
        let account = match self.block {
            BlockEnum::LegacyOpen(open) => open.account(),
            BlockEnum::State(state) => state.account(),
            BlockEnum::LegacySend(_) | BlockEnum::LegacyReceive(_) | BlockEnum::LegacyChange(_) => {
                self.get_account_from_frontier_table()?
            }
        };
        Ok(account)
    }

    fn get_account_from_frontier_table(&self) -> Result<rsnano_core::PublicKey, ProcessResult> {
        let previous = self
            .ledger
            .get_block(self.txn, &self.block.previous())
            .ok_or(ProcessResult::GapPrevious)?;
        self.ensure_valid_predecessor(&previous)?;
        Ok(self.ensure_frontier(&self.block.previous())?)
    }

    fn ensure_frontier(&self, previous: &BlockHash) -> Result<Account, ProcessResult> {
        self.ledger
            .get_frontier(self.txn, &previous)
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_valid_predecessor(&self, previous: &BlockEnum) -> Result<(), ProcessResult> {
        if !self.block.valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }
}
