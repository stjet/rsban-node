use rsnano_core::{
    utils::seconds_since_epoch, validate_block_signature, validate_message, AccountInfo, Amount,
    Block, BlockDetails, BlockHash, BlockSideband, BlockSubType, Epoch, Epochs, PendingInfo,
    PendingKey, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, ProcessResult};

pub(crate) struct StateBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut StateBlock,
}

impl<'a> StateBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut StateBlock,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
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
            self.epoch_block_impl()
        } else {
            StateBlockProcessorImpl::new(self.ledger, self.txn, self.block).process()
        }
    }

    fn epoch_block_impl(&mut self) -> Result<(), ProcessResult> {
        let hash = self.block.hash();
        let existing = self
            .ledger
            .block_or_pruned_exists_txn(self.txn.txn(), &hash);

        let mut result = ProcessResult::Progress;

        // Have we seen this block before? (Unambiguous)
        result = if existing {
            ProcessResult::Old
        } else {
            ProcessResult::Progress
        };
        if result == ProcessResult::Progress {
            // Is this block signed correctly (Unambiguous)
            result = match validate_message(
                &self
                    .ledger
                    .epoch_signer(&self.block.link())
                    .unwrap_or_default()
                    .into(),
                hash.as_bytes(),
                self.block.block_signature(),
            ) {
                Ok(_) => ProcessResult::Progress,
                Err(_) => ProcessResult::BadSignature,
            };
            if result == ProcessResult::Progress {
                debug_assert!(validate_message(
                    &self
                        .ledger
                        .epoch_signer(&self.block.link())
                        .unwrap_or_default()
                        .into(),
                    hash.as_bytes(),
                    self.block.block_signature()
                )
                .is_ok());
                // Is this for the burn account? (Unambiguous)
                result = if self.block.account().is_zero() {
                    ProcessResult::OpenedBurnAccount
                } else {
                    ProcessResult::Progress
                };
                if result == ProcessResult::Progress {
                    let mut info = AccountInfo::default();
                    let mut account_error = false;
                    match self
                        .ledger
                        .store
                        .account()
                        .get(self.txn.txn(), &self.block.account())
                    {
                        Some(i) => {
                            // Account already exists
                            info = i;
                            // Has this account already been opened? (Ambigious)
                            result = if self.block.previous().is_zero() {
                                ProcessResult::Fork
                            } else {
                                ProcessResult::Progress
                            };
                            if result == ProcessResult::Progress {
                                // Is the previous block the account's head block? (Ambigious)
                                result = if self.block.previous() == info.head {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::Fork
                                };
                                if result == ProcessResult::Progress {
                                    result = if self.block.representative() == info.representative {
                                        ProcessResult::Progress
                                    } else {
                                        ProcessResult::RepresentativeMismatch
                                    };
                                }
                            }
                        }
                        None => {
                            account_error = true;
                            result = if self.block.representative().is_zero() {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::RepresentativeMismatch
                            };
                            // Non-exisitng account should have pending entries
                            if result == ProcessResult::Progress {
                                let pending_exists = self
                                    .ledger
                                    .store
                                    .pending()
                                    .any(self.txn.txn(), &self.block.account());
                                result = if pending_exists {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::GapEpochOpenPending
                                };
                            }
                        }
                    }

                    if result == ProcessResult::Progress {
                        let epoch = self
                            .ledger
                            .constants
                            .epochs
                            .epoch(&self.block.link())
                            .unwrap_or(Epoch::Invalid);
                        // Must be an epoch for an unopened account or the epoch upgrade must be sequential
                        let is_valid_epoch_upgrade = if account_error {
                            epoch != Epoch::Invalid
                        } else {
                            Epochs::is_sequential(info.epoch, epoch)
                        };
                        result = if is_valid_epoch_upgrade {
                            ProcessResult::Progress
                        } else {
                            ProcessResult::BlockPosition
                        };
                        if result == ProcessResult::Progress {
                            result = if self.block.balance() == info.balance {
                                ProcessResult::Progress
                            } else {
                                ProcessResult::BalanceMismatch
                            };
                            if result == ProcessResult::Progress {
                                let block_details = BlockDetails::new(epoch, false, false, true);
                                // Does this block have sufficient work (Malformed)
                                result = if self.ledger.constants.work.difficulty_block(self.block)
                                    >= self
                                        .ledger
                                        .constants
                                        .work
                                        .threshold2(self.block.work_version(), &block_details)
                                {
                                    ProcessResult::Progress
                                } else {
                                    ProcessResult::InsufficientWork
                                };
                                if result == ProcessResult::Progress {
                                    self.ledger.observer.block_added(BlockSubType::Epoch);
                                    self.block.set_sideband(BlockSideband::new(
                                        self.block.account(), /* unused */
                                        BlockHash::zero(),
                                        Amount::zero(), /* unused */
                                        info.block_count + 1,
                                        seconds_since_epoch(),
                                        block_details,
                                        Epoch::Epoch0, /* unused */
                                    ));
                                    self.ledger.store.block().put(self.txn, &hash, self.block);
                                    let new_info = AccountInfo {
                                        head: hash,
                                        representative: self.block.representative(),
                                        open_block: if info.open_block.is_zero() {
                                            hash
                                        } else {
                                            info.open_block
                                        },
                                        balance: info.balance,
                                        modified: seconds_since_epoch(),
                                        block_count: info.block_count + 1,
                                        epoch,
                                    };
                                    self.ledger.update_account(
                                        self.txn,
                                        &self.block.account(),
                                        &info,
                                        &new_info,
                                    );
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
                        }
                    }
                }
            }
        }

        if result == ProcessResult::Progress {
            Ok(())
        } else {
            Err(result)
        }
    }
}

// Processes state blocks that don't have an epoch link
pub(crate) struct StateBlockProcessorImpl<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut StateBlock,
    old_account_info: Option<AccountInfo>,
    pending_receive: Option<PendingInfo>,
}

impl<'a> StateBlockProcessorImpl<'a> {
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

    pub(crate) fn process(&mut self) -> Result<(), ProcessResult> {
        self.initialize();
        self.ensure_valid_state_block()?;
        self.add_block();
        self.update_representative_cache();
        self.update_pending_store();
        self.update_account_info();
        self.delete_frontier();
        Ok(())
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
        self.ledger.observer.state_block_added();
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

    fn new_block_count(&self) -> u64 {
        self.old_account_info
            .as_ref()
            .map(|a| a.block_count)
            .unwrap_or_default()
            + 1
    }

    fn open_block(&self) -> BlockHash {
        if let Some(acc_info) = &self.old_account_info {
            acc_info.open_block
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
        BlockDetails::new(self.epoch(), self.is_send(), self.is_receive(), false)
    }

    fn get_old_account_info(&mut self) -> Option<AccountInfo> {
        self.ledger
            .get_account_info(self.txn.txn(), &self.block.account())
    }
}
