use super::PriorityScheduler;
use crate::{
    block_processing::BlockProcessor,
    websocket::{MessageBuilder, Topic, WebsocketListener},
};
use rsnano_core::BlockEnum;
use rsnano_ledger::{BlockStatus, Ledger};
use rsnano_store_lmdb::LmdbReadTransaction;
use std::sync::Arc;

/// Observes confirmed blocks and dispatches the process_live function.
pub struct ProcessLiveDispatcher {
    ledger: Arc<Ledger>,
    scheduler: Arc<PriorityScheduler>,
    websocket: Option<Arc<WebsocketListener>>,
}

impl ProcessLiveDispatcher {
    pub fn new(
        ledger: Arc<Ledger>,
        scheduler: Arc<PriorityScheduler>,
        websocket: Option<Arc<WebsocketListener>>,
    ) -> Self {
        Self {
            ledger,
            scheduler,
            websocket,
        }
    }

    fn inspect(&self, result: &BlockStatus, block: &BlockEnum, tx: &LmdbReadTransaction) {
        if *result == BlockStatus::Progress {
            self.process_live(block, tx);
        }
    }

    fn process_live(&self, block: &BlockEnum, tx: &LmdbReadTransaction) {
        // Start collecting quorum on block
        if self.ledger.dependents_confirmed(tx, block) {
            self.scheduler.activate(&block.account(), tx);
        }

        if let Some(websocket) = &self.websocket {
            if websocket.any_subscriber(Topic::NewUnconfirmedBlock) {
                websocket.broadcast(&MessageBuilder::new_block_arrived(block));
            }
        }
    }
}

pub trait ProcessLiveDispatcherExt {
    fn connect(&self, block_processor: &BlockProcessor);
}

impl ProcessLiveDispatcherExt for Arc<ProcessLiveDispatcher> {
    fn connect(&self, block_processor: &BlockProcessor) {
        let self_w = Arc::downgrade(self);
        block_processor.add_batch_processed_observer(Box::new(move |batch| {
            if let Some(self_l) = self_w.upgrade() {
                let tx = self_l.ledger.read_txn();
                for (result, context) in batch {
                    self_l.inspect(result, &context.block, &tx);
                }
            }
        }));
    }
}
