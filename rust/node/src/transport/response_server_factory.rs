use rsnano_core::KeyPair;
use rsnano_ledger::Ledger;

use super::{Network, ResponseServerImpl, ResponseServerObserver, Socket, SynCookies};
use crate::{
    block_processing::BlockProcessor,
    bootstrap::{BootstrapInitiator, BootstrapMessageVisitorFactory},
    config::{NodeConfig, NodeFlags},
    stats::Stats,
    utils::{AsyncRuntime, ThreadPool},
    NetworkParams,
};
use std::sync::Arc;

pub(crate) struct ResponseServerFactory {
    pub(crate) runtime: Arc<AsyncRuntime>,
    pub(crate) syn_cookies: Arc<SynCookies>,
    pub(crate) stats: Arc<Stats>,
    pub(crate) node_id: KeyPair,
    pub(crate) ledger: Arc<Ledger>,
    pub(crate) workers: Arc<dyn ThreadPool>,
    pub(crate) block_processor: Arc<BlockProcessor>,
    pub(crate) bootstrap_initiator: Arc<BootstrapInitiator>,
    pub(crate) network: Arc<Network>,
    pub(crate) node_flags: NodeFlags,
    pub(crate) network_params: NetworkParams,
    pub(crate) node_config: NodeConfig,
}

impl ResponseServerFactory {
    pub(crate) fn create_response_server(
        &self,
        socket: Arc<Socket>,
        observer: &Arc<dyn ResponseServerObserver>,
    ) -> Arc<ResponseServerImpl> {
        let message_visitor_factory = Arc::new(BootstrapMessageVisitorFactory::new(
            Arc::clone(&self.runtime),
            Arc::clone(&self.syn_cookies),
            Arc::clone(&self.stats),
            self.network_params.network.clone(),
            self.node_id.clone(),
            Arc::clone(&self.ledger),
            Arc::clone(&self.workers),
            Arc::clone(&self.block_processor),
            Arc::clone(&self.bootstrap_initiator),
            self.node_flags.clone(),
        ));

        let observer = Arc::downgrade(observer);

        Arc::new(ResponseServerImpl::new(
            Arc::clone(&self.runtime),
            &self.network.clone(),
            socket,
            Arc::new(self.node_config.clone()),
            observer,
            Arc::clone(&self.network.publish_filter),
            Arc::new(self.network_params.clone()),
            Arc::clone(&self.stats),
            Arc::clone(&self.network.tcp_message_manager),
            message_visitor_factory,
            true,
            Arc::clone(&self.syn_cookies),
            self.node_id.clone(),
        ))
    }
}
