use std::sync::{Arc, Mutex};

use rsnano_core::{utils::Logger, work::WorkThresholds};
use rsnano_ledger::Ledger;

use crate::{
    block_processing::BlockProcessor,
    config::{Logging, NodeFlags},
    messages::{BulkPull, BulkPullAccount, MessageVisitor},
    stats::Stats,
    transport::TcpServer,
    utils::ThreadPool,
};

use super::{bulk_pull_account_server, BootstrapInitiator, BulkPullAccountServer, BulkPullServer};

pub struct BootstrapMessageVisitorImpl {
    pub ledger: Arc<Ledger>,
    pub logger: Arc<dyn Logger>,
    pub connection: Arc<TcpServer>,
    pub thread_pool: Arc<dyn ThreadPool>,
    pub block_processor: Arc<BlockProcessor>,
    pub bootstrap_initiator: Arc<BootstrapInitiator>,
    pub stats: Arc<Stats>,
    pub work_thresholds: WorkThresholds,
    pub flags: NodeFlags,
    pub processed: bool,
    pub logging_config: Logging,
}

impl MessageVisitor for BootstrapMessageVisitorImpl {
    fn bulk_pull(&mut self, message: &BulkPull) {
        if self.flags.disable_bootstrap_bulk_pull_server {
            return;
        }

        if self.logging_config.bulk_pull_logging() {
            self.logger.try_log(&format!(
                "Received bulk pull for {} down to {}, maximum of {} from {}",
                message.start,
                message.end,
                message.count,
                self.connection.remote_endpoint()
            ));
        }

        let message = message.clone();
        let connection = Arc::clone(&self.connection);
        let ledger = Arc::clone(&self.ledger);
        let logger = Arc::clone(&self.logger);
        let thread_pool = Arc::clone(&self.thread_pool);
        let enable_logging = self.logging_config.bulk_pull_logging();
        self.thread_pool.push_task(Box::new(move || {
            // TODO from original code: Add completion callback to bulk pull server
            // TODO from original code: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
            let mut bulk_pull_server = BulkPullServer::new(
                message,
                connection,
                ledger,
                logger,
                thread_pool,
                enable_logging,
            );
            bulk_pull_server.send_next();
        }));

        self.processed = true;
    }

    fn bulk_pull_account(&mut self, message: &BulkPullAccount) {
        if self.flags.disable_bootstrap_bulk_pull_server {
            return;
        }

        if self.logging_config.bulk_pull_logging() {
            self.logger.try_log(&format!(
                "Received bulk pull account for {} with a minimum amount of {}",
                message.account.encode_account(),
                message.minimum_amount.format_balance(10)
            ));
        }

        let message = message.clone();
        let connection = Arc::clone(&self.connection);
        let ledger = Arc::clone(&self.ledger);
        let logger = Arc::clone(&self.logger);
        let thread_pool = Arc::clone(&self.thread_pool);
        let enable_logging = self.logging_config.bulk_pull_logging();
        self.thread_pool.push_task(Box::new(move || {
            // original code TODO: Add completion callback to bulk pull server
            // original code TODO: There should be no need to re-copy message as unique pointer, refactor those bulk/frontier pull/push servers
            let bulk_pull_account_server = BulkPullAccountServer::new(
                connection,
                message,
                logger,
                thread_pool,
                ledger,
                enable_logging,
            );
            bulk_pull_account_server.send_frontier();
        }));

        self.processed = true;
    }
}
