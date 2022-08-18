mod ledger;
pub mod lmdb;
mod write_database_queue;

use std::any::Any;

pub use ledger::Ledger;
pub use write_database_queue::{WriteDatabaseQueue, WriteGuard, Writer};

pub trait Transaction {
    fn as_any(&self) -> &(dyn Any + '_);
}

pub trait ReadTransaction: Transaction {}

pub trait WriteTransaction: Transaction {
    fn as_transaction(&self) -> &dyn Transaction;
}
