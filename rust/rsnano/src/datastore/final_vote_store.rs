use super::WriteTransaction;
use crate::{BlockHash, QualifiedRoot};

pub trait FinalVoteStore {
    fn put(&self, txn: &dyn WriteTransaction, root: &QualifiedRoot, hash: &BlockHash) -> bool;
}
