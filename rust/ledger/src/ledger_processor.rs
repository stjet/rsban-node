use std::sync::atomic::Ordering;

use crate::{BlockInserter, BlockValidation, LegacyBlockValidator, StateBlockValidator};
use rsnano_core::{Block, BlockEnum, StateBlock};
use rsnano_store_traits::WriteTransaction;

use super::{Ledger, ProcessResult};

pub(crate) struct LedgerProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    pub result: Result<(), ProcessResult>,
}

impl<'a> LedgerProcessor<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a mut dyn WriteTransaction) -> Self {
        Self {
            ledger,
            txn,
            result: Ok(()),
        }
    }

    fn process_legacy_block(&mut self, block: &mut dyn Block) {
        let validation = LegacyBlockValidator::new(self.ledger, self.txn.txn(), block).validate();
        self.apply(validation, block);
    }

    fn apply(&mut self, validation: Result<BlockValidation, ProcessResult>, block: &mut dyn Block) {
        self.result = match validation {
            Ok(validation) => {
                let mut block_inserter =
                    BlockInserter::new(self.ledger, self.txn, block, &validation);
                block_inserter.insert();
                Ok(())
            }
            Err(x) => Err(x),
        };
    }

    pub(crate) fn process(&mut self, block: &mut BlockEnum) {
        match block {
            BlockEnum::State(state_block) => self.process_state_block(state_block),
            _ => self.process_legacy_block(block.as_block_mut()),
        };
        if self.result.is_ok() {
            self.ledger.cache.block_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn process_state_block(&mut self, block: &mut StateBlock) {
        let validation = StateBlockValidator::new(self.ledger, self.txn.txn(), block).process();
        self.apply(validation, block);
    }
}
