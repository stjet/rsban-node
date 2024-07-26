use super::bootstrap_initiator::BootstrapInitiatorHandle;
use crate::{
    block_processing::BlockProcessorHandle, ledger::datastore::LedgerHandle,
    websocket::WebsocketListenerHandle, StringDto, StringHandle,
};
use num::FromPrimitive;
use rsnano_node::bootstrap::{BootstrapAttempt, BootstrapStrategy};
use std::{
    ffi::{CStr, CString},
    ops::Deref,
    os::raw::c_char,
    sync::{atomic::Ordering, Arc},
};

pub struct BootstrapAttemptHandle(Arc<BootstrapStrategy>);

impl BootstrapAttemptHandle {
    pub fn new(strategy: Arc<BootstrapStrategy>) -> *mut BootstrapAttemptHandle {
        Box::into_raw(Box::new(BootstrapAttemptHandle(strategy)))
    }
}

impl Deref for BootstrapAttemptHandle {
    type Target = Arc<BootstrapStrategy>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_create(
    websocket_server: *mut WebsocketListenerHandle,
    block_processor: *const BlockProcessorHandle,
    bootstrap_initiator: *const BootstrapInitiatorHandle,
    ledger: *const LedgerHandle,
    id: *const c_char,
    mode: u8,
    incremental_id: u64,
) -> *mut BootstrapAttemptHandle {
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let mode = FromPrimitive::from_u8(mode).unwrap();
    let websocket_server = if websocket_server.is_null() {
        None
    } else {
        Some(Arc::clone((*websocket_server).deref()))
    };
    let block_processor = Arc::downgrade(&*block_processor);
    let bootstrap_initiator = Arc::downgrade(&*bootstrap_initiator);
    let ledger = Arc::clone(&*ledger);
    BootstrapAttemptHandle::new(Arc::new(BootstrapStrategy::Other(
        BootstrapAttempt::new(
            websocket_server,
            block_processor,
            bootstrap_initiator,
            ledger,
            id_str.to_string(),
            mode,
            incremental_id,
        )
        .unwrap(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_destroy(handle: *mut BootstrapAttemptHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_stop(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_id(
    handle: *const BootstrapAttemptHandle,
    result: *mut StringDto,
) {
    let id = CString::new((*handle).0.attempt().id.as_str()).unwrap();
    let string_handle = Box::new(StringHandle(id));
    let result = &mut (*result);
    result.value = string_handle.0.as_ptr();
    result.handle = Box::into_raw(string_handle);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_total_blocks(
    handle: *const BootstrapAttemptHandle,
) -> u64 {
    (*handle).0.attempt().total_blocks.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_total_blocks_inc(
    handle: *const BootstrapAttemptHandle,
) {
    (*handle)
        .0
        .attempt()
        .total_blocks
        .fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pulling(
    handle: *const BootstrapAttemptHandle,
) -> u32 {
    (*handle).0.attempt().pulling.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pulling_inc(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.attempt().pulling.fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_started(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.attempt().started.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_stopped(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.attempt().stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_requeued_pulls(
    handle: *const BootstrapAttemptHandle,
) -> u32 {
    (*handle).0.attempt().requeued_pulls.load(Ordering::SeqCst)
}
