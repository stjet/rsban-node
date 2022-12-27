use rsnano_core::{
    Account, AccountInfo, BlockEnum, BlockHash, ConfirmationHeightInfo, PendingInfo, PendingKey,
};
use rsnano_store_traits::Transaction;

use crate::Ledger;

use super::rollback_planner::RollbackPlanner;

pub(crate) struct RollbackPlannerFactory<'a> {
    ledger: &'a Ledger,
    txn: &'a dyn Transaction,
    head_block: &'a BlockEnum,
}

impl<'a> RollbackPlannerFactory<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a dyn Transaction,
        head_block: &'a BlockEnum,
    ) -> Self {
        Self {
            ledger,
            txn,
            head_block,
        }
    }

    pub(crate) fn create_planner(&self) -> anyhow::Result<RollbackPlanner<'a>> {
        let account = self.get_account(self.head_block)?;
        let planner = RollbackPlanner {
            epochs: &self.ledger.constants.epochs,
            head_block: &self.head_block,
            account,
            current_account_info: self.load_account(&account),
            previous_representative: self.get_previous_representative()?,
            previous: self.load_previous_block()?,
            linked_account: self.load_linked_account(),
            pending_receive: self.load_pending_receive(),
            latest_block_for_destination: self.latest_block_for_destination(),
            confirmation_height: self.account_confirmation_height(),
        };
        Ok(planner)
    }

    fn latest_block_for_destination(&self) -> Option<BlockHash> {
        self.ledger
            .latest(self.txn, &self.head_block.destination_or_link())
    }

    fn load_pending_receive(&self) -> Option<PendingInfo> {
        self.ledger.store.pending().get(
            self.txn,
            &PendingKey::new(
                self.head_block.destination_or_link(),
                self.head_block.hash(),
            ),
        )
    }

    fn load_linked_account(&self) -> Account {
        self.ledger
            .account(self.txn, &self.head_block.source_or_link())
            .unwrap_or_default()
    }

    fn load_previous_block(&self) -> anyhow::Result<Option<BlockEnum>> {
        let previous = self.head_block.previous();
        Ok(if previous.is_zero() {
            None
        } else {
            Some(self.load_block(&previous)?)
        })
    }

    fn account_confirmation_height(&self) -> ConfirmationHeightInfo {
        self.ledger
            .store
            .confirmation_height()
            .get(self.txn, &self.head_block.account_calculated())
            .unwrap_or_default()
    }

    fn get_account(&self, block: &BlockEnum) -> anyhow::Result<Account> {
        self.ledger
            .account(self.txn, &block.hash())
            .ok_or_else(|| anyhow!("account not found"))
    }

    fn load_account(&self, account: &Account) -> AccountInfo {
        self.ledger
            .store
            .account()
            .get(self.txn, account)
            .unwrap_or_default()
    }

    fn load_block(&self, block_hash: &BlockHash) -> anyhow::Result<BlockEnum> {
        self.ledger
            .store
            .block()
            .get(self.txn, block_hash)
            .ok_or_else(|| anyhow!("block not found"))
    }

    fn get_previous_representative(&self) -> anyhow::Result<Option<Account>> {
        let previous = self.head_block.previous();
        let rep_block_hash = if !previous.is_zero() {
            self.ledger.representative_block_hash(self.txn, &previous)
        } else {
            BlockHash::zero()
        };

        let previous_rep = if !rep_block_hash.is_zero() {
            let rep_block = self.load_block(&rep_block_hash)?;
            Some(rep_block.representative().unwrap_or_default())
        } else {
            None
        };
        Ok(previous_rep)
    }
}
