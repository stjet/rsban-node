use std::sync::Arc;

use crate::NetworkFilter;

pub struct NetworkFilterHandle(Arc<NetworkFilter>);

#[no_mangle]
pub extern "C" fn rsn_network_filter_create(size: usize) -> *mut NetworkFilterHandle {
    Box::into_raw(Box::new(NetworkFilterHandle(Arc::new(NetworkFilter::new(
        size,
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_destroy(handle: *mut NetworkFilterHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_apply(
    handle: *mut NetworkFilterHandle,
    bytes: *const u8,
    size: usize,
    digest: *mut u8,
) -> bool {
    let (calc_digest, existed) = (*handle).0.apply(std::slice::from_raw_parts(bytes, size));
    if !digest.is_null() {
        std::slice::from_raw_parts_mut(digest, 16).copy_from_slice(&calc_digest.to_be_bytes());
    }
    existed
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_clear(
    handle: *mut NetworkFilterHandle,
    digest: *const [u8; 16],
) {
    let digest = u128::from_be_bytes(*digest);
    (*handle).0.clear(digest);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_clear_many(
    handle: *mut NetworkFilterHandle,
    digests: *const [u8; 16],
    count: usize,
) {
    let digests = std::slice::from_raw_parts(digests, count)
        .iter()
        .map(|bytes| u128::from_be_bytes(*bytes));
    (*handle).0.clear_many(digests);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_clear_bytes(
    handle: *mut NetworkFilterHandle,
    bytes: *const u8,
    count: usize,
) {
    let bytes = std::slice::from_raw_parts(bytes, count);
    (*handle).0.clear_bytes(bytes);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_clear_all(handle: *mut NetworkFilterHandle) {
    (*handle).0.clear_all();
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_filter_hash(
    handle: *mut NetworkFilterHandle,
    bytes: *const u8,
    count: usize,
    digest: *mut [u8; 16],
) {
    let bytes = std::slice::from_raw_parts(bytes, count);
    let result = (*handle).0.hash(bytes);
    (*digest) = result.to_be_bytes();
}
