use crate::{EndpointKey, NoValue};

use super::{DbIterator, Transaction};

/// Endpoints for peers
/// nano::endpoint_key -> no_value
pub trait PeerStore<R, W, IT>
where
    IT: DbIterator<EndpointKey, NoValue>,
{
    fn put(&self, txn: &W, endpoint: &EndpointKey);
    fn del(&self, txn: &W, endpoint: &EndpointKey);
    fn exists(&self, txn: &Transaction<R, W>, endpoint: &EndpointKey) -> bool;
    fn count(&self, txn: &Transaction<R, W>) -> usize;
    fn clear(&self, txn: &W);
    fn begin(&self, txn: &Transaction<R, W>) -> IT;
}
