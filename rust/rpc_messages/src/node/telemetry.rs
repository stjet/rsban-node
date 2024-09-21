use std::net::Ipv6Addr;
use crate::RpcCommand;
use serde::{Serialize, Deserialize};

impl RpcCommand {
    pub fn telemetry(raw: Option<bool>, address: Option<Ipv6Addr>, port: Option<u16>) -> Self {
        Self::Telemetry(TelemetryArgs::new(raw, address, port))
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TelemetryArgs {
    pub raw: Option<bool>,
    pub address: Option<Ipv6Addr>,
    pub port: Option<u16>
}

impl TelemetryArgs {
    pub fn new(raw: Option<bool>, address: Option<Ipv6Addr>, port: Option<u16>) -> Self {
        Self {
            raw,
            address,
            port,
        }
    }
}