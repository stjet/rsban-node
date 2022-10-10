use crate::{Account, BlockHash};

use super::{DbIterator, Transaction};

/// Maps head block to owning account
/// BlockHash -> Account
pub trait FrontierStore<R, W> {
    fn put(&self, txn: &W, hash: &BlockHash, account: &Account);
    fn get(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Account;
    fn del(&self, txn: &W, hash: &BlockHash);
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<BlockHash, Account>>;

    fn begin_at_hash(
        &self,
        txn: &Transaction<R, W>,
        hash: &BlockHash,
    ) -> Box<dyn DbIterator<BlockHash, Account>>;

    fn for_each_par(
        &self,
        action: &(dyn Fn(
            R,
            &mut dyn DbIterator<BlockHash, Account>,
            &mut dyn DbIterator<BlockHash, Account>,
        ) + Send
              + Sync),
    );

    fn end(&self) -> Box<dyn DbIterator<BlockHash, Account>>;
}
