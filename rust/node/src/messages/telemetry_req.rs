use super::MessageHeader;
use crate::messages::MessageType;
use rsnano_core::utils::{Serialize, Stream};
use std::fmt::Display;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TelemetryReqPayload;

impl TelemetryReqPayload {
    pub fn deserialize(_stream: &mut impl Stream, header: &MessageHeader) -> anyhow::Result<Self> {
        debug_assert!(header.message_type == MessageType::TelemetryReq);
        Ok(Self {})
    }
}

impl Serialize for TelemetryReqPayload {
    fn serialize(&self, _stream: &mut dyn Stream) -> anyhow::Result<()> {
        Ok(())
    }
}

impl Display for TelemetryReqPayload {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
