use super::{iterator::DbIteratorImpl, DbIterator, Transaction, WriteTransaction};
use crate::core::{EndpointKey, NoValue};

pub type PeerIterator<I> = DbIterator<EndpointKey, NoValue, I>;

/// Endpoints for peers
/// nano::endpoint_key -> no_value
pub trait PeerStore<I>
where
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut dyn WriteTransaction, endpoint: &EndpointKey);
    fn del(&self, txn: &mut dyn WriteTransaction, endpoint: &EndpointKey);
    fn exists(&self, txn: &dyn Transaction, endpoint: &EndpointKey) -> bool;
    fn count(&self, txn: &dyn Transaction) -> usize;
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn begin(&self, txn: &dyn Transaction) -> PeerIterator<I>;
}
