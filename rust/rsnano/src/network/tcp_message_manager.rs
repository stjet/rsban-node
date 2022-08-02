use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

use crate::{messages::Message, Account};

use super::SocketImpl;

pub struct TcpMessageItem {
    pub message: Option<Box<dyn Message>>,
    pub endpoint: SocketAddr,
    pub node_id: Account,
    pub socket: Option<Arc<SocketImpl>>,
}

impl TcpMessageItem {
    pub fn new() -> Self {
        Self {
            message: None,
            endpoint: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            node_id: Account::new(),
            socket: None,
        }
    }
}

pub struct TcpMessageManager {
    max_entries: usize,
}

impl TcpMessageManager {
    pub fn new(incoming_connections_max: usize) -> Self {
        Self {
            max_entries: incoming_connections_max * MAX_ENTRIES_PER_CONNECTION + 1,
        }
    }
}

const MAX_ENTRIES_PER_CONNECTION: usize = 16;
