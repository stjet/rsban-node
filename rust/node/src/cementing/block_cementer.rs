use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    BlockChainSection, BlockEnum, Epochs,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};

use super::{
    block_cache::BlockCache, cementation_walker::CementationWalker,
    AccountsConfirmedMapContainerInfo, BatchWriteSizeManager, CementCallbackRefs,
    CementationQueueContainerInfo, LedgerAdapter, LedgerDataRequester, WriteBatcher,
};

pub(super) struct BlockCementer {
    stopped: Arc<AtomicBool>,

    processing_timer: Instant,
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
            processing_timer: Instant::now(),
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
            self.processing_timer = Instant::now();
        }

        let mut txn = self.ledger.store.tx_begin_read();
        let ledger_clone = Arc::clone(&self.ledger);
        let mut ledger_adapter = LedgerAdapter::new(txn.txn_mut(), &ledger_clone);

        //-----
        // plan:
        // match self.logic.enqueue_block(original_block) {
        //   CementerDecision::Hold => {},
        //   CementerDecision::TryFlush => {},
        //   CementerDecision::ForceFlush => {
        //     start_txn();
        //     while let Some(section) = self.logic.next_write(){
        //        write(section)
        //}
        //   }
        // }

        if !self.logic.is_processing_block {
            self.logic.initialize(original_block.clone());
            self.logic.is_processing_block = true;
        }

        while let Some(section) = self
            .logic
            .cementation_walker
            .next_cementation(&mut ledger_adapter)
        {
            self.logic.write_batcher.enqueue(section);
            if self.logic.should_flush(
                callbacks,
                self.logic.is_current_block_done(),
                self.processing_timer.elapsed(),
            ) {
                if self.logic.is_write_queue_full() {
                    self.force_flush(callbacks)
                } else {
                    self.try_flush(callbacks);
                }
            }

            if !self.logic.is_current_block_done() {
                ledger_adapter.refresh_transaction();
            }
        }

        if self.logic.was_block_already_cemented() {
            (callbacks.block_already_cemented)(original_block.hash());
        }

        self.logic.is_processing_block = false;
    }

    /// If nothing is currently using the database write lock then write the cemented pending blocks otherwise continue iterating
    fn try_flush(&mut self, callbacks: &mut CementCallbackRefs) {
        if let Some(write_guard) = self
            .write_database_queue
            .try_lock(Writer::ConfirmationHeight)
        {
            self.flush(write_guard, callbacks);
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

            txn.commit();
            write_guard.release();
            let time_spent_cementing = self.write_txn_started.elapsed();

            self.log_cemented_blocks(time_spent_cementing, section_to_cement.block_count());

            self.logic
                .cementation_written(section_to_cement, time_spent_cementing, callbacks);

            // Only aquire transaction if there are blocks left
            if !self.logic.is_done_writing() {
                write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
                txn.renew();
                self.write_txn_started = Instant::now();
            }
        }
    }

    pub fn write_pending_blocks(&mut self, callbacks: &mut CementCallbackRefs) {
        if self.logic.write_batcher.has_pending_writes() {
            self.force_flush(callbacks);
        }
    }

    fn log_cemented_blocks(&self, time_spent_cementing: Duration, cemented_count: u64) {
        if self.enable_timing_logging {
            self.logger.always_log(&format!(
                "Cemented {} blocks in {} ms (bounded processor)",
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
    is_processing_block: bool,
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
            is_processing_block: false,
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

    pub fn is_current_block_done(&self) -> bool {
        self.cementation_walker.is_done()
    }

    pub fn is_done_writing(&self) -> bool {
        self.write_batcher.is_done()
    }

    pub fn was_block_already_cemented(&self) -> bool {
        self.cementation_walker.num_accounts_walked() == 0
    }

    pub fn next_write<T: LedgerDataRequester>(
        &mut self,
        data_requester: &T,
    ) -> Option<BlockChainSection> {
        self.write_batcher.next_write(data_requester)
    }

    fn cementation_written(
        &mut self,
        section_to_cement: BlockChainSection,
        time_spent_cementing: Duration,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.cementation_walker
            .cementation_written(&section_to_cement.account, section_to_cement.top_height);

        self.write_batcher
            .batch_write_size
            .adjust_size(time_spent_cementing);

        self.write_batcher
            .publish_cemented_blocks(callbacks.block_cemented);
    }

    pub(crate) fn container_info(&self) -> BlockCementerContainerInfo {
        BlockCementerContainerInfo {
            cementation_queue: self.write_batcher.container_info(),
            accounts_confirmed: self.cementation_walker.container_info(),
        }
    }
}
