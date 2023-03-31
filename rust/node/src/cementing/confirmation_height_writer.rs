use std::{
    cmp::max,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use rsnano_core::UpdateConfirmationHeight;
use rsnano_ledger::Ledger;
use rsnano_store_traits::WriteTransaction;

use crate::stats::{DetailType, Direction, StatType, Stats};

use super::write_details_queue::WriteDetailsQueue;

pub(crate) struct ConfirmationHeightWriter<'a> {
    pub cemented_batch_timer: Instant,
    pub pending_writes: &'a mut WriteDetailsQueue,
    ledger: &'a Ledger,
    stats: &'a Stats,
    batch_write_size: &'a AtomicUsize,
}

impl<'a> ConfirmationHeightWriter<'a> {
    pub(crate) const MINIMUM_BATCH_WRITE_SIZE: usize = 16384;

    pub fn new(
        pending_writes: &'a mut WriteDetailsQueue,
        ledger: &'a Ledger,
        stats: &'a Stats,
        batch_write_size: &'a AtomicUsize,
    ) -> Self {
        Self {
            cemented_batch_timer: Instant::now(),
            pending_writes,
            ledger,
            stats,
            batch_write_size,
        }
    }

    pub fn reset_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    pub fn write_confirmation_height(
        &self,
        txn: &mut dyn WriteTransaction,
        update_confirmation_height: &UpdateConfirmationHeight,
    ) {
        self.ledger
            .write_confirmation_height(txn, &update_confirmation_height);

        self.stats.add(
            StatType::ConfirmationHeight,
            DetailType::BlocksConfirmedBounded,
            Direction::In,
            update_confirmation_height.num_blocks_cemented,
            false,
        );
    }

    pub fn increase_batch_write_size(&self, amount_to_change: usize) {
        self.batch_write_size
            .fetch_add(amount_to_change, Ordering::SeqCst);
    }

    pub fn reduce_batch_write_size(&self, amount_to_change: usize) {
        // Reduce (unless we have hit a floor)
        self.batch_write_size.store(
            max(
                ConfirmationHeightWriter::MINIMUM_BATCH_WRITE_SIZE,
                self.batch_write_size.load(Ordering::SeqCst) - amount_to_change,
            ),
            Ordering::SeqCst,
        );
    }
}
