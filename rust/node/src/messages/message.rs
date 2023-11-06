use super::*;
use crate::{stats::DetailType, utils::BlockUniquer, voting::VoteUniquer};
use anyhow::Result;
use bitvec::prelude::BitArray;
use rsnano_core::utils::{Serialize, Stream};
use std::{fmt::Display, ops::Deref};

pub trait MessageVariant: Serialize + Display + std::fmt::Debug {
    fn message_type(&self) -> MessageType;
    fn header_extensions(&self, _payload_len: u16) -> BitArray<u16> {
        Default::default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
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

impl Message {
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
                Message::Keepalive(KeepalivePayload::deserialize(&header, stream)?)
            }
            MessageType::Publish => Message::Publish(PublishPayload::deserialize(
                stream,
                &header,
                digest,
                block_uniquer,
            )?),
            MessageType::AscPullAck => {
                Message::AscPullAck(AscPullAckPayload::deserialize(stream, &header)?)
            }
            MessageType::AscPullReq => {
                Message::AscPullReq(AscPullReqPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPull => {
                Message::BulkPull(BulkPullPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPullAccount => {
                Message::BulkPullAccount(BulkPullAccountPayload::deserialize(stream, &header)?)
            }
            MessageType::BulkPush => {
                Message::BulkPush(BulkPushPayload::deserialize(stream, &header)?)
            }
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

impl Deref for Message {
    type Target = dyn MessageVariant;

    fn deref(&self) -> &Self::Target {
        match &self {
            Message::Keepalive(x) => x,
            Message::Publish(x) => x,
            Message::AscPullAck(x) => x,
            Message::AscPullReq(x) => x,
            Message::BulkPull(x) => x,
            Message::BulkPullAccount(x) => x,
            Message::BulkPush(x) => x,
            Message::ConfirmAck(x) => x,
            Message::ConfirmReq(x) => x,
            Message::FrontierReq(x) => x,
            Message::NodeIdHandshake(x) => x,
            Message::TelemetryAck(x) => x,
            Message::TelemetryReq(x) => x,
        }
    }
}

impl Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.deref(), f)
    }
}
