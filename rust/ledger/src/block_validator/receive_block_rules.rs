use super::BlockValidator;
use crate::ProcessResult;
use rsnano_core::{BlockEnum, Epoch};

impl<'a> BlockValidator<'a> {
    pub fn ensure_pending_receive_is_correct(&self) -> Result<(), ProcessResult> {
        self.ensure_source_block_exists()?;
        self.ensure_receive_block_receives_pending_amount()?;
        self.ensure_legacy_source_is_epoch_0()
    }

    fn ensure_source_block_exists(&self) -> Result<(), ProcessResult> {
        let source = match self.block {
            BlockEnum::LegacyReceive(receive) => receive.mandatory_source(),
            BlockEnum::LegacyOpen(open) => open.mandatory_source(),
            BlockEnum::State(_) => {
                if self.is_receive() {
                    self.block.link().into()
                } else {
                    return Ok(());
                }
            }
            _ => return Ok(()),
        };

        if !self.ledger.block_or_pruned_exists_txn(self.txn, &source) {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    fn ensure_receive_block_receives_pending_amount(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            match &self.pending_receive_info {
                Some(pending) => {
                    if self.amount() != pending.amount {
                        return Err(ProcessResult::BalanceMismatch);
                    }
                }
                None => {
                    return Err(ProcessResult::Unreceivable);
                }
            };
        }

        Ok(())
    }

    fn ensure_legacy_source_is_epoch_0(&self) -> Result<(), ProcessResult> {
        let is_legacy_receive = match self.block {
            BlockEnum::LegacyReceive(_) | BlockEnum::LegacyOpen(_) => true,
            _ => false,
        };

        if is_legacy_receive
            && self
                .pending_receive_info
                .as_ref()
                .map(|x| x.epoch)
                .unwrap_or_default()
                != Epoch::Epoch0
        {
            Err(ProcessResult::Unreceivable)
        } else {
            Ok(())
        }
    }
}
