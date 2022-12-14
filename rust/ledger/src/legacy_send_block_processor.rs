use rsnano_core::{validate_message, Account, Block, SendBlock};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

/// Processes a single state block
pub(crate) struct LegacySendBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
}

impl<'a> LegacySendBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut dyn Block,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn process_legacy_send(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_previous_block()?;
        let account = self.ensure_frontier()?;
        self.ensure_valid_signature(account)?;
        Ok(())
    }

    fn ensure_valid_signature(
        &mut self,
        account: rsnano_core::PublicKey,
    ) -> Result<(), ProcessResult> {
        validate_message(
            &account.into(),
            self.block.hash().as_bytes(),
            self.block.block_signature(),
        )
        .map_err(|_| ProcessResult::BadSignature)?;
        Ok(())
    }

    fn ensure_frontier(&self) -> Result<Account, ProcessResult> {
        self.ledger
            .get_frontier(self.txn.txn(), &self.block.previous())
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.hash())
        {
            Err(ProcessResult::Old)
        } else {
            Ok(())
        }
    }

    fn ensure_valid_previous_block(&self) -> Result<(), ProcessResult> {
        let Some(previous) = self
            .ledger
            .get_block(self.txn.txn(), &self.block.previous()) else { return Err(ProcessResult::GapPrevious)};

        if !SendBlock::valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }
}
