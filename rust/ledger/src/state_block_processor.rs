use rsnano_core::{
    utils::seconds_since_epoch, validate_block_signature, AccountInfo, Amount, Block, BlockDetails,
    BlockEnum, BlockHash, BlockSideband, BlockSubType, Epoch, Epochs, PendingInfo, PendingKey,
    StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

/// Processes a single state block
pub(crate) struct StateBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut StateBlock,
    old_account_info: Option<AccountInfo>,
    pending_receive: Option<PendingInfo>,
    previous_block: Option<BlockEnum>,
}

impl<'a> StateBlockProcessor<'a> {
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
            previous_block: None,
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

        if !self.block.previous().is_zero() {
            self.previous_block = self
                .ledger
                .get_block(self.txn.txn(), &self.block.previous());
        }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.initialize();

        // Epoch block pre-checks for early return
        self.ensure_block_signature_for_epoch_block_candidate_is_maybe_valid()?;
        self.ensure_previous_block_exists_for_epoch_block_candidate()?;

        // Common rules
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
        self.ensure_sufficient_work()?;

        // Epoch block rules
        self.ensure_epoch_block_does_not_change_representative()?;
        self.ensure_epoch_open_has_burn_account_as_rep()?;
        self.ensure_epoch_open_has_pending_entry()?;
        self.ensure_valid_epoch_for_unopened_account()?;
        self.ensure_epoch_upgrade_is_sequential_for_existing_account()?;
        self.ensure_epoch_block_does_not_change_balance()?;

        self.add_block();
        self.update_representative_cache();
        self.update_pending_store();
        self.update_account_info();
        self.delete_frontier();

        Ok(())
    }

    fn ensure_valid_block_signature(&self) -> Result<(), ProcessResult> {
        let result = if self.is_epoch_block() {
            self.ledger.validate_epoch_signature(self.block)
        } else {
            validate_block_signature(self.block)
        };
        result.map_err(|_| ProcessResult::BadSignature)
    }

    /// This check only makes sense after ensure_previous_block_exists_for_epoch_block_candidate,
    /// because we need the previous block for the balance change check!
    fn is_epoch_block(&self) -> bool {
        self.has_epoch_link() && !self.balance_changed()
    }

    fn balance_changed(&self) -> bool {
        self.previous_balance() != self.block.balance()
    }

    fn previous_balance(&self) -> Amount {
        self.previous_block
            .as_ref()
            .map(|b| b.balance_calculated())
            .unwrap_or_default()
    }

    fn has_epoch_link(&self) -> bool {
        self.ledger.is_epoch_link(&self.block.link())
    }

    fn block_epoch_version(&self) -> Epoch {
        self.ledger
            .constants
            .epochs
            .epoch(&self.block.link())
            .unwrap_or(Epoch::Invalid)
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
        // receives from the epoch account are forbidden
        if self.has_epoch_link() {
            return false;
        }

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
        if self.is_epoch_block() {
            self.block_epoch_version()
        } else {
            let epoch = self
                .old_account_info
                .as_ref()
                .map(|i| i.epoch)
                .unwrap_or(Epoch::Epoch0);

            std::cmp::max(epoch, self.source_epoch())
        }
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

    /// This is a precheck that allows for an early return if a block with an epoch link
    /// is not signed by the account owner or the epoch signer.
    /// It is not sure yet, if the block is an epoch block, because it could just be
    /// a send to the epoch account.
    fn ensure_block_signature_for_epoch_block_candidate_is_maybe_valid(
        &self,
    ) -> Result<(), ProcessResult> {
        // Check for possible regular state blocks with epoch link (send subtype)
        if self.has_epoch_link()
            && (validate_block_signature(self.block).is_err()
                && self.ledger.validate_epoch_signature(self.block).is_err())
        {
            return Err(ProcessResult::BadSignature);
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_exists_for_epoch_block_candidate(&self) -> Result<(), ProcessResult> {
        if self.has_epoch_link()
            && !self.block.previous().is_zero()
            && self.previous_block.is_none()
        {
            Err(ProcessResult::GapPrevious)
        } else {
            Ok(())
        }
    }

    fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.block.account().is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.account_exists() && self.previous_block.is_none() {
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

    fn ensure_epoch_block_does_not_change_representative(&self) -> Result<(), ProcessResult> {
        if self.is_epoch_block() {
            if let Some(info) = &self.old_account_info {
                if self.block.mandatory_representative() != info.representative {
                    return Err(ProcessResult::RepresentativeMismatch);
                };
            }
        }
        Ok(())
    }

    fn ensure_epoch_open_has_burn_account_as_rep(&self) -> Result<(), ProcessResult> {
        if self.is_epoch_block()
            && self.is_new_account()
            && !self.block.mandatory_representative().is_zero()
        {
            Err(ProcessResult::RepresentativeMismatch)
        } else {
            Ok(())
        }
    }

    fn ensure_epoch_open_has_pending_entry(&self) -> Result<(), ProcessResult> {
        if self.is_new_account() && self.is_epoch_block() {
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
        if self.is_new_account()
            && self.is_epoch_block()
            && self.block_epoch_version() == Epoch::Invalid
        {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_epoch_upgrade_is_sequential_for_existing_account(&self) -> Result<(), ProcessResult> {
        if self.is_epoch_block() {
            if let Some(info) = &self.old_account_info {
                if !Epochs::is_sequential(info.epoch, self.block_epoch_version()) {
                    return Err(ProcessResult::BlockPosition);
                }
            }
        }
        Ok(())
    }

    fn ensure_epoch_block_does_not_change_balance(&self) -> Result<(), ProcessResult> {
        if self.is_epoch_block() {
            if let Some(info) = &self.old_account_info {
                if self.block.balance() != info.balance {
                    return Err(ProcessResult::BalanceMismatch);
                };
            }
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
        if self.is_epoch_block() {
            self.ledger.observer.block_added(BlockSubType::Epoch);
        } else {
            self.ledger.observer.state_block_added();
        }

        self.block.set_sideband(self.create_sideband());
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
                self.block.mandatory_representative(),
                self.block.balance(),
            );
        } else {
            // Add in amount delta only
            self.ledger
                .cache
                .rep_weights
                .representation_add(self.block.mandatory_representative(), self.block.balance());
        }
    }

    fn create_account_info(&self) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: self.block.mandatory_representative(),
            open_block: self.open_block(),
            balance: self.block.balance(),
            modified: seconds_since_epoch(),
            block_count: self.new_block_count(),
            epoch: self.epoch(),
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

    fn block_details(&self) -> BlockDetails {
        BlockDetails::new(
            self.epoch(),
            self.is_send(),
            self.is_receive(),
            self.is_epoch_block(),
        )
    }

    fn get_old_account_info(&mut self) -> Option<AccountInfo> {
        self.ledger
            .get_account_info(self.txn.txn(), &self.block.account())
    }
}
