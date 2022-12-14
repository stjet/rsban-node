use rsnano_core::{
    utils::seconds_since_epoch, validate_block_signature, validate_message, AccountInfo, Amount,
    Block, BlockDetails, BlockHash, BlockSideband, BlockSubType, Epoch, Epochs, PendingInfo,
    PendingKey, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

/// Processes a single block
pub(crate) struct SingleBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
    old_account_info: Option<AccountInfo>,
    pending_receive: Option<PendingInfo>,
}

impl<'a> SingleBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut StateBlock,
    ) -> Self {
        Self {
            ledger,
            txn,
            block,
            old_account_info: None,
            pending_receive: None,
        }
    }

    fn initialize(&mut self) {
        self.old_account_info = self.get_old_account_info();

        if self.is_receive() {
            self.pending_receive = self
                .ledger
                .store
                .pending()
                .get(self.txn.txn(), &PendingKey::for_receive_block(self.block));
        }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.initialize();

        let is_epoch = if self.ledger.is_epoch_link(&self.block.link()) {
            if !self.block.previous().is_zero() {
                if self
                    .ledger
                    .store
                    .block()
                    .exists(self.txn.txn(), &self.block.previous())
                {
                    let previous_balance =
                        self.ledger.balance(self.txn.txn(), &self.block.previous());
                    self.block.balance() == previous_balance
                } else {
                    // Check for possible regular state blocks with epoch link (send subtype)
                    if validate_block_signature(self.block).is_err()
                        && self.ledger.validate_epoch_signature(self.block).is_err()
                    {
                        return Err(ProcessResult::BadSignature);
                    } else {
                        return Err(ProcessResult::GapPrevious);
                    }
                }
            } else {
                self.block.balance() == Amount::zero()
            }
        } else {
            false
        };

        if is_epoch {
            self.process_epoch_block()
        } else {
            self.process_state_block()
        }
    }

    fn ensure_valid_epoch_block_signature(&self) -> anyhow::Result<(), ProcessResult> {
        validate_message(
            &self
                .ledger
                .epoch_signer(&self.block.link())
                .unwrap_or_default()
                .into(),
            self.block.hash().as_bytes(),
            self.block.block_signature(),
        )
        .map_err(|_| ProcessResult::BadSignature)
    }

    fn process_epoch_block(&mut self) -> Result<(), ProcessResult> {
        self.ensure_valid_epoch_block()?;
        self.add_epoch_block();
        self.update_account_info_for_epoch_block();
        self.delete_frontier();
        Ok(())
    }

    fn ensure_valid_epoch_block(&mut self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_epoch_block_signature()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_is_account_head()?;
        self.ensure_representative_did_not_change()?;
        self.ensure_epoch_open_has_burn_account_as_rep()?;
        self.ensure_epoch_open_pending_entry()?;
        self.ensure_valid_epoch_for_unopened_account()?;
        self.ensure_epoch_upgrade_is_sequential_for_existing_account()?;
        self.ensure_epoch_block_does_not_change_balance()?;
        self.ensure_sufficient_work_for_epoch_block()?;
        Ok(())
    }

    fn epoch_block_details(&self) -> BlockDetails {
        let epoch = self.block_epoch_version();
        BlockDetails::new(epoch, false, false, true)
    }

    fn block_epoch_version(&self) -> Epoch {
        self.ledger
            .constants
            .epochs
            .epoch(&self.block.link())
            .unwrap_or(Epoch::Invalid)
    }

    pub(crate) fn process_state_block(&mut self) -> Result<(), ProcessResult> {
        self.initialize();
        self.ensure_valid_state_block()?;
        self.add_block();
        self.update_representative_cache();
        self.update_pending_store();
        self.update_account_info();
        self.delete_frontier();
        Ok(())
    }

    fn ensure_valid_state_block(&self) -> Result<(), ProcessResult> {
        self.ensure_block_does_not_exist_yet()?;
        self.ensure_valid_block_signature()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_exists()?;
        self.ensure_previous_block_is_account_head()?;
        self.ensure_new_account_has_link()?;
        self.ensure_no_receive_balance_change_without_link()?;
        self.ensure_receive_block_links_to_existing_block()?;
        self.ensure_receive_block_receives_pending_amount()?;
        self.ensure_sufficient_work()
    }

    fn account_exists(&self) -> bool {
        self.old_account_info.is_some()
    }

    fn is_new_account(&self) -> bool {
        self.old_account_info.is_none()
    }

    fn is_send(&self) -> bool {
        match &self.old_account_info {
            Some(info) => self.block.balance() < info.balance,
            None => false,
        }
    }

    fn is_receive(&self) -> bool {
        match &self.old_account_info {
            Some(info) => self.block.balance() >= info.balance && !self.block.link().is_zero(),
            None => true,
        }
    }

    fn amount(&self) -> Amount {
        match &self.old_account_info {
            Some(info) => {
                if self.is_send() {
                    info.balance - self.block.balance()
                } else {
                    self.block.balance() - info.balance
                }
            }
            None => self.block.balance(),
        }
    }

    fn epoch(&self) -> Epoch {
        let epoch = self
            .old_account_info
            .as_ref()
            .map(|i| i.epoch)
            .unwrap_or(Epoch::Epoch0);

        std::cmp::max(epoch, self.source_epoch())
    }

    fn source_epoch(&self) -> Epoch {
        self.pending_receive
            .as_ref()
            .map(|p| p.epoch)
            .unwrap_or(Epoch::Epoch0)
    }

    fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.hash())
        {
            return Err(ProcessResult::Old);
        }
        Ok(())
    }

    fn ensure_valid_block_signature(&self) -> Result<(), ProcessResult> {
        validate_block_signature(self.block).map_err(|_| ProcessResult::BadSignature)
    }

    fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.block.account().is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.account_exists()
            && !self
                .ledger
                .store
                .block()
                .exists(self.txn.txn(), &self.block.previous())
        {
            return Err(ProcessResult::GapPrevious);
        }

        if self.is_new_account() && !self.block.previous().is_zero() {
            return Err(ProcessResult::GapPrevious);
        }

        Ok(())
    }

    fn ensure_no_double_account_open(&self) -> Result<(), ProcessResult> {
        if self.account_exists() && self.block.previous().is_zero() {
            Err(ProcessResult::Fork)
        } else {
            Ok(())
        }
    }

    fn ensure_new_account_has_link(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() && self.block.link().is_zero() {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    /// Is the previous block the account's head block? (Ambigious)
    fn ensure_previous_block_is_account_head(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if self.block.previous() != info.head {
                return Err(ProcessResult::Fork);
            }
        }

        Ok(())
    }

    fn ensure_representative_did_not_change(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if self.block.representative() != info.representative {
                return Err(ProcessResult::RepresentativeMismatch);
            };
        }
        Ok(())
    }

    fn ensure_epoch_open_has_burn_account_as_rep(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() && !self.block.representative().is_zero() {
            Err(ProcessResult::RepresentativeMismatch)
        } else {
            Ok(())
        }
    }

    fn ensure_epoch_open_pending_entry(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() {
            // Non-exisitng account should have pending entries
            let pending_exists = self
                .ledger
                .store
                .pending()
                .any(self.txn.txn(), &self.block.account());
            if !pending_exists {
                return Err(ProcessResult::GapEpochOpenPending);
            };
        }
        Ok(())
    }

    fn ensure_valid_epoch_for_unopened_account(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() && self.block_epoch_version() == Epoch::Invalid {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_epoch_upgrade_is_sequential_for_existing_account(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if !Epochs::is_sequential(info.epoch, self.block_epoch_version()) {
                return Err(ProcessResult::BlockPosition);
            }
        }
        Ok(())
    }

    fn ensure_epoch_block_does_not_change_balance(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.old_account_info {
            if self.block.balance() != info.balance {
                return Err(ProcessResult::BalanceMismatch);
            };
        }
        Ok(())
    }

    fn ensure_link_block_exists(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &self.block.link().into())
        {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    /// If there's no link, the balance must remain the same, only the representative can change
    fn ensure_no_receive_balance_change_without_link(&self) -> Result<(), ProcessResult> {
        if !self.is_send() && self.block.link().is_zero() {
            if !self.amount().is_zero() {
                return Err(ProcessResult::BalanceMismatch);
            }
        }

        Ok(())
    }

    fn ensure_receive_block_links_to_existing_block(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            self.ensure_link_block_exists()?;
        }
        Ok(())
    }

    fn ensure_receive_block_receives_pending_amount(&self) -> Result<(), ProcessResult> {
        if self.is_receive() {
            match &self.pending_receive {
                Some(pending) => {
                    if self.amount() != pending.amount {
                        return Err(ProcessResult::BalanceMismatch);
                    }
                }
                None => {
                    return Err(ProcessResult::Unreceivable);
                }
            };
        }

        Ok(())
    }

    fn ensure_sufficient_work_for_epoch_block(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .constants
            .work
            .is_valid_pow(self.block, &&self.epoch_block_details())
        {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    fn ensure_sufficient_work(&self) -> Result<(), ProcessResult> {
        if !self
            .ledger
            .constants
            .work
            .is_valid_pow(self.block, &self.block_details())
        {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    fn add_block(&mut self) {
        self.ledger.observer.state_block_added();
        self.block.set_sideband(self.create_sideband());
        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);
    }

    fn add_epoch_block(&mut self) {
        self.ledger.observer.block_added(BlockSubType::Epoch);
        self.block
            .set_sideband(self.create_sideband_for_epoch_block());
        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);
    }

    fn update_pending_store(&mut self) {
        if self.is_send() {
            self.add_pending_receive();
        } else if self.is_receive() {
            self.delete_pending_receive();
        }
    }

    fn delete_frontier(&mut self) {
        if let Some(info) = &self.old_account_info {
            if self
                .ledger
                .store
                .frontier()
                .get(self.txn.txn(), &info.head)
                .is_some()
            {
                self.ledger.store.frontier().del(self.txn, &info.head);
            }
        }
    }

    fn update_account_info(&mut self) {
        let new_account_info = self.create_account_info();
        self.ledger.update_account(
            self.txn,
            &self.block.account(),
            &self.old_account_info.clone().unwrap_or_default(),
            &new_account_info,
        );
    }

    fn update_account_info_for_epoch_block(&mut self) {
        let new_account_info = self.create_account_info_from_epoch_block();
        self.ledger.update_account(
            self.txn,
            &self.block.account(),
            &self.old_account_info.clone().unwrap_or_default(),
            &new_account_info,
        );
    }

    fn add_pending_receive(&mut self) {
        let key = PendingKey::for_send_block(self.block);
        let info = PendingInfo::new(self.block.account(), self.amount(), self.epoch());
        self.ledger.store.pending().put(self.txn, &key, &info);
    }

    fn delete_pending_receive(&mut self) {
        self.ledger
            .store
            .pending()
            .del(self.txn, &PendingKey::for_receive_block(self.block));
    }

    fn update_representative_cache(&mut self) {
        if let Some(acc_info) = &self.old_account_info {
            // Move existing representation & add in amount delta
            self.ledger.cache.rep_weights.representation_add_dual(
                acc_info.representative,
                Amount::zero().wrapping_sub(acc_info.balance),
                self.block.representative(),
                self.block.balance(),
            );
        } else {
            // Add in amount delta only
            self.ledger
                .cache
                .rep_weights
                .representation_add(self.block.representative(), self.block.balance());
        }
    }

    fn create_account_info(&self) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: self.block.representative(),
            open_block: self.open_block(),
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: self.new_block_count(),
            epoch: self.epoch(),
        }
    }

    fn create_account_info_from_epoch_block(&self) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: self.block.representative(),
            open_block: self.open_block(),
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: self.new_block_count(),
            epoch: self.block_epoch_version(),
        }
    }

    fn new_block_count(&self) -> u64 {
        self.old_account_info
            .as_ref()
            .map(|a| a.block_count)
            .unwrap_or_default()
            + 1
    }

    fn open_block(&self) -> BlockHash {
        if let Some(info) = &self.old_account_info {
            info.open_block
        } else {
            self.block.hash()
        }
    }
    fn create_sideband(&self) -> BlockSideband {
        BlockSideband::new(
            self.block.account(), /* unused */
            BlockHash::zero(),
            Amount::zero(), /* unused */
            self.new_block_count(),
            seconds_since_epoch(),
            self.block_details(),
            self.source_epoch(),
        )
    }

    fn create_sideband_for_epoch_block(&self) -> BlockSideband {
        BlockSideband::new(
            self.block.account(), /* unused */
            BlockHash::zero(),
            Amount::zero(), /* unused */
            self.new_block_count(),
            seconds_since_epoch(),
            self.epoch_block_details(),
            self.source_epoch(),
        )
    }

    fn block_details(&self) -> BlockDetails {
        BlockDetails::new(self.epoch(), self.is_send(), self.is_receive(), false)
    }

    fn get_old_account_info(&mut self) -> Option<AccountInfo> {
        self.ledger
            .get_account_info(self.txn.txn(), &self.block.account())
    }
}
