use rsnano_core::{Account, Amount, BlockEnum, BlockHash, BlockWithSideband, Epoch};

use crate::{DbIterator, ReadTransaction, Transaction, WriteTransaction};

pub type BlockIterator = Box<dyn DbIterator<BlockHash, BlockWithSideband>>;

pub trait BlockStore {
    fn put(&self, txn: &mut dyn WriteTransaction, block: &BlockEnum);
    fn exists(&self, txn: &dyn Transaction, hash: &BlockHash) -> bool;
    fn successor(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockHash>;
    fn successor_clear(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn get(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum>;
    fn get_no_sideband(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<BlockEnum>;
    fn del(&self, txn: &mut dyn WriteTransaction, hash: &BlockHash);
    fn count(&self, txn: &dyn Transaction) -> u64;
    fn account(&self, txn: &dyn Transaction, hash: &BlockHash) -> Option<Account>;
    fn begin(&self, txn: &dyn Transaction) -> BlockIterator;
    fn begin_at_hash(&self, txn: &dyn Transaction, hash: &BlockHash) -> BlockIterator;
    fn end(&self) -> BlockIterator;
    fn random(&self, txn: &dyn Transaction) -> Option<BlockEnum>;
    fn balance(&self, txn: &dyn Transaction, hash: &BlockHash) -> Amount;
    fn version(&self, txn: &dyn Transaction, hash: &BlockHash) -> Epoch;
    fn for_each_par(
        &self,
        action: &(dyn Fn(&dyn ReadTransaction, BlockIterator, BlockIterator) + Send + Sync),
    );
    fn account_height(&self, txn: &dyn Transaction, hash: &BlockHash) -> u64;
}
