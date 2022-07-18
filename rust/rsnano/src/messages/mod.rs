use std::any::Any;

mod message_header;
pub use message_header::*;

mod node_id_handshake;
pub use node_id_handshake::*;

mod keepalive;
pub use keepalive::*;

mod publish;
pub use publish::*;

mod confirm_req;
pub use confirm_req::*;

mod confirm_ack;
pub use confirm_ack::*;

mod frontier_req;
pub use frontier_req::*;

mod bulk_pull;
pub use bulk_pull::*;

mod bulk_pull_account;
pub use bulk_pull_account::*;

mod bulk_push;
pub use bulk_push::*;

mod telemetry_req;
pub use telemetry_req::*;

mod telemetry_ack;
pub use telemetry_ack::*;

use crate::utils::{MemoryStream, Stream};
use anyhow::Result;

pub trait Message {
    fn header(&self) -> &MessageHeader;
    fn set_header(&mut self, header: &MessageHeader);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn serialize(&self, stream: &mut dyn Stream) -> Result<()>;
    fn visit(&self, visitor: &dyn MessageVisitor);
    fn clone_box(&self) -> Box<dyn Message>;
    fn message_type(&self) -> MessageType;
    fn to_bytes(&self) -> Vec<u8> {
        let mut stream = MemoryStream::new();
        self.serialize(&mut stream).unwrap();
        stream.to_vec()
    }
}

pub trait MessageVisitor {
    fn keepalive(&self, message: &Keepalive);
    fn publish(&self, message: &Publish);
    fn confirm_req(&self, message: &ConfirmReq);
    fn confirm_ack(&self, message: &ConfirmAck);
    fn bulk_pull(&self, message: &BulkPull);
    fn bulk_pull_account(&self, message: &BulkPullAccount);
    fn bulk_push(&self, message: &BulkPush);
    fn frontier_req(&self, message: &FrontierReq);
    fn node_id_handshake(&self, message: &NodeIdHandshake);
    fn telemetry_req(&self, message: &TelemetryReq);
    fn telemetry_ack(&self, message: &TelemetryAck);
}

pub trait MessageExt {
    fn to_bytes(&self) -> Vec<u8>;
}
