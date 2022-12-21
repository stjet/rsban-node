use std::sync::{Arc, RwLock};

use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, Block, BlockEnum, BlockHash,
    BlockSubType, ChangeBlock, Epoch, OpenBlock, PendingInfo, PendingKey, ReceiveBlock, SendBlock,
    StateBlock,
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
            BlockEnum::LegacySend(send) => self.rollback_legacy_send(block, send),
            BlockEnum::LegacyReceive(receive) => self.rollback_legacy_receive(block, receive),
            BlockEnum::LegacyOpen(open) => self.rollback_legacy_open(block, open),
            BlockEnum::LegacyChange(change) => self.rollback_legacy_change(block, change),
            BlockEnum::State(state) => self.rollback_state_block(block, state),
        }
    }

    pub(crate) fn rollback_legacy_send(
        &mut self,
        block: &BlockEnum,
        send: &SendBlock,
    ) -> anyhow::Result<()> {
        let pending_key = send.pending_key();
        let pending_info =
            self.roll_back_destination_account_until_send_block_is_unreceived(&pending_key)?;

        let account = &pending_info.source;
        let current_account_info = self.load_account(account)?;
        self.ledger.store.pending().del(self.txn, &pending_key);

        self.roll_back_send_in_representative_cache(
            &current_account_info.representative,
            &pending_info.amount,
        );

        self.do_roll_back(
            block,
            &current_account_info,
            account,
            &Account::zero(),
            &pending_info.amount,
            None,
        );

        self.ledger.observer.block_rolled_back(BlockSubType::Send);
        Ok(())
    }

    pub(crate) fn rollback_legacy_receive(
        &mut self,
        block: &BlockEnum,
        receive: &ReceiveBlock,
    ) -> anyhow::Result<()> {
        let amount = self.ledger.amount(self.txn.txn(), &block.hash()).unwrap();
        let account = self.ledger.account(self.txn.txn(), &block.hash()).unwrap();
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let linked_account = self.get_source_account(receive);
        let current_account_info = self.load_account(&account)?;

        self.roll_back_receive_in_representative_cache(
            &current_account_info.representative,
            amount,
        );

        self.do_roll_back(
            block,
            &current_account_info,
            &account,
            &linked_account,
            &amount,
            None,
        );

        self.ledger
            .observer
            .block_rolled_back(BlockSubType::Receive);
        Ok(())
    }

    pub(crate) fn rollback_legacy_open(
        &mut self,
        block: &BlockEnum,
        open: &OpenBlock,
    ) -> anyhow::Result<()> {
        let current_account_info = AccountInfo::default();

        let amount = self.ledger.amount(self.txn.txn(), &block.hash()).unwrap();
        let account = self.ledger.account(self.txn.txn(), &block.hash()).unwrap();
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let linked_account = self
            .ledger
            .account(self.txn.txn(), &open.mandatory_source())
            .unwrap_or_default();

        self.roll_back_receive_in_representative_cache(&open.hashables.representative, amount);

        self.do_roll_back(
            block,
            &current_account_info,
            &account,
            &linked_account,
            &amount,
            None,
        );

        self.ledger.observer.block_rolled_back(BlockSubType::Open);
        Ok(())
    }

    pub(crate) fn rollback_legacy_change(
        &mut self,
        block: &BlockEnum,
        change: &ChangeBlock,
    ) -> anyhow::Result<()> {
        let amount = Amount::zero();
        let account = self.ledger.account(self.txn.txn(), &change.hash()).unwrap();

        let linked_account = Account::zero();
        let current_account_info = self.load_account(&account)?;

        let previous_representative = self.get_previous_representative(block)?.unwrap();
        let previous_balance = self.ledger.balance(self.txn.txn(), &block.previous());

        self.roll_back_change_in_representative_cache(
            &change.mandatory_representative(),
            &previous_balance,
            &previous_representative,
            &previous_balance,
        );

        self.do_roll_back(
            block,
            &current_account_info,
            &account,
            &linked_account,
            &amount,
            Some(previous_representative),
        );

        self.ledger.observer.block_rolled_back(BlockSubType::Change);
        Ok(())
    }

    pub(crate) fn rollback_state_block(
        &mut self,
        block: &BlockEnum,
        state: &StateBlock,
    ) -> anyhow::Result<()> {
        let previous_rep = self.get_previous_representative(block)?;
        let previous_balance = self.ledger.balance(self.txn.txn(), &block.previous());
        let is_send = block.balance() < previous_balance;
        if let Some(previous_rep) = previous_rep {
            self.roll_back_change_in_representative_cache(
                &state.mandatory_representative(),
                &block.balance(),
                &previous_rep,
                &previous_balance,
            );
        } else {
            self.roll_back_receive_in_representative_cache(
                &state.mandatory_representative(),
                block.balance(),
            )
        }

        let current_account_info = self.load_account(&block.account())?;

        if is_send {
            let key = PendingKey::new(block.link().into(), block.hash());
            self.roll_back_destination_account_until_send_block_is_unreceived(&key)?;
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
        pending_key: &PendingKey,
    ) -> anyhow::Result<PendingInfo> {
        loop {
            if let Some(info) = self.ledger.store.pending().get(self.txn.txn(), pending_key) {
                return Ok(info);
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

    fn get_source_account(&self, block: &ReceiveBlock) -> rsnano_core::PublicKey {
        self.ledger
            .account(self.txn.txn(), &block.mandatory_source())
            .unwrap_or_default()
    }

    fn roll_back_send_in_representative_cache(&self, representative: &Account, amount: &Amount) {
        self.ledger
            .cache
            .rep_weights
            .representation_add(*representative, *amount);
    }

    fn roll_back_receive_in_representative_cache(&self, representative: &Account, amount: Amount) {
        self.ledger
            .cache
            .rep_weights
            .representation_add(*representative, Amount::zero().wrapping_sub(amount));
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

    fn load_account(&self, account: &Account) -> anyhow::Result<AccountInfo> {
        self.ledger
            .store
            .account()
            .get(self.txn.txn(), account)
            .ok_or_else(|| anyhow!("account not found"))
    }

    fn load_block(&self, block_hash: &BlockHash) -> anyhow::Result<BlockEnum> {
        self.ledger
            .store
            .block()
            .get(self.txn.txn(), block_hash)
            .ok_or_else(|| anyhow!("block not found"))
    }

    fn get_previous_representative(&self, block: &BlockEnum) -> anyhow::Result<Option<Account>> {
        let rep_block_hash = if !block.previous().is_zero() {
            self.ledger
                .representative_block_hash(self.txn.txn(), &block.previous())
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
