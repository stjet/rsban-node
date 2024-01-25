use super::{ServerSocket, SynCookies, TcpChannels, TcpServer};
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
    data: TcpListenerData,
}

struct TcpListenerData {
    connections: HashMap<usize, Weak<TcpServer>>,
    on: bool,
    listening_socket: Option<Arc<ServerSocket>>, // TODO remove arc
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
            data: TcpListenerData {
                connections: HashMap::new(),
                on: false,
                listening_socket: None,
            },
        }
    }

    pub fn add_connection(&mut self, conn: &Arc<TcpServer>) {
        self.data
            .connections
            .insert(conn.unique_id(), Arc::downgrade(conn));
    }

    pub fn remove_connection(&mut self, connection_id: usize) {
        self.data.connections.remove(&connection_id);
    }

    pub fn connection_count(&self) -> usize {
        self.data.connections.len()
    }

    pub fn clear_connections(&mut self) {
        // TODO swap with lock and then clear after lock dropped
        self.data.connections.clear();
    }

    pub fn is_on(&self) -> bool {
        self.data.on
    }

    pub fn set_on(&mut self) {
        self.data.on = true;
    }

    pub fn set_off(&mut self) {
        self.data.on = false;
    }

    pub fn set_listening_socket(&mut self, socket: Arc<ServerSocket>) {
        self.data.listening_socket = Some(socket);
    }

    pub fn close_listening_socket(&mut self) {
        self.data.listening_socket = None;
    }

    pub fn has_listening_socket(&self) -> bool {
        self.data.listening_socket.is_some()
    }
}
