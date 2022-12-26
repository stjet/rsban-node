use std::sync::atomic::Ordering;

use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, BlockEnum, BlockHash, BlockSubType,
    ConfirmationHeightInfo, Epoch, Epochs, PendingInfo, PendingKey,
};
use rsnano_store_traits::WriteTransaction;

use super::Ledger;

pub(crate) enum RollbackStep {
    RollBackBlock(RollbackInstructions),
    RequestDependencyRollback(BlockHash),
}

#[derive(Default)]
pub(crate) struct RollbackInstructions {
    block_hash: BlockHash,
    account: Account,
    remove_pending: Option<PendingKey>,
    add_pending: Option<(PendingKey, PendingInfo)>,
    set_account_info: AccountInfo,
    old_account_info: AccountInfo,
    delete_frontier: Option<BlockHash>,
    add_frontier: Option<(BlockHash, Account)>,
    clear_successor: Option<BlockHash>,
}

pub(crate) struct BlockRollbackPerformer<'a> {
    ledger: &'a Ledger,
    pub txn: &'a mut dyn WriteTransaction,
    pub rolled_back: Vec<BlockEnum>,

    planner: RollbackPlanner<'a>,
}

impl<'a> BlockRollbackPerformer<'a> {
    pub(crate) fn new(ledger: &'a Ledger, txn: &'a mut dyn WriteTransaction) -> Self {
        Self {
            ledger,
            txn,
            rolled_back: Vec::new(),
            planner: RollbackPlanner {
                epochs: &ledger.constants.epochs,
                account: Account::zero(),
                current_account_info: Default::default(),
                previous_representative: Default::default(),
                previous: Default::default(),
                linked_account: Account::zero(),
                pending_receive: None,
                latest_block_for_destination: None,
            },
        }
    }

    pub(crate) fn roll_back(mut self, block_hash: &BlockHash) -> anyhow::Result<Vec<BlockEnum>> {
        let block = self.load_block(block_hash)?;
        while self.block_exists(block_hash) {
            self.ensure_block_is_not_confirmed(&block)?;
            let head_block = self.load_account_head(&block)?;
            self.planner.account = self.get_account(&head_block)?;
            self.planner.current_account_info = self.load_account(&self.planner.account);
            self.planner.previous_representative =
                self.get_representative(&head_block.previous())?;

            self.planner.previous = if head_block.previous().is_zero() {
                None
            } else {
                Some(self.load_block(&head_block.previous())?)
            };

            self.planner.linked_account = self
                .ledger
                .account(self.txn.txn(), &head_block.source_or_link())
                .unwrap_or_default();

            self.planner.pending_receive = self.ledger.store.pending().get(
                self.txn.txn(),
                &PendingKey::new(head_block.destination_or_link(), head_block.hash()),
            );
            self.planner.latest_block_for_destination = self
                .ledger
                .latest(self.txn.txn(), &head_block.destination_or_link());

            match self.roll_back_head_block(&head_block)? {
                RollbackStep::RollBackBlock(instructions) => {
                    self.apply(instructions);
                    self.rolled_back.push(head_block.clone());
                }
                RollbackStep::RequestDependencyRollback(hash) => self.recurse_roll_back(&hash)?,
            }
        }

        Ok(self.rolled_back)
    }

    fn apply(&mut self, instructions: RollbackInstructions) {
        if let Some(pending_key) = instructions.remove_pending {
            self.ledger.store.pending().del(self.txn, &pending_key);
        }
        if let Some((key, info)) = instructions.add_pending {
            self.ledger.store.pending().put(self.txn, &key, &info);
        }
        self.ledger.update_account(
            self.txn,
            &instructions.account,
            &instructions.old_account_info,
            &instructions.set_account_info,
        );
        self.ledger
            .store
            .block()
            .del(self.txn, &instructions.block_hash);
        if let Some(hash) = instructions.delete_frontier {
            self.ledger.store.frontier().del(self.txn, &hash);
        }
        if let Some((hash, account)) = instructions.add_frontier {
            self.ledger.store.frontier().put(self.txn, &hash, &account)
        }
        if let Some(hash) = instructions.clear_successor {
            self.ledger.store.block().successor_clear(self.txn, &hash);
        }
    }

    fn load_account_head(&self, block: &BlockEnum) -> anyhow::Result<BlockEnum> {
        let account_info = self.get_account_info(block);
        self.load_block(&account_info.head)
    }

    fn get_account_info(&self, block: &BlockEnum) -> AccountInfo {
        self.ledger
            .store
            .account()
            .get(self.txn.txn(), &block.account_calculated())
            .unwrap()
    }

    fn ensure_block_is_not_confirmed(&self, block: &BlockEnum) -> anyhow::Result<()> {
        let conf_height = self.account_confirmation_height(block);

        if block.sideband().unwrap().height <= conf_height.height {
            bail!("Only unconfirmed blocks can be rolled back")
        }

        Ok(())
    }

    fn account_confirmation_height(&self, block: &BlockEnum) -> ConfirmationHeightInfo {
        self.ledger
            .store
            .confirmation_height()
            .get(self.txn.txn(), &block.account_calculated())
            .unwrap_or_default()
    }

    fn block_exists(&self, block_hash: &BlockHash) -> bool {
        self.ledger.store.block().exists(self.txn.txn(), block_hash)
    }

    pub(crate) fn roll_back_head_block(
        &mut self,
        head_block: &BlockEnum,
    ) -> anyhow::Result<RollbackStep> {
        let mut instructions = RollbackInstructions {
            block_hash: head_block.hash(),
            account: self.planner.account,
            old_account_info: self.planner.current_account_info.clone(),
            ..Default::default()
        };

        let previous_balance = self
            .planner
            .previous
            .as_ref()
            .map(|b| b.balance_calculated())
            .unwrap_or_default();

        let sub_type = if self.planner.current_account_info.balance < previous_balance {
            BlockSubType::Send
        } else if self.planner.current_account_info.balance > previous_balance {
            if head_block.is_open() {
                BlockSubType::Open
            } else {
                BlockSubType::Receive
            }
        } else if self.planner.epochs.is_epoch_link(&head_block.link()) {
            BlockSubType::Epoch
        } else {
            BlockSubType::Change
        };

        match sub_type {
            BlockSubType::Send => {
                let destination = head_block.destination_or_link();
                match self.roll_back_destination_account_if_send_block_is_received()? {
                    Some(step) => return Ok(step),
                    None => {
                        instructions.remove_pending =
                            Some(PendingKey::new(destination, head_block.hash()));
                    }
                }
            }
            BlockSubType::Receive | BlockSubType::Open => {
                let source_hash = head_block.source_or_link();
                // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing

                instructions.add_pending = Some((
                    PendingKey::new(self.planner.account, source_hash),
                    PendingInfo::new(
                        self.planner.linked_account,
                        self.planner.current_account_info.balance - previous_balance,
                        head_block.sideband().unwrap().source_epoch,
                    ),
                ));
            }
            _ => {}
        }

        instructions.set_account_info = self.previous_account_info(
            head_block,
            &self.planner.current_account_info,
            self.planner.previous_representative,
        );

        if head_block.is_legacy() {
            instructions.delete_frontier = Some(head_block.hash());
            if let Some(previous) = &self.planner.previous {
                instructions.add_frontier = Some((previous.hash(), self.planner.account));
            }
        }

        instructions.clear_successor = self.planner.previous.as_ref().map(|b| b.hash());

        self.roll_back_representative_cache(
            &self.planner.current_account_info.representative,
            &self.planner.current_account_info.balance,
            self.planner.previous_representative,
            previous_balance,
        );

        self.ledger.cache.block_count.fetch_sub(1, Ordering::SeqCst);
        self.ledger.observer.block_rolled_back(sub_type);
        Ok(RollbackStep::RollBackBlock(instructions))
    }

    /*************************************************************
     * Helper Functions
     *************************************************************/

    fn get_account(&self, block: &BlockEnum) -> anyhow::Result<Account> {
        self.ledger
            .account(self.txn.txn(), &block.hash())
            .ok_or_else(|| anyhow!("account not found"))
    }

    fn roll_back_destination_account_if_send_block_is_received(
        &mut self,
    ) -> anyhow::Result<Option<RollbackStep>> {
        if self.planner.pending_receive.is_some() {
            return Ok(None);
        }

        let latest_destination_block = self
            .planner
            .latest_block_for_destination
            .ok_or_else(|| anyhow!("no latest block for destination"))?;

        Ok(Some(RollbackStep::RequestDependencyRollback(
            latest_destination_block,
        )))
    }

    fn recurse_roll_back(&mut self, block_hash: &BlockHash) -> anyhow::Result<()> {
        let mut rolled_back = self.ledger.rollback(self.txn, block_hash)?;
        self.rolled_back.append(&mut rolled_back);
        Ok(())
    }

    fn latest_block_for_account(&self, account: &Account) -> anyhow::Result<BlockHash> {
        self.ledger
            .latest(self.txn.txn(), account)
            .ok_or_else(|| anyhow!("no latest block found"))
    }

    fn roll_back_receive_in_representative_cache(&self, representative: &Account, amount: Amount) {
        self.ledger
            .cache
            .rep_weights
            .representation_add(*representative, Amount::zero().wrapping_sub(amount));
    }

    fn roll_back_representative_cache(
        &self,
        current_rep: &Account,
        current_balance: &Amount,
        previous_rep: Option<Account>,
        previous_balance: Amount,
    ) {
        if let Some(previous_rep) = previous_rep {
            self.roll_back_change_in_representative_cache(
                current_rep,
                current_balance,
                &previous_rep,
                &previous_balance,
            );
        } else {
            self.roll_back_receive_in_representative_cache(current_rep, *current_balance)
        }
    }

    fn roll_back_change_in_representative_cache(
        &self,
        current_representative: &Account,
        current_balance: &Amount,
        previous_representative: &Account,
        previous_balance: &Amount,
    ) {
        self.ledger.cache.rep_weights.representation_add_dual(
            *current_representative,
            Amount::zero().wrapping_sub(*current_balance),
            *previous_representative,
            *previous_balance,
        );
    }

    fn previous_account_info(
        &self,
        block: &BlockEnum,
        current_info: &AccountInfo,
        previous_rep: Option<Account>,
    ) -> AccountInfo {
        if block.previous().is_zero() {
            Default::default()
        } else {
            let balance = match &self.planner.previous {
                Some(previous) => previous.balance_calculated(),
                None => Amount::zero(),
            };

            let epoch = match &self.planner.previous {
                Some(previous) => previous.sideband().unwrap().details.epoch,
                None => Epoch::Epoch0,
            };

            AccountInfo {
                head: block.previous(),
                representative: previous_rep.unwrap_or(current_info.representative),
                open_block: current_info.open_block,
                balance,
                modified: seconds_since_epoch(),
                block_count: current_info.block_count - 1,
                epoch,
            }
        }
    }

    fn load_account(&self, account: &Account) -> AccountInfo {
        self.ledger
            .store
            .account()
            .get(self.txn.txn(), account)
            .unwrap_or_default()
    }

    fn load_block(&self, block_hash: &BlockHash) -> anyhow::Result<BlockEnum> {
        self.ledger
            .store
            .block()
            .get(self.txn.txn(), block_hash)
            .ok_or_else(|| anyhow!("block not found"))
    }

    fn get_representative(&self, block_hash: &BlockHash) -> anyhow::Result<Option<Account>> {
        let rep_block_hash = if !block_hash.is_zero() {
            self.ledger
                .representative_block_hash(self.txn.txn(), block_hash)
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

pub(crate) struct RollbackPlanner<'a> {
    pub epochs: &'a Epochs,
    pub account: Account,
    pub current_account_info: AccountInfo,
    pub previous_representative: Option<Account>,
    pub previous: Option<BlockEnum>,
    pub linked_account: Account,
    pub pending_receive: Option<PendingInfo>,
    pub latest_block_for_destination: Option<BlockHash>,
}
