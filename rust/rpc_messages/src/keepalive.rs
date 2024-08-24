use serde::{Deserialize, Serialize};
use std::net::Ipv6Addr;

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct KeepaliveRequest {
    pub address: Ipv6Addr,
    pub port: u16,
}
