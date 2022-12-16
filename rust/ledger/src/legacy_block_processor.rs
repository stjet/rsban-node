use rsnano_core::{
    utils::seconds_since_epoch, validate_message, Account, AccountInfo, Amount, Block,
    BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockSubType, BlockType, ChangeBlock, Epoch,
    OpenBlock, PendingInfo, PendingKey, PublicKey, ReceiveBlock, SendBlock,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

pub(crate) struct LegacyBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
    previous: Option<BlockHash>,
    representative: Option<Account>,
    destination: Option<Account>,
    block_type: BlockSubType,
}

impl<'a> LegacyBlockProcessor<'a> {
    pub(crate) fn open_block(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut OpenBlock,
    ) -> Self {
        Self {
            block_type: BlockSubType::Open,
            destination: None,
            previous: None,
            representative: Some(block.representative()),
            block,
            ledger,
            txn,
        }
    }

    pub(crate) fn receive_block(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut ReceiveBlock,
    ) -> Self {
        Self {
            block_type: BlockSubType::Receive,
            destination: None,
            previous: Some(block.previous()),
            representative: None,
            block,
            ledger,
            txn,
        }
    }

    pub(crate) fn send_block(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut SendBlock,
    ) -> Self {
        Self {
            block_type: BlockSubType::Send,
            destination: Some(block.hashables.destination),
            previous: Some(block.previous()),
            representative: None,
            block,
            ledger,
            txn,
        }
    }

    pub(crate) fn change_block(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut ChangeBlock,
    ) -> Self {
        Self {
            block_type: BlockSubType::Change,
            destination: None,
            previous: Some(block.previous()),
            representative: Some(block.representative()),
            block,
            ledger,
            txn,
        }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_previous_block(self.previous)?;
        let (account, account_info) = if let Some(prev) = &self.previous {
            let account = self.ensure_frontier(prev)?;
            let account_info = self.ensure_account_exists(&account)?;
            self.ensure_previous_block_is_account_head(prev, &account_info)?;
            (account, account_info)
        } else {
            let account = self.block.account();
            self.ensure_account_not_opened_yet(&account)?;
            (account, AccountInfo::default())
        };
        self.ensure_valid_signature(&account)?;
        let (amount_received, pending_key) = if let Some(source) = self.block.source() {
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
        self.ensure_no_negative_amount_spend(&account_info)?;

        if let Some(key) = &pending_key {
            self.ledger.store.pending().del(self.txn, key);
        }

        let amount_sent = if self.block.block_type() == BlockType::Send {
            account_info.balance - self.block.balance()
        } else {
            Amount::zero()
        };
        let new_balance = account_info.balance + amount_received - amount_sent;

        self.block.set_sideband(BlockSideband::new(
            account,
            BlockHash::zero(),
            new_balance,
            account_info.block_count + 1,
            seconds_since_epoch(),
            unused_block_details(),
            Epoch::Epoch0, /* unused */
        ));

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        let open_block = if account_info.head.is_zero() {
            self.block.hash()
        } else {
            account_info.open_block
        };
        let new_info = AccountInfo {
            head: self.block.hash(),
            representative: self.representative.unwrap_or(account_info.representative),
            open_block,
            balance: new_balance,
            modified: seconds_since_epoch(),
            block_count: account_info.block_count + 1,
            epoch: Epoch::Epoch0,
        };
        self.ledger
            .update_account(self.txn, &account, &account_info, &new_info);

        if !amount_received.is_zero() {
            self.ledger
                .cache
                .rep_weights
                .representation_add(new_info.representative, amount_received);
        } else if !amount_sent.is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                account_info.representative,
                Amount::zero().wrapping_sub(amount_sent),
            );
        } else {
            self.ledger.cache.rep_weights.representation_add_dual(
                new_info.representative,
                new_balance,
                account_info.representative,
                Amount::zero().wrapping_sub(new_balance),
            );
        }

        if let Some(destination) = self.destination {
            self.ledger.store.pending().put(
                self.txn,
                &PendingKey::new(destination, self.block.hash()),
                &PendingInfo::new(account, amount_sent, Epoch::Epoch0),
            );
        }

        if let Some(previous) = &self.previous {
            self.ledger.store.frontier().del(self.txn, previous);
        }

        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &account);

        self.ledger.observer.block_added(self.block_type);
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

    fn ensure_valid_previous_block(
        &self,
        previous: Option<BlockHash>,
    ) -> Result<(), ProcessResult> {
        if let Some(hash) = previous {
            let previous = self.ensure_previous_block_exists(&hash)?;
            self.ensure_valid_predecessor(&previous)?;
        }
        Ok(())
    }

    fn ensure_previous_block_exists(
        &self,
        previous: &BlockHash,
    ) -> Result<BlockEnum, ProcessResult> {
        self.ledger
            .get_block(self.txn.txn(), previous)
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
            .get_frontier(self.txn.txn(), &previous)
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_account_exists(&self, account: &Account) -> Result<AccountInfo, ProcessResult> {
        self.ledger
            .get_account_info(self.txn.txn(), account)
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
        if !self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &source)
        {
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
        match self.ledger.store.account().get(self.txn.txn(), account) {
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
            .get(self.txn.txn(), &pending_key)
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

fn unused_block_details() -> BlockDetails {
    BlockDetails::new(
        Epoch::Epoch0,
        false, /* unused */
        false, /* unused */
        false, /* unused */
    )
}
