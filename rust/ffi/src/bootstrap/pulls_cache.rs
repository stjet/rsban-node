use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use rsnano_core::{BlockHash, HashOrAccount};
use rsnano_node::bootstrap::{PullInfo, PullsCache};

pub struct PullsCacheHandle(Arc<Mutex<PullsCache>>);

impl PullsCacheHandle {
    pub fn new(cache: Arc<Mutex<PullsCache>>) -> *mut Self {
        Box::into_raw(Box::new(Self(cache)))
    }
}

impl Deref for PullsCacheHandle {
    type Target = Arc<Mutex<PullsCache>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
pub struct PullInfoDto {
    account_or_head: [u8; 32],
    head: [u8; 32],
    head_original: [u8; 32],
    end: [u8; 32],
    count: u32,
    attempts: u32,
    processed: u64,
    retry_limit: u32,
    bootstrap_id: u64,
}

impl From<&PullInfoDto> for PullInfo {
    fn from(dto: &PullInfoDto) -> Self {
        PullInfo {
            account_or_head: HashOrAccount::from_bytes(dto.account_or_head),
            head: BlockHash::from_bytes(dto.head),
            head_original: BlockHash::from_bytes(dto.head_original),
            end: BlockHash::from_bytes(dto.end),
            count: dto.count,
            attempts: dto.attempts,
            processed: dto.processed,
            retry_limit: dto.retry_limit,
            bootstrap_id: dto.bootstrap_id,
        }
    }
}

impl From<&PullInfo> for PullInfoDto {
    fn from(pull: &PullInfo) -> Self {
        Self {
            account_or_head: *pull.account_or_head.as_bytes(),
            head: *pull.head.as_bytes(),
            head_original: *pull.head_original.as_bytes(),
            end: *pull.end.as_bytes(),
            count: pull.count,
            attempts: pull.attempts,
            processed: pull.processed,
            retry_limit: pull.retry_limit,
            bootstrap_id: pull.bootstrap_id,
        }
    }
}

#[no_mangle]
pub extern "C" fn rsn_pulls_cache_create() -> *mut PullsCacheHandle {
    Box::into_raw(Box::new(PullsCacheHandle(Arc::new(Mutex::new(
        PullsCache::new(),
    )))))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pulls_cache_destroy(handle: *mut PullsCacheHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pulls_cache_size(handle: *mut PullsCacheHandle) -> usize {
    (*handle).0.lock().unwrap().size()
}

#[no_mangle]
pub extern "C" fn rsn_pulls_cache_element_size() -> usize {
    PullsCache::ELEMENT_SIZE
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pulls_cache_add(
    handle: *mut PullsCacheHandle,
    pull: *const PullInfoDto,
) {
    (*handle).0.lock().unwrap().add(&PullInfo::from(&*pull));
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pulls_cache_update_pull(
    handle: *mut PullsCacheHandle,
    pull: *mut PullInfoDto,
) {
    let mut p = PullInfo::from(&*pull);
    (*handle).0.lock().unwrap().update_pull(&mut p);
    *pull = PullInfoDto::from(&p);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_pulls_cache_remove(
    handle: *mut PullsCacheHandle,
    pull: *const PullInfoDto,
) {
    (*handle).0.lock().unwrap().remove(&PullInfo::from(&*pull));
}
