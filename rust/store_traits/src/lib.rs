mod iterator;
pub use iterator::{BinaryDbIterator, DbIterator, DbIteratorImpl};

use std::any::Any;

pub trait Transaction {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub trait ReadTransaction {
    fn txn(&self) -> &dyn Transaction;
    fn reset(&mut self);
    fn renew(&mut self);
    fn refresh(&mut self);
}

pub trait WriteTransaction {
    fn txn(&self) -> &dyn Transaction;
    fn txn_mut(&mut self) -> &mut dyn Transaction;
    fn refresh(&mut self);
    fn renew(&mut self);
    fn commit(&mut self);
}
