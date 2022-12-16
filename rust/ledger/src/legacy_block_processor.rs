use rsnano_core::{
    utils::seconds_since_epoch, validate_message, Account, AccountInfo, Amount, Block,
    BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockType, Epoch, PendingInfo, PendingKey,
    PublicKey,
};
use rsnano_store_traits::{Transaction, WriteTransaction};

use crate::{Ledger, ProcessResult};

pub(crate) struct LegacyBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
}

pub(crate) struct BlockValidation {
    pub account: Account,
    pub old_account_info: AccountInfo,
    pub new_account_info: AccountInfo,
    pub amount_received: Amount,
    pub amount_sent: Amount,
    pub pending_received: Option<PendingKey>,
    pub new_pending: Option<(PendingKey, PendingInfo)>,
    pub new_sideband: BlockSideband,
}

pub(crate) struct LegacyBlockValidator<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
    block: &'a dyn Block,
}

impl<'a> LegacyBlockValidator<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a dyn Transaction, block: &'a dyn Block) -> Self {
        Self { ledger, txn, block }
    }

    fn validate(&mut self) -> Result<BlockValidation, ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_previous_block()?;
        let (account, old_account_info) = if self.block.block_type() == BlockType::Open {
            let account = self.block.account();
            self.ensure_account_not_opened_yet(&account)?;
            (account, AccountInfo::default())
        } else {
            let account = self.ensure_frontier(&self.block.previous())?;
            let account_info = self.ensure_account_exists(&account)?;
            self.ensure_previous_block_is_account_head(&self.block.previous(), &account_info)?;
            (account, account_info)
        };
        self.ensure_valid_signature(&account)?;
        let (amount_received, pending_received) = if let Some(source) = self.block.source() {
            self.ensure_source_block_exists(&source)?;
            let pending_key = PendingKey::new(account, source);
            let pending_info = self.ensure_source_not_received_yet(&pending_key)?;
            self.ensure_source_is_epoch_0(&pending_info)?;
            (pending_info.amount, Some(pending_key))
        } else {
            (Amount::zero(), None)
        };
        self.ensure_block_is_not_for_burn_account(&account)?;
        self.ensure_sufficient_work()?;
        self.ensure_no_negative_amount_spend(&old_account_info)?;

        let amount_sent = if self.block.block_type() == BlockType::Send {
            old_account_info.balance - self.block.balance()
        } else {
            Amount::zero()
        };

        let new_balance = old_account_info.balance + amount_received - amount_sent;

        let open_block = if old_account_info.head.is_zero() {
            self.block.hash()
        } else {
            old_account_info.open_block
        };

        let new_account_info = AccountInfo {
            head: self.block.hash(),
            representative: self
                .block
                .representative()
                .unwrap_or(old_account_info.representative),
            open_block,
            balance: new_balance,
            modified: seconds_since_epoch(),
            block_count: old_account_info.block_count + 1,
            epoch: Epoch::Epoch0,
        };

        let new_sideband = BlockSideband::new(
            account,
            BlockHash::zero(),
            new_balance,
            old_account_info.block_count + 1,
            seconds_since_epoch(),
            unused_block_details(),
            Epoch::Epoch0, /* unused */
        );

        let new_pending = if let Some(destination) = self.block.destination() {
            Some((
                PendingKey::new(destination, self.block.hash()),
                PendingInfo::new(account, amount_sent, Epoch::Epoch0),
            ))
        } else {
            None
        };

        Ok(BlockValidation {
            account,
            old_account_info,
            new_account_info,
            amount_received,
            amount_sent,
            pending_received,
            new_sideband,
            new_pending,
        })
    }

    fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn, &self.block.hash())
        {
            Err(ProcessResult::Old)
        } else {
            Ok(())
        }
    }

    fn ensure_valid_previous_block(&self) -> Result<(), ProcessResult> {
        if self.block.block_type() != BlockType::Open {
            let previous = self.ensure_previous_block_exists(&self.block.previous())?;
            self.ensure_valid_predecessor(&previous)?;
        }
        Ok(())
    }

    fn ensure_previous_block_exists(
        &self,
        previous: &BlockHash,
    ) -> Result<BlockEnum, ProcessResult> {
        self.ledger
            .get_block(self.txn, previous)
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_valid_predecessor(&self, previous: &BlockEnum) -> Result<(), ProcessResult> {
        if !self.block.valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_frontier(&self, previous: &BlockHash) -> Result<Account, ProcessResult> {
        self.ledger
            .get_frontier(self.txn, &previous)
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_account_exists(&self, account: &Account) -> Result<AccountInfo, ProcessResult> {
        self.ledger
            .get_account_info(self.txn, account)
            .ok_or(ProcessResult::GapPrevious)
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

    fn ensure_source_block_exists(&self, source: &BlockHash) -> Result<(), ProcessResult> {
        if !self.ledger.block_or_pruned_exists_txn(self.txn, &source) {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_is_account_head(
        &self,
        previous: &BlockHash,
        account_info: &AccountInfo,
    ) -> Result<(), ProcessResult> {
        // Block doesn't immediately follow latest block (Harmless)
        if account_info.head != *previous {
            Err(ProcessResult::GapPrevious)
        } else {
            Ok(())
        }
    }

    fn ensure_account_not_opened_yet(&self, account: &Account) -> Result<(), ProcessResult> {
        match self.ledger.store.account().get(self.txn, account) {
            Some(_) => Err(ProcessResult::Fork),
            None => Ok(()),
        }
    }

    fn ensure_source_not_received_yet(
        &self,
        pending_key: &PendingKey,
    ) -> Result<PendingInfo, ProcessResult> {
        self.ledger
            .store
            .pending()
            .get(self.txn, &pending_key)
            .ok_or(ProcessResult::Unreceivable)
    }

    fn ensure_block_is_not_for_burn_account(&self, account: &Account) -> Result<(), ProcessResult> {
        if account.is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    fn ensure_source_is_epoch_0(&self, pending_info: &PendingInfo) -> Result<(), ProcessResult> {
        if pending_info.epoch != Epoch::Epoch0 {
            Err(ProcessResult::Unreceivable)
        } else {
            Ok(())
        }
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

    fn ensure_no_negative_amount_spend(&self, info: &AccountInfo) -> Result<(), ProcessResult> {
        // Is this trying to spend a negative amount (Malicious)
        if self.block.block_type() == BlockType::Send && info.balance < self.block.balance() {
            return Err(ProcessResult::NegativeSpend);
        };

        Ok(())
    }
}

impl<'a> LegacyBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut dyn Block,
    ) -> Self {
        Self { block, ledger, txn }
    }

    pub(crate) fn process(&'a mut self) -> Result<(), ProcessResult> {
        let validation =
            LegacyBlockValidator::new(self.ledger, self.txn.txn(), self.block).validate()?;

        if let Some(key) = &validation.pending_received {
            self.ledger.store.pending().del(self.txn, key);
        }

        self.block.set_sideband(validation.new_sideband);

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        self.ledger.update_account(
            self.txn,
            &validation.account,
            &validation.old_account_info,
            &validation.new_account_info,
        );

        if !validation.amount_received.is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                validation.new_account_info.representative,
                validation.amount_received,
            );
        } else if !validation.amount_sent.is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                validation.old_account_info.representative,
                Amount::zero().wrapping_sub(validation.amount_sent),
            );
        } else {
            self.ledger.cache.rep_weights.representation_add_dual(
                validation.new_account_info.representative,
                validation.new_account_info.balance,
                validation.old_account_info.representative,
                Amount::zero().wrapping_sub(validation.new_account_info.balance),
            );
        }

        if let Some((key, info)) = validation.new_pending {
            self.ledger.store.pending().put(self.txn, &key, &info);
        }

        if self.block.block_type() != BlockType::Open {
            self.ledger
                .store
                .frontier()
                .del(self.txn, &self.block.previous());
        }

        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &validation.account);

        self.ledger.observer.block_added(self.block, false);
        Ok(())
    }
}

fn unused_block_details() -> BlockDetails {
    BlockDetails::new(Epoch::Epoch0, false, false, false)
}
