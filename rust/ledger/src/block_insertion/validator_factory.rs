use rsnano_core::{Account, BlockEnum, BlockHash, PendingInfo, PendingKey};
use rsnano_store_traits::Transaction;

use crate::Ledger;

use super::BlockValidator;

pub(crate) struct BlockValidatorFactory<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
    block: &'a BlockEnum,
}

impl<'a> BlockValidatorFactory<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a dyn Transaction, block: &'a BlockEnum) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn create_validator(&self) -> BlockValidator<'a> {
        let account = self.get_account();
        let frontier_missing = account.is_none();
        let account = account.unwrap_or_default();
        let previous_block = self.load_previous_block();
        let source_block = self.block.source_or_link();
        let source_block_exists = !source_block.is_zero()
            && self
                .ledger
                .block_or_pruned_exists_txn(self.txn, &source_block);

        let pending_receive_info = if source_block.is_zero() {
            None
        } else {
            self.load_pending_receive_info(account, source_block)
        };

        BlockValidator {
            block: self.block,
            epochs: &self.ledger.constants.epochs,
            work: &self.ledger.constants.work,
            account,
            frontier_missing,
            block_exists: self
                .ledger
                .block_or_pruned_exists_txn(self.txn, &self.block.hash()),
            old_account_info: self.ledger.account_info(self.txn, &account),
            pending_receive_info,
            any_pending_exists: self.any_pending_exists(&account),
            source_block_exists,
            previous_block,
        }
    }

    fn get_account(&self) -> Option<Account> {
        match self.block {
            BlockEnum::LegacyOpen(_) | BlockEnum::State(_) => Some(self.block.account()),
            _ => self.get_account_from_frontier_table(),
        }
    }

    fn get_account_from_frontier_table(&self) -> Option<Account> {
        self.ledger.get_frontier(self.txn, &self.block.previous())
    }

    fn load_previous_block(&self) -> Option<BlockEnum> {
        if !self.block.previous().is_zero() {
            self.ledger.get_block(self.txn, &self.block.previous())
        } else {
            None
        }
    }

    fn load_pending_receive_info(
        &self,
        account: Account,
        source: BlockHash,
    ) -> Option<PendingInfo> {
        self.ledger
            .pending_info(self.txn, &PendingKey::new(account, source))
    }

    fn any_pending_exists(&self, account: &Account) -> bool {
        self.ledger.store.pending().any(self.txn, account)
    }
}
