use rsnano_core::{
    utils::seconds_since_epoch, validate_block_signature, validate_message, Account, AccountInfo,
    Amount, Block, BlockDetails, BlockEnum, BlockHash, BlockSideband, BlockType, Epoch, Epochs,
    PendingInfo, PendingKey, StateBlock,
};
use rsnano_store_traits::Transaction;

use crate::{Ledger, ProcessResult};

pub(crate) struct BlockValidation {
    pub account: Account,
    pub old_account_info: AccountInfo,
    pub new_account_info: AccountInfo,
    pub pending_received: Option<PendingKey>,
    pub new_pending: Option<(PendingKey, PendingInfo)>,
    pub new_sideband: BlockSideband,
    pub is_epoch_block: bool,
}

struct BlockValidationHelper<'a> {
    account: Account,
    previous_block: Option<BlockEnum>,
    old_account_info: Option<AccountInfo>,
    pending_receive_key: Option<PendingKey>,
    pending_receive_info: Option<PendingInfo>,
    block: &'a BlockEnum,
    epochs: &'a Epochs,
}

impl<'a> BlockValidationHelper<'a> {
    pub fn new(block: &'a BlockEnum, epochs: &'a Epochs) -> Self {
        Self {
            account: Default::default(),
            previous_block: None,
            old_account_info: None,
            pending_receive_key: None,
            pending_receive_info: None,
            block,
            epochs,
        }
    }
}

impl<'a> BlockValidationHelper<'a> {
    fn previous_balance(&self) -> Amount {
        self.previous_block
            .as_ref()
            .map(|b| b.balance_calculated())
            .unwrap_or_default()
    }

    fn is_send(&self) -> bool {
        match self.block {
            BlockEnum::LegacySend(_) => true,
            BlockEnum::State(state) => match &self.old_account_info {
                Some(info) => state.balance() < info.balance,
                None => false,
            },
            _ => false,
        }
    }

    fn is_receive(&self) -> bool {
        match self.block {
            BlockEnum::LegacyReceive(_) => true,
            BlockEnum::State(state_block) => {
                // receives from the epoch account are forbidden
                if self.has_epoch_link(state_block) {
                    return false;
                }

                match &self.old_account_info {
                    Some(info) => {
                        self.block.balance() >= info.balance && !state_block.link().is_zero()
                    }
                    None => true,
                }
            }
            _ => false,
        }
    }

    fn source_epoch(&self) -> Epoch {
        self.pending_receive_info
            .as_ref()
            .map(|p| p.epoch)
            .unwrap_or(Epoch::Epoch0)
    }

    fn amount_received(&self) -> Amount {
        match &self.block {
            BlockEnum::LegacyReceive(_) | BlockEnum::LegacyOpen(_) => self
                .pending_receive_info
                .as_ref()
                .map(|i| i.amount)
                .unwrap_or_default(),
            BlockEnum::State(state) => {
                let previous = self.previous_balance();
                if previous < state.balance() {
                    state.balance() - previous
                } else {
                    Amount::zero()
                }
            }
            _ => Amount::zero(),
        }
    }

    fn amount_sent(&self) -> Amount {
        if let Some(info) = &self.old_account_info {
            match self.block {
                BlockEnum::LegacySend(_) | BlockEnum::State(_) => {
                    if self.block.balance() < info.balance {
                        return info.balance - self.block.balance();
                    }
                }
                _ => {}
            }
        }
        Amount::zero()
    }

    fn new_balance(&self) -> Amount {
        self.old_account_info
            .as_ref()
            .map(|i| i.balance)
            .unwrap_or_default()
            + self.amount_received()
            - self.amount_sent()
    }

    fn has_epoch_link(&self, state_block: &StateBlock) -> bool {
        self.epochs.is_epoch_link(&state_block.link())
    }

    /// This check only makes sense after ensure_previous_block_exists_for_epoch_block_candidate,
    /// because we need the previous block for the balance change check!
    fn is_epoch_block(&self) -> bool {
        match self.block {
            BlockEnum::State(state_block) => {
                self.has_epoch_link(state_block) && self.previous_balance() == self.block.balance()
            }
            _ => false,
        }
    }

    fn block_epoch_version(&self) -> Epoch {
        match self.block {
            BlockEnum::State(state) => self.epochs.epoch(&state.link()).unwrap_or(Epoch::Invalid),
            _ => Epoch::Epoch0,
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

    fn open_block(&self) -> BlockHash {
        let open_block = match &self.old_account_info {
            Some(info) => info.open_block,
            None => self.block.hash(),
        };
        open_block
    }

    fn new_representative(&self) -> rsnano_core::PublicKey {
        self.block.representative().unwrap_or(
            self.old_account_info
                .as_ref()
                .map(|x| x.representative)
                .unwrap_or_default(),
        )
    }

    fn amount(&self) -> Amount {
        let old_balance = self
            .old_account_info
            .as_ref()
            .map(|x| x.balance)
            .unwrap_or_default();

        let new_balance = self.new_balance();

        if old_balance > new_balance {
            old_balance - new_balance
        } else {
            new_balance - old_balance
        }
    }
    fn new_pending_info(&self) -> Option<(PendingKey, PendingInfo)> {
        match self.block {
            BlockEnum::State(state) => {
                if self.is_send() {
                    let key = PendingKey::for_send_state_block(state);
                    let info = PendingInfo::new(self.account, self.amount(), self.epoch());
                    Some((key, info))
                } else {
                    None
                }
            }
            BlockEnum::LegacySend(send) => {
                let amount_sent = self.amount_sent();
                Some((
                    PendingKey::new(send.hashables.destination, send.hash()),
                    PendingInfo::new(self.account, amount_sent, Epoch::Epoch0),
                ))
            }
            _ => None,
        }
    }

    fn get_pending_receive_key(&self) -> Option<PendingKey> {
        match &self.block {
            BlockEnum::State(state) => {
                if self.is_receive() {
                    Some(PendingKey::for_receive_state_block(state))
                } else {
                    None
                }
            }
            BlockEnum::LegacyOpen(open) => {
                Some(PendingKey::new(self.account, open.mandatory_source()))
            }
            BlockEnum::LegacyReceive(open) => {
                Some(PendingKey::new(self.account, open.mandatory_source()))
            }
            _ => None,
        }
    }

    fn new_sideband(&self) -> BlockSideband {
        BlockSideband::new(
            self.account,
            BlockHash::zero(),
            self.new_balance(),
            self.new_block_count(),
            seconds_since_epoch(),
            self.block_details(),
            self.source_epoch(),
        )
    }

    fn new_account_info(&self) -> AccountInfo {
        AccountInfo {
            head: self.block.hash(),
            representative: self.new_representative(),
            open_block: self.open_block(),
            balance: self.new_balance(),
            modified: seconds_since_epoch(),
            block_count: self.new_block_count(),
            epoch: self.epoch(),
        }
    }

    fn new_block_count(&self) -> u64 {
        self.old_account_info
            .as_ref()
            .map(|info| info.block_count)
            .unwrap_or_default()
            + 1
    }

    fn block_details(&self) -> BlockDetails {
        BlockDetails::new(
            self.epoch(),
            self.is_send(),
            self.is_receive(),
            self.is_epoch_block(),
        )
    }

    fn balance_changed(&self) -> bool {
        if let Some(info) = &self.old_account_info {
            self.new_balance() != info.balance
        } else {
            false
        }
    }
}

/// Validates a single block before it gets inserted into the ledger
pub(crate) struct BlockValidator<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
    helper: BlockValidationHelper<'a>,
}

impl<'a> BlockValidator<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a dyn Transaction, block: &'a BlockEnum) -> Self {
        Self {
            ledger,
            txn,
            helper: BlockValidationHelper::new(block, &ledger.constants.epochs),
        }
    }

    pub(crate) fn validate(&mut self) -> Result<BlockValidation, ProcessResult> {
        // Epoch block pre-checks for early return
        // It's important to abort here with BadSignature first, so that the block does
        // not get added to the unchecked map!
        self.ensure_epoch_block_candidate_is_signed_by_owner_or_epoch_account()?;
        self.ensure_previous_block_exists_for_epoch_block_candidate()?;
        self.ensure_block_does_not_exist_yet()?;

        self.load_related_block_data()?;

        self.ensure_valid_signature()?;
        self.ensure_block_is_not_for_burn_account()?;
        self.ensure_account_exists()?;
        self.ensure_no_double_account_open()?;
        self.ensure_previous_block_is_correct()?;
        self.ensure_open_block_has_link()?;
        self.ensure_corresponding_send_is_correct()?;
        self.ensure_sufficient_work()?;
        self.ensure_no_negative_amount_spend()?;
        self.ensure_valid_epoch_block()?;

        Ok(self.create_validation())
    }

    fn ensure_previous_block_is_correct(&self) -> Result<(), ProcessResult> {
        self.ensure_previous_block_exists()?;
        self.ensure_previous_block_is_account_head()
    }

    fn ensure_valid_epoch_block(&self) -> Result<(), ProcessResult> {
        self.ensure_epoch_block_does_not_change_representative()?;
        self.ensure_epoch_open_has_burn_account_as_rep()?;
        self.ensure_epoch_open_has_pending_entry()?;
        self.ensure_valid_epoch_for_unopened_account()?;
        self.ensure_epoch_upgrade_is_sequential_for_existing_account()?;
        self.ensure_epoch_block_does_not_change_balance()
    }

    fn ensure_corresponding_send_is_correct(&self) -> Result<(), ProcessResult> {
        self.ensure_no_receive_balance_change_without_link()?;
        self.ensure_source_block_exists()?;
        self.ensure_receive_block_receives_pending_amount()?;
        self.ensure_legacy_source_is_epoch_0()
    }

    /// This is a precheck that allows for an early return if a block with an epoch link
    /// is not signed by the account owner or the epoch signer.
    /// It is not sure yet, if the block is an epoch block, because it could just be
    /// a send to the epoch account.
    fn ensure_epoch_block_candidate_is_signed_by_owner_or_epoch_account(
        &self,
    ) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state_block) = self.helper.block {
            // Check for possible regular state blocks with epoch link (send subtype)
            if self.helper.has_epoch_link(state_block)
                && (validate_block_signature(self.helper.block).is_err()
                    && self
                        .ledger
                        .validate_epoch_signature(self.helper.block)
                        .is_err())
            {
                return Err(ProcessResult::BadSignature);
            }
        }
        Ok(())
    }

    fn ensure_previous_block_exists_for_epoch_block_candidate(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state_block) = self.helper.block {
            if self.helper.has_epoch_link(state_block)
                && !self.helper.block.previous().is_zero()
                && !self
                    .ledger
                    .store
                    .block()
                    .exists(self.txn, &state_block.previous())
            {
                return Err(ProcessResult::GapPrevious);
            }
        }
        Ok(())
    }

    fn ensure_block_does_not_exist_yet(&self) -> Result<(), ProcessResult> {
        if self
            .ledger
            .block_or_pruned_exists_txn(self.txn, &self.helper.block.hash())
        {
            return Err(ProcessResult::Old);
        }
        Ok(())
    }

    fn get_account(&self) -> Result<Account, ProcessResult> {
        let account = match self.helper.block {
            BlockEnum::LegacyOpen(open) => open.account(),
            BlockEnum::State(state) => state.account(),
            BlockEnum::LegacySend(_) | BlockEnum::LegacyReceive(_) | BlockEnum::LegacyChange(_) => {
                let previous = self.ensure_previous_block_exists2(&self.helper.block.previous())?;
                self.ensure_valid_predecessor(&previous)?;
                self.ensure_frontier(&self.helper.block.previous())?
            }
        };
        Ok(account)
    }

    fn ensure_frontier(&self, previous: &BlockHash) -> Result<Account, ProcessResult> {
        self.ledger
            .get_frontier(self.txn, &previous)
            .ok_or(ProcessResult::Fork)
    }

    fn ensure_valid_predecessor(&self, previous: &BlockEnum) -> Result<(), ProcessResult> {
        if !self.helper.block.valid_predecessor(previous.block_type()) {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_exists2(
        &self,
        previous: &BlockHash,
    ) -> Result<BlockEnum, ProcessResult> {
        self.ledger
            .get_block(self.txn, previous)
            .ok_or(ProcessResult::GapPrevious)
    }

    fn ensure_valid_signature(&self) -> Result<(), ProcessResult> {
        let result = if self.helper.is_epoch_block() {
            self.ledger.validate_epoch_signature(self.helper.block)
        } else {
            validate_message(
                &self.helper.account,
                self.helper.block.hash().as_bytes(),
                self.helper.block.block_signature(),
            )
        };
        result.map_err(|_| ProcessResult::BadSignature)
    }

    fn ensure_block_is_not_for_burn_account(&self) -> Result<(), ProcessResult> {
        if self.helper.account.is_zero() {
            Err(ProcessResult::OpenedBurnAccount)
        } else {
            Ok(())
        }
    }

    fn ensure_no_double_account_open(&self) -> Result<(), ProcessResult> {
        if self.helper.old_account_info.is_some() && self.helper.block.is_open() {
            Err(ProcessResult::Fork)
        } else {
            Ok(())
        }
    }

    fn ensure_previous_block_exists(&self) -> Result<(), ProcessResult> {
        if self.helper.old_account_info.is_some() && self.helper.previous_block.is_none() {
            return Err(ProcessResult::GapPrevious);
        }

        if self.helper.old_account_info.is_none() && !self.helper.block.previous().is_zero() {
            return Err(ProcessResult::GapPrevious);
        }

        Ok(())
    }

    /// Is the previous block the account's head block? (Ambigious)
    fn ensure_previous_block_is_account_head(&self) -> Result<(), ProcessResult> {
        if let Some(info) = &self.helper.old_account_info {
            if self.helper.block.previous() != info.head {
                return Err(ProcessResult::Fork);
            }
        }

        Ok(())
    }

    fn ensure_account_exists(&self) -> Result<(), ProcessResult> {
        if !self.helper.block.is_open() && self.helper.old_account_info.is_none() {
            Err(ProcessResult::GapPrevious)
        } else {
            Ok(())
        }
    }

    fn ensure_open_block_has_link(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.helper.block {
            if self.helper.old_account_info.is_none() && state.link().is_zero() {
                return Err(ProcessResult::GapSource);
            }
        }
        Ok(())
    }

    /// If there's no link, the balance must remain the same, only the representative can change
    fn ensure_no_receive_balance_change_without_link(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.helper.block {
            if !self.helper.is_send() && state.link().is_zero() {
                if !self.helper.amount().is_zero() {
                    return Err(ProcessResult::BalanceMismatch);
                }
            }
        }

        Ok(())
    }

    fn ensure_receive_block_receives_pending_amount(&self) -> Result<(), ProcessResult> {
        if self.helper.is_receive() {
            match &self.helper.pending_receive_info {
                Some(pending) => {
                    if self.helper.amount() != pending.amount {
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

    fn ensure_source_block_exists(&self) -> Result<(), ProcessResult> {
        let source = match self.helper.block {
            BlockEnum::LegacyReceive(receive) => receive.mandatory_source(),
            BlockEnum::LegacyOpen(open) => open.mandatory_source(),
            BlockEnum::State(_) => {
                if self.helper.is_receive() {
                    self.helper.block.link().into()
                } else {
                    return Ok(());
                }
            }
            _ => return Ok(()),
        };

        if !self.ledger.block_or_pruned_exists_txn(self.txn, &source) {
            Err(ProcessResult::GapSource)
        } else {
            Ok(())
        }
    }

    fn ensure_legacy_source_is_epoch_0(&self) -> Result<(), ProcessResult> {
        let is_legacy_receive = match self.helper.block {
            BlockEnum::LegacyReceive(_) | BlockEnum::LegacyOpen(_) => true,
            _ => false,
        };

        if is_legacy_receive
            && self
                .helper
                .pending_receive_info
                .as_ref()
                .map(|x| x.epoch)
                .unwrap_or_default()
                != Epoch::Epoch0
        {
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
            .is_valid_pow(self.helper.block, &self.helper.block_details())
        {
            Err(ProcessResult::InsufficientWork)
        } else {
            Ok(())
        }
    }

    fn ensure_no_negative_amount_spend(&self) -> Result<(), ProcessResult> {
        // Is this trying to spend a negative amount (Malicious)
        if self.helper.block.block_type() == BlockType::LegacySend
            && self.helper.previous_balance() < self.helper.block.balance()
        {
            return Err(ProcessResult::NegativeSpend);
        };

        Ok(())
    }

    fn ensure_epoch_block_does_not_change_representative(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.helper.block {
            if self.helper.is_epoch_block() {
                if let Some(info) = &self.helper.old_account_info {
                    if state.mandatory_representative() != info.representative {
                        return Err(ProcessResult::RepresentativeMismatch);
                    };
                }
            }
        }
        Ok(())
    }

    fn ensure_epoch_open_has_burn_account_as_rep(&self) -> Result<(), ProcessResult> {
        if let BlockEnum::State(state) = self.helper.block {
            if self.helper.is_epoch_block()
                && self.helper.old_account_info.is_none()
                && !state.mandatory_representative().is_zero()
            {
                return Err(ProcessResult::RepresentativeMismatch);
            }
        }
        Ok(())
    }

    fn ensure_epoch_open_has_pending_entry(&self) -> Result<(), ProcessResult> {
        if self.helper.old_account_info.is_none() && self.helper.is_epoch_block() {
            // Non-exisitng account should have pending entries
            let pending_exists = self
                .ledger
                .store
                .pending()
                .any(self.txn, &self.helper.block.account());
            if !pending_exists {
                return Err(ProcessResult::GapEpochOpenPending);
            };
        }
        Ok(())
    }

    fn ensure_valid_epoch_for_unopened_account(&self) -> Result<(), ProcessResult> {
        if self.helper.old_account_info.is_none()
            && self.helper.is_epoch_block()
            && self.helper.block_epoch_version() == Epoch::Invalid
        {
            Err(ProcessResult::BlockPosition)
        } else {
            Ok(())
        }
    }

    fn ensure_epoch_upgrade_is_sequential_for_existing_account(&self) -> Result<(), ProcessResult> {
        if self.helper.is_epoch_block() {
            if let Some(info) = &self.helper.old_account_info {
                if !Epochs::is_sequential(info.epoch, self.helper.block_epoch_version()) {
                    return Err(ProcessResult::BlockPosition);
                }
            }
        }
        Ok(())
    }

    fn ensure_epoch_block_does_not_change_balance(&self) -> Result<(), ProcessResult> {
        if self.helper.is_epoch_block() && self.helper.balance_changed() {
            return Err(ProcessResult::BalanceMismatch);
        }
        Ok(())
    }

    fn create_validation(&self) -> BlockValidation {
        BlockValidation {
            account: self.helper.account,
            old_account_info: self.helper.old_account_info.clone().unwrap_or_default(),
            new_account_info: self.helper.new_account_info(),
            pending_received: self.helper.pending_receive_key.clone(),
            new_pending: self.helper.new_pending_info(),
            new_sideband: self.helper.new_sideband(),
            is_epoch_block: self.helper.is_epoch_block(),
        }
    }

    fn load_related_block_data(&mut self) -> Result<(), ProcessResult> {
        self.helper.account = self.get_account()?;
        self.helper.old_account_info = self.ledger.get_account_info(self.txn, &self.helper.account);
        self.helper.previous_block = self.load_previous_block();
        self.helper.pending_receive_key = self.helper.get_pending_receive_key();
        self.helper.pending_receive_info = self.load_pending_receive_info();
        Ok(())
    }

    fn load_pending_receive_info(&self) -> Option<PendingInfo> {
        if let Some(key) = &self.helper.pending_receive_key {
            self.ledger.store.pending().get(self.txn, &key)
        } else {
            None
        }
    }

    fn load_previous_block(&self) -> Option<BlockEnum> {
        if !self.helper.block.previous().is_zero() {
            self.ledger
                .get_block(self.txn, &self.helper.block.previous())
        } else {
            None
        }
    }
}
