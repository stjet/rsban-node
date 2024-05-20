use crate::{
    core::{BlockCallback, BlockHandle, BlockHashCallback},
    ledger::datastore::LedgerHandle,
    utils::{ContainerInfoComponentHandle, ContextWrapper},
    VoidPointerCallback,
};
use rsnano_core::{BlockEnum, BlockHash};
use rsnano_node::cementation::ConfirmingSet;
use std::{
    ffi::{c_char, c_void, CStr},
    ops::Deref,
    sync::Arc,
    time::Duration,
};

pub struct ConfirmingSetHandle(pub Arc<ConfirmingSet>);

impl Deref for ConfirmingSetHandle {
    type Target = Arc<ConfirmingSet>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_confirming_set_create(
    ledger: &LedgerHandle,
    batch_time_ms: u64,
) -> *mut ConfirmingSetHandle {
    Box::into_raw(Box::new(ConfirmingSetHandle(Arc::new(ConfirmingSet::new(
        Arc::clone(ledger),
        Duration::from_millis(batch_time_ms),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_destroy(handle: *mut ConfirmingSetHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_add_cemented_observer(
    handle: &mut ConfirmingSetHandle,
    callback: BlockCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move |block: &Arc<BlockEnum>| {
        let block_handle = Box::into_raw(Box::new(BlockHandle(Arc::new(block.deref().clone()))));
        callback(context_wrapper.get_context(), block_handle);
        drop(Box::from_raw(block_handle));
    });
    (*handle).0.add_cemented_observer(callback_wrapper);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_add_already_cemented_observer(
    handle: &mut ConfirmingSetHandle,
    callback: BlockHashCallback,
    context: *mut c_void,
    delete_context: VoidPointerCallback,
) {
    let context_wrapper = ContextWrapper::new(context, delete_context);
    let callback_wrapper = Box::new(move |block_hash: BlockHash| {
        callback(
            context_wrapper.get_context(),
            block_hash.as_bytes().as_ptr(),
        );
    });
    (*handle).0.add_already_cemented_observer(callback_wrapper);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_add(handle: &mut ConfirmingSetHandle, hash: *const u8) {
    handle.0.add(BlockHash::from_ptr(hash));
}

#[no_mangle]
pub extern "C" fn rsn_confirming_set_start(handle: &mut ConfirmingSetHandle) {
    handle.0.start();
}

#[no_mangle]
pub extern "C" fn rsn_confirming_set_stop(handle: &mut ConfirmingSetHandle) {
    handle.0.stop();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_exists(
    handle: &mut ConfirmingSetHandle,
    hash: *const u8,
) -> bool {
    handle.0.exists(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_len(handle: &mut ConfirmingSetHandle) -> usize {
    handle.0.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_confirming_set_collect_container_info(
    handle: &ConfirmingSetHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = handle
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
