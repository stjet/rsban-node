mod ledger;
pub mod lmdb;
mod write_database_queue;

pub use ledger::Ledger;
pub use write_database_queue::{WriteDatabaseQueue, WriteGuard, Writer};

pub trait Transaction {}

pub trait ReadTransaction: Transaction {}
