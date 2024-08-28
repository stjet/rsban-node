mod keepalive_args;

use super::RpcCommand;
pub use keepalive_args::*;
use std::net::Ipv6Addr;

impl RpcCommand {
    pub fn keepalive(address: Ipv6Addr, port: u16) -> Self {
        Self::Keepalive(KeepaliveArgs { address, port })
    }
}
