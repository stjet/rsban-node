use crate::consensus::election_status::ElectionStatusHandle;

pub struct RecentlyCementedCachedRawData(pub Vec<*mut ElectionStatusHandle>);

#[repr(C)]
pub struct RecentlyCementedCachedDto {
    pub items: *const *mut ElectionStatusHandle,
    pub count: usize,
    pub raw_data: *mut RecentlyCementedCachedRawData,
}

#[no_mangle]
pub unsafe extern "C" fn rsn_recently_cemented_cache_destroy_dto(
    list: *mut RecentlyCementedCachedDto,
) {
    drop(Box::from_raw((*list).raw_data))
}
