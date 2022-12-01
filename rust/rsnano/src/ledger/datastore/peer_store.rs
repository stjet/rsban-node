use rsnano_core::EndpointKey;

use super::{DbIterator, Transaction, WriteTransaction};
use crate::core::NoValue;

pub type PeerIterator = Box<dyn DbIterator<EndpointKey, NoValue>>;

/// Endpoints for peers
/// nano::endpoint_key -> no_value
pub trait PeerStore {
    fn put(&self, txn: &mut dyn WriteTransaction, endpoint: &EndpointKey);
    fn del(&self, txn: &mut dyn WriteTransaction, endpoint: &EndpointKey);
    fn exists(&self, txn: &dyn Transaction, endpoint: &EndpointKey) -> bool;
    fn count(&self, txn: &dyn Transaction) -> u64;
    fn clear(&self, txn: &mut dyn WriteTransaction);
    fn begin(&self, txn: &dyn Transaction) -> PeerIterator;
}
