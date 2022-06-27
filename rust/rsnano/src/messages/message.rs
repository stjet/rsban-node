use super::{MessageHeader, MessageType};
use crate::{
    deserialize_block, serialized_block_size,
    utils::Stream,
    voting::{Vote, VoteUniquer},
    BlockEnum, BlockHash, BlockType, BlockUniquer, NetworkConstants, Root,
};
use anyhow::Result;
use std::{
    any::Any,
    fmt::Write,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::{Arc, RwLock},
};

pub trait Message {
    fn header(&self) -> &MessageHeader;
    fn set_header(&mut self, header: &MessageHeader);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Clone)]
pub struct Keepalive {
    header: MessageHeader,
    peers: [SocketAddr; 8],
}

impl Keepalive {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::Keepalive),
            peers: empty_peers(),
        }
    }

    pub fn with_version_using(constants: &NetworkConstants, version_using: u8) -> Self {
        Self {
            header: MessageHeader::with_version_using(
                constants,
                MessageType::Keepalive,
                version_using,
            ),
            peers: empty_peers(),
        }
    }

    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
            peers: empty_peers(),
        }
    }

    pub fn peers(&self) -> &[SocketAddr; 8] {
        &self.peers
    }

    pub fn set_peers(&mut self, peers: &[SocketAddr; 8]) {
        self.peers = *peers;
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.header().serialize(stream)?;
        for peer in self.peers() {
            match peer {
                SocketAddr::V4(_) => panic!("ipv6 expected but was ipv4"), //todo make peers IpAddrV6?
                SocketAddr::V6(addr) => {
                    let ip_bytes = addr.ip().octets();
                    stream.write_bytes(&ip_bytes)?;

                    let port_bytes = addr.port().to_le_bytes();
                    stream.write_bytes(&port_bytes)?;
                }
            }
        }
        Ok(())
    }

    pub fn deserialize(&mut self, stream: &mut impl Stream) -> Result<()> {
        debug_assert!(self.header().message_type() == MessageType::Keepalive);

        for i in 0..8 {
            let mut addr_buffer = [0u8; 16];
            let mut port_buffer = [0u8; 2];
            stream.read_bytes(&mut addr_buffer, 16)?;
            stream.read_bytes(&mut port_buffer, 2)?;

            let port = u16::from_le_bytes(port_buffer);
            let ip_addr = Ipv6Addr::from(addr_buffer);

            self.peers[i] = SocketAddr::new(IpAddr::V6(ip_addr), port);
        }
        Ok(())
    }

    pub fn size() -> usize {
        8 * (16 + 2)
    }
}

fn empty_peers() -> [SocketAddr; 8] {
    [SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0); 8]
}

impl Message for Keepalive {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct Publish {
    header: MessageHeader,
    pub block: Option<Arc<RwLock<BlockEnum>>>, //todo remove Option
    pub digest: u128,
}

impl Publish {
    pub fn new(constants: &NetworkConstants, block: Arc<RwLock<BlockEnum>>) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::Publish);
        header.set_block_type(block.read().unwrap().block_type());

        Self {
            header,
            block: Some(block),
            digest: 0,
        }
    }
    pub fn with_header(header: &MessageHeader, digest: u128) -> Self {
        Self {
            header: header.clone(),
            block: None,
            digest,
        }
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.header().serialize(stream)?;
        let block = self.block.as_ref().ok_or_else(|| anyhow!("no block"))?;
        let lck = block.read().unwrap();
        lck.as_block().serialize(stream)
    }

    pub fn deserialize(
        &mut self,
        stream: &mut impl Stream,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<()> {
        debug_assert!(self.header.message_type() == MessageType::Publish);
        self.block = Some(deserialize_block(
            self.header.block_type(),
            stream,
            uniquer,
        )?);
        Ok(())
    }
}

impl Message for Publish {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct ConfirmReq {
    header: MessageHeader,
    block: Option<Arc<RwLock<BlockEnum>>>,
    roots_hashes: Vec<(BlockHash, Root)>,
}

impl ConfirmReq {
    pub fn with_block(constants: &NetworkConstants, block: Arc<RwLock<BlockEnum>>) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::ConfirmReq);
        header.set_block_type(block.read().unwrap().block_type());

        Self {
            header,
            block: Some(block),
            roots_hashes: Vec::new(),
        }
    }

    pub fn with_roots_hashes(
        constants: &NetworkConstants,
        roots_hashes: Vec<(BlockHash, Root)>,
    ) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::ConfirmReq);
        // not_a_block (1) block type for hashes + roots request
        header.set_block_type(BlockType::NotABlock);

        debug_assert!(roots_hashes.len() < 16);
        header.set_count(roots_hashes.len() as u8);

        Self {
            header,
            block: None,
            roots_hashes,
        }
    }

    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
            block: None,
            roots_hashes: Vec::new(),
        }
    }

    pub fn block(&self) -> Option<&Arc<RwLock<BlockEnum>>> {
        self.block.as_ref()
    }

    pub fn roots_hashes(&self) -> &[(BlockHash, Root)] {
        &self.roots_hashes
    }

    pub fn serialize(&self, stream: &mut impl Stream) -> Result<()> {
        self.header().serialize(stream)?;
        if self.header().block_type() == BlockType::NotABlock {
            debug_assert!(!self.roots_hashes().is_empty());
            // Write hashes & roots
            for (hash, root) in self.roots_hashes() {
                stream.write_bytes(hash.as_bytes())?;
                stream.write_bytes(root.as_bytes())?;
            }
        } else {
            match self.block() {
                Some(block) => {
                    block.read().unwrap().as_block().serialize(stream)?;
                }
                None => bail!("block not set"),
            }
        }

        Ok(())
    }

    pub fn deserialize(
        &mut self,
        stream: &mut impl Stream,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<()> {
        debug_assert!(self.header().message_type() == MessageType::ConfirmReq);

        if self.header().block_type() == BlockType::NotABlock {
            let count = self.header().count() as usize;
            for _ in 0..count {
                let block_hash = BlockHash::deserialize(stream)?;
                let root = Root::deserialize(stream)?;
                if !block_hash.is_zero() || !root.is_zero() {
                    self.roots_hashes.push((block_hash, root));
                }
            }

            if self.roots_hashes.is_empty() || self.roots_hashes.len() != count {
                bail!("roots hashes empty or incorrect count");
            }
        } else {
            self.block = Some(deserialize_block(
                self.header().block_type(),
                stream,
                uniquer,
            )?);
        }

        Ok(())
    }

    pub fn roots_string(&self) -> String {
        let mut result = String::new();
        for (hash, root) in self.roots_hashes() {
            write!(&mut result, "{}:{}, ", hash, root).unwrap();
        }
        result
    }

    pub fn serialized_size(block_type: BlockType, count: usize) -> usize {
        let mut result = 0;
        if block_type != BlockType::Invalid && block_type != BlockType::NotABlock {
            result = serialized_block_size(block_type);
        } else if block_type == BlockType::NotABlock {
            result = count * (BlockHash::serialized_size() + Root::serialized_size());
        }
        result
    }
}

impl Message for ConfirmReq {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl PartialEq for ConfirmReq {
    fn eq(&self, other: &Self) -> bool {
        let mut equal = false;
        if let Some(block_a) = self.block() {
            if let Some(block_b) = other.block() {
                let lck_a = block_a.read().unwrap();
                let lck_b = block_b.read().unwrap();
                equal = lck_a.eq(&lck_b);
            }
        } else if !self.roots_hashes().is_empty() && !other.roots_hashes().is_empty() {
            equal = self.roots_hashes() == other.roots_hashes()
        }

        equal
    }
}

#[derive(Clone)]
pub struct ConfirmAck {
    header: MessageHeader,
    vote: Option<Arc<RwLock<Vote>>>,
}

impl ConfirmAck {
    pub fn new(constants: &NetworkConstants, vote: Arc<RwLock<Vote>>) -> Self {
        let mut header = MessageHeader::new(constants, MessageType::ConfirmAck);
        header.set_block_type(BlockType::NotABlock);
        let vote_lk = vote.read().unwrap();
        debug_assert!(vote_lk.hashes.len() < 16);
        header.set_count(vote_lk.hashes.len() as u8);
        drop(vote_lk);

        Self {
            header,
            vote: Some(vote),
        }
    }
    pub fn with_header(
        header: &MessageHeader,
        stream: &mut impl Stream,
        uniquer: Option<&VoteUniquer>,
    ) -> Result<Self> {
        let mut vote = Vote::null();
        vote.deserialize(stream)?;
        let mut vote = Arc::new(RwLock::new(vote));

        if let Some(uniquer) = uniquer {
            vote = uniquer.unique(&vote);
        }

        Ok(Self {
            header: header.clone(),
            vote: Some(vote),
        })
    }

    pub fn vote(&self) -> Option<&Arc<RwLock<Vote>>> {
        self.vote.as_ref()
    }
}

impl Message for ConfirmAck {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct FrontierReq {
    header: MessageHeader,
}

impl FrontierReq {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::FrontierReq),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for FrontierReq {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct BulkPull {
    header: MessageHeader,
}

impl BulkPull {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::BulkPull),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }
}

impl Message for BulkPull {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct BulkPullAccount {
    header: MessageHeader,
}

impl BulkPullAccount {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::BulkPullAccount),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for BulkPullAccount {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct BulkPush {
    header: MessageHeader,
}

impl BulkPush {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::BulkPush),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for BulkPush {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct TelemetryReq {
    header: MessageHeader,
}

impl TelemetryReq {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::TelemetryReq),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for TelemetryReq {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct TelemetryAck {
    header: MessageHeader,
}

impl TelemetryAck {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::TelemetryAck),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for TelemetryAck {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct NodeIdHandshake {
    header: MessageHeader,
}

impl NodeIdHandshake {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::NodeIdHandshake),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
}

impl Message for NodeIdHandshake {
    fn header(&self) -> &MessageHeader {
        &self.header
    }

    fn set_header(&mut self, header: &MessageHeader) {
        self.header = header.clone();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
