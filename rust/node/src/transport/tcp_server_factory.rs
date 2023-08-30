use std::{sync::Arc, time::SystemTime};

use rsnano_core::utils::Logger;

use crate::{
    bootstrap::BootstrapMessageVisitorFactory,
    config::NodeConfig,
    stats::Stats,
    utils::{BlockUniquer, IoContext},
    voting::VoteUniquer,
    NetworkParams,
};

use super::{
    Channel, ChannelTcp, NetworkFilter, Socket, SocketType, TcpMessageManager, TcpServer,
    TcpServerExt, TcpServerObserver,
};

pub struct TcpServerFactory {
    pub config: Arc<NodeConfig>,
    pub logger: Arc<dyn Logger>,
    pub observer: Arc<dyn TcpServerObserver>,
    pub publish_filter: Arc<NetworkFilter>,
    pub io_ctx: Arc<dyn IoContext>,
    pub network: Arc<NetworkParams>,
    pub stats: Arc<Stats>,
    pub block_uniquer: Arc<BlockUniquer>,
    pub vote_uniquer: Arc<VoteUniquer>,
    pub tcp_message_manager: Arc<TcpMessageManager>,
    pub message_visitor_factory: Option<Arc<BootstrapMessageVisitorFactory>>,
}
impl TcpServerFactory {
    pub fn create_tcp_server(&self, channel: &ChannelTcp, socket: Arc<Socket>) -> Arc<TcpServer> {
        channel.set_last_packet_sent(SystemTime::now());
        let response_server = TcpServer::new(
            socket,
            Arc::clone(&self.config),
            Arc::clone(&self.logger),
            Arc::clone(&self.observer),
            Arc::clone(&self.publish_filter),
            Arc::clone(&self.io_ctx),
            Arc::clone(&self.network),
            Arc::clone(&self.stats),
            Arc::clone(&self.block_uniquer),
            Arc::clone(&self.vote_uniquer),
            Arc::clone(&self.tcp_message_manager),
            Arc::clone(
                &self
                    .message_visitor_factory
                    .as_ref()
                    .expect("no message visitor factory provided"),
            ),
            true,
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
