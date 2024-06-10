use std::sync::{Arc, Weak};

use rsnano_core::work::WorkThresholds;
use rsnano_ledger::Ledger;
use rsnano_messages::{Message, MessageVisitor};

use crate::{
    block_processing::BlockProcessor,
    config::NodeFlags,
    stats::Stats,
    transport::{BootstrapMessageVisitor, ResponseServerImpl},
    utils::{AsyncRuntime, ThreadPool},
};

use super::{
    BootstrapInitiator, BulkPullAccountServer, BulkPullServer, BulkPushServer, FrontierReqServer,
};

pub struct BootstrapMessageVisitorImpl {
    pub async_rt: Arc<AsyncRuntime>,
    pub ledger: Arc<Ledger>,
    pub connection: Arc<ResponseServerImpl>,
    pub thread_pool: Weak<dyn ThreadPool>,
    pub block_processor: Weak<BlockProcessor>,
    pub bootstrap_initiator: Weak<BootstrapInitiator>,
    pub stats: Arc<Stats>,
    pub work_thresholds: WorkThresholds,
    pub flags: NodeFlags,
    pub processed: bool,
}

impl MessageVisitor for BootstrapMessageVisitorImpl {
    fn received(&mut self, message: &Message) {
        match message {
            Message::BulkPull(payload) => {
                if self.flags.disable_bootstrap_bulk_pull_server {
                    return;
                }

                let Some(thread_pool) = self.thread_pool.upgrade() else {
                    return;
                };

                let payload = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let thread_pool2 = Arc::clone(&thread_pool);
                thread_pool.push_task(Box::new(move || {
                    // TODO from original code: Add completion callback to bulk pull server
                    // TODO from original code: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let mut bulk_pull_server =
                        BulkPullServer::new(payload, connection, ledger, thread_pool2);
                    bulk_pull_server.send_next();
                }));

                self.processed = true;
            }
            Message::BulkPullAccount(payload) => {
                if self.flags.disable_bootstrap_bulk_pull_server {
                    return;
                }
                let Some(thread_pool) = self.thread_pool.upgrade() else {
                    return;
                };

                let payload = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let thread_pool2 = Arc::clone(&thread_pool);
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: Add completion callback to bulk pull server
                    // original code TODO: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let bulk_pull_account_server =
                        BulkPullAccountServer::new(connection, payload, thread_pool2, ledger);
                    bulk_pull_account_server.send_frontier();
                }));

                self.processed = true;
            }
            Message::BulkPush => {
                let Some(thread_pool) = self.thread_pool.upgrade() else {
                    return;
                };
                let Some(block_processor) = self.block_processor.upgrade() else {
                    return;
                };
                let Some(bootstrap_initiator) = self.bootstrap_initiator.upgrade() else {
                    return;
                };
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let thread_pool2 = Arc::clone(&thread_pool);
                let stats = Arc::clone(&self.stats);
                let work_thresholds = self.work_thresholds.clone();
                let async_rt = Arc::clone(&self.async_rt);
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: Add completion callback to bulk pull server
                    let bulk_push_server = BulkPushServer::new(
                        async_rt,
                        connection,
                        ledger,
                        thread_pool2,
                        block_processor,
                        bootstrap_initiator,
                        stats,
                        work_thresholds,
                    );
                    bulk_push_server.throttled_receive();
                }));

                self.processed = true;
            }
            Message::FrontierReq(payload) => {
                let Some(thread_pool) = self.thread_pool.upgrade() else {
                    return;
                };

                let request = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let thread_pool2 = Arc::clone(&thread_pool);
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let response =
                        FrontierReqServer::new(connection, request, thread_pool2, ledger);
                    response.send_next();
                }));

                self.processed = true;
            }
            _ => {}
        }
    }
}

impl BootstrapMessageVisitor for BootstrapMessageVisitorImpl {
    fn processed(&self) -> bool {
        self.processed
    }

    fn as_message_visitor(&mut self) -> &mut dyn MessageVisitor {
        self
    }
}
