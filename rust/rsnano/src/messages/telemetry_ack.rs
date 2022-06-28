use crate::{Account, BlockHash, Signature};
use std::time::SystemTime;

#[repr(u8)]
#[derive(FromPrimitive, Copy, Clone, PartialEq, Eq)]
pub enum TelemetryMaker {
    NfNode = 0,
    NfPrunedNode = 1,
}

#[derive(Clone)]
pub struct TelemetryData {
    pub signature: Signature,
    pub node_id: Account,
    pub block_count: u64,
    pub cemented_count: u64,
    pub unchecked_count: u64,
    pub account_count: u64,
    pub bandwidth_cap: u64,
    pub uptime: u64,
    pub peer_count: u32,
    pub protocol_version: u8,
    pub genesis_block: BlockHash,
    pub major_version: u8,
    pub minor_version: u8,
    pub patch_version: u8,
    pub pre_release_version: u8,
    pub maker: u8, // Where this telemetry information originated
    pub timestamp: SystemTime,
    pub active_difficulty: u64,
    pub unknown_data: Vec<u8>,
}

impl TelemetryData {
    pub fn new() -> Self {
        Self {
            signature: Signature::new(),
            node_id: Account::new(),
            block_count: 0,
            cemented_count: 0,
            unchecked_count: 0,
            account_count: 0,
            bandwidth_cap: 0,
            uptime: 0,
            peer_count: 0,
            protocol_version: 0,
            genesis_block: BlockHash::new(),
            major_version: 0,
            minor_version: 0,
            patch_version: 0,
            pre_release_version: 0,
            maker: TelemetryMaker::NfNode as u8,
            timestamp: SystemTime::UNIX_EPOCH,
            active_difficulty: 0,
            unknown_data: Vec::new(),
        }
    }
}
