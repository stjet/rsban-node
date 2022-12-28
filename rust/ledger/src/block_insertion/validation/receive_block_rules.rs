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
        if self.is_receive() && !self.source_block_exists {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    fn ensure_receive_block_receives_pending_amount(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            match &self.pending_receive_info {
                Some(pending) => {
                    if self.amount_received() != pending.amount {
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
