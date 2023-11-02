use crate::utils::BlockUniquer;

use super::{
    AccountInfoAckPayload, AscPullAckPayload, AscPullAckType, BlocksAckPayload, Message,
    MessageHeader, MessageType, MessageVisitor, ProtocolInfo, PublishPayload,
};
use anyhow::Result;
use rsnano_core::{utils::Stream, BlockEnum};
use std::{
    any::Any,
    fmt::Display,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageEnum {
    pub header: MessageHeader,
    pub payload: Payload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    Keepalive(KeepalivePayload),
    Publish(PublishPayload),
    AscPullAck(AscPullAckPayload),
}

impl Payload {
    fn serialize(&self, stream: &mut dyn Stream) -> std::result::Result<(), anyhow::Error> {
        match &self {
            Payload::Keepalive(x) => x.serialize(stream),
            Payload::Publish(x) => x.serialize(stream),
            Payload::AscPullAck(x) => x.serialize(stream),
        }
    }
}

impl Display for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Payload::Keepalive(x) => x.fmt(f),
            Payload::Publish(x) => x.fmt(f),
            Payload::AscPullAck(x) => x.fmt(f),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct KeepalivePayload {
    pub peers: [SocketAddr; 8],
}

impl KeepalivePayload {
    pub fn deserialize(header: &MessageHeader, stream: &mut impl Stream) -> Result<Self> {
        debug_assert!(header.message_type == MessageType::Keepalive);

        let mut peers = empty_peers();

        for i in 0..8 {
            let mut addr_buffer = [0u8; 16];
            let mut port_buffer = [0u8; 2];
            stream.read_bytes(&mut addr_buffer, 16)?;
            stream.read_bytes(&mut port_buffer, 2)?;

            let port = u16::from_le_bytes(port_buffer);
            let ip_addr = Ipv6Addr::from(addr_buffer);

            peers[i] = SocketAddr::new(IpAddr::V6(ip_addr), port);
        }

        Ok(Self { peers })
    }

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        for peer in &self.peers {
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

    pub fn serialized_size() -> usize {
        8 * (16 + 2)
    }
}

impl Default for KeepalivePayload {
    fn default() -> Self {
        Self {
            peers: empty_peers(),
        }
    }
}

impl Display for KeepalivePayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for peer in &self.peers {
            writeln!(f, "{}", peer)?;
        }
        Ok(())
    }
}

impl MessageEnum {
    pub fn new_keepalive(protocol_info: &ProtocolInfo) -> Self {
        Self {
            header: MessageHeader::new(MessageType::Keepalive, protocol_info),
            payload: Payload::Keepalive(Default::default()),
        }
    }

    pub fn new_publish(protocol_info: &ProtocolInfo, block: Arc<BlockEnum>) -> Self {
        let mut header = MessageHeader::new(MessageType::Publish, protocol_info);
        header.set_block_type(block.block_type());

        Self {
            header,
            payload: Payload::Publish(PublishPayload {
                block: Some(block),
                digest: 0,
            }),
        }
    }

    pub fn new_asc_pull_ack_blocks(
        protocol_info: &ProtocolInfo,
        id: u64,
        blocks: Vec<BlockEnum>,
    ) -> Self {
        let blocks = BlocksAckPayload::new(blocks);
        let header =
            MessageHeader::new_with_payload_len(MessageType::AscPullAck, protocol_info, &blocks);

        Self {
            header,
            payload: Payload::AscPullAck(AscPullAckPayload {
                id,
                pull_type: AscPullAckType::Blocks(blocks),
            }),
        }
    }

    pub fn new_asc_pull_ack_accounts(
        protocol_info: &ProtocolInfo,
        id: u64,
        accounts: AccountInfoAckPayload,
    ) -> Self {
        let header =
            MessageHeader::new_with_payload_len(MessageType::AscPullAck, protocol_info, &accounts);

        Self {
            header,
            payload: Payload::AscPullAck(AscPullAckPayload {
                id,
                pull_type: AscPullAckType::AccountInfo(accounts),
            }),
        }
    }

    pub fn deserialize(
        header: MessageHeader,
        stream: &mut impl Stream,
        digest: u128,
        uniquer: Option<&BlockUniquer>,
    ) -> Result<Self> {
        let payload = match header.message_type {
            MessageType::Keepalive => {
                Payload::Keepalive(KeepalivePayload::deserialize(&header, stream)?)
            }
            MessageType::Publish => Payload::Publish(PublishPayload::deserialize(
                stream, &header, digest, uniquer,
            )?),
            MessageType::AscPullAck => {
                Payload::AscPullAck(AscPullAckPayload::deserialize(stream, &header)?)
            }
            _ => unimplemented!(),
        };
        Ok(Self { header, payload })
    }
}

fn empty_peers() -> [SocketAddr; 8] {
    [SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0); 8]
}

impl Message for MessageEnum {
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

    fn serialize(&self, stream: &mut dyn Stream) -> Result<()> {
        self.header().serialize(stream)?;
        self.payload.serialize(stream)
    }

    fn visit(&self, visitor: &mut dyn MessageVisitor) {
        visitor.keepalive(self)
    }

    fn clone_box(&self) -> Box<dyn Message> {
        Box::new(self.clone())
    }

    fn message_type(&self) -> MessageType {
        self.header.message_type
    }
}

impl Display for MessageEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.header.fmt(f)?;
        writeln!(f)?;
        self.payload.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::ProtocolInfo;
    use rsnano_core::utils::MemoryStream;
    use std::str::FromStr;

    #[test]
    fn serialize_no_peers() {
        let request1 = MessageEnum::new_keepalive(&ProtocolInfo::dev_network());
        let mut stream = MemoryStream::new();
        request1.serialize(&mut stream).unwrap();
        let header = MessageHeader::deserialize(&mut stream).unwrap();
        let request2 = MessageEnum::deserialize(header, &mut stream, 0, None).unwrap();
        let Payload::Keepalive(payload1) = request1.payload else { panic!("not a keepalive message")};
        let Payload::Keepalive(payload2) = request2.payload else { panic!("not a keepalive message")};
        assert_eq!(payload1, payload2);
    }

    #[test]
    fn serialize_peers() -> Result<()> {
        let mut keepalive = KeepalivePayload::default();
        keepalive.peers[0] = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 10000);

        let mut stream = MemoryStream::new();
        keepalive.serialize(&mut stream)?;
        let header = MessageHeader::new(MessageType::Keepalive, &ProtocolInfo::default());
        let deserialized = KeepalivePayload::deserialize(&header, &mut stream)?;
        assert_eq!(keepalive, deserialized);
        Ok(())
    }

    #[test]
    fn keepalive_with_no_peers_to_string() {
        let hdr = MessageHeader::new(MessageType::Keepalive, &ProtocolInfo::dev_network());
        let keepalive = MessageEnum::new_keepalive(&ProtocolInfo::dev_network());
        let expected =
            hdr.to_string() + "\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n";
        assert_eq!(keepalive.to_string(), expected);
    }

    #[test]
    fn keepalive_string() {
        let mut keepalive = KeepalivePayload::default();
        keepalive.peers[1] = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 45);
        keepalive.peers[2] = SocketAddr::new(
            IpAddr::from_str("2001:db8:85a3:8d3:1319:8a2e:370:7348").unwrap(),
            0,
        );
        keepalive.peers[3] = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 65535);
        keepalive.peers[4] = SocketAddr::new(IpAddr::from_str("::ffff:1.2.3.4").unwrap(), 1234);
        keepalive.peers[5] = SocketAddr::new(IpAddr::from_str("::ffff:1.2.3.4").unwrap(), 1234);
        keepalive.peers[6] = SocketAddr::new(IpAddr::from_str("::ffff:1.2.3.4").unwrap(), 1234);
        keepalive.peers[7] = SocketAddr::new(IpAddr::from_str("::ffff:1.2.3.4").unwrap(), 1234);

        let mut expected = String::new();
        expected.push_str("[::]:0\n");
        expected.push_str("[::1]:45\n");
        expected.push_str("[2001:db8:85a3:8d3:1319:8a2e:370:7348]:0\n");
        expected.push_str("[::]:65535\n");
        expected.push_str("[::ffff:1.2.3.4]:1234\n");
        expected.push_str("[::ffff:1.2.3.4]:1234\n");
        expected.push_str("[::ffff:1.2.3.4]:1234\n");
        expected.push_str("[::ffff:1.2.3.4]:1234\n");

        assert_eq!(keepalive.to_string(), expected);
    }
}
