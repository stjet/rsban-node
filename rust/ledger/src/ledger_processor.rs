use crate::{
    LegacyChangeBlockProcessor, LegacyOpenBlockProcessor, LegacyReceiveBlockProcessor,
    LegacySendBlockProcessor, StateBlockProcessor,
};
use rsnano_core::{
    ChangeBlock, MutableBlockVisitor, OpenBlock, ReceiveBlock, SendBlock, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use super::{Ledger, ProcessResult};

pub(crate) struct LedgerProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    pub result: ProcessResult,
}

impl<'a> LedgerProcessor<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a mut dyn WriteTransaction) -> Self {
        Self {
            ledger,
            txn,
            result: ProcessResult::Progress,
        }
    }
}

impl<'a> MutableBlockVisitor for LedgerProcessor<'a> {
    fn send_block(&mut self, block: &mut SendBlock) {
        self.result = match LegacySendBlockProcessor::new(self.ledger, self.txn, block)
            .process_legacy_send()
        {
            Ok(()) => ProcessResult::Progress,
            Err(res) => res,
        };
    }

    fn receive_block(&mut self, block: &mut ReceiveBlock) {
        self.result = match LegacyReceiveBlockProcessor::new(self.ledger, self.txn, block).process()
        {
            Ok(()) => ProcessResult::Progress,
            Err(res) => res,
        };
    }

    fn open_block(&mut self, block: &mut OpenBlock) {
        self.result = match LegacyOpenBlockProcessor::new(self.ledger, self.txn, block).process() {
            Ok(()) => ProcessResult::Progress,
            Err(res) => res,
        };
    }

    fn change_block(&mut self, block: &mut ChangeBlock) {
        self.result = match LegacyChangeBlockProcessor::new(self.ledger, self.txn, block).process()
        {
            Ok(()) => ProcessResult::Progress,
            Err(res) => res,
        };
    }

    fn state_block(&mut self, block: &mut StateBlock) {
        self.result = match StateBlockProcessor::new(self.ledger, self.txn, block).process() {
            Ok(()) => ProcessResult::Progress,
            Err(res) => res,
        }
    }
}
