use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use rsnano_core::{utils::Logger, BlockEnum};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};

use super::{
    BatchWriteSizeManager, BlockCache, BlockCementerContainerInfo, BlockCementorLogic,
    CementCallbackRefs, FlushDecision, LedgerAdapter, LedgerDataRequester,
};

pub(super) struct BlockCementer {
    stopped: Arc<AtomicBool>,

    processing_started: Instant,
    write_txn_started: Instant,
    write_database_queue: Arc<WriteDatabaseQueue>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    ledger: Arc<Ledger>,
    logic: BlockCementorLogic,
}

impl BlockCementer {
    pub fn new(
        ledger: Arc<Ledger>,
        write_database_queue: Arc<WriteDatabaseQueue>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        minimum_batch_separation: Duration,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        let logic = BlockCementorLogic::new(
            ledger.constants.epochs.clone(),
            stopped.clone(),
            minimum_batch_separation,
        );

        Self {
            write_database_queue,
            logger,
            enable_timing_logging,
            ledger,
            stopped,
            processing_started: Instant::now(),
            write_txn_started: Instant::now(),
            logic,
        }
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        self.logic.batch_write_size()
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.logic.block_cache()
    }

    pub(crate) fn process(
        &mut self,
        original_block: &BlockEnum,
        callbacks: &mut CementCallbackRefs,
    ) {
        if !self.logic.has_pending_writes() {
            self.processing_started = Instant::now();
        }

        let mut txn = self.ledger.store.tx_begin_read();
        let ledger_clone = Arc::clone(&self.ledger);
        let mut ledger_adapter = LedgerAdapter::new(txn.txn_mut(), &ledger_clone);

        self.logic.enqueue_block(original_block.clone());

        while self
            .logic
            .process_current_block(&mut ledger_adapter, callbacks)
        {
            let awaiting_processing = (callbacks.awaiting_processing_count)();
            match self
                .logic
                .get_flush_decision(awaiting_processing, self.processing_started.elapsed())
            {
                FlushDecision::DontFlush => {}
                FlushDecision::TryFlush(has_more) => {
                    if self.try_flush(callbacks) && has_more {
                        ledger_adapter.refresh_transaction();
                    }
                }
                FlushDecision::ForceFlush(has_more) => {
                    self.force_flush(callbacks);
                    if has_more {
                        ledger_adapter.refresh_transaction();
                    }
                }
            }
        }
    }

    /// If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
    fn try_flush(&mut self, callbacks: &mut CementCallbackRefs) -> bool {
        if let Some(write_guard) = self
            .write_database_queue
            .try_lock(Writer::ConfirmationHeight)
        {
            self.flush(write_guard, callbacks);
            true
        } else {
            false
        }
    }

    fn force_flush(&mut self, callbacks: &mut CementCallbackRefs) {
        let write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
        self.flush(write_guard, callbacks);
    }

    /// This only writes to the confirmation_height table and is the only place to do so in a single process
    fn flush(&mut self, mut write_guard: WriteGuard, callbacks: &mut CementCallbackRefs) {
        let mut txn = self.ledger.store.tx_begin_write();
        self.write_txn_started = Instant::now();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while let Some(section_to_cement) = self
            .logic
            .next_write(&LedgerAdapter::new(txn.txn_mut(), &self.ledger))
        {
            self.ledger
                .write_confirmation_height(txn.as_mut(), &section_to_cement);
            self.logic.section_cemented(&section_to_cement);

            if self.logic.should_start_new_write_batch() {
                //todo remove duplication!
                txn.commit();
                write_guard.release();
                let time_spent_cementing = self.write_txn_started.elapsed();
                self.log_cemented_blocks(
                    time_spent_cementing,
                    self.logic.unpublished_cemented_blocks_len(),
                );
                self.logic.batch_completed(time_spent_cementing, callbacks);

                write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
                txn.renew();
                self.write_txn_started = Instant::now();
            }
        }

        //todo remove duplication!
        txn.commit();
        write_guard.release();
        let time_spent_cementing = self.write_txn_started.elapsed();
        self.log_cemented_blocks(
            time_spent_cementing,
            self.logic.unpublished_cemented_blocks_len(),
        );
        self.logic.batch_completed(time_spent_cementing, callbacks);
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if self.logic.has_pending_writes() {
            self.force_flush(callbacks);
        }
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration, cemented_count: usize) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms",
                cemented_count,
                time_spent_cementing.as_millis()
            ));
        }
    }

    pub fn clear_process_vars(&mut self) {
        self.logic.clear_cached_accounts();
    }

    pub fn has_pending_writes(&self) -> bool {
        self.logic.has_pending_writes()
    }

    pub fn container_info(&self) -> BlockCementerContainerInfo {
        self.logic.container_info()
    }
}
