use std::sync::{Arc, RwLock};

use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, Block, BlockEnum, BlockHash,
    BlockSubType, BlockType, ChangeBlock, Epoch, OpenBlock, PendingInfo, PendingKey, ReceiveBlock,
    SendBlock, StateBlock,
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
            BlockEnum::LegacyOpen(open) => self.rollback_legacy_open(open),
            BlockEnum::LegacyChange(change) => self.rollback_legacy_change(change),
            BlockEnum::State(state) => self.rollback_state_block(state),
        }
    }

    pub(crate) fn rollback_legacy_send(
        &mut self,
        block: &BlockEnum,
        send: &SendBlock,
    ) -> anyhow::Result<()> {
        let pending_info =
            self.rollback_destination_account_until_send_block_is_unreceived(send)?;

        let account = &pending_info.source;
        let old_account_info = self.load_account(account)?;
        self.delete_pending(send);

        self.roll_back_send_in_representative_cache(
            &old_account_info.representative,
            &pending_info.amount,
        );

        self.do_roll_back(
            block,
            &old_account_info,
            account,
            &Account::zero(),
            &pending_info.amount,
        );

        self.ledger.observer.block_rolled_back(BlockSubType::Send);
        Ok(())
    }

    pub(crate) fn rollback_legacy_receive(
        &mut self,
        block: &BlockEnum,
        receive: &ReceiveBlock,
    ) -> anyhow::Result<()> {
        let amount = self.ledger.amount(self.txn.txn(), &receive.hash()).unwrap();
        let destination_account = self
            .ledger
            .account(self.txn.txn(), &receive.hash())
            .unwrap();
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let source_account = self.get_source_account(receive);
        let old_account_info = self.load_account(&destination_account)?;

        self.roll_back_receive_in_representative_cache(&old_account_info.representative, amount);

        self.do_roll_back(
            block,
            &old_account_info,
            &destination_account,
            &source_account,
            &amount,
        );

        self.ledger
            .observer
            .block_rolled_back(BlockSubType::Receive);
        Ok(())
    }

    pub(crate) fn rollback_legacy_open(&mut self, block: &OpenBlock) -> anyhow::Result<()> {
        let hash = block.hash();
        let amount = self.ledger.amount(self.txn.txn(), &hash).unwrap();
        let destination_account = self.ledger.account(self.txn.txn(), &hash).unwrap();
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let source_account = self
            .ledger
            .account(self.txn.txn(), &block.mandatory_source())
            .unwrap_or_default();

        self.ledger.cache.rep_weights.representation_add(
            block.hashables.representative,
            Amount::zero().wrapping_sub(amount),
        );

        self.ledger.update_account(
            self.txn,
            &destination_account,
            &AccountInfo::default(),
            &AccountInfo::default(),
        );

        self.ledger.store.block().del(self.txn, &hash);
        self.ledger.store.pending().put(
            self.txn,
            &PendingKey::new(destination_account, block.mandatory_source()),
            &PendingInfo::new(source_account, amount, Epoch::Epoch0),
        );

        self.ledger.store.frontier().del(self.txn, &hash);

        self.ledger.observer.block_rolled_back(BlockSubType::Open);
        Ok(())
    }

    pub(crate) fn rollback_legacy_change(&mut self, block: &ChangeBlock) -> anyhow::Result<()> {
        let hash = block.hash();
        let rep_block = self
            .ledger
            .representative_block(self.txn.txn(), &block.previous());
        let account = self
            .ledger
            .account(self.txn.txn(), &block.previous())
            .unwrap();

        let account_info = self
            .ledger
            .store
            .account()
            .get(self.txn.txn(), &account)
            .unwrap();

        let balance = self.ledger.balance(self.txn.txn(), &block.previous());
        let rep_block = self
            .ledger
            .store
            .block()
            .get(self.txn.txn(), &rep_block)
            .unwrap();

        let representative = rep_block.representative().unwrap_or_default();
        self.ledger.cache.rep_weights.representation_add_dual(
            block.mandatory_representative(),
            Amount::zero().wrapping_sub(balance),
            representative,
            balance,
        );

        self.ledger.store.block().del(self.txn, &hash);
        let new_info = AccountInfo {
            head: block.previous(),
            representative,
            open_block: account_info.open_block,
            balance: account_info.balance,
            modified: seconds_since_epoch(),
            block_count: account_info.block_count - 1,
            epoch: Epoch::Epoch0,
        };

        self.ledger
            .update_account(self.txn, &account, &account_info, &new_info);

        self.ledger.store.frontier().del(self.txn, &hash);

        self.ledger
            .store
            .frontier()
            .put(self.txn, &block.previous(), &account);

        self.ledger
            .store
            .block()
            .successor_clear(self.txn, &block.previous());

        self.ledger.observer.block_rolled_back(BlockSubType::Change);
        Ok(())
    }

    pub(crate) fn rollback_state_block(&mut self, block: &StateBlock) -> anyhow::Result<()> {
        let hash = block.hash();
        let mut rep_block_hash = BlockHash::zero();
        if !block.previous().is_zero() {
            rep_block_hash = self
                .ledger
                .representative_block(self.txn.txn(), &block.previous());
        }
        let balance = self.ledger.balance(self.txn.txn(), &block.previous());
        let is_send = block.balance() < balance;
        let mut representative = Account::zero();
        if !rep_block_hash.is_zero() {
            // Move existing representation & add in amount delta
            let rep_block = self
                .ledger
                .store
                .block()
                .get(self.txn.txn(), &rep_block_hash)
                .unwrap();
            representative = rep_block.representative().unwrap_or_default();
            self.ledger.cache.rep_weights.representation_add_dual(
                representative,
                balance,
                block.mandatory_representative(),
                Amount::zero().wrapping_sub(block.balance()),
            );
        } else {
            // Add in amount delta only
            self.ledger.cache.rep_weights.representation_add(
                block.mandatory_representative(),
                Amount::zero().wrapping_sub(block.balance()),
            );
        }

        let (mut error, account_info) = match self
            .ledger
            .store
            .account()
            .get(self.txn.txn(), &block.account())
        {
            Some(info) => (false, info),
            None => (true, AccountInfo::default()),
        };

        if is_send {
            let key = PendingKey::new(block.link().into(), hash);
            while !error && !self.ledger.store.pending().exists(self.txn.txn(), &key) {
                let latest = self
                    .ledger
                    .latest(self.txn.txn(), &block.link().into())
                    .unwrap();
                match self.ledger.rollback(self.txn, &latest) {
                    Ok(mut list) => self.rolled_back.append(&mut list),
                    Err(_) => error = true,
                };
            }
            self.ledger.store.pending().del(self.txn, &key);
            self.ledger.observer.block_rolled_back(BlockSubType::Send);
        } else if !block.link().is_zero() && !self.ledger.is_epoch_link(&block.link()) {
            // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
            let source_account = self
                .ledger
                .account(self.txn.txn(), &block.link().into())
                .unwrap_or_default();
            let pending_info = PendingInfo::new(
                source_account,
                block.balance() - balance,
                block.sideband().unwrap().source_epoch,
            );
            self.ledger.store.pending().put(
                self.txn,
                &PendingKey::new(block.account(), block.link().into()),
                &pending_info,
            );
            self.ledger
                .observer
                .block_rolled_back(BlockSubType::Receive);
        }
        assert!(!error);
        let previous_version = self
            .ledger
            .store
            .block()
            .version(self.txn.txn(), &block.previous());

        let new_info = AccountInfo {
            head: block.previous(),
            representative,
            open_block: account_info.open_block,
            balance,
            modified: seconds_since_epoch(),
            block_count: account_info.block_count - 1,
            epoch: previous_version,
        };

        self.ledger
            .update_account(self.txn, &block.account(), &account_info, &new_info);

        match self
            .ledger
            .store
            .block()
            .get(self.txn.txn(), &block.previous())
        {
            Some(previous) => {
                self.ledger
                    .store
                    .block()
                    .successor_clear(self.txn, &block.previous());
                match previous.block_type() {
                    BlockType::Invalid | BlockType::NotABlock => unreachable!(),
                    BlockType::LegacySend
                    | BlockType::LegacyReceive
                    | BlockType::LegacyOpen
                    | BlockType::LegacyChange => {
                        self.ledger.store.frontier().put(
                            self.txn,
                            &block.previous(),
                            &block.account(),
                        );
                    }
                    BlockType::State => {}
                }
            }
            None => {
                self.ledger.observer.block_rolled_back(BlockSubType::Open);
            }
        }

        self.ledger.store.block().del(self.txn, &hash);
        Ok(())
    }

    /*************************************************************
     * Helper Functions
     *************************************************************/

    fn load_pending_info_for_send_block(&self, block: &SendBlock) -> Option<PendingInfo> {
        self.ledger
            .store
            .pending()
            .get(self.txn.txn(), &block.pending_key())
    }

    fn rollback_destination_account_until_send_block_is_unreceived(
        &mut self,
        block: &SendBlock,
    ) -> anyhow::Result<PendingInfo> {
        loop {
            if let Some(info) = self.load_pending_info_for_send_block(block) {
                return Ok(info);
            }

            self.recurse_roll_back(&self.latest_block_for_destination(block)?)?;
        }
    }

    fn recurse_roll_back(&mut self, block_hash: &BlockHash) -> anyhow::Result<()> {
        let mut rolled_back = self.ledger.rollback(self.txn, block_hash)?;
        self.rolled_back.append(&mut rolled_back);
        Ok(())
    }

    fn latest_block_for_destination(&self, block: &SendBlock) -> anyhow::Result<BlockHash> {
        self.ledger
            .latest(self.txn.txn(), &block.hashables.destination)
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

    fn do_roll_back(
        &mut self,
        block: &BlockEnum,
        old_account_info: &AccountInfo,
        account: &Account,
        linked_account: &Account,
        amount: &Amount,
    ) {
        let new_account_info = self.new_account_info(block, old_account_info);
        self.ledger
            .update_account(self.txn, account, old_account_info, &new_account_info);
        self.ledger.store.block().del(self.txn, &block.hash());

        if let BlockEnum::LegacyReceive(receive) = block {
            self.ledger.store.pending().put(
                self.txn,
                &PendingKey::new(*account, receive.mandatory_source()),
                &PendingInfo::new(*linked_account, *amount, Epoch::Epoch0),
            );
        }

        self.ledger.store.frontier().del(self.txn, &block.hash());

        self.ledger
            .store
            .frontier()
            .put(self.txn, &block.previous(), account);

        self.ledger
            .store
            .block()
            .successor_clear(self.txn, &block.previous());
    }

    fn delete_pending(&mut self, block: &SendBlock) {
        self.ledger
            .store
            .pending()
            .del(self.txn, &block.pending_key());
    }

    fn new_account_info(&self, block: &BlockEnum, account_info: &AccountInfo) -> AccountInfo {
        AccountInfo {
            head: block.previous(),
            representative: account_info.representative,
            open_block: account_info.open_block,
            balance: self.ledger.balance(self.txn.txn(), &block.previous()),
            modified: seconds_since_epoch(),
            block_count: account_info.block_count - 1,
            epoch: Epoch::Epoch0,
        }
    }

    fn load_account(&self, account: &Account) -> anyhow::Result<AccountInfo> {
        self.ledger
            .store
            .account()
            .get(self.txn.txn(), account)
            .ok_or_else(|| anyhow!("account not found"))
    }
}
