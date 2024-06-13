use super::EndpointDto;
use rsnano_node::transport::SynCookies;
use std::{net::SocketAddrV6, ops::Deref, sync::Arc};

pub struct SynCookiesHandle(pub Arc<SynCookies>);

impl Deref for SynCookiesHandle {
    type Target = Arc<SynCookies>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_syn_cookies_destroy(handle: *mut SynCookiesHandle) {
    drop(Box::from_raw(handle))
}

#[no_mangle]
pub unsafe extern "C" fn rsn_syn_cookies_cookie(
    handle: *mut SynCookiesHandle,
    endpoint: *const EndpointDto,
    result: *mut u8,
) -> bool {
    let endpoint = SocketAddrV6::from(&*endpoint);
    match (*handle).0.cookie(&endpoint) {
        Some(cookie) => {
            let result = std::slice::from_raw_parts_mut(result, 32);
            result.copy_from_slice(&cookie);
            true
        }
        None => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_syn_cookies_cookies_count(handle: *mut SynCookiesHandle) -> usize {
    (*handle).0.cookies_count()
}
