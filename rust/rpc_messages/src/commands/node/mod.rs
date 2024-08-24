mod keepalive;
mod stop;

pub use keepalive::*;
use serde::{Deserialize, Serialize};
use std::net::Ipv6Addr;
pub use stop::*;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum NodeRpcCommand {
    Keepalive(KeepaliveArgs),
    Stop,
}

impl NodeRpcCommand {
    pub fn keepalive(address: Ipv6Addr, port: u16) -> Self {
        Self::Keepalive(KeepaliveArgs { address, port })
    }
}
