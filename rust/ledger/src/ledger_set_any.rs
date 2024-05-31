use rsnano_core::{
    Account, AccountInfo, Amount, BlockEnum, BlockHash, PendingInfo, PendingKey, QualifiedRoot,
};
use rsnano_store_lmdb::{LmdbStore, Transaction};

pub struct LedgerSetAny<'a> {
    store: &'a LmdbStore,
}

impl<'a> LedgerSetAny<'a> {
    pub fn new(store: &'a LmdbStore) -> Self {
        Self { store }
    }

    pub fn get_block(&self, tx: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum> {
        self.store.block.get(tx, hash)
    }

    pub fn get_account(&self, tx: &dyn Transaction, account: &Account) -> Option<AccountInfo> {
        self.store.account.get(tx, account)
    }

    pub fn account_head(&self, tx: &dyn Transaction, account: &Account) -> Option<BlockHash> {
        self.get_account(tx, account).map(|i| i.head)
    }

    pub fn account_balance(&self, tx: &dyn Transaction, account: &Account) -> Option<Amount> {
        let head = self.account_head(tx, account)?;
        self.get_block(tx, &head).map(|b| b.balance())
    }

    pub fn account_height(&self, tx: &dyn Transaction, account: &Account) -> u64 {
        let Some(head) = self.account_head(tx, account) else {
            return 0;
        };
        self.get_block(tx, &head)
            .map(|b| b.sideband().unwrap().height)
            .expect("Head block not in ledger!")
    }

    pub fn block_account(&self, tx: &dyn Transaction, hash: &BlockHash) -> Option<Account> {
        self.get_block(tx, hash).map(|b| b.account())
    }

    pub fn block_amount(&self, tx: &dyn Transaction, hash: &BlockHash) -> Option<Amount> {
        let block = self.get_block(tx, hash)?;
        let block_balance = block.balance();
        if block.previous().is_zero() {
            Some(block_balance)
        } else {
            let previous_balance = self.block_balance(tx, &block.previous())?;
            if block_balance > previous_balance {
                Some(block_balance - previous_balance)
            } else {
                Some(previous_balance - block_balance)
            }
        }
    }

    pub fn block_balance(&self, tx: &dyn Transaction, hash: &BlockHash) -> Option<Amount> {
        if hash.is_zero() {
            return None;
        }

        self.get_block(tx, hash).map(|b| b.balance())
    }

    pub fn block_exists(&self, tx: &dyn Transaction, hash: &BlockHash) -> bool {
        self.store.block.exists(tx, hash)
    }

    pub fn block_exists_or_pruned(&self, tx: &dyn Transaction, hash: &BlockHash) -> bool {
        if self.store.pruned.exists(tx, hash) {
            true
        } else {
            self.store.block.exists(tx, hash)
        }
    }

    pub fn block_height(&self, tx: &dyn Transaction, hash: &BlockHash) -> u64 {
        self.get_block(tx, hash)
            .map(|b| b.sideband().unwrap().height)
            .unwrap_or_default()
    }

    pub fn block_successor(&self, tx: &dyn Transaction, hash: &BlockHash) -> Option<BlockHash> {
        self.block_successor_by_qualified_root(tx, &QualifiedRoot::new(hash.into(), *hash))
    }

    pub fn block_successor_by_qualified_root(
        &self,
        tx: &dyn Transaction,
        root: &QualifiedRoot,
    ) -> Option<BlockHash> {
        if !root.previous.is_zero() {
            self.store.block.successor(tx, &root.previous)
        } else {
            self.get_account(tx, &root.root.into())
                .map(|i| i.open_block)
        }
    }

    pub fn get_pending(&self, tx: &dyn Transaction, key: &PendingKey) -> Option<PendingInfo> {
        self.store.pending.get(tx, key)
    }
}
