use super::*;
use crate::{stats::DetailType, utils::BlockUniquer, voting::VoteUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{Serialize, Stream};
use std::{fmt::Display, ops::Deref};

pub trait MessageVariant: Serialize + Display + std::fmt::Debug {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Keepalive(Keepalive),
    Publish(Publish),
    AscPullAck(AscPullAck),
    AscPullReq(AscPullReq),
    BulkPull(BulkPull),
    BulkPullAccount(BulkPullAccount),
    BulkPush,
    ConfirmAck(ConfirmAckPayload),
    ConfirmReq(ConfirmReqPayload),
    FrontierReq(FrontierReqPayload),
    NodeIdHandshake(NodeIdHandshakePayload),
    TelemetryAck(TelemetryAckPayload),
    TelemetryReq(TelemetryReqPayload),
}

impl Message {
    pub fn serialize(&self, stream: &mut dyn Stream) -> anyhow::Result<()> {
        match &self {
            Message::Keepalive(x) => x.serialize(stream),
            Message::Publish(x) => x.serialize(stream),
            Message::AscPullAck(x) => x.serialize(stream),
            Message::AscPullReq(x) => x.serialize(stream),
            Message::BulkPull(x) => x.serialize(stream),
            Message::BulkPullAccount(x) => x.serialize(stream),
            Message::ConfirmAck(x) => x.serialize(stream),
            Message::ConfirmReq(x) => x.serialize(stream),
            Message::FrontierReq(x) => x.serialize(stream),
            Message::NodeIdHandshake(x) => x.serialize(stream),
            Message::TelemetryAck(x) => x.serialize(stream),
            Message::TelemetryReq(x) => x.serialize(stream),
            Message::BulkPush => Ok(()),
        }
    }

    pub fn message_type(&self) -> MessageType {
        match &self {
            Message::Keepalive(_) => MessageType::Keepalive,
            Message::Publish(_) => MessageType::Publish,
            Message::AscPullAck(_) => MessageType::AscPullAck,
            Message::AscPullReq(_) => MessageType::AscPullReq,
            Message::BulkPull(_) => MessageType::BulkPull,
            Message::BulkPullAccount(_) => MessageType::BulkPullAccount,
            Message::BulkPush => MessageType::BulkPush,
            Message::ConfirmAck(_) => MessageType::ConfirmAck,
            Message::ConfirmReq(_) => MessageType::ConfirmReq,
            Message::FrontierReq(_) => MessageType::FrontierReq,
            Message::NodeIdHandshake(_) => MessageType::NodeIdHandshake,
            Message::TelemetryAck(_) => MessageType::TelemetryAck,
            Message::TelemetryReq(_) => MessageType::TelemetryReq,
        }
    }

    pub fn header_extensions(&self, payload_len: u16) -> BitArray<u16> {
        match &self {
            Message::Publish(x) => x.header_extensions(payload_len),
            Message::AscPullAck(x) => x.header_extensions(payload_len),
            Message::AscPullReq(x) => x.header_extensions(payload_len),
            Message::BulkPull(x) => x.header_extensions(payload_len),
            Message::ConfirmAck(x) => x.header_extensions(payload_len),
            Message::ConfirmReq(x) => x.header_extensions(payload_len),
            Message::FrontierReq(x) => x.header_extensions(payload_len),
            Message::NodeIdHandshake(x) => x.header_extensions(payload_len),
            Message::TelemetryAck(x) => x.header_extensions(payload_len),
            _ => Default::default(),
        }
    }

    pub fn deserialize(
        stream: &mut impl Stream,
        header: &MessageHeader,
        digest: u128,
        block_uniquer: Option<&BlockUniquer>,
        vote_uniquer: Option<&VoteUniquer>,
    ) -> Result<Self> {
        let msg = match header.message_type {
            MessageType::Keepalive => Message::Keepalive(Keepalive::deserialize(&header, stream)?),
            MessageType::Publish => Message::Publish(Publish::deserialize(
                stream,
                &header,
                digest,
                block_uniquer,
            )?),
            MessageType::AscPullAck => {
                Message::AscPullAck(AscPullAck::deserialize(stream, &header)?)
            }
            MessageType::AscPullReq => {
                Message::AscPullReq(AscPullReq::deserialize(stream, &header)?)
            }
            MessageType::BulkPull => Message::BulkPull(BulkPull::deserialize(stream, &header)?),
            MessageType::BulkPullAccount => {
                Message::BulkPullAccount(BulkPullAccount::deserialize(stream, &header)?)
            }
            MessageType::BulkPush => Message::BulkPush,
            MessageType::ConfirmAck => {
                Message::ConfirmAck(ConfirmAckPayload::deserialize(stream, vote_uniquer)?)
            }
            MessageType::ConfirmReq => Message::ConfirmReq(ConfirmReqPayload::deserialize(
                stream,
                &header,
                block_uniquer,
            )?),
            MessageType::FrontierReq => {
                Message::FrontierReq(FrontierReqPayload::deserialize(stream, &header)?)
            }
            MessageType::NodeIdHandshake => {
                Message::NodeIdHandshake(NodeIdHandshakePayload::deserialize(stream, &header)?)
            }
            MessageType::TelemetryAck => {
                Message::TelemetryAck(TelemetryAckPayload::deserialize(stream, &header)?)
            }
            MessageType::TelemetryReq => {
                Message::TelemetryReq(TelemetryReqPayload::deserialize(stream, &header)?)
            }
            MessageType::Invalid | MessageType::NotAType => bail!("invalid message type"),
        };
        Ok(msg)
    }
}

impl From<&Message> for DetailType {
    fn from(value: &Message) -> Self {
        value.message_type().into()
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.deref(), f)
    }
}
