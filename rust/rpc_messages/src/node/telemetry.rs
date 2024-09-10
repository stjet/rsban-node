use std::net::Ipv6Addr;

use crate::RpcCommand;
use rsnano_core::BlockHash;
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

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct TelemetryDto {
    block_count: u64,
    cemented_count: u64,
    unchecked_count: u64,
    account_count: u64,
    bandwidth_cap: u64,
    peer_count: u64,
    protocol_version: u64,
    uptime: u64,
    genesis_block: BlockHash,
    major_version: u64,
    minor_version: u64,
    patch_version: u64,
    pre_release_version: u64,
    maker: u64,
    timestamp: u64,
    active_difficulty: u64,
}

impl TelemetryDto {
    pub fn new(
        block_count: u64,
        cemented_count: u64,
        unchecked_count: u64,
        account_count: u64,
        bandwidth_cap: u64,
        peer_count: u64,
        protocol_version: u64,
        uptime: u64,
        genesis_block: BlockHash,
        major_version: u64,
        minor_version: u64,
        patch_version: u64,
        pre_release_version: u64,
        maker: u64,
        timestamp: u64,
        active_difficulty: u64,
    ) -> Self {
        Self {
            block_count,
            cemented_count,
            unchecked_count,
            account_count,
            bandwidth_cap,
            peer_count,
            protocol_version,
            uptime,
            genesis_block,
            major_version,
            minor_version,
            patch_version,
            pre_release_version,
            maker,
            timestamp,
            active_difficulty,
        }
    }
}
