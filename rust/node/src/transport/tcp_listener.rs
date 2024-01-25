use super::{SynCookies, TcpChannels, TcpServer};
use crate::config::NodeConfig;
use rsnano_core::utils::Logger;
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
};

pub struct TcpListener {
    port: u16,
    max_inbound_connections: usize,
    config: NodeConfig,
    logger: Arc<dyn Logger>,
    tcp_channels: Arc<TcpChannels>,
    syn_cookies: Arc<SynCookies>,
    pub connections: HashMap<usize, Weak<TcpServer>>,
}

impl TcpListener {
    pub fn new(
        port: u16,
        max_inbound_connections: usize,
        config: NodeConfig,
        logger: Arc<dyn Logger>,
        tcp_channels: Arc<TcpChannels>,
        syn_cookies: Arc<SynCookies>,
    ) -> Self {
        Self {
            port,
            max_inbound_connections,
            config,
            logger,
            tcp_channels,
            syn_cookies,
            connections: HashMap::new(),
        }
    }
}
