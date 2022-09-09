use crate::{Account, Block, BlockEnum, BlockHash, BlockWithSideband};

use super::{DbIterator, Transaction, WriteTransaction};

pub trait BlockStore {
    fn put(&self, txn: &dyn WriteTransaction, hash: &BlockHash, block: &dyn Block);
    fn exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool;
    fn successor(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockHash;
    fn successor_clear(&self, txn: &dyn WriteTransaction, hash: &BlockHash);
    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum>;
    fn get_no_sideband(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum>;
    fn del(&self, txn: &dyn WriteTransaction, hash: &BlockHash);
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn account_calculated(&self, block: &dyn Block) -> Account;
    fn account(&self, txn: &dyn Transaction, hash: &BlockHash) -> Account;
    fn begin(
        &self,
        transaction: &dyn Transaction,
    ) -> Box<dyn DbIterator<BlockHash, BlockWithSideband>>;
}
