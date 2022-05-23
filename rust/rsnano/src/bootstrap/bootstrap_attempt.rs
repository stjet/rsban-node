use crate::{
    block_processor::BlockProcessor,
    encode_hex,
    logger_mt::Logger,
    unchecked_info::UncheckedInfo,
    websocket::{Listener, MessageBuilder},
    Account, BlockEnum, HardenedConstants, Ledger,
};
use anyhow::Result;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, RwLock, Weak,
    },
    time::{Duration, Instant},
};

use super::{bootstrap_limits, BootstrapInitiator, BootstrapMode};

pub(crate) struct BootstrapAttempt {
    incremental_id: u64,
    pub id: String,
    pub mode: BootstrapMode,
    pub total_blocks: AtomicU64,
    next_log: Mutex<Instant>,
    logger: Arc<dyn Logger>,
    websocket_server: Arc<dyn Listener>,
    ledger: Arc<Ledger>,
    attempt_start: Instant,

    /// There is a circular dependency between BlockProcessor and BootstrapAttempt,
    /// that's why we take a Weak reference
    block_processor: Weak<BlockProcessor>,

    /// There is a circular dependency between BootstrapInitiator and BootstrapAttempt,
    /// that's why we take a Weak reference
    bootstrap_initiator: Weak<BootstrapInitiator>,
}

impl BootstrapAttempt {
    pub(crate) fn new(
        logger: Arc<dyn Logger>,
        websocket_server: Arc<dyn Listener>,
        block_processor: Weak<BlockProcessor>,
        bootstrap_initiator: Weak<BootstrapInitiator>,
        ledger: Arc<Ledger>,
        id: &str,
        mode: BootstrapMode,
        incremental_id: u64,
    ) -> Result<Self> {
        let id = if id.is_empty() {
            encode_hex(HardenedConstants::get().random_128)
        } else {
            id.to_owned()
        };

        let result = Self {
            incremental_id,
            id,
            next_log: Mutex::new(Instant::now()),
            logger,
            block_processor,
            bootstrap_initiator,
            mode,
            websocket_server,
            ledger,
            attempt_start: Instant::now(),
            total_blocks: AtomicU64::new(0),
        };

        result.start()?;
        Ok(result)
    }

    fn start(&self) -> Result<()> {
        let mode = self.mode_text();
        let id = &self.id;
        self.logger
            .always_log(&format!("Starting {mode} bootstrap attempt with ID {id}"));
        self.websocket_server
            .broadcast(&MessageBuilder::bootstrap_started(id, mode)?)?;
        Ok(())
    }

    pub(crate) fn stop(&self) {
        if let Some(initiator) = self.bootstrap_initiator.upgrade() {
            initiator.clear_pulls(self.incremental_id);
        }
    }

    pub(crate) fn should_log(&self) -> bool {
        let mut next_log = self.next_log.lock().unwrap();
        let now = Instant::now();
        if *next_log < now {
            *next_log = now + Duration::from_secs(15);
            true
        } else {
            false
        }
    }

    pub(crate) fn mode_text(&self) -> &'static str {
        match self.mode {
            BootstrapMode::Legacy => "legacy",
            BootstrapMode::Lazy => "lazy",
            BootstrapMode::WalletLazy => "wallet_lazy",
        }
    }

    pub(crate) fn process_block(
        &self,
        block: Arc<RwLock<BlockEnum>>,
        known_account: &Account,
        pull_blocks_processed: u64,
        _max_blocks: u32,
        _block_expected: bool,
        _retry_limit: u32,
    ) -> bool {
        let mut stop_pull = false;
        let hash = { block.read().unwrap().as_block().hash() };
        // If block already exists in the ledger, then we can avoid next part of long account chain
        if pull_blocks_processed % bootstrap_limits::PULL_COUNT_PER_CHECK == 0
            && self.ledger.block_or_pruned_exists(&hash)
        {
            stop_pull = true;
        } else {
            let unchecked_info =
                UncheckedInfo::new(block, known_account, crate::SignatureVerification::Unknown);
            if let Some(p) = self.block_processor.upgrade() {
                p.add(&unchecked_info);
            }
        }

        stop_pull
    }
}

impl Drop for BootstrapAttempt {
    fn drop(&mut self) {
        let mode = self.mode_text();
        let id = &self.id;
        self.logger
            .always_log(&format!("Exiting {mode} bootstrap attempt with ID {id}"));

        let duration = self.attempt_start.elapsed();
        self.websocket_server
            .broadcast(
                &MessageBuilder::bootstrap_exited(
                    id,
                    mode,
                    duration,
                    self.total_blocks.load(Ordering::SeqCst),
                )
                .unwrap(),
            )
            .unwrap();
    }
}
