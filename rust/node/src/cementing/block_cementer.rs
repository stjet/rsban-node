use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    BlockChainSection, BlockEnum,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use super::{
    block_cache::BlockCache, cementation_walker::CementationWalker,
    AccountsConfirmedMapContainerInfo, BatchWriteSizeManager, CementCallbackRefs, LedgerAdapter,
    LedgerDataRequester, WriteBatcher, WriteDetailsContainerInfo,
};

pub(super) struct BlockCementer {
    stopped: Arc<AtomicBool>,
    batch_separate_pending_min_time: Duration,
    write_batcher: WriteBatcher,

    processing_timer: Instant,
    cemented_batch_timer: Instant,
    write_database_queue: Arc<WriteDatabaseQueue>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    ledger: Arc<Ledger>,
    cementation_walker: CementationWalker,
}

impl BlockCementer {
    pub fn new(
        ledger: Arc<Ledger>,
        write_database_queue: Arc<WriteDatabaseQueue>,
        logger: Arc<dyn Logger>,
        enable_timing_logging: bool,
        batch_separate_pending_min_time: Duration,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        let helper = CementationWalker::builder()
            .epochs(ledger.constants.epochs.clone())
            .stopped(stopped.clone())
            .build();

        Self {
            write_database_queue,
            logger,
            enable_timing_logging,
            ledger,
            stopped,
            processing_timer: Instant::now(),
            batch_separate_pending_min_time,
            cemented_batch_timer: Instant::now(),
            write_batcher: WriteBatcher::default(),
            cementation_walker: helper,
        }
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.write_batcher.batch_write_size
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.cementation_walker.block_cache()
    }

    pub(crate) fn process(
        &mut self,
        original_block: &BlockEnum,
        callbacks: &mut CementCallbackRefs,
    ) {
        if !self.has_pending_writes() {
            self.clear_process_vars();
            self.processing_timer = Instant::now();
        }

        self.cementation_walker.initialize(original_block.clone());

        let mut txn = self.ledger.store.tx_begin_read();
        let ledger_clone = Arc::clone(&self.ledger);

        let mut ledger_adapter = LedgerAdapter::new(txn.txn_mut(), &ledger_clone);

        while let Some(section) = self
            .cementation_walker
            .next_cementation(&mut ledger_adapter)
        {
            self.write_batcher.enqueue(section);
            if self.should_flush(callbacks, self.cementation_walker.is_done()) {
                self.try_flush(callbacks);
            }

            if !self.cementation_walker.is_done() {
                ledger_adapter.refresh_transaction();
            }
        }

        if self.cementation_walker.num_accounts_walked() == 0 {
            (callbacks.block_already_cemented)(original_block.hash());
        }
    }

    fn should_flush(&self, callbacks: &mut CementCallbackRefs, current_process_done: bool) -> bool {
        let is_batch_full = self.write_batcher.max_batch_write_size_reached();

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let awaiting_processing = (callbacks.awaiting_processing_count)();
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded());

        let should_flush = is_done_processing || is_batch_full || self.is_write_queue_full();
        should_flush && !self.write_batcher.has_pending_writes()
    }

    fn is_write_queue_full(&self) -> bool {
        self.write_batcher.max_pending_writes_reached()
            || self.cementation_walker.is_accounts_cache_full()
    }

    fn try_flush(&mut self, callbacks: &mut CementCallbackRefs) {
        // If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
        if let Some(mut write_guard) = self
            .write_database_queue
            .try_lock(Writer::ConfirmationHeight)
        {
            self.write_pending_blocks_with_write_guard(&mut write_guard, callbacks);
        } else if self.is_write_queue_full() {
            // Block and wait until we have DB access. We must flush because the queue is full.
            let mut write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            self.write_pending_blocks_with_write_guard(&mut write_guard, callbacks);
        }
    }

    fn is_min_processing_time_exceeded(&self) -> bool {
        self.processing_timer.elapsed() >= self.batch_separate_pending_min_time
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if !self.write_batcher.has_pending_writes() {
            return;
        }

        let mut write_guard = self
            .write_database_queue
            .wait(rsnano_ledger::Writer::ConfirmationHeight);

        self.write_pending_blocks_with_write_guard(&mut write_guard, callbacks);
    }

    fn write_pending_blocks_with_write_guard(
        &mut self,
        scoped_write_guard: &mut WriteGuard,
        callbacks: &mut CementCallbackRefs,
    ) {
        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();

        self.start_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while let Some(section_to_cement) = self
            .write_batcher
            .next_write(&LedgerAdapter::new(txn.txn_mut(), &self.ledger))
            .unwrap()
        {
            self.flush(
                txn.as_mut(),
                &section_to_cement,
                scoped_write_guard,
                callbacks,
            );
            if self.write_batcher.is_done() {
                self.cementation_walker
                    .clear_cached_account(&section_to_cement.account, section_to_cement.top_height);
            }
        }
        drop(txn);

        let unpublished_count = self.write_batcher.unpublished_cemented_blocks();
        self.stop_batch_timer(unpublished_count);

        if unpublished_count > 0 {
            scoped_write_guard.release();
            self.write_batcher
                .publish_cemented_blocks(callbacks.block_cemented);
        }

        self.processing_timer = Instant::now();
    }

    fn start_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    fn stop_batch_timer(&mut self, cemented_count: usize) {
        let time_spent_cementing = self.cemented_batch_timer.elapsed();

        if time_spent_cementing > Duration::from_millis(50) {
            self.log_cemented_blocks(time_spent_cementing, cemented_count);
        }

        self.write_batcher
            .batch_write_size
            .adjust_size(time_spent_cementing);
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration, cemented_count: usize) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
                cemented_count,
                time_spent_cementing.as_millis()
            ));
        }
    }

    fn flush(
        &mut self,
        txn: &mut dyn WriteTransaction,
        update: &BlockChainSection,
        scoped_write_guard: &mut WriteGuard,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.ledger.write_confirmation_height(txn, update);

        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        txn.commit();

        self.log_cemented_blocks(
            time_spent_cementing,
            self.write_batcher.unpublished_cemented_blocks(),
        );
        self.write_batcher
            .batch_write_size
            .adjust_size(time_spent_cementing);
        scoped_write_guard.release();
        self.write_batcher
            .publish_cemented_blocks(callbacks.block_cemented);

        // Only aquire transaction if there are blocks left
        if !self.write_batcher.is_done() {
            *scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.start_batch_timer();
    }

    pub fn clear_process_vars(&mut self) {
        self.cementation_walker.clear_all_cached_accounts();
    }

    pub fn has_pending_writes(&self) -> bool {
        self.write_batcher.has_pending_writes()
    }

    pub fn container_info(&self) -> BoundedModeContainerInfo {
        BoundedModeContainerInfo {
            pending_writes: self.write_batcher.container_info(),
            accounts_confirmed: self.cementation_walker.container_info(),
        }
    }
}

pub(super) struct BoundedModeContainerInfo {
    pending_writes: WriteDetailsContainerInfo,
    accounts_confirmed: AccountsConfirmedMapContainerInfo,
}

impl BoundedModeContainerInfo {
    pub fn collect(&self) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            "bounded_mode".to_owned(),
            vec![
                self.pending_writes.collect("pending_writes".to_owned()),
                self.accounts_confirmed
                    .collect("accounts_confirmed".to_owned()),
            ],
        )
    }
}
