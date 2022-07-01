use super::MessageHandle;
use crate::{ffi::DestroyCallback, messages::*};
use std::ffi::c_void;
use MessageVisitor;

type MessageVisitorCallback = unsafe extern "C" fn(*mut c_void, *mut MessageHandle, u8);
static mut MESSAGE_VISITOR_VISIT: Option<MessageVisitorCallback> = None;
static mut MESSAGE_VISITOR_DESTROY: Option<DestroyCallback> = None;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_message_visitor_visit(f: MessageVisitorCallback) {
    MESSAGE_VISITOR_VISIT = Some(f);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_message_visitor_destroy(f: DestroyCallback) {
    MESSAGE_VISITOR_DESTROY = Some(f);
}

pub(crate) struct FfiMessageVisitor {
    handle: *mut c_void,
}

impl FfiMessageVisitor {
    pub(crate) fn new(handle: *mut c_void) -> Self {
        Self { handle }
    }

    fn visit_callback<T>(&self, message: &T)
    where
        T: 'static + Message + Clone,
    {
        let message_handle = MessageHandle::new(message.clone());
        unsafe {
            match MESSAGE_VISITOR_VISIT {
                Some(f) => f(
                    self.handle,
                    message_handle,
                    message.header().message_type() as u8,
                ),
                None => panic!("MESSAGE_VISITOR_CALLBACK missing"),
            }
        }
    }
}

impl MessageVisitor for FfiMessageVisitor {
    fn keepalive(&self, message: &Keepalive) {
        self.visit_callback(message);
    }

    fn publish(&self, message: &Publish) {
        self.visit_callback(message);
    }

    fn confirm_req(&self, message: &ConfirmReq) {
        self.visit_callback(message);
    }

    fn confirm_ack(&self, message: &ConfirmAck) {
        self.visit_callback(message);
    }

    fn bulk_pull(&self, message: &BulkPull) {
        self.visit_callback(message);
    }

    fn bulk_pull_account(&self, message: &BulkPullAccount) {
        self.visit_callback(message);
    }

    fn bulk_push(&self, message: &BulkPush) {
        self.visit_callback(message);
    }

    fn frontier_req(&self, message: &FrontierReq) {
        self.visit_callback(message);
    }

    fn node_id_handshake(&self, message: &NodeIdHandshake) {
        self.visit_callback(message);
    }

    fn telemetry_req(&self, message: &TelemetryReq) {
        self.visit_callback(message);
    }

    fn telemetry_ack(&self, message: &TelemetryAck) {
        self.visit_callback(message);
    }
}

impl Drop for FfiMessageVisitor {
    fn drop(&mut self) {
        unsafe {
            match MESSAGE_VISITOR_DESTROY {
                Some(f) => f(self.handle),
                None => panic!("MESSAGE_VISITOR_DESTROY missing"),
            }
        }
    }
}
