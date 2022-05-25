use std::{
    ffi::{c_void, CStr, CString},
    os::raw::c_char,
    sync::{atomic::Ordering, Arc, MutexGuard},
    time::Duration,
};

use num::FromPrimitive;

use crate::{
    websocket::{Listener, NullListener},
    Account, BootstrapAttempt,
};

use crate::ffi::{
    BlockHandle, BlockProcessorHandle, FfiListener, LedgerHandle, LoggerMT, StringDto, StringHandle,
};

use super::bootstrap_initiator::BootstrapInitiatorHandle;

pub struct BootstrapAttemptHandle(BootstrapAttempt);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_create(
    logger: *mut c_void,
    websocket_server: *mut c_void,
    block_processor: *const BlockProcessorHandle,
    bootstrap_initiator: *const BootstrapInitiatorHandle,
    ledger: *const LedgerHandle,
    id: *const c_char,
    mode: u8,
    incremental_id: u64,
) -> *mut BootstrapAttemptHandle {
    let logger = Arc::new(LoggerMT::new(logger));
    let id_str = CStr::from_ptr(id).to_str().unwrap();
    let mode = FromPrimitive::from_u8(mode).unwrap();
    let websocket_server: Arc<dyn Listener> = if websocket_server.is_null() {
        Arc::new(NullListener::new())
    } else {
        Arc::new(FfiListener::new(websocket_server))
    };
    let block_processor = Arc::downgrade(&*block_processor);
    let bootstrap_initiator = Arc::downgrade(&*bootstrap_initiator);
    let ledger = Arc::clone(&*ledger);
    Box::into_raw(Box::new(BootstrapAttemptHandle(
        BootstrapAttempt::new(
            logger,
            websocket_server,
            block_processor,
            bootstrap_initiator,
            ledger,
            id_str,
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
    let id = CString::new((*handle).0.id.as_str()).unwrap();
    let string_handle = Box::new(StringHandle(id));
    let result = &mut (*result);
    result.value = string_handle.0.as_ptr();
    result.handle = Box::into_raw(string_handle);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_should_log(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.should_log()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_bootstrap_mode(
    handle: *const BootstrapAttemptHandle,
    len: *mut usize,
) -> *const c_char {
    let mode_text = (*handle).0.mode_text();
    *len = mode_text.len();
    mode_text.as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_total_blocks(
    handle: *const BootstrapAttemptHandle,
) -> u64 {
    (*handle).0.total_blocks.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_total_blocks_inc(
    handle: *const BootstrapAttemptHandle,
) {
    (*handle).0.total_blocks.fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_process_block(
    handle: *const BootstrapAttemptHandle,
    block: *const BlockHandle,
    known_account: *const u8,
    pull_blocks_processed: u64,
    max_blocks: u32,
    block_expected: bool,
    retry_limit: u32,
) -> bool {
    let block = (*block).block.clone();
    (*handle).0.process_block(
        block,
        &Account::from(known_account),
        pull_blocks_processed,
        max_blocks,
        block_expected,
        retry_limit,
    )
}

pub struct LockHandle(Option<MutexGuard<'static, u8>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_lock(
    handle: *mut BootstrapAttemptHandle,
) -> *mut LockHandle {
    let guard = (*handle).0.mutex.lock().unwrap();
    Box::into_raw(Box::new(LockHandle(Some(std::mem::transmute::<
        MutexGuard<u8>,
        MutexGuard<'static, u8>,
    >(guard)))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_unlock(handle: *mut LockHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_notifiy_all(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.condition.notify_all();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_notifiy_one(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.condition.notify_one();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_wait(
    handle: *mut BootstrapAttemptHandle,
    lck: *mut LockHandle,
) {
    let guard = (*handle)
        .0
        .condition
        .wait((*lck).0.take().unwrap())
        .unwrap();
    (*lck).0 = Some(std::mem::transmute::<MutexGuard<u8>, MutexGuard<'static, u8>>(guard));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_wait_for(
    handle: *mut BootstrapAttemptHandle,
    lck: *mut LockHandle,
    timeout_millis: u64,
) {
    let (guard, _) = (*handle)
        .0
        .condition
        .wait_timeout(
            (*lck).0.take().unwrap(),
            Duration::from_millis(timeout_millis),
        )
        .unwrap();
    (*lck).0 = Some(std::mem::transmute::<MutexGuard<u8>, MutexGuard<'static, u8>>(guard));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_incremental_id(
    handle: *const BootstrapAttemptHandle,
) -> u64 {
    (*handle).0.incremental_id
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pulling(
    handle: *const BootstrapAttemptHandle,
) -> u32 {
    (*handle).0.pulling.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pulling_inc(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.pulling.fetch_add(1, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pull_started(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.pull_started();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_pull_finished(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.pull_finished();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_stopped(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_set_stopped(handle: *mut BootstrapAttemptHandle) {
    (*handle).0.set_stopped()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_bootstrap_attempt_still_pulling(
    handle: *const BootstrapAttemptHandle,
) -> bool {
    (*handle).0.still_pulling()
}
