use rsnano_node::voting::{Election, ElectionStatus};
use std::{
    ops::Deref,
    sync::{Arc, MutexGuard},
};

use crate::{copy_root_bytes, core::BlockHandle};

use super::election_status::ElectionStatusHandle;

pub struct ElectionHandle(Arc<Election>);

impl Deref for ElectionHandle {
    type Target = Arc<Election>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_election_create(block: &BlockHandle) -> *mut ElectionHandle {
    Box::into_raw(Box::new(ElectionHandle(Arc::new(Election::new(
        Arc::clone(block),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_destroy(handle: *mut ElectionHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock(handle: &ElectionHandle) -> *mut ElectionLockHandle {
    let guard = handle.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<MutexGuard<ElectionStatus>, MutexGuard<'static, ElectionStatus>>(
            guard,
        )
    };
    Box::into_raw(Box::new(ElectionLockHandle(Some(guard))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_election_root(handle: &ElectionHandle, result: *mut u8) {
    copy_root_bytes(handle.root, result);
}

pub struct ElectionLockHandle(Option<MutexGuard<'static, ElectionStatus>>);

#[no_mangle]
pub unsafe extern "C" fn rsn_election_lock_destroy(handle: *mut ElectionLockHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_status(
    handle: &ElectionLockHandle,
) -> *mut ElectionStatusHandle {
    Box::into_raw(Box::new(ElectionStatusHandle(
        handle.0.as_ref().unwrap().deref().clone(),
    )))
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_status_set(
    handle: &mut ElectionLockHandle,
    status: &ElectionStatusHandle,
) {
    let current = handle.0.as_mut().unwrap();
    **current = status.deref().clone();
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_unlock(handle: &mut ElectionLockHandle) {
    handle.0.take();
}

#[no_mangle]
pub extern "C" fn rsn_election_lock_lock(
    handle: &mut ElectionLockHandle,
    election: &ElectionHandle,
) {
    assert!(handle.0.is_none());
    let guard = election.mutex.lock().unwrap();
    let guard = unsafe {
        std::mem::transmute::<MutexGuard<ElectionStatus>, MutexGuard<'static, ElectionStatus>>(
            guard,
        )
    };
    handle.0 = Some(guard);
}
