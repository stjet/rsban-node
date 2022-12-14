use rsnano_core::{
    utils::seconds_since_epoch, validate_message, Account, AccountInfo, Amount, Block,
    BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockSubType, Epoch, PendingInfo,
    PendingKey, SendBlock,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

/// Processes a single state block
pub(crate) struct LegacySendBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut SendBlock,
}

impl<'a> LegacySendBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut SendBlock,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn process_legacy_send(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        let previous = self.ensure_previous_block_exists()?;
        self.ensure_valid_predecessor(&previous)?;
        let account = self.ensure_frontier()?;
        self.ensure_valid_signature(account)?;
        self.ensure_sufficient_work()?;
        let account_info = self.ensure_account_exists(&account)?;
        debug_assert!(account_info.head == self.block.previous());
        self.ensure_no_negative_amount(&account_info)?;
        let amount = account_info.balance - self.block.balance();
        self.update_representative_cache(&account_info, amount);

        self.block
            .set_sideband(self.create_sideband(account, &account_info));

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        let new_info = self.new_account_info(&account_info);
        self.ledger
            .update_account(self.txn, &account, &account_info, &new_info);

        self.ledger.store.pending().put(
            self.txn,
            &PendingKey::new(self.block.hashables.destination, self.block.hash()),
            &PendingInfo::new(account, amount, Epoch::Epoch0),
        );

        self.ledger
            .store
            .frontier()
            .del(self.txn, &self.block.previous());

        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &account);

        self.ledger.observer.block_added(BlockSubType::Send);
        Ok(())
    }

    fn new_account_info(&self, account_info: &AccountInfo) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: account_info.representative,
            open_block: account_info.open_block,
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: account_info.block_count + 1,
            epoch: Epoch::Epoch0,
        }
    }

    fn create_sideband(&self, account: Account, account_info: &AccountInfo) -> BlockSideband {
        BlockSideband::new(
            account,
            BlockHash::zero(),
            self.block.balance(), /* unused */
            account_info.block_count + 1,
            seconds_since_epoch(),
            send_block_details(),
            Epoch::Epoch0, /* unused */
        )
    }

    fn update_representative_cache(&self, info: &AccountInfo, amount: Amount) {
        self.ledger
            .cache
            .rep_weights
            .representation_add(info.representative, Amount::zero().wrapping_sub(amount));
    }

    fn ensure_no_negative_amount(&self, info: &AccountInfo) -> Result<(), ProcessResult> {
        // Is this trying to spend a negative amount (Malicious)
        if info.balance < self.block.balance() {
            return Err(ProcessResult::NegativeSpend);
        };

        Ok(())
    }

    fn ensure_account_exists(&self, account: &Account) -> Result<AccountInfo, ProcessResult> {
        self.ledger
            .get_account_info(self.txn.txn(), account)
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_sufficient_work(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .constants
            .work
            .is_valid_pow(self.block, &send_block_details())
        {
            return Err(ProcessResult::InsufficientWork);
        };

        Ok(())
    }

    fn ensure_valid_signature(&self, account: rsnano_core::PublicKey) -> Result<(), ProcessResult> {
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

    fn ensure_previous_block_exists(&self) -> Result<BlockEnum, ProcessResult> {
        self.ledger
            .get_block(self.txn.txn(), &self.block.previous())
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_valid_predecessor(&self, previous: &BlockEnum) -> Result<(), ProcessResult> {
        if !SendBlock::valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }
}

fn send_block_details() -> BlockDetails {
    BlockDetails::new(
        Epoch::Epoch0,
        false, /* unused */
        false, /* unused */
        false, /* unused */
    )
}
