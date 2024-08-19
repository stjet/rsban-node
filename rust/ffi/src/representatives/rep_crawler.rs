use rsnano_node::representatives::RepCrawler;
use std::{
    ffi::{c_char, CStr},
    ops::Deref,
    sync::Arc,
};

pub struct RepCrawlerHandle(pub Arc<RepCrawler>);

impl Deref for RepCrawlerHandle {
    type Target = Arc<RepCrawler>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_destroy(handle: *mut RepCrawlerHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_rep_crawler_keepalive(
    handle: &RepCrawlerHandle,
    address: *const c_char,
    port: u16,
) {
    let address = CStr::from_ptr(address).to_str().unwrap().to_string();
    handle.keepalive_or_connect(address, port);
}
