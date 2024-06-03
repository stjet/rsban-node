use std::ops::Deref;

use rsnano_core::{
    Account, AccountInfo, Amount, BlockEnum, BlockHash, PendingInfo, PendingKey, QualifiedRoot,
};
use rsnano_store_lmdb::{LmdbPendingStore, LmdbStore, Transaction};

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

    /// Returns the next receivable entry for the account 'account' with hash greater than 'hash'
    pub fn account_receivable_upper_bound<'txn>(
        &self,
        txn: &'txn dyn Transaction,
        account: Account,
        hash: BlockHash,
    ) -> AnyReceivableIterator<'txn>
    where
        'a: 'txn,
    {
        AnyReceivableIterator::<'txn> {
            txn,
            pending: self.store.pending.deref(),
            requested_account: account,
            actual_account: Some(account),
            next_hash: hash.inc(),
        }
    }

    /// Returns the next receivable entry for an account greater than 'account'
    pub fn receivable_upper_bound<'txn>(
        &self,
        txn: &'txn dyn Transaction,
        account: Account,
    ) -> AnyReceivableIterator<'txn>
    where
        'a: 'txn,
    {
        match account.inc() {
            None => AnyReceivableIterator::<'txn> {
                txn,
                pending: self.store.pending.deref(),
                requested_account: Default::default(),
                actual_account: None,
                next_hash: None,
            },
            Some(account) => AnyReceivableIterator::<'txn> {
                txn,
                pending: self.store.pending.deref(),
                requested_account: account,
                actual_account: None,
                next_hash: Some(BlockHash::zero()),
            },
        }
    }

    /// Returns the next receivable entry for an account greater than or equal to 'account'
    pub fn receivable_lower_bound<'txn>(
        &'a self,
        txn: &'a dyn Transaction,
        account: Account,
    ) -> AnyReceivableIterator<'txn>
    where
        'a: 'txn,
    {
        AnyReceivableIterator::<'txn> {
            txn,
            pending: self.store.pending.deref(),
            requested_account: account,
            actual_account: None,
            next_hash: Some(BlockHash::zero()),
        }
    }
}

pub struct AnyReceivableIterator<'a> {
    pub txn: &'a dyn Transaction,
    pub pending: &'a LmdbPendingStore,
    pub requested_account: Account,
    pub actual_account: Option<Account>,
    pub next_hash: Option<BlockHash>,
}

impl<'a> Iterator for AnyReceivableIterator<'a> {
    type Item = (PendingKey, PendingInfo);

    fn next(&mut self) -> Option<Self::Item> {
        let hash = self.next_hash?;
        let it = self.pending.begin_at_key(
            self.txn,
            &PendingKey::new(self.actual_account.unwrap_or(self.requested_account), hash),
        );

        let (key, info) = it.current()?;
        match self.actual_account {
            Some(account) => {
                if key.receiving_account == account {
                    self.next_hash = key.send_block_hash.inc();
                    Some((key.clone(), info.clone()))
                } else {
                    None
                }
            }
            None => {
                self.actual_account = Some(key.receiving_account);
                self.next_hash = key.send_block_hash.inc();
                Some((key.clone(), info.clone()))
            }
        }
    }
}
