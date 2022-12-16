use crate::{LegacyBlockInserter, LegacyBlockValidator, StateBlockProcessor};
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

    fn process(&mut self, block: &mut dyn Block) {
        let validation = LegacyBlockValidator::new(self.ledger, self.txn.txn(), block).validate();
        self.result = match validation {
            Ok(validation) => {
                let mut block_inserter =
                    LegacyBlockInserter::new(self.ledger, self.txn, block, &validation);
                block_inserter.insert();
                Ok(())
            }
            Err(x) => Err(x),
        };
    }
}

impl<'a> MutableBlockVisitor for LedgerProcessor<'a> {
    fn send_block(&mut self, block: &mut SendBlock) {
        self.process(block);
    }

    fn receive_block(&mut self, block: &mut ReceiveBlock) {
        self.process(block);
    }

    fn open_block(&mut self, block: &mut OpenBlock) {
        self.process(block);
    }

    fn change_block(&mut self, block: &mut ChangeBlock) {
        self.process(block);
    }

    fn state_block(&mut self, block: &mut StateBlock) {
        self.result = StateBlockProcessor::new(self.ledger, self.txn, block).process();
    }
}
