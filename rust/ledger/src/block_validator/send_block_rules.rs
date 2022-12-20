use super::BlockValidator;
use crate::ProcessResult;
use rsnano_core::{Block, BlockEnum, BlockType};

impl<'a> BlockValidator<'a> {
    /// If there's no link, the balance must remain the same, only the representative can change
    pub(crate) fn ensure_no_reveive_balance_change_without_link(
        &self,
    ) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.block {
            if state.link().is_zero() && !self.amount_received().is_zero() {
                return Err(ProcessResult::BalanceMismatch);
            }
        }

        Ok(())
    }

    pub(crate) fn ensure_no_negative_amount_send(&self) -> Result<(), ProcessResult> {
        // Is this trying to spend a negative amount (Malicious)
        if self.block.block_type() == BlockType::LegacySend
            && self.previous_balance() < self.block.balance()
        {
            return Err(ProcessResult::NegativeSpend);
        };

        Ok(())
    }
}
