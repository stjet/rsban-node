use rsnano_core::{BlockHash, QualifiedRoot};
use rsnano_node::consensus::RecentlyConfirmedCache;
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
};

use crate::utils::ContainerInfoComponentHandle;

pub struct RecentlyConfirmedCacheHandle(Arc<RecentlyConfirmedCache>);

impl RecentlyConfirmedCacheHandle {
    pub fn new(cache: Arc<RecentlyConfirmedCache>) -> *mut Self {
        Box::into_raw(Box::new(Self(cache)))
    }
}

impl Deref for RecentlyConfirmedCacheHandle {
    type Target = Arc<RecentlyConfirmedCache>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub extern "C" fn rsn_recently_confirmed_cache_create(
    max_len: usize,
) -> *mut RecentlyConfirmedCacheHandle {
    RecentlyConfirmedCacheHandle::new(Arc::new(RecentlyConfirmedCache::new(max_len)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_destroy(
    handle: *mut RecentlyConfirmedCacheHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_put(
    handle: &mut RecentlyConfirmedCacheHandle,
    root: *const u8,
    hash: *const u8,
) {
    handle.put(QualifiedRoot::from_ptr(root), BlockHash::from_ptr(hash));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_erase(
    handle: &mut RecentlyConfirmedCacheHandle,
    hash: *const u8,
) {
    handle.erase(&BlockHash::from_ptr(hash));
}

#[no_mangle]
pub extern "C" fn rsn_recently_confirmed_cache_clear(handle: &mut RecentlyConfirmedCacheHandle) {
    handle.clear();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_hash_exists(
    handle: &RecentlyConfirmedCacheHandle,
    hash: *const u8,
) -> bool {
    handle.hash_exists(&BlockHash::from_ptr(hash))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_root_exists(
    handle: &RecentlyConfirmedCacheHandle,
    root: *const u8,
) -> bool {
    handle.root_exists(&QualifiedRoot::from_ptr(root))
}

#[no_mangle]
pub extern "C" fn rsn_recently_confirmed_cache_len(handle: &RecentlyConfirmedCacheHandle) -> usize {
    handle.len()
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_back(
    handle: &RecentlyConfirmedCacheHandle,
    root_result: *mut u8,
    hash_result: *mut u8,
) {
    let (root, hash) = handle.back().unwrap();
    root.copy_bytes(root_result);
    hash.copy_bytes(hash_result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_confirmed_cache_collect_container_info(
    handle: &RecentlyConfirmedCacheHandle,
    name: *const c_char,
) -> *mut ContainerInfoComponentHandle {
    let container_info =
        handle.collect_container_info(CStr::from_ptr(name).to_str().unwrap().to_owned());
    Box::into_raw(Box::new(ContainerInfoComponentHandle(container_info)))
}
