use rsnano_core::{Amount, Block, BlockType};
use rsnano_store_traits::WriteTransaction;

use crate::{Ledger, LegacyBlockValidator, ProcessResult};

pub(crate) struct LegacyBlockProcessor<'a> {
    ledger: &'a Ledger,
    txn: &'a mut dyn WriteTransaction,
    block: &'a mut dyn Block,
}

impl<'a> LegacyBlockProcessor<'a> {
    pub(crate) fn new(
        ledger: &'a Ledger,
        txn: &'a mut dyn WriteTransaction,
        block: &'a mut dyn Block,
    ) -> Self {
        Self { block, ledger, txn }
    }

    pub(crate) fn process(&'a mut self) -> Result<(), ProcessResult> {
        let validation =
            LegacyBlockValidator::new(self.ledger, self.txn.txn(), self.block).validate()?;

        if let Some(key) = &validation.pending_received {
            self.ledger.store.pending().del(self.txn, key);
        }

        self.block.set_sideband(validation.new_sideband);

        self.ledger
            .store
            .block()
            .put(self.txn, &self.block.hash(), self.block);

        self.ledger.update_account(
            self.txn,
            &validation.account,
            &validation.old_account_info,
            &validation.new_account_info,
        );

        if !validation.amount_received.is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                validation.new_account_info.representative,
                validation.amount_received,
            );
        } else if !validation.amount_sent.is_zero() {
            self.ledger.cache.rep_weights.representation_add(
                validation.old_account_info.representative,
                Amount::zero().wrapping_sub(validation.amount_sent),
            );
        } else {
            self.ledger.cache.rep_weights.representation_add_dual(
                validation.new_account_info.representative,
                validation.new_account_info.balance,
                validation.old_account_info.representative,
                Amount::zero().wrapping_sub(validation.new_account_info.balance),
            );
        }

        if let Some((key, info)) = validation.new_pending {
            self.ledger.store.pending().put(self.txn, &key, &info);
        }

        if self.block.block_type() != BlockType::Open {
            self.ledger
                .store
                .frontier()
                .del(self.txn, &self.block.previous());
        }

        self.ledger
            .store
            .frontier()
            .put(self.txn, &self.block.hash(), &validation.account);

        self.ledger.observer.block_added(self.block, false);
        Ok(())
    }
}
