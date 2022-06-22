use std::any::Any;

use crate::NetworkConstants;

use super::{MessageHeader, MessageType};

pub trait Message {
    fn header(&self) -> &MessageHeader;
    fn set_header(&mut self, header: &MessageHeader);
    fn as_any(&self) -> &dyn Any;
}

#[derive(Clone)]
pub struct Keepalive {
    header: MessageHeader,
}

impl Keepalive {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::Keepalive),
        }
    }

    pub fn with_version_using(constants: &NetworkConstants, version_using: u8) -> Self {
        Self {
            header: MessageHeader::with_version_using(
                constants,
                MessageType::Keepalive,
                version_using,
            ),
        }
    }

    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
    }
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
}

#[derive(Clone)]
pub struct Publish {
    header: MessageHeader,
}

impl Publish {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::Publish),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
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
}

#[derive(Clone)]
pub struct ConfirmReq {
    header: MessageHeader,
}

impl ConfirmReq {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::ConfirmReq),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
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
}

#[derive(Clone)]
pub struct ConfirmAck {
    header: MessageHeader,
}

impl ConfirmAck {
    pub fn new(constants: &NetworkConstants) -> Self {
        Self {
            header: MessageHeader::new(constants, MessageType::ConfirmAck),
        }
    }
    pub fn with_header(header: &MessageHeader) -> Self {
        Self {
            header: header.clone(),
        }
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
}
