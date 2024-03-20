use rsnano_core::{utils::seconds_since_epoch, Account, BlockEnum, PendingKey};
use rsnano_store_lmdb::{Environment, Transaction};

use crate::Ledger;

use super::BlockValidator;

pub(crate) struct BlockValidatorFactory<'a, T: Environment + 'static> {
    ledger: &'a Ledger<T>,
    txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
    block: &'a BlockEnum,
}

impl<'a, T: Environment + 'static> BlockValidatorFactory<'a, T> {
    pub(crate) fn new(
        ledger: &'a Ledger<T>,
        txn: &'a dyn Transaction<Database = T::Database, RoCursor = T::RoCursor>,
        block: &'a BlockEnum,
    ) -> Self {
        Self { ledger, txn, block }
    }

    pub(crate) fn create_validator(&self) -> BlockValidator<'a> {
        let account = self.get_account();
        let frontier_missing = account.is_none();
        let account = account.unwrap_or_default();
        let previous_block = self.load_previous_block();
        let source_block = self.block.source_or_link();
        let source_block_exists = !source_block.is_zero()
            && self
                .ledger
                .block_or_pruned_exists_txn(self.txn, &source_block);

        let pending_receive_info = if source_block.is_zero() {
            None
        } else {
            self.ledger
                .pending_info(self.txn, &PendingKey::new(account, source_block))
        };

        BlockValidator {
            block: self.block,
            epochs: &self.ledger.constants.epochs,
            work: &self.ledger.constants.work,
            account,
            frontier_missing,
            block_exists: self
                .ledger
                .block_or_pruned_exists_txn(self.txn, &self.block.hash()),
            old_account_info: self.ledger.account_info(self.txn, &account),
            pending_receive_info,
            any_pending_exists: self.ledger.store.pending.any(self.txn, &account),
            source_block_exists,
            previous_block,
            seconds_since_epoch: seconds_since_epoch(),
        }
    }

    fn get_account(&self) -> Option<Account> {
        match self.block.account_field() {
            Some(account) => Some(account),
            None => self.get_account_from_frontier_table(),
        }
    }

    fn get_account_from_frontier_table(&self) -> Option<Account> {
        self.ledger.get_frontier(self.txn, &self.block.previous())
    }

    fn load_previous_block(&self) -> Option<BlockEnum> {
        if !self.block.previous().is_zero() {
            self.ledger.get_block(self.txn, &self.block.previous())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsnano_core::{AccountInfo, BlockBuilder, BlockHash, Link, PendingInfo};

    #[test]
    fn block_for_unknown_account() {
        let block = BlockBuilder::state().build();
        let ledger = Ledger::create_null_with().build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();

        assert_eq!(validator.block.hash(), block.hash());
        assert_eq!(validator.epochs, &ledger.constants.epochs);
        assert_eq!(validator.account, block.account_field().unwrap());
        assert_eq!(validator.frontier_missing, false);
        assert_eq!(validator.block_exists, false);
        assert_eq!(validator.old_account_info, None);
        assert_eq!(validator.pending_receive_info, None);
        assert_eq!(validator.any_pending_exists, false);
        assert_eq!(validator.source_block_exists, false);
        assert_eq!(validator.previous_block, None);
        assert!(validator.seconds_since_epoch >= seconds_since_epoch());
    }

    #[test]
    fn frontier_missing() {
        let block = BlockBuilder::legacy_send().build();
        let ledger = Ledger::create_null_with().build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();

        assert_eq!(validator.account, Account::zero());
        assert_eq!(validator.frontier_missing, true);
    }

    #[test]
    fn frontier_not_missing() {
        let block = BlockBuilder::legacy_send().build();
        let ledger = Ledger::create_null_with()
            .frontier(&block.previous(), &Account::from(42))
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();

        assert_eq!(validator.account, Account::from(42));
        assert_eq!(validator.frontier_missing, false);
    }

    #[test]
    fn block_exists() {
        let block = BlockBuilder::state().with_sideband().build();
        let ledger = Ledger::create_null_with().block(&block).build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.block_exists, true);
    }

    #[test]
    fn pruned_block_exists() {
        let block = BlockBuilder::state().build();
        let ledger = Ledger::create_null_with().pruned(&block.hash()).build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.block_exists, true);
    }

    #[test]
    fn account_info() {
        let block = BlockBuilder::state().build();
        let account_info = AccountInfo::create_test_instance();
        let ledger = Ledger::create_null_with()
            .account_info(&block.account_field().unwrap(), &account_info)
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.old_account_info, Some(account_info));
    }

    #[test]
    fn pending_receive_info_for_state_block() {
        let block = BlockBuilder::state().link(Link::from(42)).build();
        let pending_info = PendingInfo::create_test_instance();
        let ledger = Ledger::create_null_with()
            .pending(
                &PendingKey::new(block.account_field().unwrap(), BlockHash::from(42)),
                &pending_info,
            )
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.pending_receive_info, Some(pending_info));
    }

    #[test]
    fn pending_receive_info_for_legacy_receive() {
        let block = BlockBuilder::legacy_receive()
            .source(BlockHash::from(42))
            .build();
        let account = Account::from(1111);
        let pending_info = PendingInfo::create_test_instance();
        let ledger = Ledger::create_null_with()
            .frontier(&block.previous(), &account)
            .pending(
                &PendingKey::new(account, BlockHash::from(42)),
                &pending_info,
            )
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.pending_receive_info, Some(pending_info));
    }

    #[test]
    fn any_pending_exists() {
        let block = BlockBuilder::state().build();
        let pending_info = PendingInfo::create_test_instance();
        let ledger = Ledger::create_null_with()
            .pending(
                &PendingKey::new(block.account_field().unwrap(), BlockHash::from(42)),
                &pending_info,
            )
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.any_pending_exists, true);
    }

    #[test]
    fn source_block_exists() {
        let source = BlockBuilder::state().with_sideband().build();
        let block = BlockBuilder::state().link(source.hash()).build();
        let ledger = Ledger::create_null_with().block(&source).build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.source_block_exists, true);
    }

    #[test]
    fn pruned_source_block_exists() {
        let block = BlockBuilder::state().link(BlockHash::from(42)).build();
        let ledger = Ledger::create_null_with()
            .pruned(&BlockHash::from(42))
            .build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.source_block_exists, true);
    }

    #[test]
    fn previous_block() {
        let previous = BlockBuilder::state().with_sideband().build();
        let block = BlockBuilder::state().previous(previous.hash()).build();
        let ledger = Ledger::create_null_with().block(&previous).build();
        let txn = ledger.read_txn();
        let validator = BlockValidatorFactory::new(&ledger, &txn, &block).create_validator();
        assert_eq!(validator.previous_block, Some(previous));
    }
}
