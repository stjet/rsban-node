use crate::{Account, Amount, Block, BlockEnum, BlockHash, BlockWithSideband, Epoch};

use super::{iterator::DbIteratorImpl, DbIterator, Transaction};

pub trait BlockStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &W, hash: &BlockHash, block: &dyn Block);
    fn exists(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> bool;
    fn successor(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> BlockHash;
    fn successor_clear(&self, txn: &W, hash: &BlockHash);
    fn get(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Option<BlockEnum>;
    fn get_no_sideband(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Option<BlockEnum>;
    fn del(&self, txn: &W, hash: &BlockHash);
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn account_calculated(&self, block: &dyn Block) -> Account;
    fn account(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Account;
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<BlockHash, BlockWithSideband>>;
    fn begin_at_hash(
        &self,
        txn: &Transaction<R, W>,
        hash: &BlockHash,
    ) -> Box<dyn DbIterator<BlockHash, BlockWithSideband>>;
    fn end(&self) -> Box<dyn DbIterator<BlockHash, BlockWithSideband>>;
    fn random(&self, txn: &Transaction<R, W>) -> Option<BlockEnum>;
    fn balance(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Amount;
    fn balance_calculated(&self, block: &BlockEnum) -> Amount;
    fn version(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> Epoch;
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            R,
            &mut dyn DbIterator<BlockHash, BlockWithSideband>,
            &mut dyn DbIterator<BlockHash, BlockWithSideband>,
        ) + Send
              + Sync),
    );
    fn account_height(&self, txn: &Transaction<R, W>, hash: &BlockHash) -> u64;
}
