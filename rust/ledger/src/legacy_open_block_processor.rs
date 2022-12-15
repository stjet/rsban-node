use rsnano_core::{
    utils::seconds_since_epoch, validate_message, AccountInfo, Block, BlockDetails, BlockHash,
    BlockSideband, BlockSubType, Epoch, OpenBlock, PendingInfo, PendingKey,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

pub(crate) struct LegacyOpenBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut OpenBlock,
}

impl<'a> LegacyOpenBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut OpenBlock,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_signature()?;
        self.ensure_source_block_exists()?;
        self.ensure_account_not_opened_yet()?;
        let pending_info = self.ensure_source_not_received_yet()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_source_is_epoch_0(&pending_info)?;
        self.ensure_sufficient_work()?;

        let key = PendingKey::new(self.block.account(), self.block.source());
        self.ledger.store.pending().del(self.txn, &key);
        self.block.set_sideband(BlockSideband::new(
            self.block.account(),
            BlockHash::zero(),
            pending_info.amount,
            1,
            seconds_since_epoch(),
            open_block_details(),
            Epoch::Epoch0, /* unused */
        ));
        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);
        let new_info = AccountInfo {
            head: self.block.hash(),
            representative: self.block.representative(),
            open_block: self.block.hash(),
            balance: pending_info.amount,
            modified: seconds_since_epoch(),
            block_count: 1,
            epoch: Epoch::Epoch0,
        };
        self.ledger.update_account(
            self.txn,
            &self.block.account(),
            &AccountInfo::default(),
            &new_info,
        );
        self.ledger
            .cache
            .rep_weights
            .representation_add(self.block.representative(), pending_info.amount);
        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &self.block.account());
        self.ledger.observer.block_added(BlockSubType::Open);
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

    fn ensure_valid_signature(&self) -> Result<(), ProcessResult> {
        validate_message(
            &self.block.account(),
            self.block.hash().as_bytes(),
            self.block.block_signature(),
        )
        .map_err(|_| ProcessResult::BadSignature)?;
        Ok(())
    }

    fn ensure_source_block_exists(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.source())
        {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    fn ensure_account_not_opened_yet(&self) -> Result<(), ProcessResult> {
        match self
            .ledger
            .store
            .account()
            .get(self.txn.txn(), &self.block.account())
        {
            Some(_) => Err(ProcessResult::Fork),
            None => Ok(()),
        }
    }

    fn ensure_source_not_received_yet(&self) -> Result<PendingInfo, ProcessResult> {
        let key = PendingKey::new(self.block.account(), self.block.source());
        self.ledger
            .store
            .pending()
            .get(self.txn.txn(), &key)
            .ok_or(ProcessResult::Unreceivable)
    }

    fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.block.account().is_zero() {
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
            .is_valid_pow(self.block, &open_block_details())
        {
            return Err(ProcessResult::InsufficientWork);
        };

        Ok(())
    }
}

fn open_block_details() -> BlockDetails {
    BlockDetails::new(
        Epoch::Epoch0,
        false, /* unused */
        false, /* unused */
        false, /* unused */
    )
}
