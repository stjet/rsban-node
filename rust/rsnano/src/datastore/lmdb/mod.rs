use std::sync::Arc;

pub struct LmdbReadTransaction {
    txn_id: u64,
    callbacks: Arc<dyn TxnCallbacks>,
}

impl LmdbReadTransaction {
    pub fn new(txn_id: u64, callbacks: Arc<dyn TxnCallbacks>) -> Self {
        Self { txn_id, callbacks }
    }
}

pub trait TxnCallbacks {
    fn txn_start(&self, txn_id: u64, is_write: bool);
    fn txn_end(&self, txn_id: u64);
}
