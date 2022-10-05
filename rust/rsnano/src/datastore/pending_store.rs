use crate::{Account, PendingInfo, PendingKey};

use super::{DbIterator, Transaction};

/// Maps (destination account, pending block) to (source account, amount, version).
/// nano::account, nano::block_hash -> nano::account, nano::amount, nano::epoch
pub trait PendingStore<R, W> {
    fn put(&self, txn: &W, key: &PendingKey, pending: &PendingInfo);
    fn del(&self, txn: &W, key: &PendingKey);
    fn get(&self, txn: &Transaction<R, W>, key: &PendingKey) -> Option<PendingInfo>;
    fn begin(&self, txn: &Transaction<R, W>) -> Box<dyn DbIterator<PendingKey, PendingInfo>>;
    fn begin_at_key(
        &self,
        txn: &Transaction<R, W>,
        key: &PendingKey,
    ) -> Box<dyn DbIterator<PendingKey, PendingInfo>>;
    fn end(&self) -> Box<dyn DbIterator<PendingKey, PendingInfo>>;
    fn exists(&self, txn: &Transaction<R, W>, key: &PendingKey) -> bool;
    fn any(&self, txn: &Transaction<R, W>, account: &Account) -> bool;
    fn for_each_par(
        &self,
        action: &(dyn Fn(
            &R,
            &mut dyn DbIterator<PendingKey, PendingInfo>,
            &mut dyn DbIterator<PendingKey, PendingInfo>,
        ) + Send
              + Sync),
    );
}
