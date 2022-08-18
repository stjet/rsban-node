mod account_store;
mod ledger;
pub mod lmdb;
mod write_database_queue;

use std::any::Any;

pub use account_store::AccountStore;
pub use ledger::Ledger;
pub use write_database_queue::{WriteDatabaseQueue, WriteGuard, Writer};

use self::lmdb::LmdbRawIterator;

pub trait Transaction {
    fn as_any(&self) -> &(dyn Any + '_);
}

pub trait ReadTransaction: Transaction {}

pub trait WriteTransaction: Transaction {
    fn as_transaction(&self) -> &dyn Transaction;
}

pub trait DbIterator<K, V> {
    fn take_lmdb_raw_iterator(&mut self) -> LmdbRawIterator;
}
