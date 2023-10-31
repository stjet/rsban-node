use super::{Message, MessageHeader, MessageType, MessageVisitor, ProtocolInfo};
use anyhow::Result;
use rsnano_core::utils::Stream;
use std::{
    any::Any,
    fmt::Display,
    net::{IpAddr, Ipv6Addr, SocketAddr},
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MessageEnum {
    pub header: MessageHeader,
    pub payload: Payload,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Payload {
    Keepalive(KeepalivePayload),
}
impl Payload {
    fn serialize(&self, stream: &mut dyn Stream) -> std::result::Result<(), anyhow::Error> {
        match &self {
            Payload::Keepalive(x) => x.serialize(stream),
        }
    }
}

impl Display for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Payload::Keepalive(x) => x.fmt(f),
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
            write!(f, "\n{}", peer)?;
        }
        Ok(())
    }
}

impl MessageEnum {
    pub fn new(protocol_info: &ProtocolInfo) -> Self {
        Self {
            header: MessageHeader::new(MessageType::Keepalive, protocol_info),
            payload: Payload::Keepalive(Default::default()),
        }
    }

    pub fn deserialize(header: MessageHeader, stream: &mut impl Stream) -> Result<Self> {
        let payload = match header.message_type {
            MessageType::Keepalive => {
                Payload::Keepalive(KeepalivePayload::deserialize(&header, stream)?)
            }
            _ => unimplemented!(),
        };
        Ok(Self { header, payload })
    }

    pub fn serialized_size() -> usize {
        KeepalivePayload::serialized_size()
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
        self.payload.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use rsnano_core::utils::MemoryStream;
    use std::str::FromStr;

    use super::*;
    use crate::messages::ProtocolInfo;

    #[test]
    fn serialize_no_peers() -> Result<()> {
        let request1 = MessageEnum::new(&ProtocolInfo::dev_network());
        let mut stream = MemoryStream::new();
        request1.serialize(&mut stream)?;
        let header = MessageHeader::from_stream(&mut stream)?;
        let request2 = MessageEnum::deserialize(header, &mut stream)?;
        assert_eq!(request1, request2);
        Ok(())
    }

    #[test]
    fn serialize_peers() -> Result<()> {
        let mut request1 = MessageEnum::new(&ProtocolInfo::dev_network());

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
        let keepalive = MessageEnum::new(&ProtocolInfo::dev_network());
        let expected =
            hdr.to_string() + "\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0\n[::]:0";
        assert_eq!(keepalive.to_string(), expected);
    }

    #[test]
    fn keepalive_string() {
        let hdr = MessageHeader::new(MessageType::Keepalive, &ProtocolInfo::dev_network());

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
        expected.push_str("\n[::]:0");
        expected.push_str("\n[::1]:45");
        expected.push_str("\n[2001:db8:85a3:8d3:1319:8a2e:370:7348]:0");
        expected.push_str("\n[::]:65535");
        expected.push_str("\n[::ffff:1.2.3.4]:1234");
        expected.push_str("\n[::ffff:1.2.3.4]:1234");
        expected.push_str("\n[::ffff:1.2.3.4]:1234");
        expected.push_str("\n[::ffff:1.2.3.4]:1234");

        assert_eq!(keepalive.to_string(), expected);
    }
}
