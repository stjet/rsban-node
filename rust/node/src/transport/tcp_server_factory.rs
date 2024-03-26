use rsnano_core::KeyPair;

use super::{
    Channel, ChannelTcp, NetworkFilter, Socket, SocketType, SynCookies, TcpMessageManager,
    TcpServer, TcpServerExt, TcpServerObserver,
};
use crate::{
    bootstrap::BootstrapMessageVisitorFactory, config::NodeConfig, stats::Stats,
    utils::AsyncRuntime, NetworkParams,
};
use std::{
    sync::{Arc, Weak},
    time::SystemTime,
};

pub struct TcpServerFactory {
    pub async_rt: Arc<AsyncRuntime>,
    pub config: Arc<NodeConfig>,
    pub observer: Weak<dyn TcpServerObserver>,
    pub publish_filter: Arc<NetworkFilter>,
    pub network: Arc<NetworkParams>,
    pub stats: Arc<Stats>,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    pub message_visitor_factory: Option<Arc<BootstrapMessageVisitorFactory>>,
    pub syn_cookies: Arc<SynCookies>,
    pub node_id: KeyPair,
}
impl TcpServerFactory {
    pub fn create_tcp_server(
        &self,
        channel: &Arc<ChannelTcp>,
        socket: Arc<Socket>,
    ) -> Arc<TcpServer> {
        channel.set_last_packet_sent(SystemTime::now());
        let response_server = TcpServer::new(
            Arc::clone(&self.async_rt),
            socket,
            Arc::clone(&self.config),
            Weak::clone(&self.observer),
            Arc::clone(&self.publish_filter),
            Arc::clone(&self.network),
            Arc::clone(&self.stats),
            Arc::clone(&self.tcp_message_manager),
            Arc::clone(
                &self
                    .message_visitor_factory
                    .as_ref()
                    .expect("no message visitor factory provided"),
            ),
            true,
            Arc::clone(&self.syn_cookies),
            self.node_id.clone(),
        );
        // Listen for possible responses
        response_server
            .socket
            .set_socket_type(SocketType::RealtimeResponseServer);
        *response_server.remote_node_id.lock().unwrap() = channel.get_node_id().unwrap_or_default();
        let response_server = Arc::new(response_server);
        response_server.start();
        response_server
    }
}
