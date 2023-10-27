use std::sync::{Arc, Weak};

use rsnano_core::{utils::Logger, KeyPair};
use rsnano_ledger::Ledger;

use crate::{
    block_processing::BlockProcessor,
    config::{Logging, NetworkConstants, NodeFlags},
    stats::Stats,
    transport::{
        BootstrapMessageVisitor, HandshakeMessageVisitor, HandshakeMessageVisitorImpl,
        RealtimeMessageVisitor, RealtimeMessageVisitorImpl, SynCookies, TcpServer,
    },
    utils::{AsyncRuntime, ThreadPool},
};

use super::{BootstrapInitiator, BootstrapMessageVisitorImpl};

pub struct BootstrapMessageVisitorFactory {
    async_rt: Arc<AsyncRuntime>,
    logger: Arc<dyn Logger>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    node_id: Arc<KeyPair>,
    network_constants: NetworkConstants,
    ledger: Arc<Ledger>,
    thread_pool: Weak<dyn ThreadPool>,
    block_processor: Weak<BlockProcessor>,
    bootstrap_initiator: Weak<BootstrapInitiator>,
    flags: NodeFlags,
    logging_config: Logging,
    pub handshake_logging: bool,
}

impl BootstrapMessageVisitorFactory {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        logger: Arc<dyn Logger>,
        syn_cookies: Arc<SynCookies>,
        stats: Arc<Stats>,
        network_constants: NetworkConstants,
        node_id: Arc<KeyPair>,
        ledger: Arc<Ledger>,
        thread_pool: Arc<dyn ThreadPool>,
        block_processor: Arc<BlockProcessor>,
        bootstrap_initiator: Arc<BootstrapInitiator>,
        flags: NodeFlags,
        logging_config: Logging,
    ) -> Self {
        Self {
            async_rt,
            logger,
            syn_cookies,
            stats,
            node_id,
            handshake_logging: false,
            network_constants,
            ledger,
            thread_pool: Arc::downgrade(&thread_pool),
            block_processor: Arc::downgrade(&block_processor),
            bootstrap_initiator: Arc::downgrade(&bootstrap_initiator),
            flags,
            logging_config,
        }
    }

    pub fn handshake_visitor(&self, server: Arc<TcpServer>) -> Box<dyn HandshakeMessageVisitor> {
        let mut visitor = Box::new(HandshakeMessageVisitorImpl::new(
            server,
            Arc::clone(&self.logger),
            Arc::clone(&self.syn_cookies),
            Arc::clone(&self.stats),
            Arc::clone(&self.node_id),
            self.network_constants.clone(),
        ));
        visitor.disable_tcp_realtime = self.flags.disable_tcp_realtime;
        visitor.handshake_logging = self.handshake_logging;
        visitor
    }

    pub fn realtime_visitor(&self, server: Arc<TcpServer>) -> Box<dyn RealtimeMessageVisitor> {
        Box::new(RealtimeMessageVisitorImpl::new(
            server,
            Arc::clone(&self.stats),
        ))
    }

    pub fn bootstrap_visitor(&self, server: Arc<TcpServer>) -> Box<dyn BootstrapMessageVisitor> {
        Box::new(BootstrapMessageVisitorImpl {
            async_rt: Arc::clone(&self.async_rt),
            ledger: Arc::clone(&self.ledger),
            logger: Arc::clone(&self.logger),
            connection: server,
            thread_pool: self.thread_pool.clone(),
            block_processor: self.block_processor.clone(),
            bootstrap_initiator: self.bootstrap_initiator.clone(),
            stats: Arc::clone(&self.stats),
            work_thresholds: self.network_constants.work.clone(),
            flags: self.flags.clone(),
            processed: false,
            logging_config: self.logging_config.clone(),
        })
    }
}
