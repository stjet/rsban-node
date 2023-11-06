use std::sync::{Arc, Weak};

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_ledger::Ledger;

use crate::{
    block_processing::BlockProcessor,
    config::{Logging, NodeFlags},
    messages::{Message, MessageVisitor},
    stats::Stats,
    transport::{BootstrapMessageVisitor, TcpServer},
    utils::{AsyncRuntime, ThreadPool},
};

use super::{
    BootstrapInitiator, BulkPullAccountServer, BulkPullServer, BulkPushServer, FrontierReqServer,
};

pub struct BootstrapMessageVisitorImpl {
    pub async_rt: Arc<AsyncRuntime>,
    pub ledger: Arc<Ledger>,
    pub logger: Arc<dyn Logger>,
    pub connection: Arc<TcpServer>,
    pub thread_pool: Weak<dyn ThreadPool>,
    pub block_processor: Weak<BlockProcessor>,
    pub bootstrap_initiator: Weak<BootstrapInitiator>,
    pub stats: Arc<Stats>,
    pub work_thresholds: WorkThresholds,
    pub flags: NodeFlags,
    pub processed: bool,
    pub logging_config: Logging,
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

                if self.logging_config.bulk_pull_logging() {
                    self.logger.try_log(&format!(
                        "Received bulk pull for {} down to {}, maximum of {} from {}",
                        payload.start,
                        payload.end,
                        payload.count,
                        self.connection.remote_endpoint()
                    ));
                }

                let payload = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let logger = Arc::clone(&self.logger);
                let thread_pool2 = Arc::clone(&thread_pool);
                let enable_logging = self.logging_config.bulk_pull_logging();
                thread_pool.push_task(Box::new(move || {
                    // TODO from original code: Add completion callback to bulk pull server
                    // TODO from original code: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let mut bulk_pull_server = BulkPullServer::new(
                        payload,
                        connection,
                        ledger,
                        logger,
                        thread_pool2,
                        enable_logging,
                    );
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

                if self.logging_config.bulk_pull_logging() {
                    self.logger.try_log(&format!(
                        "Received bulk pull account for {} with a minimum amount of {}",
                        payload.account.encode_account(),
                        payload.minimum_amount.format_balance(10)
                    ));
                }

                let payload = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let thread_pool2 = Arc::clone(&thread_pool);
                let logger = Arc::clone(&self.logger);
                let enable_logging = self.logging_config.bulk_pull_logging();
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: Add completion callback to bulk pull server
                    // original code TODO: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let bulk_pull_account_server = BulkPullAccountServer::new(
                        connection,
                        payload,
                        logger,
                        thread_pool2,
                        ledger,
                        enable_logging,
                    );
                    bulk_pull_account_server.send_frontier();
                }));

                self.processed = true;
            }
            Message::BulkPush(_) => {
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
                let logger = Arc::clone(&self.logger);
                let enable_logging = self.logging_config.bulk_pull_logging();
                let enable_packet_logging = self.logging_config.network_packet_logging();
                let stats = Arc::clone(&self.stats);
                let work_thresholds = self.work_thresholds.clone();
                let async_rt = Arc::clone(&self.async_rt);
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: Add completion callback to bulk pull server
                    let bulk_push_server = BulkPushServer::new(
                        async_rt,
                        connection,
                        ledger,
                        logger,
                        thread_pool2,
                        enable_logging,
                        enable_packet_logging,
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
                if self.logging_config.bulk_pull_logging() {
                    self.logger.try_log(&format!(
                        "Received frontier request for {} with age {}",
                        payload.start.encode_account(),
                        payload.age
                    ));
                }

                let request = payload.clone();
                let connection = Arc::clone(&self.connection);
                let ledger = Arc::clone(&self.ledger);
                let logger = Arc::clone(&self.logger);
                let enable_logging = self.logging_config.bulk_pull_logging();
                let enable_network_logging = self.logging_config.network_logging_value;
                let thread_pool2 = Arc::clone(&thread_pool);
                thread_pool.push_task(Box::new(move || {
                    // original code TODO: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
                    let response = FrontierReqServer::new(
                        connection,
                        request,
                        thread_pool2,
                        logger,
                        enable_logging,
                        enable_network_logging,
                        ledger,
                    );
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
