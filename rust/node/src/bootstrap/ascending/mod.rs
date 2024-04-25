mod account_sets;
mod account_sets_config;
mod ordered_blocking;
mod ordered_priorities;
mod ordered_tags;

pub use account_sets_config::*;

use rsnano_core::BlockEnum;
use rsnano_ledger::{BlockStatus, Ledger};
use rsnano_messages::{AscPullReq, AscPullReqType, BlocksReqPayload, HashType, Message};
use rsnano_store_lmdb::LmdbReadTransaction;
use std::{
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

use crate::{
    block_processing::BlockProcessor,
    bootstrap::ascending::ordered_tags::QueryType,
    config::NodeConfig,
    stats::{DetailType, Direction, StatType, Stats},
    transport::{BufferDropPolicy, ChannelEnum, TrafficType},
};

use self::ordered_tags::AsyncTag;

pub struct BootstrapAscending {
    block_processor: Arc<BlockProcessor>,
    ledger: Arc<Ledger>,
    stats: Arc<Stats>,
    thread: Mutex<Option<JoinHandle<()>>>,
    timeout_thread: Mutex<Option<JoinHandle<()>>>,
    mutex: Mutex<BootstrapAscendingImpl>,
    condition: Condvar,
    config: NodeConfig,
}

impl BootstrapAscending {
    pub fn new(
        block_processor: Arc<BlockProcessor>,
        ledger: Arc<Ledger>,
        stats: Arc<Stats>,
        config: NodeConfig,
    ) -> Self {
        Self {
            block_processor,
            ledger,
            stats,
            thread: Mutex::new(None),
            timeout_thread: Mutex::new(None),
            mutex: Mutex::new(BootstrapAscendingImpl { stopped: false }),
            condition: Condvar::new(),
            config,
        }
    }

    pub fn stop(&self) {
        self.mutex.lock().unwrap().stopped = true;
        self.condition.notify_all();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
        if let Some(handle) = self.timeout_thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }

    fn send(&self, channel: &Arc<ChannelEnum>, tag: AsyncTag) {
        debug_assert!(matches!(
            tag.query_type,
            QueryType::BlocksByHash | QueryType::BlocksByAccount
        ));

        let request_payload = BlocksReqPayload {
            start_type: if tag.query_type == QueryType::BlocksByHash {
                HashType::Block
            } else {
                HashType::Account
            },
            start: tag.start,
            count: self.config.bootstrap_ascending.pull_count as u8,
        };
        let request = Message::AscPullReq(AscPullReq {
            id: tag.id,
            req_type: AscPullReqType::Blocks(request_payload),
        });

        self.stats.inc_dir(
            StatType::BootstrapAscending,
            DetailType::Request,
            Direction::Out,
        );

        // TODO: There is no feedback mechanism if bandwidth limiter starts dropping our requests
        channel.send(
            &request,
            None,
            BufferDropPolicy::Limiter,
            TrafficType::Bootstrap,
        );
    }

    pub fn priority_size(&self) -> usize {
        //TODO port more
        todo!()
    }

    /// Inspects a block that has been processed by the block processor
    fn inspect(&self, _tx: &LmdbReadTransaction, _status: BlockStatus, _block: &Arc<BlockEnum>) {
        todo!()
    }

    fn run(&self) {
        todo!()
    }

    fn run_timeouts(&self) {
        todo!()
    }
}

impl Drop for BootstrapAscending {
    fn drop(&mut self) {
        // All threads must be stopped before destruction
        debug_assert!(self.thread.lock().unwrap().is_none());
        debug_assert!(self.timeout_thread.lock().unwrap().is_none());
    }
}

pub trait BootstrapAscendingExt {
    fn initialize(&self);
    fn start(&self);
}

impl BootstrapAscendingExt for Arc<BootstrapAscending> {
    fn initialize(&self) {
        let self_w = Arc::downgrade(self);
        self.block_processor
            .add_batch_processed_observer(Box::new(move |batch| {
                if let Some(self_l) = self_w.upgrade() {
                    let _guard = self_l.mutex.lock().unwrap();
                    let tx = self_l.ledger.read_txn();
                    for (result, context) in batch {
                        self_l.inspect(&tx, *result, &context.block);
                    }

                    self_l.condition.notify_all();
                }
            }))
    }

    fn start(&self) {
        debug_assert!(self.thread.lock().unwrap().is_none());
        debug_assert!(self.timeout_thread.lock().unwrap().is_none());

        let self_l = Arc::clone(self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Bootstrap asc".to_string())
                .spawn(Box::new(move || self_l.run()))
                .unwrap(),
        );

        let self_l = Arc::clone(self);
        *self.thread.lock().unwrap() = Some(
            std::thread::Builder::new()
                .name("Bootstrap asc".to_string())
                .spawn(Box::new(move || self_l.run_timeouts()))
                .unwrap(),
        );
    }
}

struct BootstrapAscendingImpl {
    stopped: bool,
}
