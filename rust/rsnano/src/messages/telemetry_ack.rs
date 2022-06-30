use crate::utils::{MemoryStream, Stream, StreamExt};
use crate::{sign_message, validate_message, Account, BlockHash, KeyPair, Signature};
use anyhow::Result;
use std::mem::size_of;
use std::time::{Duration, SystemTime};

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

    /// Size does not include unknown_data
    pub fn serialized_size_without_unknown_data() -> usize {
        Signature::serialized_size()
        + Account::serialized_size()
        + size_of::<u64>() //block_count
          + size_of::<u64>()// cemented_count 
          + size_of::<u64>() // unchecked_count 
          + size_of::<u64>() // account_count 
          + size_of::<u64>() // bandwidth_cap 
          + size_of::<u32>() // peer_count
          + size_of::<u8>() // protocol_version
          + size_of::<u64>() // uptime 
          + BlockHash::serialized_size()
          + size_of::<u8>() // major_version 
          + size_of::<u8>() // minor_version 
          + size_of::<u8>() // patch_version 
          + size_of::<u8>() // pre_release_version 
          + size_of::<u8>() // maker 
          + size_of::<u64>() // timestamp 
          + size_of::<u64>() //active_difficulty)
    }

    /// This needs to be updated for each new telemetry version
    pub fn latest_size() -> usize {
        Self::serialized_size_without_unknown_data()
    }

    pub fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.signature.serialize(stream)?;
        self.serialize_without_signature(stream)
    }

    fn serialize_without_signature(&self, stream: &mut dyn Stream) -> Result<()> {
        // All values should be serialized in big endian
        self.node_id.serialize(stream)?;
        stream.write_u64_be(self.block_count)?;
        stream.write_u64_be(self.cemented_count)?;
        stream.write_u64_be(self.unchecked_count)?;
        stream.write_u64_be(self.account_count)?;
        stream.write_u64_be(self.bandwidth_cap)?;
        stream.write_u32_be(self.peer_count)?;
        stream.write_u8(self.protocol_version)?;
        stream.write_u64_be(self.uptime)?;
        self.genesis_block.serialize(stream)?;
        stream.write_u8(self.major_version)?;
        stream.write_u8(self.minor_version)?;
        stream.write_u8(self.patch_version)?;
        stream.write_u8(self.pre_release_version)?;
        stream.write_u8(self.maker)?;
        stream.write_u64_be(
            self.timestamp
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis() as u64,
        )?;
        stream.write_u64_be(self.active_difficulty)?;
        stream.write_bytes(&self.unknown_data)?;
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut dyn Stream, payload_length: u16) -> Result<()> {
        self.signature = Signature::deserialize(stream)?;
        self.node_id = Account::deserialize(stream)?;
        self.block_count = stream.read_u64_be()?;
        self.cemented_count = stream.read_u64_be()?;
        self.unchecked_count = stream.read_u64_be()?;
        self.account_count = stream.read_u64_be()?;
        self.bandwidth_cap = stream.read_u64_be()?;
        self.peer_count = stream.read_u32_be()?;
        self.protocol_version = stream.read_u8()?;
        self.uptime = stream.read_u64_be()?;
        self.genesis_block = BlockHash::deserialize(stream)?;
        self.major_version = stream.read_u8()?;
        self.minor_version = stream.read_u8()?;
        self.patch_version = stream.read_u8()?;
        self.pre_release_version = stream.read_u8()?;
        self.maker = stream.read_u8()?;

        let timestamp_ms = stream.read_u64_be()?;
        self.timestamp = SystemTime::UNIX_EPOCH + Duration::from_millis(timestamp_ms);
        self.active_difficulty = stream.read_u64_be()?;

        if payload_length as usize > Self::latest_size() {
            let unknown_len = (payload_length as usize) - Self::latest_size();
            self.unknown_data.resize(unknown_len, 0);
            stream.read_bytes(&mut self.unknown_data, unknown_len)?;
        }
        Ok(())
    }

    pub fn sign(&mut self, keys: &KeyPair) -> Result<()> {
        debug_assert!(self.node_id.public_key == keys.public_key());
        let mut stream = MemoryStream::new();
        self.serialize_without_signature(&mut stream)?;
        self.signature = sign_message(&keys.private_key(), &keys.public_key(), stream.as_bytes())?;
        Ok(())
    }

    pub fn validate_signature(&self) -> bool {
        let mut stream = MemoryStream::new();
        if self.serialize_without_signature(&mut stream).is_ok() {
            validate_message(&self.node_id.public_key, stream.as_bytes(), &self.signature).is_ok()
        } else {
            false
        }
    }
}
