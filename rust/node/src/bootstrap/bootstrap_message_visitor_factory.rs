use std::sync::{Arc, Weak};

use rsnano_core::KeyPair;
use rsnano_ledger::Ledger;

use crate::{
    block_processing::BlockProcessor,
    config::{NetworkConstants, NodeFlags},
    stats::Stats,
    transport::{
        BootstrapMessageVisitor, RealtimeMessageVisitor, RealtimeMessageVisitorImpl,
        ResponseServer, SynCookies,
    },
    utils::{AsyncRuntime, ThreadPool},
};

use super::{BootstrapInitiator, BootstrapMessageVisitorImpl};

pub struct BootstrapMessageVisitorFactory {
    async_rt: Arc<AsyncRuntime>,
    syn_cookies: Arc<SynCookies>,
    stats: Arc<Stats>,
    node_id: KeyPair,
    network_constants: NetworkConstants,
    ledger: Arc<Ledger>,
    thread_pool: Weak<dyn ThreadPool>,
    block_processor: Weak<BlockProcessor>,
    bootstrap_initiator: Weak<BootstrapInitiator>,
    flags: NodeFlags,
}

impl BootstrapMessageVisitorFactory {
    pub fn new(
        async_rt: Arc<AsyncRuntime>,
        syn_cookies: Arc<SynCookies>,
        stats: Arc<Stats>,
        network_constants: NetworkConstants,
        node_id: KeyPair,
        ledger: Arc<Ledger>,
        thread_pool: Arc<dyn ThreadPool>,
        block_processor: Arc<BlockProcessor>,
        bootstrap_initiator: Arc<BootstrapInitiator>,
        flags: NodeFlags,
    ) -> Self {
        Self {
            async_rt,
            syn_cookies,
            stats,
            node_id,
            network_constants,
            ledger,
            thread_pool: Arc::downgrade(&thread_pool),
            block_processor: Arc::downgrade(&block_processor),
            bootstrap_initiator: Arc::downgrade(&bootstrap_initiator),
            flags,
        }
    }

    pub fn realtime_visitor(&self, server: Arc<ResponseServer>) -> Box<dyn RealtimeMessageVisitor> {
        Box::new(RealtimeMessageVisitorImpl::new(
            server,
            Arc::clone(&self.stats),
        ))
    }

    pub fn bootstrap_visitor(
        &self,
        server: Arc<ResponseServer>,
    ) -> Box<dyn BootstrapMessageVisitor> {
        Box::new(BootstrapMessageVisitorImpl {
            async_rt: Arc::clone(&self.async_rt),
            ledger: Arc::clone(&self.ledger),
            connection: server,
            thread_pool: self.thread_pool.clone(),
            block_processor: self.block_processor.clone(),
            bootstrap_initiator: self.bootstrap_initiator.clone(),
            stats: Arc::clone(&self.stats),
            work_thresholds: self.network_constants.work.clone(),
            flags: self.flags.clone(),
            processed: false,
        })
    }
}
