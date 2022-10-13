use crate::{EndpointKey, NoValue};

use super::{iterator::DbIteratorImpl, DbIterator, Transaction};

pub type PeerIterator<I> = DbIterator<EndpointKey, NoValue, I>;

/// Endpoints for peers
/// nano::endpoint_key -> no_value
pub trait PeerStore<'a, R, W, I>
where
    R: 'a,
    W: 'a,
    I: DbIteratorImpl,
{
    fn put(&self, txn: &mut W, endpoint: &EndpointKey);
    fn del(&self, txn: &mut W, endpoint: &EndpointKey);
    fn exists(&self, txn: &Transaction<R, W>, endpoint: &EndpointKey) -> bool;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &mut W);
    fn begin(&self, txn: &Transaction<R, W>) -> PeerIterator<I>;
}
