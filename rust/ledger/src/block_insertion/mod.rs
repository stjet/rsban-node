mod block_inserter;
mod common_rules;
mod epoch_block_rules;
mod helpers;
mod open_block_rules;
mod receive_block_rules;
mod rules;
mod send_block_rules;

pub(crate) use block_inserter::{BlockInsertInstructions, BlockInserter};

use rsnano_core::{
    work::WorkThresholds, Account, AccountInfo, BlockEnum, BlockHash, Epochs, PendingInfo,
    PendingKey,
};
use rsnano_store_traits::Transaction;

use crate::{Ledger, ProcessResult};

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
            old_account_info: self.ledger.get_account_info(self.txn, &account),
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
            .store
            .pending()
            .get(self.txn, &PendingKey::new(account, source))
    }

    fn any_pending_exists(&self, account: &Account) -> bool {
        self.ledger.store.pending().any(self.txn, account)
    }
}

/// Validates a single block before it gets inserted into the ledger
pub(crate) struct BlockValidator<'a> {
    block: &'a BlockEnum,
    epochs: &'a Epochs,
    work: &'a WorkThresholds,
    block_exists: bool,
    account: Account,
    frontier_missing: bool,
    previous_block: Option<BlockEnum>,
    old_account_info: Option<AccountInfo>,
    pending_receive_info: Option<PendingInfo>,
    any_pending_exists: bool,
    source_block_exists: bool,
}

impl<'a> BlockValidator<'a> {
    pub(crate) fn validate(&self) -> Result<BlockInsertInstructions, ProcessResult> {
        self.epoch_block_pre_checks()?;
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_predecessor()?;
        self.ensure_frontier_not_missing()?;
        self.ensure_valid_signature()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_account_exists_for_none_open_block()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_is_correct()?;
        self.ensure_open_block_has_link()?;
        self.ensure_no_reveive_balance_change_without_link()?;
        self.ensure_pending_receive_is_correct()?;
        self.ensure_sufficient_work()?;
        self.ensure_no_negative_amount_send()?;
        self.ensure_valid_epoch_block()?;

        Ok(self.create_instructions())
    }

    fn create_instructions(&self) -> BlockInsertInstructions {
        BlockInsertInstructions {
            account: self.account,
            old_account_info: self.old_account_info.clone().unwrap_or_default(),
            new_account_info: self.new_account_info(),
            pending_received: if self.pending_receive_info.is_some() {
                Some(PendingKey::new(self.account, self.block.source_or_link()))
            } else {
                None
            },
            new_pending: self.new_pending_info(),
            new_sideband: self.new_sideband(),
            is_epoch_block: self.is_epoch_block(),
        }
    }
}
