use std::{collections::VecDeque, sync::Arc, time::Instant};

use bounded_vec_deque::BoundedVecDeque;
use rsnano_core::{Account, BlockHash};
use rsnano_ledger::{WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

pub struct ConfirmationHeightBounded {
    write_database_queue: Arc<WriteDatabaseQueue>,
    pub pending_writes: VecDeque<WriteDetails>,
}

impl ConfirmationHeightBounded {
    pub fn new(write_database_queue: Arc<WriteDatabaseQueue>) -> Self {
        Self {
            write_database_queue,
            pending_writes: VecDeque::new(),
        }
    }

    pub fn cement_blocks(
        &self,
        _timer: Instant,
        txn: &mut dyn WriteTransaction,
        last_iteration: bool,
    ) -> (Instant, Option<WriteGuard>) {
        let mut scoped_write_guard = None;
        // Only aquire transaction if there are blocks left
        if !(last_iteration && self.pending_writes.len() == 1) {
            scoped_write_guard = Some(self.write_database_queue.wait(Writer::ConfirmationHeight));
            txn.renew();
        }

        (Instant::now(), scoped_write_guard)
    }
}

pub struct WriteDetails {
    pub account: Account,
    // This is the first block hash (bottom most) which is not cemented
    pub bottom_height: u64,
    pub bottom_hash: BlockHash,
    // Desired cemented frontier
    pub top_height: u64,
    pub top_hash: BlockHash,
}

pub fn truncate_after(buffer: &mut BoundedVecDeque<BlockHash>, hash: &BlockHash) {
    if let Some((index, _)) = buffer.iter().enumerate().find(|(_, h)| *h != hash) {
        buffer.truncate(index);
    }
}
