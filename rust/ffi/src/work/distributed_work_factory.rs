use crate::core::BlockHandle;
use rsnano_core::work::{DistributedWorkFactory, MAKE_BLOCKING};
use std::{ffi::c_void, ops::Deref, sync::Arc};

pub struct DistributedWorkFactoryHandle(Arc<DistributedWorkFactory>);

impl Deref for DistributedWorkFactoryHandle {
    type Target = Arc<DistributedWorkFactory>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_distributed_work_factory_create(
    factory_pointer: *mut c_void,
) -> *mut DistributedWorkFactoryHandle {
    Box::into_raw(Box::new(DistributedWorkFactoryHandle(Arc::new(
        DistributedWorkFactory::new(factory_pointer),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_distributed_work_factory_destroy(
    handle: *mut DistributedWorkFactoryHandle,
) {
    drop(Box::from_raw(handle))
}

pub type WorkMakeBlockingCallback =
    unsafe extern "C" fn(*mut c_void, *mut BlockHandle, u64, *mut u64) -> bool;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_work_make_blocking(f: WorkMakeBlockingCallback) {
    MAKE_BLOCKING_WRAPPER = Some(f);
    MAKE_BLOCKING = Some(|factory_pointer, block, difficulty| {
        let block_handle = BlockHandle::new(Arc::new(block.clone()));
        let mut work = 0;
        if MAKE_BLOCKING_WRAPPER.unwrap()(factory_pointer, block_handle, difficulty, &mut work) {
            Some(work)
        } else {
            None
        }
    });
}

pub static mut MAKE_BLOCKING_WRAPPER: Option<WorkMakeBlockingCallback> = None;
