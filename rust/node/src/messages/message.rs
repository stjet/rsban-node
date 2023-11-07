use super::*;
use crate::{stats::DetailType, utils::BlockUniquer, voting::VoteUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{BufferWriter, Serialize, Stream};
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

pub trait MessageVariant: Display + Serialize {
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
}

impl Message {
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

    pub fn as_message_variant(&self) -> Option<&dyn MessageVariant> {
        match &self {
            Message::Keepalive(x) => Some(x),
            Message::Publish(x) => Some(x),
            Message::AscPullAck(x) => Some(x),
            Message::AscPullReq(x) => Some(x),
            Message::BulkPull(x) => Some(x),
            Message::BulkPullAccount(x) => Some(x),
            Message::ConfirmAck(x) => Some(x),
            Message::ConfirmReq(x) => Some(x),
            Message::FrontierReq(x) => Some(x),
            Message::NodeIdHandshake(x) => Some(x),
            Message::TelemetryAck(x) => Some(x),
            _ => None,
        }
    }

    pub fn serialize(&self, stream: &mut dyn BufferWriter) {
        if let Some(variant) = self.as_message_variant() {
            variant.serialize_safe(stream);
        }
    }

    pub fn header_extensions(&self, payload_len: u16) -> BitArray<u16> {
        match self.as_message_variant() {
            Some(variant) => variant.header_extensions(payload_len),
            None => Default::default(),
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
        match self.as_message_variant() {
            Some(variant) => variant.fmt(f),
            None => Ok(()),
        }
    }
}
