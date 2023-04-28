use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use rsnano_core::{
    utils::{ContainerInfoComponent, Logger},
    BlockEnum, ConfirmationHeightUpdate,
};
use rsnano_ledger::{Ledger, WriteDatabaseQueue, WriteGuard, Writer};
use rsnano_store_traits::WriteTransaction;

use super::{
    block_cache::BlockCache,
    bounded_mode_helper::{CementationStep, BoundedModeHelper},
    AccountsConfirmedMapContainerInfo, BatchWriteSizeManager, CementCallbackRefs, LedgerAdapter,
    LedgerDataRequester, MultiAccountCementer, WriteDetailsContainerInfo,
};

pub(super) struct BlockCementer {
    stopped: Arc<AtomicBool>,
    batch_separate_pending_min_time: Duration,
    cementer: MultiAccountCementer,

    processing_timer: Instant,
    cemented_batch_timer: Instant,
    write_database_queue: Arc<WriteDatabaseQueue>,
    logger: Arc<dyn Logger>,
    enable_timing_logging: bool,
    ledger: Arc<Ledger>,
    helper: BoundedModeHelper,
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
        let helper = BoundedModeHelper::builder()
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
            cementer: MultiAccountCementer::new(),
            helper,
        }
    }

    pub(crate) fn batch_write_size(&self) -> &Arc<BatchWriteSizeManager> {
        &self.cementer.batch_write_size
    }

    pub fn block_cache(&self) -> &Arc<BlockCache> {
        &self.helper.block_cache()
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

        self.helper.initialize(original_block.clone());

        let mut txn = self.ledger.store.tx_begin_read();
        let ledger_clone = Arc::clone(&self.ledger);

        let mut ledger_adapter = LedgerAdapter::new(txn.txn_mut(), &ledger_clone);

        loop {
            match self.helper.get_next_step(&mut ledger_adapter).unwrap() {
                CementationStep::Write(write_details) => {
                    self.cementer.enqueue(write_details);
                    if self.should_flush(callbacks, self.helper.is_done()) {
                        self.try_flush(callbacks);
                    }
                }
                CementationStep::AlreadyCemented(hash) => {
                    (callbacks.block_already_cemented)(hash);
                    return;
                }
                CementationStep::Done => break,
            }

            if self.helper.is_done() || self.stopped.load(Ordering::SeqCst) {
                break;
            }
            ledger_adapter.refresh_transaction();
        }
    }

    fn should_flush(&self, callbacks: &mut CementCallbackRefs, current_process_done: bool) -> bool {
        let is_batch_full = self.cementer.max_batch_write_size_reached();

        // When there are a lot of pending confirmation height blocks, it is more efficient to
        // bulk some of them up to enable better write performance which becomes the bottleneck.
        let awaiting_processing = (callbacks.awaiting_processing_count)();
        let is_done_processing = current_process_done
            && (awaiting_processing == 0 || self.is_min_processing_time_exceeded());

        let should_flush = is_done_processing || is_batch_full || self.is_write_queue_full();
        should_flush && !self.cementer.has_pending_writes()
    }

    fn is_write_queue_full(&self) -> bool {
        self.cementer.max_pending_writes_reached() || self.helper.is_accounts_cache_full()
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
        if !self.cementer.has_pending_writes() {
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
        while let Some((update_command, account_done)) = self
            .cementer
            .cement_next(&LedgerAdapter::new(txn.txn_mut(), &self.ledger))
            .unwrap()
        {
            self.flush(txn.as_mut(), &update_command, scoped_write_guard, callbacks);
            if account_done {
                self.helper
                    .clear_cached_account(&update_command.account, update_command.new_height);
            }
        }
        drop(txn);

        let unpublished_count = self.cementer.unpublished_cemented_blocks();
        self.stop_batch_timer(unpublished_count);

        if unpublished_count > 0 {
            scoped_write_guard.release();
            self.cementer
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

        self.cementer
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
        update: &ConfirmationHeightUpdate,
        scoped_write_guard: &mut WriteGuard,
        callbacks: &mut CementCallbackRefs,
    ) {
        self.ledger.write_confirmation_height(txn, update);

        let time_spent_cementing = self.cemented_batch_timer.elapsed();
        txn.commit();

        self.log_cemented_blocks(
            time_spent_cementing,
            self.cementer.unpublished_cemented_blocks(),
        );
        self.cementer
            .batch_write_size
            .adjust_size(time_spent_cementing);
        scoped_write_guard.release();
        self.cementer
            .publish_cemented_blocks(callbacks.block_cemented);

        // Only aquire transaction if there are blocks left
        if !self.cementer.is_done() {
            *scoped_write_guard = self.write_database_queue.wait(Writer::ConfirmationHeight);
            txn.renew();
        }

        self.start_batch_timer();
    }

    pub fn clear_process_vars(&mut self) {
        self.helper.clear_all_cached_accounts();
    }

    pub fn has_pending_writes(&self) -> bool {
        self.cementer.has_pending_writes()
    }

    pub fn container_info(&self) -> BoundedModeContainerInfo {
        BoundedModeContainerInfo {
            pending_writes: self.cementer.container_info(),
            accounts_confirmed: self.helper.container_info(),
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
