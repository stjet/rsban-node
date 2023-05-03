use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    BlockChainSection, BlockEnum, Epochs,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use super::{
    block_cache::BlockCache, cementation_walker::CementationWalker,
    AccountsConfirmedMapContainerInfo, BatchWriteSizeManager, CementCallbackRefs,
    CementationQueueContainerInfo, LedgerAdapter, LedgerDataRequester, WriteBatcher,
};

pub(super) struct BlockCementer {
    stopped: Arc<AtomicBool>,

    processing_timer: Instant,
    cemented_batch_timer: Instant,
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
            processing_timer: Instant::now(),
            cemented_batch_timer: Instant::now(),
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
            self.processing_timer = Instant::now();
        }

        let mut txn = self.ledger.store.tx_begin_read();
        let ledger_clone = Arc::clone(&self.ledger);
        let mut ledger_adapter = LedgerAdapter::new(txn.txn_mut(), &ledger_clone);

        //-----
        // plan: while let Some(next) = self.logic.get_next() {
        //   flush(next);
        // }

        if !self.logic.is_initialized {
            self.logic.initialize(original_block.clone());
            self.logic.is_initialized = true;
        }

        while let Some(section) = self
            .logic
            .cementation_walker
            .next_cementation(&mut ledger_adapter)
        {
            self.logic.write_batcher.enqueue(section);
            if self.logic.should_flush(
                callbacks,
                self.logic.is_done(),
                self.processing_timer.elapsed(),
            ) {
                self.try_flush(callbacks, self.logic.is_write_queue_full());
            }

            if !self.logic.is_done() {
                ledger_adapter.refresh_transaction();
            }
        }

        if self.logic.was_block_already_cemented() {
            (callbacks.block_already_cemented)(original_block.hash());
        }

        self.logic.is_initialized = false;
    }

    fn try_flush(&mut self, callbacks: &mut CementCallbackRefs, force_flush: bool) {
        let Some(mut write_guard) = self.get_write_guard(force_flush) else { return; };

        // This only writes to the confirmation_height table and is the only place to do so in a single process
        let mut txn = self.ledger.store.tx_begin_write();
        self.start_batch_timer();

        // Cement all pending entries, each entry is specific to an account and contains the least amount
        // of blocks to retain consistent cementing across all account chains to genesis.
        while let Some(section_to_cement) = self
            .logic
            .write_batcher
            .next_write(&LedgerAdapter::new(txn.txn_mut(), &self.ledger))
        {
            self.flush(
                txn.as_mut(),
                &section_to_cement,
                &mut write_guard,
                callbacks,
            );
            self.logic
                .cementation_walker
                .cementation_written(&section_to_cement.account, section_to_cement.top_height);
        }
        drop(txn);

        let unpublished_count = self.logic.write_batcher.unpublished_cemented_blocks();
        self.stop_batch_timer(unpublished_count);

        if unpublished_count > 0 {
            write_guard.release();
            self.logic
                .write_batcher
                .publish_cemented_blocks(callbacks.block_cemented);
        }

        self.processing_timer = Instant::now();
    }

    fn get_write_guard(&self, should_block: bool) -> Option<WriteGuard> {
        if should_block {
            // Block and wait until we have DB access. We must flush because the queue is full.
            Some(self.write_database_queue.wait(Writer::ConfirmationHeight))
        } else {
            // If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
            self.write_database_queue
                .try_lock(Writer::ConfirmationHeight)
        }
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if self.logic.write_batcher.has_pending_writes() {
            self.try_flush(callbacks, true);
        }
    }

    fn start_batch_timer(&mut self) {
        self.cemented_batch_timer = Instant::now();
    }

    fn stop_batch_timer(&mut self, cemented_count: usize) {
        let time_spent_cementing = self.cemented_batch_timer.elapsed();

        if time_spent_cementing > Duration::from_millis(50) {
            self.log_cemented_blocks(time_spent_cementing, cemented_count);
        }

        self.logic
            .write_batcher
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
            self.logic.write_batcher.unpublished_cemented_blocks(),
        );
        self.logic
            .write_batcher
            .batch_write_size
            .adjust_size(time_spent_cementing);
        scoped_write_guard.release();
        self.logic
            .write_batcher
            .publish_cemented_blocks(callbacks.block_cemented);

        // Only aquire transaction if there are blocks left
        if !self.logic.write_batcher.is_done() {
            *scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.start_batch_timer();
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

pub(crate) struct BlockCementerContainerInfo {
    cementation_queue: CementationQueueContainerInfo,
    accounts_confirmed: AccountsConfirmedMapContainerInfo,
}

impl BlockCementerContainerInfo {
    pub fn collect(&self) -> ContainerInfoComponent {
        ContainerInfoComponent::Composite(
            "bounded_mode".to_owned(),
            vec![
                self.cementation_queue
                    .collect("cementation_queue".to_owned()),
                self.accounts_confirmed
                    .collect("accounts_confirmed".to_owned()),
            ],
        )
    }
}

pub(crate) struct BlockCementorLogic {
    cementation_walker: CementationWalker,
    write_batcher: WriteBatcher,
    minimum_batch_separation: Duration,
    is_initialized: bool,
}

impl BlockCementorLogic {
    pub fn new(
        epochs: Epochs,
        stopped: Arc<AtomicBool>,
        minimum_batch_separation: Duration,
    ) -> Self {
        let cementation_walker = CementationWalker::builder()
            .epochs(epochs)
            .stopped(stopped)
            .build();

        Self {
            cementation_walker,
            write_batcher: Default::default(),
            minimum_batch_separation,
            is_initialized: false,
        }
    }

    pub fn initialize(&mut self, original_block: BlockEnum) {
        if !self.write_batcher.has_pending_writes() {
            self.clear_cached_accounts();
        }

        self.cementation_walker.initialize(original_block);
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.cementation_walker.block_cache()
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.write_batcher.batch_write_size
    }

    pub fn has_pending_writes(&self) -> bool {
        self.write_batcher.has_pending_writes()
    }

    pub fn clear_cached_accounts(&mut self) {
        self.cementation_walker.clear_all_cached_accounts();
    }

    pub fn is_write_queue_full(&self) -> bool {
        self.write_batcher.max_pending_writes_reached()
            || self.cementation_walker.is_accounts_cache_full()
    }

    fn is_min_processing_time_exceeded(&self, processing_time: Duration) -> bool {
        processing_time >= self.minimum_batch_separation
    }

    fn should_flush(
        &self,
        callbacks: &mut CementCallbackRefs,
        current_process_done: bool,
        processing_time: Duration,
    ) -> bool {
        let is_batch_full = self.write_batcher.max_batch_write_size_reached();

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let awaiting_processing = (callbacks.awaiting_processing_count)();
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded(processing_time));

        let should_flush = is_done_processing || is_batch_full || self.is_write_queue_full();
        should_flush && !self.write_batcher.has_pending_writes()
    }

    pub fn is_done(&self) -> bool {
        self.cementation_walker.is_done()
    }

    pub fn was_block_already_cemented(&self) -> bool {
        self.cementation_walker.num_accounts_walked() == 0
    }

    pub(crate) fn container_info(&self) -> BlockCementerContainerInfo {
        BlockCementerContainerInfo {
            cementation_queue: self.write_batcher.container_info(),
            accounts_confirmed: self.cementation_walker.container_info(),
        }
    }
}
