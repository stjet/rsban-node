use crate::core::{Account, Amount, Block, BlockEnum, BlockHash, BlockWithSideband, Epoch};

use super::{iterator::DbIteratorImpl, DbIterator, Transaction};

pub type BlockIterator<I> = DbIterator<BlockHash, BlockWithSideband, I>;

pub trait BlockStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, hash: &BlockHash, block: &dyn Block);
    fn exists(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> bool;
    fn successor(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> BlockHash;
    fn successor_clear(&self, txn: &mut W, hash: &BlockHash);
    fn get(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Option<BlockEnum>;
    fn get_no_sideband(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Option<BlockEnum>;
    fn del(&self, txn: &mut W, hash: &BlockHash);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn account_calculated(&self, block: &dyn Block) -> Account;
    fn account(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Account;
    fn begin(&self, txn: &Transaction<R, W>) -> BlockIterator<I>;
    fn begin_at_hash(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> BlockIterator<I>;
    fn end(&self) -> BlockIterator<I>;
    fn random(&self, txn: &Transaction<R, W>) -> Option<BlockEnum>;
    fn balance(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Amount;
    fn balance_calculated(&self, block: &BlockEnum) -> Amount;
    fn version(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Epoch;
    fn for_each_par(
        &'a self,
        action: &(dyn Fn(R, BlockIterator<I>, BlockIterator<I>) + Send + Sync),
    );
    fn account_height(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> u64;
}
