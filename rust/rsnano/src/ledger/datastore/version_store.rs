use super::Transaction;

pub trait VersionStore<R, W> {
    fn put(&self, txn: &mut W, version: i32);
    fn get(&self, txn: &Transaction<R, W>) -> i32;
}
