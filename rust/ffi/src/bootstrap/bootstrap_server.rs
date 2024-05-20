use rsnano_messages::{DeserializedMessage, Message};
use rsnano_node::bootstrap::BootstrapServer;
use std::{ffi::c_void, ops::Deref, sync::Arc};

use crate::{
    ledger::datastore::LedgerHandle, messages::MessageHandle, transport::ChannelHandle,
    utils::ContextWrapper, StatHandle, VoidPointerCallback,
};

pub struct BootstrapServerHandle(pub Arc<BootstrapServer>);

impl Deref for BootstrapServerHandle {
    type Target = Arc<BootstrapServer>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_bootstrap_server_create(
    stats: &StatHandle,
    ledger: &LedgerHandle,
) -> *mut BootstrapServerHandle {
    Box::into_raw(Box::new(BootstrapServerHandle(Arc::new(
        BootstrapServer::new(Arc::clone(stats), Arc::clone(ledger)),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_destroy(handle: *mut BootstrapServerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_start(handle: &BootstrapServerHandle) {
    handle.0.start();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_stop(handle: &BootstrapServerHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_request(
    handle: &BootstrapServerHandle,
    message: &MessageHandle,
    channel: &ChannelHandle,
) -> bool {
    let Message::AscPullReq(req) = &message.message else {
        panic!("wrong message type")
    };
    handle.0.request(req.clone(), Arc::clone(channel))
}

pub type BootstrapServerResponseCallback =
    unsafe extern "C" fn(*mut c_void, *mut MessageHandle, *mut ChannelHandle);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_server_set_callback(
    handle: &BootstrapServerHandle,
    callback: BootstrapServerResponseCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    handle
        .0
        .set_response_callback(Box::new(move |msg, channel| {
            let msg_handle = MessageHandle::new(DeserializedMessage::new(
                Message::AscPullAck(msg.clone()),
                Default::default(),
            ));

            let channel_handle = ChannelHandle::new(Arc::clone(channel));
            callback(context_wrapper.get_context(), msg_handle, channel_handle);
        }));
}
