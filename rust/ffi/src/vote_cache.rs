use crate::utils::ContainerInfoComponentHandle;
use rsnano_core::{Account, Amount};
use rsnano_node::vote_cache::VoteCache;
use std::ffi::{c_char, c_void, CStr};

pub struct VoteCacheHandle(VoteCache);
pub type DeleteRepWeightQueryCallback = unsafe extern "C" fn(*mut c_void);
pub type ExecuteRepWeightQueryCallback = unsafe extern "C" fn(*mut c_void, *const u8, *mut u8);

struct FfiRepWeightQueryWrapper {
    handle: *mut c_void,
    execute_callback: ExecuteRepWeightQueryCallback,
    delete_callback: DeleteRepWeightQueryCallback,
}

impl FfiRepWeightQueryWrapper {
    pub fn execute(&self, rep: &Account) -> Amount {
        unsafe {
            let bytes = rep.as_bytes();
            let mut amount = [0u8; 16];
            (self.execute_callback)(self.handle, bytes.as_ptr(), amount.as_mut_ptr());
            Amount::from_be_bytes(amount)
        }
    }
}

impl Drop for FfiRepWeightQueryWrapper {
    fn drop(&mut self) {
        unsafe { (self.delete_callback)(self.handle) }
    }
}

#[no_mangle]
pub extern "C" fn rsn_vote_cache_create(
    max_size: usize,
    rep_weight_query_handle: *mut c_void,
    execute_rep_weight_query: ExecuteRepWeightQueryCallback,
    delete_rep_weight_query: DeleteRepWeightQueryCallback,
) -> *mut VoteCacheHandle {
    let rep_query_wrapper = FfiRepWeightQueryWrapper {
        handle: rep_weight_query_handle,
        execute_callback: execute_rep_weight_query,
        delete_callback: delete_rep_weight_query,
    };
    let rep_query = Box::new(move |rep: &_| rep_query_wrapper.execute(rep));
    Box::into_raw(Box::new(VoteCacheHandle(VoteCache::new(
        max_size, rep_query,
    ))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_destroy(handle: *mut VoteCacheHandle) {
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_vote_cache_collect_container_info(
    handle: *const VoteCacheHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info = (*handle)
        .0
        .collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
