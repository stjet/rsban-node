use std::sync::{Arc, RwLock};

use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, Block, BlockEnum, BlockHash,
    BlockSubType, Epoch, PendingInfo, PendingKey, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use super::Ledger;

pub(crate) struct BlockRollbackPerformer<'a> {
    ledger: &'a Ledger,
    pub txn: &'a mut dyn WriteTransaction,
    pub rolled_back: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
}

impl<'a> BlockRollbackPerformer<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        list: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
    ) -> Self {
        Self {
            ledger,
            txn,
            rolled_back: list,
        }
    }

    pub(crate) fn roll_back(&mut self, block: &BlockEnum) -> anyhow::Result<()> {
        match block {
            BlockEnum::LegacySend(_)
            | BlockEnum::LegacyReceive(_)
            | BlockEnum::LegacyOpen(_)
            | BlockEnum::LegacyChange(_) => self.rollback_legacy_block(block),
            BlockEnum::State(state) => self.rollback_state_block(block, state),
        }
    }

    pub(crate) fn rollback_legacy_block(&mut self, block: &BlockEnum) -> anyhow::Result<()> {
        let account = self.get_account(block)?;
        let amount = self.get_amount(block)?;
        let current_account_info = self.load_account(&account);
        let previous_representative = self.get_representative(&block.previous())?;
        let previous_balance = self.ledger.balance(self.txn.txn(), &block.previous());

        if let Some(destination) = block.destination() {
            self.roll_back_destination_account_until_send_block_is_unreceived(
                destination,
                block.hash(),
            )?;
        }

        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let linked_account = block
            .source()
            .map(|source| {
                self.ledger
                    .account(self.txn.txn(), &source)
                    .ok_or_else(|| anyhow!("linked account not found"))
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        if let Some(destination) = block.destination() {
            let pending_key = PendingKey::new(destination, block.hash());
            self.ledger.store.pending().del(self.txn, &pending_key);
        }

        self.do_roll_back(
            block,
            &current_account_info,
            &account,
            &linked_account,
            &amount,
            previous_representative,
        );

        self.roll_back_representative_cache(
            &current_account_info.representative,
            &current_account_info.balance,
            previous_representative,
            previous_balance,
        );

        self.ledger.observer.block_rolled_back2(block, false);
        Ok(())
    }

    pub(crate) fn rollback_state_block(
        &mut self,
        block: &BlockEnum,
        state: &StateBlock,
    ) -> anyhow::Result<()> {
        let current_account_info = self.load_account(&block.account());
        let previous_rep = self.get_representative(&block.previous())?;
        let previous_balance = self.ledger.balance(self.txn.txn(), &block.previous());

        self.roll_back_representative_cache(
            &state.mandatory_representative(),
            &state.balance(),
            previous_rep,
            previous_balance,
        );

        let is_send = block.balance() < previous_balance;
        if is_send {
            let destination_account = block.link().into();
            let key = PendingKey::new(block.link().into(), block.hash());
            self.roll_back_destination_account_until_send_block_is_unreceived(
                destination_account,
                block.hash(),
            )?;
            self.ledger.store.pending().del(self.txn, &key);
            self.ledger.observer.block_rolled_back(BlockSubType::Send);
        } else if !block.link().is_zero() && !self.ledger.is_epoch_link(&block.link()) {
            self.add_pending_receive(block, previous_balance);
            self.ledger
                .observer
                .block_rolled_back(BlockSubType::Receive);
        }

        let previous_account_info =
            self.previous_account_info(block, &current_account_info, previous_rep);

        self.ledger.update_account(
            self.txn,
            &block.account(),
            &current_account_info,
            &previous_account_info,
        );

        if block.is_open() {
            self.ledger.observer.block_rolled_back(BlockSubType::Open);
        } else {
            let previous = self.load_block(&block.previous())?;

            self.ledger
                .store
                .block()
                .successor_clear(self.txn, &block.previous());

            self.add_frontier(&previous, &block.account());
        }

        self.ledger.store.block().del(self.txn, &block.hash());
        Ok(())
    }

    /*************************************************************
     * Helper Functions
     *************************************************************/

    fn get_account(&self, block: &BlockEnum) -> anyhow::Result<Account> {
        self.ledger
            .account(self.txn.txn(), &block.hash())
            .ok_or_else(|| anyhow!("account not found"))
    }

    fn get_amount(&self, block: &BlockEnum) -> anyhow::Result<Amount> {
        self.ledger
            .amount(self.txn.txn(), &block.hash())
            .ok_or_else(|| anyhow!("amount not found"))
    }

    fn add_frontier(&mut self, block: &BlockEnum, account: &Account) {
        match block {
            BlockEnum::State(_) => {}
            _ => self
                .ledger
                .store
                .frontier()
                .put(self.txn, &block.hash(), account),
        }
    }

    fn add_pending_receive(&mut self, block: &BlockEnum, previous_balance: Amount) {
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let linked_account = self
            .ledger
            .account(self.txn.txn(), &block.link().into())
            .unwrap_or_default();

        let pending_info = PendingInfo::new(
            linked_account,
            block.balance() - previous_balance,
            block.sideband().unwrap().source_epoch,
        );

        self.ledger.store.pending().put(
            self.txn,
            &PendingKey::new(block.account(), block.link().into()),
            &pending_info,
        );
    }

    fn roll_back_destination_account_until_send_block_is_unreceived(
        &mut self,
        destination_account: Account,
        send_block: BlockHash,
    ) -> anyhow::Result<()> {
        let pending_key = PendingKey::new(destination_account, send_block);
        loop {
            if self
                .ledger
                .store
                .pending()
                .get(self.txn.txn(), &pending_key)
                .is_some()
            {
                return Ok(());
            }

            self.recurse_roll_back(&self.latest_block_for_account(&pending_key.account)?)?;
        }
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

    fn do_roll_back(
        &mut self,
        block: &BlockEnum,
        current_account_info: &AccountInfo,
        account: &Account,
        linked_account: &Account,
        amount: &Amount,
        previous_representative: Option<Account>,
    ) {
        let previous_account_info =
            self.previous_account_info(block, current_account_info, previous_representative);

        self.ledger.update_account(
            self.txn,
            account,
            current_account_info,
            &previous_account_info,
        );

        self.ledger.store.block().del(self.txn, &block.hash());

        let receive_source_block = match block {
            BlockEnum::LegacyReceive(receive) => Some(receive.mandatory_source()),
            BlockEnum::LegacyOpen(open) => Some(open.mandatory_source()),
            _ => None,
        };
        if let Some(source) = receive_source_block {
            self.ledger.store.pending().put(
                self.txn,
                &PendingKey::new(*account, source),
                &PendingInfo::new(*linked_account, *amount, Epoch::Epoch0),
            );
        }

        self.ledger.store.frontier().del(self.txn, &block.hash());

        if !block.previous().is_zero() {
            self.ledger
                .store
                .frontier()
                .put(self.txn, &block.previous(), account);

            self.ledger
                .store
                .block()
                .successor_clear(self.txn, &block.previous());
        }
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
            AccountInfo {
                head: block.previous(),
                representative: previous_rep.unwrap_or(current_info.representative),
                open_block: current_info.open_block,
                balance: self.ledger.balance(self.txn.txn(), &block.previous()),
                modified: seconds_since_epoch(),
                block_count: current_info.block_count - 1,
                epoch: self.get_block_version(&block.previous()),
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

    fn get_block_version(&self, block_hash: &BlockHash) -> Epoch {
        self.ledger
            .store
            .block()
            .version(self.txn.txn(), block_hash)
    }
}
