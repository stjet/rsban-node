use rsnano_core::{Account, Amount, BlockEnum, BlockHash};
use rsnano_store_lmdb::{LmdbStore, Transaction};

pub struct LedgerSetConfirmed<'a> {
    store: &'a LmdbStore,
}

impl<'a> LedgerSetConfirmed<'a> {
    pub fn new(store: &'a LmdbStore) -> Self {
        Self { store }
    }

    pub fn get_block(&self, tx: &impl Transaction, hash: &BlockHash) -> Option<BlockEnum> {
        self.store.block.get(tx, hash)
    }

    pub fn account_head(&self, tx: &impl Transaction, account: &Account) -> Option<BlockHash> {
        self.store.account.get(tx, account).map(|i| i.head)
    }

    pub fn account_height(&self, tx: &impl Transaction, account: &Account) -> u64 {
        let Some(head) = self.account_head(tx, account) else {
            return 0;
        };
        self.get_block(tx, &head)
            .map(|b| b.sideband().unwrap().height)
            .expect("Head block not in ledger!")
    }

    pub fn block_balance(&self, tx: &impl Transaction, hash: &BlockHash) -> Option<Amount> {
        if hash.is_zero() {
            return None;
        }

        self.get_block(tx, hash).map(|b| b.balance())
    }

    pub fn block_exists(&self, tx: &impl Transaction, hash: &BlockHash) -> bool {
        self.store.block.exists(tx, hash)
    }

    pub fn block_exists_or_pruned(&self, tx: &impl Transaction, hash: &BlockHash) -> bool {
        if self.store.pruned.exists(tx, hash) {
            true
        } else {
            self.store.block.exists(tx, hash)
        }
    }
}
