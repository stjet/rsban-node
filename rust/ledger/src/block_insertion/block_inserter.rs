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
    use super::*;
    use rsnano_core::{BlockBuilder, BlockHash};
    use rsnano_store_lmdb::EnvironmentStub;

    #[test]
    fn insert_open_block() {
        let (mut block, instructions) = open_state_block_instructions();
        let ledger = Ledger::create_null();

        let result = insert(&ledger, &mut block, &instructions);

        assert_eq!(block.sideband().unwrap(), &instructions.set_sideband);
        assert_eq!(result.saved_blocks, vec![block]);
        assert_eq!(
            result.saved_accounts,
            vec![(instructions.account, instructions.set_account_info.clone())]
        );
        assert_eq!(
            ledger
                .cache
                .rep_weights
                .representation_get(&instructions.set_account_info.representative),
            instructions.set_account_info.balance
        );
        assert_eq!(ledger.cache.block_count.load(Ordering::Relaxed), 1);
        assert_eq!(result.deleted_pending, Vec::new());
    }

    #[test]
    fn delete_old_frontier() {
        let mut open = BlockBuilder::legacy_open().build();
        let sideband = BlockSideband {
            successor: BlockHash::zero(),
            ..BlockSideband::create_test_instance()
        };
        open.set_sideband(sideband);

        let (mut block, instructions) = state_block_instructions(&open);

        let ledger = Ledger::create_null_with()
            .block(&open)
            .frontier(&instructions.old_account_info.head, &instructions.account)
            .build();

        let result = insert(&ledger, &mut block, &instructions);

        assert_eq!(
            result.deleted_frontiers,
            vec![instructions.old_account_info.head]
        )
    }

    fn insert(
        ledger: &Ledger<EnvironmentStub>,
        block: &mut BlockEnum,
        instructions: &BlockInsertInstructions,
    ) -> InsertResult {
        let mut txn = ledger.rw_txn();
        let saved_blocks = ledger.store.block.track_puts();
        let saved_accounts = ledger.store.account.track_puts();
        let deleted_pending = ledger.store.pending.track_deletions();
        let deleted_frontiers = ledger.store.frontier.track_deletions();

        let mut block_inserter = BlockInserter::new(&ledger, &mut txn, block, &instructions);
        block_inserter.insert();

        InsertResult {
            saved_blocks: saved_blocks.output(),
            saved_accounts: saved_accounts.output(),
            deleted_pending: deleted_pending.output(),
            deleted_frontiers: deleted_frontiers.output(),
        }
    }

    struct InsertResult {
        saved_blocks: Vec<BlockEnum>,
        saved_accounts: Vec<(Account, AccountInfo)>,
        deleted_pending: Vec<PendingKey>,
        deleted_frontiers: Vec<BlockHash>,
    }

    fn open_state_block_instructions() -> (BlockEnum, BlockInsertInstructions) {
        let block = BlockBuilder::state().previous(BlockHash::zero()).build();
        let sideband = BlockSideband {
            successor: BlockHash::zero(),
            ..BlockSideband::create_test_instance()
        };
        let account_info = AccountInfo {
            head: block.hash(),
            open_block: block.hash(),
            ..AccountInfo::create_test_instance()
        };
        let instructions = BlockInsertInstructions {
            account: Account::from(1),
            old_account_info: AccountInfo::default(),
            set_account_info: account_info,
            delete_pending: None,
            insert_pending: None,
            set_sideband: sideband,
            is_epoch_block: false,
        };

        (block, instructions)
    }

    fn state_block_instructions(previous: &BlockEnum) -> (BlockEnum, BlockInsertInstructions) {
        let block = BlockBuilder::state().previous(previous.hash()).build();
        let sideband = BlockSideband {
            successor: BlockHash::zero(),
            ..BlockSideband::create_test_instance()
        };
        let old_account_info = AccountInfo {
            head: previous.hash(),
            ..AccountInfo::create_test_instance()
        };
        let new_account_info = AccountInfo {
            head: block.hash(),
            open_block: block.hash(),
            ..AccountInfo::create_test_instance()
        };
        let instructions = BlockInsertInstructions {
            account: Account::from(1),
            old_account_info,
            set_account_info: new_account_info,
            delete_pending: None,
            insert_pending: None,
            set_sideband: sideband,
            is_epoch_block: false,
        };

        (block, instructions)
    }
}
