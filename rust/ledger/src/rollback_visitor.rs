use std::sync::{Arc, RwLock};

use rsnano_core::{
    utils::seconds_since_epoch, Account, AccountInfo, Amount, Block, BlockEnum, BlockHash,
    BlockSubType, BlockType, BlockVisitor, ChangeBlock, Epoch, OpenBlock, PendingInfo, PendingKey,
    ReceiveBlock, SendBlock, StateBlock,
};
use rsnano_store_traits::WriteTransaction;

use super::Ledger;

pub(crate) struct RollbackVisitor<'a> {
    pub txn: &'a mut dyn WriteTransaction,
    ledger: &'a Ledger,
    pub list: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
    pub is_error: bool,
}

struct BlockRollbackPerformer<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    list: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
    pending_key: PendingKey,
    pending_info: PendingInfo,
}

impl<'a> BlockRollbackPerformer<'a> {
    fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        list: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
    ) -> Self {
        Self {
            ledger,
            txn,
            list,
            pending_key: Default::default(),
            pending_info: Default::default(),
        }
    }

    pub(crate) fn rollback_legacy_send(&mut self, block: &'a SendBlock) -> anyhow::Result<()> {
        self.pending_key = PendingKey::new(block.hashables.destination, block.hash());
        self.pending_info = PendingInfo::default();
        loop {
            match self
                .ledger
                .store
                .pending()
                .get(self.txn.txn(), &self.pending_key)
            {
                Some(info) => {
                    self.pending_info = info;
                    break;
                }
                None => {
                    self.pending_info = PendingInfo::default();
                }
            }

            let latest_block = self
                .ledger
                .latest(self.txn.txn(), &block.hashables.destination)
                .unwrap();

            let mut blocks = self.ledger.rollback(self.txn, &latest_block)?;
            self.list.append(&mut blocks);
        }

        let account_info = self
            .ledger
            .store
            .account()
            .get(self.txn.txn(), &self.pending_info.source)
            .unwrap();

        self.ledger.store.pending().del(self.txn, &self.pending_key);

        self.ledger
            .cache
            .rep_weights
            .representation_add(account_info.representative, self.pending_info.amount);

        let new_info = AccountInfo {
            head: block.previous(),
            representative: account_info.representative,
            open_block: account_info.open_block,
            balance: self.ledger.balance(self.txn.txn(), &block.previous()),
            modified: seconds_since_epoch(),
            block_count: account_info.block_count - 1,
            epoch: Epoch::Epoch0,
        };

        self.ledger.update_account(
            self.txn,
            &self.pending_info.source,
            &account_info,
            &new_info,
        );

        self.ledger.store.block().del(self.txn, &block.hash());
        self.ledger.store.frontier().del(self.txn, &block.hash());

        self.ledger
            .store
            .frontier()
            .put(self.txn, &block.previous(), &self.pending_info.source);

        self.ledger
            .store
            .block()
            .successor_clear(self.txn, &block.previous());

        self.ledger.observer.block_rolled_back(BlockSubType::Send);
        Ok(())
    }

    pub(crate) fn rollback_legacy_receive(
        &mut self,
        block: &'a ReceiveBlock,
    ) -> anyhow::Result<()> {
        let amount = self.ledger.amount(self.txn.txn(), &block.hash()).unwrap();
        let destination_account = self.ledger.account(self.txn.txn(), &block.hash()).unwrap();
        // Pending account entry can be incorrect if source block was pruned. But it's not affecting correct ledger processing
        let source_account = self
            .ledger
            .account(self.txn.txn(), &block.mandatory_source())
            .unwrap_or_default();

        let account_info = self
            .ledger
            .store
            .account()
            .get(self.txn.txn(), &destination_account)
            .unwrap();

        self.ledger.cache.rep_weights.representation_add(
            account_info.representative,
            Amount::zero().wrapping_sub(amount),
        );

        let new_info = AccountInfo {
            head: block.previous(),
            representative: account_info.representative,
            open_block: account_info.open_block,
            balance: self.ledger.balance(self.txn.txn(), &block.previous()),
            modified: seconds_since_epoch(),
            block_count: account_info.block_count - 1,
            epoch: Epoch::Epoch0,
        };
        self.ledger
            .update_account(self.txn, &destination_account, &account_info, &new_info);

        self.ledger.store.block().del(self.txn, &block.hash());

        self.ledger.store.pending().put(
            self.txn,
            &PendingKey::new(destination_account, block.mandatory_source()),
            &PendingInfo::new(source_account, amount, Epoch::Epoch0),
        );

        self.ledger.store.frontier().del(self.txn, &block.hash());

        self.ledger
            .store
            .frontier()
            .put(self.txn, &block.previous(), &destination_account);

        self.ledger
            .store
            .block()
            .successor_clear(self.txn, &block.previous());

        self.ledger
            .observer
            .block_rolled_back(BlockSubType::Receive);
        Ok(())
    }

    pub(crate) fn rollback_legacy_open(&mut self, block: &'a OpenBlock) -> anyhow::Result<()> {
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

    pub(crate) fn rollback_legacy_change(&mut self, block: &'a ChangeBlock) -> anyhow::Result<()> {
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

    pub(crate) fn rollback_state_block(&mut self, block: &'a StateBlock) -> anyhow::Result<()> {
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
                    Ok(mut list) => self.list.append(&mut list),
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
}

impl<'a> RollbackVisitor<'a> {
    pub(crate) fn new(
        txn: &'a mut dyn WriteTransaction,
        ledger: &'a Ledger,
        list: &'a mut Vec<Arc<RwLock<BlockEnum>>>,
    ) -> Self {
        Self {
            txn,
            ledger,
            list,
            is_error: false,
        }
    }
}

impl<'a> BlockVisitor for RollbackVisitor<'a> {
    fn send_block(&mut self, block: &SendBlock) {
        if self.is_error {
            return;
        }

        let mut performer = BlockRollbackPerformer::new(self.ledger, self.txn, self.list);
        self.is_error = performer.rollback_legacy_send(block).is_err();
    }

    fn receive_block(&mut self, block: &ReceiveBlock) {
        if self.is_error {
            return;
        }

        let mut performer = BlockRollbackPerformer::new(self.ledger, self.txn, self.list);
        self.is_error = performer.rollback_legacy_receive(block).is_err();
    }

    fn open_block(&mut self, block: &OpenBlock) {
        if self.is_error {
            return;
        }
        let mut performer = BlockRollbackPerformer::new(self.ledger, self.txn, self.list);
        self.is_error = performer.rollback_legacy_open(block).is_err();
    }

    fn change_block(&mut self, block: &ChangeBlock) {
        if self.is_error {
            return;
        }
        let mut performer = BlockRollbackPerformer::new(self.ledger, self.txn, self.list);
        self.is_error = performer.rollback_legacy_change(block).is_err();
    }

    fn state_block(&mut self, block: &StateBlock) {
        if self.is_error {
            return;
        }
        let mut performer = BlockRollbackPerformer::new(self.ledger, self.txn, self.list);
        self.is_error = performer.rollback_state_block(block).is_err();
    }
}