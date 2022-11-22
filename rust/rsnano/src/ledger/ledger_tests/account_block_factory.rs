use crate::{
    core::{
        Account, AccountInfo, Amount, BlockBuilder, ChangeBlockBuilder, Epoch, KeyPair,
        StateBlockBuilder,
    },
    ledger::{datastore::Transaction, Ledger, DEV_GENESIS_KEY},
    DEV_CONSTANTS,
};

/// Test helper that creates blocks for a single account
pub(crate) struct AccountBlockFactory<'a> {
    key: KeyPair,
    ledger: &'a Ledger,
}

impl<'a> AccountBlockFactory<'a> {
    pub(crate) fn account(&self) -> Account {
        self.key.public_key().into()
    }

    pub(crate) fn info(&self, txn: &dyn Transaction) -> Option<AccountInfo> {
        self.ledger.store.account().get(txn, &self.account())
    }

    pub(crate) fn genesis(ledger: &'a Ledger) -> Self {
        Self {
            key: DEV_GENESIS_KEY.clone(),
            ledger,
        }
    }

    pub(crate) fn epoch_v1(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(info.representative)
            .balance(info.balance)
            .link(*DEV_CONSTANTS.epochs.link(Epoch::Epoch1).unwrap())
            .sign(&self.key)
    }

    pub(crate) fn epoch_v2(&self, txn: &dyn Transaction) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(info.representative)
            .balance(info.balance)
            .link(*DEV_CONSTANTS.epochs.link(Epoch::Epoch2).unwrap())
            .sign(&self.key)
    }

    pub(crate) fn change_representative(
        &self,
        txn: &dyn Transaction,
        representative: Account,
    ) -> ChangeBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::change()
            .previous(info.head)
            .representative(representative)
            .sign(&self.key)
    }

    pub(crate) fn state_send(
        &self,
        txn: &dyn Transaction,
        destination: Account,
        amount: Amount,
    ) -> StateBlockBuilder {
        let info = self.info(txn).unwrap();
        BlockBuilder::state()
            .account(self.account())
            .previous(info.head)
            .representative(info.representative)
            .balance(info.balance - amount)
            .link(destination)
            .sign(&self.key)
    }
}
