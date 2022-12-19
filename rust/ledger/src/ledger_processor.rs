use crate::{BlockInserter, BlockValidation, BlockValidator};
use rsnano_core::BlockEnum;
use rsnano_store_traits::WriteTransaction;

use super::{Ledger, ProcessResult};

pub(crate) struct LedgerProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
}

impl<'a> LedgerProcessor<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a mut dyn WriteTransaction) -> Self {
        Self { ledger, txn }
    }

    pub(crate) fn process(&mut self, block: &mut BlockEnum) -> Result<(), ProcessResult> {
        match block {
            BlockEnum::State(_) => self.process_state_block(block)?,
            _ => self.process_legacy_block(block)?,
        };
        Ok(())
    }

    fn process_state_block(&mut self, block: &mut BlockEnum) -> Result<(), ProcessResult> {
        let validation = BlockValidator::new(self.ledger, self.txn.txn(), block).validate();
        self.apply(validation, block)
    }

    fn process_legacy_block(&mut self, block: &mut BlockEnum) -> Result<(), ProcessResult> {
        let validation = BlockValidator::new(self.ledger, self.txn.txn(), block).validate();
        self.apply(validation, block)
    }

    fn apply(
        &mut self,
        validation: Result<BlockValidation, ProcessResult>,
        block: &mut BlockEnum,
    ) -> Result<(), ProcessResult> {
        match validation {
            Ok(validation) => {
                BlockInserter::new(self.ledger, self.txn, block, &validation).insert();
                Ok(())
            }
            Err(x) => Err(x),
        }
    }
}
