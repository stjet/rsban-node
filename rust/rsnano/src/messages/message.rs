use super::{MessageHeader, MessageType};
use crate::NetworkConstants;
use std::any::Any;

pub trait Message {
    fn header(&self) -> &MessageHeader;
    fn set_header(&mut self, header: &MessageHeader);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
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
