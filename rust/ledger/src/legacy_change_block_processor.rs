use rsnano_core::{
    utils::seconds_since_epoch, validate_message, Account, AccountInfo, Amount, Block,
    BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockSubType, ChangeBlock, Epoch, PublicKey,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

pub(crate) struct LegacyChangeBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut ChangeBlock,
}

impl<'a> LegacyChangeBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut ChangeBlock,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        let previous = self.ensure_previous_block_exists()?;
        self.ensure_valid_predecessor(&previous)?;
        let account = self.ensure_frontier()?;
        let account_info = self.ensure_account_exists(&account)?;
        self.ensure_previous_block_is_account_head(&account_info)?;
        self.ensure_valid_signature(&account)?;
        self.ensure_sufficient_work()?;

        self.block.set_sideband(BlockSideband::new(
            account,
            BlockHash::zero(),
            account_info.balance,
            account_info.block_count + 1,
            seconds_since_epoch(),
            unused_block_details(),
            Epoch::Epoch0, /* unused */
        ));

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        let balance = self.ledger.balance(self.txn.txn(), &self.block.previous());
        self.ledger.cache.rep_weights.representation_add_dual(
            self.block.representative(),
            balance,
            account_info.representative,
            Amount::zero().wrapping_sub(balance),
        );
        let new_info = AccountInfo {
            head: self.block.hash(),
            representative: self.block.representative(),
            open_block: account_info.open_block,
            balance: account_info.balance,
            modified: seconds_since_epoch(),
            block_count: account_info.block_count + 1,
            epoch: Epoch::Epoch0,
        };
        self.ledger
            .update_account(self.txn, &account, &account_info, &new_info);
        self.ledger
            .store
            .frontier()
            .del(self.txn, &self.block.previous());
        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &account);
        self.ledger.observer.block_added(BlockSubType::Change);
        Ok(())
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

    fn ensure_previous_block_exists(&self) -> Result<BlockEnum, ProcessResult> {
        self.ledger
            .get_block(self.txn.txn(), &self.block.previous())
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_valid_predecessor(&self, previous: &BlockEnum) -> Result<(), ProcessResult> {
        if !self.block.valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_frontier(&self) -> Result<Account, ProcessResult> {
        self.ledger
            .get_frontier(self.txn.txn(), &self.block.previous())
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_account_exists(&self, account: &Account) -> Result<AccountInfo, ProcessResult> {
        self.ledger
            .get_account_info(self.txn.txn(), account)
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_previous_block_is_account_head(
        &self,
        account_info: &AccountInfo,
    ) -> Result<(), ProcessResult> {
        // Block doesn't immediately follow latest block (Harmless)
        if account_info.head != self.block.previous() {
            Err(ProcessResult::GapPrevious)
        } else {
            Ok(())
        }
    }

    fn ensure_valid_signature(&self, account: &PublicKey) -> Result<(), ProcessResult> {
        validate_message(
            account,
            self.block.hash().as_bytes(),
            self.block.block_signature(),
        )
        .map_err(|_| ProcessResult::BadSignature)?;
        Ok(())
    }

    fn ensure_sufficient_work(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .constants
            .work
            .is_valid_pow(self.block, &unused_block_details())
        {
            return Err(ProcessResult::InsufficientWork);
        };

        Ok(())
    }
}

fn unused_block_details() -> BlockDetails {
    BlockDetails::new(
        Epoch::Epoch0,
        false, /* unused */
        false, /* unused */
        false, /* unused */
    )
}
