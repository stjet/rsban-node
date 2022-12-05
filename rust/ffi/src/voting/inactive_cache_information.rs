use crate::{copy_hash_bytes, voting::inactive_cache_status::InactiveCacheStatusHandle, StringDto};
use rsnano_core::{Account, BlockHash};
use rsnano_node::voting::InactiveCacheInformation;

pub struct InactiveCacheInformationHandle(InactiveCacheInformation);

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_create(
) -> *mut InactiveCacheInformationHandle {
    let info = InactiveCacheInformation::default();
    Box::into_raw(Box::new(InactiveCacheInformationHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_create1(
    arrival: i64,
    hash: *const u8,
    status: *const InactiveCacheStatusHandle,
    initial_rep: *const u8,
    initial_timestamp: u64,
) -> *mut InactiveCacheInformationHandle {
    let hash = BlockHash::from_ptr(hash);
    let status = (*status).0.clone();
    let initial_rep = Account::from_ptr(initial_rep);
    let info = InactiveCacheInformation::new(arrival, hash, status, initial_rep, initial_timestamp);
    Box::into_raw(Box::new(InactiveCacheInformationHandle(info)))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_clone(
    handle: *const InactiveCacheInformationHandle,
) -> *mut InactiveCacheInformationHandle {
    Box::into_raw(Box::new(InactiveCacheInformationHandle(
        (*handle).0.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_destroy(
    handle: *mut InactiveCacheInformationHandle,
) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_get_arrival(
    handle: *const InactiveCacheInformationHandle,
) -> i64 {
    (*handle).0.arrival
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_get_hash(
    handle: *const InactiveCacheInformationHandle,
    result: *mut u8,
) {
    copy_hash_bytes((*handle).0.hash, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_get_status(
    handle: *const InactiveCacheInformationHandle,
) -> *mut InactiveCacheStatusHandle {
    Box::into_raw(Box::new(InactiveCacheStatusHandle(
        (*handle).0.status.clone(),
    )))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_get_voters(
    handle: *const InactiveCacheInformationHandle,
    vector: *mut VotersDto,
) {
    let items: Vec<VotersItemDto> = (*handle)
        .0
        .voters
        .iter()
        .map(|(a, t)| VotersItemDto {
            account: *a.as_bytes(),
            timestamp: *t,
        })
        .collect();
    let raw_data = Box::new(VotersRawData(items));
    (*vector).items = raw_data.0.as_ptr();
    (*vector).count = raw_data.0.len();
    (*vector).raw_data = Box::into_raw(raw_data);
}

#[repr(C)]
pub struct VotersItemDto {
    account: [u8; 32],
    timestamp: u64,
}

pub struct VotersRawData(Vec<VotersItemDto>);

#[repr(C)]
pub struct VotersDto {
    items: *const VotersItemDto,
    count: usize,
    pub raw_data: *mut VotersRawData,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_destroy_dto(vector: *mut VotersDto) {
    drop(Box::from_raw((*vector).raw_data))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_inactive_cache_information_to_string(
    handle: *const InactiveCacheInformationHandle,
    result: *mut StringDto,
) {
    *result = (*handle).0.to_string().into();
}
