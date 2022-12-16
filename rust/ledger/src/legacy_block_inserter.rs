use rsnano_core::{Amount, Block, BlockType};
use rsnano_store_traits::WriteTransaction;

use crate::{legacy_block_validator::BlockValidation, Ledger};

pub(crate) struct LegacyBlockInserter<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
    validation: &'a BlockValidation,
}

impl<'a> LegacyBlockInserter<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut dyn Block,
        validation: &'a BlockValidation,
    ) -> Self {
        Self {
            ledger,
            txn,
            block,
            validation,
        }
    }

    pub(crate) fn insert(&mut self) {
        self.delete_received_pending_entry();
        self.set_sideband();

        self.ledger.store.block().put(self.txn, self.block);

        self.update_account();
        self.update_representative_cache();
        self.insert_pending_receive();
        self.delete_old_frontier();
        self.insert_frontier();
        self.ledger.observer.block_added(self.block, false);
    }

    fn set_sideband(&mut self) {
        self.block
            .set_sideband(self.validation.new_sideband.clone());
    }

    fn insert_frontier(&mut self) {
        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &self.validation.account);
    }

    fn delete_old_frontier(&mut self) {
        if self.block.block_type() != BlockType::Open {
            self.ledger
                .store
                .frontier()
                .del(self.txn, &self.block.previous());
        }
    }

    fn insert_pending_receive(&mut self) {
        if let Some((key, info)) = &self.validation.new_pending {
            self.ledger.store.pending().put(self.txn, key, info);
        }
    }

    fn update_account(&mut self) {
        self.ledger.update_account(
            self.txn,
            &self.validation.account,
            &self.validation.old_account_info,
            &self.validation.new_account_info,
        );
    }

    fn update_representative_cache(&mut self) {
        if !self.validation.amount_received().is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                self.validation.new_account_info.representative,
                self.validation.amount_received(),
            );
        } else if !self.validation.amount_sent().is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                self.validation.old_account_info.representative,
                Amount::zero().wrapping_sub(self.validation.amount_sent()),
            );
        } else {
            self.ledger.cache.rep_weights.representation_add_dual(
                self.validation.new_account_info.representative,
                self.validation.new_account_info.balance,
                self.validation.old_account_info.representative,
                Amount::zero().wrapping_sub(self.validation.new_account_info.balance),
            );
        }
    }

    fn delete_received_pending_entry(&mut self) {
        if let Some(key) = &self.validation.pending_received {
            self.ledger.store.pending().del(self.txn, key);
        }
    }
}
