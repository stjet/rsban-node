mod common_rules;
mod epoch_block_rules;
mod helpers;
mod open_block_rules;
mod receive_block_rules;
mod send_block_rules;

use rsnano_core::{Account, AccountInfo, BlockEnum, PendingInfo, PendingKey};
use rsnano_store_traits::Transaction;

use crate::{BlockInsertInstructions, Ledger, ProcessResult};

/// Validates a single block before it gets inserted into the ledger
pub(crate) struct BlockValidator<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
    account: Account,
    previous_block: Option<BlockEnum>,
    old_account_info: Option<AccountInfo>,
    pending_receive_key: Option<PendingKey>,
    pending_receive_info: Option<PendingInfo>,
    block: &'a BlockEnum,
}

impl<'a> BlockValidator<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a dyn Transaction, block: &'a BlockEnum) -> Self {
        Self {
            ledger,
            txn,
            account: Default::default(),
            previous_block: None,
            old_account_info: None,
            pending_receive_key: None,
            pending_receive_info: None,
            block,
        }
    }

    pub(crate) fn validate(&mut self) -> Result<BlockInsertInstructions, ProcessResult> {
        self.epoch_block_pre_checks()?;
        self.ensure_block_does_not_exist_yet()?;

        self.load_related_block_data()?;

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
            pending_received: self.pending_receive_key.clone(),
            new_pending: self.new_pending_info(),
            new_sideband: self.new_sideband(),
            is_epoch_block: self.is_epoch_block(),
        }
    }
}
