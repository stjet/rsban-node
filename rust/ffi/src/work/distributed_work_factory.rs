use super::work_pool::WorkPoolHandle;
use crate::{core::BlockHandle, NodeConfigDto, PeerDto};
use rsnano_node::work::{DistributedWorkFactory, MAKE_BLOCKING, MAKE_BLOCKING_2};
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
    node_config: &NodeConfigDto,
    work_pool: &WorkPoolHandle,
) -> *mut DistributedWorkFactoryHandle {
    Box::into_raw(Box::new(DistributedWorkFactoryHandle(Arc::new(
        DistributedWorkFactory::new(
            factory_pointer,
            node_config.try_into().unwrap(),
            Arc::clone(work_pool),
        ),
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_distributed_work_factory_destroy(
    handle: *mut DistributedWorkFactoryHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_distributed_work_factory_enabled(
    handle: &DistributedWorkFactoryHandle,
) -> bool {
    handle.work_generation_enabled()
}

#[no_mangle]
pub extern "C" fn rsn_distributed_work_factory_enabled_secondary(
    handle: &DistributedWorkFactoryHandle,
) -> bool {
    handle.work_generation_enabled_secondary()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_distributed_work_factory_enabled_peers(
    handle: &DistributedWorkFactoryHandle,
    peers: *const PeerDto,
    len: usize,
) -> bool {
    let peers = std::slice::from_raw_parts(peers, len);
    handle.work_generation_enabled_peers(&peers.iter().map(|i| i.into()).collect::<Vec<_>>())
}

pub type WorkMakeBlockingCallback =
    unsafe extern "C" fn(*mut c_void, *mut BlockHandle, u64, *mut u64) -> bool;

pub type WorkMakeBlocking2Callback =
    unsafe extern "C" fn(*mut c_void, u8, *const u8, u64, bool, *const u8, &mut u64) -> bool;

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_work_make_blocking(f: WorkMakeBlockingCallback) {
    MAKE_BLOCKING_WRAPPER = Some(f);
    MAKE_BLOCKING = Some(|factory_pointer, block, difficulty| {
        let block_handle = BlockHandle::new(Arc::new(block.clone()));
        let mut work = 0;
        if MAKE_BLOCKING_WRAPPER.unwrap()(factory_pointer, block_handle, difficulty, &mut work) {
            block.set_work(work);
            Some(work)
        } else {
            None
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn rsn_callback_work_make_blocking_2(f: WorkMakeBlocking2Callback) {
    MAKE_BLOCKING_2_WRAPPER = Some(f);
    MAKE_BLOCKING_2 = Some(|factory_pointer, version, root, difficulty, account| {
        let mut work = 0;
        let account_raw = account.unwrap_or_default();
        if MAKE_BLOCKING_2_WRAPPER.unwrap()(
            factory_pointer,
            version as u8,
            root.as_bytes().as_ptr(),
            difficulty,
            account.is_some(),
            account_raw.as_bytes().as_ptr(),
            &mut work,
        ) {
            Some(work)
        } else {
            None
        }
    });
}

pub static mut MAKE_BLOCKING_WRAPPER: Option<WorkMakeBlockingCallback> = None;
pub static mut MAKE_BLOCKING_2_WRAPPER: Option<WorkMakeBlocking2Callback> = None;
