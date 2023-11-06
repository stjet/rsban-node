use crate::{
    stats::DetailType, transport::MAX_MESSAGE_SIZE, utils::BlockUniquer, voting::VoteUniquer,
};

use super::*;
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{MutStreamAdapter, Serialize, Stream};
use std::{fmt::Display, ops::Deref};

pub trait MessageVariant: Serialize + Display + std::fmt::Debug {
    fn message_type(&self) -> MessageType;
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    Keepalive(KeepalivePayload),
    Publish(PublishPayload),
    AscPullAck(AscPullAckPayload),
    AscPullReq(AscPullReqPayload),
    BulkPull(BulkPullPayload),
    BulkPullAccount(BulkPullAccountPayload),
    BulkPush(BulkPushPayload),
    ConfirmAck(ConfirmAckPayload),
    ConfirmReq(ConfirmReqPayload),
    FrontierReq(FrontierReqPayload),
    NodeIdHandshake(NodeIdHandshakePayload),
    TelemetryAck(TelemetryAckPayload),
    TelemetryReq(TelemetryReqPayload),
}

impl Payload {
    fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        self.deref().serialize(stream)
    }

    pub fn message_type(&self) -> MessageType {
        self.deref().message_type()
    }

    pub fn deserialize(
        stream: &mut impl Stream,
        header: &MessageHeader,
        digest: u128,
        block_uniquer: Option<&BlockUniquer>,
        vote_uniquer: Option<&VoteUniquer>,
    ) -> Result<Self> {
        let msg = match header.message_type {
            MessageType::Keepalive => {
                Payload::Keepalive(KeepalivePayload::deserialize(&header, stream)?)
            }
            MessageType::Publish => Payload::Publish(PublishPayload::deserialize(
                stream,
                &header,
                digest,
                block_uniquer,
            )?),
            MessageType::AscPullAck => {
                Payload::AscPullAck(AscPullAckPayload::deserialize(stream, &header)?)
            }
            MessageType::AscPullReq => {
                Payload::AscPullReq(AscPullReqPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPull => {
                Payload::BulkPull(BulkPullPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPullAccount => {
                Payload::BulkPullAccount(BulkPullAccountPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPush => {
                Payload::BulkPush(BulkPushPayload::deserialize(stream, &header)?)
            }
            MessageType::ConfirmAck => {
                Payload::ConfirmAck(ConfirmAckPayload::deserialize(stream, vote_uniquer)?)
            }
            MessageType::ConfirmReq => Payload::ConfirmReq(ConfirmReqPayload::deserialize(
                stream,
                &header,
                block_uniquer,
            )?),
            MessageType::FrontierReq => {
                Payload::FrontierReq(FrontierReqPayload::deserialize(stream, &header)?)
            }
            MessageType::NodeIdHandshake => {
                Payload::NodeIdHandshake(NodeIdHandshakePayload::deserialize(stream, &header)?)
            }
            MessageType::TelemetryAck => {
                Payload::TelemetryAck(TelemetryAckPayload::deserialize(stream, &header)?)
            }
            MessageType::TelemetryReq => {
                Payload::TelemetryReq(TelemetryReqPayload::deserialize(stream, &header)?)
            }
            MessageType::Invalid | MessageType::NotAType => bail!("invalid message type"),
        };
        Ok(msg)
    }
}

impl From<&Payload> for DetailType {
    fn from(value: &Payload) -> Self {
        value.message_type().into()
    }
}

impl Deref for Payload {
    type Target = dyn MessageVariant;

    fn deref(&self) -> &Self::Target {
        match &self {
            Payload::Keepalive(x) => x,
            Payload::Publish(x) => x,
            Payload::AscPullAck(x) => x,
            Payload::AscPullReq(x) => x,
            Payload::BulkPull(x) => x,
            Payload::BulkPullAccount(x) => x,
            Payload::BulkPush(x) => x,
            Payload::ConfirmAck(x) => x,
            Payload::ConfirmReq(x) => x,
            Payload::FrontierReq(x) => x,
            Payload::NodeIdHandshake(x) => x,
            Payload::TelemetryAck(x) => x,
            Payload::TelemetryReq(x) => x,
        }
    }
}

impl Display for Payload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.deref(), f)
    }
}

pub struct MessageSerializer {
    protocol: ProtocolInfo,
    buffer: [u8; Self::BUFFER_SIZE],
}

impl MessageSerializer {
    const BUFFER_SIZE: usize = MessageHeader::SERIALIZED_SIZE + MAX_MESSAGE_SIZE;
    pub fn new(protocol: ProtocolInfo) -> Self {
        Self {
            protocol,
            buffer: [0; Self::BUFFER_SIZE],
        }
    }

    pub fn serialize(&'_ mut self, message: &Payload) -> anyhow::Result<&'_ [u8]> {
        let payload_len;
        {
            let mut payload_stream =
                MutStreamAdapter::new(&mut self.buffer[MessageHeader::SERIALIZED_SIZE..]);
            message.serialize(&mut payload_stream)?;
            payload_len = payload_stream.bytes_written();

            let mut header_stream =
                MutStreamAdapter::new(&mut self.buffer[..MessageHeader::SERIALIZED_SIZE]);
            let mut header = MessageHeader::new(message.message_type(), self.protocol);
            header.extensions = message.header_extensions(payload_len as u16);
            header.serialize(&mut header_stream)?;
        }
        Ok(&self.buffer[..MessageHeader::SERIALIZED_SIZE + payload_len])
    }
}

impl Default for MessageSerializer {
    fn default() -> Self {
        Self::new(ProtocolInfo::default())
    }
}
