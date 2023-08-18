mod stream;
use std::{
    ffi::c_void,
    net::{IpAddr, Ipv6Addr, SocketAddr},
};

use rsnano_node::utils::{ipv4_address_or_ipv6_subnet, map_address_to_subnetwork};
pub use stream::FfiStream;

mod toml;
pub use toml::FfiToml;

mod thread_pool;
pub use thread_pool::{ThreadPoolHandle, VoidFnCallbackHandle};
mod io_context;
pub use io_context::{DispatchCallback, FfiIoContext, IoContextHandle};
mod logger_mt;
pub use logger_mt::*;

mod timer;
pub use timer::*;

mod atomics;
pub use atomics::*;

mod latch;
pub use latch::*;

mod container_info;
pub use container_info::*;

use crate::{transport::EndpointDto, VoidPointerCallback};

pub struct ContextWrapper {
    context: *mut c_void,
    drop_context: VoidPointerCallback,
}

impl ContextWrapper {
    pub fn new(context: *mut c_void, drop_context: VoidPointerCallback) -> Self {
        Self {
            context,
            drop_context,
        }
    }

    pub fn get_context(&self) -> *mut c_void {
        self.context
    }
}

unsafe impl Send for ContextWrapper {}
unsafe impl Sync for ContextWrapper {}

impl Drop for ContextWrapper {
    fn drop(&mut self) {
        unsafe {
            (self.drop_context)(self.context);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_map_address_to_subnetwork(ipv6_bytes: *const u8, result: *mut u8) {
    let input = ptr_into_ipv6addr(ipv6_bytes);
    let output = map_address_to_subnetwork(input);
    copy_ipv6addr_bytes(output, result);
}

#[no_mangle]
pub unsafe extern "C" fn rsn_ipv4_address_or_ipv6_subnet(ipv6_bytes: *const u8, result: *mut u8) {
    let input = ptr_into_ipv6addr(ipv6_bytes);
    let output = ipv4_address_or_ipv6_subnet(input);
    copy_ipv6addr_bytes(output, result);
}

pub(crate) unsafe fn copy_ipv6addr_bytes(res: Ipv6Addr, target: *mut u8) {
    let result_slice = std::slice::from_raw_parts_mut(target, 16);
    result_slice.copy_from_slice(&res.octets());
}

pub(crate) unsafe fn ptr_into_ipv6addr(ipv6_bytes: *const u8) -> Ipv6Addr {
    let octets: [u8; 16] = std::slice::from_raw_parts(ipv6_bytes, 16)
        .try_into()
        .unwrap();
    Ipv6Addr::from(octets)
}
