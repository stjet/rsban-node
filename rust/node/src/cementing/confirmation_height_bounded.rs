use std::{sync::Arc, time::Instant};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::BlockHash;
use rsnano_ledger::WriteDatabaseQueue;
use rsnano_store_traits::WriteTransaction;

pub struct ConfirmationHeightBounded {
    write_database_queue: Arc<WriteDatabaseQueue>,
}

impl ConfirmationHeightBounded {
    pub fn new(write_database_queue: Arc<WriteDatabaseQueue>) -> Self {
        Self {
            write_database_queue,
        }
    }

    pub fn cement_blocks(&self, _timer: Instant, _txn: &mut dyn WriteTransaction) -> Instant {
        Instant::now()
    }
}

pub fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}
