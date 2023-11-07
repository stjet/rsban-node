use super::*;
use crate::{stats::DetailType, utils::BlockUniquer, voting::VoteUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{Serialize, Stream};
use std::fmt::Display;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Keepalive(Keepalive),
    Publish(Publish),
    AscPullAck(AscPullAck),
    AscPullReq(AscPullReq),
    BulkPull(BulkPull),
    BulkPullAccount(BulkPullAccount),
    BulkPush,
    ConfirmAck(ConfirmAck),
    ConfirmReq(ConfirmReq),
    FrontierReq(FrontierReq),
    NodeIdHandshake(NodeIdHandshake),
    TelemetryAck(TelemetryAck),
    TelemetryReq,
}

pub trait MessageHeaderExtender {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
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
            _ => Ok(()),
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
            Message::TelemetryReq => MessageType::TelemetryReq,
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
            MessageType::Keepalive => Message::Keepalive(Keepalive::deserialize(stream)?),
            MessageType::Publish => Message::Publish(Publish::deserialize(
                stream,
                header.extensions,
                digest,
                block_uniquer,
            )?),
            MessageType::AscPullAck => Message::AscPullAck(AscPullAck::deserialize(stream)?),
            MessageType::AscPullReq => Message::AscPullReq(AscPullReq::deserialize(stream)?),
            MessageType::BulkPull => {
                Message::BulkPull(BulkPull::deserialize(stream, header.extensions)?)
            }
            MessageType::BulkPullAccount => {
                Message::BulkPullAccount(BulkPullAccount::deserialize(stream)?)
            }
            MessageType::BulkPush => Message::BulkPush,
            MessageType::ConfirmAck => {
                Message::ConfirmAck(ConfirmAck::deserialize(stream, vote_uniquer)?)
            }
            MessageType::ConfirmReq => Message::ConfirmReq(ConfirmReq::deserialize(
                stream,
                header.extensions,
                block_uniquer,
            )?),
            MessageType::FrontierReq => {
                Message::FrontierReq(FrontierReq::deserialize(stream, header.extensions)?)
            }
            MessageType::NodeIdHandshake => {
                Message::NodeIdHandshake(NodeIdHandshake::deserialize(stream, header.extensions)?)
            }
            MessageType::TelemetryAck => {
                Message::TelemetryAck(TelemetryAck::deserialize(stream, header.extensions)?)
            }
            MessageType::TelemetryReq => Message::TelemetryReq,
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
        match &self {
            Message::Keepalive(x) => x.fmt(f),
            Message::Publish(x) => x.fmt(f),
            Message::AscPullAck(x) => x.fmt(f),
            Message::AscPullReq(x) => x.fmt(f),
            Message::BulkPull(x) => x.fmt(f),
            Message::BulkPullAccount(x) => x.fmt(f),
            Message::ConfirmAck(x) => x.fmt(f),
            Message::ConfirmReq(x) => x.fmt(f),
            Message::FrontierReq(x) => x.fmt(f),
            Message::NodeIdHandshake(x) => x.fmt(f),
            Message::TelemetryAck(x) => x.fmt(f),
            _ => Ok(()),
        }
    }
}
