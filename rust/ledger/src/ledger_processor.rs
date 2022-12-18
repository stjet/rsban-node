use crate::{
    legacy_block_validator::BlockValidation, BlockInserter, LegacyBlockValidator,
    StateBlockProcessor,
};
use rsnano_core::{
    Block, ChangeBlock, MutableBlockVisitor, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
};
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

    fn process_legacy(&mut self, block: &mut dyn Block) {
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
}

impl<'a> MutableBlockVisitor for LedgerProcessor<'a> {
    fn send_block(&mut self, block: &mut SendBlock) {
        self.process_legacy(block);
    }

    fn receive_block(&mut self, block: &mut ReceiveBlock) {
        self.process_legacy(block);
    }

    fn open_block(&mut self, block: &mut OpenBlock) {
        self.process_legacy(block);
    }

    fn change_block(&mut self, block: &mut ChangeBlock) {
        self.process_legacy(block);
    }

    fn state_block(&mut self, block: &mut StateBlock) {
        let validation = StateBlockProcessor::new(self.ledger, self.txn, block).process();
        self.apply(validation, block);
    }
}
