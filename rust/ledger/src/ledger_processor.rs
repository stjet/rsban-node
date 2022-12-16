use crate::{LegacyBlockProcessor, StateBlockProcessor};
use rsnano_core::{
    ChangeBlock, MutableBlockVisitor, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
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
}

impl<'a> MutableBlockVisitor for LedgerProcessor<'a> {
    fn send_block(&mut self, block: &mut SendBlock) {
        self.result = LegacyBlockProcessor::new(self.ledger, self.txn, block).process();
    }

    fn receive_block(&mut self, block: &mut ReceiveBlock) {
        self.result = LegacyBlockProcessor::new(self.ledger, self.txn, block).process();
    }

    fn open_block(&mut self, block: &mut OpenBlock) {
        self.result = LegacyBlockProcessor::new(self.ledger, self.txn, block).process();
    }

    fn change_block(&mut self, block: &mut ChangeBlock) {
        self.result = LegacyBlockProcessor::new(self.ledger, self.txn, block).process();
    }

    fn state_block(&mut self, block: &mut StateBlock) {
        self.result = StateBlockProcessor::new(self.ledger, self.txn, block).process();
    }
}
