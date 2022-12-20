use rsnano_core::{
    utils::seconds_since_epoch, AccountInfo, Amount, Block, BlockDetails, BlockEnum, BlockHash,
    BlockSideband, Epoch, PendingInfo, PendingKey, StateBlock,
};

use super::BlockValidator;
use crate::ProcessResult;

impl<'a> BlockValidator<'a> {
    pub(crate) fn previous_balance(&self) -> Amount {
        self.previous_block
            .as_ref()
            .map(|b| b.balance_calculated())
            .unwrap_or_default()
    }

    pub(crate) fn is_send(&self) -> bool {
        match self.block {
            BlockEnum::LegacySend(_) => true,
            BlockEnum::State(state) => match &self.old_account_info {
                Some(info) => state.balance() < info.balance,
                None => false,
            },
            _ => false,
        }
    }

    pub(crate) fn is_receive(&self) -> bool {
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

    pub(crate) fn source_epoch(&self) -> Epoch {
        self.pending_receive_info
            .as_ref()
            .map(|p| p.epoch)
            .unwrap_or(Epoch::Epoch0)
    }

    pub(crate) fn amount_received(&self) -> Amount {
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

    pub(crate) fn amount_sent(&self) -> Amount {
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

    pub(crate) fn new_balance(&self) -> Amount {
        self.old_account_info
            .as_ref()
            .map(|i| i.balance)
            .unwrap_or_default()
            + self.amount_received()
            - self.amount_sent()
    }

    pub(crate) fn has_epoch_link(&self, state_block: &StateBlock) -> bool {
        self.ledger.is_epoch_link(&state_block.link())
    }

    /// This check only makes sense after ensure_previous_block_exists_for_epoch_block_candidate,
    /// because we need the previous block for the balance change check!
    pub(crate) fn is_epoch_block(&self) -> bool {
        match self.block {
            BlockEnum::State(state_block) => {
                self.has_epoch_link(state_block) && self.previous_balance() == self.block.balance()
            }
            _ => false,
        }
    }

    pub(crate) fn block_epoch_version(&self) -> Epoch {
        match self.block {
            BlockEnum::State(state) => self
                .ledger
                .constants
                .epochs
                .epoch(&state.link())
                .unwrap_or(Epoch::Invalid),
            _ => Epoch::Epoch0,
        }
    }

    pub(crate) fn epoch(&self) -> Epoch {
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

    pub(crate) fn open_block(&self) -> BlockHash {
        let open_block = match &self.old_account_info {
            Some(info) => info.open_block,
            None => self.block.hash(),
        };
        open_block
    }

    pub(crate) fn new_representative(&self) -> rsnano_core::PublicKey {
        self.block.representative().unwrap_or(
            self.old_account_info
                .as_ref()
                .map(|x| x.representative)
                .unwrap_or_default(),
        )
    }

    pub(crate) fn amount(&self) -> Amount {
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

    pub(crate) fn new_pending_info(&self) -> Option<(PendingKey, PendingInfo)> {
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

    pub(crate) fn get_pending_receive_key(&self) -> Option<PendingKey> {
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

    pub(crate) fn new_sideband(&self) -> BlockSideband {
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

    pub(crate) fn new_account_info(&self) -> AccountInfo {
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

    pub(crate) fn new_block_count(&self) -> u64 {
        self.old_account_info
            .as_ref()
            .map(|info| info.block_count)
            .unwrap_or_default()
            + 1
    }

    pub(crate) fn block_details(&self) -> BlockDetails {
        BlockDetails::new(
            self.epoch(),
            self.is_send(),
            self.is_receive(),
            self.is_epoch_block(),
        )
    }

    pub(crate) fn balance_changed(&self) -> bool {
        if let Some(info) = &self.old_account_info {
            self.new_balance() != info.balance
        } else {
            false
        }
    }

    pub(crate) fn load_related_block_data(&mut self) -> Result<(), ProcessResult> {
        self.account = self.get_account()?;
        self.old_account_info = self.ledger.get_account_info(self.txn, &self.account);
        self.previous_block = self.load_previous_block();
        self.pending_receive_key = self.get_pending_receive_key();
        self.pending_receive_info = self.load_pending_receive_info();
        Ok(())
    }

    pub fn load_pending_receive_info(&self) -> Option<PendingInfo> {
        if let Some(key) = &self.pending_receive_key {
            self.ledger.store.pending().get(self.txn, &key)
        } else {
            None
        }
    }

    pub fn load_previous_block(&self) -> Option<BlockEnum> {
        if !self.block.previous().is_zero() {
            self.ledger.get_block(self.txn, &self.block.previous())
        } else {
            None
        }
    }
}
