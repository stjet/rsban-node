use std::sync::atomic::Ordering;

use crate::Ledger;
use rsnano_core::{
    Account, AccountInfo, Amount, BlockEnum, BlockSideband, BlockType, PendingInfo, PendingKey,
};
use rsnano_store_lmdb::{Environment, LmdbWriteTransaction};

pub(crate) struct BlockInsertInstructions {
    pub account: Account,
    pub old_account_info: AccountInfo,
    pub set_account_info: AccountInfo,
    pub delete_pending: Option<PendingKey>,
    pub insert_pending: Option<(PendingKey, PendingInfo)>,
    pub set_sideband: BlockSideband,
    pub is_epoch_block: bool,
}

/// Inserts a new block into the ledger
pub(crate) struct BlockInserter<'a, T: Environment + 'static> {
    ledger: &'a Ledger<T>,
    txn: &'a mut LmdbWriteTransaction<T>,
    block: &'a mut BlockEnum,
    instructions: &'a BlockInsertInstructions,
}

impl<'a, T: Environment> BlockInserter<'a, T> {
    pub(crate) fn new(
        ledger: &'a Ledger<T>,
        txn: &'a mut LmdbWriteTransaction<T>,
        block: &'a mut BlockEnum,
        instructions: &'a BlockInsertInstructions,
    ) -> Self {
        Self {
            ledger,
            txn,
            block,
            instructions,
        }
    }

    pub(crate) fn insert(&mut self) {
        self.set_block_sideband();
        self.ledger.store.block.put(self.txn, self.block);
        self.update_account();
        self.delete_old_pending_info();
        self.insert_new_pending_info();
        self.delete_old_frontier();
        self.insert_new_frontier();
        self.update_representative_cache();
        self.ledger
            .observer
            .block_added(self.block, self.instructions.is_epoch_block);
        self.ledger.cache.block_count.fetch_add(1, Ordering::SeqCst);
    }

    fn set_block_sideband(&mut self) {
        self.block
            .set_sideband(self.instructions.set_sideband.clone());
    }

    fn update_account(&mut self) {
        self.ledger.update_account(
            self.txn,
            &self.instructions.account,
            &self.instructions.old_account_info,
            &self.instructions.set_account_info,
        );
    }

    fn delete_old_frontier(&mut self) {
        if self
            .ledger
            .store
            .frontier
            .get(self.txn, &self.instructions.old_account_info.head)
            .is_some()
        {
            self.ledger
                .store
                .frontier
                .del(self.txn, &self.instructions.old_account_info.head);
        }
    }

    fn insert_new_frontier(&mut self) {
        if self.block.block_type() != BlockType::State {
            self.ledger.store.frontier.put(
                self.txn,
                &self.block.hash(),
                &self.instructions.account,
            );
        }
    }

    fn delete_old_pending_info(&mut self) {
        if let Some(key) = &self.instructions.delete_pending {
            self.ledger.store.pending.del(self.txn, key);
        }
    }

    fn insert_new_pending_info(&mut self) {
        if let Some((key, info)) = &self.instructions.insert_pending {
            self.ledger.store.pending.put(self.txn, key, info);
        }
    }

    fn update_representative_cache(&mut self) {
        if !self.instructions.old_account_info.head.is_zero() {
            // Move existing representation & add in amount delta
            self.ledger.cache.rep_weights.representation_add_dual(
                self.instructions.old_account_info.representative,
                Amount::zero().wrapping_sub(self.instructions.old_account_info.balance),
                self.instructions.set_account_info.representative,
                self.instructions.set_account_info.balance,
            );
        } else {
            // Add in amount delta only
            self.ledger.cache.rep_weights.representation_add(
                self.instructions.set_account_info.representative,
                self.instructions.set_account_info.balance,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::{BlockBuilder, BlockHash};

    use super::*;

    #[test]
    fn insert_open_block() {
        let ledger = Ledger::create_null();
        let mut txn = ledger.rw_txn();
        let mut block = BlockBuilder::state().previous(BlockHash::zero()).build();
        let sideband = BlockSideband {
            successor: BlockHash::zero(),
            ..BlockSideband::create_test_instance()
        };
        let instructions = BlockInsertInstructions {
            account: Account::from(1),
            old_account_info: AccountInfo::create_test_instance(),
            set_account_info: AccountInfo::create_test_instance(),
            delete_pending: None,
            insert_pending: None,
            set_sideband: sideband,
            is_epoch_block: false,
        };

        let put_tracker = ledger.store.block.track_puts();
        let mut block_inserter = BlockInserter::new(&ledger, &mut txn, &mut block, &instructions);
        block_inserter.insert();

        assert_eq!(block.sideband().unwrap(), &instructions.set_sideband);
        assert_eq!(put_tracker.output(), vec![block]);
        // todo more asserts
    }
}
