use std::{ffi::CStr, os::raw::c_char};

use num::FromPrimitive;

use crate::config::NetworkConstants;

mod work_thresholds;

#[repr(C)]
pub struct NetworkConstantsDto {
    pub current_network: u16,
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_active_network() -> u16 {
    NetworkConstants::active_network() as u16
}

#[no_mangle]
pub extern "C" fn rsn_network_constants_active_network_set(network: u16) {
    if let Some(net) = FromPrimitive::from_u16(network) {
        NetworkConstants::set_active_network(net);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsn_network_constants_active_network_set_str(
    network: *const c_char,
) -> i32 {
    let network = CStr::from_ptr(network).to_string_lossy();
    match NetworkConstants::set_active_network_from_str(network) {
        Ok(_) => 0,
        Err(_) => -1,
    }
}
